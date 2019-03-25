use failure::Error;
use futures::sync::mpsc::UnboundedSender;
use futures::sync::oneshot::Sender as OneShotSender;
use loqui_protocol::codec::LoquiFrame;
use loqui_protocol::frames::{Push, Request, Response};
use loqui_server::connection::{Connection, Event, EventHandler, HandleEventResult};
use std::collections::HashMap;

#[derive(Debug)]
pub enum ClientEvent {
    Request {
        payload: Vec<u8>,
        // TODO: probably need to handle error better?
        sender: OneShotSender<Result<Vec<u8>, Error>>,
    },
    Push {
        payload: Vec<u8>,
    },
}

pub struct ClientEventHandler {
    waiters: HashMap<u32, OneShotSender<Result<Vec<u8>, Error>>>,
    next_seq: u32,
}

impl ClientEventHandler {
    pub fn new() -> Self {
        Self {
            // TODO: should probably sweep these, probably request timeout
            waiters: HashMap::new(),
            next_seq: 1,
        }
    }

    fn next_seq(&mut self) -> u32 {
        let seq = self.next_seq;
        self.next_seq += 1;
        seq
    }
}

impl EventHandler<ClientEvent> for ClientEventHandler {
    fn handle_event(&mut self, event: Event<ClientEvent>) -> HandleEventResult {
        match event {
            Event::Internal(ClientEvent::Request { payload, sender }) => {
                let sequence_id = self.next_seq();
                self.waiters.insert(sequence_id, sender);
                Ok(Some(LoquiFrame::Request(Request {
                    sequence_id,
                    flags: 0,
                    payload,
                })))
            }
            Event::Internal(ClientEvent::Push { payload }) => {
                Ok(Some(LoquiFrame::Push(Push { flags: 0, payload })))
            }
            Event::Socket(frame) => {
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
        }
    }
}
