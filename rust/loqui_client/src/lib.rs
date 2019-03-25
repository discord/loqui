#![feature(await_macro, async_await, futures_api)]

use self::event_handler::ClientEventHandler;
use failure::Error;
use futures::oneshot;
use futures::sync::mpsc;
use futures::sync::mpsc::UnboundedSender;
use loqui_protocol::codec::LoquiFrame;
use loqui_protocol::frames::{Push, Request, Response};
use loqui_server::connection::{Connection, Event, EventHandler, Forward, HandleEventResult};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;

mod event_handler;

// TODO: get right values
const UPGRADE_REQUEST: &'static str =
    "GET /_rpc HTTP/1.1\r\nHost: 127.0.0.1 \r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";

#[derive(Clone)]
pub struct Client {
    sender: mpsc::UnboundedSender<Event>,
}

impl Client {
    pub async fn connect<A: AsRef<str>>(address: A) -> Result<Client, Error> {
        let addr: SocketAddr = address.as_ref().parse()?;
        let mut tcp_stream = await!(TcpStream::connect(&addr))?;
        let (tx, rx) = mpsc::unbounded::<Event>();
        let mut connection = Connection::new(rx, tcp_stream);
        connection = await!(connection.upgrade());
        tokio::spawn_async(connection.run(Box::new(ClientEventHandler::new())));
        let client = Self { sender: tx };
        Ok(client)
    }

    pub async fn request(&self, payload: Vec<u8>) -> Result<Vec<u8>, Error> {
        let (waiter_tx, waiter_rx) = oneshot();
        self.sender
            .unbounded_send(Event::Forward(Forward::Request { payload, waiter_tx }))?;
        // TODO: handle send error better
        await!(waiter_rx).map_err(|e| Error::from(e))?
    }

    pub async fn push(&self, payload: Vec<u8>) -> Result<(), Error> {
        self.sender
            .unbounded_send(Event::Forward(Forward::Push { payload }))
            .map_err(|e| Error::from(e))
    }
}
