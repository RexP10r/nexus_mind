use crate::error::WorkerError;
use crate::model::{ChatMessage, ChatRole, GenerationParams, Message};

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
        r#"You are a conversation summarizer. Produce a concise, combined summary of the entire conversation history.
Output ONLY the summary text, no JSON, no formatting.
{previous_summary}
## New Messages to Incorporate
{conversation_text}
## Combined Summary"#,
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

    llm.generate(chat_messages, &summary_params())
        .await
        .map(|o| o.text)
        .map_err(|e| {
            tracing::warn!(error = %e, "LLM summarization failed");
            e
        })
}
