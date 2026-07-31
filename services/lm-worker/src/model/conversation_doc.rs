use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::agent::{AgentAction, AgentResult};
use super::message::Message;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationDoc {
    pub conversation_id: String,
    pub created_at: DateTime<Utc>,
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

    pub fn append_turn(&mut self, user_msg: &Message, agent_result: &AgentResult) {
        self.timeline.push(ConversationEntry::Message {
            role: user_msg.role.clone(),
            content: user_msg.content.clone(),
        });

        for step in &agent_result.reasoning_steps {
            self.timeline.push(ConversationEntry::Step {
                thought: step.thought.clone(),
                action: step.action.clone(),
                observation: step.observation.clone(),
            });
        }

        if !agent_result.final_answer.is_empty() {
            self.timeline.push(ConversationEntry::Message {
                role: "assistant".to_string(),
                content: agent_result.final_answer.clone(),
            });
        }

        self.total_tokens = self.total_tokens.saturating_add(agent_result.total_tokens);
        self.updated_at = Utc::now();
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
