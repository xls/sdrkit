//! Varicodes of the keyboard protocols: the PSK31 varicode, the IZ8BLY varicode of MFSK16 and THOR (with THOR's secondary
//! characters), the DominoEX nibble varicode, and the FSQ and IFKP symbol varicodes.
//!
//! Each decoder is a state machine that takes one bit (or one symbol) at a time.

pub mod dominoex;
pub mod fsq;
pub mod mfsk;
pub mod psk;

/// A variable-length binary code: `len` bits, the first sent in the most significant of the `len` low bits of `bits`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BitCode {
    /// The bits, the first sent in bit `len - 1`.
    pub bits: u16,
    /// How many bits.
    pub len: u8,
}

impl BitCode {
    /// The code whose length is that of `bits` (codes that start with a 1).
    pub const fn from_value(bits: u16) -> BitCode {
        BitCode {
            bits,
            len: (16 - bits.leading_zeros()) as u8,
        }
    }

    /// The bits in the order sent.
    pub fn iter(self) -> impl Iterator<Item = u8> {
        (0..self.len)
            .rev()
            .map(move |i| ((self.bits >> i) & 1) as u8)
    }

    /// The code as text of 0 and 1, for display.
    pub fn to_bit_string(self) -> String {
        self.iter()
            .map(|b| if b == 1 { '1' } else { '0' })
            .collect()
    }
}
