use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, MessageRole};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let messages = app.messages();
    if messages.is_empty() {
        let placeholder = Paragraph::new("No messages yet. Type something and press Enter.")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::bordered().title("Chat"));
        frame.render_widget(placeholder, area);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    for msg in messages {
        let (prefix, color) = match msg.role {
            MessageRole::User => ("You: ", Color::Cyan),
            MessageRole::Assistant => ("Assistant: ", Color::Green),
            MessageRole::System => ("", Color::Yellow),
        };

        // Split content by newlines and create separate lines
        let content_lines: Vec<&str> = msg.content.split('\n').collect();
        
        for (i, content_line) in content_lines.iter().enumerate() {
            if i == 0 {
                // First line gets the role prefix
                let role_span = Span::styled(prefix, Style::default().fg(color).add_modifier(Modifier::BOLD));
                let content_span = Span::styled(*content_line, Style::default().fg(Color::White));
                lines.push(Line::from(vec![role_span, content_span]));
            } else {
                // Subsequent lines are indented continuation
                let content_span = Span::styled(*content_line, Style::default().fg(Color::White));
                lines.push(Line::from(content_span));
            }
        }
        
        lines.push(Line::from("")); // blank line between messages
    }

    let visible_height = area.height.saturating_sub(2) as u16;
    let total_lines = lines.len() as u16;
    let max_scroll = total_lines.saturating_sub(visible_height);
    let scroll = max_scroll.saturating_sub(app.scroll_offset());

    let paragraph = Paragraph::new(lines)
        .block(Block::bordered().title("Chat"))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    frame.render_widget(paragraph, area);
}
