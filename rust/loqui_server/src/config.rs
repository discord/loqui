use super::request_handler::RequestHandler;
use bytesize::ByteSize;
use loqui_connection::EncoderFactory;
use std::marker::PhantomData;
use std::time::Duration;

pub struct Config<R: RequestHandler<F>, F: EncoderFactory> {
    _f: PhantomData<F>,
    pub request_handler: R,
    pub max_payload_size: ByteSize,
    pub ping_interval: Duration,
}

impl<F: EncoderFactory, R: RequestHandler<F>> Config<R, F> {
    pub fn new(request_handler: R, max_payload_size: ByteSize, ping_interval: Duration) -> Self {
        Self {
            _f: PhantomData,
            request_handler,
            max_payload_size,
            ping_interval,
        }
    }
}
