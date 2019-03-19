use byteorder::{BigEndian, ByteOrder};
use bytes::{BufMut, BytesMut};

use crate::codec::LoquiCodec;
use crate::errors::ProtocolError;

type DecodeResult<T> = Result<Option<T>, ProtocolError>;
type EncodeResult = Result<(), ProtocolError>;

#[derive(Debug, PartialEq, Clone)]
pub struct Hello {
    pub flags: u8,
    pub version: u8,
    pub encodings: Vec<String>,
    pub compressions: Vec<String>,
}

impl Hello {
    pub const OP_CODE: u8 = 1;
    pub const MIN_BYTES: u8 = 7;

    pub fn encode(&mut self, dst: &mut BytesMut) -> EncodeResult {
        maybe_extend(dst, 8 + 8 + 8);
        dst.put_u8(Self::OP_CODE);
        dst.put_u8(self.flags);
        dst.put_u8(self.version);

        let payload = format!(
            "{}|{}",
            self.encodings.join(","),
            self.compressions.join(","),
        );
        let payload = payload.bytes().collect::<Vec<u8>>();

        maybe_extend(dst, 32 + payload.len() * 8);
        dst.put_u32_be(payload.len() as u32);
        dst.put(payload);
        Ok(())
    }

    pub fn decode(codec: &mut LoquiCodec, buf: &mut BytesMut) -> DecodeResult<Self> {
        if buf.len() < Self::MIN_BYTES as usize {
            // Not enough data for a frame. Wait for more.
            return Ok(None);
        }

        let flags = buf[1];
        let version = buf[2];
        let payload_bytes = BigEndian::read_u32(&buf[3..7]);

        if payload_bytes > codec.max_payload_bytes {
            return Err(ProtocolError::PayloadTooLarge(
                payload_bytes,
                codec.max_payload_bytes,
            ));
        }

        let required_len = Self::MIN_BYTES as usize + payload_bytes as usize;
        if buf.len() < required_len {
            // Don't have complete payload. Wait for more.
            return Ok(None);
        }

        // We have enough information to determine whether this is a valid
        // frame or not so split out the frame.
        let frame = buf.split_to(required_len);
        let payload = ::std::str::from_utf8(&frame[7..required_len])
            .map_err(|_| ProtocolError::InvalidPayload("Failed to decode as string".into()))?;

        let settings: Vec<&str> = payload.split("|").collect();
        if settings.len() != 2 {
            return Err(ProtocolError::InvalidPayload(
                "Expected exactly two settings.".into(),
            ));
        }

        let encodings = settings[0]
            .split_terminator(",")
            .map(|s| String::from(s))
            .collect::<Vec<String>>();

        let compressions = settings[1]
            .split_terminator(",")
            .map(|s| String::from(s))
            .collect::<Vec<String>>();

        Ok(Some(Self {
            flags: flags,
            version: version,
            encodings: encodings,
            compressions: compressions,
        }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct HelloAck {
    pub flags: u8,
    pub ping_interval_ms: u32,
    pub encoding: String,
    pub compression: String,
}

impl HelloAck {
    pub const OP_CODE: u8 = 2;
    pub const MIN_BYTES: u8 = 10;

    pub fn encode(&mut self, dst: &mut BytesMut) -> EncodeResult {
        maybe_extend(dst, 8 + 8 + 8 + 32);
        dst.put_u8(Self::OP_CODE);
        dst.put_u8(self.flags);
        dst.put_u32_be(self.ping_interval_ms);

        let payload = format!("{}|{}", self.encoding, self.compression);
        maybe_extend(dst, 32 + payload.len() * 8);
        dst.put_u32_be(payload.len() as u32);
        dst.put(payload);
        Ok(())
    }

    pub fn decode(codec: &mut LoquiCodec, buf: &mut BytesMut) -> DecodeResult<Self> {
        if buf.len() < Self::MIN_BYTES as usize {
            // Not enough data for a frame. Wait for more.
            return Ok(None);
        }

        let flags = buf[1];
        let ping_interval_ms = BigEndian::read_u32(&buf[2..6]);
        let payload_bytes = BigEndian::read_u32(&buf[6..10]);

        if payload_bytes > codec.max_payload_bytes {
            return Err(ProtocolError::PayloadTooLarge(
                payload_bytes,
                codec.max_payload_bytes,
            ));
        }

        let required_len = Self::MIN_BYTES as usize + payload_bytes as usize;
        if buf.len() < required_len {
            // Don't have complete payload. Wait for more.
            return Ok(None);
        }

        // We have enough information to determine whether this is a valid
        // frame or not so split out the frame.
        let frame = buf.split_to(required_len);
        let payload = ::std::str::from_utf8(&frame[10..required_len])
            .map_err(|_| ProtocolError::InvalidPayload("Failed to decode as string".into()))?;

        let settings: Vec<&str> = payload.split("|").collect();
        if settings.len() != 2 {
            return Err(ProtocolError::InvalidPayload(
                "Expected exactly two settings.".into(),
            ));
        }

        Ok(Some(Self {
            flags: flags,
            ping_interval_ms: ping_interval_ms,
            encoding: settings[0].into(),
            compression: settings[1].into(),
        }))
    }

    fn negotiate_encoding(
        server_encodings: &[String],
        client_encodings: &[String],
    ) -> Option<String> {
        for server_encoding in server_encodings {
            for client_encoding in client_encodings {
                if server_encoding == client_encoding {
                    return Some(server_encoding.clone());
                }
            }
        }
        None
    }

    fn negotiate_compression(
        server_compressions: &[String],
        client_compressions: &[String],
    ) -> Option<String> {
        for server_compression in server_compressions {
            for client_compression in client_compressions {
                if server_compression == client_compression {
                    return Some(server_compression.clone());
                }
            }
        }
        None
    }

    // Negotiates a HelloAck frame from a &Hello.
    pub fn from_hello(
        hello: &Hello,
        ping_interval_ms: u32,
        encodings: &[String],
        compressions: &[String],
    ) -> Option<Self> {
        let encoding = HelloAck::negotiate_encoding(encodings, hello.encodings.as_ref())?;

        let compression =
            HelloAck::negotiate_compression(compressions, hello.compressions.as_ref())
                .unwrap_or(String::from(""));

        Some(HelloAck {
            flags: 0,
            ping_interval_ms: ping_interval_ms,
            encoding: encoding,
            compression: compression,
        })
    }
}

fn maybe_extend(dst: &mut BytesMut, message_size: usize) {
    let remaining = dst.remaining_mut();
    if remaining >= message_size {
        return;
    }

    let missing = message_size - remaining;
    if missing > 0 {
        dst.reserve(missing);
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Ping {
    pub flags: u8,
    pub sequence_id: u32,
}

impl Ping {
    pub const OP_CODE: u8 = 3;
    pub const MIN_BYTES: u8 = 6;

    pub fn encode(&mut self, dst: &mut BytesMut) -> EncodeResult {
        maybe_extend(dst, 8 + 8 + 32);
        dst.put_u8(Self::OP_CODE);
        dst.put_u8(self.flags);
        dst.put_u32_be(self.sequence_id);
        Ok(())
    }

    pub fn decode(_codec: &mut LoquiCodec, buf: &mut BytesMut) -> DecodeResult<Self> {
        if buf.len() < Self::MIN_BYTES as usize {
            // Not enough data for a frame. Wait for more.
            return Ok(None);
        }

        let flags = buf[1];
        let sequence_id = BigEndian::read_u32(&buf[2..6]);

        // We have enough information to determine whether this is a valid
        // frame or not so advance the buffer.
        buf.advance(Self::MIN_BYTES as usize);

        Ok(Some(Self {
            flags: flags,
            sequence_id: sequence_id,
        }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Pong {
    pub flags: u8,
    pub sequence_id: u32,
}

impl Pong {
    pub const OP_CODE: u8 = 4;
    pub const MIN_BYTES: u8 = 6;

    pub fn encode(&mut self, dst: &mut BytesMut) -> EncodeResult {
        maybe_extend(dst, 8 + 8 + 32);
        dst.put_u8(Self::OP_CODE);
        dst.put_u8(self.flags);
        dst.put_u32_be(self.sequence_id);
        Ok(())
    }

    pub fn decode(_codec: &mut LoquiCodec, buf: &mut BytesMut) -> DecodeResult<Self> {
        if buf.len() < Self::MIN_BYTES as usize {
            // Not enough data for a frame. Wait for more.
            return Ok(None);
        }

        let flags = buf[1];
        let sequence_id = BigEndian::read_u32(&buf[2..6]);

        // We have enough information to determine whether this is a valid
        // frame or not so advance the buffer.
        buf.advance(Self::MIN_BYTES as usize);

        Ok(Some(Self {
            flags: flags,
            sequence_id: sequence_id,
        }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Request {
    pub flags: u8,
    pub sequence_id: u32,
    pub payload: Vec<u8>,
}

impl Request {
    pub const OP_CODE: u8 = 5;
    pub const MIN_BYTES: u8 = 10;

    pub fn encode(&mut self, dst: &mut BytesMut) -> EncodeResult {
        maybe_extend(dst, 8 + 8 + 32 + 32 + self.payload.len() * 8);
        dst.put_u8(Self::OP_CODE);
        dst.put_u8(self.flags);
        dst.put_u32_be(self.sequence_id);
        dst.put_u32_be(self.payload.len() as u32);
        dst.put(self.payload.clone());
        Ok(())
    }

    pub fn decode(codec: &mut LoquiCodec, buf: &mut BytesMut) -> DecodeResult<Self> {
        if buf.len() < Self::MIN_BYTES as usize {
            // Not enough data for a frame. Wait for more.
            return Ok(None);
        }

        let flags = buf[1];
        let sequence_id = BigEndian::read_u32(&buf[2..6]);
        let payload_bytes = BigEndian::read_u32(&buf[6..10]);

        if payload_bytes > codec.max_payload_bytes {
            return Err(ProtocolError::PayloadTooLarge(
                payload_bytes,
                codec.max_payload_bytes,
            ));
        }

        let required_len = Self::MIN_BYTES as usize + payload_bytes as usize;
        if buf.len() < required_len {
            // Don't have complete payload. Wait for more.
            return Ok(None);
        }

        // We have enough information to determine whether this is a valid
        // frame or not so split out the frame.
        let frame = buf.split_to(required_len);
        let payload = frame[10..required_len].to_vec();

        Ok(Some(Self {
            flags: flags,
            sequence_id: sequence_id,
            payload: payload,
        }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Response {
    pub flags: u8,
    pub sequence_id: u32,
    pub payload: Vec<u8>,
}

impl Response {
    pub const OP_CODE: u8 = 6;
    pub const MIN_BYTES: u8 = 10;

    pub fn encode(&mut self, dst: &mut BytesMut) -> EncodeResult {
        maybe_extend(dst, 8 + 8 + 32 + 32 + self.payload.len() * 8);
        dst.put_u8(Self::OP_CODE);
        dst.put_u8(self.flags);
        dst.put_u32_be(self.sequence_id);
        dst.put_u32_be(self.payload.len() as u32);
        dst.extend_from_slice(&self.payload[..]);
        Ok(())
    }

    pub fn decode(codec: &mut LoquiCodec, buf: &mut BytesMut) -> DecodeResult<Self> {
        if buf.len() < Self::MIN_BYTES as usize {
            // Not enough data for a frame. Wait for more.
            return Ok(None);
        }

        let flags = buf[1];
        let sequence_id = BigEndian::read_u32(&buf[2..6]);
        let payload_bytes = BigEndian::read_u32(&buf[6..10]);

        if payload_bytes > codec.max_payload_bytes {
            return Err(ProtocolError::PayloadTooLarge(
                payload_bytes,
                codec.max_payload_bytes,
            ));
        }

        let required_len = Self::MIN_BYTES as usize + payload_bytes as usize;
        if buf.len() < required_len {
            // Don't have complete payload. Wait for more.
            return Ok(None);
        }

        // We have enough information to determine whether this is a valid
        // frame or not so split out the frame.
        let frame = buf.split_to(required_len);
        let payload = frame[10..required_len].to_vec();

        Ok(Some(Self {
            flags: flags,
            sequence_id: sequence_id,
            payload: payload,
        }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Push {
    pub flags: u8,
    pub payload: Vec<u8>,
}

impl Push {
    pub const OP_CODE: u8 = 7;
    pub const MIN_BYTES: u8 = 6;

    pub fn encode(&mut self, dst: &mut BytesMut) -> EncodeResult {
        maybe_extend(dst, 8 + 8 + 32 + self.payload.len() * 8);
        dst.put_u8(Self::OP_CODE);
        dst.put_u8(self.flags);
        dst.put_u32_be(self.payload.len() as u32);
        dst.extend_from_slice(&self.payload[..]);
        Ok(())
    }

    pub fn decode(codec: &mut LoquiCodec, buf: &mut BytesMut) -> DecodeResult<Self> {
        if buf.len() < Self::MIN_BYTES as usize {
            // Not enough data for a frame. Wait for more.
            return Ok(None);
        }

        let flags = buf[1];
        let payload_bytes = BigEndian::read_u32(&buf[2..6]);

        if payload_bytes > codec.max_payload_bytes {
            return Err(ProtocolError::PayloadTooLarge(
                payload_bytes,
                codec.max_payload_bytes,
            ));
        }

        let required_len = Self::MIN_BYTES as usize + payload_bytes as usize;
        if buf.len() < required_len {
            // Don't have complete payload. Wait for more.
            return Ok(None);
        }

        // We have enough information to determine whether this is a valid
        // frame or not so split out the frame.
        let frame = buf.split_to(required_len);
        let payload = frame[6..required_len].to_vec();

        Ok(Some(Self {
            flags: flags,
            payload: payload,
        }))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct GoAway {
    pub flags: u8,
    pub code: u16,
    pub payload: Vec<u8>,
}

impl GoAway {
    pub const OP_CODE: u8 = 8;
    pub const MIN_BYTES: u8 = 8;

    pub fn encode(&mut self, dst: &mut BytesMut) -> EncodeResult {
        maybe_extend(dst, 8 + 8 + 16 + 32 + self.payload.len() * 8);
        dst.put_u8(Self::OP_CODE);
        dst.put_u8(self.flags);
        dst.put_u16_be(self.code);
        dst.put_u32_be(self.payload.len() as u32);
        dst.put(self.payload.clone());
        Ok(())
    }

    pub fn decode(codec: &mut LoquiCodec, buf: &mut BytesMut) -> DecodeResult<Self> {
        if buf.len() < Self::MIN_BYTES as usize {
            // Not enough data for a frame. Wait for more.
            return Ok(None);
        }

        let flags = buf[1];
        let code = BigEndian::read_u16(&buf[2..4]);
        let payload_bytes = BigEndian::read_u32(&buf[4..8]);

        if payload_bytes > codec.max_payload_bytes {
            return Err(ProtocolError::PayloadTooLarge(
                payload_bytes,
                codec.max_payload_bytes,
            ));
        }

        let required_len = Self::MIN_BYTES as usize + payload_bytes as usize;
        if buf.len() < required_len {
            // Don't have complete payload. Wait for more.
            return Ok(None);
        }

        // We have enough information to determine whether this is a valid
        // frame or not so split out the frame.
        let frame = buf.split_to(required_len);
        let payload = frame[8..required_len].to_vec();

        Ok(Some(Self {
            flags: flags,
            code: code,
            payload: payload,
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

impl Error {
    pub const OP_CODE: u8 = 9;
    pub const MIN_BYTES: u8 = 12;

    pub fn encode(&mut self, dst: &mut BytesMut) -> EncodeResult {
        maybe_extend(dst, 8 + 8 + 32 + 16 + 32 + self.payload.len() * 8);
        dst.put_u8(Self::OP_CODE);
        dst.put_u8(self.flags);
        dst.put_u32_be(self.sequence_id);
        dst.put_u16_be(self.code);
        dst.put_u32_be(self.payload.len() as u32);
        dst.put(self.payload.clone());
        Ok(())
    }

    pub fn decode(codec: &mut LoquiCodec, buf: &mut BytesMut) -> DecodeResult<Self> {
        if buf.len() < Self::MIN_BYTES as usize {
            // Not enough data for a frame. Wait for more.
            return Ok(None);
        }

        let flags = buf[1];
        let sequence_id = BigEndian::read_u32(&buf[2..6]);
        let code = BigEndian::read_u16(&buf[6..8]);
        let payload_bytes = BigEndian::read_u32(&buf[8..12]);

        if payload_bytes > codec.max_payload_bytes {
            return Err(ProtocolError::PayloadTooLarge(
                payload_bytes,
                codec.max_payload_bytes,
            ));
        }

        let required_len = Self::MIN_BYTES as usize + payload_bytes as usize;
        if buf.len() < required_len {
            // Don't have complete payload. Wait for more.
            return Ok(None);
        }

        // We have enough information to determine whether this is a valid
        // frame or not so split out the frame.
        let frame = buf.split_to(required_len);
        let payload = frame[12..required_len].to_vec();

        Ok(Some(Self {
            flags: flags,
            sequence_id: sequence_id,
            code: code,
            payload: payload,
        }))
    }
}
