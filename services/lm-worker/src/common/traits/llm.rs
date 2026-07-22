use async_trait::async_trait;

use crate::grpc::lm_service::{ChatMessage, GenerateResponse, HealthCheckResponse};
use crate::common::GenerationParams;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate(
        &self,
        messages: Vec<ChatMessage>,
        params: &GenerationParams,
    ) -> Result<GenerateResponse, crate::error::WorkerError>;

    async fn health_check(&self) -> Result<HealthCheckResponse, crate::error::WorkerError>;
}
