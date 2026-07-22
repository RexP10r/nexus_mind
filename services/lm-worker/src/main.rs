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
use crate::config::{AgentType, Config, ProviderType};
use crate::error::WorkerError;
use crate::provider::grpc::GrpcLlmProvider;
use crate::server::AppState;
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
            let tool_registry = ToolRegistry::from_tools(vec![Box::new(CalculatorTool)]);
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

    let llm = init_llm(&config).await?;
    let agent = init_agent(llm, &config).await?;

    let state = AppState { agent };

    let router = server::build_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], config.http_port));

    tracing::info!(address = %addr, "HTTP server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
