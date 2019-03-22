use std::sync::Arc;

use futures::stream::SplitSink;
use futures::sync::mpsc::{self, UnboundedSender};
use std::future::Future;
use tokio::await as tokio_await;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio_codec::Framed;

use super::{frame_handler::FrameHandler, RequestContext};
use failure::{err_msg, Error};
use futures::sync::oneshot::Sender as OneShotSender;
use loqui_protocol::codec::{LoquiCodec, LoquiFrame};

#[derive(Debug)]
pub enum Event<E> {
    Socket(LoquiFrame),
    Internal(E),
}

pub type HandleEventResult = Box<dyn Future<Output = Result<Option<LoquiFrame>, Error>> + Send>;

pub trait EventHandler<E>: Send + Sync {
    fn handle_event(&self, event: Event<E>, tx: UnboundedSender<Event<E>>) -> HandleEventResult;
}

pub struct Connection<E> {
    tcp_stream: TcpStream,
    //frame_handler: Arc<dyn FrameHandler>,
    event_handler: Arc<dyn EventHandler<E>>,
    encoding: String,
}

impl<E> Connection<E> {
    pub fn new(tcp_stream: TcpStream, event_handler: Arc<dyn EventHandler<E>>) -> Self {
        Self {
            tcp_stream,
            event_handler,
            // TODO:
            encoding: "json".to_string(),
        }
    }

    pub async fn run<'e>(mut self) {
        self = await!(self.await_upgrade());
        let framed_socket = Framed::new(self.tcp_stream, LoquiCodec::new(50000 * 1000));
        let (mut writer, mut reader) = framed_socket.split();
        // TODO: handle disconnect

        let (tx, rx) = mpsc::unbounded::<Event<E>>();
        let mut stream = reader
            .map(|frame| Event::Socket(frame))
            .select(rx.map_err(|()| err_msg("rx error")));

        while let Some(event) = await!(stream.next()) {
            // TODO: handle error
            match event {
                Ok(event) => {
                    match await!(Box::into_pin(self.event_handler.handle_event(event, tx.clone()))) {
                        Ok(Some(frame)) => {
                            match tokio_await!(writer.send(frame)) {
                                Ok(new_writer) => writer = new_writer,
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
                Err(e) => {
                    dbg!(e);
                }
            }
        }
        println!("connection closed");
    }

    pub async fn await_upgrade(mut self) -> Self {
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
