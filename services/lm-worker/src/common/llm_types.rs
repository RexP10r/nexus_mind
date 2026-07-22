#[derive(Debug, Clone)]
pub struct LlmMessage {
    pub role: LlmRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct GenerateOutput {
    pub text: String,
    pub tokens_processed: u32,
    pub tokens_generated: u32,
}

#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub is_ready: bool,
    pub model_name: String,
    pub context_length: u32,
}
