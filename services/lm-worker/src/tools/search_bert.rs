use std::sync::Arc;

use async_trait::async_trait;
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

pub struct SearchBertTool {
    store: Arc<QdrantVectorStore>,
}

impl SearchBertTool {
    pub fn new(store: Arc<QdrantVectorStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for SearchBertTool {
    fn name(&self) -> &str {
        "search_bert"
    }

    fn description(&self) -> &str {
        "searches the knowledge base using BERT (semantic understanding). input: JSON {\"query\": \"...\", \"limit\": 5}"
    }

    async fn execute(&self, input: &str) -> String {
        let parsed: SearchInput = match serde_json::from_str(input) {
            Ok(v) => v,
            Err(e) => {
                return format!(
                    "Invalid input for search_bert: {}. Expected JSON with 'query' field.",
                    e
                )
            }
        };

        if parsed.query.is_empty() {
            return "Empty query provided.".to_string();
        }

        match self.store.search_bert(&parsed.query, parsed.limit).await {
            Ok(results) => {
                if results.is_empty() {
                    return "No relevant documents found.".to_string();
                }
                format_results(&results)
            }
            Err(e) => format!("BERT search failed: {}", e),
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
