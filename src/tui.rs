use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use crate::ConnectionData;

use testsuite::Message;
#[derive(Copy, Clone, Debug)]
enum Event<I> {
    Input(I),
    Tick
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

pub struct Response {
    addr: SocketAddr,
    http_response: testsuite::Response,
}

pub fn handle_message(message: Message, connections: Arc<Mutex<ConnectionData>>) {
    todo!();
}

pub struct Tui{
    connections: Arc<Mutex<ConnectionData>>
}
impl Tui {
    pub fn render(&self) {
        todo!();
    }
    pub fn default(connections: Arc<Mutex<ConnectionData>>) -> Tui{
        Tui{
            connections
        }
    }
}
