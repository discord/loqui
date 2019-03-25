use std::sync::Arc;

use failure::{err_msg, Error};
use futures::oneshot;
use futures::stream::SplitSink;
use futures::sync::mpsc::unbounded;
use futures::sync::mpsc::UnboundedReceiver;
use futures::sync::mpsc::{self, UnboundedSender};
use futures::sync::oneshot::{Receiver as OneShotReceiver, Sender as OneShotSender};
use futures_timer::Interval;
use loqui_protocol::codec::{LoquiCodec, LoquiFrame};
use loqui_protocol::frames::{Hello, Ping, Pong, Push, Request, Response};
use std::future::Future;
use std::time::Duration;
use tokio::await as tokio_await;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio_codec::Framed;

// TODO: get right values
const UPGRADE_REQUEST: &'static str =
    "GET /_rpc HTTP/1.1\r\nHost: 127.0.0.1 \r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";

struct Sequencer {
    next_seq: u32,
}

// TODO: maybe make this an iterator?
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

#[derive(Clone)]
pub struct ConnectionSender {
    tx: UnboundedSender<Event>,
}

impl ConnectionSender {
    fn new() -> (Self, UnboundedReceiver<Event>) {
        let (tx, rx) = mpsc::unbounded();
        (Self { tx }, rx)
    }

    pub fn request(
        &self,
        payload: Vec<u8>,
    ) -> Result<OneShotReceiver<Result<Vec<u8>, Error>>, Error> {
        let (waiter_tx, waiter_rx) = oneshot();
        self.tx
            .unbounded_send(Event::Forward(Forward::Request { payload, waiter_tx }))?;
        Ok(waiter_rx)
    }

    pub fn push(&self, payload: Vec<u8>) -> Result<(), Error> {
        self.tx
            .unbounded_send(Event::Forward(Forward::Push { payload }))
            .map_err(Error::from)
    }

    fn ping(&self) -> Result<(), Error> {
        self.tx
            .unbounded_send(Event::Forward(Forward::Frame(LoquiFrame::Ping(Ping {
                // TODO
                sequence_id: 0,
                flags: 0,
            }))))
            .map_err(Error::from)
    }

    pub fn hello(&self) -> Result<(), Error> {
        self.tx
            .unbounded_send(Event::Forward(Forward::Frame(LoquiFrame::Hello(Hello {
                // TODO
                flags: 0,
                // TODO
                version: 0,
                encodings: vec!["json".to_string()],
                // TODO
                compressions: vec![],
            }))))
            .map_err(Error::from)
    }

    pub fn frame(&self, frame: LoquiFrame) -> Result<(), Error> {
        self.tx
            .unbounded_send(Event::Forward(Forward::Frame(frame)))
            .map_err(Error::from)
    }
}

pub struct Connection {
    tcp_stream: TcpStream,
    self_rx: UnboundedReceiver<Event>,
    self_sender: ConnectionSender,
    sequencer: Sequencer,
}

impl Connection {
    pub fn new(tcp_stream: TcpStream) -> (ConnectionSender, Self) {
        let (self_sender, self_rx) = ConnectionSender::new();
        (
            self_sender.clone(),
            Self {
                self_sender,
                self_rx,
                tcp_stream,
                sequencer: Sequencer::new(),
            },
        )
    }

    pub fn spawn(mut self, event_handler: Box<dyn EventHandler + 'static>) {
        tokio::spawn_async(self.run(event_handler));
    }

    fn spawn_ping(connection_sender: ConnectionSender) {
        tokio::spawn_async(
            async move {
                // TODO: from hello ack
                // TODO: need to timeout from pong
                let mut interval = Interval::new(Duration::from_secs(3));
                while let Some(Ok(())) = tokio_await!(interval.next()) {
                    connection_sender.ping().expect("failed to ping");
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

        Connection::spawn_ping(self.self_sender.clone());

        let mut stream = reader
            // TODO: we might want to separate out the ping channel so it doesn't get backed up
            .map(|frame| Event::SocketReceive(frame))
            .select(self.self_rx.map_err(|()| err_msg("rx error")));

        while let Some(event) = await!(stream.next()) {
            // TODO: handle error
            //dbg!(&event);
            match event {
                Ok(event) => {
                    match event {
                        Event::SocketReceive(frame) => {
                            match frame {
                                LoquiFrame::Ping(ping) => {
                                    let pong = Pong {
                                        flags: ping.flags,
                                        sequence_id: ping.sequence_id,
                                    };
                                    let frame = LoquiFrame::Pong(pong);
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
                                LoquiFrame::Pong(pong) => {
                                    // TODO
                                    dbg!(pong);
                                }
                                frame => {
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
