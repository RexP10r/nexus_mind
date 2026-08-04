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
        match llm_response {
            AgentResponse::FinalAnswer { answer } => {
                tracing::debug!(answer = %answer, "Agent reached final answer");
                state.add_final_answer(answer.clone());
                Some(AgentResult {
                    final_answer: answer,
                    total_tokens: state.tokens_used,
                    reasoning_steps: state.reasoning_steps.clone(),
                })
            }
            AgentResponse::Think {
                thought,
                next_action,
            } => match next_action {
                Some(AgentAction::ExecuteTool {
                    tool_name,
                    tool_input,
                }) => {
                    tracing::debug!(
                        thought = %thought,
                        tool_name = %tool_name,
                        "Agent decided to use tool"
                    );
                    tool_handler.execute_with_state(state, thought, tool_name, tool_input);
                    None
                }
                None => {
                    tracing::debug!(
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
