use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::agent::Message;
use crate::server::AppState;
use crate::common::GenerationParams;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    #[serde(default)]
    pub conversation_id: Option<String>,
    pub messages: Vec<Message>,
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

impl ChatRequest {
    fn to_generation_params(&self, defaults: GenerationParams) -> GenerationParams {
        GenerationParams {
            temperature: self.temperature.unwrap_or(defaults.temperature),
            max_tokens: self.max_tokens.unwrap_or(defaults.max_tokens),
            top_p: self.top_p.unwrap_or(defaults.top_p),
            top_k: self.top_k.unwrap_or(defaults.top_k),
        }
    }
}

pub async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Json<serde_json::Value> {
    let default_params = GenerationParams::default();
    let params = req.to_generation_params(default_params);

    match state.agent.run(&req.messages, &params).await {
        Ok(agent_result) => {
            let conversation_id = req
                .conversation_id
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let response = ChatResponse {
                conversation_id,
                message: Message {
                    role: "assistant".into(),
                    content: agent_result.final_answer,
                },
            };

            Json(serde_json::to_value(&response).unwrap())
        }
        Err(e) => {
            let error = ErrorResponse {
                error: e.to_string(),
                status: "error".into(),
            };
            Json(serde_json::to_value(&error).unwrap())
        }
    }
}

pub async fn health(State(_state): State<AppState>) -> Json<serde_json::Value> {
    let health = HealthResponse {
        status: "ok".into(),
    };
    Json(serde_json::to_value(&health).unwrap())
}
