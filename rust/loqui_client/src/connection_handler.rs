use crate::waiter::ResponseWaiter;
use crate::Config;
use bytesize::ByteSize;
use failure::{err_msg, Error};
use loqui_connection::handler::{DelegatedFrame, Handler, Ready};
use loqui_connection::{Encoder, EncoderFactory, IdSequence, LoquiError, ReaderWriter};
use loqui_protocol::frames::{
    Error as ErrorFrame, Frame, Hello, HelloAck, LoquiFrame, Push, Request, Response,
};
use loqui_protocol::upgrade::{Codec, UpgradeFrame};
use loqui_protocol::VERSION;
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
    Encoded: Serialize + Send,
    Decoded: DeserializeOwned + Send,
{
    Request {
        payload: Encoded,
        waiter: ResponseWaiter<Decoded>,
    },
    Push {
        payload: Encoded,
    },
}

pub struct ConnectionHandler<F: EncoderFactory> {
    waiters: HashMap<u32, ResponseWaiter<F::Decoded>>,
    config: Arc<Config>,
}

impl<F: EncoderFactory> ConnectionHandler<F> {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            waiters: HashMap::new(),
            config,
        }
    }
}

impl<F: EncoderFactory> Handler for ConnectionHandler<F> {
    type EncoderFactory = F;
    type InternalEvent = InternalEvent<F::Encoded, F::Decoded>;
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
        encoder: Arc<Box<Encoder<Encoded = F::Encoded, Decoded = F::Decoded>>>,
    ) -> Option<Self::HandleFrameFuture> {
        match frame {
            DelegatedFrame::Response(response) => {
                self.handle_response(response, encoder);
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
        event: InternalEvent<F::Encoded, F::Decoded>,
        id_sequence: &mut IdSequence,
        encoder: Arc<Box<Encoder<Encoded = F::Encoded, Decoded = F::Decoded>>>,
    ) -> Option<LoquiFrame> {
        // Forward Request and Push events to the connection so it can send them to the server.
        match event {
            InternalEvent::Request { payload, waiter } => {
                let sequence_id = id_sequence.next();
                self.handle_request(payload, sequence_id, waiter, encoder)
            }
            InternalEvent::Push { payload } => self.handle_push(payload, encoder),
        }
    }

    fn handle_ping(&mut self) {
        // Use to sweep dead waiters.
        let now = Instant::now();
        self.waiters
            .retain(|_sequence_id, waiter| waiter.deadline > now);
    }
}

impl<F: EncoderFactory> ConnectionHandler<F> {
    fn handle_push(
        &mut self,
        payload: F::Encoded,
        encoder: Arc<Box<dyn Encoder<Encoded = F::Encoded, Decoded = F::Decoded>>>,
    ) -> Option<LoquiFrame> {
        match encoder.encode(payload) {
            Ok(payload) => {
                let push = Push { payload, flags: 0 };
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
        payload: F::Encoded,
        sequence_id: u32,
        waiter: ResponseWaiter<F::Decoded>,
        encoder: Arc<Box<dyn Encoder<Encoded = F::Encoded, Decoded = F::Decoded>>>,
    ) -> Option<LoquiFrame> {
        if waiter.deadline <= Instant::now() {
            waiter.notify(Err(LoquiError::RequestTimeout.into()));
            return None;
        }

        match encoder.encode(payload) {
            Ok(payload) => {
                // Store the waiter so we can notify it when we get a response.
                self.waiters.insert(sequence_id, waiter);
                let request = Request {
                    payload,
                    sequence_id,
                    flags: 0,
                };
                Some(request.into())
            }
            Err(error) => {
                waiter.notify(Err(error));
                None
            }
        }
    }

    fn handle_response(
        &mut self,
        response: Response,
        encoder: Arc<Box<dyn Encoder<Encoded = F::Encoded, Decoded = F::Decoded>>>,
    ) {
        let Response {
            flags: _flags,
            sequence_id,
            payload,
        } = response;
        match self.waiters.remove(&sequence_id) {
            Some(waiter) => {
                let response = encoder.decode(payload);
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
            encodings: F::ENCODINGS
                .to_owned()
                .into_iter()
                .map(String::from)
                .collect(),
            // compression not supported
            compressions: vec![],
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
        let encoding = match F::find_encoding(hello_ack.encoding) {
            Some(encoding) => encoding,
            None => return Err(LoquiError::InvalidEncoding.into()),
        };

        // compression not supported
        if hello_ack.compression.is_some() {
            return Err(LoquiError::InvalidCompression.into());
        };
        let ping_interval = Duration::from_millis(u64::from(hello_ack.ping_interval_ms));
        Ok(Ready {
            ping_interval,
            encoding,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::future_utils::block_on_all;
    use tokio::await;

    #[derive(Clone)]
    struct BytesEncoder {}

    impl Encoder for BytesEncoder {
        type Decoded = Vec<u8>;
        type Encoded = Vec<u8>;

        const ENCODINGS: &'static [&'static str] = &["bytes"];
        const COMPRESSIONS: &'static [&'static str] = &[];

        fn decode(
            &self,
            _encoding: &'static str,
            _compressed: bool,
            payload: Vec<u8>,
        ) -> Result<Self::Decoded, Error> {
            Ok(payload)
        }

        fn encode(
            &self,
            _encoding: &'static str,
            payload: Self::Encoded,
        ) -> Result<(Vec<u8>, bool), Error> {
            Ok((payload, false))
        }
    }

    fn make_handler() -> ConnectionHandler<BytesEncoder> {
        let config = Config {
            encoder: BytesEncoder {},
            max_payload_size: ByteSize::b(5000),
            request_timeout: Duration::from_secs(5),
        };

        ConnectionHandler::new(Arc::new(config))
    }

    fn make_transport_options() -> TransportOptions {
        TransportOptions {
            encoding: "bytes",
            compression: None,
        }
    }

    #[test]
    fn it_handles_request_response() {
        let mut handler = make_handler();
        let transport_options = make_transport_options();
        let mut id_sequence = IdSequence::default();
        let (waiter, awaitable) = ResponseWaiter::new(Duration::from_secs(5));
        let payload = b"hello".to_vec();
        let request = handler
            .handle_internal_event(
                InternalEvent::Request {
                    payload: payload.clone(),
                    waiter,
                },
                &mut id_sequence,
                &transport_options,
            )
            .expect("no request");
        match request {
            LoquiFrame::Request(request) => {
                let response = Response {
                    sequence_id: request.sequence_id,
                    flags: 0,
                    payload: payload.clone(),
                };
                let frame = handler.handle_frame(response.into(), &transport_options);
                assert!(frame.is_none())
            }
            _other => panic!("request not returned"),
        }
        let result = block_on_all(async { await!(awaitable) }).unwrap();
        assert_eq!(result, payload)
    }

    #[test]
    fn it_handles_request_response_diff_sequence_id() {
        let mut handler = make_handler();
        let transport_options = make_transport_options();
        let mut id_sequence = IdSequence::default();
        let (waiter, awaitable) = ResponseWaiter::new(Duration::from_secs(1));
        let _request = handler
            .handle_internal_event(
                InternalEvent::Request {
                    payload: vec![],
                    waiter,
                },
                &mut id_sequence,
                &transport_options,
            )
            .expect("no request");
        let response = Response {
            sequence_id: id_sequence.next(),
            flags: 0,
            payload: vec![],
        };
        let _frame = handler.handle_frame(response.into(), &transport_options);
        let result = block_on_all(async { await!(awaitable) });
        assert!(result.is_err())
    }

}
