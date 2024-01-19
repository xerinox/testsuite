use std::io::Read;
use colored::Colorize;
use std::{collections::HashMap, net::TcpStream};

use core::str::from_utf8;
use crate::Response;
use nanohttp::{Method, Request as HttpRequest, Response as HttpResponse, Status};

use std::io::{BufWriter, Result, Write};
pub struct SingleBufTcpStream {
    pub input: BufWriter<TcpStream>,
    pub output: TcpStream,
}

impl SingleBufTcpStream {
    pub fn new(stream: &TcpStream) -> Result<Self> {
        let input = BufWriter::new(stream.try_clone()?);
        let output = stream.try_clone()?;
        Ok(SingleBufTcpStream { input, output })
    }
}

fn handle(req: HttpRequest, map: &HashMap<String, Response>) -> HttpResponse {
    let method = req.method;
    match method {
        Method::GET => {
            if let Some(data) = map.get(req.path.uri.as_str()) {
                let format = data.format.to_string();
                match &data.content {
                    Some(data) => {
                        eprintln!("{}", format!("GET {}, response: {:?}", &req.path.uri.as_str(), data).green());
                        HttpResponse::content(&data, &format).status(Status::Ok)
                    },
                    None => {
                        eprintln!("{}", format!("GET {} response empty", &req.path.uri.as_str()).green());
                        HttpResponse::empty().status(Status::Ok)
                    }
                }
            } else {
                eprintln!("{}", format!("GET {} 404", &req.path.uri.as_str()).yellow());
                HttpResponse::empty().status(Status::NotFound)
            }
        }

        Method::POST => {
            if let Some(data) = map.get(req.path.uri.as_str()) {
                let format = data.format.to_string();
                eprintln!("{}", format!("Post {}, response: {:?}", &req.path.uri.as_str(), &req.body).green());
                HttpResponse::content(&req.body, &format).status(Status::Ok) 
            } else {
                eprintln!("{}", format!("Post {}, 404", &req.path.uri.as_str()).yellow());
                HttpResponse::empty().status(Status::NotFound)
            }
        }
        _ => HttpResponse::empty().status(Status::NotAllowed)
    }
}

pub fn handle_connection(
    mut streams: SingleBufTcpStream,
    map: &HashMap<String, Response>,
) -> Result<()> {
    let mut buffer = [0; 1024];
    streams.output.read(&mut buffer)?;
    let req_text = from_utf8(&buffer).unwrap().trim_end_matches("\0");
    let req = HttpRequest::from_string(&req_text).unwrap();
    let res = handle(req, map).to_string();
    let bytes = res.as_bytes();

    streams.input.write(bytes)?;
    streams.input.flush()?;
    Ok(())
}
