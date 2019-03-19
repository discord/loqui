#![feature(await_macro, async_await, futures_api)]

extern crate loqui;

use failure::Error;
use loqui::protocol::Request;
use loqui::server::{Handler, Server};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

const ADDRESS: &'static str = "127.0.0.1:8080";

struct EchoHandler {}

impl Handler for EchoHandler {
    fn handle_request(
        &self,
        request: Request,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send>> {
        Box::pin(async { Ok(request.payload) })
    }
}

fn main() {
    tokio::run_async(
        async {
            let server = Server {
                handler: Arc::new(EchoHandler {}),
            };
            let result = await!(server.listen_and_serve(ADDRESS.to_string()));
            println!("Server finished. result={:?}", result);
        },
    );
}
