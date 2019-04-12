#![feature(await_macro, async_await, futures_api, existential_type)]

use bytesize::ByteSize;
use failure::Error;
use loqui_server::{Config, Encoder, EncoderFactory, RequestHandler, Server};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

const ADDRESS: &str = "127.0.0.1:8080";

struct EchoHandler {}

#[derive(Clone)]
struct BytesEncoder {}

impl RequestHandler<EncoderEncoderFactory> for EchoHandler {
    existential type RequestFuture: Future<Output = Vec<u8>>;
    existential type PushFuture: Send + Future<Output = ()>;

    fn handle_request(&self, request: Vec<u8>) -> Self::RequestFuture {
        async { request }
    }

    fn handle_push(&self, _push: Vec<u8>) -> Self::PushFuture {
        async {}
    }
}

#[derive(Clone)]
struct EncoderEncoderFactory {}

impl EncoderFactory for EncoderEncoderFactory {
    type Decoded = Vec<u8>;
    type Encoded = Vec<u8>;

    const ENCODINGS: &'static [&'static str] = &["msgpack", "identity"];
    const COMPRESSIONS: &'static [&'static str] = &[];

    fn make(
        _encoding: &'static str,
    ) -> Arc<Box<Encoder<Encoded = Self::Encoded, Decoded = Self::Decoded>>> {
        Arc::new(Box::new(IdentityEncoder {}))
    }
}

#[derive(Clone)]
struct IdentityEncoder {}

impl Encoder for IdentityEncoder {
    type Decoded = Vec<u8>;
    type Encoded = Vec<u8>;

    fn decode(&self, payload: Vec<u8>) -> Result<Self::Decoded, Error> {
        Ok(payload)
    }

    fn encode(&self, payload: Self::Encoded) -> Result<(Vec<u8>, bool), Error> {
        Ok((payload, false))
    }
}

fn main() {
    tokio::run_async(
        async {
            let config = Config::<EchoHandler, EncoderEncoderFactory>::new(
                EchoHandler {},
                ByteSize::kb(5000),
                Duration::from_secs(5),
            );
            let server = Server::new(config);
            let result = await!(server.listen_and_serve(ADDRESS.to_string()));
            println!("Run result={:?}", result);
        },
    );
}
