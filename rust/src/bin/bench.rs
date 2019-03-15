#![feature(await_macro, async_await, futures_api)]

extern crate loqui;

use loqui::client::Client;
use std::{thread, time::Duration};

const ADDRESS: &'static str = "127.0.0.1:8080";

fn make_message() -> Vec<u8> {
    "message".as_bytes().to_vec()
}

async fn do_work(client: Client) {
    dbg!("do_work");
    let message = dbg!(make_message());
    match await!(client.request(message)) {
        Ok(resp) => {}
        Err(e) => {
            dbg!(e);
        }
    }
}

async fn work_loop(client: Client) {
    loop {
        await!(do_work(client.clone()));
    }
}

fn log_loop() {
    let mut i = 0;
    loop {
        i += 1;
    }
}

fn main() {
    tokio::run_async(
        async {
            let client = await!(Client::connect(ADDRESS)).unwrap();
            for _ in 0..10 {
                tokio::spawn_async(work_loop(client.clone()));
            }

        },
    );
    log_loop();
}
