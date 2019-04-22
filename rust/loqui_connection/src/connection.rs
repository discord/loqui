use crate::encoder::Factory;
use crate::event_handler::EventHandler;
use crate::framed_io::ReaderWriter;
use crate::handler::{Handler, Ready};
use crate::select_break::StreamExt;
use crate::sender::Sender;
use crate::LoquiError;
use failure::Error;
use futures::future::Future;
use futures::sync::mpsc::UnboundedReceiver;
use futures::sync::oneshot;
use futures_timer::FutureExt;
use futures_timer::Interval;
use loqui_protocol::frames::{LoquiFrame, Response};
use std::time::Instant;
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio_async_await::compat::backward::Compat;

#[derive(Debug)]
pub struct Connection<H: Handler> {
    self_sender: Sender<H::InternalEvent>,
}

impl<H: Handler> Connection<H> {
    /// Spawn a new `Connection` that runs in a separate task. Returns a handle for sending to
    /// the `Connection`.
    ///
    /// # Arguments
    ///
    /// * `tcp_stream` - the tcp socket
    /// * `handler` - implements client or server specific logic
    /// * `handshake_deadline` - how long until we fail due to handshake not completing
    /// * `ready_tx` - a sender used to notify that the connection is ready for requests
    pub fn spawn(
        tcp_stream: TcpStream,
        handler: H,
        handshake_deadline: Instant,
        ready_tx: Option<oneshot::Sender<()>>,
    ) -> Self {
        let (self_sender, self_rx) = Sender::new();
        let connection = Self {
            self_sender: self_sender.clone(),
        };
        tokio::spawn_async(
            async move {
                let result = await!(run(
                    tcp_stream,
                    self_sender,
                    self_rx,
                    handler,
                    handshake_deadline,
                    ready_tx
                ));
                if let Err(e) = result {
                    error!("Connection closed. error={:?}", e)
                }
            },
        );
        connection
    }

    pub fn send(&self, event: H::InternalEvent) -> Result<(), Error> {
        self.self_sender.internal(event)
    }

    pub fn close(&self) -> Result<(), Error> {
        self.self_sender.close()
    }

    pub fn is_closed(&self) -> bool {
        self.self_sender.is_closed()
    }
}

/// The events that can be received by the core connection loop once it begins running.
#[derive(Debug)]
pub enum Event<InternalEvent: Send + 'static> {
    /// A full frame was received on the socket.
    SocketReceive(LoquiFrame),
    /// A ping should be sent.
    Ping,
    /// Generic event that will be delegated to the connection handler.
    InternalEvent(InternalEvent),
    /// A response for a request was computed and should be sent back over the socket.
    ResponseComplete(Result<Response, (Error, u32)>),
    /// Close the connection gracefully.
    Close,
}

/// The core run loop for a connection.
/// Negotiates the connection then handles events until the socket dies or there is an error.
///
/// # Arguments
///
/// * `tcp_stream` - the tcp socket
/// * `self_sender` - a sender that is used to for the connection to enqueue an event to itself.
///                   This is used when a response for a request is computed asynchronously in a task.
/// * `self_rx` - a receiver that InternalEvents will be sent over
/// * `handler` - implements logic for the client or server specific things
/// * `handshake_deadline` - how long until we fail due to handshake not completing
/// * `ready_tx` - a sender used to notify that the connection is ready for requests
async fn run<H: Handler>(
    tcp_stream: TcpStream,
    self_sender: Sender<H::InternalEvent>,
    self_rx: UnboundedReceiver<Event<H::InternalEvent>>,
    handler: H,
    handshake_deadline: Instant,
    ready_tx: Option<oneshot::Sender<()>>,
) -> Result<(), Error> {
    let (ready, reader_writer, handler) =
        await!(negotiate(tcp_stream, handler, ready_tx).timeout_at(handshake_deadline))?;
    debug!("Ready. {:?}", ready);
    let (reader, mut writer) = reader_writer.split();
    let encoder =
        H::EncoderFactory::make(ready.encoding).ok_or_else(|| LoquiError::InvalidEncoding)?;

    // Convert each stream into a Result<Event, Error> stream.
    let ping_stream = Interval::new(ready.ping_interval)
        .map(|()| Event::Ping)
        .map_err(Error::from);
    let framed_reader = reader.map(Event::SocketReceive);
    let self_rx = self_rx.map_err(|()| LoquiError::EventReceiveError.into());

    let mut stream = framed_reader
        .select_break(self_rx)
        .select_break(ping_stream);

    let mut event_handler = EventHandler::new(self_sender, handler, encoder);
    while let Some(event) = await!(stream.next()) {
        let event = event?;

        match event_handler.handle_event(event) {
            Ok(Some(frame)) => writer = await!(writer.write(frame))?,
            Ok(None) => {}
            Err(error) => {
                await!(writer.close(Some(&error), None));
                return Ok(());
            }
        }
    }

    Err(LoquiError::ConnectionClosed.into())
}

/// Negotiates the connection.
///
/// # Arguments
///
/// * `tcp_stream` - the tcp socket
/// * `handler` - implements logic for the client or server specific things
/// * `ready_tx` - a sender used to notify that the connection is ready for requests
fn negotiate<H: Handler>(
    tcp_stream: TcpStream,
    mut handler: H,
    ready_tx: Option<oneshot::Sender<()>>,
) -> impl Future<Item = (Ready, ReaderWriter, H), Error = Error> {
    Compat::new(
        async move {
            let tcp_stream = await!(handler.upgrade(tcp_stream))?;
            let max_payload_size = handler.max_payload_size();
            let reader_writer = ReaderWriter::new(tcp_stream, max_payload_size, H::SEND_GO_AWAY);

            match await!(handler.handshake(reader_writer)) {
                Ok((ready, reader_writer)) => {
                    if let Some(ready_tx) = ready_tx {
                        ready_tx
                            .send(())
                            .map_err(|()| Error::from(LoquiError::ReadySendFailed))?;
                    }
                    Ok((ready, reader_writer, handler))
                }
                Err((error, reader_writer)) => {
                    debug!("Not ready. e={:?}", error);
                    if let Some(reader_writer) = reader_writer {
                        await!(reader_writer.close(Some(&error)));
                    }
                    Err(error)
                }
            }
        },
    )
}
