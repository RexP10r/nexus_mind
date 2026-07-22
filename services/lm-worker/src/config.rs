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

    #[arg(env = "MAX_ITERATIONS", long, default_value_t = 10)]
    pub max_iterations: u32,

    #[arg(env = "PROVIDER_TYPE", long, default_value = "grpc")]
    pub provider_type: ProviderType,

    #[arg(env = "AGENT_TYPE", long, default_value = "rag")]
    pub agent_type: AgentType,
}

impl Config {
    pub fn from_env() -> Self {
        Self::parse()
    }
}
