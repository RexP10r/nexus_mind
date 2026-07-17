pub mod routes;

use std::sync::Arc;

use crate::traits::agent::Agent;

#[derive(Clone)]
pub struct AppState {
    pub agent: Arc<dyn Agent>,
}

pub fn build_router(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/api/v1/chat", axum::routing::post(routes::chat))
        .route("/health", axum::routing::get(routes::health))
        .with_state(state)
}
