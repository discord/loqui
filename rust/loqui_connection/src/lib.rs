#![feature(await_macro, async_await, futures_api)]

#[macro_use]
extern crate log;

mod async_backoff;
mod connection;
mod encoder;
mod error;
mod event_handler;
mod framed_io;
mod id_sequence;
mod select_break;
mod sender;
mod supervisor;

pub mod handler;

pub use connection::Connection;
pub use encoder::{ArcEncoder, Encoder, Factory as EncoderFactory};
pub use error::{convert_timeout_error, LoquiError, LoquiErrorCode};
pub use framed_io::ReaderWriter;
pub use id_sequence::IdSequence;
pub use supervisor::Supervisor;
