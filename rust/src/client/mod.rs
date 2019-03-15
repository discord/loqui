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

#[derive(Debug, Clone)]
pub struct Client {
    sender: mpsc::UnboundedSender<Message>,
}

#[derive(Debug)]
enum Message {
    Request {
        payload: Vec<u8>,
        // TODO: probably need to handle error better?
        sender: OneShotSender<Result<Vec<u8>, Error>>,
    },
    Push {
        payload: Vec<u8>,
    },
    Response(LoquiFrame),
}

struct MessageHandler {
    waiters: HashMap<u32, OneShotSender<Result<Vec<u8>, Error>>>,
    next_seq: u32,
}

impl MessageHandler {
    fn new() -> Self {
        Self {
            // TODO: should probably sweep these, probably request timeout
            waiters: HashMap::new(),
            next_seq: 1,
        }
    }

    fn next_seq(&mut self) -> u32 {
        let seq = self.next_seq;
        self.next_seq += 1;
        seq
    }

    fn handle(&mut self, message: Message) -> Option<LoquiFrame> {
        match message {
            Message::Request { payload, sender } => {
                let sequence_id = self.next_seq();
                self.waiters.insert(sequence_id, sender);
                Some(LoquiFrame::Request(Request {
                    sequence_id,
                    flags: 2,
                    payload,
                }))
            }
            Message::Push { payload } => {
                println!("push {:?}", payload);
                None
            }
            Message::Response(frame) => {
                match frame {
                    LoquiFrame::Response(Response {
                        flags,
                        sequence_id,
                        payload,
                    }) => {
                        let sender = self.waiters.remove(&sequence_id).unwrap();
                        sender.send(Ok(payload));
                    }
                    frame => {
                        dbg!(frame);
                    }
                }
                None
            }
        }
    }
}

struct SocketHandler {}

impl SocketHandler {
    async fn handle(socket: TcpStream, rx: UnboundedReceiver<Message>) {
        //  TODO: also, don't allow requests until ready?
        // await!(client.upgrade())?;

        let framed_socket = Framed::new(socket, LoquiCodec::new(50000));
        let (mut writer, mut reader) = framed_socket.split();
        let mut message_handler = MessageHandler::new();

        let mut stream = reader
            .map(|frame| Message::Response(frame))
            .select(rx.map_err(|()| err_msg("rx error")));

        while let Some(message) = await!(stream.next()) {
            match message {
                Ok(message) => {
                    if let Some(frame) = message_handler.handle(message) {
                        writer = await!(writer.send(frame)).unwrap();
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

    pub async fn request(&self, payload: Vec<u8>) -> Result<Vec<u8>, Error> {
        let (sender, receiver) = oneshot();
        self.sender
            .unbounded_send(Message::Request { payload, sender })?;
        // TODO: handle send error better
        await!(receiver).map_err(|e| Error::from(e))?
    }
}
