use std::sync::Arc;

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
}

impl RAGAgent {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        tool_registry: ToolRegistry,
        max_iterations: u32,
    ) -> Self {
        Self {
            llm,
            tool_registry,
            max_iterations,
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
        let observation = self.execute_tool(&tool_name, &tool_input);
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
            LlmResponse::FinalAnswer { answer } => Some(AgentResult {
                final_answer: answer,
                total_tokens: state.tokens_used,
                reasoning_steps: state.reasoning_steps.clone(),
            }),
            LlmResponse::Think {
                thought,
                next_action,
            } => match next_action {
                Some(Action::ExecuteTool {
                    tool_name,
                    tool_input,
                }) => {
                    self.execute_tool_action(state, thought, tool_name, tool_input);
                    None
                }
                None => {
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
                return Ok(AgentResult {
                    final_answer: format!(
                        "Agent stopped after {} iterations without final answer",
                        max_iterations
                    ),
                    total_tokens: state.tokens_used,
                    reasoning_steps: state.reasoning_steps,
                });
            }

            let llm_messages = messages_to_llm(&state.conversation, &system_prompt);
            let response = self
                .llm
                .generate(llm_messages, params)
                .await
                .map_err(|e| WorkerError::LlmProvider(e.to_string()))?;

            state.consume_tokens(response.tokens_processed, response.tokens_generated)?;

            match extract_llm_response(&response.text) {
                Ok(llm_response) => {
                    if let Some(result) = self.handle_parsed_response(&mut state, llm_response) {
                        return Ok(result);
                    }
                }
                Err(_) => {
                    state.conversation.push(Message {
                        role: "system".to_string(),
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
    async fn run(
        &self,
        messages: &[Message],
        params: &GenerationParams,
    ) -> Result<AgentResult, WorkerError> {
        let state = AgentState::new(messages);
        self.execute_state(state, params).await
    }
}
