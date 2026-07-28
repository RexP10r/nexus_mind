use std::sync::Arc;
use std::time::Duration;

use super::prompt::build_system_prompt;
use super::response_handler::ResponseHandler;
use super::schema::extract_llm_response;
use super::state::AgentState;
use super::tool_handler::ToolHandler;
use crate::error::WorkerError;
use crate::model::{messages_to_llm, AgentResult, GenerationParams, Message};
use crate::traits::llm::LlmProvider;

pub(crate) struct AgentLoop<'a> {
    llm: Arc<dyn LlmProvider>,
    tool_handler: ToolHandler<'a>,
    max_iterations: u32,
    request_timeout: Duration,
}

impl<'a> AgentLoop<'a> {
    pub(crate) fn new(
        llm: Arc<dyn LlmProvider>,
        tool_handler: ToolHandler<'a>,
        max_iterations: u32,
        request_timeout: Duration,
    ) -> Self {
        Self {
            llm,
            tool_handler,
            max_iterations,
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
        let max_iterations = self.max_iterations.max(1);
        let mut iteration: u32 = 0;

        loop {
            iteration += 1;

            if let Some(result) = self.check_iteration_limit(iteration, max_iterations, &state) {
                return Ok(result);
            }

            tracing::info!(iteration, max_iterations, "Agent iteration");

            let response_text = self.call_llm(&mut state, &system_prompt, params).await?;

            if let Some(result) = self.process_llm_response(&mut state, &response_text) {
                return Ok(result);
            }
        }
    }

    fn check_iteration_limit(
        &self,
        iteration: u32,
        max_iterations: u32,
        state: &AgentState,
    ) -> Option<AgentResult> {
        if iteration > max_iterations {
            tracing::warn!(
                iteration,
                max_iterations,
                tokens_used = state.tokens_used,
                "Max iterations reached"
            );
            return Some(AgentResult {
                final_answer: format!(
                    "Agent stopped after {} iterations without final answer",
                    max_iterations
                ),
                total_tokens: state.tokens_used,
                reasoning_steps: state.reasoning_steps.clone(),
            });
        }
        None
    }

    async fn call_llm(
        &self,
        state: &mut AgentState,
        system_prompt: &str,
        params: &GenerationParams,
    ) -> Result<String, WorkerError> {
        let llm_messages = messages_to_llm(&state.conversation, system_prompt);
        let llm_start = std::time::Instant::now();

        let response = tokio::time::timeout(
            self.request_timeout,
            self.llm.generate(llm_messages, params),
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

        tracing::info!(
            tokens_processed = response.tokens_processed,
            tokens_generated = response.tokens_generated,
            tokens_total = state.tokens_used,
            llm_elapsed_ms,
            "LLM response received"
        );

        Ok(response.text)
    }

    fn process_llm_response(
        &self,
        state: &mut AgentState,
        text: &str,
    ) -> Option<AgentResult> {
        match extract_llm_response(text) {
            Ok(llm_response) => {
                if let Some(result) = ResponseHandler::handle(state, llm_response, &self.tool_handler) {
                    tracing::info!(
                        total_tokens = result.total_tokens,
                        reasoning_steps = result.reasoning_steps.len(),
                        "Agent run completed"
                    );
                    return Some(result);
                }
                None
            }
            Err(raw) => {
                tracing::warn!(
                    raw_preview = %raw,
                    "Failed to parse LLM response as JSON"
                );
                state.conversation.push(Message {
                    role: "user".to_string(),
                    content:
                        "Your last response was not valid JSON. Output ONLY valid JSON matching the schema."
                            .to_string(),
                });
                None
            }
        }
    }
}
