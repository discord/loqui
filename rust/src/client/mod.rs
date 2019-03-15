use std::net::SocketAddr;

use failure::Error;
use futures::oneshot;
use futures::sync::mpsc;
use tokio::await;
use tokio::net::TcpStream;

// TODO: can probably encapsulate the Message in SocketHandler
use self::socket_handler::{Message, SocketHandler};

mod socket_handler;

const UPGRADE_REQUEST: &'static str =
    "GET #{loqui_path} HTTP/1.1\r\nHost: #{host}\r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";

#[derive(Debug, Clone)]
pub struct Client {
    sender: mpsc::UnboundedSender<Message>,
}

impl Client {
    pub async fn connect<A: AsRef<str>>(address: A) -> Result<Client, Error> {
        let addr: SocketAddr = address.as_ref().parse()?;
        let socket = await!(TcpStream::connect(&addr))?;
        let (tx, mut rx) = mpsc::unbounded::<Message>();
        tokio::spawn_async(SocketHandler::run(socket, rx));
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
