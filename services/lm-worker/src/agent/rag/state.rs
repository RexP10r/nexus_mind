use crate::error::WorkerError;
use crate::model::{AgentAction, AgentStep, Message};

#[derive(Debug)]
pub struct AgentState {
    pub tokens_used: u32,
    pub conversation: Vec<Message>,
    pub reasoning_steps: Vec<AgentStep>,
}

impl AgentState {
    pub fn new(messages: &[Message]) -> Self {
        Self {
            tokens_used: 0,
            conversation: messages.to_vec(),
            reasoning_steps: Vec::new(),
        }
    }

    pub fn consume_tokens(
        &mut self,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> Result<(), WorkerError> {
        self.tokens_used = self
            .tokens_used
            .checked_add(prompt_tokens)
            .and_then(|v| v.checked_add(completion_tokens))
            .ok_or_else(|| WorkerError::Agent("token counter overflow".to_string()))?;
        Ok(())
    }

    pub fn record_thought(&mut self, thought: String) {
        self.reasoning_steps.push(AgentStep {
            thought,
            action: None,
            observation: None,
        });
    }

    pub fn record_parse_error(&mut self, raw: &str) {
        let preview: String = raw.chars().take(200).collect();
        self.reasoning_steps.push(AgentStep {
            thought: format!(
                "[PARSE ERROR] LLM response could not be parsed: {}",
                preview
            ),
            action: None,
            observation: None,
        });
    }

    pub fn add_final_answer(&mut self, answer: String) {
        self.conversation.push(Message {
            role: "assistant".to_string(),
            content: answer,
        });
    }

    pub fn add_turn(&mut self, thought: String, observation: String, action: Option<AgentAction>) {
        self.reasoning_steps.push(AgentStep {
            thought,
            observation: Some(observation),
            action,
        });
    }
}
