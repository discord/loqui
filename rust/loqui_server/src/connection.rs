use std::sync::Arc;

use futures::stream::SplitSink;
use futures::sync::mpsc::{self, UnboundedSender};
use futures_timer::Interval;
use std::future::Future;
use std::time::Duration;
use tokio::await as tokio_await;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio_codec::Framed;

use failure::{err_msg, Error};
use futures::sync::mpsc::UnboundedReceiver;
use futures::sync::oneshot::Sender as OneShotSender;
use loqui_protocol::codec::{LoquiCodec, LoquiFrame};
use loqui_protocol::frames::{Hello, Ping, Push, Request};

// TODO: get right values
const UPGRADE_REQUEST: &'static str =
    "GET /_rpc HTTP/1.1\r\nHost: 127.0.0.1 \r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";

struct Sequencer {
    next_seq: u32,
}

impl Sequencer {
    fn new() -> Self {
        Self { next_seq: 1 }
    }

    fn next_seq(&mut self) -> u32 {
        let seq = self.next_seq;
        self.next_seq += 1;
        seq
    }
}

#[derive(Debug)]
pub enum Event {
    SocketReceive(LoquiFrame),
    Forward(Forward),
}

#[derive(Debug)]
pub enum Forward {
    Request {
        payload: Vec<u8>,
        waiter_tx: OneShotSender<Result<Vec<u8>, Error>>,
    },
    Push {
        payload: Vec<u8>,
    },
    Frame(LoquiFrame),
}

pub type HandleEventResult = Result<Option<LoquiFrame>, Error>;

pub trait EventHandler: Send + Sync {
    fn upgrade(
        &self,
        tcp_stream: TcpStream,
    ) -> Box<dyn Future<Output = Result<TcpStream, Error>> + Send>;
    fn handle_received(&mut self, frame: LoquiFrame) -> HandleEventResult;
    fn handle_sent(&mut self, sequence_id: u32, waiter_tx: OneShotSender<Result<Vec<u8>, Error>>);
}

pub struct Connection {
    tcp_stream: TcpStream,
    self_rx: UnboundedReceiver<Event>,
    self_tx: UnboundedSender<Event>,
    sequencer: Sequencer,
}

use loqui_protocol::Response;
use std::fmt::Debug;

impl Connection {
    pub fn new(tcp_stream: TcpStream) -> (UnboundedSender<Event>, Self) {
        let (self_tx, self_rx) = mpsc::unbounded();
        (
            self_tx.clone(),
            Self {
                self_tx,
                self_rx,
                tcp_stream,
                sequencer: Sequencer::new(),
            },
        )
    }

    pub fn spawn(mut self, event_handler: Box<dyn EventHandler + 'static>) {
        tokio::spawn_async(self.run(event_handler));
    }

    fn spawn_ping(self_tx: UnboundedSender<Event>) {
        tokio::spawn_async(
            async move {
                let mut interval = Interval::new(Duration::from_secs(3));
                while let Some(result) = tokio_await!(interval.next()) {
                    println!("result {:?}", result);
                    let self_tx = self_tx.clone();
                    tokio_await!(self_tx.send(Event::Forward(Forward::Frame(LoquiFrame::Ping(
                        Ping {
                            flags: 0,
                            // TODO
                            sequence_id: 1
                        }
                    )))))
                    .expect("We need to handle this so we dont do this forever");
                }
            },
        );
    }

    async fn run(mut self, mut event_handler: Box<dyn EventHandler + 'static>) {
        self.tcp_stream = await!(Box::into_pin(event_handler.upgrade(self.tcp_stream)))
            .expect("Failed to upgrade");
        let framed_socket = Framed::new(self.tcp_stream, LoquiCodec::new(50000 * 1000));
        let (mut writer, mut reader) = framed_socket.split();
        // TODO: handle disconnect

        Connection::spawn_ping(self.self_tx.clone());

        let mut stream = reader
            .map(|frame| Event::SocketReceive(frame))
            .select(self.self_rx.map_err(|()| err_msg("rx error")));

        while let Some(event) = await!(stream.next()) {
            // TODO: handle error
            //dbg!(&event);
            match event {
                Ok(event) => {
                    match event {
                        Event::SocketReceive(frame) => {
                            match event_handler.handle_received(frame) {
                                Ok(Some(frame)) => {
                                    match tokio_await!(writer.send(frame)) {
                                        Ok(new_writer) => {
                                            writer = new_writer;
                                        }
                                        // TODO: better handle this error
                                        Err(e) => {
                                            // TODO
                                            dbg!(e);
                                            return;
                                        }
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    dbg!(e);
                                    return;
                                }
                            }
                        }
                        Event::Forward(forward_request) => {
                            match forward_request {
                                Forward::Request { payload, waiter_tx } => {
                                    let sequence_id = self.sequencer.next_seq();
                                    let frame = LoquiFrame::Request(Request {
                                        payload,
                                        sequence_id,
                                        // TODO
                                        flags: 0,
                                    });
                                    match tokio_await!(writer.send(frame)) {
                                        Ok(new_writer) => {
                                            writer = new_writer;
                                            event_handler.handle_sent(sequence_id, waiter_tx)
                                        }
                                        // TODO: better handle this error
                                        Err(e) => {
                                            // TODO
                                            dbg!(e);
                                            return;
                                        }
                                    }
                                }
                                Forward::Push { payload } => {
                                    let sequence_id = self.sequencer.next_seq();
                                    let frame = LoquiFrame::Push(Push { payload, flags: 0 });
                                    match tokio_await!(writer.send(frame)) {
                                        Ok(new_writer) => {
                                            writer = new_writer;
                                        }
                                        // TODO: better handle this error
                                        Err(e) => {
                                            // TODO
                                            dbg!(e);
                                            return;
                                        }
                                    }
                                }
                                Forward::Frame(frame) => {
                                    match tokio_await!(writer.send(frame)) {
                                        Ok(new_writer) => {
                                            writer = new_writer;
                                        }
                                        // TODO: better handle this error
                                        Err(e) => {
                                            // TODO
                                            dbg!(e);
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    dbg!(e);
                }
            }
        }
        println!("connection closed");
    }
}
