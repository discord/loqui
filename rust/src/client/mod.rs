use std::net::SocketAddr;
use std::collections::HashMap;

use failure::Error;
use tokio::await;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

const UPGRADE_REQUEST: &'static str = "GET #{loqui_path} HTTP/1.1\r\nHost: #{host}\r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";

pub struct Client {
    socket: TcpStream,
    waiters: HashMap<u32, Option<String>>,
}

impl Client {
    pub async fn connect<A: AsRef<str>>(address: A) -> Result<Client, Error> {
        let addr: SocketAddr = address.as_ref().parse()?;
        let socket = await!(TcpStream::connect(&addr))?;
        let mut client = Self {
            socket
        };
        await!(client.upgrade())?;
        Ok(client)
    }

    async fn write<'a>(&'a mut self, data: &'a [u8]) -> Result<(), Error> {
        await!(self.socket.write_all_async(data))?;
        await!(self.socket.flush_async())?;
        Ok(())
    }

    pub async fn upgrade(&mut self) -> Result<(), Error> {
        await!(self.write(UPGRADE_REQUEST.as_bytes()))?;
        Ok(())
    }

    pub async fn request(&mut self, message: String) -> Result<String, Error> {
        let data = message.as_bytes();
        await!(self.write(data))?;

        let mut response = [0; 4];
        await!(self.socket.read_exact_async(&mut response))?;
        Ok(String::from_utf8(response.to_vec())?)
    }
}
