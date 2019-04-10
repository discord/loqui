use crate::framed_io::ReaderWriter;
use crate::id_sequence::IdSequence;
use bytesize::ByteSize;
use failure::Error;
use loqui_protocol::frames::{Error as ErrorFrame, LoquiFrame, Push, Request, Response};
use std::future::Future;
use std::time::Duration;
use tokio::net::TcpStream;

/// Negotiated transport options.
#[derive(Debug)]
pub struct TransportOptions {
    pub encoding: &'static str,
    pub compression: Option<&'static str>,
}

/// Specific types of loqui frames that are delegated to a connection handler.  The rest of the
/// frames will be handled by the connection itself.
#[derive(Debug)]
pub enum DelegatedFrame {
    Push(Push),
    Request(Request),
    Response(Response),
    Error(ErrorFrame),
}

/// Settings negotiated from handshake.
#[derive(Debug)]
pub struct Ready {
    pub ping_interval: Duration,
    pub transport_options: TransportOptions,
}

/// A trait that handles the specific functionality of a connection. The client and server each
/// implement this.
pub trait Handler: Send + Sync + 'static {
    /// Events specific to the implementing connection handler. They will be passed through to the
    /// handle_internal_event callback.
    type InternalEvent: Send;
    /// Result of upgrading. Needs to be a type so we don't have to box the future.
    type UpgradeFuture: Send + Future<Output = Result<TcpStream, Error>>;
    /// Result of handshaking. Needs to be a type so we don't have to box the future.
    type HandshakeFuture: Send
        + Future<
            Output = Result<(Ready, ReaderWriter), (Error, Option<ReaderWriter>)>,
        >;
    /// Result of handling a frame. Needs to be a type so we don't have to box the future.
    type HandleFrameFuture: Send + Future<Output = Result<Response, (Error, u32)>>;

    // Whether or not the connection should send a GoAway frame on close.
    const SEND_GO_AWAY: bool;

    /// The maximum payload size this connection can handle.
    fn max_payload_size(&self) -> ByteSize;
    /// Takes a tcp stream and completes an HTTP upgrade.
    fn upgrade(&self, tcp_stream: TcpStream) -> Self::UpgradeFuture;
    /// Hello/HelloAck handshake.
    fn handshake(&mut self, reader_writer: ReaderWriter) -> Self::HandshakeFuture;
    /// Handle a single delegated frame. Optionally returns a future that resolves to a
    /// Response. The Response will be sent back through the socket to the other side.
    fn handle_frame(
        &mut self,
        frame: DelegatedFrame,
        transport_options: &TransportOptions,
    ) -> Option<Self::HandleFrameFuture>;
    /// Handle internal events for this connection. Completely opaque to the connection. Optionally
    /// return a `LoquiFrame` that will be sent back through the socket to the other side.
    fn handle_internal_event(
        &mut self,
        event: Self::InternalEvent,
        id_sequence: &mut IdSequence,
        transport_options: &TransportOptions,
    ) -> Option<LoquiFrame>;
    /// Periodic callback that fires whenever a ping fires.
    fn handle_ping(&mut self);
}

impl From<Push> for DelegatedFrame {
    fn from(push: Push) -> DelegatedFrame {
        DelegatedFrame::Push(push)
    }
}

impl From<Request> for DelegatedFrame {
    fn from(request: Request) -> DelegatedFrame {
        DelegatedFrame::Request(request)
    }
}

impl From<Response> for DelegatedFrame {
    fn from(response: Response) -> DelegatedFrame {
        DelegatedFrame::Response(response)
    }
}

impl From<ErrorFrame> for DelegatedFrame {
    fn from(error: ErrorFrame) -> DelegatedFrame {
        DelegatedFrame::Error(error)
    }
}
