use super::request_handler::RequestHandler;
use bytesize::ByteSize;
use std::time::Duration;

/// Configuration for the server.
pub struct Config<R: RequestHandler> {
    /// Handler for push and request.
    pub request_handler: R,
    /// Maximum size of the payload part of a loqui frame.
    pub max_payload_size: ByteSize,
    /// The duration between ping frames being sent.
    pub ping_interval: Duration,
    /// The duration before we give up handshaking.
    pub handshake_timeout: Duration,
}
