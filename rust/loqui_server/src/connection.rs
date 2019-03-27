use crate::error::LoquiError;
use crate::error::LoquiErrorCode;
use crate::ping::{Handle, Ping as PingStream};
use failure::{err_msg, Error};
use futures::oneshot;
use futures::stream::SplitSink;
use futures::sync::mpsc::unbounded;
use futures::sync::mpsc::UnboundedReceiver;
use futures::sync::mpsc::{self, UnboundedSender};
use futures::sync::oneshot::{Receiver as OneShotReceiver, Sender as OneShotSender};
use futures_timer::Interval;
use loqui_protocol::codec::{LoquiCodec, LoquiFrame};
use loqui_protocol::frames::{
    Error as ErrorFrame, GoAway, Hello, Ping, Pong, Push, Request, Response,
};
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio_codec::Framed;

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
    Ready {
        ping_interval: u32,
    },
    Ping,
    Request {
        payload: Vec<u8>,
        waiter_tx: OneShotSender<Result<Vec<u8>, Error>>,
    },
    Push {
        payload: Vec<u8>,
    },
    SendFrame(LoquiFrame),
}

pub type HandleEventResult = Result<Option<LoquiFrame>, Error>;

pub trait FrameHandler: Send + Sync {
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
            .unbounded_send(Event::Request { payload, waiter_tx })?;
        Ok(waiter_rx)
    }

    pub fn push(&self, payload: Vec<u8>) -> Result<(), Error> {
        self.tx
            .unbounded_send(Event::Push { payload })
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
            .unbounded_send(Event::SendFrame(LoquiFrame::Hello(Hello {
                // TODO
                flags: 0,
                // TODO
                version: 0,
                encodings: vec!["json".to_string()],
                // TODO
                compressions: vec![],
            })))
            .map_err(Error::from)
    }

    pub fn response(&self, payload: Vec<u8>, sequence_id: u32, flags: u8) -> Result<(), Error> {
        self.frame(LoquiFrame::Response(Response {
            flags,
            sequence_id,
            payload,
        }))
    }

    pub fn error(
        &self,
        sequence_id: u32,
        code: u16,
        flags: u8,
        payload: Vec<u8>,
    ) -> Result<(), Error> {
        self.frame(LoquiFrame::Error(ErrorFrame {
            flags,
            sequence_id,
            code,
            payload,
        }))
    }

    fn frame(&self, frame: LoquiFrame) -> Result<(), Error> {
        self.tx
            .unbounded_send(Event::SendFrame(frame))
            .map_err(Error::from)
    }
}

pub struct Connection {
    tcp_stream: TcpStream,
    self_rx: UnboundedReceiver<Event>,
    self_sender: ConnectionSender,
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
                // TODO: these prob shouldn't be set??
            },
        )
    }

    pub fn spawn(self, frame_handler: Box<dyn FrameHandler + 'static>) {
        tokio::spawn_async(
            async move {
                match await!(self.run(frame_handler)) {
                    Ok(()) => {}
                    Err(e) => {
                        if let Some(e) = e.downcast_ref::<io::Error>() {
                            println!("Connection closed. error_kind={:?}", e.kind())
                        } else {
                            println!("Connection closed. error={:?}", e)
                        }
                    }
                }
            },
        );
    }

    async fn run(
        mut self,
        mut frame_handler: Box<dyn FrameHandler + 'static>,
    ) -> Result<(), Error> {
        self.tcp_stream = await!(Box::into_pin(frame_handler.upgrade(self.tcp_stream)))
            .expect("Failed to upgrade");
        let framed_socket = Framed::new(self.tcp_stream, LoquiCodec::new(50000 * 1000));
        let (mut writer, reader) = framed_socket.split();
        // TODO: handle disconnect

        let mut ping_stream = PingStream::new();

        let mut inner = EventHandler::new(ping_stream.handle(), frame_handler);

        let mut stream = reader
            // TODO: we might want to separate out the ping channel so it doesn't get backed up
            .map(|frame| Event::SocketReceive(frame))
            // TODO: maybe buffer unordered so we don't have to spawn and send back?
            .select(self.self_rx.map_err(|()| err_msg("rx error")))
            .select(ping_stream);

        while let Some(event) = await!(stream.next()) {
            // TODO: handle error
            //dbg!(&event);
            match event {
                Ok(event) => match inner.handle_event(event) {
                    Ok(Some(frame)) => {
                        writer = await!(writer.send(frame))?;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        dbg!(e);
                    }
                },
                Err(e) => {
                    dbg!(e);
                }
            }
        }
        Err(err_msg("Unreachable"))
    }
}

pub struct EventHandler {
    frame_handler: Box<dyn FrameHandler>,
    ping_handle: Handle,
    pong_received: bool,
    sequencer: Sequencer,
}

type MaybeFrameResult = Result<Option<LoquiFrame>, Error>;

impl EventHandler {
    fn new(ping_handle: Handle, frame_handler: Box<dyn FrameHandler>) -> Self {
        Self {
            frame_handler,
            ping_handle,
            pong_received: true,
            sequencer: Sequencer::new(),
        }
    }

    fn handle_ready(&mut self, ping_interval: u32) -> MaybeFrameResult {
        self.ping_handle.start(ping_interval);
        Ok(None)
    }

    fn handle_ping(&mut self) -> MaybeFrameResult {
        if self.pong_received {
            let sequence_id = self.sequencer.next();
            let frame = LoquiFrame::Ping(Ping {
                sequence_id,
                flags: 0,
            });
            self.pong_received = false;
            Ok(Some(frame))
        } else {
            Err(LoquiError::PingTimeout.into())
        }
    }

    fn handle_ping_frame(&self, ping: Ping) -> MaybeFrameResult {
        let pong = Pong {
            flags: ping.flags,
            sequence_id: ping.sequence_id,
        };
        Ok(Some(LoquiFrame::Pong(pong)))
    }

    fn handle_pong_frame(&mut self, pong: Pong) -> MaybeFrameResult {
        self.pong_received = true;
        Ok(None)
    }

    fn handle_request(
        &mut self,
        payload: Vec<u8>,
        waiter_tx: OneShotSender<Result<Vec<u8>, Error>>,
    ) -> MaybeFrameResult {
        let sequence_id = self.sequencer.next();
        let frame = LoquiFrame::Request(Request {
            payload,
            sequence_id,
            // TODO
            flags: 0,
        });
        self.frame_handler.handle_sent(sequence_id, waiter_tx);
        Ok(Some(frame))
    }

    fn handle_push(&self, payload: Vec<u8>) -> MaybeFrameResult {
        let frame = LoquiFrame::Push(Push { payload, flags: 0 });
        Ok(Some(frame))
    }

    fn handle_event(&mut self, event: Event) -> Result<Option<LoquiFrame>, Error> {
        match event {
            Event::Ready { ping_interval } => self.handle_ready(ping_interval),
            Event::Ping => self.handle_ping(),
            Event::SocketReceive(frame) => match frame {
                LoquiFrame::Ping(ping) => self.handle_ping_frame(ping),
                LoquiFrame::Pong(pong) => self.handle_pong_frame(pong),
                LoquiFrame::GoAway(goaway) => Err(LoquiError::GoAway.into()),
                frame => self.frame_handler.handle_received(frame),
            },
            Event::Request { payload, waiter_tx } => self.handle_request(payload, waiter_tx),
            Event::Push { payload } => self.handle_push(payload),
            Event::SendFrame(frame) => Ok(Some(frame)),
        }
    }
}
