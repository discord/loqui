use super::connection::{Connection, Event, EventHandler, HandleEventResult};
use super::event_handler::ServerEventHandler;
use super::request_handler::RequestHandler;
use failure::Error;
use futures::sync::mpsc::{self, UnboundedSender};
use loqui_protocol::codec::LoquiFrame;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::await as tokio_await;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

pub struct Server {
    // TODO: should be arc so we don't clone entire thing per connection
    supported_encodings: Vec<String>,
    request_handler: Arc<dyn RequestHandler>,
}

impl Server {
    pub fn new(request_handler: Arc<dyn RequestHandler>, supported_encodings: Vec<String>) -> Self {
        Self {
            request_handler,
            supported_encodings,
        }
    }

    fn handle_connection(&self, tcp_stream: TcpStream) {
        let (tx, rx) = mpsc::unbounded::<Event>();
        let mut connection = Connection::new(rx, tcp_stream);
        let request_handler = self.request_handler.clone();
        let supported_encodings = self.supported_encodings.clone();
        tokio::spawn_async(
            async {
                let event_handler =
                    ServerEventHandler::new(tx, request_handler, supported_encodings);
                await!(connection.run(Box::new(event_handler)));
            },
        );
    }

    // TODO
    //pub async fn serve<A: AsRef<str>>(&self, address: A) -> Result<(), Error> {
    pub async fn listen_and_serve(&self, address: String) -> Result<(), Error> {
        let addr: SocketAddr = address.parse()?;
        let listener = TcpListener::bind(&addr)?;
        println!("Starting {:?} ...", address);
        let mut incoming = listener.incoming();
        loop {
            match await!(incoming.next()) {
                Some(Ok(tcp_stream)) => {
                    self.handle_connection(tcp_stream);
                }
                other => {
                    println!("incoming.next() return odd result. {:?}", other);
                }
            }
        }
    }
}
