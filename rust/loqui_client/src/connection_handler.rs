use crate::Config;
use bytesize::ByteSize;
use failure::Error;
use futures::sync::oneshot::Sender as OneShotSender;
use loqui_connection::{
    ConnectionHandler, DelegatedFrame, Encoder, FramedReaderWriter, IdSequence, LoquiError, Ready,
    TransportOptions,
};
use loqui_protocol::frames::{Frame, Hello, HelloAck, LoquiFrame, Push, Request, Response};
use loqui_protocol::VERSION;
use loqui_protocol::{is_compressed, make_flags, Flags};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::await;
use tokio::net::TcpStream;
use tokio::prelude::*;

// TODO: get right values
const UPGRADE_REQUEST: &'static str =
    "GET /_rpc HTTP/1.1\r\nHost: 127.0.0.1 \r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";

#[derive(Debug)]
pub enum InternalEvent<I: Serialize + Send + Sync, O: DeserializeOwned + Send + Sync> {
    Request {
        payload: I,
        waiter_tx: OneShotSender<Result<O, Error>>,
    },
    Push {
        payload: I,
    },
}

pub struct ClientConnectionHandler<E: Encoder> {
    waiters: HashMap<u32, OneShotSender<Result<E::Decoded, Error>>>,
    config: Arc<Config<E>>,
}

impl<E: Encoder> ClientConnectionHandler<E> {
    pub fn new(config: Arc<Config<E>>) -> Self {
        Self {
            // TODO: should probably sweep these, probably request timeout
            waiters: HashMap::new(),
            // TODO:
            config,
        }
    }
}

impl<E: Encoder> ConnectionHandler for ClientConnectionHandler<E> {
    type InternalEvent = InternalEvent<E::Encoded, E::Decoded>;
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
            await!(tcp_stream.write_all_async(&UPGRADE_REQUEST.as_bytes()))?;
            await!(tcp_stream.flush_async())?;
            let mut payload = [0; 1024];
            // TODO: this is just wrong
            // TODO: handle disconnect, bytes_read=0
            while let Ok(_bytes_read) = await!(tcp_stream.read_async(&mut payload)) {
                let response = String::from_utf8(payload.to_vec()).expect("Failed to make string");
                // TODO: case insensitive
                if response.contains(&"Upgrade") || response.contains(&"upgrade") {
                    return Ok(tcp_stream);
                } else {
                    return Err(LoquiError::UpgradeFailed.into());
                }
            }
            Err(LoquiError::UpgradeFailed.into())
        }
    }

    fn handshake(&mut self, mut reader_writer: FramedReaderWriter) -> Self::HandshakeFuture {
        async move {
            let hello = Self::make_hello();
            reader_writer = match await!(reader_writer.write(hello)) {
                Ok(read_writer) => read_writer,
                Err(e) => return Err((e.into(), None)),
            };

            // TODO: this error could be something for real?
            if let Some(Ok(frame)) = await!(reader_writer.reader.next()) {
                match Self::handle_handshake_frame(frame) {
                    Ok(ready) => Ok((ready, reader_writer)),
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
            DelegatedFrame::Response(response) => {
                let Response {
                    flags,
                    sequence_id,
                    payload,
                } = response;
                match self.waiters.remove(&sequence_id) {
                    Some(waiter_tx) => {
                        let response = self.config.encoder.decode(
                            transport_options.encoding,
                            is_compressed(&flags),
                            payload,
                        );
                        if let Err(_e) = waiter_tx.send(response) {
                            error!("Waiter is no longer listening.")
                        }
                    }
                    None => {
                        error!("No waiter for sequence_id. sequence_id={:?}", sequence_id);
                    }
                }
                None
            }
            DelegatedFrame::Push(_) => None,
            // XXX: hack for existential types :ablobgrimace:
            //
            // The type system needs a concrete impl Future, otherwise it can't infer the `T` of
            // `Option<T>`. Using None::<Self::HandleFrameFuture> results in a cycle.
            //
            // This code path should never be hit if the server implements loqui properly.
            DelegatedFrame::Request(_) => Some(
                async {
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
            InternalEvent::Request { payload, waiter_tx } => {
                let sequence_id = id_sequence.next();
                match self.config.encoder.encode(encoding, payload) {
                    Ok((payload, compressed)) => {
                        // Store the waiter so we can notify it when we get a response.
                        self.waiters.insert(sequence_id, waiter_tx);
                        let request = Request {
                            payload,
                            sequence_id,
                            flags: make_flags(compressed),
                        };
                        Some(request.into())
                    }
                    Err(e) => {
                        error!("Failed to encode payload. error={:?}", e);
                        None
                    }
                }
            }
            InternalEvent::Push { payload } => {
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
        }
    }
}

impl<E: Encoder> ClientConnectionHandler<E> {
    fn make_hello() -> Hello {
        Hello {
            flags: 0,
            version: VERSION,
            encodings: E::ENCODINGS.iter().map(|s| s.to_string()).collect(),
            compressions: E::COMPRESSIONS.iter().map(|s| s.to_string()).collect(),
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
        let ping_interval = Duration::from_millis(hello_ack.ping_interval_ms as u64);
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
