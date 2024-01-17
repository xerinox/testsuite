use crate::Response;
use anyhow::Result;
use nanohttp::{Method, Request as HttpRequest, Response as HttpResponse, Status};
use std::str::from_utf8;
use std::{collections::HashMap, io, sync::Arc};
use tokio::{
    io::{AsyncWriteExt, AsyncReadExt, BufWriter},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};

pub struct MultiBufTcpStream {
    pub input: BufWriter<OwnedWriteHalf>,
    pub output: OwnedReadHalf,
}

impl MultiBufTcpStream {
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        let (read, write) = stream.into_split();
        let input = BufWriter::new(write);
        Ok(MultiBufTcpStream {
            input,
            output: read,
        })
    }
}

async fn handle(req: HttpRequest, map: &Arc<HashMap<String, Response>>) -> HttpResponse {
    match req.method {
        Method::GET => {
            if let Some(data) = map.get(req.path.uri.as_str()) {
                let format = data.format.to_string();
                if let Some(content) = &data.content {
                    println!("GET {}, response: {:?}", &req.path.uri.as_str(), content);
                    HttpResponse::content(&content, &format).status(Status::Ok)
                } else {
                    println!("GET {}, response empty", &req.path.uri.as_str());
                    HttpResponse::empty().status(Status::Ok)
                }
            } else {
                println!("GET {}, 404", &req.path.uri.as_str());
                HttpResponse::empty().status(Status::NotFound)
            }
        }
        Method::POST => {
            if let Some(data) = map.get(req.path.uri.as_str()) {
                let format = data.format.to_string();
                println!("Post {}, response: {:?}", &req.path.uri.as_str(), &req.body);
                HttpResponse::content(&req.body, &format).status(Status::Ok) 
            } else {
                println!("Post {}, 404", &req.path.uri.as_str());
                HttpResponse::empty().status(Status::NotFound)
            }
        }
        _ => HttpResponse::empty().status(Status::NotAllowed),
    }
}

pub async fn handle_connection_async(
    mut streams: MultiBufTcpStream,
    map: &Arc<HashMap<String, Response>>,
) -> Result<()> {
    let mut buffer = [0; 1024];
    streams.output.read(&mut buffer).await?;
    let req_text = from_utf8(&buffer).unwrap().trim_end_matches("\0");
    let req = HttpRequest::from_string(req_text).unwrap();
    let res = handle(req, map).await.to_string();

    let mut stream = BufWriter::new(&mut streams.input);
    let bytes = res.as_bytes();

    stream.write(bytes).await?;
    stream.flush().await?;
    Ok(())
}
