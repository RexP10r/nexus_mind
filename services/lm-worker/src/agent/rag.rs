use std::sync::Arc;

use async_trait::async_trait;

use crate::agent::prompt::build_system_prompt;
use crate::agent::schema::{extract_llm_response, Action, LlmResponse};
use crate::agent::state::AgentState;
use crate::agent::{AgentAction, AgentResult, AgentStep, Message};
use crate::common::tools::registry::ToolRegistry;
use crate::common::traits::agent::Agent;
use crate::common::traits::llm::LlmProvider;
use crate::common::GenerationParams;
use crate::error::WorkerError;
use crate::grpc::lm_service::{ChatMessage, MessageRole};

const MAX_RETRY_ON_PARSE_FAILURE: u32 = 2;

pub struct RAGAgent {
    llm: Arc<dyn LlmProvider>,
    tool_registry: ToolRegistry,
}

impl RAGAgent {
    pub fn new(llm: Arc<dyn LlmProvider>, tool_registry: ToolRegistry) -> Self {
        Self { llm, tool_registry }
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
    async fn execute_state(
        &self,
        mut state: AgentState,
        params: &GenerationParams,
    ) -> Result<AgentResult, WorkerError> {
        let tool_descriptions = self.tool_registry.tool_descriptions();
        let system_prompt = build_system_prompt(&tool_descriptions);

        let proto_messages = self.to_proto_messages(&state.conversation, &system_prompt);
        let response = self
            .llm
            .generate(proto_messages, params)
            .await
            .map_err(|e| WorkerError::LlmProvider(e.to_string()))?;

        let prompt_tokens = response.tokens_processed as u32;
        let completion_tokens = response.tokens_generated as u32;
        state.consume_tokens(prompt_tokens, completion_tokens);

        let raw_text = response.text.clone();
        let llm_response = match extract_llm_response(&raw_text) {
            Ok(resp) => resp,
            Err(_) => {
                if state.reasoning_steps.len() as u32 >= MAX_RETRY_ON_PARSE_FAILURE {
                    return Ok(AgentResult {
                        final_answer: format!(
                            "Failed to produce valid JSON after {} attempts",
                            MAX_RETRY_ON_PARSE_FAILURE
                        ),
                        total_tokens: state.tokens_used,
                        reasoning_steps: state.reasoning_steps,
                    });
                }
                state.conversation.push(Message {
                    role: "system".to_string(),
                    content: "Your last response was not valid JSON. Output ONLY valid JSON matching the schema.".to_string(),
                });
                return Box::pin(self.execute_state(state, params)).await;
            }
        };
        match llm_response {
            LlmResponse::FinalAnswer { answer } => Ok(AgentResult {
                final_answer: answer,
                total_tokens: state.tokens_used,
                reasoning_steps: state.reasoning_steps,
            }),
            LlmResponse::Think {
                thought,
                next_action,
            } => match next_action {
                Some(Action::ExecuteTool {
                    tool_name,
                    tool_input,
                }) => {
                    let observation = self
                        .tool_registry
                        .execute(&tool_name, &tool_input)
                        .unwrap_or_else(|| {
                            format!("Tool '{}' not found. Available: {}", tool_name, {
                                let desc = self.tool_registry.tool_descriptions();
                                if desc.is_empty() {
                                    "none".to_string()
                                } else {
                                    desc
                                }
                            })
                        });
                    let action = AgentAction::ExecuteTool {
                        tool_name,
                        tool_input,
                    };
                    let new_state = state.add_turn(thought, observation, Some(action));
                    Box::pin(self.execute_state(new_state, params)).await
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
                    Box::pin(self.execute_state(state, params)).await
                }
            },
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
        Self::execute_state(&self, state, params).await
    }
}
