use failure::Error;
use futures::sync::oneshot::{self, Receiver, Sender};
use serde::{de::DeserializeOwned};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Waiter<Decoded: DeserializeOwned + Send + Sync> {
    tx: Sender<Result<Decoded, Error>>,
    pub deadline: Instant,
}

impl<Decoded: DeserializeOwned + Send + Sync> Waiter<Decoded> {
    pub fn new(timeout: Duration) -> (Self, Receiver<Result<Decoded, Error>>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                tx,
                deadline: Instant::now() + timeout,
            },
            rx,
        )
    }

    pub fn notify(self, result: Result<Decoded, Error>) {
        if let Err(_e) = self.tx.send(result) {
            warn!("Waiter is no longer listening.")
        }
    }
}
