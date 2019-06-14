#![feature(await_macro, async_await)]

use bytesize::ByteSize;
use failure::Error;
#[macro_use]
extern crate log;
use loqui_bench_common::{configure_logging, make_socket_address};
use loqui_client::{Client, Config};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tokio_futures::compat::infallible_into_01;

#[derive(Default)]
struct State {
    request_count: AtomicUsize,
    failed_requests: AtomicUsize,
    in_flight: AtomicUsize,
    max_age: AtomicUsize,
    request_time: AtomicUsize,
}

fn make_message() -> Vec<u8> {
    b"hello world".to_vec()
}

async fn do_work(client: Arc<Client>, state: Arc<State>) {
    let message = make_message();
    let start = Instant::now();
    state.in_flight.fetch_add(1, Ordering::SeqCst);

    match client.request(message).await {
        Ok(payload) => {
            if &payload[..] != b"hello world" {
                state.failed_requests.fetch_add(1, Ordering::SeqCst);
            } else {
                state.request_count.fetch_add(1, Ordering::SeqCst);
            }
        }
        Err(e) => {
            state.failed_requests.fetch_add(1, Ordering::SeqCst);
            dbg!(e);
        }
    }

    let age = Instant::now().duration_since(start).as_micros() as usize;
    state.request_time.fetch_add(age, Ordering::SeqCst);

    if age > state.max_age.load(Ordering::SeqCst) {
        state.max_age.store(age, Ordering::SeqCst)
    }

    state.in_flight.fetch_sub(1, Ordering::SeqCst);
}

async fn work_loop(client: Arc<Client>, state: Arc<State>) {
    loop {
        do_work(client.clone(), state.clone()).await;
    }
}

fn log_loop(state: Arc<State>) {
    let mut last_request_count = 0;
    let mut last = Instant::now();
    loop {
        thread::sleep(Duration::from_secs(1));
        let now = Instant::now();
        let elapsed = now.duration_since(last).as_millis() as f64 / 1000.0;
        let request_count = state.request_count.load(Ordering::SeqCst);
        let req_sec = (request_count - last_request_count) as f64 / elapsed;
        let avg_time = if request_count > last_request_count {
            state.request_time.load(Ordering::SeqCst) / (request_count - last_request_count)
        } else {
            0
        };

        let failed_requests = state.failed_requests.load(Ordering::SeqCst);
        let in_flight = state.in_flight.load(Ordering::SeqCst);
        let max_age = state.max_age.load(Ordering::SeqCst);
        info!(
            "{} total requests ({}/sec). last log {} sec ago. {} failed, {} in flight, {} µs max, {} µs avg response time",
            request_count, req_sec, elapsed, failed_requests, in_flight, max_age, avg_time
        );
        last_request_count = request_count;
        last = now;
        state.max_age.store(0, Ordering::SeqCst);
        state.request_time.store(0, Ordering::SeqCst);
    }
}

fn main() -> Result<(), Error> {
    let state = Arc::new(State::default());
    let log_state = state.clone();
    configure_logging()?;

    tokio::run(infallible_into_01(async move {
        tokio::spawn(infallible_into_01(async move {
            log_loop(log_state.clone());
        }));

        let config = Config {
            max_payload_size: ByteSize::kb(5000),
            request_timeout: Duration::from_secs(5),
            handshake_timeout: Duration::from_secs(5),
            supported_encodings: &["msgpack", "identity"],
        };
        let client = Arc::new(
            Client::start_connect(make_socket_address(), config)
                .await
                .expect("Failed to connect"),
        );
        client.await_ready().await.expect("Ready failed");
        for _ in 0..100 {
            tokio::spawn(infallible_into_01(work_loop(client.clone(), state.clone())));
        }
    }));
    Ok(())
}
