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

use std::future::Future;
use std::time::Instant;

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

use failure::Error;
use std::io::{Error as IoError, ErrorKind};
use tokio::time::{timeout_at as tokio_timeout_at, Instant as TokioInstant};

pub async fn timeout_at<F, O, E>(deadline: Instant, future: F) -> F::Output
where
    F: Future<Output = Result<O, E>>,
    E: From<IoError>,
{
    timeout_at_tk(TokioInstant::from_std(deadline), future).await
}

pub async fn timeout_at_tk<F, O, E>(deadline: TokioInstant, future: F) -> F::Output
where
    F: Future<Output = Result<O, E>>,
    E: From<IoError>,
{
    tokio_timeout_at(deadline, future)
        .await
        .unwrap_or_else(|_e| Err(IoError::new(ErrorKind::TimedOut, "timeout").into()))
}
