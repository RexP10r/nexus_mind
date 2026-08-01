use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::agent::AgentAction;
use super::message::Message;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationDoc {
    pub conversation_id: String,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub updated_at: DateTime<Utc>,
    pub summary: Option<String>,
    pub total_tokens: u32,
    pub timeline: Vec<ConversationEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConversationEntry {
    #[serde(rename = "message")]
    Message {
        role: String,
        content: String,
    },
    #[serde(rename = "step")]
    Step {
        thought: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        action: Option<AgentAction>,
        #[serde(skip_serializing_if = "Option::is_none")]
        observation: Option<String>,
    },
}

impl ConversationDoc {
    pub fn new(conversation_id: String) -> Self {
        let now = Utc::now();
        Self {
            conversation_id,
            created_at: now,
            updated_at: now,
            summary: None,
            total_tokens: 0,
            timeline: Vec::new(),
        }
    }

    pub fn to_messages(&self) -> Vec<Message> {
        self.timeline
            .iter()
            .filter_map(|entry| match entry {
                ConversationEntry::Message { role, content } => Some(Message {
                    role: role.clone(),
                    content: content.clone(),
                }),
                _ => None,
            })
            .collect()
    }

    pub fn message_count(&self) -> usize {
        self.timeline
            .iter()
            .filter(|e| matches!(e, ConversationEntry::Message { .. }))
            .count()
    }

    pub fn older_messages(&self, keep_last: u32) -> Vec<Message> {
        let messages: Vec<&ConversationEntry> = self
            .timeline
            .iter()
            .filter(|e| matches!(e, ConversationEntry::Message { .. }))
            .collect();

        let keep = keep_last as usize;
        if messages.len() <= keep {
            return Vec::new();
        }

        messages[..messages.len() - keep]
            .iter()
            .map(|entry| match entry {
                ConversationEntry::Message { role, content } => Message {
                    role: role.clone(),
                    content: content.clone(),
                },
                _ => unreachable!(),
            })
            .collect()
    }
}
