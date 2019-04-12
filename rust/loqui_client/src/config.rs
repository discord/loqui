use bytesize::ByteSize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub max_payload_size: ByteSize,
    pub request_timeout: Duration,
    pub request_queue_size: usize,
}
