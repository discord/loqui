use bytes::BytesMut;
use tokio_codec::{Decoder, Encoder};

use crate::errors::ProtocolError;
use crate::frames;

#[derive(Debug, PartialEq)]
pub enum LoquiFrame {
    Hello(frames::Hello),
    HelloAck(frames::HelloAck),
    Ping(frames::Ping),
    Pong(frames::Pong),
    Request(frames::Request),
    Response(frames::Response),
    Push(frames::Push),
    GoAway(frames::GoAway),
    Error(frames::Error),
}

#[derive(Debug)]
pub struct LoquiCodec {
    pub max_payload_bytes: u32,
}

impl LoquiCodec {
    pub fn new(max_payload_bytes: u32) -> LoquiCodec {
        LoquiCodec {
            max_payload_bytes: max_payload_bytes,
        }
    }
}

impl Encoder for LoquiCodec {
    type Item = LoquiFrame;
    type Error = ::failure::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            LoquiFrame::Hello(mut frame) => frame.encode(dst),
            LoquiFrame::HelloAck(mut frame) => frame.encode(dst),
            LoquiFrame::Ping(mut frame) => frame.encode(dst),
            LoquiFrame::Pong(mut frame) => frame.encode(dst),
            LoquiFrame::Request(mut frame) => frame.encode(dst),
            LoquiFrame::Response(mut frame) => frame.encode(dst),
            LoquiFrame::Push(mut frame) => frame.encode(dst),
            LoquiFrame::GoAway(mut frame) => frame.encode(dst),
            LoquiFrame::Error(mut frame) => frame.encode(dst),
        }
        .map_err(|err| err.into())
    }
}

impl Decoder for LoquiCodec {
    type Item = LoquiFrame;
    type Error = ::failure::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if buf.is_empty() {
            return Ok(None);
        }

        let op_code = buf[0];
        Ok(match op_code {
            frames::Hello::OP_CODE => {
                frames::Hello::decode(self, buf)?.map(|frame| LoquiFrame::Hello(frame))
            }
            frames::HelloAck::OP_CODE => {
                frames::HelloAck::decode(self, buf)?.map(|frame| LoquiFrame::HelloAck(frame))
            }
            frames::Ping::OP_CODE => {
                frames::Ping::decode(self, buf)?.map(|frame| LoquiFrame::Ping(frame))
            }
            frames::Pong::OP_CODE => {
                frames::Pong::decode(self, buf)?.map(|frame| LoquiFrame::Pong(frame))
            }
            frames::Request::OP_CODE => {
                frames::Request::decode(self, buf)?.map(|frame| LoquiFrame::Request(frame))
            }
            frames::Response::OP_CODE => {
                frames::Response::decode(self, buf)?.map(|frame| LoquiFrame::Response(frame))
            }
            frames::Push::OP_CODE => {
                frames::Push::decode(self, buf)?.map(|frame| LoquiFrame::Push(frame))
            }
            frames::GoAway::OP_CODE => {
                frames::GoAway::decode(self, buf)?.map(|frame| LoquiFrame::GoAway(frame))
            }
            frames::Error::OP_CODE => {
                frames::Error::decode(self, buf)?.map(|frame| LoquiFrame::Error(frame))
            }
            // TODO: we need to drop the bad stuff from this byte buffer, otherwise it loops forever
            _ => {
                dbg!(buf);
                return Err(ProtocolError::InvalidOpcode(op_code).into());
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BufMut, BytesMut};

    fn test_frame_roundtrip(frame_bytes: &[u8], expected_frame: LoquiFrame) {
        let mut codec = LoquiCodec::new(500);
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
        test_frame_roundtrip(
            &b"\x01\x0f\x01\x00\x00\x00\x16msgpack,json|gzip,lzma"[..],
            LoquiFrame::Hello(frames::Hello {
                flags: 15,
                version: 1,
                encodings: vec!["msgpack".into(), "json".into()],
                compressions: vec!["gzip".into(), "lzma".into()],
            }),
        );
    }

    #[test]
    fn test_helloack() {
        test_frame_roundtrip(
            &b"\x02\x0f\x00\x00}\x00\x00\x00\x00\x0cmsgpack|gzip"[..],
            LoquiFrame::HelloAck(frames::HelloAck {
                flags: 15,
                ping_interval_ms: 32000,
                encoding: "msgpack".into(),
                compression: "gzip".into(),
            }),
        );
    }

    #[test]
    fn test_ping() {
        test_frame_roundtrip(
            &b"\x03\x0f\x00\x00\x00\x01"[..],
            LoquiFrame::Ping(frames::Ping {
                flags: 15,
                sequence_id: 1,
            }),
        );
    }

    #[test]
    fn test_pong() {
        test_frame_roundtrip(
            &b"\x04\x0f\x00\x00\x00\x01"[..],
            LoquiFrame::Pong(frames::Pong {
                flags: 15,
                sequence_id: 1,
            }),
        );
    }

    #[test]
    fn test_request() {
        test_frame_roundtrip(
            &b"\x05\x1f\x00\x00\x00\x01\x00\x00\x00\x15hello this is my data"[..],
            LoquiFrame::Request(frames::Request {
                flags: 31,
                sequence_id: 1,
                payload: b"hello this is my data".to_vec(),
            }),
        );
    }

    #[test]
    fn test_response() {
        test_frame_roundtrip(
            &b"\x06\x1f\x00\x00\x0b\xb8\x00\x00\x00\x15hello this is my data"[..],
            LoquiFrame::Response(frames::Response {
                flags: 31,
                sequence_id: 3000,
                payload: b"hello this is my data".to_vec(),
            }),
        );
    }

    #[test]
    fn test_push() {
        test_frame_roundtrip(
            &b"\x07[\x00\x00\x00\x15hello this is my push"[..],
            LoquiFrame::Push(frames::Push {
                flags: 91,
                payload: b"hello this is my push".to_vec(),
            }),
        );
    }

    #[test]
    fn test_goaway() {
        test_frame_roundtrip(
            &b"\x08\x97#)\x00\x00\x00\x0bgo away pls"[..],
            LoquiFrame::GoAway(frames::GoAway {
                flags: 151,
                code: 9001,
                payload: b"go away pls".to_vec(),
            }),
        );
    }

    #[test]
    fn test_error() {
        test_frame_roundtrip(
            &b"\t\x97\x00\r\xbc\x04\x05\xa4\x00\x00\x00\x08errrror!"[..],
            LoquiFrame::Error(frames::Error {
                flags: 151,
                code: 1444,
                sequence_id: 900100,
                payload: b"errrror!".to_vec(),
            }),
        );
    }
}
