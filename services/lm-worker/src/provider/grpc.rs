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
    #[tracing::instrument]
    pub async fn connect(addr: &str) -> Result<Self, WorkerError> {
        let channel = Channel::from_shared(addr.to_string())
            .map_err(|e| WorkerError::LlmProvider(e.to_string()))?
            .connect()
            .await
            .map_err(|e| WorkerError::LlmProvider(e.to_string()))?;
        let client = LmServiceClient::new(channel);
        tracing::info!(grpc_addr = %addr, "gRPC channel established");
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

    fn from_proto_health(resp: tonic::Response<HealthCheckResponse>) -> HealthStatus {
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
    #[tracing::instrument(skip(self, messages, params), fields(
        message_count = messages.len(),
        temperature = params.temperature,
        max_tokens = params.max_tokens,
        top_p = params.top_p,
        top_k = params.top_k,
    ))]
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

        let start = std::time::Instant::now();
        let mut client = self.client.clone();
        let result = client.generate(req).await;
        let elapsed_ms = start.elapsed().as_millis();

        match &result {
            Ok(resp) => {
                let inner = resp.get_ref();
                tracing::info!(
                    tokens_processed = inner.tokens_processed,
                    tokens_generated = inner.tokens_generated,
                    elapsed_ms,
                    "gRPC generate completed"
                );
            }
            Err(status) => {
                tracing::error!(
                    grpc_status = %status,
                    elapsed_ms,
                    "gRPC generate failed"
                );
            }
        }

        result
            .map(Self::from_proto_response)
            .map_err(WorkerError::Grpc)
    }

    #[tracing::instrument(skip(self))]
    async fn health_check(&self) -> Result<HealthStatus, WorkerError> {
        let start = std::time::Instant::now();
        let mut client = self.client.clone();
        let result = client
            .health_check(HealthCheckRequest {})
            .await;
        let elapsed_ms = start.elapsed().as_millis();

        match &result {
            Ok(resp) => {
                let inner = resp.get_ref();
                tracing::info!(
                    model_name = %inner.model_name,
                    is_ready = inner.is_ready,
                    context_length = inner.context_length,
                    elapsed_ms,
                    "Health check completed"
                );
            }
            Err(status) => {
                tracing::error!(
                    grpc_status = %status,
                    elapsed_ms,
                    "Health check failed"
                );
            }
        }

        result
            .map(Self::from_proto_health)
            .map_err(WorkerError::Grpc)
    }
}