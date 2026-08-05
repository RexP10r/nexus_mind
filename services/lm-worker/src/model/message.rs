use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

impl fmt::Display for ChatRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ChatRole::System => "system",
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
        };
        write!(f, "{}", s)
    }
}

pub fn messages_to_chat(messages: &[Message], system_prompt: &str) -> Vec<ChatMessage> {
    let mut chat_msgs: Vec<ChatMessage> = Vec::with_capacity(messages.len() + 1);

    chat_msgs.push(ChatMessage {
        role: ChatRole::System,
        content: system_prompt.to_string(),
    });

    for msg in messages {
        chat_msgs.push(ChatMessage {
            role: msg.role,
            content: msg.content.clone(),
        });
    }

    chat_msgs
}
