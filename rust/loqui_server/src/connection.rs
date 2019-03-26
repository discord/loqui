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
use loqui_protocol::frames::{GoAway, Hello, Ping, Pong, Push, Request, Response};
use std::future::Future;
use std::time::{Duration, Instant};
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio_codec::Framed;

// TODO: get right values
const UPGRADE_REQUEST: &'static str =
    "GET /_rpc HTTP/1.1\r\nHost: 127.0.0.1 \r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";

struct Sequencer {
    next: u32,
}

impl Sequencer {
    fn new() -> Self {
        Self { next: 1 }
    }

    fn next(&mut self) -> u32 {
        let seq = self.next;
        self.next += 1;
        seq
    }
}

#[derive(Debug)]
pub enum Event {
    SocketReceive(LoquiFrame),
    Ready { ping_interval: u32 },
    Ping,
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

    pub fn ready(&self, ping_interval: u32) -> Result<(), Error> {
        self.tx
            .unbounded_send(Event::Ready { ping_interval })
            .map_err(Error::from)
    }

    fn ping(&self) -> Result<(), Error> {
        self.tx.unbounded_send(Event::Ping).map_err(Error::from)
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
    pong_received: bool,
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
                // TODO: these prob shouldn't be set??
                pong_received: true,
            },
        )
    }

    pub fn spawn(mut self, event_handler: Box<dyn EventHandler + 'static>) {
        tokio::spawn_async(
            async move {
                let result = await!(self.run(event_handler));
                println!("Connection exit. result={:?}", result);
            },
        );
    }

    fn spawn_ping(ping_interval: Duration, connection_sender: ConnectionSender) {
        tokio::spawn_async(
            async move {
                let mut stream = Interval::new(ping_interval);
                while let Some(Ok(())) = await!(stream.next()) {
                    if let Err(e) = connection_sender.ping() {
                        println!("Connection closed.");
                        return;
                    }
                }
            },
        );
    }

    async fn run(
        mut self,
        mut event_handler: Box<dyn EventHandler + 'static>,
    ) -> Result<(), Error> {
        self.tcp_stream = await!(Box::into_pin(event_handler.upgrade(self.tcp_stream)))
            .expect("Failed to upgrade");
        let framed_socket = Framed::new(self.tcp_stream, LoquiCodec::new(50000 * 1000));
        let (mut writer, mut reader) = framed_socket.split();
        // TODO: handle disconnect

        let mut stream = reader
            // TODO: we might want to separate out the ping channel so it doesn't get backed up
            .map(|frame| Event::SocketReceive(frame))
            // TODO: maybe buffer unordered so we don't have to spawn and send back?
            .select(self.self_rx.map_err(|()| err_msg("rx error")));

        while let Some(event) = await!(stream.next()) {
            // TODO: handle error
            //dbg!(&event);
            match event {
                Ok(event) => {
                    match event {
                        Event::Ready { ping_interval } => {
                            // TODO: code cleanup
                            let sequence_id = self.sequencer.next();
                            let frame = LoquiFrame::Ping(Ping {
                                sequence_id,
                                flags: 0,
                            });
                            writer = await!(writer.send(frame))?;
                            self.pong_received = false;
                            Connection::spawn_ping(
                                Duration::from_millis(ping_interval as u64),
                                self.self_sender.clone(),
                            );
                        }
                        Event::Ping => {
                            let sequence_id = self.sequencer.next();
                            let frame = LoquiFrame::Ping(Ping {
                                sequence_id,
                                flags: 0,
                            });
                            writer = await!(writer.send(frame))?;
                            if dbg!(!self.pong_received) {
                                // TODO: tell them it's due to ping timeout
                                let frame = LoquiFrame::GoAway(GoAway {
                                    flags: 0,
                                    code: 0,
                                    payload: vec![],
                                });
                                // TODO
                                writer = await!(writer.send(frame))?;
                                // TODO
                                return Ok(());
                            }
                            self.pong_received = false;
                        }
                        Event::SocketReceive(frame) => {
                            match frame {
                                LoquiFrame::Ping(ping) => {
                                    let pong = Pong {
                                        flags: ping.flags,
                                        sequence_id: ping.sequence_id,
                                    };
                                    let frame = LoquiFrame::Pong(pong);
                                    writer = await!(writer.send(frame))?;
                                }
                                LoquiFrame::Pong(pong) => {
                                    self.pong_received = true;
                                }
                                LoquiFrame::GoAway(goaway) => {
                                    // TODO
                                    // TODO: also clean up the pinger
                                    println!("Told to go away! {:?}", goaway);
                                    // TODO
                                    return Ok(());
                                }
                                frame => match event_handler.handle_received(frame) {
                                    Ok(Some(frame)) => {
                                        writer = await!(writer.send(frame))?;
                                    }
                                    Ok(None) => {}
                                    Err(e) => {
                                        return Err(Error::from(e));
                                    }
                                },
                            }
                        }
                        Event::Forward(forward_request) => {
                            match forward_request {
                                Forward::Request { payload, waiter_tx } => {
                                    let sequence_id = self.sequencer.next();
                                    let frame = LoquiFrame::Request(Request {
                                        payload,
                                        sequence_id,
                                        // TODO
                                        flags: 0,
                                    });
                                    writer = await!(writer.send(frame))?;
                                    event_handler.handle_sent(sequence_id, waiter_tx);
                                }
                                Forward::Push { payload } => {
                                    let sequence_id = self.sequencer.next();
                                    let frame = LoquiFrame::Push(Push { payload, flags: 0 });
                                    writer = await!(writer.send(frame))?;
                                }
                                Forward::Frame(frame) => {
                                    writer = await!(writer.send(frame))?;
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
        Err(err_msg("Unreachable"))
    }
}
