use crate::connection_handler::ConnectionHandler;
use crate::{Config, RequestHandler};
use failure::Error;
use loqui_connection::Connection;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::{TcpListener, TcpStream};
use tokio_futures::stream::StreamExt;

pub struct Server<R: RequestHandler> {
    config: Arc<Config<R>>,
}

impl<R: RequestHandler> Server<R> {
    pub fn new(config: Config<R>) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    fn handle_connection(&self, tcp_stream: TcpStream) {
        info!("Accepted connection. {:?}", tcp_stream.peer_addr());
        let connection_handler = ConnectionHandler::new(self.config.clone());
        let handshake_deadline = Instant::now() + self.config.handshake_timeout;
        let _connection =
            Connection::spawn(tcp_stream, connection_handler, handshake_deadline, None);
    }

    pub async fn listen_and_serve(&self, address: SocketAddr) -> Result<(), Error> {
        let listener = TcpListener::bind(&address)?;
        info!("Starting {:?} ...", address);
        let mut incoming = listener.incoming();
        loop {
            match incoming.next().await {
                Some(Ok(tcp_stream)) => {
                    self.handle_connection(tcp_stream);
                }
                other => {
                    println!("incoming.next() failed. {:?}", other);
                }
            }
        }
    }
}
