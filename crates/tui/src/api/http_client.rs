use async_trait::async_trait;
use reqwest::Client;

use crate::api::dto::{AddDocsRequest, AddDocsResponse, ChatRequest, ChatResponse, ErrorResponse, HealthResponse};
use crate::api::{ChatApi, DocsApi, HealthApi};
use crate::error::TuiError;

pub struct HttpClient {
    client: Client,
    base_url: String,
}

impl HttpClient {
    pub fn new(base_url: &str) -> Result<Self, TuiError> {
        let client = Client::builder()
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    async fn post_json<Req: serde::Serialize, Resp: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &Req,
    ) -> Result<Resp, TuiError> {
        let url = format!("{}{}", self.base_url, path);
        let response = self.client.post(&url).json(body).send().await?;

        let status = response.status();
        if status.is_success() {
            let resp = response.json::<Resp>().await?;
            Ok(resp)
        } else {
            let error_body = response.json::<ErrorResponse>().await.ok();
            let message = error_body
                .map(|e| e.error)
                .unwrap_or_else(|| format!("HTTP {}", status));
            Err(TuiError::Server(message))
        }
    }

    async fn get_json<Resp: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<Resp, TuiError> {
        let url = format!("{}{}", self.base_url, path);
        let response = self.client.get(&url).send().await?;

        let status = response.status();
        if status.is_success() {
            let resp = response.json::<Resp>().await?;
            Ok(resp)
        } else {
            Err(TuiError::Server(format!("HTTP {}", status)))
        }
    }
}

#[async_trait]
impl ChatApi for HttpClient {
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, TuiError> {
        self.post_json("/api/chat", &req).await
    }
}

#[async_trait]
impl DocsApi for HttpClient {
    async fn add_docs(&self, req: AddDocsRequest) -> Result<AddDocsResponse, TuiError> {
        self.post_json("/api/docs/add", &req).await
    }
}

#[async_trait]
impl HealthApi for HttpClient {
    async fn health_check(&self) -> Result<HealthResponse, TuiError> {
        self.get_json("/health").await
    }
}
