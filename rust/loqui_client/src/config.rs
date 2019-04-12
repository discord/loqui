use bytesize::ByteSize;
use loqui_connection::EncoderFactory;
use std::marker::PhantomData;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config<F: EncoderFactory> {
    //pub encoder: E,
    _f: PhantomData<F>,
    pub max_payload_size: ByteSize,
    pub request_timeout: Duration,
    pub request_queue_size: usize,
}

impl<F: EncoderFactory> Config<F> {
    pub fn new(
        max_payload_size: ByteSize,
        request_timeout: Duration,
        request_queue_size: usize,
    ) -> Self {
        Self {
            _f: PhantomData,
            max_payload_size,
            request_timeout,
            request_queue_size,
        }
    }
}
