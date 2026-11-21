//! The HFDL burst: the A, M and T sequences, the eight rates, the scrambler, the 40-row interleaver, the K = 7 rate 1/2 code,
//! the PSK mapping and soft bits, and the data decoder that keeps its trellis and buffers from burst to burst.
//! Standard: ARINC 635-4 clause 4 (physical layer), ICAO Doc 9741.

use sdrkit_algo_fec::conv::{codes::HFDL_K7, Encoder, Ends, Viterbi};
use sdrkit_algo_fec::llr::{Llr8, Quantizer};
use sdrkit_core::Complex32;
use sdrkit_util_bits::gray::ungray;
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::lfsr::Lfsr;
use sdrkit_util_bits::seq;

/// Symbols per second.
pub const SYMBOL_RATE: f64 = 1800.0;
/// The carrier is this far above the dial.
pub const CARRIER_HZ: f64 = 1440.0;
/// Prekey symbols.
pub const PREKEY: usize = 448;
/// Symbols from A1 to the last training block.
pub const PREAMBLE: usize = 531;

/// The T sequence.
pub const T: [f32; 15] = [
    1.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, -1.0, -1.0, -1.0, -1.0,
];

const A_OCTETS: [u8; 16] = [
    0x5B, 0xBC, 0x74, 0x57, 0x03, 0xD9, 0x89, 0x39, 0xF2, 0x08, 0xD5, 0x36, 0x94, 0x2C, 0x32, 0xFE,
];
const M_BASE: &str = "0111011011110100010110010111110001000000110011011000111001110101110000100110000010101011010010010100111100100011010100001111111";
/// The M1 shifts of the eight indices.
pub const M_SHIFTS: [usize; 8] = [72, 82, 113, 123, 61, 103, 93, 9];

/// The A sequence as symbols (bit 0 = +1).
pub fn a_seq() -> [f32; 127] {
    let mut a = [0.0; 127];
    for (k, v) in a.iter_mut().enumerate() {
        let bit = (A_OCTETS[k / 8] >> (7 - k % 8)) & 1;
        *v = if bit == 0 { 1.0 } else { -1.0 };
    }
    a
}

/// The M sequence of index `i` as symbols.
pub fn m_seq(i: usize) -> [f32; 127] {
    let b = M_BASE.as_bytes();
    let s = M_SHIFTS[i & 7];
    let mut m = [0.0; 127];
    for (k, v) in m.iter_mut().enumerate() {
        *v = if b[(s + k) % 127] == b'0' { 1.0 } else { -1.0 };
    }
    m
}

/// A rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    /// M1 index.
    pub index: usize,
    /// bit/s.
    pub bps: u32,
    /// Bits per symbol.
    pub bits: usize,
    /// Each coded bit sent twice.
    pub repeat: bool,
    /// Slots.
    pub slots: u8,
    /// Data blocks.
    pub blocks: usize,
    /// Interleaver column shift.
    pub shift: usize,
}

/// The eight rates (ARINC 635 table 4-1).
pub fn rate(index: usize) -> Rate {
    let i = index & 7;
    let (bps, bits, repeat) = match i % 4 {
        0 => (300, 1, true),
        1 => (600, 1, false),
        2 => (1200, 2, false),
        _ => (1800, 3, false),
    };
    let two = i >= 4;
    Rate {
        index: i,
        bps,
        bits,
        repeat,
        slots: if two { 2 } else { 1 },
        blocks: if two { 168 } else { 72 },
        shift: if two { 23 } else { 17 },
    }
}

impl Rate {
    /// Channel bits in the burst.
    pub fn channel_bits(&self) -> usize {
        self.blocks * 30 * self.bits
    }
    /// Coded bits (before repetition).
    pub fn coded_bits(&self) -> usize {
        self.channel_bits() / if self.repeat { 2 } else { 1 }
    }
    /// Octets of PDU.
    pub fn octets(&self) -> usize {
        (self.coded_bits() / 2 - 6) / 8
    }
    /// Data symbols.
    pub fn data_symbols(&self) -> usize {
        self.blocks * 30
    }
    /// Symbols from A1 to the end.
    pub fn total_symbols(&self) -> usize {
        PREAMBLE + self.blocks * 45
    }
}

/// The scrambler: the register of [`seq::VDL2`], loaded again every 120 data symbols (ARINC 635 clause 4.4.4).
#[derive(Clone, Debug)]
pub struct Scrambler {
    reg: Lfsr,
    k: usize,
}

impl Default for Scrambler {
    fn default() -> Self {
        Scrambler {
            reg: seq::VDL2,
            k: 0,
        }
    }
}

impl Scrambler {
    /// Whether data symbol number `k` (from 0, in order) is inverted.
    #[inline]
    pub fn inverted(&mut self) -> bool {
        if self.k.is_multiple_of(120) {
            self.reg = seq::VDL2;
        }
        self.k += 1;
        self.reg.bit() != 0
    }
}

/// The constellation point of a label (first bit sent = most significant) for `bits` per symbol: BPSK, or Gray-coded 4- and
/// 8-PSK.
pub fn map(label: u32, bits: usize) -> Complex32 {
    if bits == 1 {
        return Complex32::new(if label == 0 { 1.0 } else { -1.0 }, 0.0);
    }
    let m = 1u32 << bits;
    let k = ungray(label);
    let a = std::f64::consts::TAU * k as f64 / m as f64;
    Complex32::new(a.cos() as f32, a.sin() as f32)
}

/// Soft bits of a received point (positive = 0), scaled by `w`, appended to `out`.
pub fn soft_bits(z: Complex32, bits: usize, w: f32, out: &mut Vec<f32>) {
    if bits == 1 {
        out.push(z.re * w);
        return;
    }
    let m = 1u32 << bits;
    let mut d = [0.0f32; 8];
    for (l, dl) in d.iter_mut().enumerate().take(m as usize) {
        *dl = (z - map(l as u32, bits)).norm_sqr();
    }
    for b in (0..bits).rev() {
        let (mut d0, mut d1) = (f32::MAX, f32::MAX);
        for (l, &dl) in d.iter().enumerate().take(m as usize) {
            if (l >> b) & 1 == 0 {
                d0 = d0.min(dl);
            } else {
                d1 = d1.min(dl);
            }
        }
        out.push((d1 - d0) * w);
    }
}

/// The interleaver of a rate (ARINC 635 clause 4.4.6).
pub fn interleaver(r: Rate) -> Permutation {
    Permutation::hfdl(r.coded_bits() * if r.repeat { 2 } else { 1 }, r.shift)
        .expect("every rate has a multiple of 40 channel bits")
}

/// The symbols of a burst from A1 (preamble, then data blocks with probes), for `pdu` (padded to the burst) at `r`.
pub fn burst_symbols(pdu: &[u8], r: Rate) -> Vec<Complex32> {
    let nbits = r.octets() * 8;
    let bits: Vec<u8> = (0..nbits)
        .map(|i| pdu.get(i / 8).map_or(0, |o| (o >> (i % 8)) & 1))
        .collect();
    let mut enc = Encoder::new(HFDL_K7);
    let mut coded = Vec::with_capacity(r.coded_bits());
    enc.encode(&bits, &mut coded);
    enc.terminate(&mut coded);
    coded.resize(r.coded_bits(), 0);
    if r.repeat {
        coded = coded.iter().flat_map(|&b| [b, b]).collect();
    }
    let mut chan = vec![0u8; coded.len()];
    interleaver(r)
        .apply(&coded, &mut chan)
        .expect("the interleaver has the burst's length");
    let mut s: Vec<Complex32> = Vec::with_capacity(r.total_symbols());
    let real = |v: f32| Complex32::new(v, 0.0);
    s.extend(preamble(r.index).iter().map(|&v| real(v)));
    let mut scr = Scrambler::default();
    let mut k = 0;
    for _ in 0..r.blocks {
        for _ in 0..30 {
            let mut label = 0u32;
            for _ in 0..r.bits {
                label = (label << 1) | chan[k] as u32;
                k += 1;
            }
            let p = map(label, r.bits);
            s.push(if scr.inverted() { -p } else { p });
        }
        s.extend(T.iter().map(|&v| real(v)));
    }
    s
}

/// The known symbols of the preamble (from A1).
pub fn preamble(index: usize) -> Vec<f32> {
    let mut s = Vec::with_capacity(PREAMBLE);
    let a = a_seq();
    s.extend_from_slice(&a);
    s.extend_from_slice(&a);
    let m = m_seq(index);
    s.extend_from_slice(&m);
    s.extend_from_slice(&m[..15]);
    for _ in 0..9 {
        s.extend_from_slice(&T);
    }
    s
}

/// The data decoder: the interleavers of the eight rates, one Viterbi decoder sized for the longest burst, and the buffers.
/// It allocates nothing once each rate was seen.
pub struct DataDecoder {
    perms: [Option<Permutation>; 8],
    viterbi: Viterbi,
    soft: Vec<f32>,
    coded: Vec<f32>,
    llr: Vec<Llr8>,
    bits: Vec<u8>,
}

impl Default for DataDecoder {
    fn default() -> Self {
        // The longest burst: 944 octets and the 6 tail bits.
        let steps = rate(7).octets() * 8 + 6;
        DataDecoder {
            perms: Default::default(),
            viterbi: Viterbi::new(HFDL_K7, steps).expect("the HFDL code fits the decoder"),
            soft: Vec::with_capacity(rate(7).channel_bits()),
            coded: Vec::with_capacity(rate(7).channel_bits()),
            llr: Vec::with_capacity(rate(7).coded_bits()),
            bits: Vec::with_capacity(steps),
        }
    }
}

impl DataDecoder {
    /// Decodes the data symbols of a burst (descrambled in here, `points` as received, `weights` their reliabilities) to PDU
    /// octets in `out` (cleared first, [`Rate::octets`] long).
    pub fn decode(&mut self, points: &[Complex32], weights: &[f32], r: Rate, out: &mut Vec<u8>) {
        let mut scr = Scrambler::default();
        self.soft.clear();
        for (&p, &w) in points.iter().zip(weights) {
            let p = if scr.inverted() { -p } else { p };
            soft_bits(p, r.bits, w, &mut self.soft);
        }
        let n = r.channel_bits();
        self.soft.resize(n, 0.0);
        let perm = self.perms[r.index].get_or_insert_with(|| interleaver(r));
        self.coded.clear();
        self.coded.resize(n, 0.0);
        let _ = perm.invert(&self.soft, &mut self.coded);
        if r.repeat {
            for i in 0..n / 2 {
                self.coded[i] = self.coded[2 * i] + self.coded[2 * i + 1];
            }
            self.coded.truncate(n / 2);
        }
        // The code word: the PDU bits and the six tail bits. The padding after it is not coded.
        let steps = r.octets() * 8 + 6;
        let used = &self.coded[..2 * steps];
        self.llr.clear();
        self.llr.resize(used.len(), 0);
        let _ = Quantizer::default().quantize_auto(used, &mut self.llr);
        self.bits.clear();
        self.bits.resize(steps - 6, 0);
        out.clear();
        out.resize(r.octets(), 0);
        if self
            .viterbi
            .decode(&self.llr, Ends::Zero, &mut self.bits)
            .is_err()
        {
            return;
        }
        for (i, &b) in self.bits.iter().enumerate() {
            out[i / 8] |= (b & 1) << (i % 8);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_and_octets() {
        let o: Vec<usize> = (0..8).map(|i| rate(i).octets()).collect();
        assert_eq!(o, [66, 134, 269, 404, 156, 314, 629, 944]);
        assert_eq!(rate(0).total_symbols() - PREAMBLE, 72 * 45);
    }

    #[test]
    fn every_rate_round_trips_clean() {
        let mut dec = DataDecoder::default();
        let mut out = Vec::new();
        for idx in 0..8 {
            let r = rate(idx);
            let pdu: Vec<u8> = (0..r.octets()).map(|i| (i * 31 + idx) as u8).collect();
            let s = burst_symbols(&pdu, r);
            assert_eq!(s.len(), r.total_symbols());
            let mut pts = Vec::new();
            for b in 0..r.blocks {
                pts.extend_from_slice(&s[PREAMBLE + b * 45..PREAMBLE + b * 45 + 30]);
            }
            let w = vec![1.0; pts.len()];
            dec.decode(&pts, &w, r, &mut out);
            assert_eq!(out, pdu, "rate {idx}");
        }
    }

    #[test]
    fn the_scrambler_restarts_every_120_symbols() {
        let mut s = Scrambler::default();
        let a: Vec<bool> = (0..120).map(|_| s.inverted()).collect();
        let b: Vec<bool> = (0..120).map(|_| s.inverted()).collect();
        assert_eq!(a, b);
    }
}
