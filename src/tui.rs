use crate::tui::Response as TuiResponse;
use std::collections::HashMap;
use crate::Connections;
use crossterm;
use crossterm::event::Event;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::QueueableCommand;
use futures::lock::Mutex;
use std::io::Stdout;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use testsuite::Response as bl;
use testsuite::ResponseFormat;
use anyhow::Result;

use testsuite::Message;

pub async fn parse_cli_event(
    event: Option<crossterm::event::Event>,
    out: Arc<Mutex<Stdout>>,
    connections: Arc<Mutex<Connections>>,
    w: &mut u16,
    h: &mut u16,
    exit: &mut Option<String>
) -> anyhow::Result<()> {
    let mut out = out.lock().await;
    let cw: usize = w.abs_diff(0).into();
    let mut bar = "-".repeat(cw);
    if let Some(event) = event {
        match event {
            Event::Key(key) => {
                let (letter, modifier) = (key.code, key.modifiers);
                let _res = match (letter, modifier) {
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        *exit = Some("Pressed ctrl+q".to_string());
                    }
                    _ => {
                        out.write(format!("Got letter: {key:?}").as_bytes())?;
                    }
                };
            }
            Event::FocusGained => {},
            Event::FocusLost => {},
            Event::Mouse(_) => {},
            Event::Paste(_) => {},
            Event::Resize(nw, nh) => {
                *w = nw.into();
                *h = nh.into();
            }
        }
    } else {
        out.queue(crossterm::terminal::Clear(
            crossterm::terminal::ClearType::All,
        ))?;
        out.queue(crossterm::cursor::MoveTo(0, 0))?;
        let connections = connections.lock().await;
        let len = connections.len().to_string();
        out.write(len.as_bytes())?;
    }
    Ok(())

}

#[allow(dead_code)]
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

pub async fn tick(out: Arc<Mutex<Stdout>>, connections: Arc<Mutex<Connections>>) -> Result<()> {
    let out = &mut out.lock().await;
    out.queue(crossterm::terminal::Clear(
        crossterm::terminal::ClearType::All,
    ))?;
    out.queue(crossterm::cursor::MoveTo(0, 0))?;
    out.write(
        format!(
            "antall connections: {:?}",
            connections.lock().await.len()
        )
        .as_bytes(),
    )?;
    out.flush()?;
    Ok(())
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
