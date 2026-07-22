use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use crate::agent::Message;
use crate::common::GenerationParams;
use crate::server::dto::{ChatRequest, ChatResponse, ErrorResponse, HealthResponse};
use crate::server::AppState;

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

#[tracing::instrument(skip(state, req), fields(
    conversation_id = tracing::field::Empty,
    message_count = req.messages.len(),
))]
pub async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let conversation_id = req
        .conversation_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    tracing::Span::current()
        .record("conversation_id", &conversation_id);

    let default_params = GenerationParams::default();
    let params = req.to_generation_params(default_params);

    let temperature_fmt = format!("{:.3}", params.temperature);
    let top_p_fmt = format!("{:.3}", params.top_p);

    tracing::info!(
        temperature = %temperature_fmt,
        max_tokens = params.max_tokens,
        top_p = %top_p_fmt,
        top_k = params.top_k,
        "Processing chat request"
    );

    let start = std::time::Instant::now();
    let result = state.agent.run(&req.messages, &params).await;
    let elapsed_ms = start.elapsed().as_millis();

    match result {
        Ok(agent_result) => {
            tracing::info!(
                total_tokens = agent_result.total_tokens,
                reasoning_steps = agent_result.reasoning_steps.len(),
                elapsed_ms,
                "Chat completed successfully"
            );

            let response = ChatResponse {
                conversation_id,
                message: Message {
                    role: "assistant".into(),
                    content: agent_result.final_answer,
                },
            };

            (StatusCode::OK, Json(serde_json::to_value(&response).unwrap()))
        }
        Err(e) => {
            tracing::error!(
                error = %e,
                elapsed_ms,
                "Chat request failed"
            );

            let error = ErrorResponse {
                error: e.to_string(),
                status: "error".into(),
            };
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::to_value(&error).unwrap()),
            )
        }
    }
}

pub async fn health(State(_state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let health = HealthResponse {
        status: "ok".into(),
    };
    (StatusCode::OK, Json(serde_json::to_value(&health).unwrap()))
}
