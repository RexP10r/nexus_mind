use mongodb::bson::doc;
use mongodb::Collection;

use crate::error::WorkerError;
use crate::model::ConversationDoc;

pub struct HistoryStore {
    collection: Collection<ConversationDoc>,
}

impl HistoryStore {
    pub fn new(collection: Collection<ConversationDoc>) -> Self {
        Self { collection }
    }

    #[tracing::instrument(skip(self), fields(conversation_id = %doc.conversation_id))]
    pub async fn upsert_conversation(&self, doc: &ConversationDoc) -> Result<(), WorkerError> {
        let filter = doc! { "conversation_id": &doc.conversation_id };

        self.collection
            .replace_one(filter, doc)
            .upsert(true)
            .await
            .map_err(|e| WorkerError::Db(format!("Mongo upsert error: {}", e)))?;

        tracing::info!(
            conversation_id = %doc.conversation_id,
            timeline_entries = doc.timeline.len(),
            "Upserted conversation"
        );
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
