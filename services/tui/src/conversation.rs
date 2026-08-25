use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::app::MessageRole;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub conversation_id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub summary: Option<String>,
    pub total_messages: u32,
    pub total_tokens: u32,
    pub timeline: Vec<TimelineEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TimelineEntry {
    #[serde(rename = "message")]
    Message {
        role: MessageRole,
        content: String,
    },
}

impl Conversation {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            conversation_id: uuid::Uuid::new_v4().to_string(),
            title: "New Conversation".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            total_messages: 0,
            total_tokens: 0,
            timeline: Vec::new(),
        }
    }

    pub fn add_message(&mut self, role: MessageRole, content: String) {
        // Auto-generate title from first user message
        if role == MessageRole::User && self.title == "New Conversation" {
            self.title = if content.len() > 50 {
                format!("{}...", &content[..50])
            } else {
                content.clone()
            };
        }

        self.timeline.push(TimelineEntry::Message {
            role,
            content,
        });
        self.total_messages += 1;
        self.updated_at = Utc::now();
        eprintln!("DEBUG: add_message() - role={:?}, total_messages={}, timeline_len={}", role, self.total_messages, self.timeline.len());
    }

    pub fn messages(&self) -> Vec<(MessageRole, String)> {
        self.timeline
            .iter()
            .filter_map(|entry| match entry {
                TimelineEntry::Message { role, content } => Some((*role, content.clone())),
            })
            .collect()
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}
