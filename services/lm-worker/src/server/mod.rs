pub mod dto;
pub mod routes;

use std::sync::Arc;
use std::time::Duration;

use tower_http::trace::TraceLayer;
use tracing::Span;

use crate::common::traits::agent::Agent;

#[derive(Clone)]
pub struct AppState {
    pub agent: Arc<dyn Agent>,
}

pub fn build_router(state: AppState) -> axum::Router {
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(|request: &axum::http::Request<axum::body::Body>| {
            let request_id = request
                .headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            tracing::info_span!(
                "http_request",
                request_id = %request_id,
                method = %request.method(),
                uri = %request.uri(),
            )
        })
        .on_request(|_request: &axum::http::Request<axum::body::Body>, _span: &Span| {
            tracing::info!("request started");
        })
        .on_response(
            |response: &axum::http::Response<axum::body::Body>, latency: Duration, _span: &Span| {
                tracing::info!(
                    status = response.status().as_u16(),
                    latency_ms = latency.as_millis(),
                    "request completed"
                );
            },
        );

    axum::Router::new()
        .route("/api/v1/chat", axum::routing::post(routes::chat))
        .route("/health", axum::routing::get(routes::health))
        .layer(trace_layer)
        .with_state(state)
}
