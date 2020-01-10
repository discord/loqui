use crate::error::{LoquiError, LoquiErrorCode};
use bytesize::ByteSize;
use failure::Error;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use futures::stream::{SplitSink, SplitStream};
use loqui_protocol::{
    codec::Codec,
    error::ProtocolError,
    frames::{GoAway, LoquiFrame},
};
use std::net::Shutdown;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

/// Used to read frames off the tcp socket.
pub type Reader = SplitStream<Framed<TcpStream, Codec>>;

/// Used to write frames to the tcp socket.
pub struct Writer {
    inner: SplitSink<Framed<TcpStream, Codec>, LoquiFrame>,
    /// If true, send a go away when the socket is closed.
    send_go_away: bool,
}

impl Writer {
    /// Create a new `Writer` that can write frames to a tcp socket.
    ///
    /// # Arguments
    ///
    /// * `writer` - framed sink
    /// * `send_go_away` - whether or not to send a go away when the connection closes
    pub fn new(
        writer: SplitSink<Framed<TcpStream, Codec>, LoquiFrame>,
        send_go_away: bool,
    ) -> Self {
        Self {
            inner: writer,
            send_go_away,
        }
    }

    /// Tries to write a `LoquiFrame` to the socket. Returns an error if the socket has closed.
    pub async fn write<F: Into<LoquiFrame>>(mut self, frame: F) -> Result<Self, LoquiError> {
        match self.inner.send(frame.into()).await {
            Ok(()) => Ok(self),
            Err(_error) => Err(LoquiError::TcpStreamClosed),
        }
    }

    /// Gracefully closes the socket. Optionally sends a `GoAway` frame before closing.
    pub async fn close(mut self, error: Option<&Error>, reader: Option<Reader>) {
        if !self.send_go_away {
            debug!("Closing. Not sending GoAway. error={:?}", error);
            return;
        }

        let go_away = GoAway {
            flags: 0,
            code: go_away_code(error) as u16,
            payload: vec![],
        };
        debug!("Closing. Sending GoAway. go_away={:?}", go_away);
        match self.inner.send(go_away.into()).await {
            Ok(()) => {
                if let Some(reader) = reader {
                    if let Ok(tcp_stream) =
                        self.inner.reunite(reader).map(|framed| framed.into_inner())
                    {
                        let _result = tcp_stream.shutdown(Shutdown::Both);
                    }
                }
            }
            Err(_error) => {
                error!("Error when writing close frame. error={:?}", error);
            }
        }
    }
}

/// Determines the go away code that should be sent.
///
/// # Arguments
///
/// * `error` - optional error to determine the code from
fn go_away_code(error: Option<&Error>) -> LoquiErrorCode {
    match error {
        None => LoquiErrorCode::Normal,
        Some(error) => {
            if let Some(protocol_error) = error.downcast_ref::<ProtocolError>() {
                let error_code = match protocol_error {
                    ProtocolError::InvalidOpcode { .. } => LoquiErrorCode::InvalidOpcode,
                    ProtocolError::PayloadTooLarge { .. }
                    | ProtocolError::InvalidPayload { .. } => LoquiErrorCode::InternalServerError,
                };
                return error_code;
            }
            if let Some(loqui_error) = error.downcast_ref::<LoquiError>() {
                return loqui_error.code();
            }
            LoquiErrorCode::InternalServerError
        }
    }
}

pub struct ReaderWriter {
    pub reader: Reader,
    writer: Writer,
}

impl ReaderWriter {
    /// Create a new `ReaderWriter` that can read and write frames to a tcp socket.
    ///
    /// # Arguments
    ///
    /// * `tcp_stream` - raw tcp socket
    /// * `max_payload_size` - the maximum bytes a frame payload can be
    /// * `send_go_away` - whether or not to send a go away when the connection closes
    pub fn new(tcp_stream: TcpStream, max_payload_size: ByteSize, send_go_away: bool) -> Self {
        let framed_socket = Framed::new(tcp_stream, Codec::new(max_payload_size));
        let (writer, reader) = framed_socket.split();
        let writer = Writer::new(writer, send_go_away);
        Self { reader, writer }
    }

    /// Tries to write a `LoquiFrame` to the socket. Returns an error if the socket has closed.
    pub async fn write<F: Into<LoquiFrame>>(mut self, frame: F) -> Result<Self, LoquiError> {
        match self.writer.write(frame.into()).await {
            Ok(new_writer) => {
                self.writer = new_writer;
                Ok(self)
            }
            Err(_error) => Err(LoquiError::TcpStreamClosed),
        }
    }

    /// Split this `ReaderWriter`, returning the `Reader` and `Writer` parts.
    pub fn split(self) -> (Reader, Writer) {
        let ReaderWriter { reader, writer } = self;
        (reader, writer)
    }

    /// Gracefully closes the socket. Optionally sends a `GoAway` frame before closing.
    pub async fn close(self, error: Option<&Error>) {
        self.writer.close(error, Some(self.reader)).await
    }
}
