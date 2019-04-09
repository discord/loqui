use bytes::{BufMut, BytesMut};
use bytesize::ByteSize;
use failure::Error;
use tokio_codec::{Decoder, Encoder};

use crate::error::ProtocolError;
use crate::frames::{
    Error as ErrorFrame, Frame, GoAway, Hello, HelloAck, LoquiFrame, Ping, Pong, Push, Request,
    Response,
};

#[derive(Debug)]
pub struct Codec {
    pub max_payload_size_in_bytes: u32,
}

impl Codec {
    pub fn new(max_payload_size: ByteSize) -> Self {
        Self {
            max_payload_size_in_bytes: max_payload_size.as_u64() as u32,
        }
    }
}

impl Encoder for Codec {
    type Item = LoquiFrame;
    type Error = ::failure::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            LoquiFrame::Hello(frame) => encode(frame, dst),
            LoquiFrame::HelloAck(frame) => encode(frame, dst),
            LoquiFrame::Ping(frame) => encode(frame, dst),
            LoquiFrame::Pong(frame) => encode(frame, dst),
            LoquiFrame::Request(frame) => encode(frame, dst),
            LoquiFrame::Response(frame) => encode(frame, dst),
            LoquiFrame::Push(frame) => encode(frame, dst),
            LoquiFrame::GoAway(frame) => encode(frame, dst),
            LoquiFrame::Error(frame) => encode(frame, dst),
        };
        Ok(())
    }
}

impl Decoder for Codec {
    type Item = LoquiFrame;
    type Error = Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if buf.is_empty() {
            return Ok(None);
        }
        let opcode = buf[0];
        match opcode {
            Hello::OPCODE => decode::<Hello>(self, buf),
            HelloAck::OPCODE => decode::<HelloAck>(self, buf),
            Ping::OPCODE => decode::<Ping>(self, buf),
            Pong::OPCODE => decode::<Pong>(self, buf),
            Request::OPCODE => decode::<Request>(self, buf),
            Response::OPCODE => decode::<Response>(self, buf),
            Push::OPCODE => decode::<Push>(self, buf),
            GoAway::OPCODE => decode::<GoAway>(self, buf),
            ErrorFrame::OPCODE => decode::<ErrorFrame>(self, buf),
            _ => Err(ProtocolError::InvalidOpcode(opcode).into()),
        }
    }
}

fn encode<F: Frame>(frame: F, dst: &mut BytesMut) {
    dst.reserve(F::HEADER_SIZE_IN_BYTES);
    frame.put_header(dst);
    if let Some(payload) = frame.payload() {
        dst.put_u32_be(payload.len() as u32);
        dst.extend_from_slice(&payload[..]);
    }
}

fn decode<F: Frame + Into<LoquiFrame>>(
    codec: &mut Codec,
    buf: &mut BytesMut,
) -> Result<Option<LoquiFrame>, Error> {
    if buf.len() < F::HEADER_SIZE_IN_BYTES {
        // Not enough data for a frame. Wait for more.
        return Ok(None);
    }

    let payload_size = F::read_payload_size(buf);
    if payload_size > codec.max_payload_size_in_bytes {
        return Err(
            ProtocolError::PayloadTooLarge(payload_size, codec.max_payload_size_in_bytes).into(),
        );
    }

    let required_len = F::HEADER_SIZE_IN_BYTES + payload_size as usize;
    if buf.len() < required_len {
        // Don't have complete payload. Wait for more.
        return Ok(None);
    }

    let frame_buf = buf.split_to(required_len);
    match F::from_buf(&frame_buf) {
        Ok(Some(frame)) => Ok(Some(frame.into())),
        Ok(None) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BufMut, BytesMut};

    fn test_frame_round_trip<F: Into<LoquiFrame>>(frame_bytes: &[u8], expected_frame: F) {
        let expected_frame = expected_frame.into();
        let mut codec = Codec::new(500);
        let buf = &mut BytesMut::with_capacity(1024);

        // Incomplete Payload
        buf.put(frame_bytes[..frame_bytes.len() - 1].to_vec());
        assert_eq!(codec.decode(buf).unwrap(), None);

        // Complete Frame
        buf.put(frame_bytes[frame_bytes.len() - 1..].to_vec());
        assert_eq!(codec.decode(buf).unwrap().unwrap(), expected_frame);

        // Buffer has been consumed
        assert!(buf.is_empty());
        assert_eq!(codec.decode(buf).unwrap(), None);

        // Encode a frame to make sure it can round-trip.
        let _ = codec.encode(expected_frame, buf);
        assert_eq!(buf, frame_bytes);
    }

    #[test]
    fn test_hello() {
        test_frame_round_trip(
            &b"\x01\x0f\x01\x00\x00\x00\x16msgpack,json|gzip,lzma"[..],
            Hello {
                flags: 15,
                version: 1,
                encodings: vec!["msgpack".into(), "json".into()],
                compressions: vec!["gzip".into(), "lzma".into()],
            },
        );
    }

    #[test]
    fn test_helloack() {
        test_frame_round_trip(
            &b"\x02\x0f\x00\x00}\x00\x00\x00\x00\x0cmsgpack|gzip"[..],
            HelloAck {
                flags: 15,
                ping_interval_ms: 32000,
                encoding: "msgpack".into(),
                compression: "gzip".into(),
            },
        );
    }

    #[test]
    fn test_ping() {
        test_frame_round_trip(
            &b"\x03\x0f\x00\x00\x00\x01"[..],
            Ping {
                flags: 15,
                sequence_id: 1,
            },
        );
    }

    #[test]
    fn test_pong() {
        test_frame_round_trip(
            &b"\x04\x0f\x00\x00\x00\x01"[..],
            Pong {
                flags: 15,
                sequence_id: 1,
            },
        );
    }

    #[test]
    fn test_request() {
        test_frame_round_trip(
            &b"\x05\x1f\x00\x00\x00\x01\x00\x00\x00\x15hello this is my data"[..],
            Request {
                flags: 31,
                sequence_id: 1,
                payload: b"hello this is my data".to_vec(),
            },
        );
    }

    #[test]
    fn test_response() {
        test_frame_round_trip(
            &b"\x06\x1f\x00\x00\x0b\xb8\x00\x00\x00\x15hello this is my data"[..],
            Response {
                flags: 31,
                sequence_id: 3000,
                payload: b"hello this is my data".to_vec(),
            },
        );
    }

    #[test]
    fn test_push() {
        test_frame_round_trip(
            &b"\x07[\x00\x00\x00\x15hello this is my push"[..],
            Push {
                flags: 91,
                payload: b"hello this is my push".to_vec(),
            },
        );
    }

    #[test]
    fn test_goaway() {
        test_frame_round_trip(
            &b"\x08\x97#)\x00\x00\x00\x0bgo away pls"[..],
            GoAway {
                flags: 151,
                code: 9001,
                payload: b"go away pls".to_vec(),
            },
        );
    }

    #[test]
    fn test_error() {
        test_frame_round_trip(
            &b"\t\x97\x00\r\xbc\x04\x05\xa4\x00\x00\x00\x08errrror!"[..],
            ErrorFrame {
                flags: 151,
                code: 1444,
                sequence_id: 900100,
                payload: b"errrror!".to_vec(),
            },
        );
    }
}
