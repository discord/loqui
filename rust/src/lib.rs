#![feature(await_macro, async_await, futures_api)]

use failure::Error;
use std::net::SocketAddr;

use tokio::await;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

pub fn test() -> Result<(), Error> {
    let addr: SocketAddr = "127.0.0.1:3000".parse()?;
    let listener = TcpListener::bind(&addr)?;
    tokio::run_async(async move {
        let mut incoming = listener.incoming();
        // TODO: handle other cases of incoming.next()
        while let Some(Ok(tcp_stream)) = await!(incoming.next()) {
            tokio::spawn_async(async {

                let result = await!(handle_connection(tcp_stream));
                println!("tcp result {:?}", result);
            });
        }
    });
    Ok(())
}

async fn handle_connection(mut tcp_stream: TcpStream) -> Result<(), Error> {
    println!("stream {:?}", tcp_stream);
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
