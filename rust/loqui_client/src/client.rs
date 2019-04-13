use crate::connection_handler::{ConnectionHandler, InternalEvent};
use crate::waiter::ResponseWaiter;
use crate::Config;
use failure::Error;
use loqui_connection::{EncoderFactory, Supervisor as SupervisedConnection};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::await;

pub struct Client<F: EncoderFactory> {
    connection: Arc<SupervisedConnection<ConnectionHandler<F>>>,
    request_timeout: Duration,
}

// XXX: #[derive(Clone)] requires EncoderFactory to be Clone for some unknown reason.
impl<F: EncoderFactory> Clone for Client<F> {
    fn clone(&self) -> Self {
        Self {
            connection: self.connection.clone(),
            request_timeout: self.request_timeout,
        }
    }
}

impl<F: EncoderFactory> Client<F> {
    pub async fn connect(address: SocketAddr, config: Config) -> Result<Client<F>, Error> {
        let request_timeout = config.request_timeout;
        let request_queue_size = config.request_queue_size;

        let config = Arc::new(config);
        let handler_creator = move || ConnectionHandler::new(config.clone());
        let connection = await!(SupervisedConnection::connect(
            address,
            handler_creator,
            request_queue_size
        ))?;

        let client = Self {
            connection: Arc::new(connection),
            request_timeout,
        };
        Ok(client)
    }

    /// Send a request to the server.
    pub async fn request(&self, payload: F::Encoded) -> Result<F::Decoded, Error> {
        let (waiter, awaitable) = ResponseWaiter::new(self.request_timeout);
        let deadline = waiter.deadline;
        let request = InternalEvent::Request { payload, waiter };
        await!(self.connection.send(request, deadline))?;
        await!(awaitable)
    }

    /// Send a push to the server.
    pub async fn push(&self, payload: F::Encoded) -> Result<(), Error> {
        let push = InternalEvent::Push { payload };
        let deadline = Instant::now() + self.request_timeout;
        await!(self.connection.send(push, deadline))
    }
}
