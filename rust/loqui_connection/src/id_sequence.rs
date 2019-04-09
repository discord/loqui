/// Generates `sequence_id`s for requests.
pub struct IdSequence {
    next: u32,
}

impl IdSequence {
    pub fn next(&mut self) -> u32 {
        let next = self.next;
        self.next = self.next.wrapping_add(1);
        next
    }
}

impl Default for IdSequence {
    fn default() -> Self {
        Self { next: 1 }
    }
}
