use crate::embeddings::dense::EmbedderLMProvider;
use crate::embeddings::sparse::TfIdfProvider;
use crate::error::WorkerError;
use crate::model::EmbeddingVariant;

pub struct EmbeddingProviders {
    pub tfidf: TfIdfProvider,
    pub lm: EmbedderLMProvider,
}

impl EmbeddingProviders {
    pub fn new(tfidf: TfIdfProvider, lm: EmbedderLMProvider) -> Self {
        Self { tfidf, lm }
    }

    pub fn embed_tfidf(&self, text: &str) -> Result<EmbeddingVariant, WorkerError> {
        self.tfidf.embed(text)
    }

    pub fn embed_lm(&self, text: &str) -> Result<EmbeddingVariant, WorkerError> {
        self.lm.embed(text)
    }
}
