#![feature(await_macro, async_await, futures_api, existential_type)]

use bytesize::ByteSize;
use failure::Error;
use loqui_server::{Config, Encoder, RequestHandler, Server};
use std::future::Future;
use std::time::Duration;

const ADDRESS: &str = "127.0.0.1:8080";

struct EchoHandler {}

#[derive(Clone)]
struct BytesEncoder {}

impl RequestHandler<BytesEncoder> for EchoHandler {
    existential type RequestFuture: Future<Output = Vec<u8>>;
    existential type PushFuture: Send + Future<Output = ()>;

    fn handle_request(&self, request: Vec<u8>) -> Self::RequestFuture {
        async { request }
    }

    fn handle_push(&self, _push: Vec<u8>) -> Self::PushFuture {
        async {}
    }
}

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

fn main() {
    tokio::run_async(
        async {
            let config = Config {
                request_handler: EchoHandler {},
                max_payload_size: ByteSize::kb(5000),
                ping_interval: Duration::from_secs(5),
                encoder: BytesEncoder {},
            };
            let server = Server::new(config);
            let result = await!(server.listen_and_serve(ADDRESS.to_string()));
            println!("Run result={:?}", result);
        },
    );
}
