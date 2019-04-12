use crate::{Config, RequestHandler};
use bytesize::ByteSize;
use failure::Error;
use loqui_connection::handler::{DelegatedFrame, Handler, Ready, TransportOptions};
use loqui_connection::ReaderWriter;
use loqui_connection::{Encoder, EncoderFactory, IdSequence, LoquiError};
use loqui_protocol::frames::{Frame, Hello, HelloAck, LoquiFrame, Push, Request, Response};
use loqui_protocol::upgrade::{Codec, UpgradeFrame};
use loqui_protocol::VERSION;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio_codec::Framed;

pub struct ConnectionHandler<R: RequestHandler> {
    config: Arc<Config<R>>,
}

impl<R: RequestHandler> ConnectionHandler<R> {
    pub fn new(config: Arc<Config<R>>) -> Self {
        Self { config }
    }
}

impl<R: RequestHandler> Handler<R::EncoderFactory> for ConnectionHandler<R> {
    type InternalEvent = ();
    existential type UpgradeFuture: Send + Future<Output = Result<TcpStream, Error>>;
    existential type HandshakeFuture: Send
        + Future<
            Output = Result<(Ready, ReaderWriter), (Error, Option<ReaderWriter>)>,
        >;
    existential type HandleFrameFuture: Send + Future<Output = Result<Response, (Error, u32)>>;

    const SEND_GO_AWAY: bool = true;

    fn max_payload_size(&self) -> ByteSize {
        self.config.max_payload_size
    }

    fn upgrade(&self, tcp_stream: TcpStream) -> Self::UpgradeFuture {
        let max_payload_size = self.max_payload_size();
        async move {
            let framed_socket = Framed::new(tcp_stream, Codec::new(max_payload_size));
            let (mut writer, mut reader) = framed_socket.split();

            match await!(reader.next()) {
                Some(Ok(UpgradeFrame::Request)) => {
                    writer = match await!(writer.send(UpgradeFrame::Response)) {
                        Ok(writer) => writer,
                        Err(e) => return Err(e),
                    };
                    Ok(writer.reunite(reader)?.into_inner())
                }
                Some(Ok(frame)) => Err(LoquiError::InvalidUpgradeFrame { frame }.into()),
                Some(Err(e)) => Err(e),
                None => Err(LoquiError::TcpStreamClosed.into()),
            }
        }
    }

    fn handshake(&mut self, mut reader_writer: ReaderWriter) -> Self::HandshakeFuture {
        let ping_interval = self.config.ping_interval;
        async move {
            match await!(reader_writer.reader.next()) {
                Some(Ok(frame)) => match Self::handle_handshake_frame(frame, ping_interval) {
                    Ok((ready, hello_ack)) => {
                        reader_writer = match await!(reader_writer.write(hello_ack)) {
                            Ok(reader_writer) => reader_writer,
                            Err(e) => return Err((e.into(), None)),
                        };
                        Ok((ready, reader_writer))
                    }
                    Err(e) => Err((e, Some(reader_writer))),
                },
                Some(Err(e)) => Err((e, Some(reader_writer))),
                None => Err((LoquiError::TcpStreamClosed.into(), Some(reader_writer))),
            }
        }
    }

    fn handle_frame(
        &mut self,
        frame: DelegatedFrame,
        encoder: Arc<
            Box<
                dyn Encoder<
                        Encoded = <R::EncoderFactory as EncoderFactory>::Encoded,
                        Decoded = <R::EncoderFactory as EncoderFactory>::Decoded,
                    > + 'static,
            >,
        >,
    ) -> Option<Self::HandleFrameFuture> {
        match frame {
            DelegatedFrame::Push(push) => {
                tokio::spawn_async(handle_push(self.config.clone(), push, encoder));
                None
            }
            DelegatedFrame::Request(request) => {
                let response_future = handle_request(self.config.clone(), request, encoder);
                Some(response_future)
            }
            DelegatedFrame::Error(_) => None,
            DelegatedFrame::Response(_) => None,
        }
    }

    fn handle_internal_event(
        &mut self,
        _event: (),
        _id_sequence: &mut IdSequence,
        _encoder: Arc<
            Box<
                dyn Encoder<
                        Encoded = <R::EncoderFactory as EncoderFactory>::Encoded,
                        Decoded = <R::EncoderFactory as EncoderFactory>::Decoded,
                    > + 'static,
            >,
        >,
    ) -> Option<LoquiFrame> {
        None
    }

    fn handle_ping(&mut self) {}
}

async fn handle_push<R: RequestHandler>(
    config: Arc<Config<R>>,
    push: Push,
    encoder: Arc<
        Box<
            dyn Encoder<
                    Encoded = <R::EncoderFactory as EncoderFactory>::Encoded,
                    Decoded = <R::EncoderFactory as EncoderFactory>::Decoded,
                > + 'static,
        >,
    >,
) {
    let Push {
        payload,
        flags: _flags,
    } = push;
    match encoder.decode(payload) {
        Ok(request) => {
            config.request_handler.handle_push(request);
        }
        Err(e) => {
            error!("Failed to decode payload. error={:?}", e);
        }
    }
}

async fn handle_request<R: RequestHandler>(
    config: Arc<Config<R>>,
    request: Request,
    encoder: Arc<
        Box<
            dyn Encoder<
                    Encoded = <R::EncoderFactory as EncoderFactory>::Encoded,
                    Decoded = <R::EncoderFactory as EncoderFactory>::Decoded,
                > + 'static,
        >,
    >,
) -> Result<Response, (Error, u32)> {
    let Request {
        payload,
        flags: _flags,
        sequence_id,
    } = request;
    let request = encoder.decode(payload).map_err(|e| (e, sequence_id))?;

    let response = await!(config.request_handler.handle_request(request));

    let payload = encoder.encode(response).map_err(|e| (e, sequence_id))?;
    Ok(Response {
        flags: 0,
        sequence_id,
        payload,
    })
}

impl<R: RequestHandler> ConnectionHandler<R> {
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
            compression: compression.map(String::from),
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

    fn negotiate_encoding(client_encodings: &[String]) -> Result<&'static str, Error> {
        for client_encoding in client_encodings {
            if let Some(encoding) =
                <R::EncoderFactory as EncoderFactory>::find_encoding(client_encoding)
            {
                return Ok(encoding);
            }
        }
        Err(LoquiError::NoCommonEncoding.into())
    }

    fn negotiate_compression(
        client_compressions: &[String],
    ) -> Result<Option<&'static str>, Error> {
        if client_compressions.is_empty() {
            return Ok(None);
        }

        for client_compression in client_compressions {
            if let Some(compression) =
                <R::EncoderFactory as EncoderFactory>::find_compression(client_compression)
            {
                return Ok(Some(compression));
            }
        }
        Err(LoquiError::NoCommonCompression.into())
    }
}
