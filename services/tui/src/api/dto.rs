#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDto {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub conversation_id: String,
    pub message: MessageDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    pub conversation_id: String,
    pub message: MessageDto,
}

#[derive(Debug, Serialize)]
pub struct DocumentInput {
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct AddDocsRequest {
    pub documents: Vec<DocumentInput>,
}

#[derive(Debug, Deserialize)]
pub struct AddDocsResponse {
    pub added: u64,
}

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub status: String,
}
