use crate::agent::schema::generate_schema_text;

pub fn build_system_prompt(tool_descriptions: &str) -> String {
    let schema_text = generate_schema_text();

    format!(
        r#"You are a precise reasoning agent. You MUST output ONLY valid JSON matching the schema below.

## Output Schema
```json
{}
```

## Response Types

1. `think` — express your reasoning. Set `next_action` to:
   - `execute_tool` to invoke a tool
   - `null`/omit if you need another think step

2. `final_answer` — provide the final answer to the user.

## Available Tools
{}

## Rules
- Output exactly ONE JSON object per response.
- Do NOT wrap in markdown unless necessary — bare JSON is preferred.
- Always end with a `final_answer`."#,
        schema_text, tool_descriptions
    )
}
