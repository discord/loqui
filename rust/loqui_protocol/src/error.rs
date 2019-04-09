use failure::Fail;

#[derive(Debug, PartialEq, Fail)]
pub enum ProtocolError {
    #[fail(display = "Invalid opcode: {}", opcode)]
    InvalidOpcode { opcode: u8 },

    #[fail(display = "Invalid Payload: {}", reason)]
    InvalidPayload { reason: String },

    #[fail(
        display = "Payload Size Too Large: {} bytes. Max bytes: {}",
        actual, max
    )]
    PayloadTooLarge { actual: u32, max: u32 },
}
