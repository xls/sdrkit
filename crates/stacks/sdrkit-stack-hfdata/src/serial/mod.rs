//! Parts that the serial-tone stacks of this crate share: the 1800 Hz front with its root-raised-cosine matched filter
//! ([`front`]), the T/2 decision-feedback equaliser with its phase loop ([`dfe`]), max-log soft bits ([`soft`]), the test-signal
//! modulator ([`gen`]) and the text, D_PDU table and events of a modem ([`board`]).

pub mod board;
pub mod dfe;
pub mod front;
pub mod gen;
pub mod soft;

use sdrkit_core::Complex32;
use std::f32::consts::FRAC_PI_4;

/// The audio carrier of the serial-tone waveforms, Hz above the dial.
pub const CARRIER_HZ: f64 = 1800.0;
/// The symbol rate, Bd.
pub const BAUD: f64 = 2400.0;
/// The tune offset of the stacks of this crate: the radio is tuned this far above the dial, Hz.
pub const TUNE_OFFSET_HZ: f64 = 20e3;

/// The 8-PSK point of tribit `n`: phase `n` x 45 degrees.
pub fn psk8(n: u8) -> Complex32 {
    let (s, c) = (f32::from(n & 7) * FRAC_PI_4).sin_cos();
    Complex32::new(c, s)
}

/// The bits of `bytes`, each byte least significant bit first.
pub fn bits_lsb(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .flat_map(|b| (0..8).map(move |i| (b >> i) & 1))
        .collect()
}

/// The bits of the low `n` bits of `v`, most significant first.
pub fn bits_msb(v: u64, n: usize) -> Vec<u8> {
    (0..n).rev().map(|i| ((v >> i) & 1) as u8).collect()
}

/// The 32-bit end-of-message pattern of the serial-tone modems, sent most significant bit first.
pub const EOM: u32 = 0x4B65_A5B2;

/// The neutral text of the test signals.
pub const TEST_TEXT: &str = "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG 12345 TEST MESSAGE";

/// Checks that the input is the 24 kHz channel of the family front end.
///
/// # Errors
///
/// [`sdrkit_core::ConfigError::RateNotMultiple`] for another rate.
pub fn check_rate(input: &sdrkit_graph::Format) -> Result<(), sdrkit_core::ConfigError> {
    if (input.rate - front::RATE_IN).abs() > 1e-6 {
        return Err(sdrkit_core::ConfigError::RateNotMultiple {
            got: input.rate,
            of: front::RATE_IN,
        });
    }
    Ok(())
}

/// Finds a 32-bit pattern in a stream of bits (first-sent bit the most significant of the pattern).
#[derive(Clone, Copy, Debug)]
pub struct PatternWatch {
    pattern: u32,
    reg: u32,
    seen: u32,
}

impl PatternWatch {
    /// A watch for `pattern`.
    pub fn new(pattern: u32) -> Self {
        PatternWatch {
            pattern,
            reg: 0,
            seen: 0,
        }
    }
    /// Takes the next bit; true when the last 32 bits are the pattern.
    pub fn push(&mut self, bit: u8) -> bool {
        self.reg = (self.reg << 1) | u32::from(bit & 1);
        self.seen = self.seen.saturating_add(1);
        self.seen >= 32 && self.reg == self.pattern
    }
    /// Forgets the bits seen.
    pub fn reset(&mut self) {
        self.reg = 0;
        self.seen = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eom_is_found_in_its_bits() {
        let mut w = PatternWatch::new(EOM);
        let mut bits = vec![1, 1, 0];
        bits.extend(bits_msb(u64::from(EOM), 32));
        let hits: Vec<usize> = bits
            .iter()
            .enumerate()
            .filter(|(_, &b)| w.push(b))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(hits, [34]);
        assert_eq!(
            &bits_msb(u64::from(EOM >> 24), 8),
            &[0, 1, 0, 0, 1, 0, 1, 1]
        );
    }
}
