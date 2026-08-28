use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let hints = if app.sidebar_visible() {
        vec![
            Span::styled("↑↓", Style::default().fg(Color::Yellow)),
            Span::styled(":navigate  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::styled(":select  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::styled(":close sidebar", Style::default().fg(Color::DarkGray)),
        ]
    } else {
        vec![
            Span::styled("Ctrl-C", Style::default().fg(Color::Yellow)),
            Span::styled(":quit  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Ctrl-N", Style::default().fg(Color::Yellow)),
            Span::styled(":new conv  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Ctrl-T", Style::default().fg(Color::Yellow)),
            Span::styled(":sidebar  ", Style::default().fg(Color::DarkGray)),
            Span::styled("↑↓", Style::default().fg(Color::Yellow)),
            Span::styled(":scroll  ", Style::default().fg(Color::DarkGray)),
            Span::styled("/help", Style::default().fg(Color::Cyan)),
            Span::styled(":commands", Style::default().fg(Color::DarkGray)),
        ]
    };

    let line = Line::from(hints);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
