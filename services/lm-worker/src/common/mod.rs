pub mod llm_types;
pub mod tools;
pub mod traits;

#[derive(Debug, Clone)]
pub struct GenerationParams {
    pub temperature: f32,
    pub max_tokens: u32,
    pub top_p: f32,
    pub top_k: u32,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self {
            temperature: 0.2,
            max_tokens: 512,
            top_p: 0.9,
            top_k: 32,
        }
    }
}
