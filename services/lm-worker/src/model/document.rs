use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Document {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub id: String,
    pub text: String,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub enum EmbeddingVariant {
    Dense(Vec<f32>),
    Sparse(Vec<u32>, Vec<f32>),
}
