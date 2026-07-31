use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::message::{ChatMessage, ChatRole, Message};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub thought: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<AgentAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum AgentAction {
    ExecuteTool {
        tool_name: String,
        tool_input: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub final_answer: String,
    pub total_tokens: u32,
    pub reasoning_steps: Vec<AgentStep>,
}

pub fn reasoning_steps_to_chat(steps: &[AgentStep]) -> Vec<ChatMessage> {
    let mut out = Vec::with_capacity(steps.len() * 2);

    for step in steps {
        match (&step.action, &step.observation) {
            (
                Some(AgentAction::ExecuteTool {
                    tool_name,
                    tool_input,
                }),
                Some(obs),
            ) => {
                out.push(ChatMessage {
                    role: ChatRole::Assistant,
                    content: format!(
                        "Thought: {}\n\nAction: {}({})",
                        step.thought, tool_name, tool_input
                    ),
                });
                out.push(ChatMessage {
                    role: ChatRole::User,
                    content: format!("Observation: {}", obs),
                });
            }
            _ => {
                out.push(ChatMessage {
                    role: ChatRole::Assistant,
                    content: step.thought.clone(),
                });
            }
        }
    }

    out
}

pub fn build_chat_context(
    conversation: &[Message],
    steps: &[AgentStep],
    system_prompt: &str,
) -> Vec<ChatMessage> {
    let mut msgs = super::message::messages_to_chat(conversation, system_prompt);
    msgs.extend(reasoning_steps_to_chat(steps));
    msgs
}
