use byteorder::{BigEndian, ByteOrder};
use bytes::{BufMut, BytesMut};

use crate::errors::ProtocolError;

type DecodeResult<T> = Result<Option<T>, ProtocolError>;

#[derive(Debug, PartialEq)]
pub enum LoquiFrame {
    Hello(Hello),
    HelloAck(HelloAck),
    Ping(Ping),
    Pong(Pong),
    Request(Request),
    Response(Response),
    Push(Push),
    GoAway(GoAway),
    Error(Error),
}

pub trait Frame: Sized + 'static {
    ///
    /// Opcode of the frame.
    ///
    const OPCODE: u8;
    ///
    /// Header size in bytes, including the payload size field.
    ///
    const HEADER_SIZE_IN_BYTES: usize;

    ///
    /// Put the header bytes into the destination buffer. Space is already reserved. Do not
    /// put the payload size, that will be handled for you based on what is returned by
    /// Frame::payload()
    ///
    fn put_header(&self, dst: &mut BytesMut);
    ///
    /// Return the payload bytes that should be encoded.
    ///
    fn payload(self) -> Option<Vec<u8>>;
    ///
    /// Read the payload size from the buffer.
    ///
    fn read_payload_size(buf: &mut BytesMut) -> u32;
    ///
    /// Given a buf that is a complete frame, parse and return the Frame.
    ///
    fn from_buf(buf: &BytesMut) -> DecodeResult<Self>;
}

impl LoquiFrame {
    pub fn opcode(&self) -> u8 {
        match self {
            LoquiFrame::Hello(_) => Hello::OPCODE,
            _ => 0,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Hello {
    pub flags: u8,
    pub version: u8,
    pub encodings: Vec<String>,
    pub compressions: Vec<String>,
}

impl Frame for Hello {
    const OPCODE: u8 = 1;
    const HEADER_SIZE_IN_BYTES: usize = 7;

    fn put_header(&self, dst: &mut BytesMut) {
        dst.put_u8(Self::OPCODE);
        dst.put_u8(self.flags);
        dst.put_u8(self.version);
    }

    fn payload(self) -> Option<Vec<u8>> {
        let payload = format!(
            "{}|{}",
            self.encodings.join(","),
            self.compressions.join(","),
        );
        Some(payload.as_bytes().to_vec())
    }

    fn read_payload_size(buf: &mut BytesMut) -> u32 {
        BigEndian::read_u32(&buf[3..7])
    }

    fn from_buf(buf: &BytesMut) -> Result<Option<Self>, ProtocolError> {
        let flags = buf[1];
        let version = buf[2];
        let payload = ::std::str::from_utf8(&buf[7..])
            .map_err(|_| ProtocolError::InvalidPayload("Failed to decode as string".into()))?;

        let settings: Vec<&str> = payload.split('|').collect();
        if settings.len() != 2 {
            return Err(ProtocolError::InvalidPayload(
                "Expected exactly two settings.".into(),
            ));
        }

        let encodings = settings[0]
            .split_terminator(',')
            .map(String::from)
            .collect::<Vec<String>>();

        let compressions = settings[1]
            .split_terminator(',')
            .map(String::from)
            .collect::<Vec<String>>();

        Ok(Some(Self {
            flags,
            version,
            encodings,
            compressions,
        }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct HelloAck {
    pub flags: u8,
    pub ping_interval_ms: u32,
    pub encoding: String,
    pub compression: Option<String>,
}

impl Frame for HelloAck {
    const OPCODE: u8 = 2;
    const HEADER_SIZE_IN_BYTES: usize = 10;

    #[inline]
    fn put_header(&self, dst: &mut BytesMut) {
        dst.put_u8(Self::OPCODE);
        dst.put_u8(self.flags);
        dst.put_u32_be(self.ping_interval_ms);
    }

    #[inline]
    fn payload(self) -> Option<Vec<u8>> {
        Some(
            format!(
                "{}|{}",
                self.encoding,
                self.compression.unwrap_or_else(|| "".to_string())
            )
            .as_bytes()
            .to_vec(),
        )
    }

    #[inline]
    fn read_payload_size(buf: &mut BytesMut) -> u32 {
        BigEndian::read_u32(&buf[6..10])
    }

    // TODO: make sure inline everywhere
    #[inline]
    fn from_buf(buf: &BytesMut) -> DecodeResult<Self> {
        let flags = buf[1];
        let ping_interval_ms = BigEndian::read_u32(&buf[2..6]);

        let payload = ::std::str::from_utf8(&buf[10..])
            .map_err(|_| ProtocolError::InvalidPayload("Failed to decode as string".into()))?;

        let settings: Vec<&str> = payload.split('|').collect();
        if settings.len() != 2 {
            return Err(ProtocolError::InvalidPayload(
                "Expected exactly two settings.".into(),
            ));
        }
        let encoding = settings[0].to_string();
        let compression = settings[1];
        let compression = if compression == "" {
            None
        } else {
            Some(compression.to_string())
        };

        Ok(Some(Self {
            flags,
            ping_interval_ms,
            encoding,
            compression,
        }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Ping {
    pub flags: u8,
    pub sequence_id: u32,
}

impl Frame for Ping {
    const OPCODE: u8 = 3;
    const HEADER_SIZE_IN_BYTES: usize = 6;

    fn put_header(&self, dst: &mut BytesMut) {
        dst.put_u8(Self::OPCODE);
        dst.put_u8(self.flags);
        dst.put_u32_be(self.sequence_id);
    }

    fn payload(self) -> Option<Vec<u8>> {
        None
    }

    fn read_payload_size(_buf: &mut BytesMut) -> u32 {
        0
    }

    fn from_buf(buf: &BytesMut) -> Result<Option<Self>, ProtocolError> {
        let flags = buf[1];
        let sequence_id = BigEndian::read_u32(&buf[2..6]);
        Ok(Some(Self { flags, sequence_id }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Pong {
    pub flags: u8,
    pub sequence_id: u32,
}

impl Frame for Pong {
    const OPCODE: u8 = 4;
    const HEADER_SIZE_IN_BYTES: usize = 6;

    fn put_header(&self, dst: &mut BytesMut) {
        dst.put_u8(Self::OPCODE);
        dst.put_u8(self.flags);
        dst.put_u32_be(self.sequence_id);
    }

    fn payload(self) -> Option<Vec<u8>> {
        None
    }

    fn read_payload_size(_buf: &mut BytesMut) -> u32 {
        0
    }

    fn from_buf(buf: &BytesMut) -> Result<Option<Self>, ProtocolError> {
        let flags = buf[1];
        let sequence_id = BigEndian::read_u32(&buf[2..6]);
        Ok(Some(Self { flags, sequence_id }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Request {
    pub flags: u8,
    pub sequence_id: u32,
    pub payload: Vec<u8>,
}

impl Frame for Request {
    const OPCODE: u8 = 5;
    const HEADER_SIZE_IN_BYTES: usize = 10;

    fn put_header(&self, dst: &mut BytesMut) {
        dst.put_u8(Self::OPCODE);
        dst.put_u8(self.flags);
        dst.put_u32_be(self.sequence_id);
    }

    fn payload(self) -> Option<Vec<u8>> {
        Some(self.payload)
    }

    fn read_payload_size(buf: &mut BytesMut) -> u32 {
        BigEndian::read_u32(&buf[6..10])
    }

    fn from_buf(buf: &BytesMut) -> DecodeResult<Self> {
        let flags = buf[1];
        let sequence_id = BigEndian::read_u32(&buf[2..6]);
        let payload = buf[10..].to_vec();
        Ok(Some(Self {
            flags,
            sequence_id,
            payload,
        }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Response {
    pub flags: u8,
    pub sequence_id: u32,
    pub payload: Vec<u8>,
}

impl Frame for Response {
    const OPCODE: u8 = 6;
    const HEADER_SIZE_IN_BYTES: usize = 10;

    fn put_header(&self, dst: &mut BytesMut) {
        // TODO:
        //dst.put_u8(99);
        dst.put_u8(Self::OPCODE);
        dst.put_u8(self.flags);
        dst.put_u32_be(self.sequence_id);
    }

    fn payload(self) -> Option<Vec<u8>> {
        Some(self.payload)
    }

    fn read_payload_size(buf: &mut BytesMut) -> u32 {
        BigEndian::read_u32(&buf[6..10])
    }

    fn from_buf(buf: &BytesMut) -> Result<Option<Self>, ProtocolError> {
        let flags = buf[1];
        let sequence_id = BigEndian::read_u32(&buf[2..6]);
        let payload = buf[10..].to_vec();
        Ok(Some(Self {
            flags,
            sequence_id,
            payload,
        }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Push {
    pub flags: u8,
    pub payload: Vec<u8>,
}

impl Frame for Push {
    const OPCODE: u8 = 7;
    const HEADER_SIZE_IN_BYTES: usize = 6;

    fn put_header(&self, dst: &mut BytesMut) {
        dst.put_u8(Self::OPCODE);
        dst.put_u8(self.flags);
    }

    fn payload(self) -> Option<Vec<u8>> {
        Some(self.payload)
    }

    fn read_payload_size(buf: &mut BytesMut) -> u32 {
        BigEndian::read_u32(&buf[2..6])
    }

    fn from_buf(buf: &BytesMut) -> Result<Option<Self>, ProtocolError> {
        let flags = buf[1];
        let payload = buf[6..].to_vec();
        Ok(Some(Self { flags, payload }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct GoAway {
    pub flags: u8,
    pub code: u16,
    pub payload: Vec<u8>,
}

impl Frame for GoAway {
    const OPCODE: u8 = 8;
    const HEADER_SIZE_IN_BYTES: usize = 8;

    fn put_header(&self, dst: &mut BytesMut) {
        dst.put_u8(Self::OPCODE);
        dst.put_u8(self.flags);
        dst.put_u16_be(self.code);
    }

    fn payload(self) -> Option<Vec<u8>> {
        Some(self.payload)
    }

    fn read_payload_size(buf: &mut BytesMut) -> u32 {
        BigEndian::read_u32(&buf[4..8])
    }

    fn from_buf(buf: &BytesMut) -> Result<Option<Self>, ProtocolError> {
        let flags = buf[1];
        let code = BigEndian::read_u16(&buf[2..4]);
        let payload = buf[8..].to_vec();

        Ok(Some(Self {
            flags,
            code,
            payload,
        }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Error {
    pub flags: u8,
    pub sequence_id: u32,
    pub code: u16,
    pub payload: Vec<u8>,
}

impl Frame for Error {
    const OPCODE: u8 = 9;
    const HEADER_SIZE_IN_BYTES: usize = 12;

    // TODO: does this need to be a result?
    fn put_header(&self, dst: &mut BytesMut) {
        dst.put_u8(Self::OPCODE);
        dst.put_u8(self.flags);
        dst.put_u32_be(self.sequence_id);
        dst.put_u16_be(self.code);
    }

    fn payload(self) -> Option<Vec<u8>> {
        Some(self.payload)
    }

    fn read_payload_size(buf: &mut BytesMut) -> u32 {
        BigEndian::read_u32(&buf[8..12])
    }

    // TODO: does this need to be a result?
    fn from_buf(buf: &BytesMut) -> Result<Option<Self>, ProtocolError> {
        let flags = buf[1];
        let sequence_id = BigEndian::read_u32(&buf[2..6]);
        let code = BigEndian::read_u16(&buf[6..8]);
        let payload = buf[12..].to_vec();

        Ok(Some(Self {
            flags,
            sequence_id,
            code,
            payload,
        }))
    }
}

// TODO: macro?

impl From<Hello> for LoquiFrame {
    fn from(hello: Hello) -> LoquiFrame {
        LoquiFrame::Hello(hello)
    }
}

impl From<HelloAck> for LoquiFrame {
    fn from(hello_ack: HelloAck) -> LoquiFrame {
        LoquiFrame::HelloAck(hello_ack)
    }
}

impl From<Ping> for LoquiFrame {
    fn from(ping: Ping) -> LoquiFrame {
        LoquiFrame::Ping(ping)
    }
}

impl From<Pong> for LoquiFrame {
    fn from(pong: Pong) -> LoquiFrame {
        LoquiFrame::Pong(pong)
    }
}

impl From<Request> for LoquiFrame {
    fn from(request: Request) -> LoquiFrame {
        LoquiFrame::Request(request)
    }
}

impl From<Response> for LoquiFrame {
    fn from(response: Response) -> LoquiFrame {
        LoquiFrame::Response(response)
    }
}

impl From<Push> for LoquiFrame {
    fn from(push: Push) -> LoquiFrame {
        LoquiFrame::Push(push)
    }
}

impl From<GoAway> for LoquiFrame {
    fn from(go_away: GoAway) -> LoquiFrame {
        LoquiFrame::GoAway(go_away)
    }
}

impl From<Error> for LoquiFrame {
    fn from(error: Error) -> LoquiFrame {
        LoquiFrame::Error(error)
    }
}
