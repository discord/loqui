use crate::error::{LoquiError, LoquiErrorCode};
use bytesize::ByteSize;
use failure::Error;
use futures::stream::{SplitSink, SplitStream};
use loqui_protocol::{
    codec::Codec,
    error::ProtocolError,
    frames::{GoAway, LoquiFrame},
};
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio_codec::Framed;

/// Used to read frames off the tcp socket.
pub type Reader = SplitStream<Framed<TcpStream, Codec>>;

/// Used to write frames to the tcp socket.
pub struct Writer {
    inner: SplitSink<Framed<TcpStream, Codec>>,
    send_go_away: bool,
}

impl Writer {
    pub fn new(writer: SplitSink<Framed<TcpStream, Codec>>, send_go_away: bool) -> Self {
        Self {
            inner: writer,
            send_go_away,
        }
    }

    /// Tries to write a `LoquiFrame` to the socket. Returns an error if the socket has closed.
    pub async fn write<F: Into<LoquiFrame>>(mut self, frame: F) -> Result<Self, LoquiError> {
        match await!(self.inner.send(frame.into())) {
            Ok(new_inner) => {
                self.inner = new_inner;
                Ok(self)
            }
            Err(_error) => Err(LoquiError::TcpStreamClosed),
        }
    }

    /// Gracefully closes the socket. Optionally sends a `GoAway` frame before closing.
    pub async fn close(self, error: Option<&Error>) {
        if !self.send_go_away {
            debug!("Closing. Not sending GoAway. error={:?}", error);
            return;
        }

        let go_away = GoAway {
            flags: 0,
            code: go_away_code(&error) as u16,
            payload: vec![],
        };
        debug!("Closing. Sending GoAway. go_away={:?}", go_away);
        if let Err(error) = await!(self.write(go_away)) {
            error!("Error when writing close frame. error={:?}", error);
        }
    }
}

fn go_away_code(error: &Option<&Error>) -> LoquiErrorCode {
    match error {
        None => LoquiErrorCode::Normal,
        Some(error) => {
            if let Some(protocol_error) = error.downcast_ref::<ProtocolError>() {
                let error_code = match protocol_error {
                    // TODO: elixir sends back a 413 http response
                    ProtocolError::PayloadTooLarge(_, _) => LoquiErrorCode::InternalServerError,
                    ProtocolError::InvalidOpcode(_opcode) => LoquiErrorCode::InvalidOpcode,
                    // TODO
                    ProtocolError::InvalidPayload(_reason) => LoquiErrorCode::InternalServerError,
                };
                return error_code;
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
    pub fn new(tcp_stream: TcpStream, max_payload_size: ByteSize, send_go_away: bool) -> Self {
        let framed_socket = Framed::new(tcp_stream, Codec::new(max_payload_size));
        let (writer, reader) = framed_socket.split();
        let writer = Writer::new(writer, send_go_away);
        Self { reader, writer }
    }

    /// Tries to write a `LoquiFrame` to the socket. Returns an error if the socket has closed.
    pub async fn write<F: Into<LoquiFrame>>(mut self, frame: F) -> Result<Self, LoquiError> {
        match await!(self.writer.write(frame.into())) {
            Ok(new_writer) => {
                self.writer = new_writer;
                Ok(self)
            }
            Err(_error) => Err(LoquiError::TcpStreamClosed),
        }
    }

    pub fn split(self) -> (Reader, Writer) {
        let ReaderWriter { reader, writer } = self;
        (reader, writer)
    }
}
