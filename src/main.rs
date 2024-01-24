use anyhow::Result;
use crossterm::event::EventStream;
use crossterm::QueueableCommand;
use futures::FutureExt;
use std::net::IpAddr;
use std::sync::Arc;

use futures::lock::Mutex;
use std::time::Duration;
use indexmap::IndexMap;
use tokio::sync::mpsc::channel;
mod tui;
use clap::Parser;
use std::io::stdout;
use testsuite::{populate_map, Arguments, Message, ResponseContent};
use tui::{TuiResponse, *};

use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use futures::StreamExt;

mod server;
use crate::server::*;

const REFRESH_RATE: u64 = 1000;

pub type Connections = IndexMap<IpAddr, Vec<TuiResponse>>;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let connections_ref: Arc<Mutex<Connections>> = Arc::new(Mutex::new(Connections::new())); //connections_mutex

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
                if let Err(err) = handle_connection(addr, socket, &reference, {
                    let sender = sender.clone();
                    sender
                })
                .await
                {
                    push_message(sender, Message::ConnectionFailed).await;
                    eprintln!(
                        "{} {}",
                        "Error handling connection:",
                        err.to_string()
                    );
                }
            }
        }
    });

    let stdout = stdout();
    let out = Arc::from(Mutex::from(stdout));
    enable_raw_mode()?;
    let mut reader = EventStream::new();
    let mut exit_reason = None::<String>;
    let tuistate = Arc::new(Mutex::new(TuiState::new(
        Arc::clone(&connections_ref),
    ).await));
    let tui_ref = Arc::clone(&tuistate);

    let message_client = tokio::spawn(async move {
        loop {
            if let Some(message) = request_receiver.recv().await {
                handle_message(message, {let connections = Arc::clone(&connections_ref); connections}).await;
                Arc::clone(&tui_ref).lock().await.needs_update = true;
            }
        }
    });

    let shutdown_reason = loop {
        let out = Arc::clone(&out);
        if exit_reason.is_some() {
            break exit_reason.unwrap();
        }
        let delay = futures_timer::Delay::new(Duration::from_millis(REFRESH_RATE)).fuse();
        tokio::select! {
            _ = delay => {
                if let Err(err) = parse_cli_event(None, {let out = Arc::clone(&out); out}, {let tuistate = Arc::clone(&tuistate); tuistate}, &mut exit_reason).await{
                    format!("Error in cli tick: {err:?}");
                }
            }
            Some(Ok(event)) = reader.next().fuse() => {
                let tuistate = Arc::clone(&tuistate);
                match parse_cli_event(Some(event), {let out = Arc::clone(&out); out}, tuistate,&mut exit_reason).await {
                    Ok(()) => {},
                    Err(err) => {
                        format!("Error in cli parse: {err:?}");
                    }
                }
            }
        }
    };
    out.lock().await.queue(crossterm::terminal::Clear(
        crossterm::terminal::ClearType::All,
    ))?;
    out.lock().await.queue(crossterm::cursor::MoveTo(0, 0))?;
    println!("Shutting down server due to: {shutdown_reason}");
    out.lock().await.queue(crossterm::cursor::MoveTo(0, 1))?;
    message_client.abort();
    server.abort();
    disable_raw_mode()?;
    Ok(())
}
