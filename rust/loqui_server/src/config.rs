use super::request_handler::RequestHandler;
use bytesize::ByteSize;
use loqui_connection::Encoder;
use std::time::Duration;

pub struct Config<R: RequestHandler<E>, E: Encoder> {
    pub request_handler: R,
    pub max_payload_size: ByteSize,
    pub ping_interval: Duration,
    pub encoder: E,
}
