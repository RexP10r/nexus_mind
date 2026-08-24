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
        .map(|m| format!("[{}]: {}\n", m.role, m.content))
        .collect();

    let previous_summary = match existing_summary {
        Some(s) if !s.is_empty() => format!("\n## Previous Summary\n{}\n", s),
        _ => String::new(),
    };

    format!(
        r#"You are a conversation summarizer. Produce a concise paragraph summarizing the conversation history below.
{previous_summary}
## Messages to Summarize
{conversation_text}
## Instructions
Return ONLY a JSON object with "thought" (your brief reasoning) and "answer" (the summary paragraph):
{{"thought": "...", "answer": "..."}}
Do NOT use tool calls or any other action type — only a plain "answer" string."#,
        previous_summary = previous_summary,
        conversation_text = conversation_text
    )
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

    let raw_text = llm.generate(chat_messages, &summary_params())
        .await
        .map(|o| o.text)
        .map_err(|e| {
            tracing::warn!(error = %e, "LLM summarization failed");
            e
        })?;

    match extract_json_response::<SummaryResponse>(&raw_text) {
        Ok(parsed) => Ok(parsed.answer),
        Err(_) => {
            tracing::error!(raw_preview = %raw_text.chars().take(200).collect::<String>(), "Failed to parse summary JSON, using raw text");
            Ok(raw_text)
        }
    }
}
