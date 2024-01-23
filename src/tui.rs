use crate::Connections;
pub mod elements;
use elements::*;
use chrono::{DateTime, Utc};
use crossterm;
use crossterm::event::Event;
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::style::Print;
use crossterm::QueueableCommand;
use futures::lock::Mutex;
use std::io::{Stdout, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use testsuite::ResponseContent;
use testsuite::ResponseFormat;

use testsuite::Message;

pub struct TuiState {
    pub cursor: (u16, u16),
    /// (cols, rows)
    pub window_size: (u16, u16),
    pub connections: Arc<Mutex<Connections>>,
    pub connections_cache: Connections,
    pub needs_update: bool,
    pub selected: (usize, usize),
    pub screen: Screen,
    pub prompt: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
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

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    cols: (usize, usize),
    rows: (usize, usize),
}

impl Rect {
    pub fn new<T>(cols: (T, T), rows: (T, T)) -> Rect
    where
        T: Into<usize>,
    {
        Rect {
            cols: (cols.0.into(), cols.1.into()),
            rows: (rows.0.into(), rows.1.into()),
        }
    }
    pub fn width(self) -> usize {
        self.cols.0.abs_diff(self.cols.1)
    }
    pub fn height(self) -> usize {
        self.rows.0.abs_diff(self.rows.1)
    }
}


#[allow(dead_code)]
impl TuiState {
    pub async fn new(connections: Arc<Mutex<Connections>>) -> Self {
        let window_size = crossterm::terminal::size().expect("window has a size");
        TuiState {
            selected: (0, 0),
            cursor: (2, 1),
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

        {
            let out = Arc::clone(&out);
            let mut out = out.lock().await;
            out.queue(crossterm::terminal::Clear(
                crossterm::terminal::ClearType::All,
            ))?;
            out.queue(crossterm::cursor::MoveTo(0, 0))?;
            out.queue(crossterm::cursor::MoveToNextLine(1))?;
        }

        {
            let out = Arc::clone(&out);
            let mut cl = ConnectionList::default(
                &self.connections_cache,
                Rect::new((0, self.window_size.0), (0, self.window_size.1)),
                self.selected,
                self.screen,
            );
            if let Err(e) = cl
                .render({
                    let out = Arc::clone(&out);
                    out
                })
                .await
            {
                println!("{e:?}");
            } else {
                self.selected = cl.selected();
            }
        }
        let out = Arc::clone(&out);
        let mut out = out.lock().await;

        out.queue(crossterm::cursor::MoveTo(0, self.window_size.1 - 2))?;
        out.queue(Print(format!(
            "Selected item: {}, {}, cursor pos: {}, {}",
            self.selected.0, self.selected.1, self.cursor.0, self.cursor.1
        )))?;
        out.queue(crossterm::cursor::MoveTo(self.cursor.0, self.cursor.1))?;

        out.flush()?;
        Ok(())
    }
}

#[derive(PartialEq)]
enum Direction {
    Up,
    Down,
}

pub async fn parse_cli_event(
    event: Option<crossterm::event::Event>,
    out: Arc<Mutex<Stdout>>,
    tuistate: Arc<Mutex<TuiState>>,
    exit_reason: &mut Option<String>,
) -> anyhow::Result<()> {
    let mut moved: (Option<Direction>, Option<Direction>) = (None, None);
    let mut changed_state: Option<Screen> = None;
    if let Some(event) = event {
        let mut out = out.lock().await;
        match event {
            Event::Key(key) => {
                let (letter, modifier) = (key.code, key.modifiers);
                let _res = match (letter, modifier) {
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        *exit_reason = Some("Pressed ctrl+q".to_string());
                    }
                    (KeyCode::Up, KeyModifiers::NONE) => {
                        //out.queue(crossterm::cursor::MoveUp(1))?;
                        moved = (Some(Direction::Up), None);
                    }
                    (KeyCode::Down, KeyModifiers::NONE) => {
                        //out.queue(crossterm::cursor::MoveDown(1))?;
                        moved = (Some(Direction::Down), None);
                    }
                    (KeyCode::Right, KeyModifiers::NONE) => {
                        //out.queue(crossterm::cursor::MoveRight(1))?;
                        //moved = (0, 1);
                    }
                    (KeyCode::Left, KeyModifiers::NONE) => {
                        //out.queue(crossterm::cursor::MoveLeft(1))?;
                        //moved = (0, -1);
                    }
                    (KeyCode::Enter, KeyModifiers::NONE) => {
                        changed_state = Some(Screen::Details);
                    }
                    (KeyCode::Esc, KeyModifiers::NONE) => {
                        changed_state = Some(Screen::List);
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

    if let Some(new_state) = changed_state {
        tuistate.lock().await.screen = new_state;
    }

    {
        if moved != (None, None) {
            let mut tuistate = tuistate.lock().await;
            match tuistate.screen {
                Screen::List => moved = (moved.0, None),
                Screen::Details => moved = (None, moved.0),
            }

            if let Ok((new_row, new_col)) = crossterm::cursor::position() {
                let (old_row, old_col) = tuistate.cursor;
                if (old_row, old_col) != (new_row, new_col) {
                    tuistate.cursor = (new_row, new_col);
                }
            }

            let new_selected = match moved {
                (Some(Direction::Up), None) => (
                    tuistate.selected.0.checked_sub(1).unwrap_or(0),
                    tuistate.selected.1,
                ),
                (None, Some(Direction::Up)) => (
                    tuistate.selected.0,
                    tuistate.selected.1.checked_sub(1).unwrap_or(0),
                ),
                (Some(Direction::Down), None) => (
                    tuistate
                        .selected
                        .0
                        .checked_add(1)
                        .unwrap_or(tuistate.window_size.1.into()),
                    tuistate.selected.1,
                ),
                (None, Some(Direction::Down)) => (
                    tuistate.selected.1,
                    tuistate
                        .selected
                        .1
                        .checked_add(1)
                        .unwrap_or(tuistate.window_size.1.into()),
                ),
                (None, None) => tuistate.selected,
                _ => panic!("Do not support diagonally"),
            };
            tuistate.selected = new_selected;
        }
    }
    let out = Arc::clone(&out);
    tuistate.lock().await.render(out).await?;
    Ok(())
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TuiResponse {
    addr: SocketAddr,
    http_response: ResponseContent,
    time: DateTime<Utc>,
}

pub async fn handle_message(message: Message, connections: Arc<Mutex<Connections>>) {
    let mut connections = connections.lock().await;
    match message {
        Message::ConnectionFailed => {}
        Message::Response(message) => match connections.get_mut(&message.addr.ip()) {
            Some(data) => {
                let r = TuiResponse {
                    addr: message.addr,
                    http_response: ResponseContent {
                        content: Some(message.response.to_string()),
                        format: ResponseFormat::Json,
                    },
                    time: Utc::now(),
                };
                data.push(r)
            }
            None => {
                connections.insert(
                    message.addr.ip(),
                    vec![TuiResponse {
                        addr: message.addr,
                        http_response: ResponseContent {
                            content: Some(message.response.to_string()),
                            format: ResponseFormat::Json,
                        },
                        time: Utc::now(),
                    }],
                );
            }
        },
        Message::ConnectionReceived(connection) => match connection {
            Some(connection) => match connections.get_mut(&connection.ip()) {
                Some(data) => {
                    let r = TuiResponse {
                        addr: connection,
                        http_response: ResponseContent {
                            content: Some("Established Connection".to_string()),
                            format: ResponseFormat::None,
                        },
                        time: Utc::now(),
                    };
                    data.push(r);
                }
                None => {
                    connections.insert(
                        connection.ip(),
                        vec![TuiResponse {
                            addr: connection,
                            http_response: ResponseContent {
                                content: None,
                                format: ResponseFormat::None,
                            },
                            time: Utc::now(),
                        }],
                    );
                }
            },
            None => {}
        },
    }
}
