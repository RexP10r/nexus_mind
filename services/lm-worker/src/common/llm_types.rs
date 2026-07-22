use crate::agent::Message;

#[derive(Debug, Clone)]
pub struct LlmMessage {
    pub role: LlmRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmRole {
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

pub fn messages_to_llm(messages: &[Message], system_prompt: &str) -> Vec<LlmMessage> {
    let mut llm_msgs: Vec<LlmMessage> = Vec::with_capacity(messages.len() + 1);

    llm_msgs.push(LlmMessage {
        role: LlmRole::System,
        content: system_prompt.to_string(),
    });

    for msg in messages {
        let role = match msg.role.as_str() {
            "system" => LlmRole::System,
            "user" => LlmRole::User,
            "assistant" => LlmRole::Assistant,
            _ => LlmRole::User,
        };
        llm_msgs.push(LlmMessage {
            role,
            content: msg.content.clone(),
        });
    }

    llm_msgs
}
