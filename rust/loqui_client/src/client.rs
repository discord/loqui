use crate::connection_handler::{ConnectionHandler, InternalEvent};
use crate::waiter::ResponseWaiter;
use crate::Config;
use failure::Error;
use loqui_connection::{Encoder, Supervisor as SupervisedConnection};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
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
        let connection = await!(SupervisedConnection::connect(
            address,
            handler_creator,
            config.request_queue_size
        ))?;
        let client = Self {
            connection: Arc::new(connection),
            config,
        };
        Ok(client)
    }

    /// Send a request to the server.
    pub async fn request(&self, payload: E::Encoded) -> Result<E::Decoded, Error> {
        let (waiter, awaitable) = ResponseWaiter::new(self.config.request_timeout);
        let deadline = waiter.deadline;
        let request = InternalEvent::Request { payload, waiter };
        await!(self.connection.send(request, deadline))?;
        await!(awaitable)
    }

    /// Send a push to the server.
    pub async fn push(&self, payload: E::Encoded) -> Result<(), Error> {
        let push = InternalEvent::Push { payload };
        let deadline = Instant::now() + self.config.request_timeout;
        await!(self.connection.send(push, deadline))
    }
}
