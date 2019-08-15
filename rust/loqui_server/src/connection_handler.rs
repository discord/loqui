use crate::{Config, RequestHandler};
use bytesize::ByteSize;
use failure::Error;
use loqui_connection::handler::{DelegatedFrame, Handler, Ready};
use loqui_connection::{find_encoding, ReaderWriter};
use loqui_connection::{IdSequence, LoquiError};
use loqui_protocol::frames::{Frame, Hello, HelloAck, LoquiFrame, Push, Request, Response};
use loqui_protocol::upgrade::{Codec, UpgradeFrame};
use loqui_protocol::VERSION;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio_codec::Framed;
use tokio_futures::compat::{forward::IntoAwaitable, infallible_into_01};
use tokio_futures::stream::StreamExt;

pub struct ConnectionHandler<R: RequestHandler> {
    config: Arc<Config<R>>,
}

impl<R: RequestHandler> ConnectionHandler<R> {
    pub fn new(config: Arc<Config<R>>) -> Self {
        Self { config }
    }
}

impl<R: RequestHandler> Handler for ConnectionHandler<R> {
    // Server doesn't have any internal events.
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

            match reader.next().await {
                Some(Ok(UpgradeFrame::Request)) => {
                    writer = match writer.send(UpgradeFrame::Response).into_awaitable().await {
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
        let supported_encodings = self.config.supported_encodings;
        async move {
            match reader_writer.reader.next().await {
                Some(Ok(frame)) => {
                    match Self::handle_handshake_frame(frame, ping_interval, supported_encodings) {
                        Ok((ready, hello_ack)) => {
                            reader_writer = match reader_writer.write(hello_ack).await {
                                Ok(reader_writer) => reader_writer,
                                Err(e) => return Err((e.into(), None)),
                            };
                            Ok((ready, reader_writer))
                        }
                        Err(e) => Err((e, Some(reader_writer))),
                    }
                }
                Some(Err(e)) => Err((e, Some(reader_writer))),
                None => Err((LoquiError::TcpStreamClosed.into(), Some(reader_writer))),
            }
        }
    }

    fn handle_frame(
        &mut self,
        frame: DelegatedFrame,
        encoding: &'static str,
    ) -> Option<Self::HandleFrameFuture> {
        match frame {
            DelegatedFrame::Push(push) => {
                tokio::spawn(infallible_into_01(handle_push(
                    self.config.clone(),
                    push,
                    encoding,
                )));
                None
            }
            DelegatedFrame::Request(request) => {
                let response_future = handle_request(self.config.clone(), request, encoding);
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
    ) -> Option<LoquiFrame> {
        None
    }

    fn on_ping_received(&mut self) {}
}
impl<R: RequestHandler> ConnectionHandler<R> {
    fn handle_handshake_frame(
        frame: LoquiFrame,
        ping_interval: Duration,
        supported_encodings: &'static [&'static str],
    ) -> Result<(Ready, HelloAck), Error> {
        match frame {
            LoquiFrame::Hello(hello) => {
                Self::handle_handshake_hello(hello, ping_interval, supported_encodings)
            }
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
        supported_encodings: &'static [&'static str],
    ) -> Result<(Ready, HelloAck), Error> {
        let Hello {
            flags,
            version,
            encodings,
            // compression not supported
            compressions: _compressions,
        } = hello;
        if version != VERSION {
            return Err(LoquiError::UnsupportedVersion {
                expected: VERSION,
                actual: version,
            }
            .into());
        }
        let encoding = Self::negotiate_encoding(&encodings, supported_encodings)?;
        let hello_ack = HelloAck {
            flags,
            ping_interval_ms: ping_interval.as_millis() as u32,
            encoding: encoding.to_string(),
            compression: None,
        };
        let ready = Ready {
            ping_interval,
            encoding,
        };
        Ok((ready, hello_ack))
    }

    fn negotiate_encoding(
        client_encodings: &[String],
        supported_encodings: &'static [&'static str],
    ) -> Result<&'static str, Error> {
        for client_encoding in client_encodings {
            if let Some(encoding) = find_encoding(client_encoding, supported_encodings) {
                return Ok(encoding);
            }
        }
        Err(LoquiError::NoCommonEncoding.into())
    }
}

async fn handle_push<R: RequestHandler>(
    config: Arc<Config<R>>,
    push: Push,
    encoding: &'static str,
) {
    let Push {
        payload,
        flags: _flags,
    } = push;
    config.request_handler.handle_push(payload, encoding).await
}

async fn handle_request<R: RequestHandler>(
    config: Arc<Config<R>>,
    request: Request,
    encoding: &'static str,
) -> Result<Response, (Error, u32)> {
    let Request {
        payload: request_payload,
        flags: _flags,
        sequence_id,
    } = request;
    let response_payload = config
        .request_handler
        .handle_request(request_payload, encoding).await;
    Ok(Response {
        flags: 0,
        sequence_id,
        payload: response_payload,
    })
}
