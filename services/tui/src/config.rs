use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Clone, Debug)]
#[command(name = "tui")]
pub struct Config {
    #[arg(env = "SERVER_URL", long)]
    pub server_url: String,
    #[arg(env = "HEALTH_POLL_SECS", long)]
    pub health_poll_secs: u64,
    #[arg(env = "DOCS_MAX_FILE_SIZE", long)]
    pub docs_max_file_size: u64,
    #[arg(env = "CONVERSATIONS_FILE", long)]
    pub conversations_file: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
        Self::parse()
    }
}
