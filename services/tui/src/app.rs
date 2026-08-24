use crate::api::dto::{AddDocsResponse, ChatRequest, ChatResponse, MessageDto};
use crate::conversation::Conversation;
use crate::error::TuiError;
use crate::event::{AppEvent, KeyInput};
use crate::file_reader::FileInfo;
use crate::store::ConversationStore;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Checking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl serde::Serialize for MessageRole {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
        };
        serializer.serialize_str(s)
    }
}

impl<'de> serde::Deserialize<'de> for MessageRole {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "user" => Ok(MessageRole::User),
            "assistant" => Ok(MessageRole::Assistant),
            "system" => Ok(MessageRole::System),
            _ => Err(serde::de::Error::custom(format!("unknown role: {}", s))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub role: MessageRole,
    pub content: String,
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
    SaveConversations,
    None,
}

#[derive(Debug, Clone)]
pub struct CommandSuggestion {
    pub command: &'static str,
    pub description: &'static str,
}

pub const COMMANDS: &[CommandSuggestion] = &[
    CommandSuggestion {
        command: "/help",
        description: "Show commands",
    },
    CommandSuggestion {
        command: "/docs",
        description: "Add files to knowledge base",
    },
    CommandSuggestion {
        command: "/clear",
        description: "Clear this chat",
    },
    CommandSuggestion {
        command: "/new",
        description: "New conversation",
    },
    CommandSuggestion {
        command: "/switch",
        description: "Switch conversation",
    },
    CommandSuggestion {
        command: "/delete",
        description: "Delete conversation",
    },
    CommandSuggestion {
        command: "/list",
        description: "List conversations",
    },
    CommandSuggestion {
        command: "/rename",
        description: "Rename conversation",
    },
];

pub struct App {
    state: AppState,
    conversations: Vec<Conversation>,
    active_conversation_idx: usize,
    input: String,
    scroll_offset: u16,
    connection_status: ConnectionStatus,
    running: bool,
    pending: bool,
    pending_files: Vec<FileInfo>,
    autocomplete_active: bool,
    autocomplete_selection: usize,
    autocomplete_matches: Vec<usize>,
    sidebar_visible: bool,
    sidebar_selection: usize,
    store: Arc<ConversationStore>,
}

impl App {
    pub fn new(store: Arc<ConversationStore>) -> Self {
        let conversations = store.load().unwrap_or_else(|_| vec![Conversation::new()]);
        let conversations = if conversations.is_empty() {
            vec![Conversation::new()]
        } else {
            conversations
        };

        Self {
            state: AppState::Normal,
            conversations,
            active_conversation_idx: 0,
            input: String::new(),
            scroll_offset: 0,
            connection_status: ConnectionStatus::Checking,
            running: true,
            pending: false,
            pending_files: Vec::new(),
            autocomplete_active: false,
            autocomplete_selection: 0,
            autocomplete_matches: Vec::new(),
            sidebar_visible: false,
            sidebar_selection: 0,
            store,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn active_conversation(&self) -> &Conversation {
        &self.conversations[self.active_conversation_idx]
    }

    pub fn active_conversation_mut(&mut self) -> &mut Conversation {
        &mut self.conversations[self.active_conversation_idx]
    }

    pub fn conversations(&self) -> &[Conversation] {
        &self.conversations
    }

    pub fn active_conversation_idx(&self) -> usize {
        self.active_conversation_idx
    }

    pub fn sidebar_visible(&self) -> bool {
        self.sidebar_visible
    }

    pub fn sidebar_selection(&self) -> usize {
        self.sidebar_selection
    }

    pub fn messages(&self) -> Vec<DisplayMessage> {
        self.active_conversation()
            .messages()
            .into_iter()
            .map(|(role, content)| DisplayMessage { role, content })
            .collect()
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

    pub fn autocomplete_active(&self) -> bool {
        self.autocomplete_active
    }

    pub fn autocomplete_selection(&self) -> usize {
        self.autocomplete_selection
    }

    pub fn autocomplete_matches(&self) -> &[usize] {
        &self.autocomplete_matches
    }

    pub fn update_autocomplete(&mut self) {
        if self.state != AppState::Normal {
            self.autocomplete_active = false;
            return;
        }

        let input = self.input.trim();
        if input.starts_with('/') && !input.contains(' ') {
            let prefix = input.to_lowercase();
            self.autocomplete_matches = COMMANDS
                .iter()
                .enumerate()
                .filter(|(_, cmd)| cmd.command.to_lowercase().starts_with(&prefix))
                .map(|(i, _)| i)
                .collect();

            self.autocomplete_active = !self.autocomplete_matches.is_empty();
            if self.autocomplete_active
                && self.autocomplete_selection >= self.autocomplete_matches.len()
            {
                self.autocomplete_selection = 0;
            }
        } else {
            self.autocomplete_active = false;
        }
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
        if self.sidebar_visible {
            return self.handle_key_sidebar(key);
        }

        if self.autocomplete_active {
            match key {
                KeyInput::Up => {
                    if !self.autocomplete_matches.is_empty() {
                        self.autocomplete_selection = if self.autocomplete_selection == 0 {
                            self.autocomplete_matches.len() - 1
                        } else {
                            self.autocomplete_selection - 1
                        };
                    }
                    return Command::None;
                }
                KeyInput::Down => {
                    if !self.autocomplete_matches.is_empty() {
                        self.autocomplete_selection =
                            (self.autocomplete_selection + 1) % self.autocomplete_matches.len();
                    }
                    return Command::None;
                }
                KeyInput::Enter | KeyInput::Char('\t') => {
                    if !self.autocomplete_matches.is_empty() {
                        let cmd_idx = self.autocomplete_matches[self.autocomplete_selection];
                        let cmd = COMMANDS[cmd_idx].command;
                        self.input = format!("{} ", cmd);
                        self.autocomplete_active = false;
                        return Command::None;
                    }
                }
                KeyInput::Char('\x1b') => {
                    self.autocomplete_active = false;
                    return Command::None;
                }
                _ => {}
            }
        }

        match key {
            KeyInput::CtrlC => {
                self.running = false;
                Command::None
            }
            KeyInput::CtrlN => {
                self.new_conversation();
                Command::SaveConversations
            }
            KeyInput::CtrlT => {
                self.sidebar_visible = !self.sidebar_visible;
                self.sidebar_selection = self.active_conversation_idx;
                Command::None
            }
            KeyInput::Up if !self.autocomplete_active => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                Command::None
            }
            KeyInput::Down if !self.autocomplete_active => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                Command::None
            }
            KeyInput::Enter if !self.input.is_empty() && !self.pending => self.submit_input(),
            KeyInput::Backspace => {
                self.input.pop();
                self.update_autocomplete();
                Command::None
            }
            KeyInput::Char(c) if !self.pending => {
                self.input.push(c);
                self.update_autocomplete();
                Command::None
            }
            _ => Command::None,
        }
    }

    fn handle_key_sidebar(&mut self, key: KeyInput) -> Command {
        match key {
            KeyInput::CtrlC | KeyInput::Char('\x1b') | KeyInput::CtrlT => {
                self.sidebar_visible = false;
                Command::None
            }
            KeyInput::Up => {
                if self.sidebar_selection > 0 {
                    self.sidebar_selection -= 1;
                }
                Command::None
            }
            KeyInput::Down => {
                if self.sidebar_selection < self.conversations.len() - 1 {
                    self.sidebar_selection += 1;
                }
                Command::None
            }
            KeyInput::Enter => {
                self.active_conversation_idx = self.sidebar_selection;
                self.sidebar_visible = false;
                self.scroll_offset = 0;
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
                        self.active_conversation_mut().add_message(
                            MessageRole::System,
                            "[No files to send]".to_string(),
                        );
                        return Command::None;
                    }

                    self.pending = true;
                    Command::SendFiles(files)
                } else {
                    self.state = AppState::Normal;
                    self.pending_files.clear();
                    self.active_conversation_mut().add_message(
                        MessageRole::System,
                        "[Cancelled]".to_string(),
                    );
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

        self.active_conversation_mut()
            .add_message(MessageRole::User, text.clone());
        self.pending = true;
        Command::SendChat(ChatRequest {
            conversation_id: self.active_conversation().conversation_id.clone(),
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
                    self.active_conversation_mut().add_message(
                        MessageRole::System,
                        "Usage: /docs [flat|recursive] <path>".to_string(),
                    );
                    return Command::None;
                }

                let (recursive, path_str) = if parts.len() == 3 {
                    match parts[1].to_lowercase().as_str() {
                        "flat" => (false, parts[2]),
                        "recursive" => (true, parts[2]),
                        _ => {
                            self.active_conversation_mut().add_message(
                                MessageRole::System,
                                "Usage: /docs [flat|recursive] <path>".to_string(),
                            );
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
                self.active_conversation_mut().add_message(
                    MessageRole::System,
                    "Commands:\n  /docs [flat|recursive] <path> - add documents to knowledge base\n  /help - show this help\n  /clear - clear chat history\n  /new - new conversation\n  /switch - switch conversation\n  /delete - delete conversation\n  /list - list conversations\n  /rename <name> - rename conversation".to_string(),
                );
                Command::None
            }
            "/clear" => {
                let conv = self.active_conversation_mut();
                conv.timeline.clear();
                conv.total_messages = 0;
                self.scroll_offset = 0;
                Command::SaveConversations
            }
            "/new" => {
                self.new_conversation();
                Command::SaveConversations
            }
            "/switch" => {
                self.sidebar_visible = true;
                self.sidebar_selection = self.active_conversation_idx;
                Command::None
            }
            "/delete" => {
                self.delete_conversation(self.active_conversation_idx);
                Command::SaveConversations
            }
            "/list" => {
                let mut list = String::from("Conversations:\n");
                for (i, conv) in self.conversations.iter().enumerate() {
                    let marker = if i == self.active_conversation_idx {
                        "> "
                    } else {
                        "  "
                    };
                    list.push_str(&format!("{}{}. {}\n", marker, i + 1, conv.title));
                }
                self.active_conversation_mut()
                    .add_message(MessageRole::System, list);
                Command::None
            }
            "/rename" => {
                if parts.len() < 2 {
                    self.active_conversation_mut().add_message(
                        MessageRole::System,
                        "Usage: /rename <name>".to_string(),
                    );
                    return Command::None;
                }
                let new_name = parts[1..].join(" ");
                self.active_conversation_mut().title = new_name;
                self.active_conversation_mut()
                    .add_message(MessageRole::System, "[Conversation renamed]".to_string());
                Command::SaveConversations
            }
            _ => {
                self.active_conversation_mut().add_message(
                    MessageRole::System,
                    format!("[Unknown command: {}]", cmd),
                );
                Command::None
            }
        }
    }

    fn new_conversation(&mut self) {
        self.conversations.push(Conversation::new());
        self.active_conversation_idx = self.conversations.len() - 1;
        self.scroll_offset = 0;
    }

    fn delete_conversation(&mut self, idx: usize) {
        if self.conversations.len() <= 1 {
            self.active_conversation_mut().add_message(
                MessageRole::System,
                "[Cannot delete the last conversation]".to_string(),
            );
            return;
        }

        self.conversations.remove(idx);
        if self.active_conversation_idx >= self.conversations.len() {
            self.active_conversation_idx = self.conversations.len() - 1;
        }
        self.scroll_offset = 0;
    }

    fn handle_chat_response(&mut self, result: Result<ChatResponse, TuiError>) -> Command {
        self.pending = false;
        match result {
            Ok(resp) => {
                self.active_conversation_mut().add_message(
                    MessageRole::Assistant,
                    resp.message.content,
                );
            }
            Err(e) => {
                self.active_conversation_mut().add_message(
                    MessageRole::System,
                    format!("[Error: {}]", e),
                );
            }
        }
        Command::SaveConversations
    }

    fn handle_docs_response(&mut self, result: Result<AddDocsResponse, TuiError>) -> Command {
        self.pending = false;
        match result {
            Ok(resp) => {
                self.active_conversation_mut().add_message(
                    MessageRole::System,
                    format!("[Added {} document(s)]", resp.added),
                );
            }
            Err(e) => {
                self.active_conversation_mut().add_message(
                    MessageRole::System,
                    format!("[Error: {}]", e),
                );
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
            self.active_conversation_mut().add_message(
                MessageRole::System,
                "[No supported files found]".to_string(),
            );
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
        self.active_conversation_mut()
            .add_message(MessageRole::System, preview);
    }

    pub fn save(&self) -> Result<(), TuiError> {
        self.store.save(&self.conversations)
    }
}
