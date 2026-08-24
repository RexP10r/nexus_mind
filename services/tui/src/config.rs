use std::env;
use std::path::PathBuf;

use crate::error::TuiError;

#[derive(Clone, Debug)]
pub struct Config {
    pub server_url: String,
    pub health_poll_secs: u64,
    pub docs_max_file_size: u64,
    pub docs_supported_extensions: Vec<String>,
    pub conversations_file: PathBuf,
}

impl Config {
    pub fn from_env() -> Result<Self, TuiError> {
        let server_url = env::var("NEXUS_SERVER_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

        let health_poll_secs = env::var("HEALTH_POLL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        let docs_max_file_size = env::var("DOCS_MAX_FILE_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1_048_576);

        let docs_supported_extensions = env::var("DOCS_SUPPORTED_EXTENSIONS")
            .unwrap_or_else(|_| ".md".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let conversations_file = env::var("CONVERSATIONS_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("nexus_mind")
                    .join("conversations.json")
            });

        if server_url.is_empty() {
            return Err(TuiError::Config("NEXUS_SERVER_URL must not be empty".into()));
        }

        Ok(Self {
            server_url,
            health_poll_secs,
            docs_max_file_size,
            docs_supported_extensions,
            conversations_file,
        })
    }
}
