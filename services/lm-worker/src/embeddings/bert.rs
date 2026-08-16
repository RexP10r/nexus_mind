use ort::session::Session;
use ort::value::Tensor;
use parking_lot::Mutex;
use tokenizers::Tokenizer;

use crate::error::WorkerError;
use crate::model::EmbeddingVariant;

pub struct BertProvider {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
}

impl BertProvider {
    pub fn from_files(model_path: &str, tokenizer_path: &str) -> Result<Self, WorkerError> {
        let session = Session::builder()
            .map_err(|e| WorkerError::Embedding(format!("Failed to create ORT session builder: {}", e)))?
            .commit_from_file(model_path)
            .map_err(|e| {
                WorkerError::Embedding(format!(
                    "Failed to load ONNX model from '{}': {}",
                    model_path, e
                ))
            })?;

        let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| {
            WorkerError::Embedding(format!(
                "Failed to load tokenizer from '{}': {}",
                tokenizer_path, e
            ))
        })?;

        tracing::info!(
            model_path,
            tokenizer_path,
            "BERT embedding provider initialized"
        );

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
        })
    }

    pub fn embed(&self, text: &str) -> Result<EmbeddingVariant, WorkerError> {
        let encoding = self.tokenizer.encode(text, false).map_err(|e| {
            WorkerError::Embedding(format!("Tokenization failed: {}", e))
        })?;

        let token_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let attention_mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&m| m as i64)
            .collect();

        let seq_len = token_ids.len();

        let input_tensor = Tensor::from_array(([1_usize, seq_len], token_ids))
            .map_err(|e| WorkerError::Embedding(format!("Failed to create input_ids: {}", e)))?;

        let mask_tensor = Tensor::from_array(([1_usize, seq_len], attention_mask))
            .map_err(|e| WorkerError::Embedding(format!("Failed to create attention_mask: {}", e)))?;

        let mut session = self.session.lock();
        let outputs = session
            .run(ort::inputs!["input_ids" => input_tensor, "attention_mask" => mask_tensor])
            .map_err(|e| WorkerError::Embedding(format!("Model inference failed: {}", e)))?;

        let last_hidden = outputs["last_hidden_state"]
            .try_extract_tensor::<f32>()
            .map_err(|e| WorkerError::Embedding(format!("Failed to extract output: {}", e)))?;

        let (shape, data) = last_hidden;
        let hidden_dim = shape[2] as usize;
        let seq_dim = shape[1] as usize;

        let mut embedding = vec![0.0_f32; hidden_dim];
        let attention_mask_raw = encoding.get_attention_mask();
        let valid_tokens = attention_mask_raw.iter().filter(|&&m| m == 1).count();

        for token_idx in 0..seq_dim {
            if attention_mask_raw.get(token_idx).copied().unwrap_or(0) == 1 {
                let offset = token_idx * hidden_dim;
                for dim in 0..hidden_dim {
                    embedding[dim] += data[offset + dim];
                }
            }
        }

        let count = valid_tokens.max(1) as f32;
        for dim in embedding.iter_mut() {
            *dim /= count;
        }

        let norm: f32 = embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for dim in embedding.iter_mut() {
                *dim /= norm;
            }
        }

        Ok(EmbeddingVariant::Dense(embedding))
    }
}
