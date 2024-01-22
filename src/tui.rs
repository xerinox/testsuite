use crate::tui::Response as TuiResponse;
use crate::Connections;
use chrono::{DateTime, Utc};
use colored::{ColoredString, Colorize};
use crossterm;
use crossterm::cursor::MoveTo;
use crossterm::event::Event;
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::style::Print;
use crossterm::QueueableCommand;
use futures::lock::Mutex;
use itertools::Itertools;
use std::cmp::Ordering;
use std::io::Stdout;
use std::io::Write;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use testsuite::Response as bl;
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

#[derive(Debug)]
struct ConnectionList<'a> {
    groups: Vec<ConnectionListGroup<'a>>,
    selected: (usize, usize),
    address_list_bounds: Rect,
    details_list_bounds: Rect,
    window_size: Rect,
    screen: Screen,
}

#[derive(Debug, Clone, Copy)]
struct Rect {
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

impl<'a> ConnectionList<'a> {
    fn default(
        groups: &Connections,
        window_size: Rect,
        selected: (usize, usize),
        screen: Screen,
    ) -> ConnectionList {
        let connection_list_bounds = Rect {
            rows: (window_size.rows.0, window_size.rows.1),
            cols: (window_size.cols.0, window_size.cols.1),
        };

        let addr_list_length: usize = connection_list_bounds
            .width()
            .checked_div(3)
            .unwrap_or(0);

        let (address_list_bounds, details_list_bounds): (Rect, Rect) = (
            Rect::new((1, addr_list_length), (1, connection_list_bounds.rows.1)),
            Rect::new((addr_list_length + 1, connection_list_bounds.cols.1), (1, connection_list_bounds.rows.1)),
        );

        let groups: Vec<ConnectionListGroup> = groups
            .iter()
            .map(|(host, lines)| ConnectionListGroup {
                host,
                lines: lines
                    .iter()
                    .map(|response| ConnectionListGroupLine { response })
                    .collect(),
            })
            .collect();

        let selected = (selected.0.max(0), selected.1.max(0));

        ConnectionList {
            address_list_bounds,
            details_list_bounds,
            window_size,
            selected,
            groups,
            screen,
        }
    }

    async fn render(&mut self, out: Arc<Mutex<Stdout>>) -> anyhow::Result<()> {
        let mut connection_group_list: Vec<String> =
            self.groups.iter().map(|x| x.render()).collect();

        self.selected.0 = self.selected.0.max(0).min(connection_group_list.len().checked_sub(1).unwrap_or(1));
        let mut connection_group_members =
            match self.groups.get(self.selected.0.checked_sub(1).unwrap_or(0)) {
                Some(connection_group_members) => {
                    connection_group_members.print_group_members(&self.details_list_bounds)
                }
                None => {
                    vec![]
                }
            };
        self.selected.1 = self.selected.1.max(0).min(connection_group_members.len().checked_sub(1).unwrap_or(0));

        match connection_group_list.len().cmp(&connection_group_members.len()) {
            Ordering::Less => {
                connection_group_list.resize(connection_group_members.len(), String::new());
            }
            Ordering::Greater => {
                connection_group_members.resize(connection_group_members.len(), String::new());
            }
            Ordering::Equal => {}
        }

        let mut buffer: Vec<(Option<&str>, Option<&str>)> = vec![];

        for pair in connection_group_list.iter().zip_longest(connection_group_members.iter()) {
            match pair {
                itertools::EitherOrBoth::Both(list, members) => {
                    buffer.push((Some(list), Some(members)));
                }
                itertools::EitherOrBoth::Left(list) => {
                    buffer.push((Some(list), None));
                }
                itertools::EitherOrBoth::Right(members) => {
                    buffer.push((None, Some(members)));
                }
            }
        }
        let mut out = out.lock().await;
        for (line, line_content) in buffer
            .into_iter()
            .enumerate()
            .take(self.window_size.height())
        {
            let selected_lines = (line == self.selected.0, (line == self.selected.1) && self.screen == Screen::Details);
            let line = line as u16;
            let line_text = self.print_line(line_content, selected_lines);
            out.queue(MoveTo(1, line))?;
            out.queue(Print(format!("{}", line_text.0)))?;
            out.queue(Print("|"))?;
            out.queue(Print(format!("{}", line_text.1)))?;
        }

        out.queue(crossterm::cursor::MoveToNextLine(1))?;
        out.queue(Print(format!("CurrentlySelectedItems:{:?}", self.selected)))?;
        out.queue(crossterm::cursor::MoveToNextLine(1))?;

        Ok(())
    }

    fn pad_and_truncate(str: &str, max_len: usize) -> String {
        let pad = "-".repeat(max_len);
        let mut buf = String::from(str);
        buf.push_str(&pad);
        buf.truncate(max_len);
        buf
    }

    fn print_line(
        &self,
        line: (Option<&str>, Option<&str>),
        selected: (bool, bool),
    ) -> (ColoredString, ColoredString) {
        let address_max_length = self
            .address_list_bounds.width();
        let members_max_length = self
            .details_list_bounds.width();

        let result = match line {
            (Some(list), Some(members)) => {
                let addr_max = address_max_length.checked_sub(list.len()).unwrap_or(0);
                let member_max = members_max_length.checked_sub(members.len()).unwrap_or(0);
                match selected {
                    (true, true) => (
                        Self::pad_and_truncate(list, addr_max).black().on_white(),
                        Self::pad_and_truncate(members, member_max)
                            .black()
                            .on_white(),
                    ),
                    (true, false) => (
                        Self::pad_and_truncate(list, addr_max).black().on_white(),
                        Self::pad_and_truncate(members, member_max).on_blue(),
                    ),
                    (false, true) => (
                        Self::pad_and_truncate(list, addr_max).on_blue(),
                        Self::pad_and_truncate(members, member_max)
                            .black()
                            .on_white(),
                    ),
                    (false, false) => (
                        Self::pad_and_truncate(list, addr_max).on_blue(),
                        Self::pad_and_truncate(members, member_max).on_blue(),
                    ),
                }
            }
            (Some(list), None) => {
                let addr_max = address_max_length.checked_sub(list.len()).unwrap_or(0);
                let member_max = members_max_length;
                match selected.0 {
                    true => (
                        Self::pad_and_truncate(list, addr_max).black().on_white(),
                        Self::pad_and_truncate("", member_max).on_blue(),
                    ),
                    false => (
                        Self::pad_and_truncate(list, addr_max).on_blue(),
                        Self::pad_and_truncate("", member_max).on_blue(),
                    ),
                }
            }
            (None, Some(members)) => {
                let addr_max = address_max_length;
                let member_max = members_max_length.checked_sub(members.len()).unwrap_or(0);
                match selected.1 {
                    true => (
                        Self::pad_and_truncate("", addr_max).normal(),
                        Self::pad_and_truncate(members, member_max)
                            .black()
                            .on_white(),
                    ),
                    false => (
                        Self::pad_and_truncate("", addr_max).normal(),
                        Self::pad_and_truncate(members, member_max).normal(),
                    ),
                }
            }
            _ => {
                let addr_pad = " ".repeat(address_max_length);
                let member_pad = " ".repeat(members_max_length);
                (addr_pad.on_blue(), member_pad.on_blue())
            }
        };
        (result.0, result.1)
    }
}

#[allow(dead_code)]
#[derive(Debug)]
struct ConnectionListGroup<'a> {
    host: &'a IpAddr,
    lines: Vec<ConnectionListGroupLine<'a>>,
}

#[allow(dead_code)]
impl<'a> ConnectionListGroup<'a> {
    fn render(&self) -> String {
        self.host.to_string()
    }

    fn print_group_members(&self, bounds: &Rect) -> Vec<String> {
        let max_length = bounds.height().min(self.lines.len());
        self.lines
            .iter()
            .take(self.lines.len().min(max_length))
            .map(|line| {
                if let Some(content) = &line.response.http_response.content {
                    let max_length = bounds.width().min(content.len());
                    let mut buf = String::new();
                    buf.push_str(&content.trim()[0..max_length]);
                    buf
                } else {
                    "No response".to_string()
                }
            })
            .collect()
    }
}

#[allow(dead_code)]
#[derive(Debug)]
struct ConnectionListGroupLine<'a> {
    response: &'a Response,
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
                self.selected = cl.selected;
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
    exit: &mut Option<String>,
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
                        *exit = Some("Pressed ctrl+q".to_string());
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
pub struct Response {
    addr: SocketAddr,
    http_response: testsuite::Response,
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
                    http_response: bl {
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
                        http_response: bl {
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
                        http_response: bl {
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
                            http_response: bl {
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
