use crate::Message;
use crate::Response;
use anyhow::Result;
use nanohttp::{Method, Request as HttpRequest, Response as HttpResponse, Status};
use std::net::SocketAddr;
use std::str::from_utf8;
use std::{collections::HashMap, sync::Arc};
use testsuite::ResponseMessage;
use tokio::sync::mpsc;
use tokio::{ io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};

pub async fn push_message(tx: mpsc::Sender<Message>, message: Message) {
    tx.send(message).await.unwrap();
}

async fn handle(
    req: HttpRequest,
    map: &Arc<HashMap<String, Response>>,
    addr: SocketAddr,
    sender: mpsc::Sender<Message>,
) -> HttpResponse {
    match req.method {
        Method::GET => {
            if let Some(data) = map.get(req.path.uri.as_str()) {
                let format = data.format.to_string();
                if let Some(content) = &data.content {
                    let response = HttpResponse::content(content, &format).status(Status::Ok);
                    push_message(
                        sender,
                        Message::Response(ResponseMessage::new(addr, &response)),
                    )
                    .await;
                    response
                } else {
                    let response = HttpResponse::empty().status(Status::Ok);
                    push_message(
                        sender,
                        Message::Response(ResponseMessage::new(addr, &response)),
                    )
                    .await;
                    return response;
                }
            } else {
                let response = HttpResponse::empty().status(Status::NotFound);
                push_message(
                    sender,
                    Message::Response(ResponseMessage::new(addr, &response)),
                )
                .await;
                return response;
            }
        }
        Method::POST => {
            if let Some(data) = map.get(req.path.uri.as_str()) {
                let format = data.format.to_string();
                let response = HttpResponse::content(&req.body, &format).status(Status::Ok);
                push_message(
                    sender,
                    Message::Response(ResponseMessage::new(addr, &response)),
                )
                .await;
                response
            } else {
                let response = HttpResponse::empty().status(Status::NotFound);
                push_message(
                    sender,
                    Message::Response(ResponseMessage::new(addr, &response)),
                )
                .await;
                response
            }
        }
        _ => HttpResponse::empty().status(Status::NotAllowed),
    }
}

pub async fn handle_connection(
    addr: SocketAddr,
    mut stream: TcpStream,
    map: &Arc<HashMap<String, Response>>,
    sender: tokio::sync::mpsc::Sender<Message>,
) -> Result<()> {
    push_message({let sender = sender.clone(); sender}, Message::ConnectionReceived(Some(addr))).await;

    let mut buffer = [0; 1024];
    stream.read(&mut buffer).await?;

    let req_text = from_utf8(&buffer).unwrap().trim_end_matches("\0");
    let req = HttpRequest::from_string(req_text).unwrap();
    let res = handle(req, map, addr, {
        let sender = sender.clone();
        sender
    })
    .await;

    stream.write(res.to_string().as_bytes()).await?;
    stream.flush().await.unwrap();
    Ok(())
}
