use std::fs;
use std::io::prelude::*;
use std::path::PathBuf;
use clap::Parser;
use testsuite::{ResponseFormat, Response};
use std::net::TcpStream;
use std::net::TcpListener;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to run on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Response format
    #[arg(short, long, default_value_t, value_enum)]
    format: ResponseFormat,

    /// Response content file
    #[arg(short='C', long, conflicts_with = "content")]
    content_file: Option<PathBuf>,

    /// Response content
    #[arg(short, long, conflicts_with = "content_file")]
    content: Option<String>
}

fn main() {
    let args = Args::parse();
    let port = args.port;
    let format = args.format;
    let content_file = match args.content_file {
        Some(path) => {
            match path.exists() {
                true => {
                   Some(fs::read_to_string(path).expect("File is unreadable"))
                },
                false => {
                    println!("File does not exist: {:?}", path);
                    None
                }
            }
        },
        None => {
            None
        }
    };

    let content = match args.content {
        Some(content) => {
            content
        },
        None => {
            match content_file {
                Some(content) => {
                    content
                },
                None => {
                    String::new()
                }
            }
        }
    };

    const HOST: &str = "127.0.0.1";
    let response = Response {
        content,
        format: format.clone()
    };

    let end_point: String = HOST.to_owned() + ":" + &port.to_string();
    println!("endpoint:{:?}", end_point);
    let listener = TcpListener::bind(end_point).unwrap();
    for stream in listener.incoming() {
        let _stream = stream.unwrap();
        handle_connection(_stream, &response);

        println!("Connection established!");
    }
}

fn handle_connection(mut stream: TcpStream, response: &Response) {
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).unwrap();
    stream.write(response.to_string().as_bytes()).unwrap();
    println!("Response: {}", response.to_string());
    stream.flush().unwrap();
}
