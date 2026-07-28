pub mod history;
pub mod memory;
pub mod message_doc;

use mongodb::bson::doc;
use mongodb::{Client as MongoClient, Collection};
use redis::aio::MultiplexedConnection;

use self::history::HistoryStore;
use self::memory::MemoryStore;
use self::message_doc::MessageDoc;
use crate::config::Config;
use crate::error::WorkerError;
use crate::model::{GenerationParams, LlmMessage, LlmRole, Message};

pub struct DbLayer {
    pub memory: MemoryStore,
    pub history: HistoryStore,
}

async fn connect_redis(url: &str) -> Result<MultiplexedConnection, WorkerError> {
    let client = redis::Client::open(url).map_err(|e| {
        tracing::error!(
            url,
            error = %e,
            "Invalid Redis URL. Expected format: redis://host:port"
        );
        WorkerError::Db(format!("Invalid Redis URL: {}", url))
    })?;

    client
        .get_multiplexed_tokio_connection()
        .await
        .map_err(|e| {
            tracing::error!(
                url,
                error = %e,
                "Cannot connect to Redis. Start it with:\n  docker run -d --name redis -p 6379:6379 redis:7-alpine"
            );
            WorkerError::Db(format!("Redis unavailable at {}", url))
        })
}

async fn connect_mongo(uri: &str, db_name: &str) -> Result<Collection<MessageDoc>, WorkerError> {
    let client = MongoClient::with_uri_str(uri).await.map_err(|e| {
        tracing::error!(
            uri,
            error = %e,
            "Invalid MongoDB URI. Expected format: mongodb://host:port"
        );
        WorkerError::Db(format!("Invalid MongoDB URI: {}", uri))
    })?;

    let db = client.database(db_name);
    let collection: Collection<MessageDoc> = db.collection("messages");

    collection
        .create_index(
            mongodb::IndexModel::builder()
                .keys(doc! { "conversation_id": 1, "timestamp": 1 })
                .build(),
        )
        .await
        .map_err(|e| {
            tracing::error!(
                uri,
                db = db_name,
                error = %e,
                "Cannot connect to MongoDB. Start it with:\n  docker run -d --name mongo -p 27017:27017 mongo:7"
            );
            WorkerError::Db(format!("MongoDB unavailable at {}", uri))
        })?;

    Ok(collection)
}

impl DbLayer {
    pub async fn new(config: &Config) -> Result<Self, WorkerError> {
        let (redis_result, mongo_result) = tokio::join!(
            connect_redis(&config.redis_url),
            connect_mongo(&config.mongo_uri, &config.mongo_db),
        );

        let redis_conn = redis_result?;
        let collection = mongo_result?;

        tracing::info!(
            redis_url = %config.redis_url,
            mongo_uri = %config.mongo_uri,
            "Database layer initialized"
        );

        Ok(Self {
            memory: MemoryStore::new(redis_conn, config.redis_ttl_secs),
            history: HistoryStore::new(collection),
        })
    }
}

fn should_summarize(total: u64, history_max: u32, interval: u32) -> bool {
    total > history_max as u64 && (total - history_max as u64) % interval as u64 == 0
}

fn build_summarization_prompt(messages: &[Message]) -> String {
    let conversation_text: String = messages
        .iter()
        .map(|m| format!("[{}]: {}\n", m.role, m.content))
        .collect();

    format!(
        r#"You are a conversation summarizer. Create a concise summary of the conversation below.
Focus on: key facts mentioned, decisions made, user's goals, and important context.
Output ONLY the summary text, no JSON, no formatting.

## Conversation
{}
## Summary"#,
        conversation_text
    )
}

fn summary_params() -> GenerationParams {
    GenerationParams {
        temperature: 0.2,
        max_tokens: 256,
        top_p: 0.9,
        top_k: 32,
    }
}

async fn generate_summary(
    llm: &dyn crate::traits::llm::LlmProvider,
    messages: &[Message],
) -> Result<String, WorkerError> {
    let prompt = build_summarization_prompt(messages);
    let llm_messages = vec![LlmMessage {
        role: LlmRole::User,
        content: prompt,
    }];

    llm.generate(llm_messages, &summary_params())
        .await
        .map(|o| o.text)
        .map_err(|e| {
            tracing::warn!(error = %e, "LLM summarization failed");
            e
        })
}

async fn fetch_older_messages(
    history: &HistoryStore,
    conversation_id: &str,
    history_max: u32,
) -> Result<Vec<Message>, WorkerError> {
    history
        .get_older_messages(conversation_id, history_max)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "Failed to load older messages for summarization");
            e
        })
}

pub async fn update_summary(
    db: &DbLayer,
    llm: &dyn crate::traits::llm::LlmProvider,
    conversation_id: &str,
    history_max_messages: u32,
    summary_interval: u32,
) {
    let total = match db.history.count_messages(conversation_id).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to count messages for summary check");
            return;
        }
    };

    if !should_summarize(total, history_max_messages, summary_interval) {
        return;
    }

    tracing::info!(
        total,
        trigger = format!(
            "every {} messages outside window of {}",
            summary_interval, history_max_messages
        ),
        "Triggering background summary update"
    );

    let older = match fetch_older_messages(&db.history, conversation_id, history_max_messages).await
    {
        Ok(m) if m.is_empty() => return,
        Ok(m) => m,
        Err(_) => return,
    };

    let summary = match generate_summary(llm, &older).await {
        Ok(s) => s,
        Err(_) => return,
    };

    if let Err(e) = db.memory.set_summary(conversation_id, &summary).await {
        tracing::warn!(error = %e, "Failed to save summary to Redis");
    } else {
        tracing::info!("Background summary updated successfully");
    }
}
