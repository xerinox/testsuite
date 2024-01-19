use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use std::io::IsTerminal;
use testsuite::{populate_map, Arguments, Response};

#[cfg(not(feature = "multithreaded"))]
mod singlethreaded;
#[cfg(not(feature = "multithreaded"))]
use crate::singlethreaded::{handle_connection, SingleBufTcpStream};

#[cfg(feature = "multithreaded")]
mod multithreaded;
#[cfg(feature = "multithreaded")]
use {
    crate::multithreaded::{handle_connection_async, MultiBufTcpStream},
    std::sync::Arc,
};

#[cfg(feature = "multithreaded")]
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
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
    #[cfg(feature = "multithreaded")]
    {
        let listener = tokio::net::TcpListener::bind(&end_point).await?;
        eprintln!(
            "{}{}{}{:?}",
            "Server listening on: ".green(),
            end_point.white(),
            " with the following endpoints: ".green(),
            map.keys().collect::<Vec<_>>()
        );
        loop {
            let (socket, _) = listener.accept().await?;

            let reference = Arc::clone(&map_ref);
            tokio::spawn(async move {
                if let Ok(streams) = MultiBufTcpStream::new(socket) {
                    if let Err(e) = handle_connection_async(streams, &reference).await {
                        eprintln!(
                            "{} {}",
                            "Error handling connection:".red(),
                            e.to_string().red()
                        );
                    }
                }
            });
        }
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
    eprintln!("{}, {:}{:} {} {:?}",
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
