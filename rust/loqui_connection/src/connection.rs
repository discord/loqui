use crate::event_handler::EventHandler;
use crate::framed_io::FramedReaderWriter;
use crate::handler::Handler;
use crate::sender::Sender;
use crate::LoquiError;
use failure::Error;
use futures::sync::mpsc::UnboundedReceiver;
use futures::sync::oneshot;
use futures_timer::Interval;
use loqui_protocol::frames::{LoquiFrame, Response};
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;

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
    /// * `ready_tx` - a sender used to notify that the connection is ready for requests
    pub fn spawn(tcp_stream: TcpStream, handler: H, ready_tx: Option<oneshot::Sender<()>>) -> Self {
        let (self_sender, self_rx) = Sender::new();
        let connection = Self {
            self_sender: self_sender.clone(),
        };
        tokio::spawn_async(
            async move {
                let result = await!(run(tcp_stream, self_sender, self_rx, handler, ready_tx));
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

    pub fn close(&self, go_away: bool) -> Result<(), Error> {
        self.self_sender.close(go_away)
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
    Close { go_away: bool },
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
/// * `ready_tx` - a sender used to notify that the connection is ready for requests
async fn run<H: Handler>(
    tcp_stream: TcpStream,
    self_sender: Sender<H::InternalEvent>,
    self_rx: UnboundedReceiver<Event<H::InternalEvent>>,
    mut handler: H,
    ready_tx: Option<oneshot::Sender<()>>,
) -> Result<(), Error> {
    let tcp_stream = await!(handler.upgrade(tcp_stream))?;
    let max_payload_size = handler.max_payload_size();
    let reader_writer = FramedReaderWriter::new(tcp_stream, max_payload_size);

    let (ready, reader_writer) = match await!(handler.handshake(reader_writer)) {
        Ok((ready, reader_writer)) => (ready, reader_writer),
        Err((error, reader_writer)) => {
            // TODO: send the ready error to ready_tx?
            debug!("Not ready. e={:?}", error);
            if let Some(reader_writer) = reader_writer {
                let (_reader, writer) = reader_writer.split();
                await!(writer.close(Some(error)));
            }
            // TODO:
            return Err(LoquiError::TcpStreamIntentionalClose.into());
        }
    };

    let (reader, mut writer) = reader_writer.split();
    debug!("Ready. {:?}", ready);
    if let Some(ready_tx) = ready_tx {
        ready_tx
            .send(())
            .map_err(|()| Error::from(LoquiError::ReadySendFailed))?;
    }

    // Convert each stream into a Result<Event, Error> stream.
    let ping_stream = Interval::new(ready.ping_interval)
        .map(|()| Event::Ping)
        .map_err(Error::from);
    let framed_reader = reader.map(Event::SocketReceive);
    let self_rx = self_rx.map_err(|()| LoquiError::EventReceiveError.into());

    let mut stream = framed_reader.select(self_rx).select(ping_stream);

    let mut event_handler = EventHandler::new(self_sender, handler, ready.transport_options);
    let mut closing = false;
    while let Some(event) = await!(stream.next()) {
        let event = event?;

        if let Event::Close { .. } = event {
            closing = true;
        }

        if let Some(frame) = event_handler.handle_event(event)? {
            writer = await!(writer.write(frame))?
        }

        if closing {
            return Ok(());
        }
    }

    Err(LoquiError::ConnectionClosed.into())
}
