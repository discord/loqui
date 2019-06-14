#![feature(await_macro, async_await)]
#![feature(existential_type)]

#[macro_use]
extern crate log;

use bytesize::ByteSize;
use chrono;
use failure::Error;
use fern;
use futures_timer::Delay;
use loqui_client::{Client, Config as ClientConfig};
use loqui_server::{Config as ServerConfig, RequestHandler, Server};
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::{thread, time::Duration};
use tokio_futures::compat::{forward::IntoAwaitable, infallible_into_01};

const ADDRESS: &str = "127.0.0.1:8080";

struct EchoHandler {}

impl RequestHandler for EchoHandler {
    existential type RequestFuture: Future<Output = Vec<u8>>;
    existential type PushFuture: Send + Future<Output = ()>;

    fn handle_request(&self, payload: Vec<u8>, _encoding: &'static str) -> Self::RequestFuture {
        let request: String = String::from_utf8(payload).expect("Failed to decode.");
        debug!("Handling request: {}", request);
        async move { request.as_bytes().to_vec() }
    }

    fn handle_push(&self, payload: Vec<u8>, _encoding: &'static str) -> Self::PushFuture {
        let request: String = String::from_utf8(payload).expect("Failed to decode.");
        debug!("Handling push: {}", request);
        async {}
    }
}

fn main() -> Result<(), Error> {
    configure_logging()?;
    tokio::run(infallible_into_01(async {
        spawn_server();
        // Wait for server to start.
        thread::sleep(Duration::from_secs(1));
        client_send_loop().await;
    }));
    Ok(())
}

const SUPPORTED_ENCODINGS: &[&str] = &["string"];

async fn client_send_loop() {
    let config = ClientConfig {
        max_payload_size: ByteSize::kb(5000),
        request_timeout: Duration::from_secs(5),
        handshake_timeout: Duration::from_secs(5),
        supported_encodings: SUPPORTED_ENCODINGS,
    };

    let address: SocketAddr = ADDRESS.parse().expect("Failed to parse address.");
    let client = Arc::new(Client::start_connect(address, config).await.expect("Failed to connect"));
    client.await_ready().await.expect("Ready failed");

    let messages = &["test", "test2", "test3"];
    loop {
        for message in messages {
            let client = client.clone();
            tokio::spawn(infallible_into_01(async move {
                if let Err(e) = client.push(message.as_bytes().to_vec()).await {
                    error!("Push failed. error={:?}", e);
                }
                match client.request(message.as_bytes().to_vec()).await {
                    Ok(response) => {
                        info!(
                            "Received response: {}",
                            String::from_utf8(response).expect("Failed to decode")
                        );
                    }
                    Err(e) => {
                        error!("Request failed. error={}", e);
                    }
                }
            }));
        }

        Delay::new(Duration::from_secs(1))
            .into_awaitable()
            .await
            .expect("Failed to delay.");
    }
}

fn spawn_server() {
    tokio::spawn(infallible_into_01(async {
        let config = ServerConfig {
            request_handler: EchoHandler {},
            max_payload_size: ByteSize::kb(5000),
            ping_interval: Duration::from_secs(5),
            handshake_timeout: Duration::from_secs(5),
            supported_encodings: SUPPORTED_ENCODINGS,
        };
        let server = Server::new(config);
        let address: SocketAddr = ADDRESS.parse().expect("Failed to parse address.");
        let result = await!(server.listen_and_serve(address));
        println!("Run result={:?}", result);
    }));
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
