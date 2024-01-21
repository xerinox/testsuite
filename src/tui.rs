use crate::tui::Response as TuiResponse;
use crate::Connections;
use colored::{ColoredString, Colorize};
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
}

#[derive(Debug)]
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
}

impl<'a> ConnectionList<'a> {
    fn default(groups: &Connections, window_size: Rect) -> ConnectionList {
        let bounds = Rect {
            rows: (window_size.rows.0 + 1, window_size.rows.1 - 1),
            cols: (window_size.cols.0 + 1, window_size.cols.1 - 1),
        };

        let addr_list_length: usize = bounds
            .cols
            .0
            .abs_diff(bounds.cols.1)
            .checked_div(3)
            .unwrap_or(0)
            .max(
                groups
                    .iter()
                    .map(|x| x.0.to_string().len())
                    .max()
                    .unwrap_or(0),
            );

        let (address_list_bounds, details_list_bounds): (Rect, Rect) = (
            Rect::new((1, addr_list_length), (1, bounds.rows.1)),
            Rect::new((addr_list_length + 1, bounds.cols.1), (1, bounds.rows.1)),
        );

        ConnectionList {
            address_list_bounds,
            details_list_bounds,
            window_size,
            selected: (0, 0),
            groups: groups
                .iter()
                .map(|(host, lines)| ConnectionListGroup {
                    host,
                    lines: lines
                        .iter()
                        .map(|response| ConnectionListGroupLine { response })
                        .collect(),
                })
                .collect(),
        }
    }

    async fn render(&self, out: Arc<Mutex<Stdout>>) -> anyhow::Result<()> {
        let mut out = out.lock().await;
        let connection_group_list: Vec<Vec<String>> = self
            .groups
            .iter()
            .map(|x| x.render(&self.address_list_bounds, self.selected.0))
            .collect();

        /*let connection_group_members = match self.groups.get(self.selected.0) {
            Some(connection_group_members) => connection_group_members
                .print_group_members(self.selected.1, &self.details_list_bounds),
            None => {
                let mut a = Vec::<&str>::new();
                a.push("No selected member");
                a
            }
        };*/

        out.queue(Print(format!("connection_groups: {connection_group_list:?}")))?;
        out.queue(crossterm::cursor::MoveToNextLine(1))?;
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Debug)]
struct ConnectionListGroup<'a> {
    host: &'a SocketAddr,
    lines: Vec<ConnectionListGroupLine<'a>>,
}

#[allow(dead_code)]
impl<'a> ConnectionListGroup<'a> {
    fn render(&self, addr_list_bounds: &Rect, selected: usize) -> Vec<String> {
        self
            .lines
            .iter()
            .enumerate()
            .take(addr_list_bounds.cols.1.max(self.lines.len()))
            .map(|(index, line)| {
                line.render(index == selected)
            }).collect()
    }

    fn print_group_members(&self, selected_member: usize, bounds: &Rect) -> Vec<&str> {
        self.lines.iter().enumerate().take(bounds.rows.1.max(self.lines.len() -1 )).map(|(index, line)| {
            if let Some(content) = &line.response.http_response.content {
                let _ = index == selected_member;
                let max_length = bounds.cols.0.abs_diff(bounds.cols.1).max(content.len());
                &content[0..max_length]
            } else {
                "No response"
            }
        }).collect()
    }
}

#[allow(dead_code)]
#[derive(Debug)]
struct ConnectionListGroupLine<'a> {
    response: &'a Response,
}

#[allow(dead_code)]
impl<'a> ConnectionListGroupLine<'a> {
    fn render(&self, _selected: bool) -> String{
        self.response.addr.clone().to_string()
    }
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
            let cl = ConnectionList::default(
                &self.connections_cache,
                Rect::new((0, self.window_size.0), (0, self.window_size.1)),
            );
            if let Err(e) = cl
                .render({
                    let out = Arc::clone(&out);
                    out
                })
                .await
            {
                out.lock().await.queue(Print(format!("ERROR: {e:?}")))?;
            }
        }
        let out = Arc::clone(&out);
        let mut out = out.lock().await;

        out.queue(crossterm::cursor::MoveTo(0, self.window_size.1 - 2))?;
        out.queue(Print("hello"))?;
        out.queue(crossterm::cursor::MoveTo(self.cursor.0, self.cursor.1))?;

        out.flush()?;
        Ok(())
    }
}

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

#[derive(Debug, Clone)]
pub struct Response {
    addr: SocketAddr,
    http_response: testsuite::Response,
}

pub async fn handle_message(message: Message, connections: Arc<Mutex<Connections>>) {
    println!("Handling message");
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
