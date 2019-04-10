use crate::waiter::ResponseWaiter;
use crate::Config;
use bytesize::ByteSize;
use failure::{err_msg, Error};
use loqui_connection::handler::{DelegatedFrame, Handler, Ready, TransportOptions};
use loqui_connection::{Encoder, IdSequence, LoquiError, ReaderWriter};
use loqui_protocol::frames::{
    Error as ErrorFrame, Frame, Hello, HelloAck, LoquiFrame, Push, Request, Response,
};
use loqui_protocol::upgrade::{Codec, UpgradeFrame};
use loqui_protocol::VERSION;
use loqui_protocol::{is_compressed, make_flags};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio_codec::Framed;

#[derive(Debug)]
pub enum InternalEvent<Encoded, Decoded>
where
    Encoded: Serialize + Send + Sync,
    Decoded: DeserializeOwned + Send + Sync,
{
    Request {
        payload: Encoded,
        waiter: ResponseWaiter<Decoded>,
    },
    Push {
        payload: Encoded,
    },
}

pub struct ConnectionHandler<E: Encoder> {
    waiters: HashMap<u32, ResponseWaiter<E::Decoded>>,
    config: Arc<Config<E>>,
}

impl<E: Encoder> ConnectionHandler<E> {
    pub fn new(config: Arc<Config<E>>) -> Self {
        Self {
            waiters: HashMap::new(),
            config,
        }
    }
}

impl<E: Encoder> Handler for ConnectionHandler<E> {
    type InternalEvent = InternalEvent<E::Encoded, E::Decoded>;
    existential type UpgradeFuture: Send + Future<Output = Result<TcpStream, Error>>;
    existential type HandshakeFuture: Send
        + Future<
            Output = Result<(Ready, ReaderWriter), (Error, Option<ReaderWriter>)>,
        >;
    existential type HandleFrameFuture: Send + Future<Output = Result<Response, (Error, u32)>>;

    const SEND_GO_AWAY: bool = false;

    fn max_payload_size(&self) -> ByteSize {
        self.config.max_payload_size
    }

    fn upgrade(&self, tcp_stream: TcpStream) -> Self::UpgradeFuture {
        let max_payload_size = self.max_payload_size();
        async move {
            let framed_socket = Framed::new(tcp_stream, Codec::new(max_payload_size));
            let (mut writer, mut reader) = framed_socket.split();
            writer = match await!(writer.send(UpgradeFrame::Request)) {
                Ok(writer) => writer,
                Err(_e) => return Err(LoquiError::TcpStreamClosed.into()),
            };
            match await!(reader.next()) {
                Some(Ok(UpgradeFrame::Response)) => Ok(writer.reunite(reader)?.into_inner()),
                Some(Ok(frame)) => Err(LoquiError::InvalidUpgradeFrame { frame }.into()),
                Some(Err(e)) => Err(e),
                None => Err(LoquiError::TcpStreamClosed.into()),
            }
        }
    }

    fn handshake(&mut self, mut reader_writer: ReaderWriter) -> Self::HandshakeFuture {
        async move {
            let hello = Self::make_hello();
            reader_writer = match await!(reader_writer.write(hello)) {
                Ok(read_writer) => read_writer,
                Err(e) => return Err((e.into(), None)),
            };

            match await!(reader_writer.reader.next()) {
                Some(Ok(frame)) => match Self::handle_handshake_frame(frame) {
                    Ok(ready) => Ok((ready, reader_writer)),
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
        transport_options: &TransportOptions,
    ) -> Option<Self::HandleFrameFuture> {
        match frame {
            DelegatedFrame::Response(response) => {
                self.handle_response(response, transport_options);
                None
            }
            DelegatedFrame::Error(error) => {
                self.handle_error(error);
                None
            }
            DelegatedFrame::Push(_) | DelegatedFrame::Request(_) => Some(
                async move {
                    Err((
                        LoquiError::InvalidOpcode {
                            actual: Request::OPCODE,
                            expected: None,
                        }
                        .into(),
                        0,
                    ))
                },
            ),
        }
    }

    fn handle_internal_event(
        &mut self,
        event: InternalEvent<E::Encoded, E::Decoded>,
        id_sequence: &mut IdSequence,
        transport_options: &TransportOptions,
    ) -> Option<LoquiFrame> {
        let TransportOptions { encoding, .. } = transport_options;
        // Forward Request and Push events to the connection so it can send them to the server.
        match event {
            InternalEvent::Request { payload, waiter } => {
                let sequence_id = id_sequence.next();
                self.handle_request(payload, encoding, sequence_id, waiter)
            }
            InternalEvent::Push { payload } => self.handle_push(payload, encoding),
        }
    }

    fn handle_ping(&mut self) {
        // Use to sweep dead waiters.
        let now = Instant::now();
        self.waiters
            .retain(|_sequence_id, waiter| waiter.deadline > now);
    }
}

impl<E: Encoder> ConnectionHandler<E> {
    fn handle_push(&mut self, payload: E::Encoded, encoding: &'static str) -> Option<LoquiFrame> {
        match self.config.encoder.encode(encoding, payload) {
            Ok((payload, compressed)) => {
                let push = Push {
                    payload,
                    flags: make_flags(compressed),
                };
                Some(push.into())
            }
            Err(e) => {
                error!("Failed to encode payload. error={:?}", e);
                None
            }
        }
    }

    fn handle_request(
        &mut self,
        payload: E::Encoded,
        encoding: &'static str,
        sequence_id: u32,
        waiter: ResponseWaiter<E::Decoded>,
    ) -> Option<LoquiFrame> {
        if waiter.deadline <= Instant::now() {
            waiter.notify(Err(LoquiError::RequestTimeout.into()));
            return None;
        }

        match self.config.encoder.encode(encoding, payload) {
            Ok((payload, compressed)) => {
                // Store the waiter so we can notify it when we get a response.
                self.waiters.insert(sequence_id, waiter);
                let request = Request {
                    payload,
                    sequence_id,
                    flags: make_flags(compressed),
                };
                Some(request.into())
            }
            Err(error) => {
                waiter.notify(Err(error.into()));
                None
            }
        }
    }

    fn handle_response(&mut self, response: Response, transport_options: &TransportOptions) {
        let Response {
            flags,
            sequence_id,
            payload,
        } = response;
        match self.waiters.remove(&sequence_id) {
            Some(waiter) => {
                let response = self.config.encoder.decode(
                    transport_options.encoding,
                    is_compressed(flags),
                    payload,
                );
                waiter.notify(response);
            }
            None => {
                debug!("No waiter for sequence_id. sequence_id={:?}", sequence_id);
            }
        }
    }

    fn handle_error(&mut self, error: ErrorFrame) {
        let ErrorFrame {
            sequence_id,
            payload,
            ..
        } = error;
        match self.waiters.remove(&sequence_id) {
            Some(waiter) => {
                // payload is always a string
                let result = String::from_utf8(payload)
                    .map_err(Error::from)
                    .and_then(|reason| Err(err_msg(reason)));
                waiter.notify(result);
            }
            None => {
                warn!("No waiter for sequence_id. sequence_id={:?}", sequence_id);
            }
        }
    }

    fn make_hello() -> Hello {
        Hello {
            flags: 0,
            version: VERSION,
            encodings: E::ENCODINGS
                .to_owned()
                .into_iter()
                .map(String::from)
                .collect(),
            compressions: E::COMPRESSIONS
                .to_owned()
                .into_iter()
                .map(String::from)
                .collect(),
        }
    }

    fn handle_handshake_frame(frame: LoquiFrame) -> Result<Ready, Error> {
        match frame {
            LoquiFrame::HelloAck(hello_ack) => Self::handle_handshake_hello_ack(hello_ack),
            LoquiFrame::GoAway(go_away) => Err(LoquiError::ToldToGoAway { go_away }.into()),
            frame => Err(LoquiError::InvalidOpcode {
                actual: frame.opcode(),
                expected: Some(HelloAck::OPCODE),
            }
            .into()),
        }
    }

    fn handle_handshake_hello_ack(hello_ack: HelloAck) -> Result<Ready, Error> {
        // Validate the settings and convert them to &'static str.
        let encoding = match E::find_encoding(hello_ack.encoding) {
            Some(encoding) => encoding,
            None => return Err(LoquiError::InvalidEncoding.into()),
        };
        let compression = match hello_ack.compression {
            None => None,
            Some(compression) => match E::find_compression(compression) {
                Some(compression) => Some(compression),
                None => return Err(LoquiError::InvalidCompression.into()),
            },
        };
        let ping_interval = Duration::from_millis(u64::from(hello_ack.ping_interval_ms));
        let transport_options = TransportOptions {
            encoding,
            compression,
        };
        Ok(Ready {
            ping_interval,
            transport_options,
        })
    }
}
