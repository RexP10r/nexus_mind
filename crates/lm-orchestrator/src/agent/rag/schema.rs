use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{model::AgentAction, traits::agent_response::AgentResponse};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PreRetrievalResponse {
    intent: String,
    information_gap: String,
    action: AgentAction,
}
impl AgentResponse for PreRetrievalResponse {
    fn action(&self) -> AgentAction {
        self.action.clone()
    }
    fn thought(&self) -> String {
        format!(
            "INTENT:\n{}\n\nINFORMATION GAP:\n{}",
            self.intent, self.information_gap
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PostRetrievalResponse {
    context_evaluating: String,
    self_questions: String,
    self_answers: String,
    final_answer: String,
}
impl AgentResponse for PostRetrievalResponse {
    fn action(&self) -> AgentAction {
        AgentAction::Finish {
            answer: self.final_answer.clone(),
        }
    }
    fn thought(&self) -> String {
        format!(
            "CONTEXT EVALUATING:\n{}\n\nSELF QUESTIONS:\n{}\n\nSELF ANSWERS:\n{}",
            self.context_evaluating, self.self_questions, self.self_answers
        )
    }
}

pub fn extract_llm_response<T: AgentResponse + DeserializeOwned>(raw: &str) -> Result<T, String> {
    extract_json_response::<T>(raw)
}

pub fn extract_json_response<T: DeserializeOwned>(raw: &str) -> Result<T, String> {
    let cleaned = strip_markdown_fences(raw.trim());

    if let Ok(resp) = serde_json::from_str::<T>(&cleaned) {
        return Ok(resp);
    }

    for (prefix, suffix) in EXTRACTION_PATTERNS {
        if let Ok(resp) = extract_json_between::<T>(&cleaned, prefix, suffix) {
            return Ok(resp);
        }
    }

    Err(cleaned.chars().take(200).collect::<String>())
}

fn strip_markdown_fences(raw: &str) -> String {
    let without_backticks = raw
        .strip_prefix("```json")
        .or_else(|| raw.strip_prefix("```"))
        .unwrap_or(raw);
    without_backticks
        .strip_suffix("```")
        .unwrap_or(without_backticks)
        .trim()
        .to_string()
}

static EXTRACTION_PATTERNS: &[(&str, &str)] =
    &[("```json\n", "\n```"), ("```\n", "\n```"), ("", "")];

fn extract_json_between<T: DeserializeOwned>(
    text: &str,
    prefix: &str,
    suffix: &str,
) -> Result<T, String> {
    if prefix.is_empty() && suffix.is_empty() {
        return serde_json::from_str(text).map_err(|e| e.to_string());
    }
    let start = text.find(prefix).map(|i| i + prefix.len()).unwrap_or(0);
    let after_start = &text[start..];
    let end = after_start.find(suffix).unwrap_or(after_start.len());
    let json_str = &after_start[..end];
    serde_json::from_str(json_str.trim()).map_err(|e| e.to_string())
}

pub fn generate_schema_text<T: schemars::JsonSchema + AgentResponse>() -> String {
    let schema = schemars::schema_for!(T);
    serde_json::to_string_pretty(&schema).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    mod preretrieval {
        use super::*;

        #[test]
        fn test_extract_tool_call() {
            let raw = r#"{"intent": "calculate sum", "information_gap": "need to compute", "action": {"tool_name": "calculate", "tool_input": "2+2"}}"#;
            let resp = extract_llm_response::<PreRetrievalResponse>(raw).unwrap();
            assert_eq!(resp.intent, "calculate sum");
            assert_eq!(resp.information_gap, "need to compute");
            assert!(matches!(resp.action(), AgentAction::ExecuteTool { .. }));
        }

        #[test]
        fn test_extract_finish() {
            let raw = r#"{"intent": "greeting", "information_gap": "none", "action": {"answer": "Hello!"}}"#;
            let resp = extract_llm_response::<PreRetrievalResponse>(raw).unwrap();
            assert_eq!(resp.intent, "greeting");
            assert_eq!(resp.information_gap, "none");
            match resp.action() {
                AgentAction::Finish { answer } => assert_eq!(answer, "Hello!"),
                _ => panic!("expected Finish"),
            }
        }

        #[test]
        fn test_thought_formatting() {
            let raw = r#"{"intent": "find info", "information_gap": "missing data", "action": {"answer": "done"}}"#;
            let resp = extract_llm_response::<PreRetrievalResponse>(raw).unwrap();
            let thought = resp.thought();
            assert!(thought.contains("INTENT:"));
            assert!(thought.contains("find info"));
            assert!(thought.contains("INFORMATION GAP:"));
            assert!(thought.contains("missing data"));
        }

        #[test]
        fn test_extract_with_markdown_fence() {
            let raw = "```json\n{\"intent\": \"test\", \"information_gap\": \"none\", \"action\": {\"answer\": \"Paris\"}}\n```";
            let resp = extract_llm_response::<PreRetrievalResponse>(raw).unwrap();
            match resp.action() {
                AgentAction::Finish { answer } => assert_eq!(answer, "Paris"),
                _ => panic!("expected Finish"),
            }
        }

        #[test]
        fn test_extract_invalid() {
            let raw = "some random text without json";
            assert!(extract_llm_response::<PreRetrievalResponse>(raw).is_err());
        }

        #[test]
        fn test_extract_missing_action() {
            let raw = r#"{"intent": "just thinking", "information_gap": "none"}"#;
            assert!(extract_llm_response::<PreRetrievalResponse>(raw).is_err());
        }

        #[test]
        fn test_extract_missing_intent() {
            let raw = r#"{"information_gap": "none", "action": {"answer": "test"}}"#;
            assert!(extract_llm_response::<PreRetrievalResponse>(raw).is_err());
        }

        #[test]
        fn test_schema_is_valid_json() {
            let text = generate_schema_text::<PreRetrievalResponse>();
            assert!(serde_json::from_str::<serde_json::Value>(&text).is_ok());
        }
    }

    mod postretrieval {
        use super::*;

        #[test]
        fn test_extract_full_response() {
            let raw = r#"{
                "context_evaluating": "Context is sufficient",
                "self_questions": "Is this accurate?",
                "self_answers": "Yes, based on context",
                "final_answer": "The answer is 42"
            }"#;
            let resp = extract_llm_response::<PostRetrievalResponse>(raw).unwrap();
            assert_eq!(resp.context_evaluating, "Context is sufficient");
            assert_eq!(resp.self_questions, "Is this accurate?");
            assert_eq!(resp.self_answers, "Yes, based on context");
            assert_eq!(resp.final_answer, "The answer is 42");
        }

        #[test]
        fn test_action_always_returns_finish() {
            let raw = r#"{
                "context_evaluating": "eval",
                "self_questions": "q",
                "self_answers": "a",
                "final_answer": "final"
            }"#;
            let resp = extract_llm_response::<PostRetrievalResponse>(raw).unwrap();
            match resp.action() {
                AgentAction::Finish { answer } => assert_eq!(answer, "final"),
                _ => panic!("PostRetrievalResponse must always return Finish"),
            }
        }

        #[test]
        fn test_thought_formatting() {
            let raw = r#"{
                "context_evaluating": "context eval text",
                "self_questions": "question text",
                "self_answers": "answer text",
                "final_answer": "final"
            }"#;
            let resp = extract_llm_response::<PostRetrievalResponse>(raw).unwrap();
            let thought = resp.thought();
            assert!(thought.contains("CONTEXT EVALUATING:"));
            assert!(thought.contains("context eval text"));
            assert!(thought.contains("SELF QUESTIONS:"));
            assert!(thought.contains("question text"));
            assert!(thought.contains("SELF ANSWERS:"));
            assert!(thought.contains("answer text"));
        }

        #[test]
        fn test_extract_with_markdown_fence() {
            let raw = "```json\n{\"context_evaluating\": \"eval\", \"self_questions\": \"q\", \"self_answers\": \"a\", \"final_answer\": \"Paris\"}\n```";
            let resp = extract_llm_response::<PostRetrievalResponse>(raw).unwrap();
            match resp.action() {
                AgentAction::Finish { answer } => assert_eq!(answer, "Paris"),
                _ => panic!("expected Finish"),
            }
        }

        #[test]
        fn test_extract_invalid() {
            let raw = "some random text without json";
            assert!(extract_llm_response::<PostRetrievalResponse>(raw).is_err());
        }

        #[test]
        fn test_extract_missing_final_answer() {
            let raw = r#"{
                "context_evaluating": "eval",
                "self_questions": "q",
                "self_answers": "a"
            }"#;
            assert!(extract_llm_response::<PostRetrievalResponse>(raw).is_err());
        }

        #[test]
        fn test_extract_missing_context_evaluating() {
            let raw = r#"{
                "self_questions": "q",
                "self_answers": "a",
                "final_answer": "final"
            }"#;
            assert!(extract_llm_response::<PostRetrievalResponse>(raw).is_err());
        }

        #[test]
        fn test_schema_is_valid_json() {
            let text = generate_schema_text::<PostRetrievalResponse>();
            assert!(serde_json::from_str::<serde_json::Value>(&text).is_ok());
        }
    }
}
