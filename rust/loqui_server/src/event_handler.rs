use super::connection::{Connection, Event, EventHandler, ForwardRequest, HandleEventResult};
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
    tx: UnboundedSender<Event>,
    request_handler: Arc<dyn RequestHandler>,
}

impl ServerEventHandler {
    pub fn new(tx: UnboundedSender<Event>, request_handler: Arc<dyn RequestHandler>) -> Self {
        Self {
            tx,
            request_handler,
        }
    }
}

impl EventHandler for ServerEventHandler {
    fn handle_received(&mut self, frame: LoquiFrame) -> HandleEventResult {
        let tx = self.tx.clone();
        let request_handler = self.request_handler.clone();
        handle_frame(frame, request_handler, tx)
    }

    fn handle_sent(&mut self, _: u32, _: OneShotSender<Result<Vec<u8>, Error>>) {}
}

pub fn handle_frame(
    frame: LoquiFrame,
    request_handler: Arc<dyn RequestHandler + 'static>,
    tx: UnboundedSender<Event>,
    // TODO: should we just return LoquiFrame::Error if there is an error??
) -> Result<Option<LoquiFrame>, Error> {
    match frame {
        LoquiFrame::Request(request) => {
            handle_request(request, request_handler, tx);
            Ok(None)
        }
        LoquiFrame::Hello(hello) => Ok(Some(handle_hello(hello))),
        LoquiFrame::Ping(ping) => Ok(Some(handle_ping(ping))),
        frame => {
            println!("unhandled frame {:?}", frame);
            Ok(None)
        }
    }
}

fn handle_ping(ping: Ping) -> LoquiFrame {
    let Ping { flags, sequence_id } = ping;
    LoquiFrame::Pong(Pong { flags, sequence_id })
}

fn handle_hello(hello: Hello) -> LoquiFrame {
    let Hello {
        flags,
        version,
        encodings,
        compressions,
    } = hello;
    LoquiFrame::HelloAck(HelloAck {
        flags,
        ping_interval_ms: 5000,
        encoding: encodings[0].clone(),
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
    let request_handler = request_handler.clone();
    let tx = tx.clone();
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
            tokio_await!(tx.send(Event::Forward(ForwardRequest::Frame(frame))));
        },
    );
}
