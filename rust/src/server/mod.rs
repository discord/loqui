use failure::Error;
use std::future::Future;
use std::net::SocketAddr;

use tokio::await;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use tokio_codec::Framed;

use crate::protocol::codec::{LoquiCodec, LoquiFrame};
use crate::protocol::frames::*;
use std::sync::Arc;

pub trait Handler: Send + Sync {
    fn handle_request(
        &self,
        request: Request,
    ) -> Box<dyn Future<Output = Result<Vec<u8>, Error>>>;
}

pub struct Server {
    pub handler: Arc<Handler>,
}

impl Server {
    /*
    pub fn new(handler: Handler) -> Self {
        Self {
            handler
        }
    }
    */

    // TODO
    //pub async fn serve<A: AsRef<str>>(&self, address: A) -> Result<(), Error> {
    pub async fn serve(&self, address: String) -> Result<(), Error> {
        let addr: SocketAddr = address.parse()?;
        let listener = TcpListener::bind(&addr)?;
        println!("Starting {:?} ...", address);
        let mut incoming = listener.incoming();
        loop {
            match await!(incoming.next()) {
                Some(Ok(tcp_stream)) => {
                    tokio::spawn_async(handle_connection(tcp_stream, self.handler.clone()));
                }
                other => {
                    println!("incoming.next() return odd result. {:?}", other);
                }
            }
        }
        Ok(())
    }
}

async fn handle_frame<'e>(frame: LoquiFrame, handler: Arc<Handler + 'e>) -> Result<Option<LoquiFrame>, Error> {
    match frame {
        LoquiFrame::Request(request @ Request {
            flags,
            sequence_id,
            payload,
            ..
        }) => {
            let result = await!(handler.handle_request(request));
            println!("result {:?}", result);
            Ok(Some(LoquiFrame::Response(Response {
                flags,
                sequence_id,
                payload,
            })))
        },
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

async fn handle_connection<'e>(mut socket: TcpStream, handler: Arc<Handler + 'e>) {
    socket = await!(upgrade(socket));
    let framed_socket = Framed::new(socket, LoquiCodec::new(50000 * 1000));
    let (mut writer, mut reader) = framed_socket.split();
    // TODO: handle disconnect, bytes_read=0
    while let Some(result) = await!(reader.next()) {
        match result {
            Ok(frame) => {
                // TODO: handle error
                if let Ok(Some(response)) = await!(handle_frame(frame, handler.clone())) {
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
