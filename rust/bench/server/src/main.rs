#![feature(await_macro, async_await, futures_api, existential_type)]

use bytesize::ByteSize;
use failure::Error;
use loqui_server::{Config, Encoder, RequestHandler, Server};
use std::future::Future;
use std::time::Duration;

const ADDRESS: &'static str = "127.0.0.1:8080";

struct EchoHandler {}

#[derive(Clone)]
struct BytesEncoder {}

impl RequestHandler<BytesEncoder> for EchoHandler {
    existential type RequestFuture: Future<Output = String>;
    existential type PushFuture: Send + Future<Output = ()>;

    fn handle_request(&self, request: String) -> Self::RequestFuture {
        async { request }
    }

    fn handle_push(&self, _push: String) -> Self::PushFuture {
        async {}
    }
}

impl Encoder for BytesEncoder {
    type Decoded = String;
    type Encoded = String;

    const ENCODINGS: &'static [&'static str] = &["bytes"];
    const COMPRESSIONS: &'static [&'static str] = &[];

    fn decode(
        &self,
        _encoding: &'static str,
        _compressed: bool,
        payload: Vec<u8>,
    ) -> Result<Self::Decoded, Error> {
        String::from_utf8(payload).map_err(Error::from)
    }

    fn encode(
        &self,
        _encoding: &'static str,
        payload: Self::Encoded,
    ) -> Result<(Vec<u8>, bool), Error> {
        Ok((payload.as_bytes().to_vec(), false))
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
