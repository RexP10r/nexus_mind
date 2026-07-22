use async_trait::async_trait;

use crate::common::llm_types::{GenerateOutput, HealthStatus, LlmMessage};
use crate::common::GenerationParams;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate(
        &self,
        messages: Vec<LlmMessage>,
        params: &GenerationParams,
    ) -> Result<GenerateOutput, crate::error::WorkerError>;

    async fn health_check(&self) -> Result<HealthStatus, crate::error::WorkerError>;
}
