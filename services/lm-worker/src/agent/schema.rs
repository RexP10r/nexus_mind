use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum LlmResponse {
    #[serde(rename = "think")]
    Think {
        thought: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        next_action: Option<Action>,
    },
    #[serde(rename = "final_answer")]
    FinalAnswer { answer: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action")]
pub enum Action {
    #[serde(rename = "execute_tool")]
    ExecuteTool {
        tool_name: String,
        tool_input: String,
    },
}

pub fn extract_llm_response(raw: &str) -> Result<LlmResponse, String> {
    let cleaned = strip_markdown_fences(raw.trim());

    if let Ok(resp) = serde_json::from_str::<LlmResponse>(&cleaned) {
        return Ok(resp);
    }

    for &(ref prefix, ref suffix) in EXTRACTION_PATTERNS {
        if let Ok(resp) = extract_json_between(&cleaned, prefix, suffix) {
            return Ok(resp);
        }
    }

    Err(cleaned
        .chars()
        .take(200)
        .collect::<String>())
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

static EXTRACTION_PATTERNS: &[(&str, &str)] = &[
    ("```json\n", "\n```"),
    ("```\n", "\n```"),
    ("", ""),
];

fn extract_json_between(text: &str, prefix: &str, suffix: &str) -> Result<LlmResponse, String> {
    if prefix.is_empty() && suffix.is_empty() {
        return serde_json::from_str(text).map_err(|e| e.to_string());
    }
    let start = text.find(prefix).map(|i| i + prefix.len()).unwrap_or(0);
    let after_start = &text[start..];
    let end = after_start.find(suffix).unwrap_or(after_start.len());
    let json_str = &after_start[..end];
    serde_json::from_str(json_str.trim()).map_err(|e| e.to_string())
}

pub fn generate_schema_text() -> String {
    let schema = schemars::schema_for!(LlmResponse);
    serde_json::to_string_pretty(&schema).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_think() {
        let raw = r#"{"type": "think", "thought": "I need to calculate", "next_action": {"action": "execute_tool", "tool_name": "calculate", "tool_input": "2+2"}}"#;
        let resp = extract_llm_response(raw).unwrap();
        assert!(matches!(resp, LlmResponse::Think { .. }));
    }

    #[test]
    fn test_extract_final_answer() {
        let raw = r#"{"type": "final_answer", "answer": "42"}"#;
        let resp = extract_llm_response(raw).unwrap();
        match resp {
            LlmResponse::FinalAnswer { answer } => assert_eq!(answer, "42"),
            _ => panic!("expected FinalAnswer"),
        }
    }

    #[test]
    fn test_extract_with_markdown_fence() {
        let raw = "```json\n{\"type\": \"final_answer\", \"answer\": \"Paris\"}\n```";
        let resp = extract_llm_response(raw).unwrap();
        match resp {
            LlmResponse::FinalAnswer { answer } => assert_eq!(answer, "Paris"),
            _ => panic!("expected FinalAnswer"),
        }
    }

    #[test]
    fn test_extract_invalid() {
        let raw = "some random text without json";
        assert!(extract_llm_response(raw).is_err());
    }

    #[test]
    fn test_schema_is_valid_json() {
        let text = generate_schema_text();
        assert!(serde_json::from_str::<serde_json::Value>(&text).is_ok());
    }
}
