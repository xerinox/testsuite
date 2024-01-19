use anyhow::Result;
use futures::FutureExt;
use crossterm::event;
use crossterm::event::{EventStream, Event as CtEvent};

use std::time::Duration;
use std::{collections::HashMap, net::SocketAddr};
use tokio::sync::{mpsc::channel, Mutex};
mod tui;
use clap::Parser;
use colored::Colorize;
use std::io::IsTerminal;
use testsuite::{populate_map, Arguments, Message, Response};
use tui::CliEvent;
use tui::{Response as TuiResponse, *};

#[cfg(not(feature = "multithreaded"))]
mod singlethreaded;
#[cfg(not(feature = "multithreaded"))]
use crate::singlethreaded::{handle_connection, SingleBufTcpStream};

#[cfg(feature = "multithreaded")]
mod multithreaded;
#[cfg(feature = "multithreaded")]
use {
    crate::{
        event::Event,
        multithreaded::{handle_connection_async, push_message},
    },
    std::sync::Arc,
};

const REFRESH_RATE: u64 = 1000;

type ConnectionData = Vec<TuiResponse>;
type Connections = HashMap<SocketAddr, ConnectionData>;

#[cfg(feature = "multithreaded")]
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    use std::io::stdout;

    use crossterm::{terminal::{enable_raw_mode, disable_raw_mode}, event::KeyEvent, event::KeyModifiers, event::KeyCode};
    use futures::StreamExt;

    let connections_data: Connections = Connections::new();
    let connections_mutex: Mutex<Connections> = Mutex::new(connections_data);
    let connections_ref: Arc<Mutex<Connections>> = Arc::new(connections_mutex);

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
    let (request_sender, mut request_receiver) = channel::<Message>(100);
    let _server = tokio::spawn(async move {
        let request_sender = request_sender.clone();
        let reference = Arc::clone(&map_ref);
        loop {
            if let Ok((socket, addr)) = listener.accept().await {
                let sender = request_sender.clone();
                if let Err(err) = handle_connection_async(addr, socket, &reference, {
                    let sender = sender.clone();
                    sender
                })
                .await
                {
                    push_message(sender, Message::ConnectionFailed).await;
                    eprintln!(
                        "{} {}",
                        "Error handling connection:".red(),
                        err.to_string().red()
                    );
                }
            }
        }
    });
    type I = crossterm::event::Event;

    let stdout = stdout();
    let out = Arc::from(Mutex::from(stdout));
    let out2 =Arc::clone(&out);
    let mut tui = Tui::default(Arc::clone(&connections_ref),out2);
    let (cli_send, mut cli_recv) = channel::<CliEvent<I>>(30);
    let mut reader = EventStream::new();
    enable_raw_mode()?;


    loop {
        let mut delay = futures_timer::Delay::new(Duration::from_millis(1000)).fuse();
        tokio::select! {
            biased;
            Some(message) = request_receiver.recv() => {
                let connections_ref = Arc::clone(&connections_ref);
                handle_message(message, connections_ref).await;
            },
            Some(Ok(ev)) = reader.next().fuse() => {
                    println!("Event::{:?}\r", ev);
                    cli_send.send(CliEvent::Input(ev)).await?;
            },
            Some(ev) = cli_recv.recv() => {
                match ev {
                    CliEvent::Tick => {
                        tui.render().await;
                    },
                    CliEvent::Input(input) => {
                        match input {
                            CtEvent::Key(key) => {
                                match parse_input(key) {
                                    Ok(_) => {},
                                    Err(_) => {
                                        break;
                                    }
                                }
                            },
                            _ => {}
                        }
                    }
                }
            }
            _ = delay => {
                cli_send.send(CliEvent::Tick).await?;
            }
        }
    }
    disable_raw_mode()?;
    Ok(())
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
