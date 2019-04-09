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
pub type FramedReader = SplitStream<Framed<TcpStream, Codec>>;

/// Used to write frames to the tcp socket.
pub struct FramedWriter {
    inner: SplitSink<Framed<TcpStream, Codec>>,
}

impl FramedWriter {
    pub fn new(writer: SplitSink<Framed<TcpStream, Codec>>) -> Self {
        Self { inner: writer }
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
    pub async fn close(self, error: Option<Error>) {
        if let Some(go_away_code) = go_away_code(&error) {
            let go_away = GoAway {
                flags: 0,
                code: go_away_code as u16,
                payload: vec![],
            };
            debug!("Closing. Sending GoAway. go_away={:?}", go_away);
            if let Err(error) = await!(self.write(go_away)) {
                error!("Error when writing close frame. error={:?}", error);
            }
        } else {
            debug!("Closing. Not sending GoAway. error={:?}", error)
        }
    }
}

fn go_away_code(error: &Option<Error>) -> Option<LoquiErrorCode> {
    match error {
        None => Some(LoquiErrorCode::Normal),
        Some(error) => {
            if let Some(protocol_error) = error.downcast_ref::<ProtocolError>() {
                let error_code = match protocol_error {
                    // TODO: elixir sends back a 413 http response
                    ProtocolError::PayloadTooLarge(_, _) => LoquiErrorCode::InternalServerError,
                    ProtocolError::InvalidOpcode(_opcode) => LoquiErrorCode::InvalidOpcode,
                    // TODO
                    ProtocolError::InvalidPayload(_reason) => LoquiErrorCode::InternalServerError,
                };
                return Some(error_code);
            }

            if let Some(loqui_error) = error.downcast_ref::<LoquiError>() {
                match loqui_error {
                    // We should not send back a go away if we were told to go away.
                    LoquiError::ToldToGoAway { .. } => return None,
                    _ => {}
                }
            }
            Some(LoquiErrorCode::InternalServerError)
        }
    }
}

pub struct FramedReaderWriter {
    pub reader: FramedReader,
    writer: FramedWriter,
}

impl FramedReaderWriter {
    pub fn new(tcp_stream: TcpStream, max_payload_size: ByteSize) -> Self {
        let framed_socket = Framed::new(tcp_stream, Codec::new(max_payload_size));
        let (writer, reader) = framed_socket.split();
        let writer = FramedWriter::new(writer);
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

    pub fn split(self) -> (FramedReader, FramedWriter) {
        let FramedReaderWriter { reader, writer } = self;
        (reader, writer)
    }
}
