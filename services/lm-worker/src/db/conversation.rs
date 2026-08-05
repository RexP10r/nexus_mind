use mongodb::bson::{doc, Bson, Document};
use mongodb::Collection;
use serde::{de::DeserializeOwned, Deserialize};

use crate::error::WorkerError;
use crate::model::{ConversationDoc, ConversationEntry};

pub struct ConversationStore {
    collection: Collection<ConversationDoc>,
}

fn bson_vec(entries: &[ConversationEntry]) -> Result<Vec<Bson>, WorkerError> {
    entries
        .iter()
        .map(|e| {
            mongodb::bson::to_bson(e)
                .map_err(|err| WorkerError::Db(format!("BSON serialize error: {}", err)))
        })
        .collect()
}

impl ConversationStore {
    pub fn new(collection: Collection<ConversationDoc>) -> Self {
        Self { collection }
    }

    async fn get_field<F>(
        &self,
        conversation_id: &str,
        projection: Document,
    ) -> Result<Option<F>, WorkerError>
    where
        F: DeserializeOwned + Send + Sync,
    {
        let filter = doc! { "conversation_id": conversation_id };
        self.collection
            .clone_with_type::<F>()
            .find_one(filter)
            .projection(projection)
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo find error: {}", e)))
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %conversation_id))]
    pub async fn append_timeline_entries(
        &self,
        conversation_id: &str,
        entries: &[ConversationEntry],
        tokens_added: u32,
    ) -> Result<(), WorkerError> {
        let bson_entries = bson_vec(entries)?;
        let now = mongodb::bson::DateTime::now();

        let filter = doc! { "conversation_id": conversation_id };
        let update = doc! {
            "$push": { "timeline": { "$each": bson_entries } },
            "$inc": { "total_tokens": tokens_added as u32 },
            "$inc": { "total_messages": 2 as u32 },
            "$set": { "updated_at": now },
            "$setOnInsert": { "conversation_id": conversation_id, "created_at": now },
        };

        self.collection
            .update_one(filter, update)
            .upsert(true)
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo append error: {}", e)))?;

        Ok(())
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %conversation_id))]
    pub async fn set_summary(
        &self,
        conversation_id: &str,
        summary: &str,
    ) -> Result<(), WorkerError> {
        let now = mongodb::bson::DateTime::now();
        let filter = doc! { "conversation_id": conversation_id };
        let update = doc! {
            "$set": { "summary": summary, "updated_at": now },
        };

        self.collection
            .update_one(filter, update)
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo set_summary error: {}", e)))?;

        tracing::info!(conversation_id, "Atomically set conversation summary");
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %conversation_id))]
    pub async fn get_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Option<ConversationDoc>, WorkerError> {
        let filter = doc! { "conversation_id": conversation_id };
        let result = self
            .collection
            .find_one(filter)
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo find error: {}", e)))?;

        Ok(result)
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %conversation_id))]
    pub async fn get_message_count(&self, conversation_id: &str) -> Result<u32, WorkerError> {
        #[derive(Deserialize)]
        struct CountProjection {
            total_messages: u32,
        }

        let result: Option<CountProjection> =
            self.get_field(conversation_id, doc! { "total_messages": 1 }).await?;

        Ok(result.map(|r| r.total_messages).unwrap_or(0))
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %conversation_id))]
    pub async fn get_summary_field(
        &self,
        conversation_id: &str,
    ) -> Result<Option<String>, WorkerError> {
        #[derive(Deserialize)]
        struct SummaryProjection {
            summary: Option<String>,
        }

        let result: Option<SummaryProjection> =
            self.get_field(conversation_id, doc! { "summary": 1 }).await?;

        Ok(result.and_then(|r| r.summary))
    }
}
