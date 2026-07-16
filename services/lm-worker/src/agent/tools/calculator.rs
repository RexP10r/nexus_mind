use crate::traits::tool::Tool;

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
            Ok(val) => val.to_string(),
            Err(e) => format!("Error evaluating expression: {}", e),
        }
    }
}
