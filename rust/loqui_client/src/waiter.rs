use failure::Error;
use futures::future::Future;
use futures::sync::oneshot::{self, Sender};
use futures_timer::FutureExt;
use loqui_connection::{convert_timeout_error, LoquiError};
use serde::de::DeserializeOwned;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct ResponseWaiter<Decoded: DeserializeOwned + Send> {
    tx: Sender<Result<Decoded, Error>>,
    pub deadline: Instant,
}

impl<Decoded: DeserializeOwned + Send> ResponseWaiter<Decoded> {
    /// Creates a new response waiter that will wait until the specified timeout.
    /// The returned future will resolve when someone calls waiter.notify().
    ///
    /// # Arguments
    ///
    /// * `timeout` - the `Duration` of time to wait before giving up on the request
    ///
    /// # Errors
    ///
    /// `LoquiError::RequestTimeout` or some other error from the server.
    ///
    pub fn new(timeout: Duration) -> (Self, impl Future<Item = Decoded, Error = Error>) {
        let (tx, rx) = oneshot::channel::<Result<Decoded, Error>>();

        let deadline = Instant::now() + timeout;

        let awaitable = rx
            .map_err(|_canceled| Error::from(LoquiError::ConnectionClosed))
            .timeout_at(deadline)
            .map_err(convert_timeout_error)
            // Collapses the Result<Result<Decoded, Error>> into a Result<Decoded, Error>
            .then(
                |result: Result<Result<Decoded, Error>, Error>| match result {
                    Ok(result) => result,
                    Err(error) => Err(error.into()),
                },
            );

        (Self { tx, deadline }, awaitable)
    }

    /// Notify the waiter that a result was received.
    pub fn notify(self, result: Result<Decoded, Error>) {
        if let Err(_e) = self.tx.send(result) {
            if self.deadline > Instant::now() {
                warn!("Waiter is no longer listening.")
            }
        }
    }
}
