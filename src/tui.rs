use crate::Connections;
mod elements;
pub mod style;

use chrono::Utc;
use crossterm;
use crossterm::event::Event;
use crossterm::event::{KeyCode, KeyModifiers};
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

#[derive(Debug)]
pub struct TuiState {
    /// (cols, rows)
    pub window_size: Rect,
    pub connections: Arc<Mutex<Connections>>,
    pub connections_cache: Connections,
    pub needs_update: bool,
    history: History,
    screen: Screen,
    pub prompt: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Screen {
    List,
    Details,
    Detail,
}

impl From<Screen> for usize {
    fn from(input: Screen) -> usize {
        match input {
            Screen::List => 0,
            Screen::Details => 1,
            Screen::Detail => 2,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    cols: (usize, usize),
    rows: (usize, usize),
}

#[allow(dead_code)]
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
    Addr(usize),
    Member(usize),
    Unselectable,
}

#[derive(Debug)]
pub struct History {
    pub current: (Screen, Select),
    prev: Vec<(Screen, Select)>,
}

impl History {
    pub fn peek_prev(&self, n: usize) -> Option<&(Screen, Select)> {
        if n < 1 {
            return Some(&self.current)
        }
        let item:Vec<&(Screen, Select)> = self.prev.iter().rev().skip(n-1).take(1).map(|x|  {
            x
        }).collect();
        if item.len() == 1 {
            Some(item[0])
        } else {
            None
        }
    }
    pub fn pop(&mut self) {
        if let Some(prev) = self.prev.pop() {
            self.current = prev;
        } else {
            self.current = (Screen::List, Select::Addr(0));
        }
    }
    pub fn push(&mut self, new: (Screen, Select)) {
        self.prev.push(self.current);
        self.current = new;
    }
}

impl Select {
    pub fn select(&mut self, value: Option<usize>, max_value: usize) {
        let parsed_value;
        if max_value == 0 {
            parsed_value = 0;
        } else {
            parsed_value = match value {
                Some(value) => value.clamp(0, max_value),
                _ => 0,
            };
        }

        match &self {
            Self::Addr(_) => *self = Self::Addr(parsed_value),
            Self::Member(_) => *self = Self::Member(parsed_value),
            Self::Unselectable => *self = Self::Unselectable,
        }
    }

    #[allow(dead_code)]
    pub fn has_same_value(self, value: usize) -> bool {
        match self {
            Self::Addr(inner) => {
                return inner == value;
            }
            Self::Member(inner) => {
                return inner == value;
            }
            Self::Unselectable => {
                return false
            }
        }
    }

    pub fn sub(&mut self, subtract: usize){
        *self = match &self {
            Self::Addr(value) => {
                Self::Addr(value.checked_sub(subtract).unwrap_or(0))
            },
            Self::Member(value) => {
                Self::Member(value.checked_sub(subtract).unwrap_or(0))
            },
            Self::Unselectable => {
                Self::Unselectable
            }
        }
    }
    pub fn add(&mut self, add: usize, max_value: usize){
        *self = match self {
            Self::Addr(value) => Self::Addr(value.checked_add(add).unwrap_or(usize::MAX).clamp(0,max_value)),
            Self::Member(value) => Self::Member(value.checked_add(add).unwrap_or(usize::MAX).clamp(0, max_value)),
            Self::Unselectable => Self::Unselectable,
        }
    }
}

impl Into<usize> for Select {
    fn into(self) -> usize {
        match self {
            Self::Addr(value) => value,
            Self::Member(value) => value,
            Self::Unselectable => 0,
        }
    }
}
impl Into<usize> for &Select {
    fn into(self) -> usize {
        <Select as Into<usize>>::into(*self)
    }
}

#[allow(dead_code)]
impl TuiState {
    pub async fn new(connections: Arc<Mutex<Connections>>) -> Self {
        let window_size = crossterm::terminal::size().expect("window has a size");

        TuiState {
            window_size: Rect {
                cols: (1, window_size.0.into()),
                rows: (1, window_size.1.into()),
            },
            connections: Arc::clone(&connections),
            needs_update: false,
            connections_cache: TuiState::cache(connections).await,
            history: History {
                current: (Screen::List, Select::Addr(0)),
                prev: vec![],
            },
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
            let out = Arc::clone(&out);
            let header = ProgramHeader {
                bounds: &Rect {
                    cols: (0, self.window_size.cols.1),
                    rows: (0, 0)
                },
            };
            header.render(out).await?;
        }

        {
            match self.history.current {
                (Screen::List, selected_address) => {
                    let address_list_bounds = Rect {
                        cols: (0, self.window_size.cols.1),
                        rows: (1, self.window_size.rows.1),
                    };

                    let addresses = Arc::from(Mutex::new(
                        self.connections_cache
                            .keys()
                            .map(|x| x.clone())
                            .collect_vec(),
                    ));

                    let address_list = AddressList::default(
                        address_list_bounds,
                        addresses,
                        true,
                        selected_address.into(),
                    );


                    {
                        let out = Arc::clone(&out);
                        address_list
                            .render({
                                let out = Arc::clone(&out);
                                out
                            })
                            .await?;
                    }
                }
                (Screen::Details, selected_detail) => {
                    let address_list_bounds = Rect {
                        cols: (0, self.window_size.cols.1.checked_div(3).unwrap_or(10)),
                        rows: (1, self.window_size.rows.1),
                    };

                    let connection_list_bounds = Rect {
                        cols: (address_list_bounds.width(), self.window_size.cols.1),
                        rows: (1, self.window_size.rows.1),
                    };

                    let mut connection_list_items: Vec<TuiResponse> = vec![];


                    if let Some((_, address)) = self.history.peek_prev(1) {
                        if let Select::Addr(address) = address {
                            let addresses = Arc::from(Mutex::new(
                                self.connections_cache
                                    .keys()
                                    .map(|x| x.clone())
                                    .collect_vec(),
                            ));

                            let address_list = AddressList::default(
                                address_list_bounds,
                                addresses,
                                false,
                                *address,
                            );

                            {
                                let out = Arc::clone(&out);
                                address_list
                                    .render({
                                        let out = Arc::clone(&out);
                                        out
                                    })
                                    .await?;
                            }
                            if let Some((_, items)) = self.connections_cache.get_index(*address) {
                                for item in items.into_iter() {
                                    connection_list_items.push(item.to_owned())
                                }
                            }
                        }
                    }

                    let connection_list = ConnectionsList::default(
                        Arc::from(Mutex::from(connection_list_items)),
                        connection_list_bounds,
                        true,
                        selected_detail.into(),
                    );

                    {
                        connection_list
                            .render({
                                let out = Arc::clone(&out);
                                out
                            })
                            .await?;
                    }
                },
                (Screen::Detail, _selected_detail) => {
                    let detail_bounds = Rect {
                        cols: (0, self.window_size.cols.1),
                        rows: (1, self.window_size.rows.1),
                    };

                    if let Some((Screen::List, Select::Addr(address))) = self.history.peek_prev(2) {
                        if let Some((address, responses)) = self.connections_cache.get_index(*address) {
                            if let Some((Screen::Details, Select::Member(member)))  = self.history.peek_prev(1) {
                                if let Some(response ) = responses.get(*member) {
                                    let response_content = &response.http_response.content;
                                    if let Some(content) = response_content {
                                        let content = content.trim().lines().map(|x| {
                                            x
                                        }).collect_vec();
                                        let content = Arc::from(Mutex::new(content));
                                        let detail = DetailWindow::default(detail_bounds, content, true, address.to_string());
                                        let out = Arc::clone(&out);
                                        detail.render(out).await?;
                                    }
                                } 
                            } 
                        } 
                    } 
                }
            }
        }

        let mut out = out.lock().await;

        out.flush()?;
        Ok(())
    }

    fn set_screen(&mut self, screen: Screen) {
        self.screen = screen
    }

    fn get_max_select_size(&self) -> usize {
        match self.history.current.0 {
            Screen::List => match self.connections_cache.is_empty() {
                true => 0,
                false => self.connections_cache.len().checked_sub(1).unwrap_or(0),
            },

            Screen::Details => {
                if let Some((_, addr)) = self.history.peek_prev(1) {
                    if let Some(details) = self.connections_cache.get_index(addr.into()) {
                        match details.1.is_empty() {
                            false => details.1.len().checked_sub(1).unwrap_or(0),
                            true => 0,
                        }
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
            Screen::Detail => {
                0
            }
        }
    }
}

pub async fn parse_cli_event(
    event: Option<crossterm::event::Event>,
    out: Arc<Mutex<Stdout>>,
    tuistate: Arc<Mutex<TuiState>>,
    exit_reason: &mut Option<String>,
) -> anyhow::Result<()> {
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
                        let mut tuistate = tuistate.lock().await;
                        tuistate.history.current.1.sub(1);
                    }
                    (KeyCode::Down, KeyModifiers::NONE) => {
                        let mut tuistate = tuistate.lock().await;
                        let max_select_size = tuistate.get_max_select_size();
                        tuistate.history.current.1.add(1, max_select_size);

                    }
                    (KeyCode::Right, KeyModifiers::NONE) => {}
                    (KeyCode::Left, KeyModifiers::NONE) => {}
                    (KeyCode::Enter, KeyModifiers::NONE) => {
                        let mut tuistate = tuistate.lock().await;
                        match tuistate.history.current.0 {
                            Screen::List => {
                                tuistate.history.push((Screen::Details, Select::Member(0)));
                            }
                            Screen::Details => {
                                tuistate.history.push((Screen::Detail, Select::Unselectable));
                            },
                            Screen::Detail => {
                            }
                        }
                    }
                    (KeyCode::Esc, KeyModifiers::NONE) => {
                        tuistate.lock().await.history.pop();
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
impl TuiResponse {
    fn get_response_as_line(&self) -> String {
        if let Some(content) = &self.http_response.content {
            content.lines().map(|x| {x.to_string() + " "}).collect()
        } else {
            "No content".to_string()
        }
    }
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
