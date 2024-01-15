use std::io::BufReader;
use std::io::prelude::*;
use clap::Parser;
use std::net::TcpListener;
use std::net::TcpStream;
use testsuite::{Response, Arguments};


fn main() {
    let args = Arguments::parse();
    let port = args.port;
    let response = Response::from_args(&args);

    const HOST: &str = "127.0.0.1";

    let end_point: String = HOST.to_owned() + ":" + &port.to_string();
    println!("Server running on:{:}{:}", end_point, response.endpoint);
    let listener = TcpListener::bind(end_point).unwrap();
    for stream in listener.incoming() {
        let _stream = stream.unwrap();
        handle_connection(_stream, &response);

        println!("Connection established!");
    }
}

fn handle_connection(mut stream: TcpStream, response: &Response) {
    let mut line = vec![];
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    if reader.read_until(b'\n', &mut line).unwrap() > 0 {
        let string_line= String::from_utf8(line).expect("error in stream read");
        let path = string_line.trim_start_matches("GET ").trim_start_matches("POST ").trim_end_matches(" HTTP/1.1\r\n");
        if path == response.endpoint {
            stream.write(response.to_string().as_bytes()).unwrap();
            println!("Matched path: {}, responded with: {:?}", path, response.to_string());
        } else {
            println!("Unmatched path: {}, expected: {}", path, response.endpoint);
        }
    }
    stream.flush().unwrap();
}
