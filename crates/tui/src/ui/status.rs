use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, ConnectionStatus};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let title = Span::styled(
        " Nexus Mind ",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    );

    let (status_text, status_color) = match app.connection_status() {
        ConnectionStatus::Connected => ("● Connected", Color::Green),
        ConnectionStatus::Disconnected => ("● Disconnected", Color::Red),
        ConnectionStatus::Checking => ("● Checking...", Color::Yellow),
    };

    let status = Span::styled(
        status_text,
        Style::default().fg(status_color),
    );

    let line = Line::from(vec![
        title,
        Span::raw(" ".repeat(area.width.saturating_sub(30) as usize)),
        status,
    ]);

    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
