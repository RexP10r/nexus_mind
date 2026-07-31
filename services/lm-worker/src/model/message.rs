use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

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

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct GenerateOutput {
    pub text: String,
    pub tokens_processed: u32,
    pub tokens_generated: u32,
}

#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub is_ready: bool,
    pub model_name: String,
    pub context_length: u32,
}

pub fn messages_to_chat(messages: &[Message], system_prompt: &str) -> Vec<ChatMessage> {
    let mut chat_msgs: Vec<ChatMessage> = Vec::with_capacity(messages.len() + 1);

    chat_msgs.push(ChatMessage {
        role: ChatRole::System,
        content: system_prompt.to_string(),
    });

    for msg in messages {
        let role = match msg.role.as_str() {
            "system" => ChatRole::System,
            "user" => ChatRole::User,
            "assistant" => ChatRole::Assistant,
            _ => ChatRole::User,
        };
        chat_msgs.push(ChatMessage {
            role,
            content: msg.content.clone(),
        });
    }

    chat_msgs
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
    let mut msgs = messages_to_chat(conversation, system_prompt);
    msgs.extend(reasoning_steps_to_chat(steps));
    msgs
}
