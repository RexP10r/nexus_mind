use async_trait::async_trait;

use crate::model::{ChatMessage, GenerateOutput, GenerationParams, HealthStatus};

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate(
        &self,
        messages: Vec<ChatMessage>,
        params: &GenerationParams,
    ) -> Result<GenerateOutput, crate::error::WorkerError>;

    async fn health_check(&self) -> Result<HealthStatus, crate::error::WorkerError>;
}
