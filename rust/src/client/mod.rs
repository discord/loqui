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
use futures::sync::mpsc;
use futures::oneshot;
use futures::stream::SplitSink;
use futures::sync::oneshot::{Sender as OneShotSender};
use tokio::prelude::*;
use tokio_codec::Framed;

use crate::protocol::codec::LoquiCodec;
use crate::protocol::codec::LoquiFrame;

const UPGRADE_REQUEST: &'static str =
    "GET #{loqui_path} HTTP/1.1\r\nHost: #{host}\r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";

pub struct Client {
    //reader: ReadHalf<TcpStream>,
    //writer: SplitSink<Framed<TcpStream, LoquiCodec>>,
    sender: mpsc::UnboundedSender<Message>,
    // TODO: should probably sweep these
}

#[derive(Debug)]
enum Message {
    Request(u32, OneShotSender<String>),
    Response(LoquiFrame)
}

impl Client {
    pub async fn connect<A: AsRef<str>>(address: A) -> Result<Client, Error> {
        let addr: SocketAddr = address.as_ref().parse()?;
        let socket = await!(TcpStream::connect(&addr))?;
        let framed_socket = Framed::new(socket, LoquiCodec::new(50000));
        let (mut writer, mut reader) = framed_socket.split();
        let (tx, mut rx) = mpsc::unbounded::<Message>();

        // read task
        tokio::spawn_async(async move {
            let mut waiters: HashMap<u32, OneShotSender<String>> = HashMap::new();

            let mut total = reader.map(|frame| Message::Response(frame)).select(rx.map_err(|()| err_msg("rx error")));

            // TOOD: loop?
            while let Some(item) = await!(total.next()) {//.select(reader) {
                match item {
                    Ok(Message::Request(seq, sender)) => {
                        waiters.insert(seq, sender);
                        writer = await!(writer.send(LoquiFrame::Ping(crate::protocol::frames::Ping {flags: 2, sequence_id: 1}))).unwrap();
                    },
                    Ok(Message::Response(frame)) => {
                        dbg!(&frame);
                        if let LoquiFrame::Ping(ping) = frame {
                            let sender = waiters.remove(&ping.sequence_id).unwrap();
                            sender.send("ping back".to_string());
                        }
                    },
                    Err(e) => {
                        dbg!(e);
                    }
                }
            }
            println!("client exit");
        });

        let mut client = Self {
            sender: tx,
            //writer,
        };

        await!(client.upgrade())?;
        Ok(client)
    }

    async fn write<'a>(&'a mut self, data: &'a [u8]) -> Result<(), Error> {
        //await!(self.writer.send(LoquiFrame::Ping(crate::protocol::frames::Ping {flags: 0, sequence_id: 0})))?;
        //await!(self.writer.flush_async())?;
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
        self.sender.unbounded_send(Message::Request(seq, sender))?;
        let result = await!(receiver)?;
        dbg!(&result);
        Ok(result)
    }

    fn next_seq(&mut self) -> u32 {
        1
    }
}
