use std::sync::Arc;
use tracing::info;

use async_trait::async_trait;

use crate::agent::parser::{parse_agent_output, AgentAction};
use crate::agent::tools::registry::ToolRegistry;
use crate::agent::{AgentResult, Message, ReasoningStep};
use crate::error::WorkerError;
use crate::grpc::lm_service::{ChatMessage, MessageRole};
use crate::traits::agent::Agent;
use crate::traits::llm::LlmProvider;
use crate::traits::GenerationParams;

const REACT_SYSTEM_PROMPT: &str = r#"You are a helpful assistant that uses step-by-step reasoning.

Respond using this format:

Thought: your reasoning about what to do next
Action: tool_name[tool input]

When you are ready to give the final answer, use:

Thought: I now know the answer
Final Answer: the final answer to the original question

Always finish with a Final Answer."#;

pub struct ReactAgent {
    llm: Arc<dyn LlmProvider>,
    tool_registry: ToolRegistry,
    max_iterations: u32,
}

impl ReactAgent {
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

    fn build_system_prompt(&self) -> String {
        let tools = self.tool_registry.tool_descriptions();
        if tools.is_empty() {
            REACT_SYSTEM_PROMPT.to_string()
        } else {
            format!("{}\n\nAvailable tools:\n{}", REACT_SYSTEM_PROMPT, tools)
        }
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

    fn extract_action(text: &str) -> Option<ParsedAction> {
        let text = text.trim();
        if let Some(rest) = text.strip_prefix("Action:") {
            let rest = rest.trim();
            if let Some(bracket_open) = rest.find('[') {
                let bracket_close = rest.rfind(']')?;
                let name = rest[..bracket_open].trim().to_string();
                let input = rest[bracket_open + 1..bracket_close].to_string();
                if !name.is_empty() && !input.is_empty() {
                    return Some(ParsedAction { name, input });
                }
            }
        }
        None
    }
}

#[async_trait]
impl Agent for ReactAgent {
    async fn run(
        &self,
        messages: &[Message],
        params: &GenerationParams,
    ) -> Result<AgentResult, WorkerError> {
        let system_prompt = self.build_system_prompt();
        let mut history = messages.to_vec();
        let mut reasoning_steps: Vec<ReasoningStep> = Vec::new();
        let mut iteration = 0u32;

        loop {
            if iteration >= self.max_iterations {
                return Err(WorkerError::MaxIterationsExceeded(self.max_iterations));
            }

            iteration += 1;
            info!("Agent iteration {}", iteration);

            let proto_messages = self.to_proto_messages(&history, &system_prompt);
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

            let (thought, agent_action) = parse_agent_output(&output);

            match agent_action {
                AgentAction::FinalAnswer(answer) => {
                    reasoning_steps.push(ReasoningStep {
                        iteration,
                        thought,
                        action: Some("FinalAnswer".into()),
                        action_input: Some(answer.clone()),
                        observation: None,
                    });

                    return Ok(AgentResult {
                        final_answer: answer,
                        reasoning_steps,
                    });
                }
                AgentAction::Continue(action_text) => {
                    let mut step = ReasoningStep {
                        iteration,
                        thought,
                        action: None,
                        action_input: None,
                        observation: None,
                    };

                    if let Some(action) = Self::extract_action(&action_text) {
                        step.action = Some(action.name.clone());
                        step.action_input = Some(action.input.clone());

                        let observation = match self
                            .tool_registry
                            .execute(&action.name, &action.input)
                        {
                            Some(result) => result,
                            None => {
                                step.action = None;
                                step.action_input = None;
                                action_text
                            }
                        };

                        if step.action.is_some() {
                            step.observation = Some(observation.clone());
                            history.push(Message {
                                role: "system".into(),
                                content: format!("Observation: {}", observation),
                            });
                        }
                    }

                    history.push(Message {
                        role: "assistant".into(),
                        content: output,
                    });

                    reasoning_steps.push(step);
                }
            }
        }
    }
}

struct ParsedAction {
    name: String,
    input: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_action() {
        let action = ReactAgent::extract_action("Action: calculate[15 * 7 + 3]").unwrap();
        assert_eq!(action.name, "calculate");
        assert_eq!(action.input, "15 * 7 + 3");
    }

    #[test]
    fn test_extract_action_no_brackets() {
        assert!(ReactAgent::extract_action("Action: just think").is_none());
    }
}
