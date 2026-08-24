use std::env;

use crate::error::TuiError;

#[derive(Clone, Debug)]
pub struct Config {
    pub server_url: String,
    pub health_poll_secs: u64,
    pub conversation_id: String,
    pub docs_max_file_size: u64,
    pub docs_supported_extensions: Vec<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, TuiError> {
        let server_url = env::var("NEXUS_SERVER_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

        let health_poll_secs = env::var("HEALTH_POLL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        let conversation_id = env::var("CONVERSATION_ID")
            .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());

        let docs_max_file_size = env::var("DOCS_MAX_FILE_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1_048_576);

        let docs_supported_extensions = env::var("DOCS_SUPPORTED_EXTENSIONS")
            .unwrap_or_else(|_| {
                ".md".to_string()
            })
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if server_url.is_empty() {
            return Err(TuiError::Config("NEXUS_SERVER_URL must not be empty".into()));
        }

        Ok(Self {
            server_url,
            health_poll_secs,
            conversation_id,
            docs_max_file_size,
            docs_supported_extensions,
        })
    }
}
