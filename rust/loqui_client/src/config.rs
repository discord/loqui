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
}
