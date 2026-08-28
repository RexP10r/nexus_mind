use mongodb::bson::doc;
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use std::time::Duration;

use crate::error::WorkerError;
use crate::model::ConversationDoc;

pub const MONGO_TTL_DAYS: u32 = 30;

pub async fn create_ttl_index(
    collection: &Collection<ConversationDoc>,
) -> Result<(), WorkerError> {
    let ttl_secs = MONGO_TTL_DAYS as u64 * 24 * 3600;
    let options = IndexOptions::builder()
        .expire_after(Duration::from_secs(ttl_secs))
        .build();

    collection
        .create_index(
            IndexModel::builder()
                .keys(doc! { "updated_at": 1 })
                .options(options)
                .build(),
        )
        .await
        .map_err(|e| WorkerError::Db(format!("Mongo TTL index error: {}", e)))?;

    collection
        .create_index(
            IndexModel::builder()
                .keys(doc! { "conversation_id": 1 })
                .build(),
        )
        .await
        .map_err(|e| WorkerError::Db(format!("Mongo conversation_id index error: {}", e)))?;

    Ok(())
}
