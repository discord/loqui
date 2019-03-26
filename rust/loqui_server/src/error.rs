use failure::Fail;
use loqui_protocol::codec::LoquiFrame;

#[derive(Debug, Fail)]
pub enum LoquiError {
    #[fail(display = "Connection not ready.")]
    NotReady,
    #[fail(display = "Invalid frame. frame={:?}", frame)]
    InvalidFrame { frame: LoquiFrame },
    #[fail(display = "Ping Timeout")]
    PingTimeout,
    // TODO: reason
    #[fail(display = "Go Away")]
    GoAway,
}

pub enum LoquiErrorCode {
    // Normal is sent when the connection is closing cleanly.
    Normal,
    // InvalidOp is sent when the connection receives an opcode it cannot handle.
    InvalidOp,
    // UnsupportedVersion is sent when conn does not support a version.
    UnsupportedVersion,
    // NoCommonEncoding is sent when there are no common encodings.
    NoCommonEncoding,
    // InvalidEncoding is sent by the client if the server chooses and invalid encoding.
    InvalidEncoding,
    // InvalidCompression is sent by the client if the server chooses and invalid compression.
    InvalidCompression,
    // PingTimeout is sent when connection does not receive a pong within ping interval.
    PingTimeout,
    // InternalServerError is sent when a single request dies due to an error.
    InternalServerError,
}
