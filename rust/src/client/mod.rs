use failure::Error;
use std::net::SocketAddr;

use tokio::await;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

pub struct Client {
    socket: TcpStream,
}

impl Client {
    pub async fn connect<A: AsRef<str>>(address: A) -> Result<Client, Error> {
        let addr: SocketAddr = address.as_ref().parse()?;
        let socket = await!(TcpStream::connect(&addr))?;
        Ok(Self { socket })
    }

    pub async fn request(&mut self, message: String) -> Result<String, Error> {
        let data = message.as_bytes();
        if let Err(e) = await!(self.socket.write_all_async(&data)) {
            println!("Failed to write {:?}", e);
        }
        await!(self.socket.flush_async()).map_err(|e| Error::from(e))?;

        let mut response = [0; 1024];
        let bytes_read = dbg!(await!(self.socket.read_async(&mut response))?);
        Ok(String::from_utf8(response[..bytes_read].to_vec())?)
    }
}
