use crate::Response;
use std::net::SocketAddr;
use crate::Message;
use testsuite::ResponseMessage;
use anyhow::Result;
use nanohttp::{Method, Request as HttpRequest, Response as HttpResponse, Status};
use tokio::sync::mpsc;
use std::str::from_utf8;
use std::{collections::HashMap, io, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufWriter},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};

pub struct MultiBufTcpStream {
    pub input: BufWriter<OwnedWriteHalf>,
    pub output: OwnedReadHalf,
}

pub async fn push_message(tx: mpsc::Sender<Message>, message: Message) {
    tx.send(message).await.unwrap();
    tokio::task::yield_now().await;
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

async fn handle(req: HttpRequest, map: &Arc<HashMap<String, Response>>, addr: Result<SocketAddr>, sender: mpsc::Sender<Message>) -> HttpResponse {
    match req.method {
        Method::GET => {
            if let Some(data) = map.get(req.path.uri.as_str()) {
                let format = data.format.to_string();
                if let Some(content) = &data.content {
                    let response = HttpResponse::content(content, &format).status(Status::Ok);
                    push_message(sender, Message::Response(ResponseMessage::new(addr, &response))).await;
                    response
                } else {
                    let response = HttpResponse::empty().status(Status::Ok);
                    push_message(sender, Message::Response(ResponseMessage::new(addr, &response))).await;
                    return response
                }
            } else {
                let response = HttpResponse::empty().status(Status::NotFound);
                push_message(sender, Message::Response(ResponseMessage::new(addr, &response))).await;
                return response
            }
        }
        Method::POST => {
            if let Some(data) = map.get(req.path.uri.as_str()) {
                let format = data.format.to_string();
                let response = HttpResponse::content(&req.body, &format).status(Status::Ok);
                push_message(sender, Message::Response(ResponseMessage::new(addr, &response))).await;
                response
            } else {
                let response = HttpResponse::empty().status(Status::NotFound);
                push_message(sender, Message::Response(ResponseMessage::new(addr, &response))).await;
                response
            }
        }
        _ => HttpResponse::empty().status(Status::NotAllowed),
    }
}

pub async fn handle_connection_async(
    mut streams: MultiBufTcpStream,
    map: &Arc<HashMap<String, Response>>,
    sender: tokio::sync::mpsc::Sender<Message>
) -> Result<()> {
    let mut buffer = [0; 1024];
    streams.output.read(&mut buffer).await?;
    let addr = streams.output.peer_addr().map_err(|e|  anyhow::Error::from(e)) ;
    if let Ok(addr) = addr{
        push_message({let sender = sender.clone(); sender}, Message::ConnectionReceived(Some(addr))).await;
    } else {
        push_message({let sender = sender.clone(); sender} , Message::ConnectionReceived(None)).await
    }

    let req_text = from_utf8(&buffer).unwrap().trim_end_matches("\0");
    let req = HttpRequest::from_string(req_text).unwrap();
    let res = handle(req, map, addr, sender).await.to_string();

    let mut stream = BufWriter::new(&mut streams.input);
    let bytes = res.as_bytes();

    stream.write(bytes).await?;
    stream.flush().await?;
    Ok(())
}
