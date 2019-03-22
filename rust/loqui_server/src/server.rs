use super::connection::{Connection, Event, EventHandler, HandleEventResult};
use super::event_handler::{ServerEvent, ServerEventHandler};
use super::frame_handler::{FrameHandler, ServerFrameHandler};
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
    supported_encodings: Vec<String>,
    frame_handler: Arc<dyn FrameHandler>,
}

impl Server {
    pub fn new(request_handler: Arc<dyn RequestHandler>, supported_encodings: Vec<String>) -> Self {
        let frame_handler = ServerFrameHandler::new(request_handler);
        Self {
            frame_handler: Arc::new(frame_handler),
            supported_encodings,
        }
    }

    fn handle_connection(&self, tcp_stream: TcpStream) {
        let (tx, rx) = mpsc::unbounded::<Event<ServerEvent>>();
        let mut connection = Connection::new(rx, tcp_stream);
        let frame_handler = self.frame_handler.clone();
        tokio::spawn_async(
            async {
                connection = await!(connection.await_upgrade());
                let event_handler = ServerEventHandler::new(tx, frame_handler);
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
