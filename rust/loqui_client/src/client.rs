use crate::connection_handler::{ClientConnectionHandler, InternalEvent};
use crate::Config;
use failure::Error;
use futures::sync::oneshot;
use loqui_connection::{Encoder, LoquiError, Supervisor as SupervisedConnection};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::await;

#[derive(Clone)]
pub struct Client<E: Encoder> {
    connection: Arc<SupervisedConnection<ClientConnectionHandler<E>>>,
}

impl<E: Encoder> Client<E> {
    pub async fn connect<A: AsRef<str>>(
        address_str: A,
        config: Config<E>,
    ) -> Result<Client<E>, Error> {
        let address: SocketAddr = address_str.as_ref().parse()?;
        let config = Arc::new(config);
        let connection_handler_creator = move || ClientConnectionHandler::new(config.clone());
        let connection = await!(SupervisedConnection::spawn(
            address,
            connection_handler_creator
        ));
        let client = Self {
            connection: Arc::new(connection),
        };
        Ok(client)
    }

    /// Send a request to the server.
    pub async fn request(&self, payload: E::Encoded) -> Result<E::Decoded, Error> {
        let (waiter_tx, waiter_rx) = oneshot::channel();
        let request = InternalEvent::Request { payload, waiter_tx };
        self.connection.event(request)?;
        match await!(waiter_rx) {
            Ok(result) => result,
            Err(oneshot::Canceled) => Err(LoquiError::ConnectionClosed.into()),
        }
    }

    /// Send a push to the server.
    pub async fn push(&self, payload: E::Encoded) -> Result<(), Error> {
        let push = InternalEvent::Push { payload };
        self.connection.event(push)
    }
}
