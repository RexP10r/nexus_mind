pub mod dto;
pub mod http_client;

use async_trait::async_trait;

use crate::error::TuiError;
use self::dto::{AddDocsRequest, AddDocsResponse, ChatRequest, ChatResponse, HealthResponse};

#[async_trait]
pub trait ChatApi: Send + Sync {
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, TuiError>;
}

#[async_trait]
pub trait DocsApi: Send + Sync {
    async fn add_docs(&self, req: AddDocsRequest) -> Result<AddDocsResponse, TuiError>;
}

#[async_trait]
pub trait HealthApi: Send + Sync {
    async fn health_check(&self) -> Result<HealthResponse, TuiError>;
}
