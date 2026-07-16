#[derive(Debug, Clone, PartialEq)]
pub enum AgentAction {
    FinalAnswer(String),
    Continue(String),
}

pub fn parse_agent_output(output: &str) -> (Option<String>, AgentAction) {
    let output = output.trim();

    let thought = extract_between(output, "Thought:", "\nAction")
        .or_else(|| extract_between(output, "Thought:", "\nFinal"))
        .or_else(|| extract_between_to_end(output, "Thought:"))
        .filter(|s| !s.is_empty());

    if let Some(answer) = extract_between_to_end(output, "Final Answer:") {
        return (thought, AgentAction::FinalAnswer(answer.trim().to_string()));
    }

    (thought, AgentAction::Continue(output.to_string()))
}

fn extract_between(text: &str, start: &str, end: &str) -> Option<String> {
    let start_idx = text.find(start)?;
    let after_start = &text[start_idx + start.len()..];
    let end_idx = after_start.find(end)?;
    Some(after_start[..end_idx].trim().to_string())
}

fn extract_between_to_end(text: &str, start: &str) -> Option<String> {
    let start_idx = text.find(start)?;
    let content = text[start_idx + start.len()..].trim();
    if content.is_empty() {
        None
    } else {
        Some(content.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_final_answer() {
        let text = "Thought: I need to think about this.\nFinal Answer: The result is 108.";
        let (thought, action) = parse_agent_output(text);
        assert_eq!(thought.unwrap(), "I need to think about this.");
        assert_eq!(action, AgentAction::FinalAnswer("The result is 108.".into()));
    }

    #[test]
    fn test_parse_continue() {
        let text = "Thought: Let me calculate this.\nAction: calculate[15*7+3]";
        let (thought, action) = parse_agent_output(text);
        assert_eq!(thought.unwrap(), "Let me calculate this.");
        assert!(matches!(action, AgentAction::Continue(_)));
    }

    #[test]
    fn test_parse_final_answer_no_thought() {
        let text = "Final Answer: 42";
        let (thought, action) = parse_agent_output(text);
        assert!(thought.is_none());
        assert_eq!(action, AgentAction::FinalAnswer("42".into()));
    }

    #[test]
    fn test_parse_final_answer_multiline() {
        let text = "Thought: Step 1.\nThought: Step 2.\nFinal Answer: The capital is Paris.";
        let (thought, action) = parse_agent_output(text);
        assert!(thought.is_some());
        assert_eq!(action, AgentAction::FinalAnswer("The capital is Paris.".into()));
    }
}
