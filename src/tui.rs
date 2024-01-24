use crate::Connections;
mod elements;
pub mod style;

use chrono::Utc;
use crossterm;
use crossterm::cursor::{MoveTo, MoveToNextLine};
use crossterm::event::Event;
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::style::Print;
use crossterm::QueueableCommand;
use elements::*;
use futures::lock::Mutex;
use itertools::Itertools;
use std::io::{Stdout, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use testsuite::ResponseContent;
use testsuite::ResponseFormat;

use testsuite::Message;

pub struct TuiState {
    pub cursor: (u16, u16),
    /// (cols, rows)
    pub window_size: Rect,
    pub connections: Arc<Mutex<Connections>>,
    pub connections_cache: Connections,
    pub needs_update: bool,
    pub selected: (Select, Select),
    screen: Screen,
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

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Select {
    Addr(Option<usize>),
    Member(Option<usize>),
}

impl Select {
    pub fn select(&mut self, value: Option<usize>, max_value: usize) {
        let parsed_value;
        if max_value == 0 {
            parsed_value = Some(0);
        } else {
            parsed_value = match value {
                Some(value) => Some(value.clamp(0, max_value)),
                _ => None,
            };
        }

        match &self {
            Self::Addr(_) => *self = Self::Addr(parsed_value),
            Self::Member(_) => *self = Self::Member(parsed_value),
        }
    }

    pub fn has_same_value(self, value: usize) -> bool {
        match self {
            Self::Addr(val) => match val {
                None => false,
                Some(inner) => return inner == value,
            },
            Self::Member(val) => match val {
                None => false,
                Some(inner) => return inner == value,
            },
        }
    }

    pub fn sub(self, subtract: usize) -> Self {
        match self {
            Self::Addr(val) => match val {
                Some(value) => Self::Addr(Some(value.checked_sub(subtract).unwrap_or(0))),
                None => Self::Addr(None),
            },
            Self::Member(val) => match val {
                Some(value) => Self::Member(Some(value.checked_sub(subtract).unwrap_or(0))),
                None => Self::Member(None),
            },
        }
    }
    pub fn add(self, add: usize) -> Self {
        match self {
            Self::Addr(value) => match value {
                Some(value) => Self::Addr(Some(value.checked_add(add).unwrap_or(usize::MAX))),
                None => Self::Addr(Some(0)),
            },
            Self::Member(value) => match value {
                Some(value) => Self::Member(Some(value.checked_add(add).unwrap_or(usize::MAX))),
                None => Self::Member(None),
            },
        }
    }
}

impl Into<Option<usize>> for Select {
    fn into(self) -> Option<usize> {
        match self {
            Self::Addr(value) => value,
            Self::Member(value) => value,
        }
    }
}

impl Into<usize> for Select {
    fn into(self) -> usize {
        match self {
            Self::Addr(value) => value.unwrap_or(0),
            Self::Member(value) => value.unwrap_or(0),
        }
    }
}

#[allow(dead_code)]
impl TuiState {
    pub async fn new(connections: Arc<Mutex<Connections>>) -> Self {
        let window_size = crossterm::terminal::size().expect("window has a size");

        TuiState {
            selected: (Select::Addr(None), Select::Member(None)),
            cursor: (2, 1),
            window_size: Rect {
                cols: (1, window_size.0.into()),
                rows: (1, window_size.1.into()),
            },
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
        }

        {
            let address_list_bounds = Rect {
                cols: (0, self.window_size.cols.1.checked_div(3).unwrap_or(10)),
                rows: (0, self.window_size.rows.1),
            };

            let connection_list_bounds = Rect {
                cols: (address_list_bounds.width()+1, self.window_size.cols.1),
                rows: (0, self.window_size.rows.1)
            };

            let addresses = Arc::from(Mutex::new(self.connections_cache.keys().map(|x| x.clone()).collect_vec()));
            let selected_item:usize = self.selected.0.into();
            let selected_item = selected_item.checked_sub(1).unwrap_or(0);

            let address_list = AddressList::default(
                address_list_bounds,
                addresses,
                
                true,
                selected_item
            );

            let mut connection_list_items : Vec<TuiResponse> = vec![];
            if let Some(selected_item) = &address_list.get_selected_item().await{
                if let Some(items) = self.connections_cache.get(selected_item) {
                    for item in items.into_iter() {
                        connection_list_items.push(item.to_owned())
                    }
                }
            }

            let connection_list_ref = Arc::from(Mutex::new(connection_list_items));
            let selected_item:usize = self.selected.1.into();
            let selected_item = selected_item.checked_sub(1).unwrap_or(0);
                
            {
                let mut out = out.lock().await;
                out.queue(crossterm::cursor::MoveTo(10, 10))?;
                out.queue(Print(format!("{:?}", self.selected)))?;
            }
               

            let connection_list = ConnectionsList::default(
                connection_list_ref,
                connection_list_bounds,
                false,
                selected_item
                );
            {    
                let out = Arc::clone(&out);
                address_list.render({let out = Arc::clone(&out); out}).await?;
                connection_list.render({let out = Arc::clone(&out); out}).await?;
            }
        }
        let mut out = out.lock().await;

        out.queue(crossterm::cursor::MoveTo(self.cursor.0, self.cursor.1))?;

        out.flush()?;
        Ok(())
    }

    fn set_screen(&mut self, screen: Screen) {
        self.screen = screen
    }

    fn select(&mut self, id: usize, max_value: usize) {
        match self.screen {
            Screen::List => {
                self.selected.0.select(Some(id), max_value)
            },
            Screen::Details => {
                self.selected.1.select(Some(id), max_value)
            }
        }
    }

    fn get_select(&self) -> Select {
        match self.screen {
            Screen::List => {
                self.selected.0
            },

            Screen::Details => {
                self.selected.1
            }
        }
    }

    fn get_max_select_size(&self) -> Option<usize>{
        match self.screen {
            Screen::List => {
                match self.connections_cache.is_empty() {
                    true => None,
                    false => Some(self.connections_cache.len())
                }
            },

            Screen::Details => {
                let selected_connection:usize = self.selected.0.into();
                if let Some(details) = self.connections_cache.get_index(selected_connection){
                    match details.1.is_empty() {
                        false => {
                            Some(details.1.len())
                        },
                        true => {
                            Some(selected_connection)
                        }
                    }
                } else {
                    None
                }
            }
        }
    }
}

#[derive(PartialEq, Debug)]
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
    let mut moved: Option<Direction> = None;
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
                        moved = Some(Direction::Up);
                    }
                    (KeyCode::Down, KeyModifiers::NONE) => {
                        //out.queue(crossterm::cursor::MoveDown(1))?;
                        moved = Some(Direction::Down);
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
                tuistate.lock().await.window_size = Rect {
                    cols: (0, cols.into()),
                    rows: (0, rows.into()),
                };
            }
        }
    }

    if let Some(new_state) = changed_state {
        tuistate.lock().await.screen = new_state;
    }

    {
        if moved != None {
            let mut tuistate = tuistate.lock().await;
            let max_select = tuistate.get_max_select_size();
            let new_selected= match moved {
                Some(Direction::Up) => {
                    tuistate.get_select().sub(1)
                },
                Some(Direction::Down) => {
                    tuistate.get_select().add(1)
                },
                _ => {
                    tuistate.get_select()
                },
            };
            tuistate.select(new_selected.into(), max_select.unwrap_or(0));
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
    time: String,
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
                        format: ResponseFormat::Json.to_string(),
                    },
                    time: Utc::now().to_rfc3339(),
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
                            format: ResponseFormat::Json.to_string(),
                        },
                        time: Utc::now().to_rfc3339(),
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
                            format: ResponseFormat::None.to_string(),
                        },
                        time: Utc::now().to_rfc3339(),
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
                                format: ResponseFormat::None.to_string(),
                            },
                            time: Utc::now().to_rfc3339(),
                        }],
                    );
                }
            },
            None => {}
        },
    }
}
