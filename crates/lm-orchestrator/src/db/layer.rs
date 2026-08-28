use super::cache::CacheStore;
use super::connection;
use super::conversation::ConversationStore;
use super::summary;
use super::timeline;
use crate::config::Config;
use crate::error::WorkerError;
use crate::model::{AgentResult, Message};

pub struct DbLayer {
    cache: CacheStore,
    conversation: ConversationStore,
}

impl DbLayer {
    pub async fn new(config: &Config) -> Result<Self, WorkerError> {
        let (redis_result, mongo_result) = tokio::join!(
            connection::connect_redis(&config.redis_url),
            connection::connect_mongo(&config.mongo_uri, &config.mongo_db),
        );

        let redis_conn = redis_result?;
        let collection = mongo_result?;

        tracing::info!(
            redis_url = %config.redis_url,
            mongo_uri = %config.mongo_uri,
            "Database layer initialized"
        );

        Ok(Self {
            cache: CacheStore::new(redis_conn, config.redis_ttl_secs),
            conversation: ConversationStore::new(collection),
        })
    }

    pub async fn append_turn_to_conversation(
        &self,
        conversation_id: &str,
        user_msg: &Message,
        agent_result: &AgentResult,
    ) -> Result<(), WorkerError> {
        let entries = timeline::build_timeline_entries(user_msg, agent_result);
        let tokens_added = agent_result.total_tokens;
        let entry_count = entries.len();

        self.conversation
            .append_timeline_entries(conversation_id, &entries, tokens_added)
            .await?;

        tracing::info!(
            conversation_id,
            entry_count,
            tokens_added,
            "Atomically appended turn to conversation"
        );
        Ok(())
    }
    pub async fn delete_cached_conversation(&self, conversation_id: &str) {
        if let Err(e) = self.cache.delete_conversation(conversation_id).await {
            tracing::error!(error = %e, conversation_id, "Failed to delete conversation from cache");
        }
    }

    pub async fn refresh_cache(&self, conversation_id: &str) {
        match self.conversation.get_conversation(conversation_id).await {
            Ok(Some(doc)) => {
                if let Err(e) = self.cache.cache_conversation(&doc).await {
                    tracing::error!(error = %e, conversation_id, "Failed to refresh Redis cache");
                }
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    conversation_id,
                    "Failed to read from MongoDB for cache refresh"
                );
            }
        }
    }

    pub async fn set_summary(
        &self,
        conversation_id: &str,
        summary: &str,
    ) -> Result<(), WorkerError> {
        self.conversation
            .set_summary(conversation_id, summary)
            .await?;

        Ok(())
    }

    pub async fn get_message_count(&self, conversation_id: &str) -> u32 {
        match self.conversation.get_message_count(conversation_id).await {
            Ok(count) => count as u32,
            Err(e) => {
                tracing::warn!(error = %e, conversation_id, "Failed to get message count, assuming 0");
                0
            }
        }
    }

    pub async fn get_messages(&self, conversation_id: &str) -> Vec<Message> {
        match self.cache.get_cached_conversation(conversation_id).await {
            Ok(Some(doc)) => return timeline::timeline_to_messages(&doc.timeline),
            Ok(None) => {}
            Err(e) => {
                tracing::debug!(error = %e, conversation_id, "Redis read failed, falling to MongoDB");
            }
        }

        self.get_messages_from_conversation_store(conversation_id).await
    }

    pub async fn get_messages_from_conversation_store(&self, conversation_id: &str) -> Vec<Message> {
        match self.conversation.get_conversation(conversation_id).await {
            Ok(Some(doc)) => timeline::timeline_to_messages(&doc.timeline),
            Ok(None) => Vec::new(),
            Err(e) => {
                tracing::warn!(error = %e, conversation_id, "MongoDB read failed");
                Vec::new()
            }
        }
    }

    pub async fn get_summary_text(&self, conversation_id: &str) -> Option<String> {
        match self.conversation.get_summary_field(conversation_id).await {
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
        if !summary::should_summarize(total, history_max_messages, summary_interval) {
            return;
        }

        let older = {
            let messages = self.get_messages_from_conversation_store(conversation_id).await;
            let keep = history_max_messages as usize;
            if messages.len() <= keep {
                return;
            }
            messages[..messages.len() - keep].to_vec()
        };

        let batch_size = summary_interval as usize;
        let new_batch: &[Message] = &older[older.len() - batch_size..];

        if new_batch.is_empty() {
            return;
        }

        let existing_summary = self.get_summary_text(conversation_id).await;
        let summary =
            match summary::generate_summary(llm, new_batch, existing_summary.as_deref()).await {
                Ok(s) => s,
                Err(_) => return,
            };

        if let Err(e) = self.set_summary(conversation_id, &summary).await {
            tracing::warn!(error = %e, "Failed to set summary");
        }
    }
}
