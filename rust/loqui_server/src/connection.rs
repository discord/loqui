use std::sync::Arc;

use futures::stream::SplitSink;
use futures::sync::mpsc::{self, UnboundedSender};
use tokio::await as tokio_await;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio_codec::Framed;

use super::{frame_handler::FrameHandler, RequestContext};
use failure::{err_msg, Error};
use loqui_protocol::codec::{LoquiCodec, LoquiFrame};
use futures::sync::oneshot::Sender as OneShotSender;

#[derive(Debug)]
enum Event {
    Frame(LoquiFrame),
    Internal(InternalEvent),
}

#[derive(Debug)]
enum InternalEvent {
    Request {
        payload: Vec<u8>,
        // TODO: probably need to handle error better?
        sender: OneShotSender<Result<Vec<u8>, Error>>,
    },
    Push {
        payload: Vec<u8>,
    },
    Complete(LoquiFrame),
}

pub struct Connection {
    tcp_stream: TcpStream,
    frame_handler: Arc<dyn FrameHandler>,
    encoding: String,
}

impl Connection {
    pub fn new(tcp_stream: TcpStream, frame_handler: Arc<FrameHandler>) -> Self {
        Self {
            tcp_stream,
            frame_handler,
            // TODO:
            encoding: "json".to_string(),
        }
    }

    pub async fn run<'e>(mut self) {
        self = await!(self.upgrade());
        let framed_socket = Framed::new(self.tcp_stream, LoquiCodec::new(50000 * 1000));
        let (mut writer, mut reader) = framed_socket.split();
        // TODO: handle disconnect

        let (tx, rx) = mpsc::unbounded::<Event>();
        let mut stream = reader
            .map(|frame| Event::Frame(frame))
            .select(rx.map_err(|()| err_msg("rx error")));

        while let Some(event) = await!(stream.next()) {
            // TODO: handle error
            match event {
                Ok(event) => {
                    match await!(Self::handle_event(
                        event,
                        writer,
                        tx.clone(),
                        self.frame_handler.clone()
                    )) {
                        Ok(new_writer) => writer = new_writer,
                        Err(e) => {
                            dbg!(e);
                            return;
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

    async fn handle_event(
        event: Event,
        writer: SplitSink<Framed<TcpStream, LoquiCodec>>,
        tx: UnboundedSender<Event>,
        frame_handler: Arc<dyn FrameHandler + 'static>,
    ) -> Result<SplitSink<Framed<TcpStream, LoquiCodec>>, Error> {
        match event {
            Event::Frame(frame) => {
                tokio::spawn_async(
                    async move {
                        // TODO: handle error
                        match tokio_await!(Box::into_pin(frame_handler.handle_frame(frame))) {
                            Ok(Some(frame)) => {
                                tokio_await!(tx.send(Event::Internal(InternalEvent::Complete(frame))));
                            }
                            Ok(None) => {
                                dbg!("None");
                            }
                            Err(e) => {
                                dbg!(e);
                            }
                        }
                    },
                );
                Ok(writer)
            }
            Event::Internal(InternalEvent::Complete(frame)) => {
                match tokio_await!(writer.send(frame)) {
                    Ok(new_writer) => Ok(new_writer),
                    // TODO: better handle this error
                    Err(e) => dbg!(Err(e)),
                }
            }
            event => {
                dbg!(event);
                Ok(writer)
            }
        }
    }

    async fn upgrade(mut self) -> Self {
        // TODO: buffering
        let mut payload = [0; 1024];
        // TODO: handle disconnect, bytes_read=0
        while let Ok(_bytes_read) = await!(self.tcp_stream.read_async(&mut payload)) {
            let request = String::from_utf8(payload.to_vec()).unwrap();
            // TODO: better
            if request.contains(&"upgrade") || request.contains(&"Upgrade") {
                let response =
                    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: loqui\r\nConnection: Upgrade\r\n\r\n";
                await!(self.tcp_stream.write_all_async(&response.as_bytes()[..])).unwrap();
                await!(self.tcp_stream.flush_async()).unwrap();
                break;
            }
        }
        self
    }
}
