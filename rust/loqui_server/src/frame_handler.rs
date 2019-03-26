use super::connection::{ConnectionSender, FrameHandler, HandleEventResult};
use super::error::LoquiError;
use super::request_handler::RequestHandler;
use super::RequestContext;
use failure::Error;
use futures::sync::mpsc::{self, UnboundedSender};
use futures::sync::oneshot::Sender as OneShotSender;
use loqui_protocol::codec::LoquiFrame;
use loqui_protocol::frames::{Error as ErrorFrame, Hello, HelloAck, Ping, Pong, Request, Response};
use std::future::Future;
use std::sync::Arc;
use tokio::await as tokio_await;
use tokio::net::TcpStream;
use tokio::prelude::*;

pub struct ServerFrameHandler {
    connection_sender: ConnectionSender,
    request_handler: Arc<dyn RequestHandler>,
    supported_encodings: Vec<String>,
    encoding: Option<String>,
}

impl ServerFrameHandler {
    pub fn new(
        connection_sender: ConnectionSender,
        request_handler: Arc<dyn RequestHandler>,
        supported_encodings: Vec<String>,
    ) -> Self {
        Self {
            connection_sender,
            request_handler,
            supported_encodings,
            encoding: None,
        }
    }
}

impl FrameHandler for ServerFrameHandler {
    fn upgrade(
        &self,
        mut tcp_stream: TcpStream,
    ) -> Box<dyn Future<Output = Result<TcpStream, Error>> + Send> {
        Box::new(
            async {
                let mut payload = [0; 1024];
                // TODO: handle disconnect, bytes_read=0
                while let Ok(_bytes_read) = await!(tcp_stream.read_async(&mut payload)) {
                    let request = String::from_utf8(payload.to_vec()).unwrap();
                    // TODO: better
                    if request.contains(&"upgrade") || request.contains(&"Upgrade") {
                        let response =
                        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: loqui\r\nConnection: Upgrade\r\n\r\n";
                        await!(tcp_stream.write_all_async(&response.as_bytes()[..])).unwrap();
                        await!(tcp_stream.flush_async()).unwrap();
                        break;
                    }
                }
                Ok(tcp_stream)
            },
        )
    }

    fn handle_received(&mut self, frame: LoquiFrame) -> HandleEventResult {
        self.handle_frame(frame)
    }

    fn handle_sent(&mut self, _: u32, _: OneShotSender<Result<Vec<u8>, Error>>) {}

    fn handle_request(
        &self,
        request: Request,
    ) -> Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send> {
        if self.encoding.is_none() {
            return Box::new(async { Err(LoquiError::NotReady.into()) });
        }
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
        self.request_handler.handle_request(request_context)
    }
}

impl ServerFrameHandler {
    pub fn handle_frame(
        &mut self,
        frame: LoquiFrame,
        // TODO: should we just return LoquiFrame::Error if there is an error??
    ) -> Result<Option<LoquiFrame>, Error> {
        match frame {
            LoquiFrame::Hello(hello) => {
                let frame = self.handle_hello(hello);
                if let LoquiFrame::HelloAck(hello_ack) = &frame {
                    self.encoding = Some(hello_ack.encoding.clone());
                    self.connection_sender.ready(hello_ack.ping_interval_ms)?;
                }
                Ok(Some(frame))
            }
            frame => {
                dbg!(&frame);
                Err(LoquiError::InvalidFrame { frame }.into())
            }
        }
    }

    fn negotiate_encoding(&self, client_encodings: &[String]) -> Option<String> {
        for supported_encoding in &self.supported_encodings {
            for client_encoding in client_encodings {
                if supported_encoding == client_encoding {
                    return Some(supported_encoding.clone());
                }
            }
        }
        None
    }

    fn handle_hello(&self, hello: Hello) -> LoquiFrame {
        // TODO: encoding/compression negotiation
        let Hello {
            flags,
            version,
            encodings,
            compressions,
        } = hello;
        let encoding = self
            .negotiate_encoding(&encodings)
            .expect("no common encoding");
        LoquiFrame::HelloAck(HelloAck {
            flags,
            ping_interval_ms: 5000,
            encoding,
            compression: "".to_string(),
        })
    }
}
