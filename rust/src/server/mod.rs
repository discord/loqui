use failure::Error;
use std::net::SocketAddr;

use tokio::await;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

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
    println!("Shut down.");
    Ok(())
}

async fn handle_connection(mut tcp_stream: TcpStream) -> Result<(), Error> {
    let mut data = [0; 1024];
    // TODO: handle disconnect
    while let Ok(x) = await!(tcp_stream.read_async(&mut data)) {
        println!("data! {:?}", x);
        if let Err(e) = await!(tcp_stream.write_all_async(&data)) {
            println!("Failed to write {:?}", e);
        }
    }
    await!(tcp_stream.flush_async()).unwrap();
    Ok(())
}
