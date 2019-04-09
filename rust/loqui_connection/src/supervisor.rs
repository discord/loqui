use crate::async_backoff::AsyncBackoff;
use crate::connection::Connection;
use crate::error::LoquiError;
use crate::handler::Handler;
use failure::Error;
use futures::sync::mpsc::{self, UnboundedSender};
use futures::sync::oneshot;
use std::net::SocketAddr;
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;

// TODO: when does it stop attempting? When client object is dropped?

enum Event<H: Handler> {
    Internal(H::InternalEvent),
    Close,
}

/// A connection supervisor. It will indefinitely keep the connection alive. Supports backoff.
pub struct Supervisor<H: Handler> {
    self_sender: UnboundedSender<Event<H>>,
}

impl<H: Handler> Supervisor<H> {
    /// Spawns a new supervisor. Waits until the connection is ready before returning.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to connect to
    /// * `handler_creator` - a `Fn` that creates a `Handler`. Called each time a new TCP connection is made.
    pub async fn connect<F>(address: SocketAddr, handler_creator: F) -> Result<Self, Error>
    where
        F: Fn() -> H + Send + Sync + 'static,
    {
        let (self_sender, mut self_rx) = mpsc::unbounded();
        let connection = Self {
            self_sender: self_sender.clone(),
        };
        let (ready_tx, ready_rx) = oneshot::channel();
        tokio::spawn_async(
            async move {
                let mut backoff = AsyncBackoff::new();
                // Make it an option so we only send once.
                let mut ready_tx = Some(ready_tx);
                loop {
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
                                }
                            }

                            // TODO: handle Some(Err())
                            while let Some(Ok(event)) = await!(self_rx.next()) {
                                match event {
                                    Event::Internal(internal_event) => {
                                        if let Err(e) = connection.send(internal_event) {
                                            debug!("Connection no longer running. error={:?}", e);
                                            await!(backoff.snooze());
                                            break;
                                        }
                                    }
                                    Event::Close => {
                                        debug!("Closing connection.");
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

    pub fn send(&self, event: H::InternalEvent) -> Result<(), Error> {
        self.self_sender
            .unbounded_send(Event::Internal(event))
            .map_err(|_e| LoquiError::ConnectionClosed.into())
    }

    pub fn close(&self) -> Result<(), Error> {
        self.self_sender
            .unbounded_send(Event::Close)
            .map_err(|_e| LoquiError::ConnectionClosed.into())
    }
}
