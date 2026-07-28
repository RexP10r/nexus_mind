use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDoc {
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}
