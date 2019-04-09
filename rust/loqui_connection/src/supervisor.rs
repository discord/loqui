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

/// A connection supervisor. It will indefinitely keep the connection alive. Supports backoff.
pub struct Supervisor<H: Handler> {
    self_sender: UnboundedSender<H::InternalEvent>,
}

impl<H: Handler> Supervisor<H> {
    ///
    /// Spawns a new supervisor.
    ///
    /// # Arguments
    ///
    /// * `address` - The address to connect to
    /// * `handler_creator` - a `Fn` that creates a `Handler`. Called each time a new TCP connection is made.
    pub async fn spawn<F>(address: SocketAddr, handler_creator: F) -> Self
    where
        F: Fn() -> H + Send + Sync + 'static,
    {
        let (sup_sender, mut sup_rx) = mpsc::unbounded();
        let connection = Self {
            self_sender: sup_sender.clone(),
        };
        tokio::spawn_async(
            async move {
                let mut backoff = AsyncBackoff::new();
                loop {
                    let handler = handler_creator();
                    debug!("Connecting to {}", address);

                    match await!(TcpStream::connect(&address)) {
                        Ok(tcp_stream) => {
                            info!("Connected to {}", address);
                            backoff.reset();

                            let (ready_tx, ready_rx) = oneshot::channel();
                            let connection = Connection::spawn(tcp_stream, handler, Some(ready_tx));

                            // Wait for the connection to upgrade and handshake.
                            if let Err(e) = await!(ready_rx) {
                                // Connection dropped the sender.
                                debug!("Ready failed.");
                                await!(backoff.snooze());
                                break;
                            }

                            // TODO: does this exit with the connection task still running? Probably since the connection has a sender to itself!
                            // TODO: handle Some(Err())
                            while let Some(Ok(internal_event)) = await!(sup_rx.next()) {
                                if let Err(e) = connection.send_event(internal_event) {
                                    debug!("Connection no longer running. error={:?}", e);
                                    await!(backoff.snooze());
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            debug!("Connection closed with error. error={:?}", e);
                            await!(backoff.snooze());
                        }
                    }
                }
                debug!("Connection supervisor exiting");
            },
        );
        connection
    }

    pub fn event(&self, event: H::InternalEvent) -> Result<(), Error> {
        self.self_sender
            .unbounded_send(event)
            .map_err(|_e| LoquiError::ConnectionSupervisorDead.into())
    }
}
