use failure::Error;
use futures::sync::mpsc::UnboundedSender;
use futures::sync::oneshot::Sender as OneShotSender;
use loqui_protocol::codec::LoquiFrame;
use loqui_protocol::frames::{Push, Request, Response};
use loqui_server::connection::{Connection, Event, EventHandler, HandleEventResult};
use std::collections::HashMap;

pub struct ClientEventHandler {
    waiters: HashMap<u32, OneShotSender<Result<Vec<u8>, Error>>>,
}

impl ClientEventHandler {
    pub fn new() -> Self {
        Self {
            // TODO: should probably sweep these, probably request timeout
            waiters: HashMap::new(),
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
            }
            frame => {
                // TODO:
                dbg!(frame);
            }
        }
        Ok(None)
    }

    fn handle_sent(&mut self, sequence_id: u32, waiter_tx: OneShotSender<Result<Vec<u8>, Error>>) {
        self.waiters.insert(sequence_id, waiter_tx);
    }
}
