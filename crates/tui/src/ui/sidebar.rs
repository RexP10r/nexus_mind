use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if !app.sidebar_visible() {
        return;
    }

    let conversations = app.conversations();
    let active_idx = app.active_conversation_idx();
    let selected_idx = app.sidebar_selection();

    let items: Vec<ListItem> = conversations
        .iter()
        .enumerate()
        .map(|(i, conv)| {
            let is_active = i == active_idx;
            let is_selected = i == selected_idx;

            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let marker = if is_active { "> " } else { "  " };
            let title = if conv.title.len() > 25 {
                format!("{}{}...", marker, &conv.title[..22])
            } else {
                format!("{}{}", marker, conv.title)
            };

            ListItem::new(Line::from(Span::styled(title, style)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Conversations (Enter: select, Esc: close)"),
        );

    let mut state = ListState::default();
    state.select(Some(selected_idx));

    frame.render_stateful_widget(list, area, &mut state);
}
