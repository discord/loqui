use std::net::SocketAddr;

use failure::Error;
use futures::oneshot;
use futures::sync::mpsc;
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;

// TODO: can probably encapsulate the Message in SocketHandler
use self::socket_handler::{Message, SocketHandler};

mod socket_handler;

// TODO: get right values
const UPGRADE_REQUEST: &'static str =
    "GET /_rpc HTTP/1.1\r\nHost: 127.0.0.1 \r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";

#[derive(Debug, Clone)]
pub struct Client {
    sender: mpsc::UnboundedSender<Message>,
}

async fn upgrade(mut socket: TcpStream) -> TcpStream {
    await!(socket.write_all_async(&UPGRADE_REQUEST.as_bytes())).unwrap();
    await!(socket.flush_async()).unwrap();
    let mut payload = [0; 1024];
    // TODO: handle disconnect, bytes_read=0
    while let Ok(_bytes_read) = await!(socket.read_async(&mut payload)) {
        let response = String::from_utf8(payload.to_vec()).unwrap();
        // TODO: case insensitive
        if response.contains(&"Upgrade") {
            break;
        }
    }
    socket
}

impl Client {
    pub async fn connect<A: AsRef<str>>(address: A) -> Result<Client, Error> {
        let addr: SocketAddr = address.as_ref().parse()?;
        let mut socket = await!(TcpStream::connect(&addr))?;
        socket = await!(upgrade(socket));
        let (tx, rx) = mpsc::unbounded::<Message>();
        tokio::spawn_async(SocketHandler::run(socket, rx));
        let client = Self { sender: tx };
        Ok(client)
    }

    pub async fn request(&self, payload: Vec<u8>) -> Result<Vec<u8>, Error> {
        let (sender, receiver) = oneshot();
        self.sender
            .unbounded_send(Message::Request { payload, sender })?;
        // TODO: handle send error better
        await!(receiver).map_err(|e| Error::from(e))?
    }

    pub async fn push(&self, payload: Vec<u8>) -> Result<(), Error> {
        self.sender
            .unbounded_send(Message::Push { payload })
            .map_err(|e| Error::from(e))
    }
}
