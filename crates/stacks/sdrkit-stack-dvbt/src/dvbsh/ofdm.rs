//! The OFDM form SH-A (ETSI EN 302 583 clause 5.7): the channel widths and their sample rates, the length of an SH frame in OFDM
//! symbols, the DVB-SH fields of TPS (puncturing patterns, interleaver bits, super-frame number), the bit de-multiplexing of
//! clause 5.7.1.2 and the soft demapping of a symbol's cells into one stream's LLRs.

use super::coding::{CU_BITS, IU_BITS, OFDM_FRAME_CU};
use crate::coding::{branches, constellation, symbol_permutation};
use crate::params::{Constellation, Guard, Hierarchy, Mode};
use crate::tps;
use sdrkit_algo_fec::branchdelay::ShTimeInterleaver;
use sdrkit_algo_modem::Demap;
use sdrkit_core::Complex32;

sdrkit_core::choice_enum! {
    /// A channel width of the OFDM form.
    pub enum Width {
        /// 5 MHz.
        W5 = ("5", "5 MHz"),
        /// 8 MHz.
        W8 = ("8", "8 MHz"),
        /// 7 MHz.
        W7 = ("7", "7 MHz"),
        /// 6 MHz.
        W6 = ("6", "6 MHz"),
        /// 1.7 MHz (1K only).
        W1_7 = ("1.7", "1.7 MHz"),
    }
}

impl Width {
    /// Every width, in the order of the automatic search (clause 4.7.1 of the stack's specification): 5, 8, 7, 6, 1.7 MHz.
    pub const SEARCH: [Width; 5] = [Width::W5, Width::W8, Width::W7, Width::W6, Width::W1_7];

    /// The OFDM sample rate 1/T (clause 5.7.4.1): 64/7, 8, 48/7, 40/7 and 64/35 MHz.
    pub fn ofdm_rate(self) -> f64 {
        match self {
            Width::W8 => 64e6 / 7.0,
            Width::W7 => 8e6,
            Width::W6 => 48e6 / 7.0,
            Width::W5 => 40e6 / 7.0,
            Width::W1_7 => 64e6 / 35.0,
        }
    }

    /// The width in MHz.
    pub fn mhz(self) -> f64 {
        match self {
            Width::W8 => 8.0,
            Width::W7 => 7.0,
            Width::W6 => 6.0,
            Width::W5 => 5.0,
            Width::W1_7 => 1.7,
        }
    }

    /// Whether the width can be received from samples at `rate`: its OFDM rate is at most 1.02 times the input rate.
    pub fn usable(self, rate: f64) -> bool {
        self.ofdm_rate() <= 1.02 * rate
    }
}

/// The parameters of an SH-A signal that TPS carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfdmParams {
    /// FFT size.
    pub mode: Mode,
    /// Guard interval.
    pub guard: Guard,
    /// QPSK or 16-QAM.
    pub constellation: Constellation,
    /// Hierarchy.
    pub hierarchy: Hierarchy,
    /// Punct_Pat_ID of the high-priority stream (the only stream when not hierarchical).
    pub hp: u8,
    /// Punct_Pat_ID of the low-priority stream (0 when not hierarchical).
    pub lp: u8,
    /// The time interleaver profile.
    pub ti: ShTimeInterleaver,
    /// The cell id.
    pub cell_id: u16,
}

impl Default for OfdmParams {
    /// 2K, guard 1/8, QPSK, pattern 4 (1/3), the uniform interleaver 4 / 48 / 1 / 0 / 0.
    fn default() -> Self {
        OfdmParams {
            mode: Mode::K2,
            guard: Guard::G8,
            constellation: Constellation::Qpsk,
            hierarchy: Hierarchy::None,
            hp: 4,
            lp: 0,
            ti: ShTimeInterleaver::default(),
            cell_id: 0,
        }
    }
}

/// The SH frame capacity of the OFDM form in interleaving units (816 CU).
pub const FRAME_IU: usize = OFDM_FRAME_CU * CU_BITS / IU_BITS;
/// The unit of the slice offset of the OFDM form: the SH frame capacity in units over 48.
pub const SLICE_UNIT: u32 = (FRAME_IU / 48) as u32;

/// The bits of one stream in one OFDM symbol: 2 per cell for QPSK and each stream of hierarchical 16-QAM, 4 for 16-QAM.
pub fn stream_bits(mode: Mode, c: Constellation, h: Hierarchy) -> usize {
    let per = if c == Constellation::Qpsk || h.hierarchical() {
        2
    } else {
        c.bits()
    };
    mode.data_carriers() * per
}

/// The OFDM symbols of one SH frame (table 5.10: 1/4 to 16 OFDM frames).
pub fn sh_frame_symbols(mode: Mode, c: Constellation, h: Hierarchy) -> usize {
    OFDM_FRAME_CU * CU_BITS / stream_bits(mode, c, h)
}

fn put(s: &mut [u8; 68], at: usize, n: usize, v: u32) {
    for k in 0..n {
        s[at + k] = ((v >> (n - 1 - k)) & 1) as u8;
    }
}

/// Interleaver bit `n` (1 to 32, B1 the most significant) of a profile.
fn b(ti: u32, n: u32) -> u32 {
    (ti >> (32 - n)) & 1
}

/// The 68 TPS bits of frame `frame` (0 to 3) of super-frame number `superframe` in its SH frame (clause 5.7.4.3, s53 = 1).
pub fn tps_bits(p: &OfdmParams, frame: u8, superframe: u8) -> [u8; 68] {
    let mut s = [0u8; 68];
    let sync = if frame.is_multiple_of(2) {
        tps::SYNC_ODD
    } else {
        !tps::SYNC_ODD
    };
    put(&mut s, 1, 16, sync as u32);
    put(&mut s, 17, 6, 0b100101);
    put(&mut s, 23, 2, frame as u32 & 3);
    put(
        &mut s,
        25,
        2,
        (p.constellation == Constellation::Qam16) as u32,
    );
    put(&mut s, 27, 3, p.hierarchy as u32);
    let ti = p.ti.to_bits();
    // s30-s33, s34 and s48-s52 of each frame (figure 5.36).
    let (s30, first) = match frame & 3 {
        0 => (p.hp as u32, 1),
        1 => (
            b(ti, 7) << 3 | b(ti, 8) << 2 | b(ti, 9) << 1 | b(ti, 10),
            11,
        ),
        2 => (
            if p.hierarchy.hierarchical() {
                p.lp as u32
            } else {
                0
            },
            17,
        ),
        _ => (
            b(ti, 23) << 3 | b(ti, 24) << 2 | b(ti, 25) << 1 | b(ti, 26),
            27,
        ),
    };
    put(&mut s, 30, 4, s30);
    s[34] = b(ti, first) as u8;
    for k in 0..5 {
        s[48 + k] = b(ti, first + 1 + k as u32) as u8;
    }
    let sf = if frame.is_multiple_of(2) {
        superframe >> 1
    } else {
        superframe
    };
    s[35] = sf & 1;
    put(&mut s, 36, 2, p.guard.code() as u32);
    put(&mut s, 38, 2, p.mode as u32);
    let cell = if frame.is_multiple_of(2) {
        p.cell_id >> 8
    } else {
        p.cell_id & 0xFF
    };
    put(&mut s, 40, 8, cell as u32);
    s[53] = 1;
    tps::protect(&mut s);
    s
}

/// The DVB-SH fields of one super-frame, gathered from the TPS words of its four frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShFields {
    /// Punct_Pat_ID of the high-priority stream.
    pub hp: u8,
    /// Punct_Pat_ID of the low-priority stream.
    pub lp: u8,
    /// The interleaver bits B1 .. B32.
    pub ti_bits: u32,
    /// The super-frame number in its SH frame (0 to 3).
    pub superframe: u8,
}

/// Gathers the TPS words of the frames of one super-frame. A word of frame 0 starts a new super-frame.
#[derive(Clone, Debug, Default)]
pub struct FieldCollector {
    have: u8,
    f: ShFields,
}

impl FieldCollector {
    /// Takes the word of a decoded frame (bit `i` = s_i). Returns the fields when the frames 0 to 3 of one super-frame have
    /// all been taken, in order.
    pub fn push(&mut self, word: u128) -> Option<ShFields> {
        let s = |i: u32| ((word >> i) & 1) as u32;
        let s30 = s(30) << 3 | s(31) << 2 | s(32) << 1 | s(33);
        let s48 = (48..53).fold(0u32, |a, i| a << 1 | s(i));
        let frame = (s(23) << 1 | s(24)) as u8;
        if frame == 0 {
            self.have = 0;
            self.f = ShFields::default();
        }
        if self.have != frame {
            self.have = 0;
            return None;
        }
        self.have += 1;
        let f = &mut self.f;
        // The six bits of s34 and s48-s52 of each frame land at B1, B11, B17, B27 onwards.
        let six = s(34) << 5 | s48;
        match frame {
            0 => {
                f.hp = s30 as u8;
                f.ti_bits |= six << 26;
                f.superframe = (s(35) << 1) as u8;
            }
            1 => {
                f.ti_bits |= s30 << 22 | six << 16;
                f.superframe |= s(35) as u8;
            }
            2 => {
                f.lp = s30 as u8;
                f.ti_bits |= six << 10;
            }
            _ => {
                f.ti_bits |= s30 << 6 | six;
            }
        }
        (frame == 3).then_some(self.f)
    }
}

/// The soft demapper of SH-A: one symbol's equalised cells to the LLRs of one stream, quantised to 1/4 steps.
pub struct Demapper {
    mode: Mode,
    c: Constellation,
    h: Hierarchy,
    lp: bool,
    cons: sdrkit_algo_modem::Constellation,
    perm: Vec<u16>,
    deint: Vec<(Complex32, f32)>,
}

impl Demapper {
    /// A demapper for the stream (`lp` for the low-priority one) of `mode`, `c` and `h`.
    pub fn new(mode: Mode, c: Constellation, h: Hierarchy, lp: bool) -> Self {
        Demapper {
            mode,
            c,
            h,
            lp: lp && h.hierarchical(),
            cons: constellation(c, h),
            perm: symbol_permutation(mode),
            deint: vec![(Complex32::default(), 1e6); mode.data_carriers()],
        }
    }

    /// Appends the [`stream_bits`] LLRs of symbol `l` (cells and their noise variance per real dimension, in carrier order) to
    /// `out`, 4 per unit of LLR, within plus or minus 127; an erased symbol gives zeros.
    pub fn push(
        &mut self,
        l: usize,
        cells: &[Complex32],
        noise: &[f32],
        erased: bool,
        out: &mut Vec<i8>,
    ) {
        let nd = self.mode.data_carriers();
        let map = branches(self.c, self.h, self.lp);
        if erased || cells.len() < nd || noise.len() < nd {
            out.resize(out.len() + nd * map.len(), 0);
            return;
        }
        let even = l.is_multiple_of(2);
        for q in 0..nd.min(self.perm.len()) {
            let h = self.perm[q] as usize;
            let (dst, src) = if even { (q, h) } else { (h, q) };
            self.deint[dst] = (cells[src], noise[src]);
        }
        let v = self.c.bits();
        if self.c == Constellation::Qpsk {
            // Max-log for QPSK at +-1/sqrt2, as the general demapper scales it: each axis alone, 4 sqrt2 w times the
            // coordinate; y0 on I, y1 on Q.
            for &(z, var) in &self.deint {
                let k = 4.0 * std::f32::consts::SQRT_2 / var.max(1e-12);
                out.push((k * z.re).round().clamp(-127.0, 127.0) as i8);
                out.push((k * z.im).round().clamp(-127.0, 127.0) as i8);
            }
            return;
        }
        let mut cell = [0f32; 6];
        for &(z, var) in &self.deint {
            let weight = 0.5 / var.max(1e-12);
            if self
                .cons
                .demap_weighted(z, weight, Demap::MaxLog, &mut cell[..v])
                .is_err()
            {
                cell[..v].fill(0.0);
            }
            for &e in map {
                out.push((4.0 * cell[e]).round().clamp(-127.0, 127.0) as i8);
            }
        }
    }
}

/// The cells of one OFDM symbol from the stream bits (clause 5.7.1.2 and 5.7.2): `hp` and, for hierarchical 16-QAM, `lp`, each
/// [`stream_bits`] long, are de-multiplexed into words `y`, interleaved by H(q) (even and odd symbols as DVB-T) and mapped.
pub struct Mapper {
    mode: Mode,
    c: Constellation,
    h: Hierarchy,
    points: sdrkit_algo_dsp::constellation::PointSet,
    perm: Vec<u16>,
    words: Vec<u8>,
    y: Vec<u8>,
}

impl Mapper {
    /// A mapper for `mode`, `c` and `h`.
    pub fn new(mode: Mode, c: Constellation, h: Hierarchy) -> Self {
        let nd = mode.data_carriers();
        Mapper {
            mode,
            c,
            h,
            points: crate::coding::point_set(c, h),
            perm: symbol_permutation(mode),
            words: vec![0; nd],
            y: vec![0; nd],
        }
    }

    /// Writes the data cells of symbol `l` in carrier order into `cells`.
    pub fn map(&mut self, l: usize, hp: &[u8], lp: &[u8], cells: &mut Vec<Complex32>) {
        let nd = self.mode.data_carriers();
        let v = self.c.bits();
        self.words.fill(0);
        let hp_map = branches(self.c, self.h, false);
        let per = hp_map.len();
        for (di, &x) in hp.iter().take(nd * per).enumerate() {
            self.words[di / per] |= (x & 1) << hp_map[di % per];
        }
        if self.h.hierarchical() {
            let lp_map = branches(self.c, self.h, true);
            let per = lp_map.len();
            for (di, &x) in lp.iter().take(nd * per).enumerate() {
                self.words[di / per] |= (x & 1) << lp_map[di % per];
            }
        }
        if l.is_multiple_of(2) {
            for q in 0..nd {
                self.y[self.perm[q] as usize] = self.words[q];
            }
        } else {
            for q in 0..nd {
                self.y[q] = self.words[self.perm[q] as usize];
            }
        }
        cells.clear();
        for &w in &self.y {
            // y0 is the most significant bit of the label.
            let label = (0..v).fold(0usize, |a, i| (a << 1) | ((w >> i) & 1) as usize);
            cells.push(self.points.point(label).unwrap_or_default());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &[u8; 68]) -> String {
        let groups = [
            (1, 16),
            (17, 6),
            (23, 2),
            (25, 2),
            (27, 3),
            (30, 4),
            (34, 1),
            (35, 1),
            (36, 2),
            (38, 2),
            (40, 8),
            (48, 5),
            (53, 1),
        ];
        groups
            .iter()
            .map(|&(a, n)| {
                s[a..a + n]
                    .iter()
                    .map(|b| (b'0' + b) as char)
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn the_worked_tps_example() {
        let p = OfdmParams::default();
        let want = [
            "0011010111101110 100101 00 00 000 0100 0 0 10 00 00000000 00100 1",
            "1100101000010001 100101 01 00 000 1100 0 0 10 00 00000000 00000 1",
            "0011010111101110 100101 10 00 000 0000 0 0 10 00 00000000 10000 1",
            "1100101000010001 100101 11 00 000 0000 0 0 10 00 00000000 00000 1",
        ];
        let mut col = FieldCollector::default();
        for f in 0..4u8 {
            let s = tps_bits(&p, f, 0);
            assert_eq!(text(&s), want[f as usize], "frame {}", f + 1);
            let blk = tps::decode_reading(&s, false, true).unwrap();
            assert_eq!(
                (blk.params.mode, blk.params.guard, blk.frame),
                (Mode::K2, Guard::G8, f)
            );
            let got = col.push(blk.word);
            if f == 3 {
                let g = got.unwrap();
                assert_eq!((g.hp, g.lp, g.superframe), (4, 0, 0));
                assert_eq!(ShTimeInterleaver::from_bits(g.ti_bits), p.ti);
            }
        }
        // Super-frame 1: s35 is 1 in frames 2 and 4.
        assert_eq!(tps_bits(&p, 1, 1)[35], 1);
        assert_eq!(tps_bits(&p, 3, 1)[35], 1);
        assert_eq!(tps_bits(&p, 0, 1)[35], 0);
        // A DVB-T frame is refused in the DVB-SH reading.
        let t = tps::encode(&Default::default(), 0);
        assert!(tps::decode_reading(&t, false, true).is_err());
    }

    #[test]
    fn the_qpsk_demapper_equals_the_general_one() {
        let mut d = Demapper::new(Mode::K1, Constellation::Qpsk, Hierarchy::None, false);
        let nd = Mode::K1.data_carriers();
        let cells: Vec<Complex32> = (0..nd)
            .map(|i| {
                Complex32::new(
                    ((i * 37) % 101) as f32 / 50.0 - 1.0,
                    ((i * 53) % 97) as f32 / 48.0 - 1.0,
                )
            })
            .collect();
        let noise: Vec<f32> = (0..nd).map(|i| 0.05 + (i % 7) as f32 * 0.02).collect();
        let mut fast = Vec::new();
        d.push(1, &cells, &noise, false, &mut fast);
        let cons = constellation(Constellation::Qpsk, Hierarchy::None);
        let mut slow = Vec::new();
        for &(z, var) in &d.deint {
            let mut c = [0f32; 2];
            cons.demap_weighted(z, 0.5 / var, Demap::MaxLog, &mut c)
                .unwrap();
            slow.extend(
                c.iter()
                    .map(|&x| (4.0 * x).round().clamp(-127.0, 127.0) as i8),
            );
        }
        let worst = fast
            .iter()
            .zip(&slow)
            .map(|(a, b)| (*a as i32 - *b as i32).abs())
            .max();
        assert!(worst <= Some(1), "{worst:?}");
    }

    #[test]
    fn sh_frame_lengths_of_table_5_10() {
        use Constellation::{Qam16, Qpsk};
        for (m, q, q16) in [
            (Mode::K1, 16, 8),
            (Mode::K2, 8, 4),
            (Mode::K4, 4, 2),
            (Mode::K8, 2, 1),
        ] {
            assert_eq!(sh_frame_symbols(m, Qpsk, Hierarchy::None), 68 * q);
            assert_eq!(sh_frame_symbols(m, Qam16, Hierarchy::A2), 68 * q);
            assert_eq!(sh_frame_symbols(m, Qam16, Hierarchy::None), 68 * q16);
        }
        assert!((Width::W8.ofdm_rate() - 64e6 / 7.0).abs() < 1e-6);
        assert!((Width::W1_7.ofdm_rate() - 64e6 / 35.0).abs() < 1e-6);
    }
}
