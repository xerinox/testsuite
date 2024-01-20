use anyhow::Result;
use crossterm::QueueableCommand;
use crossterm::event::EventStream;
use futures::FutureExt;
use std::sync::Arc;

use std::time::Duration;
use std::{collections::HashMap, net::SocketAddr};
use tokio::sync::mpsc::channel;
use futures::lock::Mutex;
mod tui;
use clap::Parser;
use colored::Colorize;
use testsuite::{populate_map, Arguments, Message, Response};
use tui::{Response as TuiResponse, *};
use std::io::stdout;

use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use futures::StreamExt;

mod server;
use crate::server::*;

const REFRESH_RATE: u64 = 1000;

pub type ConnectionData = Vec<TuiResponse>;
pub type Connections = HashMap<SocketAddr, ConnectionData>;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let connections_data: Connections = Connections::new();
    let connections_mutex: Mutex<Connections> = Mutex::new(connections_data);
    let connections_ref: Arc<Mutex<Connections>> = Arc::new(connections_mutex);

    let args = Arguments::parse();
    let port = args.port;

    let map = populate_map(&args);
    let map_ref = Arc::from(map.clone());

    let host = match &args.allow_remote {
        true => "0.0.0.0",
        false => "127.0.0.1",
    };
    let end_point: String = host.to_owned() + ":" + &port.to_string();

    let listener = tokio::net::TcpListener::bind(&end_point).await?;
    let (request_sender, mut request_receiver) = channel::<Message>(100);

    let server = tokio::spawn(async move {
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

    let stdout = stdout();
    let out = Arc::from(Mutex::from(stdout));
    #[allow(unused_variables, unused_mut)]
    let (mut w, mut h) = crossterm::terminal::size()?;
    #[warn(unused_mut, unused_variables)]
    let mut reader = EventStream::new();
    enable_raw_mode()?;
    let mut exit = None::<String>;

    let shutdown_reason = loop {
        if exit.is_some() {
            break exit.unwrap();
        }
        let delay = futures_timer::Delay::new(Duration::from_millis(REFRESH_RATE)).fuse();
        let connections= Arc::clone(&connections_ref);
        let out = Arc::clone(&out);
        tokio::select! {
            biased;
            Some(message) = request_receiver.recv() => {
                let connections_ref = Arc::clone(&connections_ref);
                handle_message(message, connections_ref).await;
            },
            Some(Ok(event)) = reader.next().fuse() => {
                let connections_ref= Arc::clone(&connections_ref);
                match parse_cli_event(Some(event), out, connections_ref, &mut w, &mut h, &mut exit).await {
                    Ok(()) => {},
                    Err(_) => {
                    }
                }
            }
            _ = delay => {
               _ =  tick(out, connections).await.map_err(|e| {
                    println!("Could not perform tick, out is locked: {:?}", e)
                });
            }
        }
    };
    out.lock().await.queue(crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;
    out.lock().await.queue(crossterm::cursor::MoveTo(0, 0))?;
    println!("Shutting down server due to: {shutdown_reason}");
    out.lock().await.queue(crossterm::cursor::MoveTo(0, 1))?;
    server.abort();
    disable_raw_mode()?;
    Ok(())
}
