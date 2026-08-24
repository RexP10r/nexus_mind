pub mod termion_source;

use std::time::Duration;

use crate::api::dto::{AddDocsResponse, ChatResponse};
use crate::error::TuiError;

pub enum KeyInput {
    Char(char),
    Enter,
    Backspace,
    Up,
    Down,
    CtrlC,
    Unknown,
}

pub enum AppEvent {
    Key(KeyInput),
    ChatResponse(Result<ChatResponse, TuiError>),
    DocsResponse(Result<AddDocsResponse, TuiError>),
    HealthUpdate(Result<String, TuiError>),
    Tick,
}

pub trait EventSource {
    fn next_event(&mut self, timeout: Duration) -> Result<Option<AppEvent>, TuiError>;
}
