use failure::Error;
use futures::channel::oneshot::{self, Sender};
use futures::TryFutureExt;
use loqui_connection::{convert_timeout_error, timeout_at, LoquiError};
use std::future::Future;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct ResponseWaiter {
    tx: Sender<Result<Vec<u8>, Error>>,
    pub deadline: Instant,
}

impl ResponseWaiter {
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
    pub fn new(timeout: Duration) -> (Self, impl Future<Output = Result<Vec<u8>, Error>>) {
        let (tx, rx) = oneshot::channel();

        let deadline = Instant::now() + timeout;

        let awaitable = async move {
            let rx = rx.map_err(|_cancelled| Error::from(LoquiError::ConnectionClosed));
            timeout_at(deadline, rx)
                .await
                .unwrap_or_else(|_e| Err(LoquiError::RequestTimeout.into()))
        };

        (Self { tx, deadline }, awaitable)
    }

    /// Notify the waiter that a result was received.
    pub fn notify(self, result: Result<Vec<u8>, Error>) {
        if let Err(_e) = self.tx.send(result) {
            if self.deadline > Instant::now() {
                warn!("Waiter is no longer listening.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::future_utils::{block_on_all, spawn};
    use futures_timer::Delay;
    use tokio_futures::compat::forward::IntoAwaitable;

    #[test]
    fn it_receives_ok() {
        let (waiter, awaitable) = ResponseWaiter::new(Duration::from_secs(5));
        let result = block_on_all(async {
            spawn(async {
                waiter.notify(Ok(vec![]));
                Ok(())
            });
            awaitable.await
        });
        assert!(result.is_ok())
    }

    #[test]
    fn it_receives_error() {
        let (waiter, awaitable) = ResponseWaiter::new(Duration::from_secs(5));

        let result: Result<Vec<u8>, Error> = block_on_all(async {
            spawn(async {
                waiter.notify(Err(LoquiError::ConnectionClosed.into()));
                Ok(())
            });
            awaitable.await
        });
        assert!(result.is_err())
    }

    #[test]
    fn it_times_out() {
        let (waiter, awaitable) = ResponseWaiter::new(Duration::from_millis(1));

        let result = block_on_all(async {
            spawn(async {
                Delay::new(Duration::from_millis(50))
                    .into_awaitable()
                    .await
                    .unwrap();
                waiter.notify(Ok(vec![]));
                Ok(())
            });
            awaitable.await
        });
        assert!(result.is_err())
    }
}
