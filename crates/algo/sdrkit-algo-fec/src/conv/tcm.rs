//! The 4-level trellis codes of DMR and P25: rate 3/4 (8 states, tribits in) and rate 1/2 (4 states, dibits in). A code word
//! is 49 constellation points (48 carrying information and a last one that returns the encoder to state 0), each point a pair
//! of 4-level symbols, sent as 98 interleaved dibits (196 bits).
//!
//! - [`TcmCode`] is the code as the standard prints it: for each state and input, the constellation point sent; the next state
//!   is the input.
//! - [`POINT_SYMBOLS`] maps a constellation point to its two symbols, [`symbol_dibit`] a symbol to its dibit, and [`INTERLEAVE`]
//!   is the order in which the 98 dibits are sent.
//! - [`TcmCodec`] encodes 144 (rate 3/4) or 96 (rate 1/2) bits into the 196 bits sent, and decodes them back with the
//!   [`TrellisDecoder`]: from hard bits (Hamming distance per point) or from the received symbol levels (squared distance, soft).
//!
//! Sources: ETSI TS 102 361-1 V2.7.1 clause B.2.4 (table B.7, the rate 3/4 state transitions; table B.8, constellation point to
//! dibit pair; table B.9, the interleave schedule) and clause 10.2 (dibit to symbol). The P25 tables (TIA-102.BAAA) are not
//! freely available. [`P25_3_4`] and [`P25_1_2`] follow the DMR tables: the rate 3/4 code is the one DMR adopted, and both share the DMR
//! constellation and interleave.
//!
//! ```
//! use sdrkit_algo_fec::conv::tcm::{TcmCodec, DMR_3_4};
//! let mut codec = TcmCodec::new(DMR_3_4).unwrap();
//! let info: Vec<u8> = (0..144).map(|i| (i % 3 == 0) as u8).collect();
//! let mut sent = [0u8; 196];
//! codec.encode(&info, &mut sent).unwrap();
//! sent[10] ^= 1;
//! sent[150] ^= 1;
//! let mut got = [0u8; 144];
//! assert_eq!(codec.decode_bits(&sent, &mut got).unwrap().corrected, 2);
//! assert_eq!(got[..], info[..]);
//! ```

use super::{Ends, LabelMetrics, Trellis, TrellisDecoder};
use crate::error::{Decoded, FecError, Param, Result};

/// Constellation points per code word (48 carrying information, the last returning the encoder to state 0).
pub const POINTS: usize = 49;
/// Dibits per code word.
pub const DIBITS: usize = 98;
/// Bits sent per code word.
pub const CODED_BITS: usize = 196;

/// Squared-distance units per unit of symbol level squared, in the soft metrics.
const SOFT_SCALE: f32 = 64.0;
/// Received levels are clipped to this magnitude before the soft metrics (well outside ±3, keeps the sums in range).
const LEVEL_CLIP: f32 = 16.0;

/// A 4-level trellis code: `bits` information bits per point (3 or 2), `2^bits` states, and `table[state * 2^bits + input]`
/// the constellation point (0 to 15) sent for `input` in `state`. The next state is the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcmCode {
    /// The stable snake-case name (`"dmr_3_4"`).
    pub key: &'static str,
    /// Information bits per constellation point.
    pub bits: u8,
    /// The constellation point for each (state, input), the state major.
    pub table: &'static [u8],
}

/// The DMR rate 3/4 trellis code: ETSI TS 102 361-1 V2.7.1 clause B.2.4, table B.7 (the trellis encoder state transition
/// table: rows the state, columns the input tribit, entries the constellation point).
pub const DMR_3_4: TcmCode = TcmCode {
    key: "dmr_3_4",
    bits: 3,
    table: &RATE_3_4,
};

/// The P25 rate 3/4 trellis code (confirmed data), TIA-102.BAAA: the same table as [`DMR_3_4`].
pub const P25_3_4: TcmCode = TcmCode {
    key: "p25_3_4",
    bits: 3,
    table: &RATE_3_4,
};

/// The P25 rate 1/2 trellis code (trunking signalling blocks, data headers, unconfirmed data), TIA-102.BAAA.
pub const P25_1_2: TcmCode = TcmCode {
    key: "p25_1_2",
    bits: 2,
    table: &RATE_1_2,
};

/// Every named code.
pub const ALL: &[TcmCode] = &[DMR_3_4, P25_3_4, P25_1_2];

/// The code named `key`, or `None`.
pub fn by_key(key: &str) -> Option<&'static TcmCode> {
    ALL.iter().find(|c| c.key == key)
}

#[rustfmt::skip]
const RATE_3_4: [u8; 64] = [
    0, 8, 4, 12, 2, 10, 6, 14,
    4, 12, 2, 10, 6, 14, 0, 8,
    1, 9, 5, 13, 3, 11, 7, 15,
    5, 13, 3, 11, 7, 15, 1, 9,
    3, 11, 7, 15, 1, 9, 5, 13,
    7, 15, 1, 9, 5, 13, 3, 11,
    2, 10, 6, 14, 0, 8, 4, 12,
    6, 14, 0, 8, 4, 12, 2, 10,
];

#[rustfmt::skip]
const RATE_1_2: [u8; 16] = [
    0, 15, 12, 3,
    4, 11, 8, 7,
    13, 2, 1, 14,
    9, 6, 5, 10,
];

/// Constellation point to its pair of symbols, the first sent first (TS 102 361-1 table B.8).
#[rustfmt::skip]
pub const POINT_SYMBOLS: [[i8; 2]; 16] = [
    [1, -1], [-1, -1], [3, -3], [-3, -3], [-3, -1], [3, -1], [-1, -3], [1, -3],
    [-3, 3], [3, 3], [-1, 1], [1, 1], [1, 3], [-1, 3], [3, 1], [-3, 1],
];

/// The interleave schedule (TS 102 361-1 table B.9): the dibit sent `i`-th is the coded dibit `INTERLEAVE[i]` (coded dibits
/// numbered in encoder order, two per point). The coded dibit pairs are sent in four rounds, pair `p` in round `p mod 4`.
pub const INTERLEAVE: [u8; DIBITS] = interleave();

const fn interleave() -> [u8; DIBITS] {
    let mut t = [0u8; DIBITS];
    let mut i = 0;
    let mut round = 0;
    while round < 4 {
        let mut p = round;
        while p < POINTS {
            t[i] = (2 * p) as u8;
            t[i + 1] = (2 * p + 1) as u8;
            i += 2;
            p += 4;
        }
        round += 1;
    }
    t
}

/// The dibit of a 4-level symbol (TS 102 361-1 clause 10.2: +3 is 01, +1 is 00, −1 is 10, −3 is 11). Returns `None` for any
/// other value.
pub const fn symbol_dibit(s: i8) -> Option<u8> {
    match s {
        3 => Some(0b01),
        1 => Some(0b00),
        -1 => Some(0b10),
        -3 => Some(0b11),
        _ => None,
    }
}

/// The 4-level symbol of a dibit (the low two bits of `d`; see [`symbol_dibit`]).
pub const fn dibit_symbol(d: u8) -> i8 {
    match d & 3 {
        0b01 => 3,
        0b00 => 1,
        0b10 => -1,
        _ => -3,
    }
}

/// The four bits of a point's two dibits, the first dibit in the high bits.
const fn point_nibble(p: usize) -> u8 {
    let [a, b] = POINT_SYMBOLS[p];
    let (Some(a), Some(b)) = (symbol_dibit(a), symbol_dibit(b)) else {
        return 0;
    };
    (a << 2) | b
}

const POINT_NIBBLES: [u8; 16] = {
    let mut t = [0u8; 16];
    let mut p = 0;
    while p < 16 {
        t[p] = point_nibble(p);
        p += 1;
    }
    t
};

impl TcmCode {
    /// States (and inputs per point): 8 for rate 3/4, 4 for rate 1/2.
    pub const fn states(&self) -> usize {
        1 << self.bits
    }

    /// Information bits per code word: 144 for rate 3/4, 96 for rate 1/2.
    pub const fn info_bits(&self) -> usize {
        (POINTS - 1) * self.bits as usize
    }

    /// The constellation point sent for `input` in `state` (both below [`TcmCode::states`]).
    pub fn point(&self, state: usize, input: usize) -> Option<u8> {
        let s = self.states();
        (state < s && input < s).then(|| self.table[state * s + input])
    }

    /// The code as a [`Trellis`] (labels are the constellation points).
    pub fn trellis(&self) -> Result<Trellis> {
        let s = self.states();
        if self.table.len() != s * s || self.table.iter().any(|&p| p > 15) {
            return Param::Table.conflict(Param::Bits);
        }
        let rows: [&[u8]; 8] = core::array::from_fn(|r| {
            if r < s {
                &self.table[r * s..(r + 1) * s]
            } else {
                &[]
            }
        });
        let t = Trellis::last_input(&rows[..s])?;
        // `last_input` counts labels up to the largest point used; every code here uses all 16.
        if t.labels() != 16 {
            return Param::Table.degenerate();
        }
        Ok(t)
    }
}

/// Encoder and decoder of one [`TcmCode`], sized at construction; no allocation after.
#[derive(Clone, Debug)]
pub struct TcmCodec {
    code: TcmCode,
    dec: TrellisDecoder,
    /// Branch metrics, 16 per point.
    metrics: Vec<i32>,
    inputs: [u8; POINTS],
    /// The received dibits in encoder order, and the re-encoded ones.
    rx: [u8; DIBITS],
    again: [u8; DIBITS],
}

impl TcmCodec {
    /// A codec for `code`. An error when its table is malformed.
    pub fn new(code: TcmCode) -> Result<Self> {
        let t = code.trellis()?;
        Ok(TcmCodec {
            code,
            dec: TrellisDecoder::new(&t, POINTS)?,
            metrics: vec![0; POINTS * 16],
            inputs: [0; POINTS],
            rx: [0; DIBITS],
            again: [0; DIBITS],
        })
    }

    /// The code.
    pub fn code(&self) -> &TcmCode {
        &self.code
    }

    /// The coded dibits (encoder order) of `inputs` (one per point, the last 0), from state 0.
    fn encode_dibits(code: &TcmCode, inputs: &[u8; POINTS], out: &mut [u8; DIBITS]) {
        let s = code.states();
        let mut state = 0usize;
        for (p, &x) in inputs.iter().enumerate() {
            let n = POINT_NIBBLES[code.table[state * s + x as usize] as usize];
            out[2 * p] = n >> 2;
            out[2 * p + 1] = n & 3;
            state = x as usize;
        }
    }

    /// Encodes [`TcmCode::info_bits`] bits (`u8` 0 or 1, the first sent first; each point's input is its bits read most
    /// significant first) into the [`CODED_BITS`] bits sent, interleaved, each dibit's high bit first.
    pub fn encode(&mut self, info: &[u8], out: &mut [u8]) -> Result<()> {
        let w = self.code.bits as usize;
        if info.len() != self.code.info_bits() {
            return Err(FecError::InputLength {
                expected: self.code.info_bits(),
                got: info.len(),
            });
        }
        if out.len() < CODED_BITS {
            return Err(FecError::OutputTooShort { need: CODED_BITS });
        }
        for (x, c) in self.inputs.iter_mut().zip(info.chunks_exact(w)) {
            *x = c.iter().fold(0u8, |a, &b| (a << 1) | (b & 1));
        }
        self.inputs[POINTS - 1] = 0;
        Self::encode_dibits(&self.code, &self.inputs, &mut self.again);
        for (i, &src) in INTERLEAVE.iter().enumerate() {
            let d = self.again[src as usize];
            out[2 * i] = d >> 1;
            out[2 * i + 1] = d & 1;
        }
        Ok(())
    }

    /// Decodes the [`CODED_BITS`] hard bits received (as sent by [`TcmCodec::encode`]) into [`TcmCode::info_bits`] bits.
    /// `corrected` is the number of received bits that differ from the decoded code word; `metric` is minus that count.
    pub fn decode_bits(&mut self, bits: &[u8], info: &mut [u8]) -> Result<Decoded> {
        if bits.len() != CODED_BITS {
            return Err(FecError::InputLength {
                expected: CODED_BITS,
                got: bits.len(),
            });
        }
        for (i, &dst) in INTERLEAVE.iter().enumerate() {
            self.rx[dst as usize] = ((bits[2 * i] & 1) << 1) | (bits[2 * i + 1] & 1);
        }
        for p in 0..POINTS {
            let r = (self.rx[2 * p] << 2) | self.rx[2 * p + 1];
            for (m, &n) in self.metrics[p * 16..(p + 1) * 16]
                .iter_mut()
                .zip(&POINT_NIBBLES)
            {
                *m = -((n ^ r).count_ones() as i32);
            }
        }
        self.run(info)
    }

    /// Decodes the [`DIBITS`] symbol levels received, in the order sent (nominally ±1 and ±3; scale the input to that), into
    /// [`TcmCode::info_bits`] bits by squared distance. `corrected` is the number of bits of the nearest-symbol decisions that
    /// differ from the decoded code word. An error when a level is NaN or infinite.
    pub fn decode_levels(&mut self, levels: &[f32], info: &mut [u8]) -> Result<Decoded> {
        if levels.len() != DIBITS {
            return Err(FecError::InputLength {
                expected: DIBITS,
                got: levels.len(),
            });
        }
        if let Some(at) = levels.iter().position(|v| !v.is_finite()) {
            return Err(FecError::NotFinite { at });
        }
        // Levels in encoder order, and the hard dibits for the error count.
        let mut ordered = [0f32; DIBITS];
        for (i, &dst) in INTERLEAVE.iter().enumerate() {
            let r = levels[i].clamp(-LEVEL_CLIP, LEVEL_CLIP);
            ordered[dst as usize] = r;
            let nearest = if r >= 2.0 {
                3
            } else if r >= 0.0 {
                1
            } else if r >= -2.0 {
                -1
            } else {
                -3
            };
            self.rx[dst as usize] = symbol_dibit(nearest).unwrap_or(0);
        }
        for p in 0..POINTS {
            let (r0, r1) = (ordered[2 * p], ordered[2 * p + 1]);
            for (m, s) in self.metrics[p * 16..(p + 1) * 16]
                .iter_mut()
                .zip(&POINT_SYMBOLS)
            {
                let (d0, d1) = (r0 - s[0] as f32, r1 - s[1] as f32);
                *m = -((d0 * d0 + d1 * d1) * SOFT_SCALE) as i32;
            }
        }
        self.run(info)
    }

    /// Runs the trellis decoder over `self.metrics` and compares the decoded word with `self.rx`.
    fn run(&mut self, info: &mut [u8]) -> Result<Decoded> {
        let need = self.code.info_bits();
        if info.len() < need {
            return Err(FecError::OutputTooShort { need });
        }
        let mut bm = LabelMetrics {
            labels: 16,
            metrics: &self.metrics,
        };
        self.dec
            .decode(&mut bm, POINTS, Ends::Zero, None, &mut self.inputs)?;
        let w = self.code.bits as usize;
        for (c, &x) in info[..need].chunks_exact_mut(w).zip(&self.inputs) {
            for (k, b) in c.iter_mut().enumerate() {
                *b = (x >> (w - 1 - k)) & 1;
            }
        }
        Self::encode_dibits(&self.code, &self.inputs, &mut self.again);
        let errors: u32 = self
            .rx
            .iter()
            .zip(&self.again)
            .map(|(&a, &b)| (a ^ b).count_ones())
            .sum();
        Ok(Decoded {
            corrected: errors,
            metric: -(errors as i32),
            iterations: 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_b7_rate_3_4_as_printed() {
        // TS 102 361-1 table B.7: one row per state, one column per input tribit.
        let printed: [[u8; 8]; 8] = [
            [0, 8, 4, 12, 2, 10, 6, 14],
            [4, 12, 2, 10, 6, 14, 0, 8],
            [1, 9, 5, 13, 3, 11, 7, 15],
            [5, 13, 3, 11, 7, 15, 1, 9],
            [3, 11, 7, 15, 1, 9, 5, 13],
            [7, 15, 1, 9, 5, 13, 3, 11],
            [2, 10, 6, 14, 0, 8, 4, 12],
            [6, 14, 0, 8, 4, 12, 2, 10],
        ];
        for (s, row) in printed.iter().enumerate() {
            for (x, &p) in row.iter().enumerate() {
                assert_eq!(DMR_3_4.point(s, x), Some(p), "state {s} input {x}");
                assert_eq!(P25_3_4.point(s, x), Some(p));
            }
        }
        assert_eq!(DMR_3_4.point(8, 0), None);
        let printed_12: [[u8; 4]; 4] =
            [[0, 15, 12, 3], [4, 11, 8, 7], [13, 2, 1, 14], [9, 6, 5, 10]];
        for (s, row) in printed_12.iter().enumerate() {
            for (x, &p) in row.iter().enumerate() {
                assert_eq!(P25_1_2.point(s, x), Some(p));
            }
        }
    }

    #[test]
    fn table_b8_points_and_dibits_as_printed() {
        // TS 102 361-1 table B.8: constellation point to dibit pair (the symbols printed there, as dibits by clause 10.2).
        let printed: [(u8, u8); 16] = [
            (0b00, 0b10),
            (0b10, 0b10),
            (0b01, 0b11),
            (0b11, 0b11),
            (0b11, 0b10),
            (0b01, 0b10),
            (0b10, 0b11),
            (0b00, 0b11),
            (0b11, 0b01),
            (0b01, 0b01),
            (0b10, 0b00),
            (0b00, 0b00),
            (0b00, 0b01),
            (0b10, 0b01),
            (0b01, 0b00),
            (0b11, 0b00),
        ];
        for (p, &(a, b)) in printed.iter().enumerate() {
            assert_eq!(POINT_NIBBLES[p], (a << 2) | b, "point {p}");
            assert_eq!(dibit_symbol(a), POINT_SYMBOLS[p][0]);
            assert_eq!(dibit_symbol(b), POINT_SYMBOLS[p][1]);
        }
        for d in 0..4 {
            assert_eq!(symbol_dibit(dibit_symbol(d)), Some(d));
        }
        assert_eq!(symbol_dibit(2), None);
    }

    #[test]
    fn table_b9_interleave_as_printed() {
        // TS 102 361-1 table B.9, in the order sent.
        #[rustfmt::skip]
        let printed: [u8; 98] = [
            0, 1, 8, 9, 16, 17, 24, 25, 32, 33, 40, 41, 48, 49, 56, 57, 64, 65, 72, 73, 80, 81, 88, 89, 96, 97,
            2, 3, 10, 11, 18, 19, 26, 27, 34, 35, 42, 43, 50, 51, 58, 59, 66, 67, 74, 75, 82, 83, 90, 91,
            4, 5, 12, 13, 20, 21, 28, 29, 36, 37, 44, 45, 52, 53, 60, 61, 68, 69, 76, 77, 84, 85, 92, 93,
            6, 7, 14, 15, 22, 23, 30, 31, 38, 39, 46, 47, 54, 55, 62, 63, 70, 71, 78, 79, 86, 87, 94, 95,
        ];
        assert_eq!(INTERLEAVE, printed);
    }

    #[test]
    fn a_known_word_from_the_tables() {
        // Inputs 1, 2, 3, 0 ... from state 0: points B.7[0][1] = 8, B.7[1][2] = 2, B.7[2][3] = 13, B.7[3][0] = 5, then 0s.
        let mut codec = TcmCodec::new(DMR_3_4).unwrap();
        let mut info = [0u8; 144];
        info[..12].copy_from_slice(&[0, 0, 1, 0, 1, 0, 0, 1, 1, 0, 0, 0]);
        let mut sent = [0u8; 196];
        codec.encode(&info, &mut sent).unwrap();
        // Points 8, 2, 13, 5 are dibits (11 01), (01 11), (10 01), (01 10); point 0 is (00 10). Coded dibits 0, 1 are sent first,
        // coded dibits 2, 3 at positions 26, 27, 4, 5 at 50, 51, 6, 7 at 74, 75, 8, 9 at 2, 3.
        let dibit = |i: usize| (sent[2 * i] << 1) | sent[2 * i + 1];
        assert_eq!([dibit(0), dibit(1)], [0b11, 0b01]);
        assert_eq!([dibit(26), dibit(27)], [0b01, 0b11]);
        assert_eq!([dibit(50), dibit(51)], [0b10, 0b01]);
        assert_eq!([dibit(74), dibit(75)], [0b01, 0b10]);
        assert_eq!([dibit(2), dibit(3)], [0b00, 0b10]);
        let mut got = [0u8; 144];
        assert_eq!(codec.decode_bits(&sent, &mut got).unwrap().corrected, 0);
        assert_eq!(got, info);
    }

    fn pattern(n: usize, seed: usize) -> Vec<u8> {
        (0..n).map(|i| ((i * 7 + seed) % 5 < 2) as u8).collect()
    }

    #[test]
    fn every_code_corrects_errors_from_bits_and_levels() {
        for code in ALL {
            let mut codec = TcmCodec::new(*code).unwrap();
            let info = pattern(code.info_bits(), code.bits as usize);
            let mut sent = [0u8; 196];
            codec.encode(&info, &mut sent).unwrap();
            let mut got = vec![0u8; code.info_bits()];
            let mut rx = sent;
            for i in [5, 60, 131] {
                rx[i] ^= 1;
            }
            let d = codec.decode_bits(&rx, &mut got).unwrap();
            assert_eq!(got, info, "{}", code.key);
            assert_eq!(d.corrected, 3);
            // Levels with noise large enough to make hard-decision errors.
            let mut levels: Vec<f32> = sent
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| dibit_symbol((c[0] << 1) | c[1]) as f32)
                .collect();
            for (i, v) in levels.iter_mut().enumerate() {
                *v += 0.8 * (((i * 37) % 11) as f32 / 10.0 - 0.5);
            }
            // Three symbols pushed past the decision boundary towards their neighbour.
            for i in [10, 40, 80] {
                levels[i] -= 1.3 * levels[i].signum();
            }
            let d = codec.decode_levels(&levels, &mut got).unwrap();
            assert_eq!(got, info, "{} soft", code.key);
            assert!(d.corrected > 0);
        }
    }

    #[test]
    fn bad_input_is_an_error() {
        let mut codec = TcmCodec::new(P25_1_2).unwrap();
        let mut out = [0u8; 196];
        assert!(codec.encode(&[0u8; 95], &mut out).is_err());
        assert!(codec.encode(&[0u8; 96], &mut out[..195]).is_err());
        let mut info = [0u8; 96];
        assert!(codec.decode_bits(&out[..195], &mut info).is_err());
        assert!(codec.decode_bits(&out, &mut info[..95]).is_err());
        let mut lv = [1.0f32; 98];
        lv[7] = f32::NAN;
        assert_eq!(
            codec.decode_levels(&lv, &mut info),
            Err(FecError::NotFinite { at: 7 })
        );
        assert!(TcmCodec::new(TcmCode {
            key: "x",
            bits: 2,
            table: &[0; 15]
        })
        .is_err());
        assert!(TcmCodec::new(TcmCode {
            key: "x",
            bits: 2,
            table: &[0; 16]
        })
        .is_err());
        assert_eq!(by_key("p25_1_2"), Some(&P25_1_2));
        assert_eq!(by_key("atsc"), None);
    }
}
