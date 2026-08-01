use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;

use crate::error::WorkerError;
use crate::model::ConversationDoc;

const CONVERSATION_KEY_PREFIX: &str = "conv:";

pub struct MemoryStore {
    conn: MultiplexedConnection,
    ttl_secs: u64,
}

impl MemoryStore {
    pub fn new(conn: MultiplexedConnection, ttl_secs: u64) -> Self {
        Self { conn, ttl_secs }
    }

    fn conversation_key(conversation_id: &str) -> String {
        format!("{}{}", CONVERSATION_KEY_PREFIX, conversation_id)
    }

    #[tracing::instrument(skip(self, doc), fields(conversation_id = %doc.conversation_id))]
    pub async fn cache_conversation(&self, doc: &ConversationDoc) -> Result<(), WorkerError> {
        let json = serde_json::to_string(doc)
            .map_err(|e| WorkerError::Db(format!("Serialize error: {}", e)))?;
        let mut conn = self.conn.clone();
        let key = Self::conversation_key(&doc.conversation_id);
        conn.set::<_, _, ()>(&key, &json)
            .await
            .map_err(|e| WorkerError::Db(format!("Redis set error: {}", e)))?;
        if self.ttl_secs > 0 {
            conn.expire::<_, ()>(&key, self.ttl_secs as i64)
                .await
                .map_err(|e| WorkerError::Db(format!("Redis expire error: {}", e)))?;
        }
        tracing::info!(
            conversation_id = %doc.conversation_id,
            ttl_secs = self.ttl_secs,
            "Cached conversation in Redis"
        );
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %conversation_id))]
    pub async fn delete_cached_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<(), WorkerError> {
        let mut conn = self.conn.clone();
        let key = Self::conversation_key(conversation_id);
        conn.del::<_, ()>(&key)
            .await
            .map_err(|e| WorkerError::Db(format!("Redis del error: {}", e)))?;
        tracing::info!(conversation_id, "Invalidated Redis cache for conversation");
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %conversation_id))]
    pub async fn get_cached_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Option<ConversationDoc>, WorkerError> {
        let mut conn = self.conn.clone();
        let key = Self::conversation_key(conversation_id);
        let json: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| WorkerError::Db(format!("Redis get error: {}", e)))?;
        match json {
            Some(s) => {
                let doc: ConversationDoc = serde_json::from_str(&s)
                    .map_err(|e| WorkerError::Db(format!("Deserialize error: {}", e)))?;
                tracing::info!(conversation_id, "Loaded conversation from Redis cache");
                Ok(Some(doc))
            }
            None => Ok(None),
        }
    }
}
