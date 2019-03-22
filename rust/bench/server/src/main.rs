#![feature(await_macro, async_await, futures_api)]

use failure::Error;
use loqui_server::{RequestContext, RequestHandler, Server};
use std::future::Future;
use std::sync::Arc;
use std::{thread, time::Duration};

const ADDRESS: &'static str = "127.0.0.1:8080";

struct EchoHandler {}

impl RequestHandler for EchoHandler {
    fn handle_request(
        &self,
        request: RequestContext,
    ) -> Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send> {
        Box::new(async { Ok(request.payload) })
    }
}

fn main() {
    tokio::run_async(
        async {
            let server = Server::new(Arc::new(EchoHandler {}), vec!["json".to_string()]);
            let result = await!(server.listen_and_serve(ADDRESS.to_string()));
            println!("Run result={:?}", result);
        },
    );
}
