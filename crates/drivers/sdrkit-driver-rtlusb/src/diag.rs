//! The check of the counter test mode (specification 5.5): in that mode every byte of the stream is the previous byte
//! plus 1, so a break in the sequence shows bytes that were lost.

/// Counts the breaks in a counter stream.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CounterCheck {
    next: Option<u8>,
    /// The bytes checked.
    pub bytes: u64,
    /// The places where a byte is not the previous byte plus 1.
    pub breaks: u64,
    /// The bytes lost at the breaks, each counted modulo 256 (a loss of a whole multiple of 256 bytes is not seen).
    pub lost: u64,
}

impl CounterCheck {
    /// A check that takes the first byte as the start.
    pub fn new() -> CounterCheck {
        CounterCheck::default()
    }

    /// Checks the bytes of `data`, which follow the bytes checked before.
    pub fn feed(&mut self, data: &[u8]) {
        for &b in data {
            if let Some(n) = self.next {
                if b != n {
                    self.breaks += 1;
                    self.lost += u64::from(b.wrapping_sub(n));
                }
            }
            self.next = Some(b.wrapping_add(1));
        }
        self.bytes += data.len() as u64;
    }

    /// Takes the next byte as a new start, after a gap that the stream marked.
    pub fn restart(&mut self) {
        self.next = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breaks_and_losses_are_counted_modulo_256() {
        let mut c = CounterCheck::new();
        let data: Vec<u8> = (0..1000u32).map(|i| i as u8).collect();
        c.feed(&data[..500]);
        c.feed(&data[500..]);
        assert_eq!((c.bytes, c.breaks, c.lost), (1000, 0, 0));
        // 10 bytes missing, then 300 (seen as 44).
        c.feed(&[0xF2, 0xF3]);
        assert_eq!((c.breaks, c.lost), (1, 10));
        c.feed(&[0xF4u8.wrapping_add(44)]);
        assert_eq!((c.breaks, c.lost), (2, 54));
        c.restart();
        c.feed(&[7, 8]);
        assert_eq!(c.breaks, 2);
    }
}
