use failure::{err_msg, Error};
use futures::sync::oneshot::Sender as OneShotSender;
use loqui_protocol::codec::LoquiFrame;
use loqui_protocol::frames::{Request, Response};
use loqui_server::connection::ConnectionSender;
use loqui_server::connection::{FrameHandler, HandleEventResult};
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

pub struct ClientFrameHandler {
    connection_sender: ConnectionSender,
    ready_tx: Option<OneShotSender<Result<Ready, Error>>>,
    waiters: HashMap<u32, OneShotSender<Result<Vec<u8>, Error>>>,
}

impl ClientFrameHandler {
    pub fn new(
        connection_sender: ConnectionSender,
        ready_tx: OneShotSender<Result<Ready, Error>>,
    ) -> Self {
        Self {
            // TODO: should probably sweep these, probably request timeout
            connection_sender,
            waiters: HashMap::new(),
            ready_tx: Some(ready_tx),
        }
    }
}

impl FrameHandler for ClientFrameHandler {
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
                match self.waiters.remove(&sequence_id) {
                    Some(waiter_tx) => {
                        waiter_tx
                            .send(Ok(payload))
                            .expect("Failed to send to waiter");
                        Ok(None)
                    }
                    None => {
                        // TODO: maybe should debug and move on?
                        Err(err_msg(format!(
                            "No waiter for sequence_id. sequence_id={:?}",
                            sequence_id
                        )))
                    }
                }
            }
            LoquiFrame::HelloAck(hello_ack) => {
                // TODO: compression
                let ready = Ready {
                    encoding: hello_ack.encoding,
                };
                self.ready_tx
                    .take()
                    .expect("already sent ready")
                    .send(Ok(ready))
                    .expect("failed to send to ready");
                self.connection_sender
                    .ready(hello_ack.ping_interval_ms)
                    .expect("conn dead");
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

    fn handle_request(
        &self,
        request: Request,
    ) -> Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send> {
        Box::new(async { Ok(vec![]) })
    }
}
