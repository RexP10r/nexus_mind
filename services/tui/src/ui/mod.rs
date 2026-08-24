pub mod chat;
pub mod help;
pub mod input;
pub mod status;

use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let [title_bar, chat_area, input_area, help_area] = chunks;

    status::render(frame, title_bar, app);
    chat::render(frame, chat_area, app);
    input::render(frame, input_area, app);
    help::render(frame, help_area, app);
}
