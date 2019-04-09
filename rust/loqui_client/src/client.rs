use crate::connection_handler::{ConnectionHandler, InternalEvent};
use crate::Config;
use failure::Error;
use futures::sync::oneshot;
use loqui_connection::{Encoder, LoquiError, Supervisor as SupervisedConnection};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::await;

#[derive(Clone)]
pub struct Client<E: Encoder> {
    connection: Arc<SupervisedConnection<ConnectionHandler<E>>>,
}

impl<E: Encoder> Client<E> {
    pub async fn connect<A: AsRef<str>>(
        address_str: A,
        config: Config<E>,
    ) -> Result<Client<E>, Error> {
        let address: SocketAddr = address_str.as_ref().parse()?;
        let config = Arc::new(config);
        let handler_creator = move || ConnectionHandler::new(config.clone());
        let connection = await!(SupervisedConnection::spawn(address, handler_creator));
        let client = Self {
            connection: Arc::new(connection),
        };
        Ok(client)
    }

    /// Send a request to the server.
    pub async fn request(&self, payload: E::Encoded) -> Result<E::Decoded, Error> {
        let (waiter_tx, waiter_rx) = oneshot::channel();
        let request = InternalEvent::Request { payload, waiter_tx };
        self.connection.send(request)?;
        match await!(waiter_rx) {
            Ok(result) => result,
            Err(oneshot::Canceled) => Err(LoquiError::ConnectionClosed.into()),
        }
    }

    /// Send a push to the server.
    pub async fn push(&self, payload: E::Encoded) -> Result<(), Error> {
        let push = InternalEvent::Push { payload };
        self.connection.send(push)
    }

    pub fn close(&self) -> Result<(), Error> {
        self.connection.close(false)
    }
}
