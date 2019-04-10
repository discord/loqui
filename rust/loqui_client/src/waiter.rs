use failure::Error;
use futures::future::Future;
use futures::sync::oneshot::{self, Sender};
use futures_timer::FutureExt;
use loqui_connection::LoquiError;
use serde::de::DeserializeOwned;
use std::io;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Waiter<Decoded: DeserializeOwned + Send + Sync> {
    tx: Sender<Result<Decoded, Error>>,
    pub deadline: Instant,
}

impl<Decoded: DeserializeOwned + Send + Sync> Waiter<Decoded> {
    /// Creates a new waiter that wait for the specified timeout. Awaiting on the returned future
    /// will either return the Decoded object, a timeout error, or another error from the server.
    ///
    /// # Arguments
    /// * `timeout` - the `Duration` of time to wait before giving up on the request
    pub fn new(timeout: Duration) -> (Self, impl Future<Item = Decoded, Error = Error>) {
        let (tx, rx) = oneshot::channel::<Result<Decoded, Error>>();

        let awaitable = rx
            .map_err(|_canceled| Error::from(LoquiError::ConnectionClosed))
            .timeout(timeout)
            .map_err(|error| match error.downcast::<io::Error>() {
                Ok(error) => {
                    // Change the timeout error back into one we like.
                    if error.kind() == io::ErrorKind::TimedOut {
                        LoquiError::RequestTimeout.into()
                    } else {
                        error.into()
                    }
                }
                Err(error) => error.into(),
            })
            // Collapses the Result<Result<Decoded, Error>> into a Result<Decoded, Error>
            .then(
                |result: Result<Result<Decoded, Error>, Error>| match result {
                    Ok(result) => result,
                    Err(error) => Err(error.into()),
                },
            );

        (
            Self {
                tx,
                deadline: Instant::now() + timeout,
            },
            awaitable,
        )
    }

    pub fn notify(self, result: Result<Decoded, Error>) {
        if let Err(_e) = self.tx.send(result) {
            warn!("Waiter is no longer listening.")
        }
    }
}
