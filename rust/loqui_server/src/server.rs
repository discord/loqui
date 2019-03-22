use failure::Error;
use std::net::SocketAddr;

use tokio::await as tokio_await;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

use std::sync::Arc;

use super::connection::{Connection, Event, EventHandler, HandleEventResult};
use super::frame_handler::{FrameHandler, ServerFrameHandler};
use super::request_handler::RequestHandler;
use futures::sync::mpsc::{self, UnboundedSender};
use loqui_protocol::codec::LoquiFrame;

pub struct Server {
    supported_encodings: Vec<String>,
    frame_handler: Arc<dyn FrameHandler>,
}

#[derive(Debug)]
enum ServerEvent {
    Complete(LoquiFrame),
}

struct ServerEventHandler {
    frame_handler: Arc<dyn FrameHandler>,
    tx: UnboundedSender<Event<ServerEvent>>,
}

impl ServerEventHandler {
    fn new(tx: UnboundedSender<Event<ServerEvent>>, frame_handler: Arc<dyn FrameHandler>) -> Self {
        Self { tx, frame_handler }
    }
}

impl EventHandler<ServerEvent> for ServerEventHandler {
    fn handle_event(&mut self, event: Event<ServerEvent>) -> HandleEventResult {
        let frame_handler = self.frame_handler.clone();
        let tx = self.tx.clone();
        Box::new(
            async move {
                match event {
                    Event::Socket(frame) => {
                        tokio::spawn_async(
                            async move {
                                // TODO: handle error
                                match await!(Box::into_pin(frame_handler.handle_frame(frame))) {
                                    Ok(Some(frame)) => {
                                        tokio_await!(
                                            tx.send(Event::Internal(ServerEvent::Complete(frame)))
                                        );
                                    }
                                    Ok(None) => {
                                        dbg!("None");
                                    }
                                    Err(e) => {
                                        dbg!(e);
                                    }
                                }
                            },
                        );
                        Ok(None)
                    }
                    Event::Internal(ServerEvent::Complete(frame)) => Ok(Some(frame)),
                }
            },
        )
    }
}

impl Server {
    pub fn new(request_handler: Arc<dyn RequestHandler>, supported_encodings: Vec<String>) -> Self {
        let frame_handler = ServerFrameHandler::new(request_handler);
        Self {
            frame_handler: Arc::new(frame_handler),
            supported_encodings,
        }
    }

    // TODO
    /*
    pub fn new(handler: Handler) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }
    */

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
