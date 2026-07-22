use uuid::Uuid;

use crate::agent::{AgentAction, AgentStep, Message};
use crate::error::WorkerError;

pub struct AgentState {
    pub id: Uuid,
    pub tokens_used: u32,
    pub conversation: Vec<Message>,
    pub reasoning_steps: Vec<AgentStep>,
}

impl AgentState {
    pub fn new(messages: &[Message]) -> Self {
        Self {
            id: Uuid::new_v4(),
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

    fn clone_state(&self) -> AgentState {
        AgentState {
            id: self.id,
            tokens_used: self.tokens_used,
            conversation: self.conversation.clone(),
            reasoning_steps: self.reasoning_steps.clone(),
        }
    }

    pub fn add_turn(
        &self,
        thought: String,
        observation: String,
        action: Option<AgentAction>,
    ) -> AgentState {
        let mut new_state = self.clone_state();
        new_state.conversation.push(Message {
            role: "system".to_string(),
            content: format!("Observation: {}", observation),
        });
        new_state.reasoning_steps.push(AgentStep {
            thought,
            observation: Some(observation),
            action,
        });
        new_state
    }
}
