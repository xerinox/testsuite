use clap::Parser;
#[cfg(feature="multithreaded")]
mod multithreaded;
#[cfg(feature = "multithreaded")]
use crate::multithreaded::multithreaded::ThreadPool;

use std::collections::HashMap;
use std::io::prelude::*;
use std::io::BufReader;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;
use testsuite::{Arguments, Response};


fn main() {
    let args = Arguments::parse();
    let port = args.port;
    let mut map : HashMap<String, Response> = HashMap::new();
    let (content, content_file, content_folder) = (&args.content.content, &args.content.content_file, &args.content.content_folder); 
    match (content, content_file, content_folder) {
        (Some(content), None, None) => {
            map.insert(args.endpoint.clone(), Response::from_content(&content, &args.format));
        },
        (None, Some(content_file), None) => {
            let endpoint = match content_file.file_stem() {
                Some(path) => {
                    let mut endpoint: String = String::from("/");
                    endpoint.push_str(&path.to_string_lossy());
                    endpoint
                }
                None => String::from("/"),
            };
            map.insert(endpoint, Response::from_args(&args));
        }
        (None, None, Some(content_folder)) => {
            let map_b = Response::from_folder(&content_folder, &args.format).unwrap();
            map.extend(map_b)
        }
        _ => {
            map.insert(String::from("/"), Response::default());
        }
    }

    const HOST: &str = "127.0.0.1";

    let map_ref = Arc::from(map);

    let end_point: String = HOST.to_owned() + ":" + &port.to_string();
    println!("Server running on:{:}{:}", end_point, &args.endpoint);
    let listener = TcpListener::bind(end_point).unwrap();
    #[cfg(feature = "multithreaded")]
    if args.threads > 1 {
        let pool = ThreadPool::build(args.threads);
        match &pool {
            Ok(pool) => {
                for stream in listener.incoming() {
                    let _stream = stream.unwrap();
                    let mapref = Arc::clone(&map_ref);
                    pool.execute(move|| {
                        handle_connection(_stream, &mapref);
                    });
                }
            }
            Err(_) => {
                println!("Could not initalize pool, running single threaded");
                for stream in listener.incoming() {
                    let _stream = stream.unwrap();
                    handle_connection(_stream, &map_ref);
                }
            }
        }
    } else {
        for stream in listener.incoming() {
            let _stream = stream.unwrap();
            handle_connection(_stream, &map_ref);
        }
    }
    #[cfg(not(feature = "multithreaded"))]
    for stream in listener.incoming() {
        let _stream = stream.unwrap();
        handle_connection(_stream, &response);
    }
}

fn handle_connection(mut stream: TcpStream, map: &Arc<HashMap<String, Response>>) {
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

    if map.get(path).is_some() {
        stream.write(&map.get(path).unwrap().to_string().as_bytes()).unwrap();
        println!(
            "Matched path: {}, responded with: {:?}",
            path,
            &map.get(path).unwrap().to_string()
        );
    } else {
        println!("Unmatched path: {}:", path);
        stream
            .write("HTTP/1.1 404 Not Found\r\n\r\nNot found".as_bytes())
            .unwrap();
    }
    stream.flush().unwrap();
}
