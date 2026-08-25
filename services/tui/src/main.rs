mod api;
mod app;
mod config;
mod conversation;
mod error;
mod event;
mod file_reader;
mod store;
mod terminal;
mod ui;

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use dotenvy::dotenv;

use crate::api::http_client::HttpClient;
use crate::api::{ChatApi, DocsApi, HealthApi};
use crate::app::{App, Command};
use crate::config::Config;
use crate::error::TuiError;
use crate::event::termion_source::{TermionEventSource, spawn_termion_input_thread};
use crate::event::{AppEvent, EventSource};
use crate::file_reader::FileReader;
use crate::store::ConversationStore;

fn main() -> Result<(), TuiError> {
    let _ = dotenv();
    let config = Config::from_env();

    let mut term = terminal::init()?;

    let result = run_app(&mut term, config);

    let _ = terminal::restore(&mut term);
    result
}

fn run_app(term: &mut terminal::TermionTerminal, config: Config) -> Result<(), TuiError> {
    let (event_tx, event_rx) = mpsc::channel::<AppEvent>();
    let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();

    spawn_termion_input_thread(event_tx.clone());

    let rt = tokio::runtime::Runtime::new()?;
    let _guard = rt.enter();

    let health_sender = event_tx.clone();
    let health_config = config.clone();
    let health_url = config.server_url.clone();
    rt.spawn(async move {
        health_poll_loop(&health_url, &health_config, health_sender).await;
    });

    let server_url = config.server_url.clone();
    let response_sender = event_tx.clone();
    let handle = rt.handle().clone();
    std::thread::spawn(move || {
        command_dispatch_loop(&server_url, cmd_rx, response_sender, handle);
    });

    let store = Arc::new(ConversationStore::new(config.conversations_file.clone()));
    let file_reader = FileReader::new(&config);
    let mut app = App::new(store.clone());
    let mut event_source = TermionEventSource::new(event_rx);

    while app.is_running() {
        term.draw(|frame| ui::render(frame, &app))?;

        if let Some(event) = event_source.next_event(Duration::from_millis(100))? {
            let command = app.handle_event(event);

            match command {
                Command::LoadPath(path, recursive) => {
                    let files = if recursive {
                        file_reader.read_files_recursive(&path)
                    } else {
                        file_reader.read_files_flat(&path)
                    };
                    app.show_files_preview(files);
                }
                Command::SaveConversations => {
                    if let Err(e) = app.save() {
                        eprintln!("Failed to save conversations: {}", e);
                    }
                }
                Command::None => {}
                cmd => {
                    if cmd_tx.send(cmd).is_err() {
                        break;
                    }
                }
            }
        }
    }

    if let Err(e) = app.save() {
        eprintln!("Failed to save conversations on exit: {}", e);
    }

    Ok(())
}

async fn health_poll_loop(server_url: &str, config: &Config, sender: mpsc::Sender<AppEvent>) {
    let Ok(api) = HttpClient::new(server_url) else {
        return;
    };
    loop {
        let result = api.health_check().await.map(|r| r.status);
        if sender.send(AppEvent::HealthUpdate(result)).is_err() {
            break;
        }
        tokio::time::sleep(Duration::from_secs(config.health_poll_secs)).await;
    }
}

fn command_dispatch_loop(
    server_url: &str,
    cmd_rx: mpsc::Receiver<Command>,
    sender: mpsc::Sender<AppEvent>,
    handle: tokio::runtime::Handle,
) {
    let server_url = server_url.to_string();

    for command in cmd_rx {
        match command {
            Command::SendChat(req) => {
                let url = server_url.clone();
                let sender_clone = sender.clone();
                handle.spawn(async move {
                    let result = match HttpClient::new(&url) {
                        Ok(client) => client.chat(req).await,
                        Err(e) => Err(e),
                    };
                    let _ = sender_clone.send(AppEvent::ChatResponse(result));
                });
            }
            Command::SendFiles(files) => {
                let documents: Vec<crate::api::dto::DocumentInput> = files
                    .into_iter()
                    .map(|f| crate::api::dto::DocumentInput {
                        text: format!("File: {}\n\n{}", f.path.display(), f.content),
                    })
                    .collect();

                let req = crate::api::dto::AddDocsRequest { documents };
                let url = server_url.clone();
                let sender_clone = sender.clone();
                handle.spawn(async move {
                    let result = match HttpClient::new(&url) {
                        Ok(client) => client.add_docs(req).await,
                        Err(e) => Err(e),
                    };
                    let _ = sender_clone.send(AppEvent::DocsResponse(result));
                });
            }
            Command::LoadPath(_, _) | Command::SaveConversations | Command::None => {}
        }
    }
}
