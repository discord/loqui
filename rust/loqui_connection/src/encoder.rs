use failure::Error;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;
use std::pin::Pin;

/// Interface for encoding and decoding. Used by the connection to hand back proper `Decoded`
/// and `Encoded` structs from a vector of bytes.
pub trait Encoder: Send + Sync {
    /// The resulting type when a `Vec<u8>` is decoded.
    type Decoded: DeserializeOwned + Send + Sync + Debug;
    /// The type that is encoded into a `Vec<u8>`.
    type Encoded: Serialize + Send + Sync + Debug;

    /// Decode a `Vec<u8>` into a struct.
    fn decode(&self, payload: Vec<u8>) -> Result<Self::Decoded, Error>;
    /// Encode a struct into a `Vec<u8>`. Returns `(Vec<u8>, bool)` where `bool` is true if
    /// the payload is compressed.
    fn encode(&self, payload: Self::Encoded) -> Result<(Vec<u8>, bool), Error>;
}

pub trait Factory: Send + Sync + 'static {
    /// The resulting type when a `Vec<u8>` is decoded.
    type Decoded: DeserializeOwned + Send + Sync + Debug;
    /// The type that is encoded into a `Vec<u8>`.
    type Encoded: Serialize + Send + Sync + Debug;

    /// Encodings supported.
    const ENCODINGS: &'static [&'static str];
    /// Compressions supported.
    const COMPRESSIONS: &'static [&'static str];

    fn make() -> Box<Encoder<Encoded = Self::Encoded, Decoded = Self::Decoded>>;

    fn find_encoding<S: AsRef<str>>(encoding: S) -> Option<&'static str> {
        let encoding = encoding.as_ref();
        for supported_encoding in Self::ENCODINGS {
            if encoding == *supported_encoding {
                return Some(supported_encoding);
            }
        }
        None
    }

    fn find_compression<S: AsRef<str>>(compression: S) -> Option<&'static str> {
        let compression = compression.as_ref();
        for supported_compression in Self::COMPRESSIONS {
            if compression == *supported_compression {
                return Some(supported_compression);
            }
        }
        None
    }
}
