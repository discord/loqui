use super::request_handler::RequestHandler;
use bytesize::ByteSize;
use loqui_connection::Factory;
use std::time::Duration;
use std::marker::PhantomData;

pub struct Config<R: RequestHandler<F>, F: Factory> {
    _f: PhantomData<F>,
    pub request_handler: R,
    pub max_payload_size: ByteSize,
    pub ping_interval: Duration,
}
