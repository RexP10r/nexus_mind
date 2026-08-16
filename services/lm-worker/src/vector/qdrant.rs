use std::cmp::max;
use std::collections::HashMap;

use qdrant_client::qdrant::vectors_config::Config;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, GetPointsBuilder, NamedVectors, PointStruct,
    SearchPointsBuilder, SparseVectorConfig, SparseVectorParamsBuilder, VectorParamsBuilder,
    VectorParamsMap, VectorsConfig,
};
use qdrant_client::{Payload, Qdrant};
use serde_json::Value;

use crate::embeddings::tfidf::VocabState;
use crate::embeddings::EmbeddingProviders;
use crate::error::WorkerError;
use crate::model::{Document, EmbeddingVariant, SearchResult};

const VOCAB_META_ID: u64 = 0;

pub async fn get_collection_vocab(
    client: &Qdrant,
    collection: &str,
) -> Result<Option<VocabState>, WorkerError> {
    let result = client
        .get_points(
            GetPointsBuilder::new(collection, vec![VOCAB_META_ID.into()]).with_payload(true),
        )
        .await
        .map_err(|e| WorkerError::Qdrant(format!("Failed to get vocab meta: {}", e)))?;

    if let Some(point) = result.result.into_iter().next() {
        let payload_map = point
            .payload
            .into_iter()
            .map(|(k, v)| (k, v.into_json()))
            .collect::<HashMap<String, Value>>();

        if let Some(terms_val) = payload_map.get("terms") {
            let terms_map: HashMap<String, u64> = serde_json::from_value(terms_val.clone())
                .map_err(|e| {
                    WorkerError::Qdrant(format!("Failed to parse vocab meta terms: {}", e))
                })?;

            let total_docs = payload_map
                .get("total_docs")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            let mut term_to_index: HashMap<String, usize> = HashMap::new();
            let mut term_doc_count: Vec<u64> = Vec::new();

            for (i, (term, count)) in terms_map.into_iter().enumerate() {
                term_to_index.insert(term, i);
                term_doc_count.push(count);
            }

            return Ok(Some(VocabState {
                term_to_index,
                term_doc_count,
                total_docs,
            }));
        }
    }

    Ok(None)
}

pub struct QdrantVectorStore {
    client: Qdrant,
    collection: String,
    embeddings: EmbeddingProviders,
}

impl QdrantVectorStore {
    pub async fn new(
        client: Qdrant,
        collection: String,
        embeddings: EmbeddingProviders,
    ) -> Result<Self, WorkerError> {
        let store = Self {
            client,
            collection,
            embeddings,
        };
        store.ensure_collection().await?;
        Ok(store)
    }

    async fn ensure_collection(&self) -> Result<(), WorkerError> {
        let exists = self.client.collection_info(&self.collection).await.is_ok();

        if exists {
            return Ok(());
        }

        let mut vectors_config = VectorsConfigBuilder::new();
        vectors_config
            .add_named_vector_params("bert", VectorParamsBuilder::new(384, Distance::Cosine));

        let mut sparse_config = SparseVectorsConfigBuilder::new();
        sparse_config.add_named_vector_params("tfidf", SparseVectorParamsBuilder::default());

        self.client
            .create_collection(
                CreateCollectionBuilder::new(&self.collection)
                    .vectors_config(vectors_config)
                    .sparse_vectors_config(sparse_config),
            )
            .await
            .map_err(|e| WorkerError::Qdrant(format!("Failed to create collection: {}", e)))?;

        tracing::info!(
            collection = %self.collection,
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
                qdrant_client::qdrant::UpsertPointsBuilder::new(&self.collection, points)
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
                SearchPointsBuilder::new(&self.collection, dense_fallback, limit)
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
                SearchPointsBuilder::new(&self.collection, vec, limit)
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

    pub async fn update_vocab_meta(&self, vocab: &VocabState) -> Result<(), WorkerError> {
        let terms_map: HashMap<String, u64> = vocab
            .term_to_index
            .iter()
            .map(|(term, &idx)| (term.clone(), vocab.term_doc_count[idx]))
            .collect();

        let payload: Payload = serde_json::json!({
            "terms": terms_map,
            "total_docs": vocab.total_docs,
        })
        .try_into()
        .map_err(|e| WorkerError::Qdrant(format!("Failed to build vocab meta payload: {}", e)))?;

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
                qdrant_client::qdrant::UpsertPointsBuilder::new(&self.collection, vec![point])
                    .wait(true),
            )
            .await
            .map_err(|e| WorkerError::Qdrant(format!("Failed to update vocab meta: {}", e)))?;

        Ok(())
    }

    async fn list_all_document_ids(&self) -> Result<Vec<String>, WorkerError> {
        let results = self
            .client
            .search_points(
                SearchPointsBuilder::new(&self.collection, vec![0.0; 384], 10_000)
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
                    &self.collection,
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
                        &self.collection,
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
            match self.embeddings.embed_tfidf(text) {
                Ok(EmbeddingVariant::Sparse(indices, _)) => {
                    let vocab = vocab_guard.read().unwrap();

                    for &idx in indices.iter() {
                        if idx < vocab.term_to_index.len() as u32 {
                            if let Some(term) = vocab.term_to_index.get(&idx.to_string()) {
                                new_terms.entry(term.clone().to_string()).or_insert(0);
                            }
                        }
                    }
                }
                _ => {}
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
                qdrant_client::qdrant::UpsertPointsBuilder::new(&self.collection, vec![point])
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
            return Err(WorkerError::Qdrant("Wrong tfidf embedding in a point".to_string()));
        }
    };

    let bert_dense = match bert_emb {
        EmbeddingVariant::Dense(vec) => qdrant_client::qdrant::Vector::new_dense(vec.clone()),
        _ => {
            return Err(WorkerError::Qdrant("Wrong bert embedding in a point".to_string()));
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
