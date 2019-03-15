#![feature(await_macro, async_await, futures_api)]

extern crate loqui;

use loqui::client::Client;
use loqui::server::run;
use std::{thread, time::Duration};

const ADDRESS: &'static str = "127.0.0.1:3000";

fn main() {
    tokio::run_async(
        async {
            tokio::spawn_async(
                async {
                    let result = await!(run(ADDRESS));
                    println!("Run result={:?}", result);
                },
            );
            thread::sleep(Duration::from_secs(1));
            let client = await!(Client::connect(ADDRESS)).unwrap();
            let messages = vec![
                "test",
                "test2",
                "test3",
            ];

            for message in messages {
                let message = message.clone().to_string();
                let client = client.clone();
                tokio::spawn_async(async move {
                    let resp = await!(client.request(message.as_bytes().to_vec()));
                    match resp {
                        Ok(resp) => {
                            dbg!(String::from_utf8(resp).unwrap());
                        },
                        Err(e) => {
                            dbg!(e);
                        }
                    }
                });
            }
        },
    );
}
