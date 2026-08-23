mod agent;
mod config;
mod db;
mod embeddings;
mod error;
mod grpc;
mod model;
mod provider;
mod server;
mod tools;
mod traits;
mod vector;

use crate::agent::rag::RAGAgent;
use crate::config::{AgentType, Config, ProviderType};
use crate::db::DbLayer;
use crate::embeddings::dense::EmbedderLMProvider;
use crate::embeddings::provider::EmbeddingProviders;
use crate::embeddings::sparse::TfIdfProvider;
use crate::error::WorkerError;
use crate::provider::grpc::GrpcLlmProvider;
use crate::server::AppState;
use crate::tools::calculator::CalculatorTool;
use crate::tools::registry::InMemoryToolRegistry;
use crate::tools::search_lm::SearchLMTool;
use crate::tools::search_tfidf::SearchTfIdfTool;
use crate::traits::agent::Agent;
use crate::traits::llm::LlmProvider;
use crate::vector::qdrant::{QdrantVectorStore, ensure_collection};
use qdrant_client::Qdrant;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

fn init_tracing(config: &Config) -> Result<(), WorkerError> {
    let env_filter =
        EnvFilter::try_from_default_env().map_err(|e| WorkerError::Environment(e.to_string()))?;

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
    Ok(())
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

async fn init_vector_store(config: &Config) -> Result<Arc<QdrantVectorStore>, WorkerError> {
    tracing::info!(
        qdrant_url = %config.qdrant_url,
        collection = %config.qdrant_collection_name,
        "Connecting to Qdrant"
    );

    let client = Qdrant::from_url(&config.qdrant_url)
        .build()
        .map_err(|e| WorkerError::Qdrant(format!("Failed to connect to Qdrant: {}", e)))?;

    let _ = ensure_collection(&client, &config.qdrant_collection_name).await?;

    let tfidf_provider = {
        let vocab =
            crate::vector::qdrant::get_collection_vocab(&client, &config.qdrant_collection_name)
                .await?
                .unwrap_or_default();
        tracing::info!(
            vocab_terms = vocab.term_to_index.len(),
            total_docs = vocab.total_docs,
            "Loaded TF-IDF vocabulary from Qdrant"
        );
        TfIdfProvider::new(vocab)
    };

    let embedder_lm_provider = {
        tracing::info!(
            model_path = %config.embedding_model_path,
            tokenizer_path = %config.embedding_tokenizer_path,
            "Loading Embedder LM ONNX model"
        );
        EmbedderLMProvider::from_files(&config)?
    };

    let embeddings = EmbeddingProviders::new(tfidf_provider, embedder_lm_provider);

    let store =
        QdrantVectorStore::new(client, config.qdrant_collection_name.clone(), embeddings).await?;

    tracing::info!("Vector store initialized");

    Ok(Arc::new(store))
}

async fn init_agent(
    llm: Arc<dyn LlmProvider>,
    vector_store: Arc<QdrantVectorStore>,
    config: &Config,
) -> Result<Arc<dyn Agent>, WorkerError> {
    let agent: Arc<dyn Agent> = match config.agent_type {
        AgentType::Rag => {
            let tool_registry = InMemoryToolRegistry::from_tools(vec![
                Box::new(CalculatorTool),
                Box::new(SearchTfIdfTool::new(Arc::clone(&vector_store))),
                Box::new(SearchLMTool::new(Arc::clone(&vector_store))),
            ]);
            let tool_count = tool_registry.tool_count();
            tracing::info!(
                agent_type = "rag",
                max_iterations = config.max_iterations,
                request_timeout = config.request_timeout,
                tool_count,
                "Initializing agent"
            );
            Arc::new(RAGAgent::new(
                llm,
                tool_registry,
                config.max_iterations,
                config.request_timeout,
            ))
        }
    };
    Ok(agent)
}

#[tokio::main]
async fn main() -> anyhow::Result<(), WorkerError> {
    dotenvy::dotenv().ok();
    let config = Config::from_env();
    let _ = init_tracing(&config);

    let db = match DbLayer::new(&config).await {
        Ok(db) => db,
        Err(e) => {
            tracing::error!(error = %e, "Database layer initialization failed, shutting down");
            return Err(e);
        }
    };

    let vector_store = init_vector_store(&config).await?;
    let llm = init_llm(&config).await?;
    let agent = init_agent(Arc::clone(&llm), Arc::clone(&vector_store), &config).await?;

    let state = AppState {
        agent,
        llm,
        db: Arc::new(db),
        vector_store,
        config: config.clone(),
    };

    let router = server::build_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], config.http_port));

    tracing::info!(address = %addr, "HTTP server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
