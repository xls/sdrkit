//! The TD-SCDMA test signal: synchronised cells at 2 samples a chip with a root raised cosine pulse. Each sub-frame of a cell
//! holds time slot 0 (the two P-CCPCH codes with random QPSK, scrambled with the cell parameter in use, and midambles m(1) and
//! m(2)), the SYNC-DL turned by the phase of its quadruple, and time slots 4 to 6 with random chips and midamble m(3). The cell
//! parameter in use is the initial one in frames of even SFN and its pair partner in odd frames; quadruples start at even
//! SFN. The signal has unit mean power.

use super::codes::{
    basic_midamble, channelisation, midamble, scrambling, slot_start, sync_dl, K_TS0, MIDAMBLE_START,
    S1, S2, SUBFRAME_CHIPS, SYNC_DL_START,
};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::{self, Norm, Shape};
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, Generator, Hz, SignalInfo};

/// The chip rate, chips per second.
pub const CHIP_RATE: f64 = 1.28e6;
/// The native rate of the generator and the working rate of the receiver: two samples a chip.
pub const NATIVE_RATE: f64 = 2.0 * CHIP_RATE;
/// The roll-off of the pulse (TS 25.105, not confirmed against the text).
pub const ROLL_OFF: f64 = 0.22;

/// One cell of the test signal.
#[derive(Clone, Debug, PartialEq)]
pub struct TdCell {
    /// The initial cell parameter (0 to 127).
    pub param: usize,
    /// The SFN of the frame that starts at the cell's first sub-frame.
    pub sfn: i64,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// The amplitude.
    pub amp: f32,
    /// The chip of the stream where the cell's first sub-frame starts (may be negative).
    pub chip_offset: i64,
    /// Every second quadruple is S2 (no P-CCPCH), else every quadruple is S1.
    pub s2_alternate: bool,
}

/// The cells of the stack's own test signal: parameters 37 and 90, synchronised, SFN 100 at the start.
pub fn scene() -> Vec<TdCell> {
    vec![
        TdCell {
            param: 37,
            sfn: 100,
            offset_hz: 0.0,
            amp: 1.0,
            chip_offset: 1200,
            s2_alternate: false,
        },
        TdCell {
            param: 90,
            sfn: 100,
            offset_hz: 90.0,
            amp: 0.5,
            chip_offset: 1205,
            s2_alternate: false,
        },
    ]
}

/// The chips of one cell, made a sub-frame at a time.
struct CellChips {
    cfg: TdCell,
    rng: Rng,
    /// The sub-frame held in `chips`, as a count from the cell's first.
    held: Option<i64>,
    chips: Vec<Complex32>,
    /// The SYNC-DL is written with `(-j)^i` instead of `j^i`.
    conjugate_sync: bool,
    /// The P-CCPCH codes carry their complex multipliers `w(16, 1) = -1` and `w(16, 2) = -j`.
    multipliers: bool,
}

impl CellChips {
    /// Writes sub-frame `sf` of the cell into `chips`.
    fn make(&mut self, sf: i64) {
        let c = &self.cfg;
        let frame = c.sfn + sf.div_euclid(2);
        let sub = sf.rem_euclid(2) as usize;
        let p = if frame.rem_euclid(2) == 0 { c.param } else { c.param ^ 1 };
        let quad = frame.div_euclid(2);
        let pos = 2 * frame.rem_euclid(2) as usize + sub;
        let phases = if c.s2_alternate && quad.rem_euclid(2) == 1 { S2 } else { S1 };
        let chips = &mut self.chips;
        chips.clear();
        chips.resize(SUBFRAME_CHIPS, Complex32::default());
        let basic = basic_midamble(p);
        let v = scrambling(p);
        let qpsk = |rng: &mut Rng| {
            let q = rng.next_u64();
            Complex32::new(
                if q & 1 == 0 { 1.0 } else { -1.0 },
                if q & 2 == 0 { 1.0 } else { -1.0 },
            ) * std::f32::consts::FRAC_1_SQRT_2
        };
        // Time slot 0: P-CCPCH1 and P-CCPCH2, SF 16, 22 symbols in each half burst.
        for code in 1..=2 {
            let cc = channelisation(code);
            let w = match (self.multipliers, code) {
                (false, _) => Complex32::new(1.0, 0.0),
                (true, 1) => Complex32::new(-1.0, 0.0),
                (true, _) => Complex32::new(0.0, -1.0),
            };
            for half in [0usize, MIDAMBLE_START + 144] {
                for s in 0..22 {
                    let d = qpsk(&mut self.rng) * 0.5 * w;
                    for n in 0..16 {
                        chips[half + 16 * s + n] += d * cc[n] * v[n];
                    }
                }
            }
        }
        for k in [1, 2] {
            for (i, m) in midamble(&basic, k, K_TS0).into_iter().enumerate() {
                chips[MIDAMBLE_START + i] += m * std::f32::consts::FRAC_1_SQRT_2;
            }
        }
        // DwPTS: the SYNC-DL turned by the phase of this sub-frame's place in its quadruple.
        let rot = Complex32::from_polar(1.0, phases[pos].to_radians() as f32);
        let mut code = sync_dl(super::codes::group(c.param));
        if self.conjugate_sync {
            code.iter_mut().for_each(|x| *x = x.conj());
        }
        for (i, s) in code.into_iter().enumerate() {
            chips[SYNC_DL_START + i] = s * rot;
        }
        // Time slots 4 to 6: random chips and midamble m(3).
        let m3 = midamble(&basic, 3, K_TS0);
        for ts in 4..=6 {
            let s0 = slot_start(ts);
            for n in (0..352).chain(MIDAMBLE_START + 144..MIDAMBLE_START + 496) {
                chips[s0 + n] = qpsk(&mut self.rng);
            }
            for (i, m) in m3.iter().enumerate() {
                chips[s0 + MIDAMBLE_START + i] = *m;
            }
        }
        self.held = Some(sf);
    }

    /// The chip at stream chip `t`.
    fn chip(&mut self, t: i64) -> Complex32 {
        let u = t - self.cfg.chip_offset;
        let sf = u.div_euclid(SUBFRAME_CHIPS as i64);
        if self.held != Some(sf) {
            self.make(sf);
        }
        self.chips[u.rem_euclid(SUBFRAME_CHIPS as i64) as usize] * self.cfg.amp
    }
}

/// The generator.
pub struct TdGenerator {
    rate: f64,
    cells: Vec<(CellChips, Nco)>,
    taps: Vec<f32>,
    /// The chip of the stream that the next block starts at.
    t: i64,
    /// The last `taps` samples of the zero-stuffed chips, for the filter.
    hist: Vec<Complex32>,
    native: Vec<Complex32>,
    scale: f32,
    resampler: Option<FractionalResampler<Complex32>>,
    out: Vec<Complex32>,
    pos: usize,
}

impl TdGenerator {
    /// The `cells` at output rate `rate`, from `seed`. With `conjugate_sync` the SYNC-DL codes are written with `(-j)^i`, the
    /// conjugate of the standard's code (a guard test of the receiver).
    pub fn new(rate: f64, cells: Vec<TdCell>, seed: u64, conjugate_sync: bool) -> Self {
        let mut g = Self::build(rate, cells.clone(), seed, conjugate_sync, 1.0);
        // Unit mean power: the power of the first 80 ms of a copy of the same signal.
        let mut probe = Self::build(NATIVE_RATE, cells, seed, conjugate_sync, 1.0);
        let mut x = vec![Complex32::default(); (0.08 * NATIVE_RATE) as usize];
        probe.generate(&mut x);
        let p = x.iter().map(|v| v.norm_sqr()).sum::<f32>() / x.len() as f32;
        if p > 0.0 {
            g.scale = p.sqrt().recip();
        }
        g
    }

    fn build(rate: f64, cells: Vec<TdCell>, seed: u64, conjugate_sync: bool, scale: f32) -> Self {
        let taps = pulse::taps(Shape::Rrc { beta: ROLL_OFF }, 2.0, 12.0, Norm::Energy)
            .unwrap_or_else(|_| vec![1.0]);
        let cells = cells
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let nco = Nco::new(NATIVE_RATE, c.offset_hz);
                (
                    CellChips {
                        rng: Rng::new(seed ^ (0x5444_0000 + i as u64)),
                        cfg: c,
                        held: None,
                        chips: Vec::new(),
                        conjugate_sync,
                        multipliers: false,
                    },
                    nco,
                )
            })
            .collect();
        let resampler = ((rate - NATIVE_RATE).abs() > 1e-6)
            .then(|| FractionalResampler::with_rates(NATIVE_RATE, rate).ok())
            .flatten();
        TdGenerator {
            rate,
            cells,
            hist: vec![Complex32::default(); taps.len()],
            taps,
            t: 0,
            native: Vec::new(),
            scale,
            resampler,
            out: Vec::new(),
            pos: 0,
        }
    }

    /// The same generator with a root raised cosine pulse of `roll_off` (unit energy, so the power stays).
    pub fn with_roll_off(mut self, roll_off: f64) -> Self {
        if let Ok(t) = pulse::taps(Shape::Rrc { beta: roll_off }, 2.0, 12.0, Norm::Energy) {
            self.hist = vec![Complex32::default(); t.len()];
            self.taps = t;
        }
        self
    }

    /// The same generator with the complex multipliers of the channelisation codes on the P-CCPCH (`w(16, 1) = -1`,
    /// `w(16, 2) = -j`, values not confirmed against the text of TS 25.223). They change neither the power nor the census.
    pub fn with_multipliers(mut self, on: bool) -> Self {
        for (c, _) in self.cells.iter_mut() {
            c.multipliers = on;
            c.held = None;
        }
        self
    }

    /// Makes one sub-frame of the stream at the native rate into `out`.
    fn refill(&mut self) {
        let n = SUBFRAME_CHIPS;
        let mut sum = vec![Complex32::default(); 2 * n];
        let mut part = vec![Complex32::default(); 2 * n];
        for (cell, nco) in self.cells.iter_mut() {
            for (i, p) in part.iter_mut().enumerate() {
                *p = if i % 2 == 0 {
                    cell.chip(self.t + (i / 2) as i64)
                } else {
                    Complex32::default()
                };
            }
            nco.mix_in_place(&mut part);
            for (s, p) in sum.iter_mut().zip(&part) {
                *s += p;
            }
        }
        self.t += n as i64;
        // The pulse filter over the zero-stuffed chips, its state carried from block to block.
        let l = self.taps.len();
        let mut ext = std::mem::take(&mut self.hist);
        ext.truncate(l - 1);
        ext.extend_from_slice(&sum);
        self.native.clear();
        for i in 0..sum.len() {
            let mut acc = Complex32::default();
            for (k, &h) in self.taps.iter().enumerate() {
                acc += ext[i + l - 1 - k] * h;
            }
            self.native.push(acc * self.scale);
        }
        self.hist = ext[ext.len() - (l - 1)..].to_vec();
        self.out.clear();
        self.pos = 0;
        match self.resampler.as_mut() {
            Some(r) => r.process(&self.native, &mut self.out),
            None => self.out.extend_from_slice(&self.native),
        }
    }
}

impl Generator for TdGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.pos >= self.out.len() {
                self.refill();
                continue;
            }
            let n = (self.out.len() - self.pos).min(out.len() - i);
            out[i..i + n].copy_from_slice(&self.out[self.pos..self.pos + n]);
            self.pos += n;
            i += n;
        }
        out.len()
    }
    /// The SNR of the test signal is measured over the 2.56 MHz of two samples a chip, as the stack's tests state it.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(NATIVE_RATE).with_symbol_rate(CHIP_RATE))
    }
}
