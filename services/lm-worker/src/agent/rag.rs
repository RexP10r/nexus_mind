use std::sync::Arc;

use async_trait::async_trait;

use crate::agent::prompt::build_system_prompt;
use crate::agent::schema::{extract_llm_response, Action, LlmResponse};
use crate::agent::state::AgentState;
use crate::agent::{AgentAction, AgentResult, AgentStep, Message};
use crate::common::tools::registry::ToolRegistry;
use crate::common::traits::agent::Agent;
use crate::common::traits::llm::LlmProvider;
use crate::common::traits::tool::Tool;
use crate::common::GenerationParams;
use crate::error::WorkerError;
use crate::grpc::lm_service::{ChatMessage, MessageRole};

<<<<<<< HEAD
const MAX_RETRY_ON_PARSE_FAILURE: u32 = 2;

pub struct RAGAgent<T: Tool> {
    llm: Arc<dyn LlmProvider>,
    tool_registry: ToolRegistry<T>,
}

impl<T: Tool> RAGAgent<T> {
    pub fn new(llm: Arc<dyn LlmProvider>, tool_registry: ToolRegistry<T>) -> Self {
        Self { llm, tool_registry }
=======
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
>>>>>>> cd81127 (fix(services/lm-worker): rag pipeline recursive -> loop + nadlers)
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
<<<<<<< HEAD
    async fn execute_state(
        &self,
        state: &mut AgentState,
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
                        reasoning_steps: state.reasoning_steps.clone(),
                    });
                }
                state.conversation.push(Message {
                    role: "system".to_string(),
                    content: "Your last response was not valid JSON. Output ONLY valid JSON matching the schema.".to_string(),
                });
                return Box::pin(self.execute_state(state, params)).await;
            }
=======

    fn tool_not_found_message(&self, tool_name: &str) -> String {
        let desc = self.tool_registry.tool_descriptions();
        let available = if desc.is_empty() {
            "none".to_string()
        } else {
            desc
>>>>>>> cd81127 (fix(services/lm-worker): rag pipeline recursive -> loop + nadlers)
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
<<<<<<< HEAD
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
                    state.add_turn(thought, observation, Some(action));
                    Box::pin(self.execute_state(state, params)).await
=======
                    self.execute_tool_action(state, thought, tool_name, tool_input);
                    None
>>>>>>> cd81127 (fix(services/lm-worker): rag pipeline recursive -> loop + nadlers)
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

            let proto_messages = self.to_proto_messages(&state.conversation, &system_prompt);
            let response = self
                .llm
                .generate(proto_messages, params)
                .await
                .map_err(|e| WorkerError::LlmProvider(e.to_string()))?;

            state.consume_tokens(
                response.tokens_processed as u32,
                response.tokens_generated as u32,
            );

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
impl<T: Tool> Agent for RAGAgent<T> {
    async fn run(
        &self,
        messages: &[Message],
        params: &GenerationParams,
    ) -> Result<AgentResult, WorkerError> {
<<<<<<< HEAD
        let mut state = AgentState::new(messages);
        Self::execute_state(&self, &mut state, params).await
=======
        let state = AgentState::new(messages);
        self.execute_state(state, params).await
>>>>>>> cd81127 (fix(services/lm-worker): rag pipeline recursive -> loop + nadlers)
    }
}
