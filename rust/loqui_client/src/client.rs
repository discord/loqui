use crate::connection_handler::{ConnectionHandler, InternalEvent, Waiter};
use crate::Config;
use failure::Error;
use futures::future::Future;
use futures::sync::oneshot;
use futures_timer::FutureExt;
use loqui_connection::{Encoder, LoquiError, Supervisor as SupervisedConnection};
use std::io;
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
        let (tx, rx) = oneshot::channel();
        let waiter = Waiter::new(tx, self.config.request_timeout);
        let request = InternalEvent::Request { payload, waiter };
        self.connection.send(request)?;
        let receive_future = rx
            .map_err(|oneshot::Canceled| Error::from(LoquiError::ConnectionClosed))
            .timeout(self.config.request_timeout);
        match await!(receive_future) {
            Ok(result) => result,
            Err(error) => {
                let error = error.downcast::<io::Error>()?;
                if error.kind() == io::ErrorKind::TimedOut {
                    Err(LoquiError::RequestTimeout.into())
                } else {
                    Err(error.into())
                }
            }
        }
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
