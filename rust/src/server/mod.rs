use failure::Error;
use std::net::SocketAddr;

use tokio::await;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use tokio_codec::Framed;

use crate::protocol::codec::LoquiCodec;
use crate::protocol::codec::LoquiFrame;

pub async fn run<A: AsRef<str>>(address: A) -> Result<(), Error> {
    let addr: SocketAddr = address.as_ref().parse()?;
    let listener = TcpListener::bind(&addr)?;
    println!("Starting {:?} ...", address.as_ref());
    let mut incoming = listener.incoming();
    loop {
        match await!(incoming.next()) {
            Some(Ok(tcp_stream)) => {
                tokio::spawn_async(
                    async {
                        if let Err(e) = await!(handle_connection(tcp_stream)) {
                            println!("Socket error. e={:?}", e);
                        }
                    },
                );
            }
            other => {
                println!("incoming.next() return odd result. {:?}", other);
            }
        }
    }
    Ok(())
}

async fn handle_connection(mut socket: TcpStream) -> Result<(), Error> {
    let framed_socket = Framed::new(socket, LoquiCodec::new(50000));
    let (mut writer, mut reader) = framed_socket.split();
    // TODO: handle disconnect, bytes_read=0
    while let Some(result) = await!(reader.next()) {
        dbg!(&result);
        match result {
            Ok(frame) => {
                // TODO: better handle this error
                writer = await!(writer.send(frame))?;
            },
            Err(e) => {
                dbg!(e);
            }
        }
    }
    Ok(())
}
