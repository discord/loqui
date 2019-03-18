#![feature(await_macro, async_await, futures_api)]

extern crate loqui;

use failure::Error;
use loqui::client::Client;
use loqui::protocol::Request;
use loqui::server::{Handler, Server};
use std::future::Future;
use std::{thread, time::Duration};
use tokio_async_await::compat::forward::IntoAwaitable;
use std::sync::Arc;
use std::pin::Pin;
use tokio_async_await::compat::forward::IntoAwaitable;

const ADDRESS: &'static str = "127.0.0.1:8080";

struct EchoHandler {}

impl Handler for EchoHandler {
    fn handle_request(&self, request: Request) -> Pin<Box<dyn Future<Output=Result<Vec<u8>, Error>> + Send>> {
        Box::pin(futures::future::ok(request.payload).into_awaitable())
    }
}

fn main() {
    tokio::run_async(
        async {
            let server = Server {
                handler: Arc::new(EchoHandler {}),
            };
            //let server = Server::new(handler: EchoHandler{});
            let result = await!(server.serve(ADDRESS.to_string()));
            println!("Run result={:?}", result);
        }
    );
}
