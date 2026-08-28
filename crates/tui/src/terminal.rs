use std::io::{self, stdout, Stdout, Write};

use ratatui::backend::TermionBackend;
use ratatui::Terminal;
use termion::raw::{IntoRawMode, RawTerminal};

pub type TermionTerminal = Terminal<TermionBackend<RawTerminal<Stdout>>>;

pub fn init() -> io::Result<TermionTerminal> {
    let stdout = stdout().into_raw_mode()?;
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    Ok(terminal)
}

pub fn restore(terminal: &mut TermionTerminal) -> io::Result<()> {
    terminal.backend_mut().flush()?;
    Ok(())
}
