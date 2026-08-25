use serde::Deserialize;

use crate::agent::rag::schema::extract_json_response;
use crate::error::WorkerError;
use crate::model::{ChatMessage, ChatRole, GenerationParams, Message};

#[derive(Debug, Deserialize)]
struct SummaryResponse {
    #[allow(dead_code)]
    thought: String,
    answer: String,
}

pub fn should_summarize(total: u64, history_max: u32, interval: u32) -> bool {
    total > history_max as u64 && (total - history_max as u64) % interval as u64 == 0
}

pub fn build_summarization_prompt(messages: &[Message], existing_summary: Option<&str>) -> String {
    let conversation_text: String = messages
        .iter()
        .map(|m| format!("[{}]: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n");

    let previous_summary_section = match existing_summary {
        Some(s) if !s.is_empty() => format!(
            r#"
## Previous Context Summary
{}

You MUST incorporate and build upon this previous summary. Do NOT repeat it verbatim, but extend it with new information from the conversation below.
"#,
            s
        ),
        _ => String::new(),
    };

    format!(
        r#"You are an expert conversation analyst. Your task is to create a concise, informative summary of the conversation below.

## CRITICAL INSTRUCTIONS

You MUST create a NARRATIVE SUMMARY - a paragraph that describes what was discussed, NOT a transcript or dialogue format.

### What a summary IS:
- A narrative paragraph describing the conversation
- Written in third person (e.g., "The user asked about...", "The assistant explained...")
- 2-4 sentences capturing key topics, decisions, and important context
- Focused on main points, not every detail

### What a summary is NOT:
- NOT a conversation transcript
- NOT in dialogue format (user/assistant pairs)
- NOT a list of messages
- NOT using JSON keys like "user", "assistant", "messages"

## EXAMPLES

### GOOD EXAMPLE (correct format):
Input conversation:
[user]: What's the weather like today?
[assistant]: It's sunny and 72°F with clear skies.
[user]: Should I bring an umbrella?
[assistant]: No, the forecast shows 0% chance of rain all day.

Correct summary output:
{{"thought": "User asked about weather and whether to bring umbrella", "answer": "The user inquired about today's weather conditions and learned it was sunny at 72 degrees with clear skies. The assistant confirmed no rain was expected, so an umbrella was unnecessary."}}

### BAD EXAMPLE (incorrect format - DO NOT DO THIS):
Bad summary output:
{{"user": "What's the weather?", "assistant": "It's sunny"}}
OR
{{"messages": [{{"role": "user", "content": "What's the weather?"}}]}}

These are WRONG because they echo the conversation format instead of summarizing it.

{previous_summary_section}
## Conversation to Summarize

{conversation_text}

## Your Task

Create a concise narrative summary (2-4 sentences) that captures:
- Main topics discussed
- Key decisions or conclusions reached
- Important context needed for future reference

## Required Output Format

Return ONLY a JSON object with exactly these two fields:
{{"thought": "brief reasoning about what was discussed", "answer": "narrative summary paragraph"}}

CRITICAL: The "answer" field MUST be a narrative paragraph, NOT a conversation format. Do NOT use keys like "user", "assistant", "messages", or "dialogue" in your output.

Return ONLY the JSON object, no other text."#
    )
}

fn is_valid_summary(summary: &str) -> bool {
    let lower = summary.to_lowercase();
    
    let forbidden_patterns = [
        r#""user""#,
        r#""assistant""#,
        r#""messages""#,
        r#""dialogue""#,
        r#""conversation""#,
    ];
    
    let forbidden_count = forbidden_patterns
        .iter()
        .filter(|pattern| lower.contains(*pattern))
        .count();
    
    if forbidden_count >= 2 {
        return false;
    }
    
    if lower.contains(r#""role""#) && lower.contains(r#""content""#) {
        return false;
    }
    
    if summary.len() < 20 {
        return false;
    }
    
    true
}

pub fn summary_params() -> GenerationParams {
    GenerationParams {
        temperature: 0.2,
        max_tokens: 256,
        top_p: 0.9,
        top_k: 32,
    }
}

pub async fn generate_summary(
    llm: &dyn crate::traits::llm::LlmProvider,
    messages: &[Message],
    existing_summary: Option<&str>,
) -> Result<String, WorkerError> {
    let prompt = build_summarization_prompt(messages, existing_summary);
    let chat_messages = vec![ChatMessage {
        role: ChatRole::User,
        content: prompt,
    }];

    let raw_text = llm
        .generate(chat_messages, &summary_params())
        .await
        .map(|o| o.text)
        .map_err(|e| {
            tracing::warn!(error = %e, "LLM summarization failed");
            e
        })?;

    match extract_json_response::<SummaryResponse>(&raw_text) {
        Ok(parsed) => {
            if is_valid_summary(&parsed.answer) {
                tracing::debug!("Summary updated and parsed");
                Ok(parsed.answer)
            } else {
                tracing::warn!(
                    summary_preview = %parsed.answer.chars().take(100).collect::<String>(),
                    "Summary validation failed: response appears to be a conversation echo, not a narrative summary"
                );
                Ok(parsed.answer)
            }
        }
        Err(e) => {
            tracing::error!(
                raw_preview = %raw_text.chars().take(200).collect::<String>(),
                parse_error = %e,
                "Failed to parse summary JSON, using raw text"
            );
            Ok(raw_text)
        }
    }
}
