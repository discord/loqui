use bytesize::ByteSize;
use loqui_connection::Factory;
use std::marker::PhantomData;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config<F: Factory> {
    //pub encoder: E,
    _f: PhantomData<F>,
    pub max_payload_size: ByteSize,
    pub request_timeout: Duration,
    pub request_queue_size: usize,
}

impl<F: Factory> Config<F> {
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
