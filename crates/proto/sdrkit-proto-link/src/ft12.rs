//! The FT1.2 telecontrol frame of IEC 60870-5-1 and IEC 60870-5-2 (DIN 19244): the 11-bit character (start 0, eight data bits
//! least significant first, even parity over the data, stop 1) and the frame of variable length `68 L L 68`, `L` user bytes, a
//! checksum (the sum of the user bytes modulo 256) and `16`.
//!
//! ```
//! use sdrkit_proto_link::ft12::{frame, Ft12Event, Ft12Reader};
//! let f = frame(&[0x25, 0x0C, 0x22, 0x41, 0x42, 0x43]);
//! assert_eq!(f, [0x68, 6, 6, 0x68, 0x25, 0x0C, 0x22, 0x41, 0x42, 0x43, 0x19, 0x16]);
//! let mut r = Ft12Reader::new(3, 40);
//! let got: Vec<Ft12Event> = f.iter().filter_map(|&b| r.push(b)).collect();
//! assert_eq!(got, [Ft12Event::Frame(f[4..10].to_vec())]);
//! ```

/// The start byte of a frame of variable length.
pub const START: u8 = 0x68;
/// The end byte of a frame.
pub const END: u8 = 0x16;

/// The frame of the user bytes `user` (at most 255).
pub fn frame(user: &[u8]) -> Vec<u8> {
    let l = user.len().min(255) as u8;
    let user = &user[..usize::from(l)];
    let mut f = vec![START, l, l, START];
    f.extend_from_slice(user);
    f.push(checksum(user));
    f.push(END);
    f
}

/// The checksum: the sum of `user` modulo 256.
pub fn checksum(user: &[u8]) -> u8 {
    user.iter().fold(0u8, |s, &b| s.wrapping_add(b))
}

/// The 11 bits of character `byte`, in sending order: start 0, the data least significant first, even parity, stop 1.
pub fn char_bits(byte: u8) -> [u8; 11] {
    let mut b = [0u8; 11];
    for i in 0..8 {
        b[1 + i] = (byte >> i) & 1;
    }
    b[9] = (byte.count_ones() % 2) as u8;
    b[10] = 1;
    b
}

/// What 11 received bits hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharRead {
    /// A good character.
    Byte(u8),
    /// The start bit is not 0 or the stop bit is not 1.
    Framing,
    /// The parity is wrong: the byte as received.
    Parity(u8),
}

/// Reads the character of `bits` (11 bits, sending order, each 0 or 1).
pub fn read_char(bits: &[u8]) -> CharRead {
    if bits.len() < 11 || bits[0] != 0 || bits[10] != 1 {
        return CharRead::Framing;
    }
    let byte = (0..8).fold(0u8, |v, i| v | ((bits[1 + i] & 1) << i));
    if (byte.count_ones() % 2) as u8 != bits[9] & 1 {
        CharRead::Parity(byte)
    } else {
        CharRead::Byte(byte)
    }
}

/// What the frame reader found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ft12Event {
    /// A good frame: its user bytes.
    Frame(Vec<u8>),
    /// A frame whose checksum or end byte is wrong: its user bytes as received.
    BadChecksum(Vec<u8>),
}

/// The reader of frames of variable length from a stream of good bytes.
#[derive(Clone, Debug)]
pub struct Ft12Reader {
    min_len: u8,
    max_len: u8,
    buf: Vec<u8>,
}

impl Ft12Reader {
    /// A reader of frames with `min_len` to `max_len` user bytes.
    pub fn new(min_len: u8, max_len: u8) -> Self {
        Ft12Reader {
            min_len,
            max_len,
            buf: Vec::new(),
        }
    }

    /// Drops a frame half read.
    pub fn reset(&mut self) {
        self.buf.clear();
    }

    /// Takes the next byte; returns a frame when its last byte is in.
    pub fn push(&mut self, b: u8) -> Option<Ft12Event> {
        self.buf.push(b);
        loop {
            // Skip to a start byte, and check the header as it comes in.
            match self.buf.iter().position(|&v| v == START) {
                None => {
                    self.buf.clear();
                    return None;
                }
                Some(p) if p > 0 => {
                    self.buf.drain(..p);
                }
                _ => {}
            }
            let ok = match self.buf.len() {
                1 => true,
                2 | 3 => {
                    let l = self.buf[1];
                    (self.min_len..=self.max_len).contains(&l)
                        && (self.buf.len() < 3 || self.buf[2] == l)
                }
                _ => self.buf[3] == START,
            };
            if ok {
                break;
            }
            self.buf.drain(..1);
        }
        if self.buf.len() < 4 {
            return None;
        }
        let l = usize::from(self.buf[1]);
        if self.buf.len() < 4 + l + 2 {
            return None;
        }
        let user = self.buf[4..4 + l].to_vec();
        let good = self.buf[4 + l] == checksum(&user) && self.buf[5 + l] == END;
        self.buf.clear();
        Some(if good {
            Ft12Event::Frame(user)
        } else {
            Ft12Event::BadChecksum(user)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characters_round_trip_and_errors_are_named() {
        for b in 0..=255u8 {
            let bits = char_bits(b);
            assert_eq!(read_char(&bits), CharRead::Byte(b));
            let mut p = bits;
            p[9] ^= 1;
            assert_eq!(read_char(&p), CharRead::Parity(b));
            let mut s = bits;
            s[10] = 0;
            assert_eq!(read_char(&s), CharRead::Framing);
        }
    }

    #[test]
    fn a_changed_byte_is_a_bad_checksum_and_noise_before_is_skipped() {
        let mut f = frame(&[0x25, 0x0C, 0x22, 0x41, 0x42, 0x43]);
        f[6] ^= 1;
        let mut r = Ft12Reader::new(3, 40);
        let mut s = vec![0x68, 0x00, 0x12, 0x68, 0x68, 0x07];
        s.extend(&f);
        let got: Vec<Ft12Event> = s.iter().filter_map(|&b| r.push(b)).collect();
        assert_eq!(got.len(), 1);
        assert!(matches!(&got[0], Ft12Event::BadChecksum(u) if u.len() == 6));
    }
}
