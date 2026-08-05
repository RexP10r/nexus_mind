use mongodb::Client as MongoClient;
use mongodb::Collection;
use redis::aio::MultiplexedConnection;

use super::conversation_doc::create_ttl_index;
use crate::error::WorkerError;
use crate::model::ConversationDoc;

pub async fn connect_redis(url: &str) -> Result<MultiplexedConnection, WorkerError> {
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

pub async fn connect_mongo(
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
            db_name,
            error = %e,
            "Cannot connect to MongoDB. Start it with:\n  docker run -d --name mongo -p 27017:27017 mongo:7"
        );
        e
    })?;

    Ok(collection)
}
