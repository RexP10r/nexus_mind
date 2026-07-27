use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use super::agent_loop::AgentLoop;
use super::state::AgentState;
use super::tool_handler::ToolHandler;
use crate::agent::Message;
use crate::common::tools::registry::InMemoryToolRegistry;
use crate::common::traits::agent::Agent;
use crate::common::traits::llm::LlmProvider;
use crate::common::GenerationParams;
use crate::error::WorkerError;

pub struct RAGAgent {
    llm: Arc<dyn LlmProvider>,
    tool_registry: InMemoryToolRegistry,
    max_iterations: u32,
    request_timeout: Duration,
}

impl RAGAgent {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        tool_registry: InMemoryToolRegistry,
        max_iterations: u32,
        request_timeout_secs: u64,
    ) -> Self {
        Self {
            llm,
            tool_registry,
            max_iterations,
            request_timeout: Duration::from_secs(request_timeout_secs),
        }
    }
}

#[async_trait]
impl Agent for RAGAgent {
    #[tracing::instrument(skip(self, messages, params),
        fields(message_count = messages.len())
    )]
    async fn run(
        &self,
        messages: &[Message],
        params: &GenerationParams,
    ) -> Result<crate::agent::AgentResult, WorkerError> {
        for msg in messages {
            tracing::info!(
                role = %msg.role,
                content = %msg.content,
                "User message"
            );
        }
        let state = AgentState::new(messages);
        let tool_handler = ToolHandler::new(&self.tool_registry);
        let agent_loop = AgentLoop::new(
            Arc::clone(&self.llm),
            tool_handler,
            self.max_iterations,
            self.request_timeout,
        );
        agent_loop.execute(state, params).await
    }
}
