use async_trait::async_trait;

use crate::model::{GenerateOutput, GenerationParams, HealthStatus, LlmMessage};

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate(
        &self,
        messages: Vec<LlmMessage>,
        params: &GenerationParams,
    ) -> Result<GenerateOutput, crate::error::WorkerError>;

    async fn health_check(&self) -> Result<HealthStatus, crate::error::WorkerError>;
}
