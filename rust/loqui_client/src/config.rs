use bytesize::ByteSize;
use loqui_connection::Encoder;

#[derive(Debug, Clone)]
pub struct Config<E: Encoder> {
    pub encoder: E,
    pub max_payload_size: ByteSize,
}
