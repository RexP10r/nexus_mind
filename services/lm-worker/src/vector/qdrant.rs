use std::cmp::max;
use std::collections::HashMap;

use qdrant_client::qdrant::vectors_config::Config;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, NamedVectors, PointStruct, SearchPointsBuilder,
    SparseVectorConfig, SparseVectorParamsBuilder, VectorParamsBuilder, VectorParamsMap,
    VectorsConfig,
};
use qdrant_client::{Payload, Qdrant};
use serde::{Deserialize, Serialize};

use crate::embeddings::EmbeddingProviders;
use crate::embeddings::tfidf::VocabState;
use crate::error::WorkerError;
use crate::model::{Document, EmbeddingVariant, SearchResult};

const VOCAB_META_ID: u64 = 0;

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

pub struct QdrantVectorStore {
    client: Qdrant,
    collection_name: String,
    embeddings: EmbeddingProviders,
}
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
        store.ensure_collection().await?;
        Ok(store)
    }

    async fn ensure_collection(&self) -> Result<(), WorkerError> {
        let exists = self
            .client
            .collection_exists(self.collection_name.clone())
            .await
            .map_err(|e| {
                WorkerError::Qdrant(format!("Failed to check if colletion exists: {}", e))
            })?;

        if exists {
            tracing::info!("Existing collection found");
            return Ok(());
        }
        tracing::info!("Existing collection not found, creating a new one...");

        let mut vectors_config = VectorsConfigBuilder::new();
        vectors_config
            .add_named_vector_params("bert", VectorParamsBuilder::new(384, Distance::Cosine));

        let mut sparse_config = SparseVectorsConfigBuilder::new();
        sparse_config.add_named_vector_params("tfidf", SparseVectorParamsBuilder::default());
        let metadata_value: HashMap<String, serde_json::Value> =
            serde_json::from_value(serde_json::to_value(&QdrantMeta::default()).unwrap()).unwrap();

        self.client
            .create_collection(
                CreateCollectionBuilder::new(&self.collection_name)
                    .vectors_config(vectors_config)
                    .sparse_vectors_config(sparse_config)
                    .metadata(metadata_value),
            )
            .await
            .map_err(|e| WorkerError::Qdrant(format!("Failed to create collection: {}", e)))?;

        tracing::info!(
            collection = %self.collection_name,
            "Created Qdrant collection with named vectors (bert + tfidf)"
        );

        Ok(())
    }

    pub async fn add_docs(&self, docs: &[Document]) -> Result<u64, WorkerError> {
        if docs.is_empty() {
            return Ok(0);
        }

        let mut points = Vec::with_capacity(docs.len());

        for doc in docs {
            let tfidf_emb = EmbeddingVariant::Sparse(vec![], vec![]);
            let bert_emb = self.embeddings.embed_bert(&doc.text)?;
            let point = build_point(doc, &tfidf_emb, &bert_emb)?;

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

    pub async fn search_bert(
        &self,
        query: &str,
        limit: u64,
    ) -> Result<Vec<SearchResult>, WorkerError> {
        let embedding = self.embeddings.embed_bert(query)?;
        let vec = match &embedding {
            EmbeddingVariant::Dense(v) => v.clone(),
            _ => return Ok(vec![]),
        };

        let results = self
            .client
            .search_points(
                SearchPointsBuilder::new(&self.collection_name, vec, limit)
                    .vector_name("bert".to_string())
                    .with_payload(true),
            )
            .await
            .map_err(|e| WorkerError::Qdrant(format!("BERT search failed: {}", e)))?;

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

    async fn list_all_document_ids(&self) -> Result<Vec<String>, WorkerError> {
        let results = self
            .client
            .search_points(
                SearchPointsBuilder::new(&self.collection_name, vec![0.0; 384], 10_000)
                    .vector_name("bert".to_string())
                    .with_payload(true),
            )
            .await
            .map_err(|e| WorkerError::Qdrant(format!("Failed to get document points: {}", e)))?;

        Ok(results
            .result
            .into_iter()
            .map(|p| format!("{:?}", p.id))
            .collect())
    }

    pub async fn recompute_all_vectors(&self) -> Result<u64, WorkerError> {
        let doc_ids = self.list_all_document_ids().await?;

        if doc_ids.is_empty() {
            return Ok(0);
        }

        tracing::info!(count = doc_ids.len(), "Starting full vector recomputation");

        let mut success_count = 0u64;
        let mut failure_count = 0usize;
        const MAX_FAILURES: usize = 10;

        for id in &doc_ids {
            if failure_count >= MAX_FAILURES {
                tracing::error!(failures = failure_count, "Stopping recomputation");
                break;
            }

            match self.recompute_single_vector(id).await {
                Ok(_) => success_count += 1,
                Err(e) => {
                    tracing::error!(id = %id, error = %e, "Failed to recompute vector");
                    failure_count += 1;
                }
            }
        }

        if failure_count >= MAX_FAILURES {
            return Err(WorkerError::Qdrant(format!(
                "Stopped after {} failures",
                failure_count
            )));
        }

        Ok(success_count)
    }

    async fn recompute_single_vector(&self, doc_id: &str) -> Result<(), WorkerError> {
        let results = self
            .client
            .get_points(
                qdrant_client::qdrant::GetPointsBuilder::new(
                    &self.collection_name,
                    vec![VOCAB_META_ID.into()],
                )
                .with_payload(true),
            )
            .await
            .map_err(|e| WorkerError::Qdrant(format!("Failed to get points: {}", e)))?;

        if let Some(point) = results.result.into_iter().next() {
            let text = extract_text(&point.payload);

            let tfidf_emb = self.embeddings.embed_tfidf(&text)?;
            let bert_emb = self.embeddings.embed_bert(&text)?;

            let point_struct = build_point(
                &Document {
                    id: doc_id.to_string(),
                    text,
                },
                &tfidf_emb,
                &bert_emb,
            )?;

            self.client
                .upsert_points(
                    qdrant_client::qdrant::UpsertPointsBuilder::new(
                        &self.collection_name,
                        vec![point_struct],
                    )
                    .wait(true),
                )
                .await
                .map_err(|e| WorkerError::Qdrant(format!("Failed to upsert points: {}", e)))?;
        }

        Ok(())
    }

    pub async fn update_vocab_with_new_docs(
        &self,
        doc_texts: &[String],
    ) -> Result<(), WorkerError> {
        if doc_texts.is_empty() {
            return Ok(());
        }

        let mut new_terms: HashMap<String, u64> = HashMap::new();
        let vocab_guard = self.embeddings.tfidf.vocab();

        for text in doc_texts {
            if let Ok(EmbeddingVariant::Sparse(indices, _)) = self.embeddings.embed_tfidf(text) {
                let vocab = vocab_guard.read().unwrap();
                for &idx in indices.iter() {
                    if let Some((term, _)) = vocab
                        .term_to_index
                        .iter()
                        .find(|&(_, &val)| val == idx as usize)
                    {
                        new_terms.entry(term.clone().to_string()).or_insert(0);
                    }
                }
            }
        }

        for (term, count) in &new_terms {
            let mut vocab = vocab_guard.write().unwrap();

            if !vocab.term_to_index.contains_key(term) {
                let idx = vocab.term_to_index.len();
                vocab.term_to_index.insert(term.clone(), idx);
                vocab.term_doc_count.push(*count as u64 + 1u64);
            } else {
                let idx = *vocab.term_to_index.get(term).unwrap() as usize;
                vocab.term_doc_count[idx] += count;
            }
        }

        self.embeddings.tfidf.vocab().write().unwrap().total_docs += doc_texts.len() as u64;

        let terms_map: HashMap<String, u64> = self
            .embeddings
            .tfidf
            .vocab()
            .read()
            .unwrap()
            .term_to_index
            .iter()
            .map(|(term, &idx)| {
                (
                    term.clone(),
                    self.embeddings.tfidf.vocab().read().unwrap().term_doc_count[idx],
                )
            })
            .collect();

        let payload: Payload = serde_json::json!({
            "terms": terms_map,
            "total_docs": self.embeddings.tfidf.vocab().read().unwrap().total_docs,
        })
        .try_into()
        .map_err(|e| WorkerError::Qdrant(format!("Failed to build vocab meta: {}", e)))?;

        let point = PointStruct::new(
            VOCAB_META_ID,
            NamedVectors::default()
                .add_vector(
                    "tfidf",
                    qdrant_client::qdrant::Vector::new_sparse(vec![], vec![]),
                )
                .add_vector(
                    "bert",
                    qdrant_client::qdrant::Vector::new_dense(vec![0.0_f32; 384]),
                ),
            payload,
        );

        self.client
            .upsert_points(
                qdrant_client::qdrant::UpsertPointsBuilder::new(&self.collection_name, vec![point])
                    .wait(true),
            )
            .await
            .map_err(|e| WorkerError::Qdrant(format!("Failed to upsert points: {}", e)))?;

        Ok(())
    }
}

fn build_point(
    doc: &Document,
    tfidf_emb: &EmbeddingVariant,
    bert_emb: &EmbeddingVariant,
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

    let bert_dense = match bert_emb {
        EmbeddingVariant::Dense(vec) => qdrant_client::qdrant::Vector::new_dense(vec.clone()),
        _ => {
            return Err(WorkerError::Qdrant(
                "Wrong bert embedding in a point".to_string(),
            ));
        }
    };

    let named_vectors = NamedVectors::default()
        .add_vector("tfidf", tfidf_sparse)
        .add_vector("bert", bert_dense);

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
