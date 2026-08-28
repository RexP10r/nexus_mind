use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Document {
    pub text: String,
    pub name: String,
    pub file_format: String,
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
