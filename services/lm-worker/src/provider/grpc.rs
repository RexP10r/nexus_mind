use async_trait::async_trait;
use tonic::transport::Channel;

use crate::common::llm_types::{GenerateOutput, HealthStatus, LlmMessage, LlmRole};
use crate::common::traits::llm::LlmProvider;
use crate::common::GenerationParams;
use crate::error::WorkerError;
use crate::grpc::lm_service::{
    lm_service_client::LmServiceClient, ChatMessage, GenerateRequest, GenerateResponse,
    HealthCheckRequest, HealthCheckResponse, MessageRole,
};

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

    fn to_proto_message(msg: LlmMessage) -> ChatMessage {
        let role = match msg.role {
            LlmRole::System => MessageRole::RoleSystem,
            LlmRole::User => MessageRole::RoleUser,
            LlmRole::Assistant => MessageRole::RoleAssistant,
        };
        ChatMessage {
            role: role as i32,
            content: msg.content,
        }
    }

    fn from_proto_response(resp: tonic::Response<GenerateResponse>) -> GenerateOutput {
        let inner = resp.into_inner();
        GenerateOutput {
            text: inner.text,
            tokens_processed: inner.tokens_processed,
            tokens_generated: inner.tokens_generated,
        }
    }

    fn from_proto_health(
        resp: tonic::Response<HealthCheckResponse>,
    ) -> HealthStatus {
        let inner = resp.into_inner();
        HealthStatus {
            is_ready: inner.is_ready,
            model_name: inner.model_name,
            context_length: inner.context_length,
        }
    }
}

#[async_trait]
impl LlmProvider for GrpcLlmProvider {
    async fn generate(
        &self,
        messages: Vec<LlmMessage>,
        params: &GenerationParams,
    ) -> Result<GenerateOutput, WorkerError> {
        let proto_messages: Vec<ChatMessage> =
            messages.into_iter().map(Self::to_proto_message).collect();

        let req = GenerateRequest {
            messages: proto_messages,
            temperature: params.temperature,
            max_tokens: params.max_tokens,
            top_p: params.top_p,
            top_k: params.top_k,
        };
        let mut client = self.client.clone();
        client
            .generate(req)
            .await
            .map(Self::from_proto_response)
            .map_err(WorkerError::Grpc)
    }

    async fn health_check(&self) -> Result<HealthStatus, WorkerError> {
        let mut client = self.client.clone();
        client
            .health_check(HealthCheckRequest {})
            .await
            .map(Self::from_proto_health)
            .map_err(WorkerError::Grpc)
    }
}
