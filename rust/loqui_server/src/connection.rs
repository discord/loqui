use std::sync::Arc;

use futures::sync::mpsc;
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio_codec::Framed;

use super::{Handler, RequestContext};
use failure::err_msg;
use loqui_protocol::codec::{LoquiCodec, LoquiFrame};
use loqui_protocol::frames::*;

#[derive(Debug)]
enum Message {
    Request(LoquiFrame),
    Response(LoquiFrame),
}

pub struct Connection {
    tcp_stream: TcpStream,
    handler: Arc<Handler>,
    encoding: String,
}

impl Connection {
    pub fn new(tcp_stream: TcpStream, handler: Arc<Handler>) -> Self {
        Self {
            tcp_stream,
            handler,
            // TODO:
            encoding: "json".to_string(),
        }
    }

    pub async fn run<'e>(mut self) {
        self = await!(self.upgrade());
        let framed_socket = Framed::new(self.tcp_stream, LoquiCodec::new(50000 * 1000));
        let (mut writer, mut reader) = framed_socket.split();
        // TODO: handle disconnect

        let (tx, rx) = mpsc::unbounded::<Message>();
        let mut stream = reader
            .map(|frame| Message::Request(frame))
            .select(rx.map_err(|()| err_msg("rx error")));

        while let Some(message) = await!(stream.next()) {
            // TODO: handle error
            match message {
                Ok(message) => {
                    match message {
                        Message::Request(frame) => {
                            let tx = tx.clone();
                            let handler = self.handler.clone();
                            tokio::spawn_async(
                                async move {
                                    // TODO: handle error
                                    match await!(Connection::handle_frame(frame, handler)) {
                                        Ok(Some(frame)) => {
                                            await!(tx.send(Message::Response(frame)));
                                        }
                                        Ok(None) => {
                                            dbg!("None");
                                        }
                                        Err(e) => {
                                            dbg!(e);
                                        }
                                    }
                                },
                            );
                        }
                        Message::Response(frame) => {
                            match await!(writer.send(frame)) {
                                Ok(new_writer) => writer = new_writer,
                                // TODO: better handle this error
                                Err(e) => {
                                    error!("Failed to write. error={:?}", e);
                                    return;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    dbg!(e);
                }
            }
        }
        println!("connection closed");
    }

    async fn upgrade(mut self) -> Self {
        // TODO: buffering
        let mut payload = [0; 1024];
        // TODO: handle disconnect, bytes_read=0
        while let Ok(_bytes_read) = await!(self.tcp_stream.read_async(&mut payload)) {
            let request = String::from_utf8(payload.to_vec()).unwrap();
            // TODO: better
            if request.contains(&"upgrade") || request.contains(&"Upgrade") {
                let response =
                    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: loqui\r\nConnection: Upgrade\r\n\r\n";
                await!(self.tcp_stream.write_all_async(&response.as_bytes()[..])).unwrap();
                await!(self.tcp_stream.flush_async()).unwrap();
                break;
            }
        }
        self
    }

    async fn handle_frame<'e>(
        frame: LoquiFrame,
        handler: Arc<Handler + 'e>,
    ) -> Result<Option<LoquiFrame>, Error> {
        match frame {
            LoquiFrame::Request(Request {
                payload,
                flags,
                sequence_id,
            }) => {
                let request_context = RequestContext {
                    payload,
                    flags,
                    // TODO:
                    encoding: "json".to_string(),
                };
                let result = await!(Box::into_pin(handler.handle_request(request_context)));
                match result {
                    Ok(payload) => Ok(Some(LoquiFrame::Response(Response {
                        flags,
                        sequence_id,
                        payload,
                    }))),
                    Err(e) => {
                        dbg!(e);
                        // TODO:
                        Ok(None)
                    }
                }
            }
            LoquiFrame::Hello(Hello {
                flags,
                version,
                encodings,
                compressions,
            }) => Ok(Some(LoquiFrame::HelloAck(HelloAck {
                flags,
                ping_interval_ms: 5000,
                encoding: encodings[0].clone(),
                compression: "".to_string(),
            }))),
            LoquiFrame::Ping(Ping { flags, sequence_id }) => {
                Ok(Some(LoquiFrame::Pong(Pong { flags, sequence_id })))
            }
            frame => {
                println!("unhandled frame {:?}", frame);
                Ok(None)
            }
        }
    }
}
