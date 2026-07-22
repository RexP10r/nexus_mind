mod agent;
mod common;
mod config;
mod error;
mod grpc;
mod provider;
mod server;

use crate::agent::rag::RAGAgent;
use crate::common::tools::calculator::CalculatorTool;
use crate::common::tools::registry::ToolRegistry;
use crate::common::traits::agent::Agent;
use crate::common::traits::llm::LlmProvider;
use crate::config::{Config, ProviderType, AgentType};
use crate::error::WorkerError;
use crate::provider::grpc::GrpcLlmProvider;
use crate::server::AppState;
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

async fn init_llm(config: &Config) -> Result<Arc<dyn LlmProvider>, WorkerError> {
    let llm: Arc<dyn LlmProvider> = match config.provider_type {
        ProviderType::Grpc => {
            tracing::info!("Connecting to gRPC server at {}...", config.grpc_addr);
            let provider = GrpcLlmProvider::connect(&config.grpc_addr).await?;
            Arc::new(provider)
        }
    };
    verify_provider(&llm).await?;
    Ok(llm)
}

async fn init_agent(
    llm: Arc<dyn LlmProvider>,
    config: &Config,
) -> Result<Arc<dyn Agent>, WorkerError> {
    let agent: Arc<dyn Agent> = match config.agent_type {
        AgentType::RAG => {
            let tool_registry = ToolRegistry::from_tools(vec![Box::new(CalculatorTool)]);
            Arc::new(RAGAgent::new(llm, tool_registry, config.max_iterations))
        }
    };
    Ok(agent)
}

#[tokio::main]
async fn main() -> anyhow::Result<(), WorkerError> {
    tracing_subscriber::fmt::init();

    let config = Config::from_env();
    let llm = init_llm(&config).await?;
    let agent = init_agent(llm, &config).await?;

    let state = AppState { agent };

    let router = server::build_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], config.http_port));

    tracing::info!("HTTP server listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
