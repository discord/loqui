use failure::Error;
use futures::sync::mpsc::UnboundedSender;
use futures::sync::oneshot::Sender as OneShotSender;
use loqui_protocol::codec::LoquiFrame;
use loqui_protocol::frames::{Push, Request, Response};
use loqui_server::connection::{Connection, Event, EventHandler, HandleEventResult};
use loqui_server::error::LoquiError;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Ready {
    encoding: String,
}

pub struct ClientEventHandler {
    ready_tx: Option<OneShotSender<Result<Ready, Error>>>,
    waiters: HashMap<u32, OneShotSender<Result<Vec<u8>, Error>>>,
}

impl ClientEventHandler {
    pub fn new(ready_tx: OneShotSender<Result<Ready, Error>>) -> Self {
        Self {
            // TODO: should probably sweep these, probably request timeout
            waiters: HashMap::new(),
            ready_tx: Some(ready_tx),
        }
    }
}

impl EventHandler for ClientEventHandler {
    fn handle_received(&mut self, frame: LoquiFrame) -> HandleEventResult {
        match frame {
            LoquiFrame::Response(Response {
                flags,
                sequence_id,
                payload,
            }) => {
                let sender = self.waiters.remove(&sequence_id).unwrap();
                sender.send(Ok(payload)).unwrap();
                Ok(None)
            }
            LoquiFrame::HelloAck(hello_ack) => {
                // TODO: compression
                let ready = Ready {
                    encoding: hello_ack.encoding,
                };
                let ready_tx = self
                    .ready_tx
                    .take()
                    .expect("already sent ready")
                    .send(Ok(ready))
                    .expect("failed to send to ready");
                Ok(None)
            }
            frame => {
                // TODO: handle invalid encoding or compression
                // self.ready_tx.send(Err(...));
                dbg!(&frame);
                Err(LoquiError::InvalidFrame { frame }.into())
            }
        }
    }

    fn handle_sent(&mut self, sequence_id: u32, waiter_tx: OneShotSender<Result<Vec<u8>, Error>>) {
        self.waiters.insert(sequence_id, waiter_tx);
    }
}
