use failure::{Error, Fail};
use loqui_protocol::frames::{GoAway, LoquiFrame};

#[derive(Debug, Fail)]
pub enum LoquiError {
    #[fail(display = "TCP Connection closed.")]
    TcpStreamClosed,
    #[fail(display = "TCP Connection closed by us.")]
    TcpStreamIntentionalClose,
    #[fail(display = "Connection closed.")]
    ConnectionClosed,
    #[fail(display = "Upgrade failed.")]
    UpgradeFailed,
    #[fail(display = "The connection supervisor died.")]
    ConnectionSupervisorDead,
    #[fail(display = "Connection not ready.")]
    NotReady,
    // TODO: combine
    #[fail(display = "Invalid frame. frame={:?}", frame)]
    InvalidFrame { frame: LoquiFrame },
    #[fail(display = "Go away. go_away={:?}", go_away)]
    GoAway { go_away: GoAway },
    #[fail(display = "Told to go away. go_away={:?}", go_away)]
    ToldToGoAway { go_away: GoAway },
    #[fail(
        display = "Invalid Opcode. actual={:?} expected={:?}",
        actual, expected
    )]
    InvalidOpcode { actual: u8, expected: Option<u8> },
    #[fail(
        display = "Unsupported Version. expected={} actual={}",
        expected, actual
    )]
    UnsupportedVersion { expected: u8, actual: u8 },
    #[fail(display = "No common encoding.")]
    NoCommonEncoding,
    #[fail(display = "No common compression.")]
    NoCommonCompression,
    #[fail(display = "Invalid encoding.")]
    InvalidEncoding,
    #[fail(display = "Invalid compression.")]
    InvalidCompression,
    #[fail(display = "Ping timeout.")]
    PingTimeout,
    #[fail(display = "Internal server error. error={:?}", error)]
    InternalServerError { error: Error },
    #[fail(display = "Event receive error.")]
    EventReceiveError,
    #[fail(display = "Ready send failed.")]
    ReadySendFailed,
}

pub enum LoquiErrorCode {
    // Normal is sent when the connection is closing cleanly.
    Normal,
    // InvalidOp is sent when the connection receives an opcode it cannot handle.
    InvalidOpcode,
    // UnsupportedVersion is sent when conn does not support a version.
    UnsupportedVersion,
    // NoCommonEncoding is sent when there are no common encodings.
    NoCommonEncoding,
    // InvalidEncoding is sent by the client if the server chooses an invalid encoding.
    InvalidEncoding,
    // InvalidCompression is sent by the client if the server chooses an invalid compression.
    InvalidCompression,
    // PingTimeout is sent when connection does not receive a pong within ping interval.
    PingTimeout,
    // InternalServerError is sent when a single request dies due to an error.
    InternalServerError,
}
