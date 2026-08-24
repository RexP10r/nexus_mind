use crate::api::dto::{AddDocsResponse, ChatRequest, ChatResponse, MessageDto};
use crate::error::TuiError;
use crate::event::{AppEvent, KeyInput};
use crate::file_reader::FileInfo;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Checking,
}

#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Normal,
    DocsConfirm,
}

pub enum Command {
    SendChat(ChatRequest),
    LoadPath(PathBuf, bool),
    SendFiles(Vec<FileInfo>),
    None,
}

pub struct App {
    state: AppState,
    messages: Vec<DisplayMessage>,
    input: String,
    scroll_offset: u16,
    connection_status: ConnectionStatus,
    conversation_id: String,
    running: bool,
    pending: bool,
    pending_files: Vec<FileInfo>,
}

impl App {
    pub fn new(conversation_id: String) -> Self {
        Self {
            state: AppState::Normal,
            messages: Vec::new(),
            input: String::new(),
            scroll_offset: 0,
            connection_status: ConnectionStatus::Checking,
            conversation_id,
            running: true,
            pending: false,
            pending_files: Vec::new(),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn messages(&self) -> &[DisplayMessage] {
        &self.messages
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn scroll_offset(&self) -> u16 {
        self.scroll_offset
    }

    pub fn connection_status(&self) -> ConnectionStatus {
        self.connection_status
    }

    pub fn pending(&self) -> bool {
        self.pending
    }

    pub fn handle_event(&mut self, event: AppEvent) -> Command {
        match event {
            AppEvent::Key(key) => self.handle_key(key),
            AppEvent::ChatResponse(result) => self.handle_chat_response(result),
            AppEvent::DocsResponse(result) => self.handle_docs_response(result),
            AppEvent::HealthUpdate(result) => self.handle_health_update(result),
            AppEvent::Tick => Command::None,
        }
    }

    fn handle_key(&mut self, key: KeyInput) -> Command {
        match self.state {
            AppState::Normal => self.handle_key_normal(key),
            AppState::DocsConfirm => self.handle_key_docs_confirm(key),
        }
    }

    fn handle_key_normal(&mut self, key: KeyInput) -> Command {
        match key {
            KeyInput::CtrlC => {
                self.running = false;
                Command::None
            }
            KeyInput::Up => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                Command::None
            }
            KeyInput::Down => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                Command::None
            }
            KeyInput::Enter if !self.input.is_empty() && !self.pending => self.submit_input(),
            KeyInput::Backspace => {
                self.input.pop();
                Command::None
            }
            KeyInput::Char(c) if !self.pending => {
                self.input.push(c);
                Command::None
            }
            _ => Command::None,
        }
    }

    fn handle_key_docs_confirm(&mut self, key: KeyInput) -> Command {
        match key {
            KeyInput::CtrlC => {
                self.state = AppState::Normal;
                self.input.clear();
                self.pending_files.clear();
                Command::None
            }
            KeyInput::Enter => {
                let input = self.input.trim().to_lowercase();
                self.input.clear();

                if input == "y" || input == "yes" {
                    let files = std::mem::take(&mut self.pending_files);
                    self.state = AppState::Normal;

                    if files.is_empty() {
                        self.messages.push(DisplayMessage {
                            role: MessageRole::System,
                            content: "[No files to send]".to_string(),
                        });
                        return Command::None;
                    }

                    self.pending = true;
                    Command::SendFiles(files)
                } else {
                    self.state = AppState::Normal;
                    self.pending_files.clear();
                    self.messages.push(DisplayMessage {
                        role: MessageRole::System,
                        content: "[Cancelled]".to_string(),
                    });
                    Command::None
                }
            }
            KeyInput::Backspace => {
                self.input.pop();
                Command::None
            }
            KeyInput::Char(c) => {
                self.input.push(c);
                Command::None
            }
            _ => Command::None,
        }
    }

    fn submit_input(&mut self) -> Command {
        let text = std::mem::take(&mut self.input);
        self.scroll_offset = 0;

        if text.starts_with('/') {
            return self.handle_command(text);
        }

        self.messages.push(DisplayMessage {
            role: MessageRole::User,
            content: text.clone(),
        });
        self.pending = true;
        Command::SendChat(ChatRequest {
            conversation_id: self.conversation_id.clone(),
            message: MessageDto {
                role: "user".to_string(),
                content: text,
            },
            temperature: None,
            max_tokens: None,
            top_p: None,
            top_k: None,
        })
    }

    fn handle_command(&mut self, text: String) -> Command {
        let parts: Vec<&str> = text.splitn(3, ' ').collect();
        let cmd = parts[0].to_lowercase();
        
        match cmd.as_str() {
            "/docs" => {
                if parts.len() == 1 {
                    self.messages.push(DisplayMessage {
                        role: MessageRole::System,
                        content: "Usage: /docs [flat|recursive] <path>".to_string(),
                    });
                    return Command::None;
                }

                let (recursive, path_str) = if parts.len() == 3 {
                    match parts[1].to_lowercase().as_str() {
                        "flat" => (false, parts[2]),
                        "recursive" => (true, parts[2]),
                        _ => {
                            self.messages.push(DisplayMessage {
                                role: MessageRole::System,
                                content: "Usage: /docs [flat|recursive] <path>".to_string(),
                            });
                            return Command::None;
                        }
                    }
                } else {
                    (false, parts[1])
                };

                let path = PathBuf::from(path_str);
                Command::LoadPath(path, recursive)
            }
            "/help" => {
                self.messages.push(DisplayMessage {
                    role: MessageRole::System,
                    content: "Commands:\n  /docs [flat|recursive] <path> - add documents to knowledge base\n  /help - show this help\n  /clear - clear chat history".to_string(),
                });
                Command::None
            }
            "/clear" => {
                self.messages.clear();
                self.scroll_offset = 0;
                Command::None
            }
            _ => {
                self.messages.push(DisplayMessage {
                    role: MessageRole::System,
                    content: format!("[Unknown command: {}]", cmd),
                });
                Command::None
            }
        }
    }

    fn handle_chat_response(&mut self, result: Result<ChatResponse, TuiError>) -> Command {
        self.pending = false;
        match result {
            Ok(resp) => {
                self.messages.push(DisplayMessage {
                    role: MessageRole::Assistant,
                    content: resp.message.content,
                });
            }
            Err(e) => {
                self.messages.push(DisplayMessage {
                    role: MessageRole::System,
                    content: format!("[Error: {}]", e),
                });
            }
        }
        Command::None
    }

    fn handle_docs_response(&mut self, result: Result<AddDocsResponse, TuiError>) -> Command {
        self.pending = false;
        match result {
            Ok(resp) => {
                self.messages.push(DisplayMessage {
                    role: MessageRole::System,
                    content: format!("[Added {} document(s)]", resp.added),
                });
            }
            Err(e) => {
                self.messages.push(DisplayMessage {
                    role: MessageRole::System,
                    content: format!("[Error: {}]", e),
                });
            }
        }
        Command::None
    }

    fn handle_health_update(&mut self, result: Result<String, TuiError>) -> Command {
        self.connection_status = match result {
            Ok(_) => ConnectionStatus::Connected,
            Err(_) => ConnectionStatus::Disconnected,
        };
        Command::None
    }

    pub fn show_files_preview(&mut self, files: Vec<FileInfo>) {
        if files.is_empty() {
            self.messages.push(DisplayMessage {
                role: MessageRole::System,
                content: "[No supported files found]".to_string(),
            });
            return;
        }

        let file_count = files.len();
        let total_size: u64 = files.iter().map(|f| f.size).sum();
        let total_kb = total_size as f64 / 1024.0;

        let mut preview = format!("Found {} file(s) ({:.1}KB total):\n", file_count, total_kb);
        for file in &files {
            let size_kb = file.size as f64 / 1024.0;
            preview.push_str(&format!("  - {} ({:.1}KB)\n", file.path.display(), size_kb));
        }
        preview.push_str("Send? (y/n)");

        self.pending_files = files;
        self.state = AppState::DocsConfirm;
        self.messages.push(DisplayMessage {
            role: MessageRole::System,
            content: preview,
        });
    }
}
