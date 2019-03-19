use std::sync::Arc;

use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio_codec::Framed;

use super::Handler;
use crate::protocol::codec::{LoquiCodec, LoquiFrame};
use crate::protocol::frames::*;

pub struct Connection {}

impl Connection {
    pub async fn run<'e>(mut socket: TcpStream, handler: Arc<Handler + 'e>) {
        socket = await!(Connection::upgrade(socket));
        let framed_socket = Framed::new(socket, LoquiCodec::new(50000 * 1000));
        let (mut writer, mut reader) = framed_socket.split();
        // TODO: handle disconnect, bytes_read=0
        while let Some(result) = await!(reader.next()) {
            match result {
                Ok(frame) => {
                    // TODO: handle error
                    if let Ok(Some(response)) =
                        await!(Connection::handle_frame(frame, handler.clone()))
                    {
                        match await!(writer.send(response)) {
                            Ok(new_writer) => writer = new_writer,
                            // TODO: better handle this error
                            Err(e) => {
                                error!("Failed to write. error={:?}", e);
                                return;
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

    async fn upgrade(mut socket: TcpStream) -> TcpStream {
        // TODO: buffering
        let mut payload = [0; 1024];
        while let Ok(bytes_read) = await!(socket.read_async(&mut payload)) {
            let request = String::from_utf8(payload.to_vec()).unwrap();
            // TODO: better
            if request.contains(&"upgrade") || request.contains(&"Upgrade") {
                let response =
                    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: loqui\r\nConnection: Upgrade\r\n\r\n";
                await!(socket.write_all_async(&response.as_bytes()[..])).unwrap();
                await!(socket.flush_async()).unwrap();
                break;
            }
        }
        socket
    }

    async fn handle_frame<'e>(
        frame: LoquiFrame,
        handler: Arc<Handler + 'e>,
    ) -> Result<Option<LoquiFrame>, Error> {
        match frame {
            LoquiFrame::Request(request @ Request { .. }) => {
                let sequence_id = request.sequence_id;
                let flags = request.flags;
                let result = await!(handler.handle_request(request));
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
