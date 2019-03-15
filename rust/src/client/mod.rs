use std::collections::HashMap;
use std::net::SocketAddr;

use failure::{err_msg, Error};
use futures::oneshot;
use futures::stream::SplitSink;
use futures::sync::mpsc;
use futures::sync::oneshot::Sender as OneShotSender;
use std::future::Future as StdFuture;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Poll as StdPoll, Waker};
use tokio::await;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use tokio_codec::Framed;
use tokio_io::io::WriteHalf;

use crate::protocol::codec::LoquiCodec;
use crate::protocol::codec::LoquiFrame;
use crate::protocol::frames::*;
use futures::sync::mpsc::UnboundedReceiver;

const UPGRADE_REQUEST: &'static str =
    "GET #{loqui_path} HTTP/1.1\r\nHost: #{host}\r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";

pub struct Client {
    sender: mpsc::UnboundedSender<Message>,
}

#[derive(Debug)]
enum Message {
    Request {
        data: Vec<u8>,
        sender: OneShotSender<String>,
    },
    Push {
        data: Vec<u8>,
    },
    Response(LoquiFrame),
}

struct SocketHandler {}

impl SocketHandler {
    async fn handle(socket: TcpStream, rx: UnboundedReceiver<Message>) {
        //  TODO: also, don't allow requests until ready?
        // await!(client.upgrade())?;

        let framed_socket = Framed::new(socket, LoquiCodec::new(50000));
        let (mut writer, mut reader) = framed_socket.split();
        // TODO: should probably sweep these, probably request timeout
        let mut waiters: HashMap<u32, OneShotSender<String>> = HashMap::new();
        let mut next_seq = 1;
        let mut stream = reader
            .map(|frame| Message::Response(frame))
            .select(rx.map_err(|()| err_msg("rx error")));

        while let Some(item) = await!(stream.next()) {
            match item {
                Ok(Message::Request { data, sender }) => {
                    let seq = next_seq;
                    next_seq += 1;
                    waiters.insert(seq, sender);
                    writer = await!(writer.send(LoquiFrame::Ping(crate::protocol::frames::Ping {
                        flags: 2,
                        sequence_id: seq
                    })))
                    .unwrap();
                }
                Ok(Message::Push {data}) => {
                    println!("push {:?}", data);
                }
                Ok(Message::Response(frame)) => {
                    dbg!(&frame);
                    if let LoquiFrame::Ping(ping) = frame {
                        let sender = waiters.remove(&ping.sequence_id).unwrap();
                        sender.send("ping back".to_string());
                    }
                }
                Err(e) => {
                    dbg!(e);
                }
            }
        }
        println!("client exit");
    }
}

impl Client {
    pub async fn connect<A: AsRef<str>>(address: A) -> Result<Client, Error> {
        let addr: SocketAddr = address.as_ref().parse()?;
        let socket = await!(TcpStream::connect(&addr))?;
        let (tx, mut rx) = mpsc::unbounded::<Message>();
        tokio::spawn_async(SocketHandler::handle(socket, rx));
        let mut client = Self { sender: tx };
        Ok(client)
    }

    pub async fn request(&mut self, data: Vec<u8>) -> Result<String, Error> {
        let (sender, receiver) = oneshot();
        self.sender.unbounded_send(Message::Request {
            data,
            sender,
        })?;
        await!(receiver).map_err(|e| Error::from(e))
    }
}
