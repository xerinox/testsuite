use crate::tui::Response as TuiResponse;
use crate::Connections;
use crossterm;
use crossterm::event::Event;
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::style::Print;
use crossterm::QueueableCommand;
use futures::lock::Mutex;
use std::io::Stdout;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use testsuite::Response as bl;
use testsuite::ResponseFormat;

use testsuite::Message;

pub async fn parse_cli_event(
    event: Option<crossterm::event::Event>,
    out: Arc<Mutex<Stdout>>,
    tuistate: Arc<Mutex<TuiState>>,
    exit: &mut Option<String>,
) -> anyhow::Result<()> {
    let mut moved = false;
    if let Some(event) = event {
        let mut out = out.lock().await;
        match event {
            Event::Key(key) => {
                let (letter, modifier) = (key.code, key.modifiers);
                let _res = match (letter, modifier) {
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        *exit = Some("Pressed ctrl+q".to_string());
                    }
                    (KeyCode::Up, KeyModifiers::NONE) => {
                        out.queue(crossterm::cursor::MoveUp(1))?;
                        moved = true;
                    }
                    (KeyCode::Down, KeyModifiers::NONE) => {
                        out.queue(crossterm::cursor::MoveDown(1))?;
                        moved = true;
                    }
                    (KeyCode::Right, KeyModifiers::NONE) => {
                        out.queue(crossterm::cursor::MoveRight(1))?;
                        moved = true;
                    }
                    (KeyCode::Left, KeyModifiers::NONE) => {
                        out.queue(crossterm::cursor::MoveLeft(1))?;
                        moved = true;
                    }
                    _ => {
                        out.write(format!("Got letter: {key:?}").as_bytes())?;
                    }
                };
            }
            Event::FocusGained => {}
            Event::FocusLost => {}
            Event::Mouse(_) => {}
            Event::Paste(_) => {}
            Event::Resize(cols, rows) => {
                tuistate.lock().await.window_size = (cols, rows);
            }
        }
    }

    {
        if moved == true {
            let mut tuistate = tuistate.lock().await;

            if let Ok((new_row, new_col)) = crossterm::cursor::position() {
                let (old_row, old_col) = tuistate.cursor;
                if (old_row, old_col) != (new_row, new_col) {
                    tuistate.cursor = (new_row, new_col);
                }
            }
        }
    }
    let out = Arc::clone(&out);
    tuistate.lock().await.render(out).await?;
    Ok(())
}

#[allow(dead_code)]
pub enum Screen {
    List,
    Details,
}

impl From<Screen> for usize {
    fn from(input: Screen) -> usize {
        match input {
            Screen::List => 0,
            Screen::Details => 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    addr: SocketAddr,
    http_response: testsuite::Response,
}

impl Response {
    pub fn to_line(&self, max_length: &usize) -> String {
        let mut linebuffer = String::with_capacity(*max_length);
        let addr = &self.addr.to_string();
        let resp = &self.response_content();
        let resp = resp.trim();
        if let Some(extra_space) = max_length.checked_sub(addr.len() + resp.len() + 7) {
            linebuffer.push_str(&format!(
                "| {} | {}{}|",
                addr,
                resp,
                " ".repeat(extra_space -1)
            ));
        } else if let Some(extra_space) = max_length.checked_sub(addr.len() + 7) {
            //response too long
            linebuffer.push_str(&format!("| {} | {} |", addr, &resp[0..extra_space]));
        } else if let Some(extra_space) = max_length.checked_sub(5) {
            linebuffer.push_str(&format!("| {} |", &addr[0..extra_space]));
        }
        linebuffer
    }

    fn response_content(&self) -> String {
        let mut string_buffer = String::new();
        if let Some(content) = &self.http_response.content {
            string_buffer.push_str(&content);
        } else {
            string_buffer.push_str("None");
        }

        return string_buffer;
    }
}

pub async fn handle_message(message: Message, connections: Arc<Mutex<Connections>>) {
    let mut connections = connections.lock().await;
    match message {
        Message::ConnectionFailed => {}
        Message::Response(message) => match connections.get_mut(&message.addr) {
            Some(data) => {
                let r = TuiResponse {
                    addr: message.addr,
                    http_response: bl {
                        content: Some(message.response.to_string()),
                        format: ResponseFormat::Json,
                    },
                };
                data.push(r)
            }
            None => {
                connections.insert(
                    message.addr,
                    vec![TuiResponse {
                        addr: message.addr,
                        http_response: bl {
                            content: Some(message.response.to_string()),
                            format: ResponseFormat::Json,
                        },
                    }],
                );
            }
        },
        Message::ConnectionReceived(connection) => match connection {
            Some(connection) => match connections.get_mut(&connection) {
                Some(data) => {
                    let r = TuiResponse {
                        addr: connection,
                        http_response: bl {
                            content: None,
                            format: ResponseFormat::None,
                        },
                    };
                    data.push(r);
                }
                None => {
                    connections.insert(
                        connection,
                        vec![TuiResponse {
                            addr: connection,
                            http_response: bl {
                                content: None,
                                format: ResponseFormat::None,
                            },
                        }],
                    );
                }
            },
            None => {}
        },
    }
}

pub struct TuiState {
    pub cursor: (u16, u16),
    /// (cols, rows)
    pub window_size: (u16, u16),
    pub connections: Arc<Mutex<Connections>>,
    pub connections_cache: Connections,
    pub needs_update: bool,
    pub screen: Screen,
    pub prompt: String,
}

#[allow(dead_code)]
impl TuiState {
    pub async fn new(connections: Arc<Mutex<Connections>>) -> Self {
        let window_size = crossterm::terminal::size().expect("window has a size");
        TuiState {
            cursor: (0, 0),
            window_size,
            connections: Arc::clone(&connections),
            needs_update: false,
            connections_cache: TuiState::cache(connections).await,
            screen: Screen::List,
            prompt: String::new(),
        }
    }
    async fn cache(connections: Arc<Mutex<Connections>>) -> Connections {
        let mut cache_to = Connections::new();
        {
            let cache_from = connections.lock().await;
            cache_to.clone_from(&cache_from);
        }
        return cache_to;
    }

    pub async fn render(&mut self, out: Arc<Mutex<Stdout>>) -> anyhow::Result<()> {
        if self.needs_update {
            self.connections_cache = TuiState::cache(Arc::clone(&self.connections)).await;
            self.needs_update = false;
        }

        let mut out = out.lock().await;
        out.queue(crossterm::terminal::Clear(
            crossterm::terminal::ClearType::All,
        ))?;
        out.queue(crossterm::cursor::MoveTo(0, 0))?;
        out.queue(Print("Connections:"))?;
        out.queue(crossterm::cursor::MoveToNextLine(1))?;
        {
            for host in self.connections_cache.keys() {
                let host_events = self.connections_cache.get(host).unwrap();
                for connection in host_events.into_iter() {
                    out.queue(crossterm::cursor::MoveToNextLine(1))?;
                    
                    out.queue(Print(connection.to_line(&self.window_size.0.into())))?;
                }
                out.queue(crossterm::cursor::MoveToNextLine(1))?;
                out.queue(Print("-".repeat(self.window_size.0.into())))?;
            }
        }

        out.queue(crossterm::cursor::MoveTo(0, self.window_size.1 - 2))?;
        out.queue(crossterm::cursor::MoveTo(self.cursor.0, self.cursor.1))?;
        out.flush()?;
        Ok(())
    }
}
