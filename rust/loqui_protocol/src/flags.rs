pub enum Flags {
    None = 0,
    Compressed = 1,
}

pub fn is_compressed(flags: u8) -> bool {
    let as_u8 = Flags::Compressed as u8;
    flags & as_u8 == as_u8
}

/// Creates the u8 flags for a frame. We only have one flag right now, Flags::Compressed.
pub fn make_flags(compressed: bool) -> u8 {
    let flag = if compressed {
        Flags::Compressed
    } else {
        Flags::None
    };
    flag as u8
}
