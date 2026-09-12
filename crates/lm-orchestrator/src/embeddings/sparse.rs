use std::collections::{HashMap, HashSet};
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
    #[serde(default)]
    pub pruned_counts: HashMap<String, u64>,
}

impl Default for VocabState {
    fn default() -> Self {
        Self {
            term_to_index: HashMap::new(),
            term_doc_count: Vec::new(),
            total_docs: 0,
            pruned_counts: HashMap::new(),
        }
    }
}

pub struct TfIdfProvider {
    vocab: Arc<RwLock<VocabState>>,
    max_vocab_size: u32,
}

impl TfIdfProvider {
    pub fn new(vocab: VocabState, max_vocab_size: u32) -> Self {
        Self {
            vocab: Arc::new(RwLock::new(vocab)),
            max_vocab_size,
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
    pub fn update_vocab(&self, doc_texts: &[String]) {
        let vocab_arc = self.vocab();
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
                    let initial_count = local_vocab.pruned_counts.remove(term).unwrap_or(0);
                    local_vocab.term_to_index.insert(term.clone(), idx);
                    if idx >= local_vocab.term_doc_count.len() {
                        local_vocab.term_doc_count.resize(idx.saturating_add(1), 0);
                    }
                    local_vocab.term_doc_count[idx] = initial_count;
                    idx
                };

                if idx >= local_vocab.term_doc_count.len() {
                    local_vocab.term_doc_count.resize(idx.saturating_add(1), 0);
                }

                local_vocab.term_doc_count[idx] = local_vocab.term_doc_count[idx].saturating_add(1);
            }
        }

        local_vocab.total_docs = local_vocab.total_docs.saturating_add(processed_docs);
        self.prune_vocab();
    }
    fn prune_vocab(&self) {
        let mut vocab = self.vocab.write().unwrap();
        let target_size = self.max_vocab_size as usize;
        if vocab.term_to_index.len() <= target_size {
            return;
        }

        let mut indexed_counts: Vec<(usize, u64)> =
            vocab.term_doc_count.iter().copied().enumerate().collect();

        indexed_counts.sort_unstable_by(|a, b| b.1.cmp(&a.1));

        let keep_count = target_size.min(indexed_counts.len());
        let kept_old_indices: std::collections::HashSet<usize> = indexed_counts[..keep_count]
            .iter()
            .map(|(i, _)| *i)
            .collect();
        let evicted_old_indices: std::collections::HashSet<usize> = indexed_counts[keep_count..]
            .iter()
            .map(|(i, _)| *i)
            .collect();

        let mut new_term_to_index = std::collections::HashMap::with_capacity(keep_count);
        let mut new_term_doc_count = Vec::with_capacity(keep_count);
        let mut evicted_terms: Vec<(String, u64)> = Vec::new();

        for (term, &old_index) in &vocab.term_to_index {
            if kept_old_indices.contains(&old_index) {
                let new_index = new_term_to_index.len();
                new_term_to_index.insert(term.clone(), new_index);
                new_term_doc_count.push(vocab.term_doc_count[old_index]);
            } else if evicted_old_indices.contains(&old_index) {
                let count = vocab.term_doc_count[old_index];
                if count > 0 {
                    evicted_terms.push((term.clone(), count));
                }
            }
        }

        for (term, count) in evicted_terms {
            let entry = vocab.pruned_counts.entry(term).or_insert(0);
            *entry = (*entry).max(count);
        }

        let shadow_cap = target_size.saturating_mul(10);
        if vocab.pruned_counts.len() > shadow_cap {
            let mut counts_vec: Vec<(String, u64)> = vocab.pruned_counts.drain().collect();
            counts_vec.sort_unstable_by_key(|(_, count)| std::cmp::Reverse(*count));
            counts_vec.truncate(shadow_cap);
            vocab.pruned_counts = counts_vec.into_iter().collect();
        }

        vocab.term_to_index = new_term_to_index;
        vocab.term_doc_count = new_term_doc_count;
    }
}

pub fn tokenize(text: &str) -> Vec<String> {
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

        let provider = TfIdfProvider::new(state, 10);
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

    #[test]
    fn test_pruned_term_accumulates_across_batches() {
        let state = VocabState::default();
        let provider = TfIdfProvider::new(state, 3);

        let batch_a = vec!["alpha beta gamma".to_string()];
        let batch_b = vec!["alpha beta gamma".to_string()];
        let batch_c = vec!["alpha beta gamma".to_string()];
        let batch_new = vec!["delta".to_string()];

        provider.update_vocab(&batch_a);
        provider.update_vocab(&batch_b);
        provider.update_vocab(&batch_c);

        {
            let vocab = provider.vocab.read().unwrap();
            assert!(vocab.term_to_index.contains_key("alpha"));
            assert!(vocab.term_to_index.contains_key("beta"));
            assert!(vocab.term_to_index.contains_key("gamma"));
        }

        provider.update_vocab(&batch_new);

        {
            let vocab = provider.vocab.read().unwrap();
            assert!(vocab.pruned_counts.contains_key("delta"));
            assert_eq!(vocab.pruned_counts["delta"], 1);
        }

        for _ in 0..5 {
            provider.update_vocab(&batch_new);
        }

        {
            let vocab = provider.vocab.read().unwrap();
            let shadow_delta = vocab.pruned_counts.get("delta").copied().unwrap_or(0);
            let active_delta = vocab
                .term_to_index
                .get("delta")
                .and_then(|&idx| vocab.term_doc_count.get(idx).copied())
                .unwrap_or(0);
            let total = shadow_delta + active_delta;
            assert!(
                total >= 5,
                "delta should have accumulated df >= 5 across batches, got {total}"
            );
        }
    }

    #[test]
    fn test_pruned_counts_restored_on_reentry() {
        let state = VocabState::default();
        let provider = TfIdfProvider::new(state, 2);

        provider.update_vocab(&vec!["alpha beta".to_string()]);
        provider.update_vocab(&vec!["gamma".to_string()]);

        {
            let vocab = provider.vocab.read().unwrap();
            assert!(vocab.pruned_counts.contains_key("gamma"));
        }

        provider.update_vocab(&vec!["gamma".to_string()]);

        {
            let vocab = provider.vocab.read().unwrap();
            if let Some(&idx) = vocab.term_to_index.get("gamma") {
                assert!(vocab.term_doc_count[idx] >= 2);
            }
        }
    }
}
