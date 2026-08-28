use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::app::{App, AppState};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let (prompt_style, prompt_text) = match app.state() {
        AppState::Normal => (Style::default().fg(Color::Green), "> "),
        AppState::DocsConfirm => (Style::default().fg(Color::Yellow), "confirm> "),
    };

    let pending_indicator = if app.pending() {
        Span::styled(" [processing...]", Style::default().fg(Color::DarkGray))
    } else {
        Span::raw("")
    };

    let prompt = Span::styled(prompt_text, prompt_style.add_modifier(Modifier::BOLD));
    let input_text = Span::raw(app.input());

    let line = Line::from(vec![prompt, input_text, pending_indicator]);

    let conv_title = app.active_conversation().title.clone();
    let display_title = if conv_title.len() > 30 {
        format!("{}...", &conv_title[..27])
    } else {
        conv_title
    };

    let paragraph = Paragraph::new(line).block(Block::bordered().title(display_title));

    frame.render_widget(paragraph, area);
}
