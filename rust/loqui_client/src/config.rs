use bytesize::ByteSize;
use loqui_connection::Encoder;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config<E: Encoder> {
    pub encoder: E,
    pub max_payload_size: ByteSize,
    pub request_timeout: Duration,
    pub request_queue_size: usize,
}
