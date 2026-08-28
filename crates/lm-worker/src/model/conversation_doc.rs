use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::agent::AgentAction;
use super::message::ChatRole;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationDoc {
    pub conversation_id: String,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub updated_at: DateTime<Utc>,
    pub summary: Option<String>,
    #[serde(default)]
    pub total_messages: u32,
    #[serde(default)]
    pub total_tokens: u32,
    pub timeline: Vec<ConversationEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConversationEntry {
    #[serde(rename = "message")]
    ChatMsg {
        role: ChatRole,
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
