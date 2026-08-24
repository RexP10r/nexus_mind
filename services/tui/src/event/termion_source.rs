use std::sync::mpsc::Receiver;
use std::time::Duration;

use termion::event::Key;
use termion::input::TermRead;

use crate::error::TuiError;
use crate::event::{AppEvent, EventSource, KeyInput};

pub struct TermionEventSource {
    events: Receiver<AppEvent>,
}

impl TermionEventSource {
    pub fn new(events: Receiver<AppEvent>) -> Self {
        Self { events }
    }
}

impl EventSource for TermionEventSource {
    fn next_event(&mut self, timeout: Duration) -> Result<Option<AppEvent>, TuiError> {
        match self.events.recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(Some(AppEvent::Tick)),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Ok(None),
        }
    }
}

pub fn spawn_termion_input_thread(sender: std::sync::mpsc::Sender<AppEvent>) {
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for key in stdin.keys() {
            let key_input = match key {
                Ok(Key::Char(c)) => match c {
                    '\n' => KeyInput::Enter,
                    '\x03' => KeyInput::CtrlC,
                    '\x0e' => KeyInput::CtrlN,
                    '\x14' => KeyInput::CtrlT,
                    c => KeyInput::Char(c),
                },
                Ok(Key::Backspace) => KeyInput::Backspace,
                Ok(Key::Up) => KeyInput::Up,
                Ok(Key::Down) => KeyInput::Down,
                Ok(Key::Ctrl('c')) => KeyInput::CtrlC,
                Ok(Key::Ctrl('n')) => KeyInput::CtrlN,
                Ok(Key::Ctrl('t')) => KeyInput::CtrlT,
                _ => KeyInput::Unknown,
            };

            if sender.send(AppEvent::Key(key_input)).is_err() {
                break;
            }
        }
    });
}
