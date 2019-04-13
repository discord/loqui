use failure::Error;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;
use std::sync::Arc;

/// Interface for encoding and decoding. Used by the connection to hand back proper `Decoded`
/// and `Encoded` structs from a vector of bytes.
pub trait Encoder: Send + Sync {
    /// The resulting type when a `Vec<u8>` is decoded.
    type Decoded: DeserializeOwned + Send + Debug;
    /// The type that is encoded into a `Vec<u8>`.
    type Encoded: Serialize + Send + Debug;

    /// Decode a `Vec<u8>` into a struct.
    fn decode(&self, payload: Vec<u8>) -> Result<Self::Decoded, Error>;
    /// Encode a struct into a `Vec<u8>`.
    fn encode(&self, payload: Self::Encoded) -> Result<Vec<u8>, Error>;
}

/// Trait for creating encoders based on the encoding that was selected for the connection.
pub trait Factory: Send + Sync + 'static {
    /// The resulting type when a `Vec<u8>` is decoded.
    type Decoded: DeserializeOwned + Send + Debug;
    /// The type that is encoded into a `Vec<u8>`.
    type Encoded: Serialize + Send + Debug;

    /// Encodings supported.
    const ENCODINGS: &'static [&'static str];

    /// Create an encoder if the encoding passed in is supported.
    fn make(
        encoding: &'static str,
    ) -> Option<Arc<Box<Encoder<Encoded = Self::Encoded, Decoded = Self::Decoded>>>>;

    /// Determines if an encoding is supported.
    fn find_encoding<S: AsRef<str>>(encoding: S) -> Option<&'static str> {
        let encoding = encoding.as_ref();
        for supported_encoding in Self::ENCODINGS {
            if encoding == *supported_encoding {
                return Some(supported_encoding);
            }
        }
        None
    }
}

/// Type that is passed into other functions once the `Encoder` is created. This allows us to store
/// `Factory` as a `type` param on traits instead of having to make them parameters elsewhere.
pub type ArcEncoder<F> = Arc<
    Box<
        dyn Encoder<Encoded = <F as Factory>::Encoded, Decoded = <F as Factory>::Decoded> + 'static,
    >,
>;
