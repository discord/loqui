#![feature(await_macro, async_await, futures_api)]

extern crate loqui;

use loqui::client::Client;
use loqui::server::{Handler, Request, Response, Server};
use std::{thread, time::Duration};

const ADDRESS: &'static str = "127.0.0.1:8080";

struct EchoHandler {}

impl Handler for EchoHandler {
    // TODO: this should probably be loqui frames
    async fn handle_request(request: Request) -> Response {}
}

fn main() {
    tokio::run_async(
        async {
            let server = Server {
                handler: EchoHandler {},
            };
            //let server = Server::new(handler: EchoHandler{});
            let result = server.serve(ADDRESS);
            println!("Run result={:?}", result);
        },
    );
}
