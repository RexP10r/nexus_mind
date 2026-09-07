use std::sync::Arc;
use std::time::Duration;

use super::prompt::build_system_prompt;
use super::response_handler::ResponseHandler;
use super::schema::extract_llm_response;
use super::state::AgentState;
use super::tool_handler::ToolHandler;
use crate::agent::rag::schema::ReActAgentResponse;
use crate::error::WorkerError;
use crate::model::{build_chat_context, AgentResult, GenerationParams};
use crate::traits::llm::LlmProvider;

pub(crate) struct AgentLoop<'a> {
    llm: Arc<dyn LlmProvider>,
    tool_handler: ToolHandler<'a>,
    request_timeout: Duration,
}

impl<'a> AgentLoop<'a> {
    pub(crate) fn new(
        llm: Arc<dyn LlmProvider>,
        tool_handler: ToolHandler<'a>,
        request_timeout: Duration,
    ) -> Self {
        Self {
            llm,
            tool_handler,
            request_timeout,
        }
    }

    pub(crate) async fn execute(
        &self,
        mut state: AgentState,
        params: &GenerationParams,
        summary: Option<&str>,
    ) -> Result<AgentResult, WorkerError> {
        let system_prompt = build_system_prompt(&self.tool_handler.descriptions(), summary);

        loop {


            let response_text = self.call_llm(&mut state, &system_prompt, params).await?;

            match self.process_llm_response(&mut state, &response_text).await {
                Ok(Some(result)) => return Ok(result),
                Err(e) => return Err(e),
                _ => {}
            }
        }
    }

    async fn call_llm(
        &self,
        state: &mut AgentState,
        system_prompt: &str,
        params: &GenerationParams,
    ) -> Result<String, WorkerError> {
        let chat_messages = build_chat_context(&state.conversation, &state.reasoning_steps, system_prompt);

        let llm_start = std::time::Instant::now();

        let response = tokio::time::timeout(
            self.request_timeout,
            self.llm.generate(chat_messages, params),
        )
        .await
        .map_err(|_| {
            tracing::error!(
                timeout_secs = self.request_timeout.as_secs(),
                "LLM request timed out"
            );
            WorkerError::LlmTimeout(self.request_timeout.as_secs())
        })?
        .map_err(|e| {
            tracing::error!(error = %e, "LLM generation failed");
            WorkerError::LlmProvider(e.to_string())
        })?;

        let llm_elapsed_ms = llm_start.elapsed().as_millis();

        state.consume_tokens(response.tokens_processed, response.tokens_generated)?;

        tracing::debug!(
            tokens_processed = response.tokens_processed,
            tokens_generated = response.tokens_generated,
            tokens_total = state.tokens_used,
            llm_elapsed_ms,
            "LLM response received"
        );

        Ok(response.text)
    }

    async fn process_llm_response(
        &self,
        state: &mut AgentState,
        text: &str,
    ) -> Result<Option<AgentResult>, WorkerError> {
        match extract_llm_response::<ReActAgentResponse>(text) {
            Ok(llm_response) => {
                Ok(ResponseHandler::handle(state, llm_response, &self.tool_handler).await)
            }
            Err(raw) => {
                tracing::error!(
                    raw_preview = %raw,
                    "Failed to parse LLM response as JSON"
                );
                Err(WorkerError::Agent("Failed to parse LLM response as JSON".to_string()))
            }
        }
    }
}
