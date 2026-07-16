use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkerError {
    #[error{"Io error: {0}"}]
    Io(#[from] std::io::Error),    

    #[error("LLM provider error: {0}")]
    LlmProvider(String),

    #[error("Reached maximum iterations ({0}) without final answer")]
    MaxIterationsExceeded(u32),

    #[allow(dead_code)]
    #[error("Agent error: {0}")]
    Agent(String),

    #[allow(dead_code)]
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),

    #[error("gRPC transport error: {0}")]
    GrpcTransport(#[from] tonic::transport::Error),
}
