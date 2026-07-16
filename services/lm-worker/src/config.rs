use clap::Parser;

#[derive(Parser, Clone, Debug)]
#[command(name = "lm-worker")]
pub struct Config {
    #[arg(env = "GRPC_ADDR", long, default_value = "http://[::1]:50051")]
    pub grpc_addr: String,

    #[arg(env = "HTTP_PORT", long, default_value_t = 8080)]
    pub http_port: u16,

    #[arg(env = "MAX_ITERATIONS", long, default_value_t = 10)]
    pub max_iterations: u32,

}

impl Config {
    pub fn from_env() -> Self {
        Self::parse()
    }
}
