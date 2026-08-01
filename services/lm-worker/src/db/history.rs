use mongodb::bson::{doc, Bson};
use mongodb::Collection;

use crate::error::WorkerError;
use crate::model::{ConversationDoc, ConversationEntry};

pub struct HistoryStore {
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

impl HistoryStore {
    pub fn new(collection: Collection<ConversationDoc>) -> Self {
        Self { collection }
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
            "$inc": { "total_tokens": tokens_added as i64 },
            "$set": { "updated_at": now },
            "$setOnInsert": { "conversation_id": conversation_id, "created_at": now },
        };

        self.collection
            .update_one(filter, update)
            .upsert(true)
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo append error: {}", e)))?;

        tracing::info!(
            conversation_id,
            entries_appended = entries.len(),
            tokens_added,
            "Atomically appended to conversation timeline"
        );
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

        tracing::info!(
            conversation_id,
            found = result.is_some(),
            "Loaded conversation from Mongo"
        );
        Ok(result)
    }
}
