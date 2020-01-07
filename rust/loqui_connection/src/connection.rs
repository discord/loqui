use crate::event_handler::EventHandler;
use crate::framed_io::ReaderWriter;
use crate::handler::{Handler, Ready};
use crate::select_break::StreamExt as SelectBreakStreamExt;
use crate::sender::Sender;
use crate::timeout_at;
use crate::LoquiError;
use failure::Error;
use futures::channel::mpsc::UnboundedReceiver;
use futures::channel::oneshot;
use futures::{Future, StreamExt};
use loqui_protocol::frames::{LoquiFrame, Response};
use std::net::SocketAddr;
use tokio::time::Instant;
use tokio::net::TcpStream;
use tokio::task;
use tokio::time::interval;

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
    pub fn spawn_from_address(
        address: SocketAddr,
        handler: H,
        handshake_deadline: Instant,
        ready_tx: Option<oneshot::Sender<&'static str>>,
    ) -> Self {
        let (self_sender, self_rx) = Sender::new();
        let connection = Self {
            self_sender: self_sender.clone(),
        };
        task::spawn(async move {
            match timeout_at(handshake_deadline, TcpStream::connect(&address)).await {
                Ok(tcp_stream) => {
                    info!("Connected to {}", address);
                    let result = run(
                        tcp_stream,
                        self_sender,
                        self_rx,
                        handler,
                        handshake_deadline,
                        ready_tx,
                    )
                    .await;
                    if let Err(e) = result {
                        warn!("Connection closed. ip={:?} error={:?}", address, e)
                    }
                }
                e => error!("Connect failed. error={:?}", e),
            };
        });
        connection
    }

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
        ready_tx: Option<oneshot::Sender<&'static str>>,
    ) -> Self {
        let (self_sender, self_rx) = Sender::new();
        let connection = Self {
            self_sender: self_sender.clone(),
        };
        task::spawn(async move {
            let ip = tcp_stream.peer_addr();
            let result = run(
                tcp_stream,
                self_sender,
                self_rx,
                handler,
                handshake_deadline,
                ready_tx,
            )
            .await;
            if let Err(e) = result {
                warn!("Connection closed. ip={:?} error={:?}", ip, e)
            }
        });
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
    ready_tx: Option<oneshot::Sender<&'static str>>,
) -> Result<(), Error> {
    let (ready, reader_writer, handler) =
        timeout_at(handshake_deadline, negotiate(tcp_stream, handler, ready_tx)).await?;
    debug!("Ready. {:?}", ready);
    let (reader, mut writer) = reader_writer.split();

    let Ready {
        ping_interval,
        encoding,
    } = ready;
    // Convert each stream into a Result<Event, Error> stream.
    let ping_stream = interval(ping_interval).map(|_| Ok(Event::Ping));
    let framed_reader = reader.map(|result| result.map(Event::SocketReceive));
    let self_rx = self_rx.map(|event| Ok(event));

    let mut stream = framed_reader
        .select_break(self_rx)
        .select_break(ping_stream);

    let mut event_handler = EventHandler::new(self_sender, handler, encoding);
    while let Some(event) = stream.next().await {
        let event = event?;

        match event_handler.handle_event(event) {
            Ok(Some(frame)) => writer = writer.write(frame).await?,
            Ok(None) => {}
            Err(error) => {
                writer.close(Some(&error), None).await;
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
    ready_tx: Option<oneshot::Sender<&'static str>>,
) -> impl Future<Output = Result<(Ready, ReaderWriter, H), Error>> {
    async move {
        let tcp_stream = handler.upgrade(tcp_stream).await?;
        let max_payload_size = handler.max_payload_size();
        let reader_writer = ReaderWriter::new(tcp_stream, max_payload_size, H::SEND_GO_AWAY);

        match handler.handshake(reader_writer).await {
            Ok((ready, reader_writer)) => {
                if let Some(ready_tx) = ready_tx {
                    ready_tx
                        .send(ready.encoding)
                        .map_err(|_| Error::from(LoquiError::ReadySendFailed))?;
                }
                Ok((ready, reader_writer, handler))
            }
            Err((error, reader_writer)) => {
                debug!("Not ready. e={:?}", error);
                if let Some(reader_writer) = reader_writer {
                    reader_writer.close(Some(&error)).await;
                }
                Err(error)
            }
        }
    }
}
