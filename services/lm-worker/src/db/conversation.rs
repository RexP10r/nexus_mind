use mongodb::bson::{doc, Bson, Document};
use mongodb::Collection;
use futures_util::TryStreamExt;

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
    pub async fn get_message_count(
        &self,
        conversation_id: &str,
    ) -> Result<u64, WorkerError> {
        let pipeline = vec![
            doc! { "$match": { "conversation_id": conversation_id } },
            doc! { "$project": {
                "message_count": {
                    "$size": {
                        "$filter": {
                            "input": "$timeline",
                            "as": "e",
                            "cond": { "$eq": ["$$e.type", "message"] }
                        }
                    }
                }
            }},
        ];

        let mut cursor = self
            .collection
            .clone_with_type::<Document>()
            .aggregate(pipeline)
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo aggregate error: {}", e)))?;

        let count = cursor
            .try_next()
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo cursor error: {}", e)))?
            .and_then(|d| d.get_i64("message_count").ok())
            .unwrap_or(0) as u64;

        Ok(count)
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %conversation_id))]
    pub async fn get_summary_field(
        &self,
        conversation_id: &str,
    ) -> Result<Option<String>, WorkerError> {
        let filter = doc! { "conversation_id": conversation_id };
        let projection = doc! { "summary": 1 };

        let result: Option<Document> = self
            .collection
            .clone_with_type::<Document>()
            .find_one(filter)
            .projection(projection)
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo find error: {}", e)))?;

        let summary = result.and_then(|d| d.get_str("summary").ok().map(|s| s.to_string()));
        Ok(summary)
    }
}
