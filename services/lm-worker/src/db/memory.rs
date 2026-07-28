use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;

use crate::error::WorkerError;

const SUMMARY_KEY_PREFIX: &str = "summary:";

pub struct MemoryStore {
    conn: MultiplexedConnection,
    ttl_secs: u64,
}

impl MemoryStore {
    pub fn new(conn: MultiplexedConnection, ttl_secs: u64) -> Self {
        Self { conn, ttl_secs }
    }

    fn summary_key(conversation_id: &str) -> String {
        format!("{}{}", SUMMARY_KEY_PREFIX, conversation_id)
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %conversation_id))]
    pub async fn get_summary(&self, conversation_id: &str) -> Result<Option<String>, WorkerError> {
        let mut conn = self.conn.clone();
        let key = Self::summary_key(conversation_id);
        let result: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| WorkerError::Db(format!("Redis get error: {}", e)))?;
        tracing::info!(has_summary = result.is_some(), "Loaded summary from memory");
        Ok(result)
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %conversation_id))]
    pub async fn set_summary(
        &self,
        conversation_id: &str,
        summary: &str,
    ) -> Result<(), WorkerError> {
        let mut conn = self.conn.clone();
        let key = Self::summary_key(conversation_id);
        conn.set::<_, _, ()>(&key, summary)
            .await
            .map_err(|e| WorkerError::Db(format!("Redis set error: {}", e)))?;
        if self.ttl_secs > 0 {
            conn.expire::<_, ()>(&key, self.ttl_secs as i64)
                .await
                .map_err(|e| WorkerError::Db(format!("Redis expire error: {}", e)))?;
        }
        tracing::info!(ttl_secs = self.ttl_secs, "Saved summary to memory");
        Ok(())
    }
}
