use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::conversation::Conversation;
use crate::error::TuiError;

#[derive(Debug, Serialize, Deserialize)]
struct ConversationsFile {
    conversations: Vec<Conversation>,
}

pub struct ConversationStore {
    file_path: PathBuf,
}

impl ConversationStore {
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    pub fn load(&self) -> Result<Vec<Conversation>, TuiError> {
        if !self.file_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.file_path)?;
        let file: ConversationsFile = serde_json::from_str(&content)?;
        Ok(file.conversations)
    }

    pub fn save(&self, conversations: &[Conversation]) -> Result<(), TuiError> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = ConversationsFile {
            conversations: conversations.to_vec(),
        };
        let content = serde_json::to_string_pretty(&file)?;
        fs::write(&self.file_path, content)?;
        Ok(())
    }
}
