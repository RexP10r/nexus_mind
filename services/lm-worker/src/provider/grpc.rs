use async_trait::async_trait;
use tonic::transport::Channel;

use crate::error::WorkerError;
use crate::grpc::lm_service::{
    lm_service_client::LmServiceClient, ChatMessage, GenerateRequest, GenerateResponse,
    HealthCheckRequest, HealthCheckResponse,
};
use crate::traits::llm::LlmProvider;
use crate::traits::GenerationParams;

pub struct GrpcLlmProvider {
    client: LmServiceClient<Channel>,
}

impl GrpcLlmProvider {
    pub async fn connect(addr: &str) -> Result<Self, WorkerError> {
        let channel = Channel::from_shared(addr.to_string())
            .map_err(|e| WorkerError::LlmProvider(e.to_string()))?
            .connect()
            .await
            .map_err(|e| WorkerError::LlmProvider(e.to_string()))?;
        let client = LmServiceClient::new(channel);
        Ok(Self { client })
    }
}

#[async_trait]
impl LlmProvider for GrpcLlmProvider {
    async fn generate(
        &self,
        messages: Vec<ChatMessage>,
        params: &GenerationParams,
    ) -> Result<GenerateResponse, WorkerError> {
        let req = GenerateRequest {
            messages,
            temperature: params.temperature,
            max_tokens: params.max_tokens,
            top_p: params.top_p,
            top_k: params.top_k,
        };
        let mut client = self.client.clone();
        client
            .generate(req)
            .await
            .map(|r| r.into_inner())
            .map_err(WorkerError::Grpc)
    }

    async fn health_check(&self) -> Result<HealthCheckResponse, WorkerError> {
        let mut client = self.client.clone();
        client
            .health_check(HealthCheckRequest {})
            .await
            .map(|r| r.into_inner())
            .map_err(WorkerError::Grpc)
    }
}
