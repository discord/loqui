use crate::connection_handler::{ConnectionHandler, InternalEvent};
use crate::waiter::ResponseWaiter;
use crate::Config;
use failure::Error;
use loqui_connection::{Encoder, Supervisor as SupervisedConnection};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::await;

#[derive(Clone)]
pub struct Client<E: Encoder> {
    connection: Arc<SupervisedConnection<ConnectionHandler<E>>>,
    config: Arc<Config<E>>,
}

impl<E: Encoder> Client<E> {
    pub async fn connect(address: SocketAddr, config: Config<E>) -> Result<Client<E>, Error> {
        let config = Arc::new(config);
        let handler_config = config.clone();
        let handler_creator = move || ConnectionHandler::new(handler_config.clone());
        let connection = await!(SupervisedConnection::connect(address, handler_creator))?;
        let client = Self {
            connection: Arc::new(connection),
            config,
        };
        Ok(client)
    }

    /// Send a request to the server.
    pub async fn request(&self, payload: E::Encoded) -> Result<E::Decoded, Error> {
        let (waiter, awaitable) = ResponseWaiter::new(self.config.request_timeout);
        let request = InternalEvent::Request { payload, waiter };
        self.connection.send(request)?;
        await!(awaitable)
    }

    /// Send a push to the server.
    pub async fn push(&self, payload: E::Encoded) -> Result<(), Error> {
        let push = InternalEvent::Push { payload };
        self.connection.send(push)
    }

    pub fn close(&self) -> Result<(), Error> {
        self.connection.close()
    }
}
