#![feature(await_macro, async_await, futures_api, existential_type)]

use bytesize::ByteSize;
use loqui_bench_common::{BenchEncoderFactory, ADDRESS};
use loqui_server::{Config, RequestHandler, Server};
use std::future::Future;
use std::time::Duration;

struct EchoHandler {}

impl RequestHandler<BenchEncoderFactory> for EchoHandler {
    existential type RequestFuture: Future<Output = Vec<u8>>;
    existential type PushFuture: Send + Future<Output = ()>;

    fn handle_request(&self, request: Vec<u8>) -> Self::RequestFuture {
        async { request }
    }

    fn handle_push(&self, _push: Vec<u8>) -> Self::PushFuture {
        async {}
    }
}

fn main() {
    tokio::run_async(
        async {
            let config = Config::<EchoHandler, BenchEncoderFactory>::new(
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
