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
    pub async fn close(self, reader: FramedReader, error: Option<Error>) {
        let go_away = GoAway {
            flags: 0,
            code: go_away_code(error) as u16,
            payload: vec![],
        };
        debug!("Closing. goaway={:#?}", go_away);
        if let Err(error) = await!(self.write(go_away)) {
            error!("Error when writing close frame. error={:?}", error);
        }
        // TODO: is this needed?
        /*
        self = match
            Ok(writer) => writer,
            Err(error) => {
            }
        };
        let mut tcp_stream = self
            .inner
            .reunite(reader)
            .expect("failed to reunite")
            .into_inner();
        if let Err(error) = await!(tcp_stream.flush_async()) {
            error!("Failed to flush when closing. error={:?}", error);
        }
        */
    }
}

fn go_away_code(error: Option<Error>) -> LoquiErrorCode {
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
