use failure::Error;
use std::net::SocketAddr;

use tokio::await;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

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

async fn handle_connection(mut tcp_stream: TcpStream) -> Result<(), Error> {
    let framed_socket = Framed::new(socket, LoquiCodec::new(50000));
    let (mut writer, mut reader) = framed_socket.split();
    // TODO: handle disconnect, bytes_read=0
    while let Ok(frame) = await!(reader.next()) {
        dbg!((bytes_read, data.to_vec()));
        if let Err(e) = await!(tcp_stream.write_all_async(&data)) {
            println!("Failed to write {:?}", e);
        }
    }
    await!(tcp_stream.flush_async()).unwrap();
    Ok(())
}
