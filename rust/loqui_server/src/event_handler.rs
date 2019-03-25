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
    ready_tx: OneShotSender<Result<(), Error>>,
    connection_tx: UnboundedSender<Event>,
    request_handler: Arc<dyn RequestHandler>,
    supported_encodings: Vec<String>,
    encoding: Option<String>,
}

impl ServerEventHandler {
    pub fn new(
        ready_tx: OneShotSender<Result<(), Error>>,
        connection_tx: UnboundedSender<Event>,
        request_handler: Arc<dyn RequestHandler>,
        supported_encodings: Vec<String>,
    ) -> Self {
        Self {
            ready_tx,
            connection_tx,
            request_handler,
            supported_encodings,
            encoding: None,
        }
    }
}

impl EventHandler for ServerEventHandler {
    fn handle_received(&mut self, frame: LoquiFrame) -> HandleEventResult {
        let connection_tx = self.connection_tx.clone();
        let ready_tx = self.connection_tx.clone();
        let request_handler = self.request_handler.clone();
        handle_frame(frame, request_handler, connection_tx, ready_tx)
    }

    fn handle_sent(&mut self, _: u32, _: OneShotSender<Result<Vec<u8>, Error>>) {}
}

pub fn handle_frame(
    frame: LoquiFrame,
    request_handler: Arc<dyn RequestHandler + 'static>,
    connection_tx: UnboundedSender<Event>,
    ready_tx: OneShotSender<Result<(), Error>>,
    supported_encodings: &Vec<String>,
    // TODO: should we just return LoquiFrame::Error if there is an error??
) -> Result<Option<LoquiFrame>, Error> {
    match frame {
        LoquiFrame::Request(request) => {
            handle_request(request, request_handler, connection_tx);
            Ok(None)
        }
        LoquiFrame::Hello(hello) => {
            let hello_ack = handle_hello(hello, supported_encodings);
        },
        frame => {
            dbg!(&frame);
            Err(LoquiError::InvalidFrame { frame }.into())
        }
    }
}

fn negotiate_encoding(
    server_encodings: &[String],
    client_encodings: &[String],
) -> Option<String> {
    for server_encoding in server_encodings {
        for client_encoding in client_encodings {
            if server_encoding == client_encoding {
                return Some(server_encoding.clone());
            }
        }
    }
    None
}

fn handle_ping(ping: Ping) -> LoquiFrame {
    let Ping { flags, sequence_id } = ping;
    LoquiFrame::Pong(Pong { flags, sequence_id })
}

fn handle_hello(hello: Hello, supported_encodings: &Vec<String>) -> LoquiFrame {
    // TODO: encoding/compression negotiation
    let Hello {
        flags,
        version,
        encodings,
        compressions,
    } = hello;
    let encoding =  negotiate_encoding(supported_encodings, &encodings).expect("no common encoding");
    LoquiFrame::HelloAck(HelloAck {
        flags,
        ping_interval_ms: 5000,
        encoding,
        compression: "".to_string(),
    })
}

fn handle_request(
    request: Request,
    request_handler: Arc<dyn RequestHandler + 'static>,
    tx: UnboundedSender<Event>,
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
            tokio_await!(tx.send(Event::Forward(Forward::Frame(frame))));
        },
    );
}
