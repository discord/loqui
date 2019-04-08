/// Generates `sequence_id`s for requests.
pub struct IdSequence {
    next: u32,
}

impl IdSequence {
    pub fn new() -> Self {
        Self { next: 1 }
    }

    pub fn next(&mut self) -> u32 {
        let next = self.next;
        self.next = self.next.wrapping_add(self.next);
        next
    }
}
