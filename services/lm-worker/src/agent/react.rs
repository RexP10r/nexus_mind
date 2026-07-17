use std::sync::Arc;
use tracing::info;

use async_trait::async_trait;

use crate::agent::{AgentResult, Message};
use crate::error::WorkerError;
use crate::grpc::lm_service::{ChatMessage, MessageRole};
use crate::traits::agent::Agent;
use crate::traits::llm::LlmProvider;
use crate::traits::GenerationParams;

const REACT_SYSTEM_PROMPT: &str =
    r#"You are a helpful assistant that uses step-by-step reasoning."#;

pub struct ReactAgent {
    llm: Arc<dyn LlmProvider>,
}

impl ReactAgent {
    pub fn new(llm: Arc<dyn LlmProvider>) -> Self {
        Self { llm }
    }

    fn to_proto_messages(&self, messages: &[Message], system_prompt: &str) -> Vec<ChatMessage> {
        let mut proto_msgs: Vec<ChatMessage> = Vec::with_capacity(messages.len() + 1);

        proto_msgs.push(ChatMessage {
            role: MessageRole::RoleSystem as i32,
            content: system_prompt.to_string(),
        });

        for msg in messages {
            let role = match msg.role.as_str() {
                "system" => MessageRole::RoleSystem,
                "user" => MessageRole::RoleUser,
                "assistant" => MessageRole::RoleAssistant,
                _ => MessageRole::RoleUser,
            };
            proto_msgs.push(ChatMessage {
                role: role as i32,
                content: msg.content.clone(),
            });
        }

        proto_msgs
    }
}

#[async_trait]
impl Agent for ReactAgent {
    async fn run(
        &self,
        messages: &[Message],
        params: &GenerationParams,
    ) -> Result<AgentResult, WorkerError> {
        let system_prompt = REACT_SYSTEM_PROMPT.to_string();

        let proto_messages = self.to_proto_messages(&messages, &system_prompt);
        let response = self
            .llm
            .generate(proto_messages, params)
            .await
            .map_err(|e| WorkerError::LlmProvider(e.to_string()))?;

        let output = response.text.clone();
        info!(
            "Model output ({} chars): {}",
            output.len(),
            &output[..output.len().min(200)]
        );
        Ok(AgentResult {
            final_answer: output,
        })
    }
}
