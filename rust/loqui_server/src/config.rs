use super::request_handler::RequestHandler;
use bytesize::ByteSize;
use loqui_connection::EncoderFactory;
use std::marker::PhantomData;
use std::time::Duration;

pub struct Config<R: RequestHandler> {
    pub request_handler: R,
    pub max_payload_size: ByteSize,
    pub ping_interval: Duration,
}
