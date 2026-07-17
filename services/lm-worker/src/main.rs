mod agent;
mod config;
mod error;
mod grpc;
mod provider;
mod server;
mod traits;

use crate::agent::react::ReactAgent;
use crate::config::Config;
use crate::error::WorkerError;
use crate::provider::grpc::GrpcLlmProvider;
use crate::server::AppState;
use crate::traits::agent::Agent;
use crate::traits::llm::LlmProvider;
use std::net::SocketAddr;
use std::sync::Arc;

async fn verify_provider(provider: &Arc<dyn LlmProvider>) -> Result<(), WorkerError> {
    let health = provider.health_check().await;
    match &health {
        Ok(h) => {
            tracing::info!(
                "Model {} ready: {}, context_length: {}",
                h.model_name,
                h.is_ready,
                h.context_length
            );
            Ok(())
        }
        Err(e) => {
            tracing::error!("Health check failed: {}", e);
            Err(WorkerError::LlmProvider("Health check failed".to_string()))
        }
    }
}
async fn init_agent(config: &Config) -> Result<Arc<dyn Agent>, WorkerError> {
    tracing::info!("Connecting to gRPC server at {}...", config.grpc_addr);
    let llm = GrpcLlmProvider::connect(&config.grpc_addr).await?;

    let llm: Arc<dyn LlmProvider> = Arc::new(llm);
    verify_provider(&llm).await?;

    Ok(Arc::new(ReactAgent::new(Arc::clone(&llm))))
}
#[tokio::main]
async fn main() -> anyhow::Result<(), WorkerError> {
    tracing_subscriber::fmt::init();

    let config = Config::from_env();
    let agent = match init_agent(&config).await {
        Ok(agent) => agent,
        Err(e) => return Err(e),
    };

    let state = AppState { agent };

    let router = server::build_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], config.http_port));

    tracing::info!("HTTP server listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
