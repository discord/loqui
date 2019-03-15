#![feature(await_macro, async_await, futures_api)]

extern crate loqui;

use loqui::client::Client;
use loqui::server::run;
use std::{thread, time::Duration};

const ADDRESS: &'static str = "127.0.0.1:8080";

fn main() {
    tokio::run_async(
        async {
            let result = await!(run(ADDRESS));
            println!("Run result={:?}", result);
        },
    );
}
