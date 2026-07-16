pub mod parser;
pub mod react;
pub mod tools;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    pub iteration: u32,
    pub thought: Option<String>,
    pub action: Option<String>,
    pub action_input: Option<String>,
    pub observation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub final_answer: String,
    pub reasoning_steps: Vec<ReasoningStep>,
}
