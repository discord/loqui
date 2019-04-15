use std::net::SocketAddr;

mod encoders;

pub use encoders::BenchEncoderFactory;

const ADDRESS: &str = "127.0.0.1:8080";

pub fn make_socket_address() -> SocketAddr {
    ADDRESS.parse().expect("Failed to parse address.")
}
