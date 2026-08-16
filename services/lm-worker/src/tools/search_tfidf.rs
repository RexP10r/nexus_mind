use std::sync::Arc;

use serde::Deserialize;

use crate::traits::tool::Tool;
use crate::vector::qdrant::QdrantVectorStore;

#[derive(Deserialize)]
struct SearchInput {
    query: String,
    #[serde(default = "default_limit")]
    limit: u64,
}

fn default_limit() -> u64 {
    5
}

pub struct SearchTfIdfTool {
    store: Arc<QdrantVectorStore>,
}

impl SearchTfIdfTool {
    pub fn new(store: Arc<QdrantVectorStore>) -> Self {
        Self { store }
    }
}

impl Tool for SearchTfIdfTool {
    fn name(&self) -> &str {
        "search_tfidf"
    }

    fn description(&self) -> &str {
        "searches the knowledge base using TF-IDF (keyword-based). input: JSON {\"query\": \"...\", \"limit\": 5}"
    }

    fn execute(&self, input: &str) -> String {
        let parsed: SearchInput = match serde_json::from_str(input) {
            Ok(v) => v,
            Err(e) => {
                return format!(
                    "Invalid input for search_tfidf: {}. Expected JSON with 'query' field.",
                    e
                )
            }
        };

        if parsed.query.is_empty() {
            return "Empty query provided.".to_string();
        }

        let handle = tokio::runtime::Handle::current();

        match handle.block_on(self.store.search_tfidf(&parsed.query, parsed.limit)) {
            Ok(results) => {
                if results.is_empty() {
                    return "No relevant documents found.".to_string();
                }
                format_results(&results)
            }
            Err(e) => format!("TF-IDF search failed: {}", e),
        }
    }
}

fn format_results(results: &[crate::model::SearchResult]) -> String {
    results
        .iter()
        .enumerate()
        .map(|(i, r)| format!("[{i}] (score={:.4}) {}", r.score, r.text))
        .collect::<Vec<_>>()
        .join("\n")
}
