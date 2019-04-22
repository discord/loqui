use bytesize::ByteSize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    /// The maximum size of the payload part of a loqui frame.
    pub max_payload_size: ByteSize,
    /// The duration of time from when a client makes a request to when we stop waiting for a response
    /// from the server.
    pub request_timeout: Duration,
    /// The number of requests the supervisor will buffer before back pressuring the client.
    /// The supervisor buffer begins filling up while it is reconnecting to the server.
    pub request_queue_size: usize,
}
