use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkerError {
    #[error{"Io error: {0}"}]
    Io(#[from] std::io::Error),

    #[error("LLM provider error: {0}")]
    LlmProvider(String),

    #[error("LLM request timed out after {0}s")]
    LlmTimeout(u64),

    #[allow(dead_code)]
    #[error("Agent error: {0}")]
    Agent(String),

    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),

    #[error("gRPC transport error: {0}")]
    GrpcTransport(#[from] tonic::transport::Error),
}
