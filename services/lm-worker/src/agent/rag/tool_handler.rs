use crate::agent::rag::state::AgentState;
use crate::model::AgentAction;
use crate::tools::registry::{InMemoryToolRegistry, ToolRegistry};

pub(crate) struct ToolHandler<'a> {
    registry: &'a InMemoryToolRegistry,
}

impl<'a> ToolHandler<'a> {
    pub(crate) fn new(registry: &'a InMemoryToolRegistry) -> Self {
        Self { registry }
    }

    pub(crate) fn descriptions(&self) -> String {
        self.registry.descriptions()
    }

    fn tool_not_found_message(&self, tool_name: &str) -> String {
        let desc = self.registry.descriptions();
        let available = if desc.is_empty() {
            "none".to_string()
        } else {
            desc
        };
        format!("Tool '{}' not found. Available: {}", tool_name, available)
    }

    pub(crate) fn execute(&self, tool_name: &str, tool_input: &str) -> String {
        self.registry
            .execute(tool_name, tool_input)
            .unwrap_or_else(|| self.tool_not_found_message(tool_name))
    }

    pub(crate) fn execute_with_state(
        &self,
        state: &mut AgentState,
        thought: String,
        tool_name: String,
        tool_input: String,
    ) {
        if tool_name.trim().is_empty() || tool_input.trim().is_empty() {
            let observation = format!(
                "Tool invocation failed: tool_name and tool_input must be non-empty. Got tool_name='{}', tool_input='{}'",
                tool_name, tool_input
            );
            tracing::warn!(
                tool_name = %tool_name,
                tool_input = %tool_input,
                "Empty tool name or input"
            );
            let action = AgentAction::ExecuteTool {
                tool_name,
                tool_input,
            };
            state.add_turn(thought, observation, Some(action));
            return;
        }

        let start = std::time::Instant::now();
        let observation = self.execute(&tool_name, &tool_input);
        let elapsed_ms = start.elapsed().as_millis();

        tracing::info!(
            tool_name = %tool_name,
            tool_input = %tool_input,
            observation = %observation,
            elapsed_ms,
            "Tool executed"
        );

        let action = AgentAction::ExecuteTool {
            tool_name,
            tool_input,
        };
        state.add_turn(thought, observation, Some(action));
    }
}
