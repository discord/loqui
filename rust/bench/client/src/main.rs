#![feature(await_macro, async_await, futures_api)]

use bytesize::ByteSize;
use chrono;
use failure::Error;
use fern;
use log;
use loqui_client::{Client, Config, Encoder};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const ADDRESS: &'static str = "127.0.0.1:8080";

#[derive(Clone, Default)]
struct State {
    request_count: Arc<AtomicUsize>,
    failed_requests: Arc<AtomicUsize>,
    in_flight: Arc<AtomicUsize>,
    max_age: Arc<AtomicUsize>,
    request_time: Arc<AtomicUsize>,
}

fn make_message() -> Vec<u8> {
    "hello world".as_bytes().to_vec()
}

async fn do_work(client: Client<BytesEncoder>, state: State) {
    let message = make_message();
    let start = Instant::now();
    state.in_flight.fetch_add(1, Ordering::SeqCst);

    match await!(client.request(message)) {
        Ok(payload) => {
            if &payload[..] != "hello world".as_bytes() {
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

async fn work_loop(client: Client<BytesEncoder>, state: State) {
    loop {
        await!(do_work(client.clone(), state.clone()));
    }
}

fn log_loop(state: State) {
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
        println!(
            "{} total requests ({}/sec). last log {} sec ago. {} failed, {} in flight, {} µs max, {} µs avg response time",
            request_count, req_sec, elapsed, failed_requests, in_flight, max_age, avg_time
        );
        last_request_count = request_count;
        last = now;
        state.max_age.store(0, Ordering::SeqCst);
        state.request_time.store(0, Ordering::SeqCst);
    }
}

#[derive(Clone)]
struct BytesEncoder {}
impl Encoder for BytesEncoder {
    type Decoded = Vec<u8>;
    type Encoded = Vec<u8>;

    const ENCODINGS: &'static [&'static str] = &["string"];
    const COMPRESSIONS: &'static [&'static str] = &[];

    fn decode(
        &self,
        _encoding: &'static str,
        _compressed: bool,
        payload: Vec<u8>,
    ) -> Result<Self::Decoded, Error> {
        Ok(payload)
    }

    fn encode(
        &self,
        encoding: &'static str,
        payload: Self::Encoded,
    ) -> Result<(Vec<u8>, bool), Error> {
        Ok((payload, false))
    }
}

fn main() -> Result<(), Error> {
    let state = State::default();
    let log_state = state.clone();
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}:{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.level(),
                record.target(),
                record.line().unwrap_or(0),
                message
            ));
        })
        .level(log::LevelFilter::Debug)
        .level_for("tokio_core", log::LevelFilter::Warn)
        .level_for("tokio_reactor", log::LevelFilter::Warn)
        .chain(std::io::stdout())
        .apply()?;

    tokio::run_async(
        async move {
            tokio::spawn_async(
                async move {
                    log_loop(log_state.clone());
                },
            );

            let config = Config {
                max_payload_size: ByteSize::kb(5000),
                encoder: BytesEncoder {},
            };
            let client = await!(Client::connect(ADDRESS, config)).unwrap();
            for _ in 0..100 {
                tokio::spawn_async(work_loop(client.clone(), state.clone()));
            }
        },
    );
    Ok(())
}
