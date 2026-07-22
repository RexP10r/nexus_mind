use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::agent::prompt::build_system_prompt;
use crate::agent::schema::{extract_llm_response, Action, LlmResponse};
use crate::agent::state::AgentState;
use crate::agent::{AgentAction, AgentResult, AgentStep, Message};
use crate::common::llm_types::messages_to_llm;
use crate::common::tools::registry::ToolRegistry;
use crate::common::traits::agent::Agent;
use crate::common::traits::llm::LlmProvider;
use crate::common::GenerationParams;
use crate::error::WorkerError;

pub struct RAGAgent {
    llm: Arc<dyn LlmProvider>,
    tool_registry: ToolRegistry,
    max_iterations: u32,
    request_timeout: Duration,
}

impl RAGAgent {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        tool_registry: ToolRegistry,
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

    fn tool_not_found_message(&self, tool_name: &str) -> String {
        let desc = self.tool_registry.tool_descriptions();
        let available = if desc.is_empty() {
            "none".to_string()
        } else {
            desc
        };
        format!("Tool '{}' not found. Available: {}", tool_name, available)
    }

    fn execute_tool(&self, tool_name: &str, tool_input: &str) -> String {
        self.tool_registry
            .execute(tool_name, tool_input)
            .unwrap_or_else(|| self.tool_not_found_message(tool_name))
    }

    fn execute_tool_action(
        &self,
        state: &mut AgentState,
        thought: String,
        tool_name: String,
        tool_input: String,
    ) {
        if tool_name.trim().is_empty() || tool_input.trim().is_empty() {
            let observation = format!(
                "Tool invocation failed: tool_name and tool_input must be non-empty. Got tool_name='{}', tool_input='{}'",
                tool_name, tool_input
            );
            tracing::warn!(
                tool_name = %tool_name,
                tool_input = %tool_input,
                "Empty tool name or input"
            );
            let action = AgentAction::ExecuteTool {
                tool_name,
                tool_input,
            };
            *state = state.add_turn(thought, observation, Some(action));
            return;
        }

        let start = std::time::Instant::now();
        let observation = self.execute_tool(&tool_name, &tool_input);
        let elapsed_ms = start.elapsed().as_millis();

        tracing::info!(
            tool_name = %tool_name,
            tool_input = %tool_input,
            observation = %observation,
            elapsed_ms,
            "Tool executed"
        );

        let action = AgentAction::ExecuteTool {
            tool_name,
            tool_input,
        };
        *state = state.add_turn(thought, observation, Some(action));
    }

    fn handle_parsed_response(
        &self,
        state: &mut AgentState,
        llm_response: LlmResponse,
    ) -> Option<AgentResult> {
        match llm_response {
            LlmResponse::FinalAnswer { answer } => {
                tracing::info!(answer = %answer, "Agent reached final answer");
                Some(AgentResult {
                    final_answer: answer,
                    total_tokens: state.tokens_used,
                    reasoning_steps: state.reasoning_steps.clone(),
                })
            }
            LlmResponse::Think {
                thought,
                next_action,
            } => match next_action {
                Some(Action::ExecuteTool {
                    tool_name,
                    tool_input,
                }) => {
                    tracing::info!(
                        thought = %thought,
                        tool_name = %tool_name,
                        "Agent decided to use tool"
                    );
                    self.execute_tool_action(state, thought, tool_name, tool_input);
                    None
                }
                None => {
                    tracing::info!(
                        thought = %thought,
                        "Agent thinking without tool action"
                    );
                    state.reasoning_steps.push(AgentStep {
                        thought: thought.clone(),
                        action: None,
                        observation: None,
                    });
                    state.conversation.push(Message {
                        role: "assistant".to_string(),
                        content: thought,
                    });
                    None
                }
            },
        }
    }

    #[tracing::instrument(skip(self, params, state), fields(
        state_id = %state.id,
        conversation_len = state.conversation.len(),
    ))]
    async fn execute_state(
        &self,
        mut state: AgentState,
        params: &GenerationParams,
    ) -> Result<AgentResult, WorkerError> {
        let tool_descriptions = self.tool_registry.tool_descriptions();
        let system_prompt = build_system_prompt(&tool_descriptions);

        let max_iterations = self.max_iterations.max(1);
        let mut iteration: u32 = 0;

        loop {
            iteration += 1;
            if iteration > max_iterations {
                tracing::warn!(
                    iteration,
                    max_iterations,
                    tokens_used = state.tokens_used,
                    "Max iterations reached"
                );
                return Ok(AgentResult {
                    final_answer: format!(
                        "Agent stopped after {} iterations without final answer",
                        max_iterations
                    ),
                    total_tokens: state.tokens_used,
                    reasoning_steps: state.reasoning_steps,
                });
            }

            tracing::info!(iteration, max_iterations, "Agent iteration");

            let llm_messages = messages_to_llm(&state.conversation, &system_prompt);
            let llm_start = std::time::Instant::now();

            let response = tokio::time::timeout(
                self.request_timeout,
                self.llm.generate(llm_messages, params),
            )
            .await
            .map_err(|_| {
                tracing::error!(
                    timeout_secs = self.request_timeout.as_secs(),
                    iteration,
                    "LLM request timed out"
                );
                WorkerError::LlmTimeout(self.request_timeout.as_secs())
            })?
            .map_err(|e| {
                tracing::error!(
                    error = %e,
                    iteration,
                    "LLM generation failed"
                );
                WorkerError::LlmProvider(e.to_string())
            })?;

            let llm_elapsed_ms = llm_start.elapsed().as_millis();

            state.consume_tokens(response.tokens_processed, response.tokens_generated)?;

            tracing::info!(
                iteration,
                tokens_processed = response.tokens_processed,
                tokens_generated = response.tokens_generated,
                tokens_total = state.tokens_used,
                llm_elapsed_ms,
                "LLM response received"
            );

            match extract_llm_response(&response.text) {
                Ok(llm_response) => {
                    if let Some(result) = self.handle_parsed_response(&mut state, llm_response) {
                        tracing::info!(
                            total_tokens = result.total_tokens,
                            reasoning_steps = result.reasoning_steps.len(),
                            "Agent run completed"
                        );
                        return Ok(result);
                    }
                }
                Err(raw) => {
                    tracing::warn!(
                        iteration,
                        raw_preview = %raw.chars().take(200).collect::<String>(),
                        "Failed to parse LLM response as JSON"
                    );
                    state.conversation.push(Message {
                        role: "user".to_string(),
                        content:
                            "Your last response was not valid JSON. Output ONLY valid JSON matching the schema."
                                .to_string(),
                    });
                }
            }
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
    ) -> Result<AgentResult, WorkerError> {
        for msg in messages {
            tracing::info!(
                role = %msg.role,
                content = %msg.content,
                "User message"
            );
        }
        let state = AgentState::new(messages);
        self.execute_state(state, params).await
    }
}