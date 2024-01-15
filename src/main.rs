use std::io::prelude::*;
use std::net::TcpStream;
use std::{net::TcpListener, str::FromStr};
//use clap::{Arg, Command};
use clap::Parser;
use serde::Serialize;

#[derive(clap::ValueEnum, Clone, Default, Debug, Serialize)]
enum ResponseFormat {
    #[default]
    Json,
    Html,
}

#[derive(Debug)]
enum ResponseFormatError {
    ParseFailedError(String),
}

impl FromStr for ResponseFormat {
    type Err = ResponseFormatError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "json" => Ok(ResponseFormat::Json),
            "html" => Ok(ResponseFormat::Html),
            _ => Err(ResponseFormatError::ParseFailedError(s.to_owned())),
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to run on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Response format
    #[arg(short, long, default_value_t, value_enum)]
    format: ResponseFormat,
}

#[derive(Default)]
struct Response {
    content: String,
    format: ResponseFormat,
}


impl ToString for Response {
    fn to_string(&self) -> String {
        let mut response = String::from("HTTP/1.1 200 OK\r\n");
        match &self.format {
            &ResponseFormat::Html => {
                response.push_str("Content-Type: text/html; charset=utf-8\r\n");
            }
            &ResponseFormat::Json => {
                response.push_str("Content-Type: application/json\r\n");
            }
        }
        response.push_str("Content-Length: ");
        response.push_str(&self.content.len().to_string());
        response.push_str("\r\n\r\n");
        response.push_str(&self.content);

        response
    }
}

fn main() {
    let args = Args::parse();
    let port = args.port;
    let format = args.format;
    const HOST: &str = "127.0.0.1";

    let end_point: String = HOST.to_owned() + ":" + &port.to_string();
    println!("endpoint:{:?}", end_point);
    let listener = TcpListener::bind(end_point).unwrap();
    for stream in listener.incoming() {
        let _stream = stream.unwrap();
        handle_connection(_stream, &format);

        println!("Connection established!");
    }
}

fn handle_connection(mut stream: TcpStream, format: &ResponseFormat) {
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).unwrap();
    let mut response = Response::default();
    response.format = format.clone();
    stream.write(response.to_string().as_bytes()).unwrap();
    println!("Response: {}", response.to_string());
    stream.flush().unwrap();
}
