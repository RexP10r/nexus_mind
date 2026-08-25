use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use std::sync::Arc;

use crate::model::{AgentResult, ChatRole, Document, GenerationParams, Message};
use crate::server::AppState;
use crate::server::dto::{
    AddDocsRequest, AddDocsResponse, ChatRequest, ChatResponse, ErrorResponse, HealthResponse,
    SearchRequest, SearchResponse,
};

fn doc_id(text: &str) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    format!("{:x}", h.finish())
}

struct ChatContext {
    conversation_id: Arc<String>,
    messages: Vec<Message>,
    summary: Option<String>,
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

async fn build_chat_context(state: &AppState, req: &ChatRequest) -> ChatContext {
    let conversation_id = Arc::new(req.conversation_id.clone());
    let new_message = req.message.clone();
    let history_max_size = state.config.history_max_messages as usize;

    let messages = {
        let full_history = state.db.get_messages(&conversation_id).await;
        let start = full_history.len().saturating_sub(history_max_size);
        full_history[start..].to_vec()
    };
    let summary = state.db.get_summary_text(&conversation_id).await;

    ChatContext {
        conversation_id,
        messages,
        summary,
        new_message,
    }
}

fn collect_agent_messages(ctx: &ChatContext) -> Vec<Message> {
    let mut messages = ctx.messages.clone();
    messages.push(ctx.new_message.clone());
    messages
}

fn success_response(
    conversation_id: String,
    answer: String,
) -> (StatusCode, Json<serde_json::Value>) {
    let response = ChatResponse {
        conversation_id,
        message: Message {
            role: ChatRole::Assistant,
            content: answer,
        },
    };
    (
        StatusCode::OK,
        Json(serde_json::to_value(&response).unwrap()),
    )
}

fn error_response(error: String) -> (StatusCode, Json<serde_json::Value>) {
    let err = ErrorResponse {
        error,
        status: "error".into(),
    };
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::to_value(&err).unwrap()),
    )
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

            let db = state.db.clone();
            let llm = state.llm.clone();
            let history_max = state.config.history_max_messages;
            let summary_interval = state.config.summary_interval;
            let conversation_id = ctx.conversation_id.clone();
            let new_message = ctx.new_message.clone();
            let agent_result_clone = agent_result.clone();
            tokio::spawn(async move {
                db.delete_cached_conversation(&conversation_id).await;

                if let Err(e) = db
                    .append_turn_to_conversation(
                        &conversation_id,
                        &new_message,
                        &agent_result_clone,
                    )
                    .await
                {
                    tracing::error!(
                        error = %e,
                        conversation_id = %conversation_id,
                        "Failed to persist conversation turn — data lost"
                    );
                    return;
                }
                db.update_summary(
                    llm.as_ref(),
                    &conversation_id,
                    history_max,
                    summary_interval,
                )
                .await;
                db.refresh_cache(&conversation_id).await;
            });

            success_response(ctx.conversation_id.to_string(), agent_result.final_answer)
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

    let start = std::time::Instant::now();
    let result = state
        .agent
        .run(&messages, ctx.summary.as_deref(), &params)
        .await;

    handle_agent_result(&state, &ctx, result, start.elapsed().as_millis())
}

pub async fn health(State(_state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let health = HealthResponse {
        status: "ok".into(),
    };
    (StatusCode::OK, Json(serde_json::to_value(&health).unwrap()))
}

pub async fn add_docs(
    State(state): State<AppState>,
    Json(req): Json<AddDocsRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if req.documents.is_empty() {
        let err = ErrorResponse {
            error: "No documents provided".to_string(),
            status: "error".to_string(),
        };
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::to_value(&err).unwrap()),
        );
    }

    let docs: Vec<Document> = req
        .documents
        .into_iter()
        .map(|d| Document {
            id: doc_id(&d.text),
            text: d.text,
        })
        .collect();

    let count = docs.len();
    let added_count = match state.vector_store.add_docs(&docs).await {
        Ok(added) => {
            tracing::info!(added, requested = count, "Documents added via API");
            added
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to add documents");
            let err = ErrorResponse {
                error: e.to_string(),
                status: "error".to_string(),
            };
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::to_value(&err).unwrap()),
            );
        }
    };

    let docs_texts = docs.iter().map(|d| d.text.clone()).collect::<Vec<_>>();
    let store_cloned = state.vector_store.clone();
    tokio::spawn(async move {
        if let Err(e) = store_cloned.update_vocab_with_new_docs(&docs_texts).await {
            tracing::error!(error = %e, "Vocab update failed (non-critical)")
        }
    });

    tokio::spawn(async move {
        match state.vector_store.recompute_all_vectors().await {
            Ok(n) => tracing::info!(count = n, "Sparse vectors recomputed"),
            Err(e) => tracing::error!(error = %e, "Recompute failed"),
        }
    });

    let resp = AddDocsResponse { added: added_count };
    (StatusCode::OK, Json(serde_json::to_value(&resp).unwrap()))
}

pub async fn search_sparse(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if req.query.is_empty() {
        let err = ErrorResponse {
            error: "Query must not be empty".to_string(),
            status: "error".to_string(),
        };
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::to_value(&err).unwrap()),
        );
    }

    match state.vector_store.search_tfidf(&req.query, req.limit).await {
        Ok(results) => {
            tracing::info!(
                query = %req.query,
                results_count = results.len(),
                "Sparse search completed"
            );
            let resp = SearchResponse { results };
            (StatusCode::OK, Json(serde_json::to_value(&resp).unwrap()))
        }
        Err(e) => {
            tracing::error!(error = %e, "Sparse search failed");
            let err = ErrorResponse {
                error: e.to_string(),
                status: "error".to_string(),
            };
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::to_value(&err).unwrap()),
            )
        }
    }
}

pub async fn search_dense(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if req.query.is_empty() {
        let err = ErrorResponse {
            error: "Query must not be empty".to_string(),
            status: "error".to_string(),
        };
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::to_value(&err).unwrap()),
        );
    }

    match state.vector_store.search_lm(&req.query, req.limit).await {
        Ok(results) => {
            tracing::info!(
                query = %req.query,
                results_count = results.len(),
                "Dense search completed"
            );
            let resp = SearchResponse { results };
            (StatusCode::OK, Json(serde_json::to_value(&resp).unwrap()))
        }
        Err(e) => {
            tracing::error!(error = %e, "Dense search failed");
            let err = ErrorResponse {
                error: e.to_string(),
                status: "error".to_string(),
            };
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::to_value(&err).unwrap()),
            )
        }
    }
}
