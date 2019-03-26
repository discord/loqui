#![feature(await_macro, async_await, futures_api)]

use self::frame_handler::ClientFrameHandler;
use failure::Error;
use futures::oneshot;
use loqui_server::connection::{Connection, ConnectionSender};
use std::net::SocketAddr;
use tokio::await;
use tokio::net::TcpStream;

mod frame_handler;

#[derive(Clone)]
pub struct Client {
    connection_sender: ConnectionSender,
}

impl Client {
    pub async fn connect<A: AsRef<str>>(address: A) -> Result<Client, Error> {
        let addr: SocketAddr = address.as_ref().parse()?;
        let tcp_stream = await!(TcpStream::connect(&addr))?;
        let (connection_sender, connection) = Connection::new(tcp_stream);
        let (ready_tx, ready_rx) = oneshot();
        let client_event_handler = ClientFrameHandler::new(connection_sender.clone(), ready_tx);
        connection.spawn(Box::new(client_event_handler));
        // TODO; set encoding somewhere
        connection_sender.hello()?;
        println!("[loqui_client] Waiting for ready...");
        let _ready_rx = await!(ready_rx).map_err(|e| Error::from(e))?;
        println!("[loqui_client] Ready.");
        let client = Self { connection_sender };
        Ok(client)
    }

    pub async fn request(&self, payload: Vec<u8>) -> Result<Vec<u8>, Error> {
        let waiter_rx = self.connection_sender.request(payload)?;
        await!(waiter_rx).map_err(|e| Error::from(e))?
    }

    pub async fn push(&self, payload: Vec<u8>) -> Result<(), Error> {
        self.connection_sender.push(payload)
    }
}
