use super::connection::Event;
use super::connection_handler::{ConnectionHandler, DelegatedFrame, TransportOptions};
use super::error::LoquiError;
use super::id_sequence::IdSequence;
use super::sender::ConnectionSender;
use crate::LoquiErrorCode;
use failure::Error;
use loqui_protocol::frames::{Error as ErrorFrame, LoquiFrame, Ping, Pong, Response};

/// Main handler of connection `Event`s.
pub struct EventHandler<C: ConnectionHandler> {
    connection_handler: C,
    pong_received: bool,
    id_sequence: IdSequence,
    connection_sender: ConnectionSender<C::InternalEvent>,
    transport_options: TransportOptions,
}

/// Standard return type for handler functions.
///
/// Event handler functions return an optional `LoquiFrame` that will
/// be sent back over the connection.
type MaybeFrameResult = Result<Option<LoquiFrame>, Error>;

impl<C: ConnectionHandler> EventHandler<C> {
    pub fn new(
        connection_sender: ConnectionSender<C::InternalEvent>,
        connection_handler: C,
        transport_options: TransportOptions,
    ) -> Self {
        Self {
            connection_handler,
            pong_received: true,
            id_sequence: IdSequence::new(),
            connection_sender,
            transport_options,
        }
    }

    /// High level event handler entry point. This is called by the connection whenever an
    /// event comes in.
    pub fn handle_event(&mut self, event: Event<C::InternalEvent>) -> MaybeFrameResult {
        match event {
            Event::Ping => self.handle_ping(),
            Event::SocketReceive(frame) => self.handle_frame(frame),
            Event::InternalEvent(internal_event) => self.handle_internal_event(internal_event),
            Event::ResponseComplete(response) => self.handle_response_complete(response),
        }
    }

    /// Handles a request to ping the other side. Returns an `Error` if a `Pong` hasn't been
    /// received since the last ping.
    fn handle_ping(&mut self) -> MaybeFrameResult {
        if self.pong_received {
            let sequence_id = self.id_sequence.next();
            let ping = Ping {
                sequence_id,
                flags: 0,
            };
            self.pong_received = false;
            Ok(Some(ping.into()))
        } else {
            Err(LoquiError::PingTimeout.into())
        }
    }

    /// Handles a frame received from the socket. Delegates some frames to the `ConnectionHandler`.
    /// Optionally returns a `LoquiFrame` that will be sent back over the socket.
    fn handle_frame(&mut self, frame: LoquiFrame) -> MaybeFrameResult {
        match frame {
            LoquiFrame::Hello(_) | LoquiFrame::HelloAck(_) => self.handle_handshake_frame(frame),
            LoquiFrame::Ping(ping) => self.handle_ping_frame(ping),
            LoquiFrame::Pong(pong) => self.handle_pong_frame(pong),
            LoquiFrame::Request(request) => self.delegate_frame(request),
            LoquiFrame::Response(response) => self.delegate_frame(response),
            LoquiFrame::Push(push) => self.delegate_frame(push),
            LoquiFrame::GoAway(go_away) => Err(LoquiError::GoAway { go_away }.into()),
            LoquiFrame::Error(error) => self.handle_error_frame(error),
        }
    }

    /// Handshake should have already completed. This is an error at this point.
    fn handle_handshake_frame(&mut self, frame: LoquiFrame) -> MaybeFrameResult {
        Err(LoquiError::InvalidOpcode {
            actual: frame.opcode(),
            expected: None,
        }
        .into())
    }

    /// There was an error. Shutdown.
    fn handle_error_frame(&mut self, error: ErrorFrame) -> MaybeFrameResult {
        // TODO
        println!("Error. error={:?}", error);
        Ok(None)
    }

    /// Delegates a frame to the connection handler.
    fn delegate_frame<D: Into<DelegatedFrame>>(&mut self, delegated_frame: D) -> MaybeFrameResult {
        let delegated_frame = delegated_frame.into();
        let maybe_future = self
            .connection_handler
            .handle_frame(delegated_frame, &self.transport_options);
        // If the connection handler returns a future, execute the future async and send it back
        // to the main event loop. The main event loop will send it through the socket.
        if let Some(future) = maybe_future {
            let connection_sender = self.connection_sender.clone();
            tokio::spawn_async(
                async move {
                    let response = await!(future);
                    // TODO: handle this result
                    let result = connection_sender.response_complete(response);
                },
            );
        }
        Ok(None)
    }

    fn handle_ping_frame(&self, ping: Ping) -> MaybeFrameResult {
        let pong = Pong {
            flags: ping.flags,
            sequence_id: ping.sequence_id,
        };
        Ok(Some(pong.into()))
    }

    fn handle_pong_frame(&mut self, _pong: Pong) -> MaybeFrameResult {
        self.pong_received = true;
        Ok(None)
    }

    /// A response was computed. Send it back over the socket.
    fn handle_response_complete(&self, result: Result<Response, (Error, u32)>) -> MaybeFrameResult {
        match result {
            Ok(response) => Ok(Some(response.into())),
            Err((error, sequence_id)) => {
                let error = ErrorFrame {
                    flags: 0,
                    sequence_id,
                    // TODO:
                    code: LoquiErrorCode::InternalServerError as u16,
                    payload: format!("{:?}", error.to_string()).as_bytes().to_vec(),
                };
                Ok(Some(error.into()))
            }
        }
    }

    fn handle_internal_event(&mut self, internal_event: C::InternalEvent) -> MaybeFrameResult {
        Ok(self.connection_handler.handle_internal_event(
            internal_event,
            &mut self.id_sequence,
            &self.transport_options,
        ))
    }
}
