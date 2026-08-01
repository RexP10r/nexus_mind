pub mod conversation_doc;
pub mod history;
pub mod memory;

use mongodb::{Client as MongoClient, Collection};
use redis::aio::MultiplexedConnection;

use self::conversation_doc::create_ttl_index;
use self::history::HistoryStore;
use self::memory::MemoryStore;
use crate::config::Config;
use crate::error::WorkerError;
use crate::model::{
    AgentResult, ChatMessage, ChatRole, ConversationDoc, ConversationEntry, GenerationParams,
    Message,
};

fn timeline_to_messages(timeline: &[ConversationEntry]) -> Vec<Message> {
    timeline
        .iter()
        .filter_map(|entry| match entry {
            ConversationEntry::Message { role, content } => Some(Message {
                role: role.clone(),
                content: content.clone(),
            }),
            _ => None,
        })
        .collect()
}

pub struct DbLayer {
    memory: MemoryStore,
    history: HistoryStore,
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

async fn connect_mongo(
    uri: &str,
    db_name: &str,
) -> Result<Collection<ConversationDoc>, WorkerError> {
    let client = MongoClient::with_uri_str(uri).await.map_err(|e| {
        tracing::error!(
            uri,
            error = %e,
            "Invalid MongoDB URI. Expected format: mongodb://host:port"
        );
        WorkerError::Db(format!("Invalid MongoDB URI: {}", uri))
    })?;

    let db = client.database(db_name);
    let collection: Collection<ConversationDoc> = db.collection("conversations");

    create_ttl_index(&collection).await.map_err(|e| {
        tracing::error!(
            uri,
            db = db_name,
            error = %e,
            "Cannot connect to MongoDB. Start it with:\n  docker run -d --name mongo -p 27017:27017 mongo:7"
        );
        e
    })?;

    Ok(collection)
}

fn build_timeline_entries(
    user_msg: &Message,
    agent_result: &AgentResult,
) -> Vec<ConversationEntry> {
    let mut entries = Vec::new();

    entries.push(ConversationEntry::Message {
        role: user_msg.role.clone(),
        content: user_msg.content.clone(),
    });

    for step in &agent_result.reasoning_steps {
        entries.push(ConversationEntry::Step {
            thought: step.thought.clone(),
            action: step.action.clone(),
            observation: step.observation.clone(),
        });
    }

    if !agent_result.final_answer.is_empty() {
        entries.push(ConversationEntry::Message {
            role: "assistant".to_string(),
            content: agent_result.final_answer.clone(),
        });
    }

    entries
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

    pub async fn append_turn_to_conversation(
        &self,
        conversation_id: &str,
        user_msg: &Message,
        agent_result: &AgentResult,
    ) -> Result<(), WorkerError> {
        let entries = build_timeline_entries(user_msg, agent_result);
        let tokens_added = agent_result.total_tokens;
        let entry_count = entries.len();

        self.history
            .append_timeline_entries(conversation_id, &entries, tokens_added)
            .await?;

        if let Err(e) = self
            .memory
            .delete_cached_conversation(conversation_id)
            .await
        {
            tracing::warn!(error = %e, conversation_id, "Failed to invalidate Redis cache after append");
        }

        tracing::info!(
            conversation_id,
            entry_count,
            tokens_added,
            "Atomically appended turn to conversation"
        );
        Ok(())
    }

    pub async fn set_summary(
        &self,
        conversation_id: &str,
        summary: &str,
    ) -> Result<(), WorkerError> {
        self.history.set_summary(conversation_id, summary).await?;

        if let Err(e) = self
            .memory
            .delete_cached_conversation(conversation_id)
            .await
        {
            tracing::warn!(error = %e, conversation_id, "Failed to invalidate Redis cache after summary set");
        }

        Ok(())
    }

    pub async fn get_message_count(&self, conversation_id: &str) -> u32 {
        match self.history.get_message_count(conversation_id).await {
            Ok(count) => count as u32,
            Err(e) => {
                tracing::warn!(error = %e, conversation_id, "Failed to get message count, assuming 0");
                0
            }
        }
    }

    pub async fn get_messages(&self, conversation_id: &str) -> Vec<Message> {
        match self.memory.get_cached_conversation(conversation_id).await {
            Ok(Some(doc)) => return timeline_to_messages(&doc.timeline),
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(error = %e, conversation_id, "Redis read failed, falling to MongoDB");
            }
        }

        match self.history.get_conversation(conversation_id).await {
            Ok(Some(doc)) => {
                if let Err(e) = self.memory.cache_conversation(&doc).await {
                    tracing::error!(error = %e, conversation_id, "Failed to populate Redis cache");
                }
                timeline_to_messages(&doc.timeline)
            }
            Ok(None) => {
                tracing::info!(conversation_id, "No existing conversation, starting fresh");
                Vec::new()
            }
            Err(e) => {
                tracing::warn!(error = %e, conversation_id, "MongoDB read failed");
                Vec::new()
            }
        }
    }

    pub async fn get_summary_text(&self, conversation_id: &str) -> Option<String> {
        match self.history.get_summary_field(conversation_id).await {
            Ok(summary) => summary,
            Err(e) => {
                tracing::warn!(error = %e, conversation_id, "Failed to get summary");
                None
            }
        }
    }

    pub async fn update_summary(
        &self,
        llm: &dyn crate::traits::llm::LlmProvider,
        conversation_id: &str,
        history_max_messages: u32,
        summary_interval: u32,
    ) {
        let total = self.get_message_count(conversation_id).await as u64;
        if !should_summarize(total, history_max_messages, summary_interval) {
            return;
        }

        let older = {
            let messages = self.get_messages(conversation_id).await;
            let keep = history_max_messages as usize;
            if messages.len() <= keep {
                return;
            }
            messages[..messages.len() - keep].to_vec()
        };

        let batch_size = summary_interval as usize;
        let new_batch: &[Message] = if older.len() <= batch_size {
            &older[..]
        } else {
            &older[older.len() - batch_size..]
        };

        if new_batch.is_empty() {
            return;
        }

        let existing_summary = self.get_summary_text(conversation_id).await;
        let summary = match generate_summary(llm, new_batch, existing_summary.as_deref()).await {
            Ok(s) => s,
            Err(_) => return,
        };

        if let Err(e) = self.set_summary(conversation_id, &summary).await {
            tracing::warn!(error = %e, "Failed to set summary");
        }
    }
}

fn should_summarize(total: u64, history_max: u32, interval: u32) -> bool {
    total > history_max as u64 && (total - history_max as u64) % interval as u64 == 0
}

fn build_summarization_prompt(messages: &[Message], existing_summary: Option<&str>) -> String {
    let conversation_text: String = messages
        .iter()
        .map(|m| format!("[{}]: {}\n", m.role, m.content))
        .collect();

    let previous_summary = match existing_summary {
        Some(s) if !s.is_empty() => format!("\n## Previous Summary\n{}\n", s),
        _ => String::new(),
    };

    format!(
        r#"You are a conversation summarizer. Produce a concise, combined summary of the entire conversation history.
Output ONLY the summary text, no JSON, no formatting.
{previous_summary}
## New Messages to Incorporate
{conversation_text}
## Combined Summary"#,
        previous_summary = previous_summary,
        conversation_text = conversation_text
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
    existing_summary: Option<&str>,
) -> Result<String, WorkerError> {
    let prompt = build_summarization_prompt(messages, existing_summary);
    let chat_messages = vec![ChatMessage {
        role: ChatRole::User,
        content: prompt,
    }];

    llm.generate(chat_messages, &summary_params())
        .await
        .map(|o| o.text)
        .map_err(|e| {
            tracing::warn!(error = %e, "LLM summarization failed");
            e
        })
}
