use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use std::sync::Arc;

use crate::db::{update_summary, DbLayer};
use crate::model::{AgentResult, ConversationDoc, GenerationParams, Message};
use crate::server::dto::{ChatRequest, ChatResponse, ErrorResponse, HealthResponse};
use crate::server::AppState;

struct ChatContext {
    conversation_id: String,
    conversation_doc: ConversationDoc,
    new_message: Message,
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

async fn load_conversation(db: &DbLayer, conversation_id: &str) -> ConversationDoc {
    match db.load_conversation(conversation_id).await {
        Ok(Some(doc)) => {
            tracing::info!(
                conversation_id,
                timeline_entries = doc.timeline.len(),
                "Loaded conversation"
            );
            doc
        }
        Ok(None) => {
            tracing::info!(conversation_id, "No existing conversation, starting fresh");
            ConversationDoc::new(conversation_id.to_string())
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to load conversation, starting fresh");
            ConversationDoc::new(conversation_id.to_string())
        }
    }
}

async fn build_chat_context(state: &AppState, req: &ChatRequest) -> ChatContext {
    let conversation_id = req.conversation_id.clone();
    let new_message = req.message.clone();

    let conversation_doc = load_conversation(&state.db, &conversation_id).await;

    ChatContext {
        conversation_id,
        conversation_doc,
        new_message,
    }
}

fn collect_agent_messages(ctx: &ChatContext) -> Vec<Message> {
    let mut messages = ctx.conversation_doc.to_messages();
    messages.push(ctx.new_message.clone());
    messages
}

fn log_request_params(params: &GenerationParams) {
    tracing::info!(
        temperature = format!("{:.3}", params.temperature),
        max_tokens = params.max_tokens,
        top_p = format!("{:.3}", params.top_p),
        top_k = params.top_k,
        "Processing chat request"
    );
}

fn spawn_save_conversation(db: Arc<DbLayer>, conversation_id: String, new_message: Message, agent_result: AgentResult) {
    tokio::spawn(async move {
        let mut doc = match db.load_conversation(&conversation_id).await {
            Ok(Some(d)) => d,
            Ok(None) => ConversationDoc::new(conversation_id.clone()),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load conversation for appending, saving fresh");
                let mut d = ConversationDoc::new(conversation_id.clone());
                d.append_turn(&new_message, &agent_result);
                let _ = db.save_conversation(&d).await;
                return;
            }
        };
        doc.append_turn(&new_message, &agent_result);
        if let Err(e) = db.save_conversation(&doc).await {
            tracing::warn!(error = %e, "Failed to save conversation");
        }
    });
}

fn spawn_summary_update(state: &AppState, conversation_id: String) {
    let db = state.db.clone();
    let llm = state.llm.clone();
    let history_max = state.config.history_max_messages;
    let summary_interval = state.config.summary_interval;

    tokio::spawn(async move {
        update_summary(&db, llm.as_ref(), &conversation_id, history_max, summary_interval).await;
    });
}

fn success_response(conversation_id: String, answer: String) -> (StatusCode, Json<serde_json::Value>) {
    let response = ChatResponse {
        conversation_id,
        message: Message {
            role: "assistant".into(),
            content: answer,
        },
    };
    (StatusCode::OK, Json(serde_json::to_value(&response).unwrap()))
}

fn error_response(error: String) -> (StatusCode, Json<serde_json::Value>) {
    let err = ErrorResponse {
        error,
        status: "error".into(),
    };
    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::to_value(&err).unwrap()))
}

fn handle_agent_result(
    state: &AppState,
    ctx: &ChatContext,
    result: Result<AgentResult, crate::error::WorkerError>,
    elapsed_ms: u128,
) -> (StatusCode, Json<serde_json::Value>) {
    match result {
        Ok(agent_result) => {
            tracing::info!(
                total_tokens = agent_result.total_tokens,
                reasoning_steps = agent_result.reasoning_steps.len(),
                elapsed_ms,
                "Chat completed successfully"
            );

            spawn_save_conversation(
                state.db.clone(),
                ctx.conversation_id.clone(),
                ctx.new_message.clone(),
                agent_result.clone(),
            );
            spawn_summary_update(state, ctx.conversation_id.clone());

            success_response(ctx.conversation_id.clone(), agent_result.final_answer)
        }
        Err(e) => {
            tracing::error!(error = %e, elapsed_ms, "Chat request failed");
            error_response(e.to_string())
        }
    }
}

#[tracing::instrument(skip(state, req), fields(conversation_id = %req.conversation_id))]
pub async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let ctx = build_chat_context(&state, &req).await;
    let messages = collect_agent_messages(&ctx);
    let params = req.to_generation_params(GenerationParams::default());
    log_request_params(&params);

    let start = std::time::Instant::now();
    let result = state.agent.run(&messages, ctx.conversation_doc.summary.as_deref(), &params).await;

    handle_agent_result(&state, &ctx, result, start.elapsed().as_millis())
}

pub async fn health(State(_state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let health = HealthResponse {
        status: "ok".into(),
    };
    (StatusCode::OK, Json(serde_json::to_value(&health).unwrap()))
}
