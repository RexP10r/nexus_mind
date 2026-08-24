use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;

use crate::app::{App, COMMANDS};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if !app.autocomplete_active() {
        return;
    }

    let matches = app.autocomplete_matches();
    if matches.is_empty() {
        return;
    }

    let max_height = 5;
    let height = (matches.len() as u16).min(max_height) + 2;
    
    let popup_area = Rect {
        x: area.x,
        y: area.y.saturating_sub(height),
        width: area.width,
        height,
    };

    let items: Vec<ListItem> = matches
        .iter()
        .enumerate()
        .map(|(i, &cmd_idx)| {
            let cmd = &COMMANDS[cmd_idx];
            let is_selected = i == app.autocomplete_selection();
            
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let line = Line::from(vec![
                Span::styled(format!("{:<12}", cmd.command), style),
                Span::styled(cmd.description, style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Commands"));

    frame.render_widget(list, popup_area);
}
