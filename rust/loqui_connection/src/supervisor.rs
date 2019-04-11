use crate::async_backoff::AsyncBackoff;
use crate::connection::Connection;
use crate::error::{convert_timeout_error, LoquiError};
use crate::handler::Handler;
use failure::Error;
use futures::sync::mpsc::{self, Sender, UnboundedSender};
use futures::sync::oneshot;
use futures_timer::FutureExt;
use std::net::SocketAddr;
use std::time::Instant;
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;

/// A connection supervisor. It will indefinitely keep the connection alive. Supports backoff.
pub struct Supervisor<H: Handler> {
    event_sender: Sender<H::InternalEvent>,
    /// A Sender that will drop once the client is dropped. If it's dropped then we should
    /// stop trying to connect.
    _close_sender: UnboundedSender<()>,
}

impl<H: Handler> Supervisor<H> {
    /// Spawns a new supervisor. Waits until the connection is ready before returning.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to connect to
    /// * `handler_creator` - a `Fn` that creates a `Handler`. Called each time a new TCP connection is made.
    /// * `queue_size` - the number of requests the supervisor should hold before dropping requests
    pub async fn connect<F>(
        address: SocketAddr,
        handler_creator: F,
        queue_size: usize,
    ) -> Result<Self, Error>
    where
        F: Fn() -> H + Send + Sync + 'static,
    {
        let (event_sender, mut self_rx) = mpsc::channel(queue_size);
        let (_close_sender, mut close_rx) = mpsc::unbounded::<()>();
        let connection = Self {
            event_sender,
            _close_sender,
        };
        let (ready_tx, ready_rx) = oneshot::channel();
        tokio::spawn_async(
            async move {
                let mut backoff = AsyncBackoff::new();
                // Make it an option so we only send once.
                let mut ready_tx = Some(ready_tx);
                loop {
                    // Poll the close receiver before connecting to make sure the client hasn't
                    // hung up.
                    if let Ok(Async::Ready(_)) = close_rx.poll() {
                        trace!("Client dropped while connecting.");
                        return;
                    }

                    let handler = handler_creator();
                    debug!("Connecting to {}", address);

                    match await!(TcpStream::connect(&address)) {
                        Ok(tcp_stream) => {
                            info!("Connected to {}", address);

                            let (connection_ready_tx, connection_ready_rx) = oneshot::channel();
                            let connection =
                                Connection::spawn(tcp_stream, handler, Some(connection_ready_tx));

                            // Wait for the connection to upgrade and handshake.
                            if let Err(_e) = await!(connection_ready_rx) {
                                // Connection dropped the sender.
                                debug!("Ready failed.");
                                await!(backoff.snooze());
                                continue;
                            }

                            backoff.reset();
                            if let Some(ready_tx) = ready_tx.take() {
                                if let Err(e) = ready_tx.send(()) {
                                    warn!("No one listening for ready anymore. error={:?}", e);
                                    return;
                                }
                            }

                            loop {
                                match await!(self_rx.next()) {
                                    Some(Ok(internal_event)) => {
                                        if let Err(e) = connection.send(internal_event) {
                                            debug!("Connection no longer running. Will reconnect. error={:?}", e);
                                            await!(backoff.snooze());
                                            break;
                                        }
                                    }
                                    Some(Err(_)) | None => {
                                        debug!("Client hung up. Closing connection.");
                                        let _result = connection.close();
                                        return;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            debug!("Connection closed with error. error={:?}", e);
                            await!(backoff.snooze());
                        }
                    }
                }
            },
        );
        await!(ready_rx)?;
        debug!("Supervisor ready.");
        Ok(connection)
    }

    pub async fn send(&self, event: H::InternalEvent, deadline: Instant) -> Result<(), Error> {
        let send_with_deadline = self
            .event_sender
            .clone()
            .send(event)
            .map(|_sender| ())
            .map_err(|_closed| Error::from(LoquiError::ConnectionClosed))
            .timeout_at(deadline)
            .map_err(convert_timeout_error);
        await!(send_with_deadline)
    }
}
