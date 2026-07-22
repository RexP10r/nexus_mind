pub mod prompt;
pub mod rag;
pub mod schema;
pub mod state;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub thought: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<AgentAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentAction {
    #[serde(rename = "execute_tool")]
    ExecuteTool {
        tool_name: String,
        tool_input: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub final_answer: String,
    pub total_tokens: u32,
    pub reasoning_steps: Vec<AgentStep>,
}
