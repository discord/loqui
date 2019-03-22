use failure::Error;
use std::net::SocketAddr;

use tokio::await as tokio_await;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

use std::sync::Arc;

use super::connection::{Connection, Event, EventHandler, HandleEventResult};
use super::frame_handler::{FrameHandler, ServerFrameHandler};
use super::request_handler::RequestHandler;
use loqui_protocol::codec::LoquiFrame;
use futures::sync::mpsc::UnboundedSender;

pub struct Server {
    supported_encodings: Vec<String>,
    event_handler: Arc<dyn EventHandler<InternalEvent>>,
}

#[derive(Debug)]
enum InternalEvent {
    Complete(LoquiFrame),
}

struct ServerEventHandler {
    frame_handler: Arc<dyn FrameHandler>,
}

impl ServerEventHandler {
    fn new(frame_handler: Arc<dyn FrameHandler>) -> Self {
        Self { frame_handler }
    }
}

impl EventHandler<InternalEvent> for ServerEventHandler {
    fn handle_event(&self, event: Event<InternalEvent>, tx: UnboundedSender<Event<InternalEvent>>) -> HandleEventResult {
        let frame_handler = self.frame_handler.clone();
        let tx = tx.clone();
        Box::new(
            async move {
                match event {
                    Event::Socket(frame) => {
                        tokio::spawn_async(
                            async move {
                                // TODO: handle error
                                match await!(Box::into_pin(frame_handler.handle_frame(frame))) {
                                    Ok(Some(frame)) => {
                                        tokio_await!(tx
                                            .send(Event::Internal(InternalEvent::Complete(frame))));
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
                    Event::Internal(InternalEvent::Complete(frame)) => Ok(Some(frame)),
                }
            },
        )
    }
}

impl Server {
    pub fn new(request_handler: Arc<dyn RequestHandler>, supported_encodings: Vec<String>) -> Self {
        let frame_handler = ServerFrameHandler::new(request_handler);
        let event_handler = ServerEventHandler::new(Arc::new(frame_handler));
        Self {
            event_handler: Arc::new(event_handler),
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
        let connection = Connection::new(tcp_stream, self.event_handler.clone());
        tokio::spawn_async(connection.run());
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
