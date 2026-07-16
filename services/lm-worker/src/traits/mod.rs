pub mod llm;
pub mod agent;
pub mod tool;

#[derive(Debug, Clone)]
pub struct GenerationParams {
    pub temperature: f32,
    pub max_tokens: i32,
    pub top_p: f32,
    pub top_k: f32,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 512,
            top_p: 0.0,
            top_k: 0.0,
        }
    }
}
