use serde::{Deserialize, Serialize};

use crate::model::Message;
use crate::model::SearchResult;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub conversation_id: String,
    pub message: Message,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub conversation_id: String,
    pub message: Message,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct DocumentInput {
    pub text: String,
    pub name: String,
    pub file_format: String,
}

#[derive(Debug, Deserialize)]
pub struct AddDocsRequest {
    pub documents: Vec<DocumentInput>,
}

#[derive(Debug, Serialize)]
pub struct AddDocsResponse {
    pub added: u64,
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: u64,
}

fn default_search_limit() -> u64 {
    5
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}
