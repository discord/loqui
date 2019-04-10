use crate::async_backoff::AsyncBackoff;
use crate::connection::Connection;
use crate::error::LoquiError;
use crate::handler::Handler;
use failure::Error;
use futures::sync::mpsc::{self, Sender, UnboundedSender};
use futures::sync::oneshot;
use futures_timer::FutureExt;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
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
    self_sender: Sender<Event<H>>,
    close_sender: UnboundedSender<()>,
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
        let (self_sender, mut self_rx) = mpsc::channel(1);
        let (close_sender, mut close_rx) = mpsc::unbounded::<()>();
        let connection = Self {
            self_sender: self_sender.clone(),
            close_sender,
        };
        let (ready_tx, ready_rx) = oneshot::channel();
        tokio::spawn_async(
            async move {
                let mut backoff = AsyncBackoff::new();
                // Make it an option so we only send once.
                let mut ready_tx = Some(ready_tx);
                loop {
                    debug!("ok");
                    if let Ok(Async::Ready(_)) = close_rx.poll() {
                        debug!("Close requested while connecting.");
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

                            debug!("Connection spawn");
                            // TODO: connect timeout!
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

    pub async fn send(&self, event: H::InternalEvent, timeout: Duration) -> Result<(), Error> {
        let future = self
            .self_sender
            .clone()
            .send(Event::Internal(event))
            .map_err(|_closed| Error::from(LoquiError::ConnectionClosed))
            .timeout(timeout);
        match await!(future) {
            Ok(_sender) => Ok(()),
            Err(error) => match error.downcast::<io::Error>() {
                Ok(error) => {
                    // Change the timeout error back into one we like.
                    if error.kind() == io::ErrorKind::TimedOut {
                        Err(LoquiError::RequestTimeout.into())
                    } else {
                        Err(error.into())
                    }
                }
                Err(error) => Err(error.into()),
            },
        }
    }

    pub async fn close(&self) -> Result<(), Error> {
        debug!("Closing");
        self.close_sender
            .unbounded_send(())
            .map_err(|_| Error::from(LoquiError::ConnectionClosed))?;
        debug!("CLosed");

        match await!(self.self_sender.clone().send(Event::Close)) {
            Ok(_sender) => Ok(()),
            Err(_e) => Err(LoquiError::ConnectionClosed.into()),
        }
    }
}
