use super::schema::AgentResponse;
use super::state::AgentState;
use super::tool_handler::ToolHandler;
use crate::model::{AgentAction, AgentResult};

pub(crate) struct ResponseHandler;

impl ResponseHandler {
    pub(crate) fn handle(
        state: &mut AgentState,
        llm_response: AgentResponse,
        tool_handler: &ToolHandler,
    ) -> Option<AgentResult> {
        match llm_response.action {
            AgentAction::ExecuteTool {
                tool_name,
                tool_input,
            } => {
                tracing::debug!(
                    thought = %llm_response.thought,
                    tool_name = %tool_name,
                    "Agent decided to use tool"
                );
                tool_handler.execute_with_state(state, llm_response.thought, tool_name, tool_input);
                None
            }
            AgentAction::Finish { answer } => {
                tracing::debug!(thought = %llm_response.thought, answer = %answer, "Agent reached final answer");
                state.add_final_answer(answer.clone());
                Some(AgentResult {
                    final_answer: answer,
                    total_tokens: state.tokens_used,
                    reasoning_steps: state.reasoning_steps.clone(),
                })
            }
        }
    }
}
