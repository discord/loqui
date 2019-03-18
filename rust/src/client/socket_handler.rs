use std::collections::HashMap;

use failure::{err_msg, Error};
use futures::sync::mpsc::UnboundedReceiver;
use futures::sync::oneshot::Sender as OneShotSender;
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio_codec::Framed;

use crate::protocol::codec::LoquiCodec;
use crate::protocol::codec::LoquiFrame;
use crate::protocol::frames::*;

#[derive(Debug)]
pub enum Message {
    Request {
        payload: Vec<u8>,
        // TODO: probably need to handle error better?
        sender: OneShotSender<Result<Vec<u8>, Error>>,
    },
    Push {
        payload: Vec<u8>,
    },
    Response(LoquiFrame),
}

struct MessageHandler {
    waiters: HashMap<u32, OneShotSender<Result<Vec<u8>, Error>>>,
    next_seq: u32,
}

impl MessageHandler {
    fn new() -> Self {
        Self {
            // TODO: should probably sweep these, probably request timeout
            waiters: HashMap::new(),
            next_seq: 1,
        }
    }

    fn next_seq(&mut self) -> u32 {
        let seq = self.next_seq;
        self.next_seq += 1;
        seq
    }

    fn handle(&mut self, message: Message) -> Option<LoquiFrame> {
        match message {
            Message::Request { payload, sender } => {
                let sequence_id = self.next_seq();
                self.waiters.insert(sequence_id, sender);
                Some(LoquiFrame::Request(Request {
                    sequence_id,
                    flags: 0,
                    payload,
                }))
            }
            Message::Push { payload } => {
                println!("push {:?}", payload);
                Some(LoquiFrame::Push(Push { flags: 0, payload }))
            }
            Message::Response(frame) => {
                match frame {
                    LoquiFrame::Response(Response {
                        flags,
                        sequence_id,
                        payload,
                    }) => {
                        let sender = self.waiters.remove(&sequence_id).unwrap();
                        sender.send(Ok(payload));
                    }
                    frame => {
                        dbg!(frame);
                    }
                }
                None
            }
        }
    }
}

pub struct SocketHandler {}

impl SocketHandler {
    pub async fn run(socket: TcpStream, rx: UnboundedReceiver<Message>) {
        let framed_socket = Framed::new(socket, LoquiCodec::new(50000));
        let (mut writer, mut reader) = framed_socket.split();
        let mut message_handler = MessageHandler::new();

        writer = await!(writer.send(LoquiFrame::Hello(Hello {
            flags: 0,
            version: 1,
            encodings: vec!["msgpack".to_string()],
            compressions: vec![],
        })))
        .unwrap();

        let mut stream = reader
            .map(|frame| Message::Response(frame))
            .select(rx.map_err(|()| err_msg("rx error")));

        while let Some(message) = await!(stream.next()) {
            match message {
                Ok(message) => {
                    if let Some(frame) = message_handler.handle(message) {
                        // TODO: should probably batch and send_all
                        // https://docs.rs/futures/0.1.5/futures/sink/trait.Sink.html#method.send_all
                        writer = await!(writer.send(frame)).unwrap();
                    }
                }
                Err(e) => {
                    dbg!(e);
                }
            }
        }
        println!("client exit");
    }
}
