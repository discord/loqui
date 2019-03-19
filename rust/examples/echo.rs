#![feature(await_macro, async_await, futures_api)]

extern crate loqui;

use failure::Error;
use loqui::client::Client;
use loqui::protocol::Request;
use loqui::server::{Handler, Server};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::{thread, time::Duration};

const ADDRESS: &'static str = "127.0.0.1:3000";

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
            tokio::spawn_async(
                async {
                    let server = Server {
                        handler: Arc::new(EchoHandler {}),
                    };
                    let result = await!(server.serve(ADDRESS.to_string()));
                    println!("Run result={:?}", result);
                },
            );
            thread::sleep(Duration::from_secs(1));
            let client = await!(Client::connect(ADDRESS)).unwrap();
            let messages = vec!["test", "test2", "test3"];

            for message in messages {
                let message = message.clone().to_string();
                let client = client.clone();
                tokio::spawn_async(
                    async move {
                        let resp = await!(client.request(message.as_bytes().to_vec()));
                        match resp {
                            Ok(resp) => {
                                dbg!(String::from_utf8(resp).unwrap());
                            }
                            Err(e) => {
                                dbg!(e);
                            }
                        }
                    },
                );
            }
        },
    );
}
