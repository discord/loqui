#![feature(await_macro, async_await, futures_api)]
//#![recursion_limit = "128"]

#[macro_use]
extern crate log;

use failure::Error;
use std::net::SocketAddr;

use tokio::await;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

use std::sync::Arc;

mod connection;
mod handler;

use self::connection::Connection;
pub use self::handler::Handler;

pub struct Server {
    pub handler: Arc<Handler>,
}

impl Server {
    // TODO
    /*
    pub fn new(handler: Handler) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }
    */

    fn handle_connection(&self, tcp_stream: TcpStream) {
        let connection = Connection::new(tcp_stream, self.handler.clone());
        tokio::spawn_async(connection.run());
    }

    // TODO
    //pub async fn serve<A: AsRef<str>>(&self, address: A) -> Result<(), Error> {
    pub async fn listen_and_serve(&self, address: String) -> Result<(), Error> {
        let addr: SocketAddr = address.parse()?;
        let listener = TcpListener::bind(&addr)?;
        println!("Starting {:?} ...", address);
        let mut incoming = listener.incoming();
        loop {
            match await!(incoming.next()) {
                Some(Ok(tcp_stream)) => {
                    self.handle_connection(tcp_stream);
                }
                other => {
                    println!("incoming.next() return odd result. {:?}", other);
                }
            }
        }
    }
}
