use super::schema::{Action, LlmResponse};
use super::state::AgentState;
use super::tool_handler::ToolHandler;
use crate::agent::AgentResult;

pub(crate) struct ResponseHandler;

impl ResponseHandler {
    pub(crate) fn handle(
        state: &mut AgentState,
        llm_response: LlmResponse,
        tool_handler: &ToolHandler,
    ) -> Option<AgentResult> {
        match llm_response {
            LlmResponse::FinalAnswer { answer } => {
                tracing::info!(answer = %answer, "Agent reached final answer");
                Some(AgentResult {
                    final_answer: answer,
                    total_tokens: state.tokens_used,
                    reasoning_steps: state.reasoning_steps.clone(),
                })
            }
            LlmResponse::Think {
                thought,
                next_action,
            } => match next_action {
                Some(Action::ExecuteTool {
                    tool_name,
                    tool_input,
                }) => {
                    tracing::info!(
                        thought = %thought,
                        tool_name = %tool_name,
                        "Agent decided to use tool"
                    );
                    tool_handler.execute_with_state(state, thought, tool_name, tool_input);
                    None
                }
                None => {
                    tracing::info!(
                        thought = %thought,
                        "Agent thinking without tool action"
                    );
                    state.record_thought(thought);
                    None
                }
            },
        }
    }
}
