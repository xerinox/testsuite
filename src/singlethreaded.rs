use std::{net::TcpStream, collections::HashMap};

use crate::Response;

use std::io::{BufReader, BufWriter, Result, Write, BufRead};
pub struct SingleBufTcpStream {
   pub input: BufWriter<TcpStream>,
    pub output: BufReader<TcpStream>,
}

impl SingleBufTcpStream {
    pub fn new(stream: &TcpStream) -> Result<Self> {
        let input = BufWriter::new(stream.try_clone()?);
        let output = BufReader::new(stream.try_clone()?);
        Ok(SingleBufTcpStream {
            input,
            output,
        })

    }
}

pub fn handle_connection(
    mut streams: SingleBufTcpStream,
    map: &HashMap<String, Response>,
) -> Result<()> {
    let buf_reader = BufReader::new(&mut streams.output);
    let http_request: Vec<_> = buf_reader
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| !line.is_empty())
        .collect();

    let string_line = match http_request.first() {
        Some(line) => line,
        None => "",
    };

    let path = string_line
        .trim_start_matches("GET ")
        .trim_start_matches("POST ")
        .trim_end_matches(" HTTP/1.1");

    if map.get(path).is_some() {
        if let Some(matched_path) = map.get(path) {
            streams.input.write(matched_path.to_string().as_bytes())?;
            println!(
                "Matched path: {}, responded with: {:?}",
                path,
                matched_path.to_string()
            );
        }
    } else {
        streams.input.flush()?;
    }
    Ok(())
}
