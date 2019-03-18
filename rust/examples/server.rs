#![feature(await_macro, async_await, futures_api)]

extern crate loqui;

use failure::Error;
use loqui::client::Client;
use loqui::server::{Handler, SRequest, SResponse, Server};
use std::future::Future;
use std::{thread, time::Duration};

const ADDRESS: &'static str = "127.0.0.1:8080";

struct EchoHandler {}

impl Handler for EchoHandler {
    // TODO: this should probably be loqui frames
    async fn handle_request(&self, request: SRequest) -> Result<SResponse, Error> {
        Ok(SResponse {})
    }
}

fn main() {
    tokio::run_async(
        async {
            let server = Server {
                handler: Box::new(EchoHandler {}),
            };
            //let server = Server::new(handler: EchoHandler{});
            let result = await!(server.serve(ADDRESS.to_string()));
            println!("Run result={:?}", result);
        }
    );
}
