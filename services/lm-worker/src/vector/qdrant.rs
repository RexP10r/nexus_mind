use std::cmp::max;
use std::collections::{HashMap, HashSet};

use qdrant_client::qdrant::vectors_config::Config;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, NamedVectors, PointStruct, SearchPointsBuilder,
    SparseVectorConfig, SparseVectorParamsBuilder, VectorParamsBuilder, VectorParamsMap,
    VectorsConfig,
};
use qdrant_client::{Payload, Qdrant};
use serde::{Deserialize, Serialize};

use crate::embeddings::provider::EmbeddingProviders;
use crate::embeddings::sparse::VocabState;
use crate::error::WorkerError;
use crate::model::{Document, EmbeddingVariant, SearchResult};

pub async fn get_collection_vocab(
    client: &Qdrant,
    collection_name: &str,
) -> Result<Option<VocabState>, WorkerError> {
    let info = client
        .collection_info(collection_name)
        .await
        .map_err(|e| WorkerError::Qdrant(format!("Failed to get collection info: {}", e)))?;
    if let Some(res) = info.result
        && let Some(collection_config) = res.config
        && !collection_config.metadata.is_empty()
    {
        let jspn_map: serde_json::Map<String, serde_json::Value> = collection_config
            .metadata
            .into_iter()
            .map(|(k, v)| (k, v.into_json()))
            .collect();
        let json_metadata = serde_json::Value::Object(jspn_map);
        let meta: QdrantMeta = serde_json::from_value(json_metadata).map_err(|e| {
            WorkerError::Qdrant(format!(
                "Failed to parse Qdrant metadata from given gRPC input: {}",
                e
            ))
        })?;
        return Ok(Some(meta.tfidf_vocab));
    }

    Ok(None)
}

pub async fn ensure_collection(client: &Qdrant, collection_name: &str) -> Result<(), WorkerError> {
    let exists = 
        client
        .collection_exists(collection_name)
        .await
        .map_err(|e| WorkerError::Qdrant(format!("Failed to check if colletion exists: {}", e)))?;

    if exists {
        tracing::info!("Existing collection found");
        return Ok(());
    }
    tracing::info!("Existing collection not found, creating a new one...");

    let mut vectors_config = VectorsConfigBuilder::new();
    vectors_config.add_named_vector_params("lm", VectorParamsBuilder::new(384, Distance::Cosine));

    let mut sparse_config = SparseVectorsConfigBuilder::new();
    sparse_config.add_named_vector_params("tfidf", SparseVectorParamsBuilder::default());
    let metadata_value: HashMap<String, serde_json::Value> =
        serde_json::from_value(serde_json::to_value(&QdrantMeta::default()).unwrap()).unwrap();

    client
        .create_collection(
            CreateCollectionBuilder::new(collection_name)
                .vectors_config(vectors_config)
                .sparse_vectors_config(sparse_config)
                .metadata(metadata_value),
        )
        .await
        .map_err(|e| WorkerError::Qdrant(format!("Failed to create collection: {}", e)))?;

    tracing::info!(
        collection = %collection_name,
        "Created Qdrant collection with named vectors (lm + tfidf)"
    );

    Ok(())
}

pub struct QdrantVectorStore {
    client: Qdrant,
    collection_name: String,
    embeddings: EmbeddingProviders,
}

const TFIDF_VOCAB_METADATA_KEY: &str = "tfidf_vocab";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct QdrantMeta {
    tfidf_vocab: VocabState,
}
impl Default for QdrantMeta {
    fn default() -> Self {
        Self {
            tfidf_vocab: VocabState::default(),
        }
    }
}

impl QdrantVectorStore {
    pub async fn new(
        client: Qdrant,
        collection_name: String,
        embeddings: EmbeddingProviders,
    ) -> Result<Self, WorkerError> {
        let store = Self {
            client,
            collection_name,
            embeddings,
        };
        Ok(store)
    }

    pub async fn add_docs(&self, docs: &[Document]) -> Result<u64, WorkerError> {
        if docs.is_empty() {
            return Ok(0);
        }

        let mut points = Vec::with_capacity(docs.len());

        for doc in docs {
            let tfidf_emb = EmbeddingVariant::Sparse(vec![], vec![]);
            let lm_emb = self.embeddings.embed_lm(&doc.text)?;
            let point = build_point(doc, &tfidf_emb, &lm_emb)?;

            points.push(point);
        }

        self.client
            .upsert_points(
                qdrant_client::qdrant::UpsertPointsBuilder::new(&self.collection_name, points)
                    .wait(true),
            )
            .await
            .map_err(|e| WorkerError::Qdrant(format!("Failed to upsert points: {}", e)))?;

        tracing::info!(count = docs.len(), "Documents added to Qdrant");

        Ok(docs.len() as u64)
    }

    pub async fn search_tfidf(
        &self,
        query: &str,
        limit: u64,
    ) -> Result<Vec<SearchResult>, WorkerError> {
        let embedding = self.embeddings.embed_tfidf(query)?;
        let (_indices, _values) = match &embedding {
            EmbeddingVariant::Sparse(i, v) => (i, v),
            _ => return Ok(vec![]),
        };

        let dense_fallback = vec![0.0_f32; 384];
        let results = self
            .client
            .search_points(
                SearchPointsBuilder::new(&self.collection_name, dense_fallback, limit)
                    .vector_name("tfidf".to_string())
                    .with_payload(true),
            )
            .await
            .map_err(|e| WorkerError::Qdrant(format!("TF-IDF search failed: {}", e)))?;

        Ok(results
            .result
            .into_iter()
            .map(|p| SearchResult {
                id: format!("{:?}", p.id),
                text: extract_text(&p.payload),
                score: p.score,
            })
            .collect())
    }

    pub async fn search_lm(
        &self,
        query: &str,
        limit: u64,
    ) -> Result<Vec<SearchResult>, WorkerError> {
        let embedding = self.embeddings.embed_lm(query)?;
        let vec = match &embedding {
            EmbeddingVariant::Dense(v) => v.clone(),
            _ => return Ok(vec![]),
        };

        let results = self
            .client
            .search_points(
                SearchPointsBuilder::new(&self.collection_name, vec, limit)
                    .vector_name("lm".to_string())
                    .with_payload(true),
            )
            .await
            .map_err(|e| WorkerError::Qdrant(format!("LM search failed: {}", e)))?;

        Ok(results
            .result
            .into_iter()
            .map(|p| SearchResult {
                id: format!("{:?}", p.id),
                text: extract_text(&p.payload),
                score: p.score,
            })
            .collect())
    }

    pub async fn recompute_all_vectors(&self) -> Result<u64, WorkerError> {
        const SCROLL_BATCH_SIZE: u32 = 100;
        let mut all_points: Vec<qdrant_client::qdrant::RetrievedPoint> = Vec::new();
        let mut offset: Option<qdrant_client::qdrant::PointId> = None;

        loop {
            let mut scroll_builder =
                qdrant_client::qdrant::ScrollPointsBuilder::new(&self.collection_name)
                    .limit(SCROLL_BATCH_SIZE)
                    .with_payload(true)
                    .with_vectors(true);

            if let Some(off) = offset {
                scroll_builder = scroll_builder.offset(off);
            }

            let response = self
                .client
                .scroll(scroll_builder)
                .await
                .map_err(|e| WorkerError::Qdrant(format!("Failed to scroll points: {}", e)))?;

            let batch = response.result;
            if batch.is_empty() {
                break;
            }

            all_points.extend(batch);
            offset = response.next_page_offset;

            if offset.is_none() {
                break;
            }
        }

        if all_points.is_empty() {
            return Ok(0);
        }

        tracing::info!(
            count = all_points.len(),
            "Starting TF-IDF vector recomputation"
        );

        let mut updated_points = Vec::with_capacity(all_points.len());

        for retrieved in all_points {
            let Some(id) = retrieved.id else {
                continue;
            };

            let text = extract_text(&retrieved.payload);
            if text.is_empty() {
                continue;
            }

            let tfidf_emb = self.embeddings.embed_tfidf(&text)?;

            let lm_vec = retrieved
                .vectors
                .as_ref()
                .and_then(|v| v.get_vector_by_name("lm"))
                .and_then(|vec| match vec {
                    qdrant_client::qdrant::vector_output::Vector::Dense(dense) => Some(dense.data),
                    _ => None,
                });

            let Some(lm_data) = lm_vec else {
                tracing::warn!(point_id = ?id, "Missing lm vector, skipping");
                continue;
            };

            let point_id: u64 = match &id.point_id_options {
                Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(n)) => *n,
                _ => continue,
            };

            let payload: Payload = serde_json::json!({"text": text})
                .try_into()
                .map_err(|e| WorkerError::Qdrant(format!("Failed to build payload: {}", e)))?;

            let tfidf_sparse = match &tfidf_emb {
                EmbeddingVariant::Sparse(indices, values) => {
                    qdrant_client::qdrant::Vector::new_sparse(indices.clone(), values.clone())
                }
                _ => {
                    return Err(WorkerError::Qdrant(
                        "Wrong tfidf embedding variant".to_string(),
                    ));
                }
            };

            let lm_dense = qdrant_client::qdrant::Vector::new_dense(lm_data);

            let named_vectors = NamedVectors::default()
                .add_vector("tfidf", tfidf_sparse)
                .add_vector("lm", lm_dense);

            updated_points.push(PointStruct::new(point_id, named_vectors, payload));
        }

        if updated_points.is_empty() {
            return Ok(0);
        }

        self.client
            .upsert_points(
                qdrant_client::qdrant::UpsertPointsBuilder::new(
                    &self.collection_name,
                    updated_points.clone(),
                )
                .wait(true),
            )
            .await
            .map_err(|e| WorkerError::Qdrant(format!("Failed to upsert points: {}", e)))?;

        Ok(updated_points.len() as u64)
    }

    pub async fn update_vocab_with_new_docs(
        &self,
        doc_texts: &[String],
    ) -> Result<(), WorkerError> {
        if doc_texts.is_empty() {
            return Ok(());
        }

        let mut metadata = self.get_collection_metadata_map().await?;

        if let Some(persisted_vocab) = Self::vocab_from_metadata(&metadata)? {
            *self.embeddings.tfidf.vocab().write().unwrap() = persisted_vocab;
        }

        let updated_vocab = {
            let vocab_arc = self.embeddings.tfidf.vocab();
            let mut local_vocab = vocab_arc.write().unwrap();

            let mut next_index = local_vocab
                .term_to_index
                .values()
                .copied()
                .max()
                .map(|idx| idx.saturating_add(1))
                .unwrap_or(0);

            let mut processed_docs: u64 = 0;

            for text in doc_texts {
                let terms = crate::embeddings::sparse::tokenize(text);
                if terms.is_empty() {
                    continue;
                }

                processed_docs = processed_docs.saturating_add(1);

                let unique_terms: HashSet<&String> = terms.iter().collect();

                for term in unique_terms {
                    let idx = if let Some(&idx) = local_vocab.term_to_index.get(term) {
                        idx
                    } else {
                        let idx = next_index;
                        next_index = next_index.saturating_add(1);
                        local_vocab.term_to_index.insert(term.clone(), idx);
                        idx
                    };

                    if idx >= local_vocab.term_doc_count.len() {
                        local_vocab.term_doc_count.resize(idx.saturating_add(1), 0);
                    }

                    local_vocab.term_doc_count[idx] =
                        local_vocab.term_doc_count[idx].saturating_add(1);
                }
            }

            local_vocab.total_docs = local_vocab.total_docs.saturating_add(processed_docs);

            local_vocab.clone()
        };

        // Serialize updated vocab through QdrantMeta so the shape stays the same
        // as during collection creation.
        let meta = QdrantMeta {
            tfidf_vocab: updated_vocab,
        };

        let meta_value = serde_json::to_value(&meta)
            .map_err(|e| WorkerError::Qdrant(format!("Failed to serialize QdrantMeta: {}", e)))?;

        let serde_json::Value::Object(meta_map) = meta_value else {
            return Err(WorkerError::Qdrant(
                "Serialized QdrantMeta was not a JSON object".to_string(),
            ));
        };

        for (key, value) in meta_map {
            metadata.insert(key, value);
        }

        self.update_collection_metadata(metadata).await?;

        Ok(())
    }

    async fn get_collection_metadata_map(
        &self,
    ) -> Result<serde_json::Map<String, serde_json::Value>, WorkerError> {
        let info = self
            .client
            .collection_info(&self.collection_name)
            .await
            .map_err(|e| WorkerError::Qdrant(format!("Failed to get collection info: {}", e)))?;

        if let Some(res) = info.result {
            if let Some(config) = res.config {
                if !config.metadata.is_empty() {
                    return Ok(config
                        .metadata
                        .into_iter()
                        .map(|(k, v)| (k, v.into_json()))
                        .collect());
                }
            }
        }

        Ok(serde_json::Map::new())
    }

    fn vocab_from_metadata(
        metadata: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Option<VocabState>, WorkerError> {
        let Some(vocab_value) = metadata.get(TFIDF_VOCAB_METADATA_KEY) else {
            return Ok(None);
        };

        let vocab: VocabState = serde_json::from_value(vocab_value.clone()).map_err(|e| {
            WorkerError::Qdrant(format!(
                "Failed to parse tfidf_vocab from collection metadata: {}",
                e
            ))
        })?;

        Ok(Some(vocab))
    }

    async fn update_collection_metadata(
        &self,
        metadata: serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), WorkerError> {
        let metadata_map: HashMap<String, serde_json::Value> = metadata.into_iter().collect();

        self.client
            .update_collection(
                qdrant_client::qdrant::UpdateCollectionBuilder::new(&self.collection_name)
                    .metadata(metadata_map),
            )
            .await
            .map_err(|e| {
                WorkerError::Qdrant(format!("Failed to update collection metadata: {}", e))
            })?;

        Ok(())
    }
}

fn build_point(
    doc: &Document,
    tfidf_emb: &EmbeddingVariant,
    lm_emb: &EmbeddingVariant,
) -> Result<PointStruct, WorkerError> {
    let id: u64 = max(fast_hash(&doc.id), 1);
    let payload: Payload = serde_json::json!({"text": doc.text})
        .try_into()
        .map_err(|e| WorkerError::Qdrant(format!("Failed to build payload: {}", e)))?;

    let tfidf_sparse = match tfidf_emb {
        EmbeddingVariant::Sparse(indices, values) => {
            qdrant_client::qdrant::Vector::new_sparse(indices.clone(), values.clone())
        }
        _ => {
            return Err(WorkerError::Qdrant(
                "Wrong tfidf embedding in a point".to_string(),
            ));
        }
    };

    let lm_dense = match lm_emb {
        EmbeddingVariant::Dense(vec) => qdrant_client::qdrant::Vector::new_dense(vec.clone()),
        _ => {
            return Err(WorkerError::Qdrant(
                "Wrong lm embedding in a point".to_string(),
            ));
        }
    };

    let named_vectors = NamedVectors::default()
        .add_vector("tfidf", tfidf_sparse)
        .add_vector("lm", lm_dense);

    Ok(PointStruct::new(id, named_vectors, payload))
}

fn extract_text(payload: &HashMap<String, qdrant_client::qdrant::Value>) -> String {
    payload
        .get("text")
        .and_then(|v| v.kind.as_ref())
        .and_then(|k| match k {
            qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn fast_hash(s: &str) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

struct VectorsConfigBuilder {
    params: HashMap<String, qdrant_client::qdrant::VectorParams>,
}

impl VectorsConfigBuilder {
    fn new() -> Self {
        Self {
            params: HashMap::new(),
        }
    }

    fn add_named_vector_params(
        &mut self,
        name: impl Into<String>,
        params: impl Into<qdrant_client::qdrant::VectorParams>,
    ) -> &mut Self {
        self.params.insert(name.into(), params.into());
        self
    }
}

impl From<VectorsConfigBuilder> for VectorsConfig {
    fn from(builder: VectorsConfigBuilder) -> Self {
        if builder.params.is_empty() {
            return VectorsConfig::default();
        }
        VectorsConfig {
            config: Some(Config::from(VectorParamsMap::from(builder.params))),
        }
    }
}

struct SparseVectorsConfigBuilder {
    params: HashMap<String, qdrant_client::qdrant::SparseVectorParams>,
}

impl SparseVectorsConfigBuilder {
    fn new() -> Self {
        Self {
            params: HashMap::new(),
        }
    }

    fn add_named_vector_params(
        &mut self,
        name: impl Into<String>,
        params: impl Into<qdrant_client::qdrant::SparseVectorParams>,
    ) -> &mut Self {
        self.params.insert(name.into(), params.into());
        self
    }
}

impl From<SparseVectorsConfigBuilder> for SparseVectorConfig {
    fn from(builder: SparseVectorsConfigBuilder) -> Self {
        if builder.params.is_empty() {
            return SparseVectorConfig::default();
        }
        SparseVectorConfig {
            map: builder.params,
        }
    }
}
