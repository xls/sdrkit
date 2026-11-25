//! The Fleet Broadcast receiver: the carrier found from the line of the squared signal, the 1200 Bd symbol clock from the line
//! of the signal energy, differential detection of the bits with a frequency loop, and the 16 columns of the bit stream
//! classed and framed by the repetition of one column.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32;
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// The symbol rate, Bd.
pub const BAUD: f64 = 1200.0;
/// The rate of the front end, about.
pub const WORK_RATE: f64 = 19_200.0;
/// The carrier search range around the dial, Hz.
pub const SEARCH_HZ: f64 = 4500.0;
/// Columns (channels) of a frame.
pub const COLUMNS: usize = 16;
/// The line a carrier needs over the noise, dB.
const LINE_DB: f64 = 17.0;
/// Points of the carrier search.
const FFT_N: usize = 8192;
/// The clock line over the energy that counts as a clock.
const CLOCK_RATIO: f64 = 0.015;
/// Bits analysed at most, and at least.
const ANALYSIS_BITS: usize = 4096;
const ANALYSIS_MIN: usize = 2048;
/// Bits between two analyses.
const ANALYSIS_EVERY: u64 = 1024;
/// Frames kept for the frame log.
pub const FRAME_LOG: usize = 24;

/// The class of a column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Class {
    /// Repeats with a period of 3 to 64 frames.
    Sync,
    /// 1010...
    Reversals,
    /// All ones or all zeros.
    Idle,
    /// Random-looking.
    Random,
    /// Anything else.
    #[default]
    Unknown,
}

impl Class {
    /// The letter of the status text.
    pub fn letter(self) -> char {
        match self {
            Class::Sync => 'S',
            Class::Reversals => 'r',
            Class::Idle => 'i',
            Class::Random => 'e',
            Class::Unknown => '-',
        }
    }
}

/// What the analysis found for one column.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Column {
    /// Its class.
    pub class: Class,
    /// Share of ones, 0 to 1.
    pub ones: f64,
    /// Share of changes from bit to bit, 0 to 1.
    pub activity: f64,
    /// The repeat period in frames (sync only).
    pub period: usize,
    /// The share of bits equal to the bit one period later (sync only).
    pub agreement: f64,
}

/// Classes one column of bits.
pub fn classify(bits: &[u8]) -> Column {
    let n = bits.len().max(1);
    let ones = bits.iter().filter(|&&b| b == 1).count() as f64 / n as f64;
    let changes = bits.windows(2).filter(|w| w[0] != w[1]).count();
    let activity = changes as f64 / (n.saturating_sub(1)).max(1) as f64;
    let mut c = Column {
        ones,
        activity,
        ..Column::default()
    };
    if ones >= 0.9 || ones <= 0.1 {
        c.class = Class::Idle;
        return c;
    }
    if activity >= 0.9 {
        c.class = Class::Reversals;
        return c;
    }
    for p in 3..=64 {
        let m = bits.len().saturating_sub(p);
        if m < 64 {
            break;
        }
        let same = (0..m).filter(|&i| bits[i] == bits[i + p]).count();
        let agreement = same as f64 / m as f64;
        if agreement >= 0.8 {
            c.class = Class::Sync;
            c.period = p;
            c.agreement = agreement;
            return c;
        }
    }
    if (0.3..=0.7).contains(&ones) && (0.3..=0.7).contains(&activity) {
        c.class = Class::Random;
    }
    c
}

/// The state of the receiver.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    /// Looking for a carrier.
    #[default]
    Nothing = 0,
    /// A carrier, no clock.
    Carrier = 1,
    /// Bits, not framed.
    Bits = 2,
    /// Framed.
    Framed = 3,
}

/// A message of the receiver for the log.
pub type Note = String;

/// The receiver.
pub struct FleetRx {
    rate: f64,
    decim: usize,
    fs: f64,
    sps: f64,
    /// The dial's offset from the input centre, Hz.
    dial: f64,
    /// The carrier's offset from the dial, Hz.
    pub carrier_hz: f64,
    phase: f64,
    acc: Complex32,
    nacc: usize,
    /// The state.
    pub state: State,
    /// The bit inversion.
    pub invert: bool,
    // Carrier search.
    search: Vec<Complex32>,
    fft: Fft,
    /// The line at the carrier search, dB over the noise.
    pub line_db: f64,
    // Clock.
    n: u64,
    since_search: u64,
    clock: Complex32,
    energy: f64,
    decay: f64,
    ring: VecDeque<Complex32>,
    sums: VecDeque<Complex32>,
    strobe: f64,
    sampling: bool,
    without_clock: u64,
    /// The clock error, ppm.
    pub clock_ppm: f64,
    corr_mean: f64,
    // Bits.
    prev: Option<Complex32>,
    /// Symbols so far.
    pub symbols: u64,
    a: f64,
    b: f64,
    bits: VecDeque<u8>,
    bit_count: u64,
    next_analysis: u64,
    /// The columns of the last analysis.
    pub columns: [Column; COLUMNS],
    /// The column that is channel 0, when framed.
    pub sync_column: Option<usize>,
    /// Several columns repeat.
    pub ambiguous: bool,
    misses: u32,
    /// Frames since the alignment.
    pub frames_ok: u64,
    /// Alignments lost or moved.
    pub frames_bad: u64,
    /// Notes for the log since the last call of [`FleetRx::take_notes`].
    notes: Vec<Note>,
    level: f64,
}

impl FleetRx {
    /// A receiver for an input at `rate` with the dial `dial` Hz from its centre.
    pub fn new(rate: f64, dial: f64) -> FleetRx {
        let decim = ((rate / WORK_RATE).round() as usize).max(1);
        let fs = rate / decim as f64;
        FleetRx {
            rate,
            decim,
            fs,
            sps: fs / BAUD,
            dial,
            carrier_hz: 0.0,
            phase: 0.0,
            acc: Complex32::default(),
            nacc: 0,
            state: State::Nothing,
            invert: false,
            search: Vec::with_capacity(FFT_N),
            fft: Fft::forward(FFT_N),
            line_db: 0.0,
            n: 0,
            since_search: 0,
            clock: Complex32::default(),
            energy: 0.0,
            decay: 1.0 - 1.0 / (0.5 * fs),
            ring: VecDeque::with_capacity(64),
            sums: VecDeque::with_capacity(64),
            strobe: 0.0,
            sampling: false,
            without_clock: 0,
            clock_ppm: 0.0,
            corr_mean: 0.0,
            prev: None,
            symbols: 0,
            a: 0.0,
            b: 0.0,
            bits: VecDeque::with_capacity(ANALYSIS_BITS + 1),
            bit_count: 0,
            next_analysis: ANALYSIS_EVERY,
            columns: [Column::default(); COLUMNS],
            sync_column: None,
            ambiguous: false,
            misses: 0,
            frames_ok: 0,
            frames_bad: 0,
            notes: Vec::new(),
            level: 0.0,
        }
    }

    /// The dial moved: the carrier search starts again.
    pub fn retune(&mut self, dial: f64) {
        self.dial = dial;
        self.restart(None);
    }

    /// The messages of the receiver since the last call.
    pub fn take_notes(&mut self) -> Vec<Note> {
        std::mem::take(&mut self.notes)
    }

    /// The mean input power.
    pub fn level(&self) -> f64 {
        self.level
    }

    /// The symbol to noise ratio from the decisions, dB.
    pub fn snr_db(&self) -> f64 {
        if self.b > 0.0 {
            10.0 * (self.a * self.a / self.b).log10()
        } else {
            0.0
        }
    }

    /// The eye: 0 clean, towards 1 noisy.
    pub fn eye(&self) -> f64 {
        if self.a > 0.0 {
            self.b.sqrt() / self.a
        } else {
            1.0
        }
    }

    /// The last `k` bits of column `c`, oldest first.
    pub fn column_bits(&self, c: usize, k: usize) -> Vec<u8> {
        let n = self.bits.len() as u64;
        let first = self.bit_count - n;
        let mut out: Vec<u8> = (0..n)
            .filter(|&i| ((first + i) as usize) % COLUMNS == c)
            .map(|i| self.bits[i as usize])
            .collect();
        let skip = out.len().saturating_sub(k);
        out.drain(..skip);
        out
    }

    /// The last whole frames, newest last: each as a 16-bit word, channel 0 the most significant bit, and the signal
    /// time of its first bit in seconds.
    pub fn frame_log(&self) -> Vec<(u16, f64)> {
        let Some(s) = self.sync_column else {
            return Vec::new();
        };
        let n = self.bits.len() as u64;
        let first = self.bit_count - n;
        let now = self.n as f64 / self.fs;
        let mut out = Vec::new();
        // The newest whole frame starts at a bit of column `s`.
        let mut start = self.bit_count.saturating_sub(COLUMNS as u64);
        while start % COLUMNS as u64 != s as u64 {
            start -= 1;
        }
        for k in (0..FRAME_LOG as u64).rev() {
            let Some(f0) = start.checked_sub(k * COLUMNS as u64) else {
                continue;
            };
            if f0 < first {
                continue;
            }
            let mut w = 0u16;
            for j in 0..COLUMNS as u64 {
                w = (w << 1) | u16::from(self.bits[(f0 + j - first) as usize]);
            }
            let age = (self.bit_count - 1 - f0) as f64 / BAUD;
            out.push((w, now - age));
        }
        out
    }

    fn restart(&mut self, why: Option<&str>) {
        if let Some(w) = why {
            self.notes.push(w.into());
        }
        self.state = State::Nothing;
        self.search.clear();
        self.sampling = false;
        self.prev = None;
        self.clock = Complex32::default();
        self.energy = 0.0;
        self.since_search = 0;
        self.without_clock = 0;
        self.ring.clear();
        self.sums.clear();
        self.sync_column = None;
        self.ambiguous = false;
        self.misses = 0;
    }

    /// Takes input samples.
    pub fn process(&mut self, x: &[Complex32]) {
        let step = -TAU * (self.dial + self.carrier_hz) / self.rate;
        for &v in x {
            self.level += 1e-5 * (f64::from(v.norm_sqr()) - self.level);
            let (s, c) = self.phase.sin_cos();
            self.phase = (self.phase + step).rem_euclid(TAU);
            self.acc += v * Complex32::new(c as f32, s as f32);
            self.nacc += 1;
            if self.nacc == self.decim {
                let y = self.acc / self.decim as f32;
                self.acc = Complex32::default();
                self.nacc = 0;
                self.sample(y);
            }
        }
    }

    fn sample(&mut self, y: Complex32) {
        self.n += 1;
        if self.state == State::Nothing {
            self.search.push(y);
            if self.search.len() == FFT_N {
                self.find_carrier();
            }
            return;
        }
        self.since_search += 1;
        // The energy line at the symbol rate.
        let e = f64::from(y.norm_sqr());
        let ph = -TAU * BAUD * self.n as f64 / self.fs;
        let d = self.decay as f32;
        self.clock = self.clock * d + Complex32::from_polar(e as f32, ph as f32);
        self.energy = self.energy * self.decay + e;
        let ratio = if self.energy > 0.0 {
            f64::from(self.clock.norm()) / self.energy
        } else {
            0.0
        };
        // A running sum over one symbol, kept for the box integral.
        let last = self.sums.back().copied().unwrap_or_default();
        self.sums.push_back(last + y);
        self.ring.push_back(y);
        let keep = (2.0 * self.sps) as usize + 4;
        while self.sums.len() > keep {
            self.sums.pop_front();
            self.ring.pop_front();
        }
        if ratio > CLOCK_RATIO {
            self.without_clock = 0;
        } else {
            self.without_clock += 1;
        }
        // The energy peak in samples of the symbol, from the phase of the clock line.
        let peak = {
            let a = f64::from(self.clock.im.atan2(self.clock.re));
            (-a / TAU * self.sps).rem_euclid(self.sps)
        };
        if !self.sampling {
            if self.since_search as f64 > 0.4 * self.fs && ratio > CLOCK_RATIO {
                self.sampling = true;
                self.state = State::Bits;
                self.strobe = self.n as f64
                    + ((peak + (self.sps - 1.0) / 2.0 - (self.n as f64).rem_euclid(self.sps))
                        .rem_euclid(self.sps));
                self.notes.push("1200 Bd clock locked, bits flowing".into());
            } else if self.since_search as f64 > 6.0 * self.fs {
                self.restart(Some("no 1200 Bd clock found"));
            }
            return;
        }
        if self.without_clock as f64 > 3.0 * self.fs {
            self.restart(Some("lost the 1200 Bd clock"));
            return;
        }
        if (self.n as f64) < self.strobe {
            return;
        }
        // The box integral over one symbol ending at the strobe, interpolated between samples.
        let frac = self.strobe - (self.n - 1) as f64;
        let len = self.sums.len();
        let w = self.sps;
        let at = |back: f64| -> Complex32 {
            let i = (len as f64 - 1.0 - back).max(0.0);
            let k = i.floor() as usize;
            let f = (i - k as f64) as f32;
            let a = self.sums[k];
            let b = self.sums[(k + 1).min(len - 1)];
            a * (1.0 - f) + b * f
        };
        let end = at(1.0 - frac);
        let sym = end - at(1.0 - frac + w);
        // Timing: the strobe sits half a symbol after the energy peak.
        if ratio > CLOCK_RATIO {
            let target = (peak + (self.sps - 1.0) / 2.0).rem_euclid(self.sps);
            let mut err = target - self.strobe.rem_euclid(self.sps);
            if err > self.sps / 2.0 {
                err -= self.sps;
            } else if err < -self.sps / 2.0 {
                err += self.sps;
            }
            let corr = 0.05 * err;
            self.strobe += corr;
            self.corr_mean += 0.001 * (corr - self.corr_mean);
            self.clock_ppm = -self.corr_mean / self.sps * 1e6;
        }
        self.strobe += self.sps;
        self.symbol(sym);
    }

    fn symbol(&mut self, s: Complex32) {
        self.symbols += 1;
        let Some(p) = self.prev.replace(s) else {
            return;
        };
        let d = s * p.conj();
        let mag = d.norm().max(1e-30);
        let dn = d / mag;
        let bit = u8::from(dn.re < 0.0) ^ u8::from(self.invert);
        // The frequency loop: half the angle of d squared.
        let d2 = dn * dn;
        let theta = f64::from(d2.im.atan2(d2.re)) / 2.0;
        self.carrier_hz =
            (self.carrier_hz + 0.01 * theta * BAUD / TAU).clamp(-SEARCH_HZ, SEARCH_HZ);
        self.a += 0.005 * (f64::from(dn.re.abs()) - self.a);
        self.b += 0.005 * (f64::from(dn.im * dn.im) - self.b);
        self.bits.push_back(bit);
        if self.bits.len() > ANALYSIS_BITS {
            self.bits.pop_front();
        }
        self.bit_count += 1;
        if self.state == State::Framed {
            if let Some(s) = self.sync_column {
                if (self.bit_count as usize) % COLUMNS == s {
                    self.frames_ok += 1;
                }
            }
        }
        if self.bit_count >= self.next_analysis {
            self.next_analysis += ANALYSIS_EVERY;
            if self.bits.len() >= ANALYSIS_MIN {
                self.analyse();
            }
        }
    }

    fn analyse(&mut self) {
        for c in 0..COLUMNS {
            self.columns[c] = classify(&self.column_bits(c, ANALYSIS_BITS / COLUMNS));
        }
        let sync: Vec<usize> = (0..COLUMNS)
            .filter(|&c| self.columns[c].class == Class::Sync)
            .collect();
        self.ambiguous = sync.len() > 1;
        match (sync.len(), self.sync_column) {
            (1, None) => {
                self.sync_column = Some(sync[0]);
                self.state = State::Framed;
                self.misses = 0;
                self.frames_ok = 0;
                self.notes.push(format!(
                    "sync channel found, repeats every {} frames: 16 channels framed",
                    self.columns[sync[0]].period
                ));
            }
            (1, Some(s)) if s != sync[0] => {
                self.sync_column = Some(sync[0]);
                self.frames_bad += 1;
                self.frames_ok = 0;
                self.misses = 0;
                self.notes
                    .push("the sync channel moved: frame alignment redone".into());
            }
            (1, Some(_)) => self.misses = 0,
            (_, Some(_)) => {
                self.misses += 1;
                if self.misses >= 3 {
                    self.sync_column = None;
                    self.state = State::Bits;
                    self.frames_bad += 1;
                    self.notes
                        .push("the sync channel is gone: frame alignment lost".into());
                }
            }
            (n, None) if n > 1 => {
                self.notes
                    .push("several columns repeat: the frame start cannot be told".into());
            }
            _ => {}
        }
    }

    /// The carrier search: the line of the squared signal within the search range, 17 dB over the noise.
    fn find_carrier(&mut self) {
        let n = FFT_N;
        let mut buf: Vec<Complex32> = self
            .search
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let w = 0.5 - 0.5 * (TAU * i as f64 / n as f64).cos();
                *v * *v * w as f32
            })
            .collect();
        self.search.clear();
        self.fft.process(&mut buf);
        let p: Vec<f64> = buf.iter().map(|v| f64::from(v.norm_sqr())).collect();
        let bin = self.fs / n as f64;
        let freq = |k: usize| {
            let k = k as f64;
            if k < n as f64 / 2.0 {
                k * bin
            } else {
                (k - n as f64) * bin
            }
        };
        let mut sorted = p.clone();
        sorted.sort_by(f64::total_cmp);
        let noise = sorted[n / 2] * 1.443;
        let best = (0..n)
            .filter(|&k| (freq(k) / 2.0).abs() <= SEARCH_HZ)
            .max_by(|&a, &b| p[a].total_cmp(&p[b]));
        let Some(k) = best else {
            return;
        };
        let db = 10.0 * (p[k] / noise.max(1e-300)).log10();
        if db < LINE_DB {
            return;
        }
        // A parabola through the logarithms of the peak and its neighbours.
        let l = |i: usize| p[i % n].max(1e-300).ln();
        let (a, b, c) = (l(k + n - 1), l(k), l(k + 1));
        let den = a - 2.0 * b + c;
        let frac = if den < 0.0 {
            (0.5 * (a - c) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let line = freq(k) + frac * bin;
        self.carrier_hz = (self.carrier_hz + line / 2.0).clamp(-SEARCH_HZ, SEARCH_HZ);
        self.line_db = db;
        self.state = State::Carrier;
        self.since_search = 0;
        self.notes.push(format!(
            "BPSK carrier at {:+.1} Hz from the tuned frequency, {:.0} dB over the floor",
            self.carrier_hz, db
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The m-sequence of the test signal: 5-bit register from 11111, output bit 0, new bit = bit 4 XOR bit 2 at the bottom.
    pub fn m31(n: usize) -> Vec<u8> {
        let mut r = 0x1Fu8;
        (0..n)
            .map(|_| {
                let out = r & 1;
                let new = ((r >> 4) ^ (r >> 2)) & 1;
                r = ((r << 1) | new) & 0x1F;
                out
            })
            .collect()
    }

    #[test]
    fn column_classes() {
        assert_eq!(classify(&[1; 256]).class, Class::Idle);
        let rev: Vec<u8> = (0..256).map(|i| (i % 2) as u8).collect();
        assert_eq!(classify(&rev).class, Class::Reversals);
        let s = classify(&m31(256));
        assert_eq!((s.class, s.period), (Class::Sync, 31));
        assert!((s.agreement - 1.0).abs() < 1e-9);
        let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
        let r: Vec<u8> = (0..256).map(|_| (rng.next_u32() & 1) as u8).collect();
        assert_eq!(classify(&r).class, Class::Random);
    }
}
