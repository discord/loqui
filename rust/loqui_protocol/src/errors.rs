use failure::Fail;

#[derive(Debug, PartialEq, Fail)]
pub enum ProtocolError {
    #[fail(display = "Invalid opcode: {}", _0)]
    InvalidOpcode(u8),

    #[fail(display = "Invalid Payload: {}", _0)]
    InvalidPayload(String),

    #[fail(display = "Payload Size Too Large: {} bytes. Max bytes: {}", _0, _1)]
    PayloadTooLarge(u32, u32),
}
