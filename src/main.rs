use anyhow::Result;
use crossterm::{event::EventStream, execute, QueueableCommand};
use futures::FutureExt;
use std::{net::IpAddr, sync::Arc};
#[macro_use]
extern crate log;
extern crate simplelog;
use futures::{lock::Mutex, StreamExt};
use indexmap::IndexMap;
use simplelog::*;
use std::fs::File;
use std::time::Duration;
use tokio::sync::mpsc::channel;
pub mod tui;
use clap::Parser;
use std::io::stdout;
use testsuite::{populate_map, Arguments, ConnectionFailedError, EndpointContent, Message};
use tui::{TuiResponse, *};

use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

mod server;
use crate::server::*;

const REFRESH_RATE: u64 = 1000;

pub type Connections = IndexMap<IpAddr, Vec<TuiResponse>>;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let connections_ref: Arc<Mutex<Connections>> = Arc::new(Mutex::new(Connections::new())); //connections_mutex

    let args = Arguments::parse();

    CombinedLogger::init(vec![WriteLogger::new(
        args.log.log_filter.clone().into(),
        Config::default(),
        File::create(args.log.log_file.clone()).unwrap(),
    )])
    .unwrap();

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
            match listener.accept().await {
                Ok((socket, addr)) => {
                    if let Err(err) =
                        handle_connection(addr, socket, &reference, request_sender.clone()).await
                    {
                        warn!(
                            "Could not parse request from address: {:}, error:{:}",
                            addr, err
                        );
                        let _ = request_sender
                            .send(Message::ConnectionFailed(ConnectionFailedError::Parsing((
                                addr, err,
                            ))))
                            .await;
                    }
                }
                Err(err) => {
                    warn!("Could not receive connection:{:}", err);
                }
            }
        }
    });

    execute!(stdout(), crossterm::cursor::Hide)?;
    let stdout = stdout();
    let out = Arc::from(Mutex::from(stdout));
    enable_raw_mode()?;
    let mut reader = EventStream::new();
    let mut exit_reason = None::<String>;
    let tuistate = Arc::new(Mutex::new(
        TuiState::new(Arc::clone(&connections_ref)).await,
    ));
    let tui_ref = Arc::clone(&tuistate);

    let message_client = tokio::spawn(async move {
        loop {
            if let Some(message) = request_receiver.recv().await {
                handle_message(message, Arc::clone(&connections_ref)).await;
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
                if let Err(err) = parse_cli_event(None, Arc::clone(&out), Arc::clone(&tuistate), &mut exit_reason).await{
                    warn!("{err:}");
                }
            }
            Some(Ok(event)) = reader.next().fuse() => {
                let tuistate = Arc::clone(&tuistate);
                match parse_cli_event(Some(event), Arc::clone(&out), tuistate, &mut exit_reason).await {
                    Ok(()) => {},
                    Err(err) => {
                        error!("{:}", err)
                    }
                }
            }
        }
    };

    execute!(std::io::stdout(), crossterm::cursor::Show)?;

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
