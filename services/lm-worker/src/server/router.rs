use std::time::Duration;

use tower_http::trace::TraceLayer;
use tracing::Span;

use super::routes;
use super::state::AppState;

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
        .on_request(|_: &axum::http::Request<axum::body::Body>, _: &Span| {})
        .on_response(|_: &axum::http::Response<axum::body::Body>, _: Duration, _: &Span| {});

    axum::Router::new()
        .route("/api/v1/chat", axum::routing::post(routes::chat))
        .route("/api/v1/docs", axum::routing::post(routes::add_docs))
        .route(
            "/api/v1/docs/search/sparse",
            axum::routing::post(routes::search_sparse),
        )
        .route(
            "/api/v1/docs/search/dense",
            axum::routing::post(routes::search_dense),
        )
        .route("/health", axum::routing::get(routes::health))
        .layer(trace_layer)
        .with_state(state)
}
