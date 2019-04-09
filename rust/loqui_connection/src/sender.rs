use crate::connection::Event;
use crate::LoquiError;
use failure::Error;
use futures::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use loqui_protocol::frames::Response;

#[derive(Debug)]
pub struct Sender<T: Send + 'static> {
    tx: UnboundedSender<Event<T>>,
}

impl<T: Send + 'static> Sender<T> {
    pub(crate) fn new() -> (Self, UnboundedReceiver<Event<T>>) {
        let (tx, rx) = mpsc::unbounded();
        (Self { tx }, rx)
    }

    pub(crate) fn internal(&self, event: T) -> Result<(), Error> {
        self.tx
            .unbounded_send(Event::InternalEvent(event))
            .map_err(|_e| LoquiError::TcpStreamClosed.into())
    }

    pub(crate) fn response_complete(
        &self,
        result: Result<Response, (Error, u32)>,
    ) -> Result<(), Error> {
        self.tx
            .unbounded_send(Event::ResponseComplete(result))
            .map_err(|_e| LoquiError::TcpStreamClosed.into())
    }
}

impl<T: Send> Clone for Sender<T> {
    fn clone(&self) -> Sender<T> {
        Self {
            tx: self.tx.clone(),
        }
    }
}
