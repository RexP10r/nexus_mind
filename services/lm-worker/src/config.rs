use clap::Parser;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ProviderType {
    Grpc,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum AgentType {
    RAG,
}

#[derive(Parser, Clone, Debug)]
#[command(name = "lm-worker")]
pub struct Config {
    #[arg(env = "GRPC_ADDR", long, default_value = "http://[::1]:50051")]
    pub grpc_addr: String,

    #[arg(env = "HTTP_PORT", long, default_value_t = 8080)]
    pub http_port: u16,

    #[arg(env = "LOG_LEVEL", long, default_value = "info")]
    pub log_level: String,

    #[arg(env = "LOG_JSON", long, default_value_t = false)]
    pub log_json: bool,

    #[arg(env = "MAX_ITERATIONS", long, default_value_t = 10)]
    pub max_iterations: u32,

    #[arg(env = "REQUEST_TIMEOUT_SECS", long, default_value_t = 60)]
    pub request_timeout_secs: u64,

    #[arg(env = "PROVIDER_TYPE", long, default_value = "grpc")]
    pub provider_type: ProviderType,

    #[arg(env = "AGENT_TYPE", long, default_value = "rag")]
    pub agent_type: AgentType,

    #[arg(env = "REDIS_URL", long, default_value = "redis://localhost:6379")]
    pub redis_url: String,

    #[arg(env = "MONGO_URI", long, default_value = "mongodb://localhost:27017")]
    pub mongo_uri: String,

    #[arg(env = "MONGO_DB", long, default_value = "nexus_mind")]
    pub mongo_db: String,

    #[arg(env = "HISTORY_MAX_MESSAGES", long, default_value_t = 10)]
    pub history_max_messages: u32,

    #[arg(env = "SUMMARY_INTERVAL", long, default_value_t = 5)]
    pub summary_interval: u32,

    #[arg(env = "REDIS_TTL_SECS", long, default_value_t = 0)]
    pub redis_ttl_secs: u64,
}

impl Config {
    pub fn from_env() -> Self {
        Self::parse()
    }
}
