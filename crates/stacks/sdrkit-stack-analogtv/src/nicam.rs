//! NICAM-728 framing and coding: the frame alignment word, the near-instantaneous companding with its scale factors signalled in
//! the parity bits, and the ITU-T J.17 emphasis. The scrambler and the bit interleave come from `sdrkit-util-bits`. The MAC sound uses
//! the same companding.
//!
//! Standard: ETSI EN 300 163 clause 3, ITU-T J.17.

use sdrkit_algo_dsp::iir::{Deemphasis, PreEmphasis};
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::seq::PRBS9_DISPERSAL;
use std::sync::OnceLock;

/// Bits a frame.
pub const FRAME_BITS: usize = 728;
/// The frame alignment word, first transmitted bit first.
pub const FAW: [u8; 8] = [0, 1, 0, 0, 1, 1, 1, 0];
/// Sound bits a frame.
pub const SOUND_BITS: usize = 704;
/// Sound words a frame.
pub const WORDS: usize = 64;

/// The symbol rate of the NICAM carrier, symbols a second.
pub const SYMBOL_RATE: f64 = 364_000.0;

/// The 720 scrambling bits of a frame: the energy dispersal sequence x^9 + x^5 + 1 from all ones, restarted every frame
/// (`sdrkit_util_bits::seq::PRBS9_DISPERSAL`, EN 300 163 clause 3.5).
pub fn scrambling_bits() -> &'static [u8; 720] {
    static S: OnceLock<[u8; 720]> = OnceLock::new();
    S.get_or_init(|| {
        let mut r = PRBS9_DISPERSAL;
        let mut out = [0u8; 720];
        r.fill_bits(&mut out);
        out
    })
}

/// The bit interleave of the 704 sound bits (EN 300 163 clause 3.6): transmitted bit `t` carries structure bit
/// `t/16 + 44 (t mod 16)`.
pub fn interleave() -> &'static Permutation {
    static P: OnceLock<Permutation> = OnceLock::new();
    P.get_or_init(|| Permutation::nicam().expect("704 bits"))
}

/// The structure bit carried by transmitted sound bit `t` (0 to 703).
#[inline]
pub fn deinterleave(t: usize) -> usize {
    interleave().src(t).unwrap_or(t)
}

/// The shift of a block of 14-bit samples with this peak magnitude (0 to 4).
pub fn shift_for_peak(peak: i32) -> u8 {
    let mut s = 0u8;
    while s < 4 && peak >= 512 << s {
        s += 1;
    }
    s
}

/// The 3-bit scale factor code of a block: the shift, and below 512 the protection range by level.
pub fn scale_factor(peak: i32) -> u8 {
    match shift_for_peak(peak) {
        4 => 7,
        3 => 6,
        2 => 5,
        1 => 3,
        _ => {
            if peak >= 256 {
                4
            } else if peak >= 64 {
                2
            } else {
                1
            }
        }
    }
}

/// The shift a scale factor code stands for.
pub fn shift_of_code(code: u8) -> u8 {
    match code & 7 {
        7 => 4,
        6 => 3,
        5 => 2,
        3 => 1,
        _ => 0,
    }
}

/// The peak magnitude of 14-bit samples (negative values count as `-x - 1`).
pub fn peak(x: &[i32]) -> i32 {
    x.iter()
        .map(|&v| if v < 0 { -v - 1 } else { v })
        .max()
        .unwrap_or(0)
}

/// Even parity of the six most significant bits of a 10-bit word.
#[inline]
pub fn parity6(word: u16) -> u8 {
    ((word >> 4) & 0x3F).count_ones() as u8 & 1
}

/// Compresses a block of 14-bit samples to 10-bit words with its scale factor code; returns the code.
pub fn compress(x: &[i32], words: &mut [u16]) -> u8 {
    let code = scale_factor(peak(x));
    let s = shift_of_code(code);
    for (w, &v) in words.iter_mut().zip(x) {
        let c = (v >> s).clamp(-512, 511);
        *w = (c as u16) & 0x3FF;
    }
    code
}

/// Expands a 10-bit word with a shift to a 14-bit sample.
#[inline]
pub fn expand(word: u16, shift: u8) -> i32 {
    let v = ((word as i32) << 22) >> 22;
    v << shift
}

/// The parity bits of a block of 32 words, carrying the 3 scale factor bits in turn on samples 0 to 26 (R2 on 0, 3, 6 ...; R1 on
/// 1, 4, 7 ...; R0 on 2, 5, 8 ...).
pub fn parities(words: &[u16], code: u8) -> impl Iterator<Item = u8> + '_ {
    words.iter().enumerate().map(move |(k, &w)| {
        let sig = if k < 27 { (code >> (2 - k % 3)) & 1 } else { 0 };
        parity6(w) ^ sig
    })
}

/// Recovers the scale factor of a block from its words and parity bits (a majority over the nine samples of each bit, which
/// take turns) and counts
/// the parity failures. Returns (code, the failed words as a bit mask).
pub fn recover(words: &[u16], par: &[u8]) -> (u8, u64) {
    let mut code = 0u8;
    for bit in 0..3 {
        let ones = (0..9)
            .filter(|&j| {
                let k = 3 * j + bit;
                k < words.len() && (parity6(words[k]) ^ par[k]) == 1
            })
            .count();
        code = (code << 1) | (ones >= 5) as u8;
    }
    let mut bad = 0u64;
    for (k, &w) in words.iter().enumerate() {
        let sig = if k < 27 { (code >> (2 - k % 3)) & 1 } else { 0 };
        if parity6(w) ^ par[k] != sig {
            bad |= 1 << k;
        }
    }
    (code, bad)
}

/// What a frame's control bits say.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Control {
    /// The frame flag C0.
    pub c0: u8,
    /// C1 C2 C3 as a number (C1 most significant).
    pub mode: u8,
    /// C4: the analogue sound carries the same programme.
    pub c4: u8,
}

sdrkit_core::choice_enum! {
    /// The sound mode of C1 C2 C3.
    pub enum Mode {
        /// 000.
        Stereo = ("stereo", "stereo"),
        /// 010.
        DualMono = ("dual_mono", "dual mono"),
        /// 100.
        MonoData = ("mono", "mono"),
        /// Anything else.
        Data = ("data", "data"),
    }
}

impl Mode {
    /// From C1 C2 C3.
    pub fn of(bits: u8) -> Mode {
        match bits {
            0 => Mode::Stereo,
            2 => Mode::DualMono,
            4 => Mode::MonoData,
            _ => Mode::Data,
        }
    }
    /// The C1 C2 C3 bits.
    pub fn bits(self) -> u8 {
        match self {
            Mode::Stereo => 0,
            Mode::DualMono => 2,
            Mode::MonoData => 4,
            Mode::Data => 1,
        }
    }
}

/// Builds frame number `n` (0-based) from 64 samples (stereo: A then B alternate; mono: in order) as 728 bits.
pub fn build_frame(n: u64, mode: Mode, samples: &[i32; 64], out: &mut [u8; FRAME_BITS]) {
    out[..8].copy_from_slice(&FAW);
    let c0 = (n / 8).is_multiple_of(2) as u8;
    let m = mode.bits();
    let ctl = [c0, (m >> 2) & 1, (m >> 1) & 1, m & 1, 1];
    out[8..13].copy_from_slice(&ctl);
    for b in &mut out[13..24] {
        *b = 0;
    }
    // Two blocks of 32: even and odd words.
    let mut a = [0i32; 32];
    let mut b = [0i32; 32];
    for k in 0..32 {
        a[k] = samples[2 * k];
        b[k] = samples[2 * k + 1];
    }
    let (mut wa, mut wb) = ([0u16; 32], [0u16; 32]);
    let ca = compress(&a, &mut wa);
    let cb = compress(&b, &mut wb);
    let (mut pa, mut pb) = ([0u8; 32], [0u8; 32]);
    for (d, p) in pa.iter_mut().zip(parities(&wa, ca)) {
        *d = p;
    }
    for (d, p) in pb.iter_mut().zip(parities(&wb, cb)) {
        *d = p;
    }
    let mut structure = [0u8; SOUND_BITS];
    for w in 0..WORDS {
        let (word, par) = if w % 2 == 0 {
            (wa[w / 2], pa[w / 2])
        } else {
            (wb[w / 2], pb[w / 2])
        };
        for bit in 0..10 {
            structure[w * 11 + bit] = ((word >> bit) & 1) as u8;
        }
        structure[w * 11 + 10] = par;
    }
    for t in 0..SOUND_BITS {
        out[24 + t] = structure[deinterleave(t)];
    }
    let prbs = scrambling_bits();
    for i in 0..720 {
        out[8 + i] ^= prbs[i];
    }
}

/// A decoded frame.
#[derive(Clone, Debug)]
pub struct Decoded {
    /// The control bits.
    pub control: Control,
    /// The 64 samples (14-bit), in word order.
    pub samples: [i32; 64],
    /// Words whose parity failed.
    pub bad: u64,
}

impl Default for Decoded {
    fn default() -> Self {
        Decoded {
            control: Control::default(),
            samples: [0; 64],
            bad: 0,
        }
    }
}

/// Decodes the 720 bits after the frame alignment word (still scrambled). `prev` is the last good sample of each block, used to
/// conceal a failed word.
pub fn decode_frame(bits: &[u8], prbs: &[u8; 720], prev: &mut [i32; 2], out: &mut Decoded) {
    let d = |i: usize| bits[i] ^ prbs[i];
    out.control = Control {
        c0: d(0),
        mode: (d(1) << 2) | (d(2) << 1) | d(3),
        c4: d(4),
    };
    let mut structure = [0u8; SOUND_BITS];
    for t in 0..SOUND_BITS {
        structure[deinterleave(t)] = d(16 + t);
    }
    let mut words = [[0u16; 32]; 2];
    let mut par = [[0u8; 32]; 2];
    for w in 0..WORDS {
        let mut v = 0u16;
        for bit in 0..10 {
            v |= (structure[w * 11 + bit] as u16) << bit;
        }
        words[w % 2][w / 2] = v;
        par[w % 2][w / 2] = structure[w * 11 + 10];
    }
    out.bad = 0;
    for blk in 0..2 {
        let (code, bad) = recover(&words[blk], &par[blk]);
        let s = shift_of_code(code);
        for k in 0..32 {
            let idx = 2 * k + blk;
            if bad & (1 << k) != 0 {
                out.samples[idx] = prev[blk];
                out.bad |= 1 << idx;
            } else {
                let v = expand(words[blk][k], s);
                out.samples[idx] = v;
                prev[blk] = v;
            }
        }
    }
}

/// The ITU-T J.17 pre-emphasis at the NICAM rate of 32 kHz (NICAM 728 sends J.17 pre-emphasised sound).
pub fn j17_pre() -> PreEmphasis {
    PreEmphasis::j17(32_000.0).expect("32 kHz is above the J.17 minimum rate")
}

/// The ITU-T J.17 de-emphasis at the NICAM rate of 32 kHz.
pub fn j17_de() -> Deemphasis {
    Deemphasis::j17(32_000.0).expect("32 kHz is above the J.17 minimum rate")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrambler_interleave_and_companding_follow_the_standard() {
        let s = scrambling_bits();
        let first: String = s[..20].iter().map(|b| char::from(b'0' + b)).collect();
        assert_eq!(first, "00000111101111100010");
        assert_eq!(deinterleave(0), 0);
        assert_eq!(deinterleave(1), 44);
        assert_eq!(deinterleave(2), 88);
        assert_eq!(deinterleave(16), 1);
        let mut seen = vec![false; SOUND_BITS];
        for t in 0..SOUND_BITS {
            seen[deinterleave(t)] = true;
        }
        assert!(seen.iter().all(|&b| b));
        assert_eq!(shift_for_peak(8191), 4);
        assert_eq!(shift_for_peak(511), 0);
        assert_eq!(shift_for_peak(512), 1);
        assert_eq!(shift_for_peak(4095), 3);
        assert_eq!(shift_for_peak(4096), 4);
        assert_eq!(scale_factor(8000), 7);
        assert_eq!(scale_factor(3000), 6);
        assert_eq!(scale_factor(1500), 5);
        assert_eq!(scale_factor(700), 3);
    }

    #[test]
    fn a_frame_round_trips() {
        let mut x = [0i32; 64];
        for (k, v) in x.iter_mut().enumerate() {
            *v = ((k as f64 * 0.3).sin() * 3000.0) as i32;
        }
        let mut bits = [0u8; FRAME_BITS];
        build_frame(3, Mode::Stereo, &x, &mut bits);
        assert_eq!(&bits[..8], &FAW);
        let mut prev = [0; 2];
        let mut d = Decoded::default();
        decode_frame(&bits[8..], scrambling_bits(), &mut prev, &mut d);
        assert_eq!(d.bad, 0);
        assert_eq!(d.control.mode, 0);
        assert_eq!(d.control.c4, 1);
        for k in 0..64 {
            assert!(
                (d.samples[k] - x[k]).abs() <= 8,
                "{k}: {} {}",
                d.samples[k],
                x[k]
            );
        }
    }
}
