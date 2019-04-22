use crate::connection_handler::{ConnectionHandler, InternalEvent};
use crate::waiter::ResponseWaiter;
use crate::Config;
use failure::Error;
use futures::future::Future;
use futures::sync::oneshot;
use futures_timer::FutureExt;
use loqui_connection::{convert_timeout_error, Connection, EncoderFactory, LoquiError};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::await;
use tokio::net::TcpStream;

pub struct Client<F: EncoderFactory> {
    connection: Connection<ConnectionHandler<F>>,
    request_timeout: Duration,
}

impl<F: EncoderFactory> Client<F> {
    pub async fn connect(address: SocketAddr, config: Config) -> Result<Client<F>, Error> {
        let deadline = Instant::now() + config.connect_timeout;

        let connect_future = TcpStream::connect(&address).timeout_at(deadline.clone());
        match await!(connect_future) {
            Ok(tcp_stream) => {
                info!("Connected to {}", address);

                let (ready_tx, ready_rx) = oneshot::channel();
                let awaitable = ready_rx
                    .map_err(|_canceled| Error::from(LoquiError::ConnectionClosed))
                    .timeout_at(deadline)
                    .map_err(convert_timeout_error);

                let request_timeout = config.request_timeout;
                let config = Arc::new(config);
                let handler = ConnectionHandler::new(config.clone());
                let connection = Connection::spawn(tcp_stream, handler, Some(ready_tx));
                match await!(awaitable) {
                    Ok(()) => {
                        let client = Self {
                            connection,
                            request_timeout,
                        };
                        Ok(client)
                    }
                    Err(e) => Err(e.into()),
                }
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Send a request to the server.
    pub async fn request(&self, payload: F::Encoded) -> Result<F::Decoded, Error> {
        let (waiter, awaitable) = ResponseWaiter::new(self.request_timeout);
        let request = InternalEvent::Request { payload, waiter };
        self.connection.send(request)?;
        await!(awaitable)
    }

    /// Send a push to the server.
    pub async fn push(&self, payload: F::Encoded) -> Result<(), Error> {
        let push = InternalEvent::Push { payload };
        self.connection.send(push)
    }

    pub fn is_closed(&self) -> bool {
        self.connection.is_closed()
    }
}
