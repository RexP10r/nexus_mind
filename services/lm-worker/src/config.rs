use clap::Parser;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ProviderType {
    Grpc,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum AgentType {
    Rag,
}

#[derive(Parser, Clone, Debug)]
#[command(name = "lm-worker")]
pub struct Config {
    #[arg(env = "GRPC_ADDR", long)]
    pub grpc_addr: String,

    #[arg(env = "HTTP_PORT", long)]
    pub http_port: u16,

    #[arg(env = "LOG_LEVEL", long)]
    pub log_level: String,

    #[arg(env = "LOG_JSON", long)]
    pub log_json: bool,

    #[arg(env = "MAX_ITERATIONS", long)]
    pub max_iterations: u32,

    #[arg(env = "REQUEST_TIMEOUT", long)]
    pub request_timeout: u64,

    #[arg(env = "PROVIDER_TYPE", long)]
    pub provider_type: ProviderType,

    #[arg(env = "AGENT_TYPE", long)]
    pub agent_type: AgentType,

    #[arg(env = "REDIS_URL", long)]
    pub redis_url: String,

    #[arg(env = "MONGO_URI", long)]
    pub mongo_uri: String,

    #[arg(env = "MONGO_DB", long)]
    pub mongo_db: String,

    #[arg(env = "HISTORY_MAX_MESSAGES", long)]
    pub history_max_messages: u32,

    #[arg(env = "SUMMARY_INTERVAL", long)]
    pub summary_interval: u32,

    #[arg(env = "REDIS_TTL_SECS", long)]
    pub redis_ttl_secs: u64,

    #[arg(env = "QDRANT_URL", long, default_value = "http://localhost:6334")]
    pub qdrant_url: String,

    #[arg(env = "QDRANT_COLLECTION", long, default_value = "nexus_docs")]
    pub qdrant_collection: String,

    #[arg(env = "EMBEDDING_MODEL_PATH", long, default_value = "models/model.onnx")]
    pub embedding_model_path: String,

    #[arg(env = "EMBEDDING_TOKENIZER_PATH", long, default_value = "models/tokenizer.json")]
    pub embedding_tokenizer_path: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self::parse()
    }
}
