use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use rust_stemmers::{Algorithm, Stemmer};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::error::WorkerError;
use crate::model::EmbeddingVariant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabState {
    pub term_to_index: HashMap<String, usize>,
    pub term_doc_count: Vec<u64>,
    pub total_docs: u64,
}

impl Default for VocabState {
    fn default() -> Self {
        Self {
            term_to_index: HashMap::new(),
            term_doc_count: Vec::new(),
            total_docs: 0,
        }
    }
}

pub struct TfIdfProvider {
    vocab: Arc<RwLock<VocabState>>,
}

impl TfIdfProvider {
    pub fn new(vocab: VocabState) -> Self {
        Self {
            vocab: Arc::new(RwLock::new(vocab)),
        }
    }

    pub fn vocab(&self) -> Arc<RwLock<VocabState>> {
        Arc::clone(&self.vocab)
    }

    pub fn embed(&self, text: &str) -> Result<EmbeddingVariant, WorkerError> {
        let terms = tokenize(text);
        if terms.is_empty() {
            return Ok(EmbeddingVariant::Sparse(vec![], vec![]));
        }

        let vocab = self.vocab.read().unwrap();
        let num_terms = terms.len() as f32;

        let mut term_tf: HashMap<usize, f32> = HashMap::new();
        for term in &terms {
            if let Some(&idx) = vocab.term_to_index.get(term) {
                *term_tf.entry(idx).or_insert(0.0) += 1.0;
            }
        }

        let mut entries: Vec<(usize, f32)> = Vec::new();
        for (idx, tf) in term_tf {
            let df = vocab.term_doc_count.get(idx).copied().unwrap_or(1);
            let idf = ((vocab.total_docs as f64 + 1.0) / (df as f64 + 1.0)).ln() as f32;
            entries.push((idx, (tf / num_terms) * idf));
        }

        entries.sort_by_key(|(idx, _)| *idx);
        let indices: Vec<u32> = entries.iter().map(|(i, _)| *i as u32).collect();
        let values: Vec<f32> = entries.iter().map(|(_, v)| *v).collect();

        Ok(EmbeddingVariant::Sparse(indices, values))
    }
}

fn tokenize(text: &str) -> Vec<String> {
    let stemmer = Stemmer::create(Algorithm::English);
    text.unicode_words()
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() > 1)
        .map(|w| stemmer.stem(&w).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_basic() {
        let tokens = tokenize("The quick brown foxes jumped over the lazy dogs");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_embed_with_vocab() {
        let mut state = VocabState::default();
        state.term_to_index.insert("hello".to_string(), 0);
        state.term_to_index.insert("world".to_string(), 1);
        state.term_doc_count = vec![5, 3];
        state.total_docs = 10;

        let provider = TfIdfProvider::new(state);
        let result = provider.embed("hello world").unwrap();
        match result {
            EmbeddingVariant::Sparse(indices, values) => {
                assert_eq!(indices.len(), 2);
                assert_eq!(indices[0], 0);
                assert_eq!(indices[1], 1);
                assert!(values[0] > 0.0);
                assert!(values[1] > 0.0);
            }
            _ => panic!("expected sparse"),
        }
    }
}
