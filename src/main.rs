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
