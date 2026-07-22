use crate::common::traits::tool::Tool;

pub struct CalculatorTool;

impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculate"
    }

    fn description(&self) -> &str {
        "evaluates a mathematical expression (supports +, -, *, /, parentheses, functions)"
    }

    fn execute(&self, input: &str) -> String {
        match meval::eval_str(input) {
            Ok(val) => {
                let result = val.to_string();
                tracing::debug!(
                    tool_name = "calculate",
                    expression = %input,
                    result = %result,
                    "Calculator evaluation succeeded"
                );
                result
            }
            Err(e) => {
                tracing::warn!(
                    tool_name = "calculate",
                    expression = %input,
                    error = %e,
                    "Calculator evaluation failed"
                );
                format!("Error evaluating expression: {}", e)
            }
        }
    }
}
