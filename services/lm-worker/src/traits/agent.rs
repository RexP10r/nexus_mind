use async_trait::async_trait;

use crate::model::{AgentResult, GenerationParams, Message};

#[async_trait]
pub trait Agent: Send + Sync {
    async fn run(
        &self,
        messages: &[Message],
        summary: Option<&str>,
        params: &GenerationParams,
    ) -> Result<AgentResult, crate::error::WorkerError>;
}
