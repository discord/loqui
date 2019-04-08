use crate::connection_handler::{ConnectionHandler, Ready};
use crate::event_handler::EventHandler;
use crate::framed_io::FramedReaderWriter;
use crate::sender::ConnectionSender;
use crate::LoquiError;
use failure::Error;
use futures::sync::mpsc::UnboundedReceiver;
use futures::sync::oneshot;
use futures_timer::Interval;
use loqui_protocol::frames::{LoquiFrame, Response};
use tokio::await;
use tokio::codec::Framed;
use tokio::net::TcpStream;
use tokio::prelude::*;

#[derive(Debug)]
pub struct Connection<C: ConnectionHandler> {
    self_sender: ConnectionSender<C::InternalEvent>,
}

impl<C: ConnectionHandler> Connection<C> {
    pub fn spawn(
        tcp_stream: TcpStream,
        connection_handler: C,
        ready_tx: Option<oneshot::Sender<()>>,
    ) -> Self {
        let (self_sender, self_rx) = ConnectionSender::new();
        let connection = Self {
            self_sender: self_sender.clone(),
        };
        tokio::spawn_async(
            async move {
                if let Err(e) = await!(run(
                    tcp_stream,
                    self_sender,
                    self_rx,
                    connection_handler,
                    ready_tx
                )) {
                    error!("Connection closed. error={:?}", e)
                }
            },
        );
        connection
    }

    pub fn send_event(&self, event: C::InternalEvent) -> Result<(), Error> {
        self.self_sender.internal(event)
    }
}

/// The events that can be received by the core connection loop once it begins running.
#[derive(Debug)]
pub enum Event<T: Send + 'static> {
    /// A full frame was received on the socket.
    SocketReceive(LoquiFrame),
    /// A ping should be sent.
    Ping,
    /// Generic event that will be delegated to the connection handler.
    InternalEvent(T),
    /// A response for a request was computed and should be sent back over the socket.
    ResponseComplete(Result<Response, (Error, u32)>),
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
/// * `connection_handler` - implements logic for the client or server specific things
/// * `ready_tx` - a sender used to notify that the connection is ready for requests
async fn run<C: ConnectionHandler>(
    mut tcp_stream: TcpStream,
    self_sender: ConnectionSender<C::InternalEvent>,
    self_rx: UnboundedReceiver<Event<C::InternalEvent>>,
    mut connection_handler: C,
    ready_tx: Option<oneshot::Sender<()>>,
) -> Result<(), Error> {
    let tcp_stream = await!(connection_handler.upgrade(tcp_stream))?;
    let max_payload_size = connection_handler.max_payload_size();
    let reader_writer = FramedReaderWriter::new(tcp_stream, max_payload_size);

    let (ready, reader_writer) = match await!(connection_handler.handshake(reader_writer)) {
        Ok((ready, reader_writer)) => (ready, reader_writer),
        Err((error, reader_writer)) => {
            debug!("Not ready. e={:?}", error);
            if let Some(reader_writer) = reader_writer {
                let (reader, mut writer) = reader_writer.split();
                await!(writer.close(reader, Some(error)));
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
    let framed_reader = reader.map(|frame| Event::SocketReceive(frame));
    let self_rx = self_rx.map_err(|()| LoquiError::EventReceiveError.into());

    let mut stream = framed_reader.select(self_rx).select(ping_stream);

    let mut event_handler =
        EventHandler::new(self_sender, connection_handler, ready.transport_options);
    while let Some(event) = await!(stream.next()) {
        // TODO: handle error
        let event = event?;
        if let Some(frame) = event_handler.handle_event(event)? {
            writer = await!(writer.write(frame))?;
        }
    }

    Err(LoquiError::ConnectionClosed.into())
}
