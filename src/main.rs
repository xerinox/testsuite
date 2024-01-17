use anyhow::Result;
use clap::Parser;
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
    let args = Arguments::parse();
    let port = args.port;

    let map = populate_map(&args);

    const HOST: &str = "127.0.0.1";

    let map_ref = Arc::from(map.clone());

    let end_point: String = HOST.to_owned() + ":" + &port.to_string();
    #[cfg(feature = "multithreaded")]
    {
        let listener = tokio::net::TcpListener::bind(&end_point).await?;
        println!(
            "Server running on:{:} with the following endpoints: {:?}",
            end_point,
            map.keys().collect::<Vec<_>>()
        );
        loop {
            let (socket, _) = listener.accept().await?;

            let reference = Arc::clone(&map_ref);
            tokio::spawn(async move {
                if let Ok(streams) = MultiBufTcpStream::new(socket) {
                    if let Err(e) = handle_connection_async(streams, &reference).await {
                        println!("Error handling connection: {:?}", e);
                    }
                }
            });
        }
    }
}

#[cfg(not(feature = "multithreaded"))]
fn main() -> Result<()> {
    let args = Arguments::parse();
    let port = args.port;

    let map = populate_map(&args);

    const HOST: &str = "127.0.0.1";

    let end_point: String = HOST.to_owned() + ":" + &port.to_string();
    let listener = std::net::TcpListener::bind(&end_point)?;
    println!(
        "Server running on:{:}{:} with the following endpoints: {:?}",
        end_point,
        &args.endpoint,
        map.keys().collect::<Vec<_>>()
    );
    for stream in listener.incoming() {
        let stream = stream?;
        let streams = SingleBufTcpStream::new(&stream)?;
        handle_connection(streams, &map)?;
    }
    Ok(())
}
