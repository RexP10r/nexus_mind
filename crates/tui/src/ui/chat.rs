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
    let visible_width = area.width.saturating_sub(2) as usize;
    for msg in messages {
        let (prefix, color) = match msg.role {
            MessageRole::User => ("You: ", Color::Cyan),
            MessageRole::Assistant => ("Assistant: ", Color::Green),
            MessageRole::System => ("", Color::Yellow),
        };

        let content_lines: Vec<String> = msg.content.split('\n').map(|s| s.to_string()).collect();

        for (i, content_line) in content_lines.iter().enumerate() {
            if i == 0 {
                let role_span =
                    Span::styled(prefix, Style::default().fg(color).add_modifier(Modifier::BOLD));
                let content_span =
                    Span::styled(content_line.clone(), Style::default().fg(Color::White));
                lines.push(Line::from(vec![role_span, content_span]));
            } else {
                let content_span =
                    Span::styled(content_line.clone(), Style::default().fg(Color::White));
                lines.push(Line::from(content_span));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from("-".repeat(visible_width))); 
    }

    let mut visual_line_count: u16 = 0;
    for line in &lines {
        let line_len = line.to_string().len();
        let wrapped = if line_len == 0 {
            1
        } else {
            (line_len + visible_width - 1) / visible_width
        };
        visual_line_count = visual_line_count.saturating_add(wrapped as u16);
    }

    let visible_height = area.height.saturating_sub(2) as u16;
    let max_scroll = visual_line_count.saturating_sub(visible_height);
    let scroll = max_scroll.saturating_sub(app.scroll_offset());


    let paragraph = Paragraph::new(lines)
        .block(Block::bordered().title("Chat"))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    frame.render_widget(paragraph, area);
}
