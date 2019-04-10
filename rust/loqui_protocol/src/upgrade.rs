use crate::error::ProtocolError;
use bytes::BytesMut;
use bytesize::ByteSize;
use failure::Error;
use tokio_codec::{Decoder, Encoder};

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

#[derive(Debug)]
pub enum UpgradeFrame {
    Request,
    Response,
}

const REQUEST: &str =
    "GET /_rpc HTTP/1.1\r\nHost: 127.0.0.1 \r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n";
const RESPONSE: &str =
    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: loqui\r\nConnection: Upgrade\r\n\r\n";

impl Encoder for Codec {
    type Item = UpgradeFrame;
    type Error = ::failure::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            UpgradeFrame::Request => dst.extend_from_slice(REQUEST.as_bytes()),
            UpgradeFrame::Response => dst.extend_from_slice(RESPONSE.as_bytes()),
        };
        Ok(())
    }
}

impl Decoder for Codec {
    type Item = UpgradeFrame;
    type Error = Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if buf.is_empty() {
            println!("empty");
            return Ok(None);
        }

        let payload_size = buf.len() as u32;
        if payload_size > self.max_payload_size_in_bytes {
            println!("payload large");
            return Err(ProtocolError::PayloadTooLarge {
                actual: payload_size,
                max: self.max_payload_size_in_bytes,
            }
            .into());
        }

        println!("data {:?}", String::from_utf8(buf[..].to_vec()));
        match String::from_utf8(buf[..].to_vec()) {
            Ok(payload) => {
                if !payload.ends_with("\r\n\r\n") {
                    return Ok(None);
                }

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
