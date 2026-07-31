use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
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
