use crate::model::{AgentResult, ChatRole, ConversationEntry, Message};

pub fn timeline_to_messages(timeline: &[ConversationEntry]) -> Vec<Message> {
    timeline
        .iter()
        .filter_map(|entry| match entry {
            ConversationEntry::ChatMsg { role, content } => Some(Message {
                role: *role,
                content: content.clone(),
            }),
            _ => None,
        })
        .collect()
}

pub fn build_timeline_entries(
    user_msg: &Message,
    agent_result: &AgentResult,
) -> Vec<ConversationEntry> {
    let mut entries = Vec::new();

    entries.push(ConversationEntry::ChatMsg {
        role: user_msg.role,
        content: user_msg.content.clone(),
    });

    for step in &agent_result.reasoning_steps {
        entries.push(ConversationEntry::Step {
            thought: step.thought.clone(),
            action: step.action.clone(),
            observation: step.observation.clone(),
        });
    }

    if !agent_result.final_answer.is_empty() {
        entries.push(ConversationEntry::ChatMsg {
            role: ChatRole::Assistant,
            content: agent_result.final_answer.clone(),
        });
    }

    entries
}
