#![feature(await_macro, async_await, futures_api)]
#![feature(existential_type)]

#[macro_use]
extern crate log;

use bytesize::ByteSize;
use chrono;
use failure::Error;
use fern;
use loqui_client::{Client, Config as ClientConfig};
use loqui_server::{Config as ServerConfig, Encoder, RequestHandler, Server};
use std::future::Future;
use std::net::SocketAddr;
use std::{thread, time::Duration};

const CLIENT_ADDRESS: &str = "127.0.0.1:8080";
const SERVER_ADDRESS: &str = "127.0.0.1:8080";

struct EchoHandler {}

#[derive(Clone)]
struct StringEncoder {}

impl RequestHandler<StringEncoder> for EchoHandler {
    existential type RequestFuture: Future<Output = String>;
    existential type PushFuture: Send + Future<Output = ()>;

    fn handle_request(&self, request: String) -> Self::RequestFuture {
        debug!("Handling request: {}", request);
        async { request }
    }

    fn handle_push(&self, push: String) -> Self::PushFuture {
        debug!("Handling push: {}", push);
        async {}
    }
}

impl Encoder for StringEncoder {
    type Decoded = String;
    type Encoded = String;

    const ENCODINGS: &'static [&'static str] = &["string", "bytes"];
    const COMPRESSIONS: &'static [&'static str] = &[];

    fn decode(
        &self,
        _encoding: &'static str,
        _compressed: bool,
        payload: Vec<u8>,
    ) -> Result<Self::Decoded, Error> {
        String::from_utf8(payload).map_err(Error::from)
    }

    fn encode(
        &self,
        _encoding: &'static str,
        payload: Self::Encoded,
    ) -> Result<(Vec<u8>, bool), Error> {
        Ok((payload.as_bytes().to_vec(), false))
    }
}

fn main() -> Result<(), Error> {
    configure_logging()?;
    tokio::run_async(
        async {
            spawn_server();
            // Wait for server to start.
            thread::sleep(Duration::from_secs(1));
            await!(client_send_loop());
        },
    );
    Ok(())
}

async fn client_send_loop() {
    let config = ClientConfig {
        max_payload_size: ByteSize::kb(5000),
        encoder: StringEncoder {},
        request_timeout: Duration::from_secs(5),
    };

    let address: SocketAddr = CLIENT_ADDRESS.parse().expect("Failed to parse address.");
    let client = await!(Client::connect(address, config)).expect("Failed to connect");

    let messages = &["test", "test2", "test3"];
    loop {
        for message in messages {
            let client = client.clone();
            tokio::spawn_async(
                async move {
                    if let Err(e) = await!(client.push(message.to_string())) {
                        error!("Push failed. error={:?}", e);
                    }
                    match await!(client.request(message.to_string())) {
                        Ok(response) => {
                            info!("Received response: {}", response);
                        }
                        Err(e) => {
                            error!("Request failed. error={}", e);
                        }
                    }
                },
            );
        }
        thread::sleep(Duration::from_secs(1));
    }
}

fn spawn_server() {
    tokio::spawn_async(
        async {
            let config = ServerConfig {
                request_handler: EchoHandler {},
                max_payload_size: ByteSize::kb(5000),
                ping_interval: Duration::from_secs(5),
                encoder: StringEncoder {},
            };
            let server = Server::new(config);
            let result = await!(server.listen_and_serve(SERVER_ADDRESS.to_string()));
            println!("Run result={:?}", result);
        },
    );
}

fn configure_logging() -> Result<(), Error> {
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
        .apply()
        .map_err(Error::from)
}
