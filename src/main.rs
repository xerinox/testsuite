use anyhow::Result;
use std::{collections::HashMap, net::SocketAddr};
use std::{sync::Mutex, time::Duration};
mod tui;
use clap::Parser;
use colored::Colorize;
use std::io::IsTerminal;
use testsuite::{populate_map, Arguments, Message, Response};
use tui::{Response as TuiResponse, *};

#[cfg(not(feature = "multithreaded"))]
mod singlethreaded;
#[cfg(not(feature = "multithreaded"))]
use crate::singlethreaded::{handle_connection, SingleBufTcpStream};

#[cfg(feature = "multithreaded")]
mod multithreaded;
#[cfg(feature = "multithreaded")]
use {
    crate::multithreaded::{handle_connection_async, push_message, MultiBufTcpStream},
    std::sync::Arc,
};

const REFRESH_RATE: u64 = 33;

type ConnectionData = HashMap<usize, TuiResponse>;
type Connections = HashMap<SocketAddr, ConnectionData>;

#[cfg(feature = "multithreaded")]
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let connections_data: ConnectionData = ConnectionData::new();
    let connections_mutex: Mutex<ConnectionData> = Mutex::new(connections_data);
    let connections_ref: Arc<Mutex<ConnectionData>> = Arc::new(connections_mutex);

    use tokio::sync::mpsc;

    if !(std::io::stdin().is_terminal() || std::io::stdin().is_terminal()) {
        panic!("Not a terminal");
    }
    let args = Arguments::parse();
    let port = args.port;

    let map = populate_map(&args);
    let host = match &args.allow_remote {
        true => "0.0.0.0",
        false => "127.0.0.1",
    };

    let map_ref = Arc::from(map.clone());

    let end_point: String = host.to_owned() + ":" + &port.to_string();
    let listener = tokio::net::TcpListener::bind(&end_point).await?;
    eprintln!(
        "{}{}{}{:?}",
        "Server listening on: ".green(),
        end_point.white(),
        " with the following endpoints: ".green(),
        map.keys().collect::<Vec<_>>()
    );
    let (sender, mut receiver) = mpsc::channel::<Message>(100);

    let mut tui = Tui::default(Arc::clone(&connections_ref));

    loop {
        let sender = sender.clone();

        let reference = Arc::clone(&map_ref);
        let polling = tokio::time::sleep(Duration::from_millis(REFRESH_RATE));

        tokio::select! {
            biased;
            Ok((socket, _)) = listener.accept() => {
                if let Ok(streams) = MultiBufTcpStream::new(socket) {
                    if let Err(e) = handle_connection_async(streams, &reference, {let sender = sender.clone(); sender}).await {
                        push_message({let sender = sender.clone(); sender}, Message::ConnectionFailed).await;
                        eprintln!(
                            "{} {}",
                            "Error handling connection:".red(),
                            e.to_string().red()
                        );
                    }
                }
            },
            Some(message) = receiver.recv() => {
                let connections_ref = Arc::clone(&connections_ref);
                handle_message(message, connections_ref);
            },
            _ = polling => {
                // handle tui here
                tui.render();
            }

        }
        /*
        tokio::spawn(async move {
            if let Ok(streams) = MultiBufTcpStream::new(socket) {
            }
        });
        */
    }
}

#[cfg(not(feature = "multithreaded"))]
fn main() -> Result<()> {
    if !(std::io::stdin().is_terminal() || std::io::stdin().is_terminal()) {
        panic!("Not a terminal");
    }
    let args = Arguments::parse();
    let port = args.port;

    let map = populate_map(&args);

    let host = match &args.allow_remote {
        true => "0.0.0.0",
        false => "127.0.0.1",
    };
    let end_point: String = host.to_owned() + ":" + &port.to_string();
    let listener = std::net::TcpListener::bind(&end_point)?;
    eprintln!(
        "{}, {:}{:} {} {:?}",
        "Server listening on:".green(),
        end_point.white(),
        &args.endpoint.white(),
        " with the following endpoints:".green(),
        map.keys().collect::<Vec<_>>()
    );
    for stream in listener.incoming() {
        let stream = stream?;
        let streams = SingleBufTcpStream::new(&stream)?;
        handle_connection(streams, &map)?;
    }
    Ok(())
}
