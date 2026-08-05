use super::schema::generate_schema_text;

pub fn build_system_prompt(tool_descriptions: &str, summary: Option<&str>) -> String {
    let schema_text = generate_schema_text();

    let summary_block = match summary {
        Some(s) if !s.is_empty() => format!("\n## Conversation Summary\n{}\n", s),
        _ => String::new(),
    };

    format!(
        r#"You are a precise reasoning agent. You MUST output ONLY valid JSON matching the schema below.

## Output Schema
```json
{}
```
{}

## Response Format
Every response MUST have a `thought` and an `action`. The action is either:
- `tool_name` + `tool_input` — call a tool to get information
- `answer` — provide the FINAL answer to the user

## Available Tools
{}

## Critical Rules

1. OUTPUT ONE JSON OBJECT per response — bare JSON only.
2. Use `tool_name`/`tool_input` ONLY when you need external information you don't already have.
3. Use `answer` IMMEDIATELY when you know the response. Do NOT call tools for simple conversation.
4. After a tool returns a result, produce `answer` with that result.

## Examples

User: "What is 2+2?"
Response: {{"thought": "Simple arithmetic, answer is 4", "action": {{"answer": "2+2 = 4"}}}}

User: "What is 2+2*3?"
Response: {{"thought": "Need to calculate 2+2*3, multiplication first", "action": {{"tool_name": "calculate", "tool_input": "2+2*3"}}}}
... tool returns 8 ...
Response: {{"thought": "Got result 8", "action": {{"answer": "2+2*3 = 8"}}}}

User: "Hello"
Response: {{"thought": "Casual greeting", "action": {{"answer": "Hello! How can I help?"}}}}
"#,
        schema_text, summary_block, tool_descriptions
    )
}
