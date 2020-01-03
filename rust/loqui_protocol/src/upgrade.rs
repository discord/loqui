use crate::error::ProtocolError;
use bytes::BytesMut;
use bytesize::ByteSize;
use failure::Error;
use tokio_util::codec::{Decoder, Encoder};

/// Codec for reading and writing the HTTP upgrade request.
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

#[derive(Debug, PartialEq, Clone)]
pub enum UpgradeFrame {
    Request,
    Response,
}

const REQUEST: &[u8; 77] =
    b"GET /_rpc HTTP/1.1\r\nHost: 127.0.0.1 \r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";
const RESPONSE: &[u8; 73] =
    b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: loqui\r\nConnection: Upgrade\r\n\r\n";

impl Encoder for Codec {
    type Item = UpgradeFrame;
    type Error = ::failure::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            UpgradeFrame::Request => dst.extend_from_slice(REQUEST),
            UpgradeFrame::Response => dst.extend_from_slice(RESPONSE),
        };
        Ok(())
    }
}

impl Decoder for Codec {
    type Item = UpgradeFrame;
    type Error = Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if buf.is_empty() {
            return Ok(None);
        }

        let payload_size = buf.len() as u32;
        if payload_size > self.max_payload_size_in_bytes {
            return Err(ProtocolError::PayloadTooLarge {
                actual: payload_size,
                max: self.max_payload_size_in_bytes,
            }
            .into());
        }

        match String::from_utf8(buf[..].to_vec()) {
            Ok(payload) => {
                if !payload.ends_with("\r\n\r\n") {
                    return Ok(None);
                }

                // Clear buffer.
                buf.clear();

                let payload = payload.to_lowercase();
                if payload.contains("upgrade") {
                    if payload.starts_with("get") {
                        Ok(Some(UpgradeFrame::Request))
                    } else {
                        Ok(Some(UpgradeFrame::Response))
                    }
                } else {
                    Err(ProtocolError::InvalidPayload { reason: payload }.into())
                }
            }

            Err(_e) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BufMut, BytesMut};

    fn test_round_trip(frame_bytes: &[u8], expected_frame: UpgradeFrame) {
        let expected_frame = expected_frame.into();
        let mut codec = Codec::new(ByteSize::b(500));
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
    fn it_round_trips_request() {
        test_round_trip(REQUEST, UpgradeFrame::Request)
    }

    #[test]
    fn it_round_trips_response() {
        test_round_trip(RESPONSE, UpgradeFrame::Response)
    }
}
