pub mod codec;
pub mod errors;
mod flags;
pub mod frames;

pub use self::flags::{is_compressed, make_flags, Flags};
pub use self::frames::{Error, Push, Request, Response};

pub const VERSION: u8 = 1;
