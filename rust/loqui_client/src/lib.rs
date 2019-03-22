#![feature(await_macro, async_await, futures_api)]

use failure::Error;
use futures::oneshot;
use futures::sync::mpsc;
use futures::sync::mpsc::UnboundedSender;
use futures::sync::oneshot::Sender as OneShotSender;
use loqui_protocol::codec::LoquiFrame;
use loqui_protocol::frames::{Push, Request, Response};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;
// TODO: can probably encapsulate the Message in SocketHandler
use self::socket_handler::{Message, SocketHandler};
use loqui_server::connection::{Connection, Event, EventHandler, HandleEventResult};

mod socket_handler;

// TODO: get right values
const UPGRADE_REQUEST: &'static str =
    "GET /_rpc HTTP/1.1\r\nHost: 127.0.0.1 \r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";

#[derive(Clone)]
pub struct Client {
    sender: mpsc::UnboundedSender<Event<ClientEvent>>,
}

#[derive(Debug)]
enum ClientEvent {
    Request {
        payload: Vec<u8>,
        // TODO: probably need to handle error better?
        sender: OneShotSender<Result<Vec<u8>, Error>>,
    },
    Push {
        payload: Vec<u8>,
    },
}

struct ClientEventHandler {
    waiters: HashMap<u32, OneShotSender<Result<Vec<u8>, Error>>>,
    next_seq: u32,
}

impl ClientEventHandler {
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
}

impl EventHandler<ClientEvent> for ClientEventHandler {
    fn handle_event(&mut self, event: Event<ClientEvent>) -> HandleEventResult {
        match event {
            Event::Internal(ClientEvent::Request { payload, sender }) => {
                let sequence_id = self.next_seq();
                self.waiters.insert(sequence_id, sender);
                Ok(Some(LoquiFrame::Request(Request {
                    sequence_id,
                    flags: 0,
                    payload,
                })))
            }
            Event::Internal(ClientEvent::Push { payload }) => {
                Ok(Some(LoquiFrame::Push(Push { flags: 0, payload })))
            }
            Event::Socket(frame) => {
                match frame {
                    LoquiFrame::Response(Response {
                        flags,
                        sequence_id,
                        payload,
                    }) => {
                        let sender = self.waiters.remove(&sequence_id).unwrap();
                        sender.send(Ok(payload)).unwrap();
                    }
                    frame => {
                        // TODO:
                        dbg!(frame);
                    }
                }
                Ok(None)
            }
        }
    }
}

impl Client {
    pub async fn connect<A: AsRef<str>>(address: A) -> Result<Client, Error> {
        let addr: SocketAddr = address.as_ref().parse()?;
        let mut tcp_stream = await!(TcpStream::connect(&addr))?;
        let (tx, rx) = mpsc::unbounded::<Event<ClientEvent>>();
        let mut connection = Connection::new(rx, tcp_stream);
        connection = await!(connection.upgrade());
        tokio::spawn_async(connection.run(Box::new(ClientEventHandler::new())));
        let client = Self { sender: tx };
        Ok(client)
    }

    pub async fn request(&self, payload: Vec<u8>) -> Result<Vec<u8>, Error> {
        let (sender, receiver) = oneshot();
        self.sender
            .unbounded_send(Event::Internal(ClientEvent::Request { payload, sender }))?;
        // TODO: handle send error better
        await!(receiver).map_err(|e| Error::from(e))?
    }

    pub async fn push(&self, payload: Vec<u8>) -> Result<(), Error> {
        self.sender
            .unbounded_send(Event::Internal(ClientEvent::Push { payload }))
            .map_err(|e| Error::from(e))
    }
}
