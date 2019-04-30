pub enum Flags {
    None = 0,
    Compressed = 1,
}

pub fn is_compressed(flags: u8) -> bool {
    (flags & Flags::Compressed as u8) != 0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_compressed() {
        assert!(is_compressed(1))
    }

    #[test]
    fn it_is_not_compressed() {
        assert!(!is_compressed(0))
    }

    #[test]
    fn it_is_compressed_and_other_flag() {
        assert!(is_compressed(3))
    }

    #[test]
    fn it_makes_flags_compressed() {
        let flags = make_flags(true);
        assert!(is_compressed(flags))
    }

    #[test]
    fn it_makes_flags_not_compressed() {
        let flags = make_flags(false);
        assert!(!is_compressed(flags))
    }
}
