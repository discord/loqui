use failure::Error;
use futures::sync::mpsc::UnboundedSender;
use futures::sync::oneshot::Sender as OneShotSender;
use loqui_protocol::codec::LoquiFrame;
use loqui_protocol::frames::{Push, Request, Response};
use loqui_server::connection::{Connection, Event, EventHandler, HandleEventResult};
use loqui_server::error::LoquiError;
use std::collections::HashMap;
use std::future::Future;
use tokio::net::TcpStream;
use tokio::prelude::*;

// TODO: get right values
const UPGRADE_REQUEST: &'static str =
    "GET /_rpc HTTP/1.1\r\nHost: 127.0.0.1 \r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";

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
    fn upgrade(
        &self,
        mut tcp_stream: TcpStream,
    ) -> Box<dyn Future<Output = Result<TcpStream, Error>> + Send> {
        Box::new(
            async {
                await!(tcp_stream.write_all_async(&UPGRADE_REQUEST.as_bytes())).unwrap();
                await!(tcp_stream.flush_async()).unwrap();
                let mut payload = [0; 1024];
                // TODO: handle disconnect, bytes_read=0
                while let Ok(_bytes_read) = await!(tcp_stream.read_async(&mut payload)) {
                    let response = String::from_utf8(payload.to_vec()).unwrap();
                    // TODO: case insensitive
                    if response.contains(&"Upgrade") {
                        break;
                    }
                }
                Ok(tcp_stream)
            },
        )
    }

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
