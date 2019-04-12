use failure::Error;
use loqui_client::{Encoder, EncoderFactory};
use std::sync::Arc;

#[derive(Clone)]
pub struct BenchEncoderFactory {}

impl EncoderFactory for BenchEncoderFactory {
    type Decoded = Vec<u8>;
    type Encoded = Vec<u8>;

    const ENCODINGS: &'static [&'static str] = &["msgpack", "identity"];
    const COMPRESSIONS: &'static [&'static str] = &[];

    fn make(
        _encoding: &'static str,
    ) -> Arc<Box<Encoder<Encoded = Self::Encoded, Decoded = Self::Decoded>>> {
        Arc::new(Box::new(IdentityEncoder {}))
    }
}

#[derive(Clone)]
struct IdentityEncoder {}

impl Encoder for IdentityEncoder {
    type Decoded = Vec<u8>;
    type Encoded = Vec<u8>;

    fn decode(&self, payload: Vec<u8>) -> Result<Self::Decoded, Error> {
        Ok(payload)
    }

    fn encode(&self, payload: Self::Encoded) -> Result<(Vec<u8>, bool), Error> {
        Ok((payload, false))
    }
}
