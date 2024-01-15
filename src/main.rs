use clap::Parser;
use std::io::prelude::*;
use std::io::BufReader;
use std::net::TcpListener;
use std::net::TcpStream;
#[cfg(feature = "multithreaded")]
use testsuite::ThreadPool;
use testsuite::{Arguments, Response};

fn main() {
    let args = Arguments::parse();
    let port = args.port;
    let response = Response::from_args(&args);

    const HOST: &str = "127.0.0.1";

    let end_point: String = HOST.to_owned() + ":" + &port.to_string();
    println!("Server running on:{:}{:}", end_point, response.endpoint);
    let listener = TcpListener::bind(end_point).unwrap();
    #[cfg(feature = "multithreaded")]
    if args.threads > 1 {
        let pool = ThreadPool::build(args.threads);
        match &pool {
            Ok(pool) => {
                for stream in listener.incoming() {
                    let _stream = stream.unwrap();
                    let resp = response.clone();
                    pool.execute(move || {
                        handle_connection(_stream, &resp);
                    });
                }
            }
            Err(_) => {
                println!("Could not initalize pool, running single threaded");
                for stream in listener.incoming() {
                    let _stream = stream.unwrap();
                    handle_connection(_stream, &response);
                }
            }
        }
    } else {
        for stream in listener.incoming() {
            let _stream = stream.unwrap();
            handle_connection(_stream, &response);
        }
    }
    #[cfg(not(feature = "multithreaded"))]
    for stream in listener.incoming() {
        let _stream = stream.unwrap();
        handle_connection(_stream, &response);
    }
}

fn handle_connection(mut stream: TcpStream, response: &Response) {
    let buf_reader = BufReader::new(&mut stream);
    let http_request: Vec<_> = buf_reader
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| !line.is_empty())
        .collect();

    let string_line = http_request.first().unwrap();
    let path = string_line
        .trim_start_matches("GET ")
        .trim_start_matches("POST ")
        .trim_end_matches(" HTTP/1.1");

    if path == response.endpoint {
        stream.write(response.to_string().as_bytes()).unwrap();
        println!(
            "Matched path: {}, responded with: {:?}",
            path,
            response.to_string()
        );
    } else {
        println!("Unmatched path: {}, expected: {}", path, response.endpoint);
        stream
            .write("HTTP/1.1 404 Not Found\r\n\r\nNot found".as_bytes())
            .unwrap();
    }
    stream.flush().unwrap();
}
