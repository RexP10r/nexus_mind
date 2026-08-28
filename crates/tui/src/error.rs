#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Server returned error: {0}")]
    Server(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
