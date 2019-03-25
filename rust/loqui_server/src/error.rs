use failure::Fail;
use loqui_protocol::codec::LoquiFrame;

#[derive(Debug, Fail)]
pub enum LoquiError {
    #[fail(display = "Connection not ready. frame={:?}", frame)]
    NotReady { frame: LoquiFrame },
    #[fail(display = "Invalid frame. frame={:?}", frame)]
    InvalidFrame { frame: LoquiFrame },
}
