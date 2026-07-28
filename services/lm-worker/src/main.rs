mod agent;
mod config;
mod db;
mod error;
mod grpc;
mod model;
mod provider;
mod server;
mod tools;
mod traits;

use crate::agent::rag::RAGAgent;
use crate::config::{AgentType, Config, ProviderType};
use crate::db::DbLayer;
use crate::error::WorkerError;
use crate::provider::grpc::GrpcLlmProvider;
use crate::server::AppState;
use crate::tools::calculator::CalculatorTool;
use crate::tools::registry::InMemoryToolRegistry;
use crate::traits::agent::Agent;
use crate::traits::llm::LlmProvider;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;

fn init_tracing(config: &Config) {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_span_events(FmtSpan::CLOSE)
        .with_target(false);

    if config.log_json {
        subscriber.json().init();
    } else {
        subscriber.pretty().init();
    }

    tracing_log::LogTracer::init().ok();
}

async fn verify_provider(provider: &Arc<dyn LlmProvider>) -> Result<(), WorkerError> {
    let health = provider.health_check().await;
    match &health {
        Ok(h) => {
            tracing::info!(
                model_name = %h.model_name,
                ready = h.is_ready,
                context_length = h.context_length,
                "Model ready"
            );
            Ok(())
        }
        Err(e) => {
            tracing::error!(error = %e, "Health check failed");
            Err(WorkerError::LlmProvider("Health check failed".to_string()))
        }
    }
}

async fn init_llm(config: &Config) -> Result<Arc<dyn LlmProvider>, WorkerError> {
    let llm: Arc<dyn LlmProvider> = match config.provider_type {
        ProviderType::Grpc => {
            tracing::info!(grpc_addr = %config.grpc_addr, "Connecting to gRPC server");
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
            let tool_registry = InMemoryToolRegistry::from_tools(vec![Box::new(CalculatorTool)]);
            let tool_count = tool_registry.tool_count();
            tracing::info!(
                agent_type = "rag",
                max_iterations = config.max_iterations,
                request_timeout_secs = config.request_timeout_secs,
                tool_count,
                "Initializing agent"
            );
            Arc::new(RAGAgent::new(
                llm,
                tool_registry,
                config.max_iterations,
                config.request_timeout_secs,
            ))
        }
    };
    Ok(agent)
}

#[tokio::main]
async fn main() -> anyhow::Result<(), WorkerError> {
    let config = Config::from_env();
    init_tracing(&config);

    let db = match DbLayer::new(&config).await {
        Ok(db) => db,
        Err(e) => {
            tracing::error!(error = %e, "Database layer initialization failed, shutting down");
            return Err(e);
        }
    };

    let llm = init_llm(&config).await?;
    let llm_for_state = Arc::clone(&llm);
    let agent = init_agent(llm, &config).await?;

    let http_port = config.http_port;

    let state = AppState {
        agent,
        llm: llm_for_state,
        db: Arc::new(db),
        config,
    };

    let router = server::build_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], http_port));

    tracing::info!(address = %addr, "HTTP server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
