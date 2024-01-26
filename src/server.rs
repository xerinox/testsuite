use crate::EndpointContent;
use crate::Message;
use anyhow::Result;
use indexmap::IndexMap;
use nanohttp::{Method, Request as HttpRequest, Response as HttpResponse, Status};
use std::net::SocketAddr;
use std::sync::Arc;
use testsuite::ResponseMessage;
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc;
use tokio::{io::AsyncWriteExt, io::BufReader, net::tcp::OwnedReadHalf, net::TcpStream};

pub async fn push_message(tx: mpsc::Sender<Message>, message: Message) {
    tx.send(message).await.unwrap();
}

async fn handle(
    req: HttpRequest,
    map: &Arc<IndexMap<String, EndpointContent>>,
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
                        Message::Response(ResponseMessage::new(
                            addr,
                            &response,
                            Some(Method::GET),
                            Some(Status::Ok),
                        )),
                    )
                    .await;
                    response
                } else {
                    let response = HttpResponse::empty().status(Status::Ok);
                    push_message(
                        sender,
                        Message::Response(ResponseMessage::new(
                            addr,
                            &response,
                            Some(Method::GET),
                            Some(Status::Ok),
                        )),
                    )
                    .await;
                    response
                }
            } else {
                let response = HttpResponse::empty().status(Status::NotFound);
                push_message(
                    sender,
                    Message::Response(ResponseMessage::new(
                        addr,
                        &response,
                        Some(Method::GET),
                        Some(Status::NotFound),
                    )),
                )
                .await;
                response
            }
        }
        Method::POST => {
            if let Some(data) = map.get(req.path.uri.as_str()) {
                let format = data.format.to_string();
                let response = HttpResponse::content(&req.body, &format).status(Status::Ok);
                push_message(
                    sender,
                    Message::Response(ResponseMessage::new(
                        addr,
                        &response,
                        Some(Method::POST),
                        Some(Status::Ok),
                    )),
                )
                .await;
                response
            } else {
                let response = HttpResponse::empty().status(Status::NotFound);
                push_message(
                    sender,
                    Message::Response(ResponseMessage::new(
                        addr,
                        &response,
                        Some(Method::POST),
                        Some(Status::NotFound),
                    )),
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
    stream: TcpStream,
    map: &Arc<IndexMap<String, EndpointContent>>,
    sender: tokio::sync::mpsc::Sender<Message>,
) -> Result<()> {
    push_message(sender.clone(), Message::ConnectionReceived(Some(addr))).await;

    let (mut read_half, mut write_half) = stream.into_split();
    let received = read_stream(&mut read_half).await?;

    let req = HttpRequest::from_string(&received).unwrap();
    let res = handle(req, map, addr, sender).await;
    write_half.write_all(res.to_string().as_bytes()).await?;
    write_half.flush().await.unwrap();
    Ok(())
}

async fn read_stream(stream: &mut OwnedReadHalf) -> anyhow::Result<String> {
    // 8kb internal buffer
    let mut reader = BufReader::new(stream);
    let received: Vec<u8> = reader.fill_buf().await?.to_vec();
    //process?
    //TODO: chunking of data if over 8kb
    reader.consume(received.len());
    Ok(String::from_utf8(received)?)
}
