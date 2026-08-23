use crate::embeddings::dense::EmbedderLMProvider;
use crate::embeddings::sparse::TfIdfProvider;
use crate::error::WorkerError;
use crate::model::EmbeddingVariant;

pub struct EmbeddingProviders {
    pub tfidf: TfIdfProvider,
    pub bert: EmbedderLMProvider,
}

impl EmbeddingProviders {
    pub fn new(tfidf: TfIdfProvider, bert: EmbedderLMProvider) -> Self {
        Self { tfidf, bert }
    }

    pub fn embed_tfidf(&self, text: &str) -> Result<EmbeddingVariant, WorkerError> {
        self.tfidf.embed(text)
    }

    pub fn embed_bert(&self, text: &str) -> Result<EmbeddingVariant, WorkerError> {
        self.bert.embed(text)
    }
}
