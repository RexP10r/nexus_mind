use async_trait::async_trait;

use crate::agent::AgentResult;
use crate::agent::Message;
use crate::common::GenerationParams;

#[async_trait]
pub trait Agent: Send + Sync {
    async fn run(
        &self,
        messages: &[Message],
        params: &GenerationParams,
    ) -> Result<AgentResult, crate::error::WorkerError>;
}
