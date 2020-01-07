use crate::connection_handler::ConnectionHandler;
use crate::{Config, RequestHandler};
use failure::Error;
use loqui_connection::Connection;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::time::Instant;
use tokio::net::{TcpListener, TcpStream};

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
        let mut listener = TcpListener::bind(&address).await?;
        info!("Starting {:?} ...", address);
        loop {
            match listener.accept().await {
                Ok((tcp_stream, _address)) => {
                    self.handle_connection(tcp_stream);
                }
                other => {
                    println!("listener.accept() failed. {:?}", other);
                }
            }
        }
    }
}
