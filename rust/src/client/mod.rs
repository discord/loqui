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
use futures::oneshot;
use futures::sync::oneshot::Sender;
use tokio::prelude::*;

const UPGRADE_REQUEST: &'static str =
    "GET #{loqui_path} HTTP/1.1\r\nHost: #{host}\r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";

pub struct Client {
    //reader: ReadHalf<TcpStream>,
    writer: WriteHalf<TcpStream>,
    // TODO: should probably sweep these
    waiters: Arc<RwLock<HashMap<u32, Sender<String>>>>,
}

struct ResponseFuture {
    seq: u32,
    waiters: Arc<RwLock<HashMap<u32, Sender<String>>>>,
}

/*
impl StdFuture for ResponseFuture {
    type Output = Result<String, Error>;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> StdPoll<Self::Output> {
        println!("poll");
        match self.waiters.as_ref().try_read() {
            Ok(waiters) => {
                match waiters.get(&self.seq) {
                    // TODO: need to remove it from waiters
                    Some(Some(response)) => StdPoll::Ready(Ok(response.clone())),
                    Some(None) => {
                        println!("pending");
                        StdPoll::Pending
                    },
                    None => StdPoll::Ready(Err(err_msg("Waiting for a sequence that doesn't exist"))),
                }
            },
            Err(e) => StdPoll::Pending,
        }
    }
}
*/

impl Client {
    pub async fn connect<A: AsRef<str>>(address: A) -> Result<Client, Error> {
        let addr: SocketAddr = address.as_ref().parse()?;
        let socket = await!(TcpStream::connect(&addr))?;
        let (mut reader, writer) = socket.split();
        let waiters : Arc<RwLock<HashMap<u32, Sender<String>>>> = Arc::new(RwLock::new(HashMap::new()));
        let read_waiters = waiters.clone();

        tokio::spawn_async(async move {
            let mut data = [0; 1024];
            while let Ok(_bytes_read) = await!(reader.read_async(&mut data)) {
                println!("received data from server {:?}", data.to_vec());
                let sender = read_waiters.write().unwrap().remove(&1).unwrap();
                sender.send(String::from_utf8(data.to_vec()).unwrap());
                //read_waiters.write().unwrap().insert(1, Some("done".to_string()));
            }
        });

        let mut client = Self {
            writer,
            waiters
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
        // TODO:
        let (sender, receiver) = oneshot();
        self.waiters.write().unwrap().insert(seq, sender);
        /*
        await!(ResponseFuture {
            waiters: self.waiters.clone(),
            seq,
        })
        */
        let result = await!(receiver)?;
        Ok(result)
    }

    fn next_seq(&mut self) -> u32 {
        1
    }
}
