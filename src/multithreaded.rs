use crate::Response;
use anyhow::Result;
use std::{collections::HashMap, io, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};

pub struct MultiBufTcpStream {
    pub input: BufWriter<OwnedWriteHalf>,
    pub output: BufReader<OwnedReadHalf>,
}

impl MultiBufTcpStream {
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        let (read, write) = stream.into_split();
        let input = BufWriter::new(write);
        let output = BufReader::new(read);
        Ok(MultiBufTcpStream { input, output })
    }
}

pub async fn handle_connection_async(
    mut streams: MultiBufTcpStream,
    map: &Arc<HashMap<String, Response>>,
) -> Result<()> {
    let mut output = BufReader::new(streams.output);
    let mut line = String::new();
    output.read_line(&mut line).await?;

    let path = line
        .trim_start_matches("GET ")
        .trim_start_matches("POST ")
        .trim_end_matches(" HTTP/1.1\r\n");
    let mut stream = BufWriter::new(&mut streams.input);

    if map.get(path).is_some() {
        if let Some(matched_path) = map.get(path) {
            stream.write(matched_path.to_string().as_bytes()).await?;
            println!(
                "Matched path: {}, responded with: {:?}",
                path,
                matched_path.to_string()
            );
        }
    } else {
        println!("Unmatched path: {}:", path);
        stream
            .write("HTTP/1.1 404 Not Found\r\n\r\nNot found".as_bytes())
            .await?;
    }
    stream.flush().await?;
    Ok(())
}
