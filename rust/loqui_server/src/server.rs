use failure::Error;
use std::net::SocketAddr;

use tokio::await;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

use std::sync::Arc;

use super::connection::Connection;
use super::frame_handler::{FrameHandler, ServerFrameHandler};
use super::request_handler::RequestHandler;
use loqui_protocol::codec::LoquiFrame;

pub struct Server {
    pub supported_encodings: Vec<String>,
    pub frame_handler: Arc<dyn FrameHandler>,
}

impl Server {
    pub fn new(request_handler: Arc<dyn RequestHandler>, supported_encodings: Vec<String>) -> Self {
        Self {
            frame_handler: Arc::new(ServerFrameHandler::new(request_handler)),
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
        let connection = Connection::new(tcp_stream, self.frame_handler.clone());
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
