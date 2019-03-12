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
            let mut client = await!(Client::connect(ADDRESS)).unwrap();
            let response = await!(client.request("test".to_string()));
            println!("Response={:?}", response);
        },
    );
}
