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

pub fn messages_to_llm(messages: &[Message], system_prompt: &str) -> Vec<LlmMessage> {
    let mut llm_msgs: Vec<LlmMessage> = Vec::with_capacity(messages.len() + 1);

    llm_msgs.push(LlmMessage {
        role: LlmRole::System,
        content: system_prompt.to_string(),
    });

    for msg in messages {
        let role = match msg.role.as_str() {
            "system" => LlmRole::System,
            "user" => LlmRole::User,
            "assistant" => LlmRole::Assistant,
            _ => LlmRole::User,
        };
        llm_msgs.push(LlmMessage {
            role,
            content: msg.content.clone(),
        });
    }

    llm_msgs
}

pub fn reasoning_steps_to_llm(steps: &[AgentStep]) -> Vec<LlmMessage> {
    let mut out = Vec::with_capacity(steps.len() * 2);

    for step in steps {
        match (&step.action, &step.observation) {
            (
                Some(AgentAction::ExecuteTool {
                    tool_name,
                    tool_input,
                }),
                Some(obs),
            ) => {
                out.push(LlmMessage {
                    role: LlmRole::Assistant,
                    content: format!(
                        "Thought: {}\n\nAction: {}({})",
                        step.thought, tool_name, tool_input
                    ),
                });
                out.push(LlmMessage {
                    role: LlmRole::User,
                    content: format!("Observation: {}", obs),
                });
            }
            _ => {
                out.push(LlmMessage {
                    role: LlmRole::Assistant,
                    content: step.thought.clone(),
                });
            }
        }
    }

    out
}

pub fn build_llm_context(
    conversation: &[Message],
    steps: &[AgentStep],
    system_prompt: &str,
) -> Vec<LlmMessage> {
    let mut msgs = messages_to_llm(conversation, system_prompt);
    msgs.extend(reasoning_steps_to_llm(steps));
    msgs
}
