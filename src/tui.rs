use crate::tui::Response as TuiResponse;
use std::io::Write;
use crate::{ConnectionData, Connections};
use crossterm::event;
use crossterm::QueueableCommand;
use std::io::Stdout;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::net::SocketAddr;
use std::sync::Arc;
use testsuite::Response as bl;
use testsuite::ResponseFormat;
use tokio::sync::Mutex;

use testsuite::Message;
#[derive(Copy, Clone, Debug)]
pub enum CliEvent<I> {
    Input(I),
    Tick,
}

pub fn parse_input(input: KeyEvent) -> anyhow::Result<()>{
    let (letter, modifier) = (input.code, input.modifiers);
    let res = match (letter, modifier) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            return Err(anyhow::format_err!("Pressed ctrl q"));
        },
        _ => {
            println!("Got letter: {input:?}");
            return Ok(())
        }
    };
}

enum Screen {
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
pub struct Response {
    addr: SocketAddr,
    http_response: testsuite::Response,
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

pub struct Tui {
    connections: Arc<Mutex<Connections>>,
    stdout: Arc<Mutex<Stdout>>,
}
impl Tui {
    pub async fn render(&mut self) {
        let a = &self.connections.lock().await;
        let out = &mut self.stdout.lock().await;
        let _r = out.queue(crossterm::terminal::Clear(crossterm::terminal::ClearType::All));
        let _r = out.queue(crossterm::cursor::MoveTo(0,0));
        println!("{:?}", a.values().len());
    }
    pub fn default(connections: Arc<Mutex<Connections>>, stdout: Arc<Mutex<Stdout>>) -> Tui {
        Tui { connections , stdout}
    }
}
