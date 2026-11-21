//! Bits to marks and spaces: the line codes of [`crate::slicer`] in the other direction, for generators and tests.
//!
//! Each function appends pulses to a [`Train`], with lengths given in microseconds and stored in samples at the train's rate. A
//! generator then keys a carrier from the train (`sdrkit_algo_sim::modulate::Ook` for OOK, a frequency modulator for FSK).
//!
//! ```
//! use sdrkit_proto_ookdev::{encode, Modulation, Train};
//! let mut t = Train::new(Modulation::Ook, 1e6);
//! encode::pwm(&[1, 0], (400.0, 1200.0), (1200.0, 400.0), &mut t);
//! assert_eq!(t.pulses[1].mark, 1200.0);
//! ```

use crate::train::{Pulse, Train};
use sdrkit_util_bits::line::{Manchester, ManchesterConv};

/// Pulse width: each bit is one pulse, `one` = (mark, space) µs for a 1 and `zero` for a 0.
pub fn pwm(bits: &[u8], one: (f32, f32), zero: (f32, f32), train: &mut Train) {
    for &b in bits {
        let (m, s) = if b & 1 == 1 { one } else { zero };
        train
            .pulses
            .push(Pulse::new(train.samples(m), train.samples(s)));
    }
}

/// Pulse position: each bit is a mark of `mark` µs followed by a space of `zero` µs for a 0 or `one` µs for a 1.
pub fn ppm(bits: &[u8], mark: f32, zero: f32, one: f32, train: &mut Train) {
    for &b in bits {
        let s = if b & 1 == 1 { one } else { zero };
        train
            .pulses
            .push(Pulse::new(train.samples(mark), train.samples(s)));
    }
}

/// Non-return-to-zero: each level (1 high, 0 low) lasts `bit_us`. The train starts at the first high level, so leading 0
/// levels are not sent. The last space is the length of the trailing low levels (0 when the levels end high). Returns the
/// number of leading 0 levels skipped.
pub fn nrz(levels: &[u8], bit_us: f32, train: &mut Train) -> usize {
    let t = train.samples(bit_us);
    let first = levels
        .iter()
        .position(|&l| l & 1 == 1)
        .unwrap_or(levels.len());
    let mut i = first;
    while i < levels.len() {
        let a = i;
        while i < levels.len() && levels[i] & 1 == 1 {
            i += 1;
        }
        let b = i;
        while i < levels.len() && levels[i] & 1 == 0 {
            i += 1;
        }
        train
            .pulses
            .push(Pulse::new((b - a) as f32 * t, (i - b) as f32 * t));
    }
    first
}

/// Manchester: two half-bit levels of `half_us` per bit, by `conv` (`sdrkit_util_bits::line::Manchester`), then [`nrz`]. `chips` is
/// scratch (cleared first). Returns the number of leading low half-bits skipped.
pub fn manchester(
    bits: &[u8],
    conv: ManchesterConv,
    half_us: f32,
    chips: &mut Vec<u8>,
    train: &mut Train,
) -> usize {
    chips.clear();
    Manchester::encode(bits, conv, chips);
    nrz(chips, half_us, train)
}
