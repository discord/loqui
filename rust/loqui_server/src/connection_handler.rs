use crate::{Config, RequestHandler};
use bytesize::ByteSize;
use failure::Error;
use loqui_connection::FramedReaderWriter;
use loqui_connection::LoquiErrorCode;
use loqui_connection::{
    ConnectionHandler, DelegatedFrame, Encoder, IdSequence, LoquiError, Ready, TransportOptions,
};
use loqui_protocol::errors::ProtocolError;
use loqui_protocol::frames::{Frame, GoAway, Hello, HelloAck, LoquiFrame, Push, Request, Response};
use loqui_protocol::Flags;
use loqui_protocol::{is_compressed, VERSION};
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;

pub struct ServerConnectionHandler<R: RequestHandler<E>, E: Encoder> {
    config: Arc<Config<R, E>>,
}

impl<R: RequestHandler<E>, E: Encoder> ServerConnectionHandler<R, E> {
    pub fn new(config: Arc<Config<R, E>>) -> Self {
        Self { config }
    }
}

impl<R: RequestHandler<E>, E: Encoder> ConnectionHandler for ServerConnectionHandler<R, E> {
    type InternalEvent = ();
    existential type UpgradeFuture: Send + Future<Output = Result<TcpStream, Error>>;
    existential type HandshakeFuture: Send
        + Future<
            Output = Result<(Ready, FramedReaderWriter), (Error, Option<FramedReaderWriter>)>,
        >;
    existential type HandleFrameFuture: Send + Future<Output = Result<Response, (Error, u32)>>;

    fn max_payload_size(&self) -> ByteSize {
        self.config.max_payload_size
    }

    fn upgrade(&self, mut tcp_stream: TcpStream) -> Self::UpgradeFuture {
        async {
            let mut payload = [0; 1024];
            // TODO: handle disconnect, bytes_read=0
            while let Ok(_bytes_read) = await!(tcp_stream.read_async(&mut payload)) {
                let request = String::from_utf8(payload.to_vec())?;
                // TODO: better
                if request.contains(&"upgrade") || request.contains(&"Upgrade") {
                    let response =
                        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: loqui\r\nConnection: Upgrade\r\n\r\n";
                    await!(tcp_stream.write_all_async(&response.as_bytes()[..]))?;
                    await!(tcp_stream.flush_async())?;
                    break;
                }
            }
            Ok(tcp_stream)
        }
    }

    fn handshake(&mut self, mut reader_writer: FramedReaderWriter) -> Self::HandshakeFuture {
        let ping_interval = self.config.ping_interval.clone();
        async move {
            // TODO: this error could be something for real?
            if let Some(Ok(frame)) = await!(reader_writer.reader.next()) {
                match Self::handle_handshake_frame(frame, ping_interval) {
                    Ok((ready, hello_ack)) => {
                        reader_writer = match await!(reader_writer.write(hello_ack)) {
                            Ok(reader_writer) => reader_writer,
                            Err(e) => return Err((e.into(), None)),
                        };
                        Ok((ready, reader_writer))
                    }
                    Err(e) => Err((e.into(), Some(reader_writer))),
                }
            } else {
                Err((LoquiError::TcpStreamClosed.into(), Some(reader_writer)))
            }
        }
    }

    fn handle_frame(
        &mut self,
        frame: DelegatedFrame,
        transport_options: &TransportOptions,
    ) -> Option<Self::HandleFrameFuture> {
        match frame {
            DelegatedFrame::Push(push) => {
                tokio::spawn_async(handle_push(
                    self.config.clone(),
                    push,
                    transport_options.encoding,
                ));
                None
            }
            DelegatedFrame::Request(request) => {
                let response_future =
                    handle_request(self.config.clone(), request, transport_options.encoding);
                Some(response_future)
            }
            DelegatedFrame::Response(_) => {
                // TODO: should be an error?
                None
            }
        }
    }

    fn handle_internal_event(
        &mut self,
        _event: (),
        _id_sequence: &mut IdSequence,
        _transport_options: &TransportOptions,
    ) -> Option<LoquiFrame> {
        None
    }
}

async fn handle_push<E: Encoder, R: RequestHandler<E>>(
    config: Arc<Config<R, E>>,
    push: Push,
    encoding: &'static str,
) {
    let Push { payload, flags } = push;
    match config
        .encoder
        .decode(encoding, is_compressed(&flags), payload)
    {
        Ok(request) => {
            config.request_handler.handle_push(request);
        }
        Err(e) => {
            error!("Failed to decode payload. error={:?}", e);
        }
    }
}

async fn handle_request<E: Encoder, R: RequestHandler<E>>(
    config: Arc<Config<R, E>>,
    request: Request,
    encoding: &'static str,
) -> Result<Response, (Error, u32)> {
    let Request {
        payload,
        flags,
        sequence_id,
    } = request;
    let request = config
        .encoder
        .decode(encoding, is_compressed(&flags), payload)
        .map_err(|e| (e, sequence_id))?;

    let response = await!(config.request_handler.handle_request(request));

    let (payload, compressed) = config
        .encoder
        .encode(encoding, response)
        .map_err(|e| (e, sequence_id))?;
    let flags = if compressed {
        Flags::Compressed
    } else {
        Flags::None
    };
    Ok(Response {
        flags: flags as u8,
        sequence_id,
        payload,
    })
}

impl<E: Encoder, R: RequestHandler<E>> ServerConnectionHandler<R, E> {
    fn handle_handshake_frame(
        frame: LoquiFrame,
        ping_interval: Duration,
    ) -> Result<(Ready, HelloAck), Error> {
        match frame {
            LoquiFrame::Hello(hello) => Self::handle_handshake_hello(hello, ping_interval),
            LoquiFrame::GoAway(go_away) => Err(LoquiError::ToldToGoAway { go_away }.into()),
            frame => Err(LoquiError::InvalidOpcode {
                actual: frame.opcode(),
                expected: Some(Hello::OPCODE),
            }
            .into()),
        }
    }

    fn handle_handshake_hello(
        hello: Hello,
        ping_interval: Duration,
    ) -> Result<(Ready, HelloAck), Error> {
        let Hello {
            flags,
            version,
            encodings,
            compressions,
        } = hello;
        if version != VERSION {
            return Err(LoquiError::UnsupportedVersion {
                expected: VERSION,
                actual: version,
            }
            .into());
        }
        let encoding = Self::negotiate_encoding(&encodings)?;
        let compression = Self::negotiate_compression(&compressions)?;
        let hello_ack = HelloAck {
            flags,
            ping_interval_ms: ping_interval.as_millis() as u32,
            encoding: encoding.to_string(),
            compression: compression.map(|compression| compression.to_string()),
        };
        let ready = Ready {
            ping_interval,
            transport_options: TransportOptions {
                encoding,
                compression,
            },
        };
        Ok((ready, hello_ack))
    }

    fn negotiate_encoding(client_encodings: &Vec<String>) -> Result<&'static str, Error> {
        for client_encoding in client_encodings {
            if let Some(encoding) = E::find_encoding(client_encoding) {
                return Ok(encoding);
            }
        }
        Err(LoquiError::NoCommonEncoding.into())
    }

    fn negotiate_compression(
        client_compressions: &Vec<String>,
    ) -> Result<Option<&'static str>, Error> {
        if client_compressions.len() == 0 {
            return Ok(None);
        }

        for client_compression in client_compressions {
            if let Some(compression) = E::find_compression(client_compression) {
                return Ok(Some(compression));
            }
        }
        Err(LoquiError::NoCommonCompression.into())
    }
}
