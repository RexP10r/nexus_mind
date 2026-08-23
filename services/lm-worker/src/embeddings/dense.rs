use ort::session::Session;
use ort::value::Tensor;
use parking_lot::Mutex;
use tokenizers::Tokenizer;

use crate::config::{Config, get_onnx_log_level};
use crate::error::WorkerError;
use crate::model::EmbeddingVariant;

const MAX_SEQ_LENGTH: usize = 512;

pub struct EmbedderLMProvider {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
}

impl EmbedderLMProvider{
    pub fn from_files(config: &Config) -> Result<Self, WorkerError> {
        let model_path = config.embedding_model_path.as_str();
        let tokenizer_path = config.embedding_tokenizer_path.as_str();
        let log_level = get_onnx_log_level(config)?;

        let session = Session::builder()
            .map_err(|e| {
                WorkerError::Embedding(format!("Failed to create ORT session builder: {}", e))
            })?
            .with_log_level(log_level)
            .map_err(|e| WorkerError::Embedding(format!("Failed to set ORT log level: {}", e)))?
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
            "LM embedding provider initialized"
        );

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
        })
    }

    pub fn embed(&self, text: &str) -> Result<EmbeddingVariant, WorkerError> {
        let encoding = self
            .tokenizer
            .encode(text, false)
            .map_err(|e| WorkerError::Embedding(format!("Tokenization failed: {}", e)))?;

        let mut token_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let mut token_type_ids: Vec<i64> = encoding
            .get_type_ids()
            .iter()
            .map(|&id| id as i64)
            .collect();
        let mut attention_mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&m| m as i64)
            .collect();

        let mut seq_len = token_ids.len();
        if seq_len > MAX_SEQ_LENGTH {
            tracing::warn!("Length of embedded text reached lm's sequense length limit");
            seq_len = MAX_SEQ_LENGTH;
            token_ids = token_ids[..seq_len].to_vec();
            token_type_ids = token_type_ids[..seq_len].to_vec();
            attention_mask = attention_mask[..seq_len].to_vec();
        }

        let input_tensor = Tensor::from_array(([1_usize, seq_len], token_ids))
            .map_err(|e| WorkerError::Embedding(format!("Failed to create input_ids: {}", e)))?;
        let type_ids_tensor =
            Tensor::from_array(([1_usize, seq_len], token_type_ids)).map_err(|e| {
                WorkerError::Embedding(format!("Failed to create token_type_ids: {}", e))
            })?;
        let mask_tensor =
            Tensor::from_array(([1_usize, seq_len], attention_mask)).map_err(|e| {
                WorkerError::Embedding(format!("Failed to create attention_mask: {}", e))
            })?;

        let mut session = self.session.lock();
        let outputs = session
            .run(ort::inputs![
                "input_ids" => input_tensor,
                "token_type_ids" => type_ids_tensor,
                "attention_mask" => mask_tensor
            ])
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
