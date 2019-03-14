use std::collections::HashMap;
use std::net::SocketAddr;

use failure::{Error, err_msg};
use std::future::Future as StdFuture;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Poll as StdPoll, Waker};
use tokio::await;
use tokio::net::{TcpListener, TcpStream};
use tokio_io::io::WriteHalf;
use futures::channel::mpsc;
use futures::future::{self, FutureExt};
use futures::stream::{self, StreamExt};
use futures::select;
use futures::channel::oneshot::{channel as oneshot, Sender as OneShotSender};
use tokio::prelude::*;

const UPGRADE_REQUEST: &'static str =
    "GET #{loqui_path} HTTP/1.1\r\nHost: #{host}\r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";

pub struct Client {
    //reader: ReadHalf<TcpStream>,
    writer: WriteHalf<TcpStream>,
    sender: mpsc::UnboundedSender<(u32, OneShotSender<String>)>,
    // TODO: should probably sweep these
}

impl Client {
    pub async fn connect<A: AsRef<str>>(address: A) -> Result<Client, Error> {
        let addr: SocketAddr = address.as_ref().parse()?;
        let socket = await!(TcpStream::connect(&addr))?;
        let (mut reader, writer) = socket.split();
        let (tx, rx) = mpsc::unbounded();

        // read task
        tokio::spawn_async(async move {
            let waiters: HashMap<u32, OneShotSender<String>> = HashMap::new();
            let mut data = [0; 1024];
            let mut rx = rx.fuse();
            let mut x = reader.read_async(&mut data).fuse();
            select! {
                message = rx.next() => {
                    println!("received a message. message={:?}", message)
                }
                _ =  x => {
                    println!("read data {:?}", data.to_vec());
                },
            };
            /*
            while let Ok(_bytes_read) = await!(reader.read_async(&mut data)) {
                println!("received data from server {:?}", data.to_vec());
                let sender = read_waiters.write().unwrap().remove(&1).unwrap();
                sender.send(String::from_utf8(data.to_vec()).unwrap());
            }
            */
        });

        let mut client = Self {
            sender: tx,
            writer,
        };

        await!(client.upgrade())?;
        Ok(client)
    }

    async fn write<'a>(&'a mut self, data: &'a [u8]) -> Result<(), Error> {
        await!(self.writer.write_all_async(data))?;
        await!(self.writer.flush_async())?;
        Ok(())
    }

    pub async fn upgrade(&mut self) -> Result<(), Error> {
        await!(self.write(UPGRADE_REQUEST.as_bytes()))?;
        Ok(())
    }

    pub async fn request(&mut self, message: String) -> Result<String, Error> {
        let data = message.as_bytes();
        await!(self.write(data))?;
        let seq = self.next_seq();
        let (sender, receiver) = oneshot();
        self.sender.unbounded_send((seq, sender))?;
        let result = await!(receiver)?;
        Ok(result)
    }

    fn next_seq(&mut self) -> u32 {
        1
    }
}
