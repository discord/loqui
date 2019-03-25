use super::connection::{Connection, Event, EventHandler, Forward, HandleEventResult};
use super::error::LoquiError;
use super::request_handler::RequestHandler;
use super::RequestContext;
use failure::Error;
use futures::sync::mpsc::{self, UnboundedSender};
use futures::sync::oneshot::Sender as OneShotSender;
use loqui_protocol::codec::LoquiFrame;
use loqui_protocol::frames::{Error as ErrorFrame, Hello, HelloAck, Ping, Pong, Request, Response};
use std::sync::Arc;
use tokio::await as tokio_await;
use tokio::prelude::*;

pub struct ServerEventHandler {
    connection_tx: UnboundedSender<Event>,
    request_handler: Arc<dyn RequestHandler>,
    supported_encodings: Vec<String>,
    encoding: Option<String>,
}

impl ServerEventHandler {
    pub fn new(
        connection_tx: UnboundedSender<Event>,
        request_handler: Arc<dyn RequestHandler>,
        supported_encodings: Vec<String>,
    ) -> Self {
        Self {
            connection_tx,
            request_handler,
            supported_encodings,
            encoding: None,
        }
    }
}

impl EventHandler for ServerEventHandler {
    fn handle_received(&mut self, frame: LoquiFrame) -> HandleEventResult {
        self.handle_frame(frame)
    }

    fn handle_sent(&mut self, _: u32, _: OneShotSender<Result<Vec<u8>, Error>>) {}
}

impl ServerEventHandler {
    pub fn handle_frame(
        &mut self,
        frame: LoquiFrame,
        // TODO: should we just return LoquiFrame::Error if there is an error??
    ) -> Result<Option<LoquiFrame>, Error> {
        match frame {
            LoquiFrame::Request(request) => {
                self.handle_request(request);
                Ok(None)
            }
            LoquiFrame::Hello(hello) => {
                let hello_ack = self.handle_hello(hello);
                Ok(Some(hello_ack))
            },
            frame => {
                dbg!(&frame);
                Err(LoquiError::InvalidFrame { frame }.into())
            }
        }
    }

    fn negotiate_encoding(
        &self,
        client_encodings: &[String],
    ) -> Option<String> {
        for supported_encoding in &self.supported_encodings {
            for client_encoding in client_encodings {
                if supported_encoding == client_encoding {
                    return Some(supported_encoding.clone());
                }
            }
        }
        None
    }

    fn handle_ping(&self, ping: Ping) -> LoquiFrame {
        let Ping { flags, sequence_id } = ping;
        LoquiFrame::Pong(Pong { flags, sequence_id })
    }

    fn handle_hello(&self, hello: Hello) -> LoquiFrame {
        // TODO: encoding/compression negotiation
        let Hello {
            flags,
            version,
            encodings,
            compressions,
        } = hello;
        let encoding = self.negotiate_encoding(&encodings).expect("no common encoding");
        LoquiFrame::HelloAck(HelloAck {
            flags,
            ping_interval_ms: 5000,
            encoding,
            compression: "".to_string(),
        })
    }

    fn handle_request(
        &mut self,
        request: Request,
    ) {
        let Request {
            payload,
            flags,
            sequence_id,
        } = request;
        let request_context = RequestContext {
            payload,
            flags,
            // TODO:
            encoding: "json".to_string(),
        };
        let request_handler = self.request_handler.clone();
        let connection_tx = self.connection_tx.clone();
        tokio::spawn_async(
            async move {
                let frame = match await!(Box::into_pin(
                    request_handler.handle_request(request_context)
                )) {
                    Ok(payload) => LoquiFrame::Response(Response {
                        flags,
                        sequence_id,
                        payload,
                    }),
                    Err(e) => {
                        dbg!(e);
                        // TODO:
                        LoquiFrame::Error(ErrorFrame {
                            flags,
                            sequence_id,
                            code: 0,
                            payload: vec![],
                        })
                    }
                };
                tokio_await!(connection_tx.send(Event::Forward(Forward::Frame(frame))));
            },
        );
    }
}

