#![feature(await_macro, async_await, futures_api)]

use failure::Error;
use loqui_client::Client;
use loqui_server::{Handler, RequestContext, Server};
use std::future::Future;
use std::sync::Arc;
use std::{thread, time::Duration};

const ADDRESS: &'static str = "127.0.0.1:3000";

struct EchoHandler {}

impl Handler for EchoHandler {
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
            tokio::spawn_async(
                async {
                    let server = Server {
                        handler: Arc::new(EchoHandler {}),
                        supported_encodings: vec!["json".to_string()],
                    };
                    let result = await!(server.listen_and_serve(ADDRESS.to_string()));
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
