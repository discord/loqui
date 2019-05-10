#![feature(await_macro, async_await, futures_api)]

#[macro_use]
extern crate log;

mod connection;
mod error;
mod event_handler;
mod framed_io;
mod id_sequence;
mod select_break;
mod sender;

pub mod handler;

pub use connection::Connection;
pub use error::{convert_timeout_error, LoquiError, LoquiErrorCode};
pub use framed_io::ReaderWriter;
pub use id_sequence::IdSequence;

pub fn find_encoding<S: AsRef<str>>(
    encoding: S,
    supported_encodings: &'static [&'static str],
) -> Option<&'static str> {
    let encoding = encoding.as_ref();
    for supported_encoding in supported_encodings {
        if encoding == *supported_encoding {
            return Some(supported_encoding);
        }
    }
    None
}
