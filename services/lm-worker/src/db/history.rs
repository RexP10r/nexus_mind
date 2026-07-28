use chrono::Utc;
use futures_util::TryStreamExt;
use mongodb::bson::doc;
use mongodb::options::FindOptions;
use mongodb::Collection;

use super::message_doc::MessageDoc;
use crate::error::WorkerError;
use crate::model::Message;

pub struct HistoryStore {
    collection: Collection<MessageDoc>,
}

impl HistoryStore {
    pub fn new(collection: Collection<MessageDoc>) -> Self {
        Self { collection }
    }

    fn to_message(doc: &MessageDoc) -> Message {
        Message {
            role: doc.role.clone(),
            content: doc.content.clone(),
        }
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %conversation_id))]
    pub async fn append_messages(
        &self,
        conversation_id: &str,
        messages: &[Message],
    ) -> Result<(), WorkerError> {
        let now = Utc::now();
        let docs: Vec<MessageDoc> = messages
            .iter()
            .map(|m| MessageDoc {
                conversation_id: conversation_id.to_string(),
                role: m.role.clone(),
                content: m.content.clone(),
                timestamp: now,
            })
            .collect();

        if docs.is_empty() {
            return Ok(());
        }

        self.collection
            .insert_many(docs)
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo insert error: {}", e)))?;

        tracing::info!(count = messages.len(), "Saved messages to history");
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %conversation_id))]
    pub async fn get_last_n(
        &self,
        conversation_id: &str,
        n: u32,
    ) -> Result<Vec<Message>, WorkerError> {
        let filter = doc! { "conversation_id": conversation_id };
        let opts = FindOptions::builder()
            .sort(doc! { "timestamp": 1 })
            .skip(0)
            .limit(n as i64)
            .build();

        let mut cursor = self
            .collection
            .find(filter)
            .with_options(opts)
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo find error: {}", e)))?;

        let mut messages = Vec::new();
        while let Some(doc) = cursor
            .try_next()
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo cursor error: {}", e)))?
        {
            messages.push(Self::to_message(&doc));
        }

        tracing::info!(count = messages.len(), "Loaded messages from history");
        Ok(messages)
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %conversation_id))]
    pub async fn count_messages(
        &self,
        conversation_id: &str,
    ) -> Result<u64, WorkerError> {
        let filter = doc! { "conversation_id": conversation_id };
        let count = self
            .collection
            .count_documents(filter)
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo count error: {}", e)))?;
        Ok(count)
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %conversation_id))]
    pub async fn get_older_messages(
        &self,
        conversation_id: &str,
        keep_last_n: u32,
    ) -> Result<Vec<Message>, WorkerError> {
        let total = self.count_messages(conversation_id).await?;
        if total <= keep_last_n as u64 {
            return Ok(Vec::new());
        }

        let older_count = total - keep_last_n as u64;
        let filter = doc! { "conversation_id": conversation_id };
        let opts = FindOptions::builder()
            .sort(doc! { "timestamp": 1 })
            .limit(older_count as i64)
            .build();

        let mut cursor = self
            .collection
            .find(filter)
            .with_options(opts)
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo find error: {}", e)))?;

        let mut messages = Vec::new();
        while let Some(doc) = cursor
            .try_next()
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo cursor error: {}", e)))?
        {
            messages.push(Self::to_message(&doc));
        }

        tracing::info!(
            total,
            keep_last = keep_last_n,
            older_count = messages.len(),
            "Loaded older messages for summarization"
        );
        Ok(messages)
    }
}
