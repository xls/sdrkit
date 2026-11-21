//! Continuous-phase frequency modulation: FSK and CPFSK, MSK, GMSK and GFSK, multi-level FSK, and AFSK on an audio carrier.

use crate::{Param, SimError};
use sdrkit_algo_dsp::approx::sin_cos;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_core::Complex32;
use std::f64::consts::{LN_2, TAU};

/// Phase-pulse table resolution, points per symbol.
const RES: usize = 256;

/// The amplitude envelope at the start and end of a burst, for modulators whose output has constant amplitude. The length is in
/// output samples and can be fractional. At distance `x` samples from the burst's first sample (or from its last), the weight is:
///
/// | Ramp | Weight for `x < len` |
/// |---|---|
/// | [`Ramp::None`] | 1 |
/// | [`Ramp::RaisedCosine`] | `(1 - cos(pi x / len)) / 2` |
/// | [`Ramp::Linear`] | `x / len` |
///
/// The weight is 1 from `x = len` on, and the end mirrors the start (the last sample has weight 0). The weak-signal protocols use
/// a raised cosine over 1/8 of a symbol (FT8) and over one whole ramp symbol (FT4, FT2), and JS8 a linear ramp of 10 ms
/// (Franke, Somerville and Taylor, "The FT4 and FT8 Communication Protocols", QEX July/August 2020).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Ramp {
    /// Full amplitude from the first sample to the last.
    #[default]
    None,
    /// A raised-cosine rise and fall over this many samples.
    RaisedCosine(f64),
    /// A linear rise and fall over this many samples.
    Linear(f64),
}

impl Ramp {
    /// The length in samples (0 for [`Ramp::None`]).
    pub fn len(&self) -> f64 {
        match *self {
            Ramp::None => 0.0,
            Ramp::RaisedCosine(l) | Ramp::Linear(l) => l,
        }
    }

    /// True for a ramp of no length.
    pub fn is_empty(&self) -> bool {
        self.len() <= 0.0
    }

    /// An error unless the length is finite and at least 0.
    pub fn check(&self) -> Result<(), SimError> {
        let l = self.len();
        if l.is_finite() && l >= 0.0 {
            Ok(())
        } else {
            Param::Ramp.at_least(0.0, l)
        }
    }

    /// The weight `x` samples from the edge sample of a burst (see the table of [`Ramp`]).
    #[inline]
    pub fn weight(&self, x: f64) -> f32 {
        match *self {
            Ramp::RaisedCosine(l) if x < l => {
                (0.5 * (1.0 - (std::f64::consts::PI * x / l).cos())) as f32
            }
            Ramp::Linear(l) if x < l => (x / l) as f32,
            _ => 1.0,
        }
    }

    /// The number of samples at each edge whose weight is below 1.
    pub fn samples(&self) -> usize {
        self.len().ceil() as usize
    }

    /// Multiplies a whole burst by the envelope: the rise from `burst[0]`, the fall ending one sample after the last. A burst shorter than two
    /// ramps gets the product of both.
    pub fn apply(&self, burst: &mut [Complex32]) {
        let n = burst.len();
        for k in 0..self.samples().min(n) {
            burst[k] *= self.weight(k as f64);
            burst[n - 1 - k] *= self.weight(k as f64 + 1.0);
        }
    }

    /// As [`Ramp::apply`], for real samples.
    pub fn apply_real(&self, burst: &mut [f32]) {
        let n = burst.len();
        for k in 0..self.samples().min(n) {
            burst[k] *= self.weight(k as f64);
            burst[n - 1 - k] *= self.weight(k as f64 + 1.0);
        }
    }
}

/// Applies a [`Ramp`] to a stream of bursts: the rise as the samples pass, the fall by holding back the last samples of the burst
/// until its end is known.
#[derive(Clone, Debug, Default)]
struct Shaper {
    ramp: Ramp,
    /// Samples of the current burst so far.
    i: u64,
    /// The time of output sample 0 after the burst's start, in samples (0 to 1).
    start: f64,
    /// Held samples (a ring of `fall` samples).
    held: Vec<Complex32>,
    head: usize,
    len: usize,
}

impl Shaper {
    fn new(ramp: Ramp) -> Shaper {
        Shaper {
            ramp,
            i: 0,
            start: 0.0,
            held: vec![Complex32::default(); ramp.samples()],
            head: 0,
            len: 0,
        }
    }

    #[inline]
    fn take(&mut self, z: Complex32, out: &mut Vec<Complex32>) {
        // The rise weight is taken at the sample's time from the burst's start.
        let z = z * self.ramp.weight(self.i as f64 + self.start);
        self.i += 1;
        let fall = self.held.len();
        if fall == 0 {
            out.push(z);
        } else if self.len < fall {
            self.held[(self.head + self.len) % fall] = z;
            self.len += 1;
        } else {
            out.push(self.held[self.head]);
            self.held[self.head] = z;
            self.head = (self.head + 1) % fall;
        }
    }

    /// Returns the held samples with the fall applied, and starts a new burst. `end` is the burst's end in samples from its start.
    fn end(&mut self, end: f64, out: &mut Vec<Complex32>) {
        let fall = self.held.len();
        let first = self.i - self.len as u64;
        for k in 0..self.len {
            // The fall weight is taken at the sample's time before the burst's end.
            let x = end - ((first + k as u64) as f64 + self.start);
            out.push(self.held[(self.head + k) % fall.max(1)] * self.ramp.weight(x));
        }
        self.i = 0;
        self.head = 0;
        self.len = 0;
    }
}

/// A continuous-phase modulator (CPM): the phase is `phi(t) = pi h sum_k a_k q(t - k)` radians, `t` in symbols, where `a_k` are
/// the symbol levels (`+-1` for binary FSK, `+-1, +-3` for 4-level FSK), `h` the modulation index and `q` the phase pulse, the
/// integral of the frequency pulse `g` scaled to rise from 0 to 1. So symbol `a` moves the phase by `pi h a` in all, and the
/// frequency deviation of level 1 at the centre of a long run is `h Rs / 2`.
///
/// | Signal | Pulse | `h` |
/// |---|---|---|
/// | CPFSK | rectangle | any |
/// | MSK | rectangle | 0.5 |
/// | GMSK (GSM BT 0.3, AIS BT 0.4) | Gaussian frequency pulse | 0.5 |
/// | GFSK (BLE BT 0.5 h 0.5, others) | Gaussian frequency pulse | any |
/// | 4-level FSK with a Nyquist pulse | raised cosine | per standard |
///
/// The phase pulse is tabulated once (cumulative sum of `g` at the midpoints of a grid of 256 points per symbol, normalised to end
/// at 1), so a rectangular pulse produces an exactly linear phase ramp. Phases of finished symbols are kept as one running sum in
/// `f64` cycles, so the phase is exact over any length. Output sample `n` is at `t = n / sps - L/2` (`L` the pulse span): the first
/// output is where symbol 0's pulse begins.
///
/// ```
/// use sdrkit_algo_sim::modulate::Cpm;
/// // MSK at 8 samples per symbol: each symbol turns the phase by exactly a quarter cycle.
/// let mut m = Cpm::msk(8.0).unwrap();
/// let mut out = Vec::new();
/// m.push(&[1.0, 1.0, 1.0, 1.0, 1.0], &mut out);
/// // Output 32 is the end of symbol 3: four +1 symbols, a full turn, back to 1 + 0j.
/// assert!((out[32].re - 1.0).abs() < 1e-5 && out[32].im.abs() < 1e-5);
/// ```
///
/// Two options shape a burst for standards that define its edges ([`Cpm::with_held_edges`], [`Cpm::with_ramp`]). With held
/// edges, the burst runs from the start of symbol 0 to the end of the last symbol, `ceil(N sps)` samples for `N` symbols, and the
/// first and last levels are held beyond the ends (so a Gaussian pulse does not start or end at the carrier). A ramp shapes the
/// amplitude of the first and last samples of each burst (each [`Cpm::flush`] ends a burst). With a ramp, [`Cpm::push`] holds back
/// the last [`Ramp::samples`] samples until the next push or the flush.
///
/// ```
/// use sdrkit_algo_sim::modulate::{Cpm, Ramp};
/// // FT8: 8-GFSK, BT 2, h 1, 1920 samples per symbol, a raised cosine over 1/8 of a symbol at each end.
/// let mut m = Cpm::gfsk(2.0, 1.0, 1920.0).unwrap().with_held_edges().with_ramp(Ramp::RaisedCosine(240.0)).unwrap();
/// let mut out = Vec::new();
/// m.push(&[3.0, -1.0, 5.0], &mut out);
/// m.flush(&mut out);
/// assert_eq!(out.len(), 3 * 1920);
/// assert_eq!(out[0].norm(), 0.0);
/// assert!((out[120].norm() - 0.5).abs() < 1e-6);
/// ```
#[derive(Clone, Debug)]
pub struct Cpm {
    q: Vec<f32>,
    half: f64,
    /// Phase per unit level in cycles: h / 2.
    k: f64,
    sps: f64,
    ring: Vec<f32>,
    mask: usize,
    syms: u64,
    /// Symbols folded into `acc`.
    done: u64,
    acc: f64,
    n: u64,
    /// The time of output sample 0 of a burst after its usual place, in samples (0 to 1).
    start: f64,
    /// Held edges: the time of output sample 0 in symbol units of the ring (`lead - 1/2`), and the leading copies of level 0.
    held: Option<u64>,
    shaper: Shaper,
}

impl Cpm {
    /// A modulator with frequency pulse `pulse` over `span_symbols`, modulation index `h`, at `sps` samples per symbol. Errors for a
    /// bad pulse, a span not in `(0, 64]`, `h` not finite, `sps` not positive, or a pulse with no area.
    pub fn new(pulse: Shape, span_symbols: f64, h: f64, sps: f64) -> Result<Self, SimError> {
        pulse.check()?;
        if !(span_symbols > 0.0 && span_symbols <= 64.0) {
            return Param::Span.range(0.0, 64.0, span_symbols);
        }
        Param::ModIndex.check_finite(h)?;
        if !(sps > 0.0 && sps.is_finite() && sps <= 1e6) {
            return Param::Sps.range(0.0, 1e6, sps);
        }
        let half = span_symbols / 2.0;
        let n = (span_symbols * RES as f64).round() as usize;
        let mut q = Vec::with_capacity(n + 2);
        let mut s = 0.0f64;
        q.push(0.0);
        for i in 0..n {
            s += pulse.value(-half + (i as f64 + 0.5) / RES as f64);
            q.push(s);
        }
        if !(s.abs() > 0.0 && s.is_finite()) {
            return Param::Pulse.degenerate();
        }
        let q: Vec<f32> = q.iter().map(|v| (v / s) as f32).collect();
        let size = ((span_symbols.ceil() as usize) + 4).next_power_of_two();
        Ok(Cpm {
            q,
            half,
            k: h / 2.0,
            sps,
            ring: vec![0.0; size],
            mask: size - 1,
            syms: 0,
            done: 0,
            acc: 0.0,
            n: 0,
            start: 0.0,
            held: None,
            shaper: Shaper::default(),
        })
    }

    /// Holds the first and last levels of each burst beyond its ends, and makes the burst run from the start of symbol 0 to the
    /// end of the last symbol: output sample `n` is at `n / sps` symbols from the start of symbol 0, and a burst of `N` symbols
    /// has `ceil(N sps)` samples. This is how the weak-signal protocols shape a Gaussian pulse at the ends of a transmission. Takes
    /// effect from the next burst (after [`Cpm::flush`], or at once on a new modulator).
    pub fn with_held_edges(mut self) -> Self {
        // Copies of level 0 needed before it: every symbol whose pulse reaches past the start of symbol 0.
        self.held = Some((self.half - 0.5).max(0.0).ceil() as u64);
        self
    }

    /// Shapes the amplitude of each burst with `ramp` (see [`Ramp`] and the type documentation). Allocates the hold-back
    /// buffer here. An error for a ramp length that is negative or not finite.
    pub fn with_ramp(mut self, ramp: Ramp) -> Result<Self, SimError> {
        ramp.check()?;
        self.shaper = Shaper {
            start: self.start,
            ..Shaper::new(ramp)
        };
        Ok(self)
    }

    /// Moves the sample grid of the next bursts by `offset` samples (0 to less than 1): output sample `n` is at `n + offset`
    /// samples from the usual place. A caller that places a burst at a fractional time on its own sample grid uses it. Call it
    /// between bursts. An error for an offset out of range.
    pub fn set_start(&mut self, offset: f64) -> Result<(), SimError> {
        if !(0.0..1.0).contains(&offset) {
            return Param::Start.range(0.0, 1.0, offset);
        }
        self.start = offset;
        self.shaper.start = offset;
        Ok(())
    }

    /// Drops the burst in progress without output, and sets the phase to 0. The settings stay.
    pub fn reset(&mut self) {
        self.syms = 0;
        self.done = 0;
        self.n = 0;
        self.acc = 0.0;
        self.ring.iter_mut().for_each(|v| *v = 0.0);
        self.shaper.i = 0;
        self.shaper.head = 0;
        self.shaper.len = 0;
    }

    /// The ramp in use.
    pub fn ramp(&self) -> Ramp {
        self.shaper.ramp
    }

    /// Binary or multi-level CPFSK: rectangular frequency pulse one symbol long.
    pub fn cpfsk(h: f64, sps: f64) -> Result<Self, SimError> {
        Self::new(Shape::Rect, 1.0, h, sps)
    }

    /// MSK: CPFSK with `h = 0.5`.
    pub fn msk(sps: f64) -> Result<Self, SimError> {
        Self::cpfsk(0.5, sps)
    }

    /// GFSK of bandwidth-time product `bt` and index `h`; the pulse spans `1 + 8 sigma` symbols, `sigma = sqrt(ln 2) / (2 pi bt)`.
    pub fn gfsk(bt: f64, h: f64, sps: f64) -> Result<Self, SimError> {
        if !(bt > 0.0 && bt.is_finite()) {
            return Param::Bt.positive(bt);
        }
        let sigma = LN_2.sqrt() / (TAU * bt);
        Self::new(
            Shape::GaussianFreq { bt },
            (1.0 + 8.0 * sigma).ceil().min(64.0),
            h,
            sps,
        )
    }

    /// GMSK: GFSK with `h = 0.5`.
    pub fn gmsk(bt: f64, sps: f64) -> Result<Self, SimError> {
        Self::gfsk(bt, 0.5, sps)
    }

    /// A modulator set by its deviation: level 1 moves the frequency by `deviation_hz` (so `h = 2 deviation / Rs`), symbols at
    /// `symbol_rate_hz`, samples at `rate_hz`.
    pub fn with_deviation(
        pulse: Shape,
        span_symbols: f64,
        deviation_hz: f64,
        symbol_rate_hz: f64,
        rate_hz: f64,
    ) -> Result<Self, SimError> {
        Param::SymbolRate.check_positive(symbol_rate_hz)?;
        Param::Rate.check_positive(rate_hz)?;
        Self::new(
            pulse,
            span_symbols,
            2.0 * deviation_hz / symbol_rate_hz,
            rate_hz / symbol_rate_hz,
        )
    }

    /// Samples per symbol.
    pub fn sps(&self) -> f64 {
        self.sps
    }

    /// The modulation index.
    pub fn h(&self) -> f64 {
        2.0 * self.k
    }

    /// The output sample at the centre of symbol `k`'s pulse.
    pub fn symbol_centre(&self, k: u64) -> f64 {
        (k as f64 + self.half) * self.sps
    }

    #[inline]
    fn q_at(&self, d: f64) -> f32 {
        let u = (d + self.half) * RES as f64;
        if u <= 0.0 {
            return 0.0;
        }
        let i = u as usize;
        if i + 1 >= self.q.len() {
            return 1.0;
        }
        let f = (u - i as f64) as f32;
        self.q[i] + (self.q[i + 1] - self.q[i]) * f
    }

    /// The phase in cycles at `t` symbols, folding finished symbols into the running sum.
    #[inline]
    fn phase(&mut self, t: f64, limit: u64) -> f64 {
        while self.done < limit && t - self.done as f64 >= self.half {
            self.acc = (self.acc + self.k * self.ring[self.done as usize & self.mask] as f64)
                .rem_euclid(1.0);
            self.done += 1;
        }
        let hi = ((t + self.half).floor() as i64).min(limit as i64 - 1);
        let mut p = 0.0f32;
        let mut k = self.done as i64;
        while k <= hi {
            p += self.ring[k as usize & self.mask] * self.q_at(t - k as f64);
            k += 1;
        }
        self.acc + self.k * p as f64
    }

    /// Puts a level into the ring.
    #[inline]
    fn put(&mut self, a: f32) {
        self.ring[self.syms as usize & self.mask] = a;
        self.syms += 1;
    }

    /// The held-edge form of [`Cpm::run`]: the leading copies at a burst's first level, and sample `n` at `n / sps + lead - 1/2`.
    fn run_held(&mut self, lead: u64, levels: &[f32], emit: &mut impl FnMut(f64)) {
        for &a in levels {
            let a = if a.is_finite() { a } else { 0.0 };
            if self.syms == 0 {
                for _ in 0..lead {
                    self.put(a);
                }
            }
            self.put(a);
            loop {
                let t = (self.n as f64 + self.start) / self.sps + lead as f64 - 0.5;
                if t + self.half >= self.syms as f64 {
                    break;
                }
                let p = self.phase(t, self.syms);
                emit(p);
                self.n += 1;
            }
        }
    }

    /// The held-edge form of [`Cpm::finish`]: copies of the last level until the end of the last symbol is out.
    fn finish_held(&mut self, lead: u64, emit: &mut impl FnMut(f64)) {
        if self.syms > 0 {
            let last = self.ring[(self.syms - 1) as usize & self.mask];
            let end = self.syms as f64 - 0.5;
            loop {
                let t = (self.n as f64 + self.start) / self.sps + lead as f64 - 0.5;
                if t >= end {
                    break;
                }
                if t + self.half >= self.syms as f64 {
                    self.put(last);
                    continue;
                }
                let p = self.phase(t, self.syms);
                emit(p);
                self.n += 1;
            }
            // The next burst starts at the phase reached at the end of the last symbol.
            while self.syms as f64 <= end + self.half {
                self.put(last);
            }
            self.acc = self.phase(end, self.syms).rem_euclid(1.0);
        }
        self.syms = 0;
        self.done = 0;
        self.n = 0;
        self.ring.iter_mut().for_each(|v| *v = 0.0);
    }

    /// Runs the levels, calling `emit` with the phase (cycles) of every sample they determine.
    pub(crate) fn run(&mut self, levels: &[f32], mut emit: impl FnMut(f64)) {
        if let Some(lead) = self.held {
            return self.run_held(lead, levels, &mut emit);
        }
        for &a in levels {
            self.ring[self.syms as usize & self.mask] = if a.is_finite() { a } else { 0.0 };
            self.syms += 1;
            loop {
                let u = (self.n as f64 + self.start) / self.sps;
                if u >= self.syms as f64 {
                    break;
                }
                let p = self.phase(u - self.half, self.syms);
                emit(p);
                self.n += 1;
            }
        }
    }

    /// Emits the rest of the last pulse (the frequency returns to the carrier) and starts again, keeping the phase.
    pub(crate) fn finish(&mut self, mut emit: impl FnMut(f64)) {
        if let Some(lead) = self.held {
            return self.finish_held(lead, &mut emit);
        }
        if self.syms > 0 {
            let end = (self.syms - 1) as f64 + 2.0 * self.half;
            loop {
                let u = (self.n as f64 + self.start) / self.sps;
                if u >= end {
                    break;
                }
                let p = self.phase(u - self.half, self.syms);
                emit(p);
                self.n += 1;
            }
            while self.done < self.syms {
                self.acc = (self.acc + self.k * self.ring[self.done as usize & self.mask] as f64)
                    .rem_euclid(1.0);
                self.done += 1;
            }
        }
        self.syms = 0;
        self.done = 0;
        self.n = 0;
        self.ring.iter_mut().for_each(|v| *v = 0.0);
    }

    /// Appends the samples (unit amplitude, times the ramp if one is set) the levels so far determine. NaN or infinite levels
    /// count as 0. With a ramp, the last [`Ramp::samples`] samples wait for the next push or the flush.
    pub fn push(&mut self, levels: &[f32], out: &mut Vec<Complex32>) {
        if self.shaper.ramp.is_empty() {
            self.run(levels, |p| {
                let (s, c) = sin_cos(TAU * p);
                out.push(Complex32::new(c, s));
            });
            return;
        }
        let mut sh = std::mem::take(&mut self.shaper);
        self.run(levels, |p| {
            let (s, c) = sin_cos(TAU * p);
            sh.take(Complex32::new(c, s), out);
        });
        self.shaper = sh;
    }

    /// Appends the tail of the burst (the rest of the last pulse, or of the last symbol with held edges), with the fall of the
    /// ramp, and starts a new burst at the phase reached.
    pub fn flush(&mut self, out: &mut Vec<Complex32>) {
        if self.shaper.ramp.is_empty() {
            self.finish(|p| {
                let (s, c) = sin_cos(TAU * p);
                out.push(Complex32::new(c, s));
            });
            return;
        }
        let end = match self.held {
            Some(lead) => self.syms.saturating_sub(lead) as f64,
            None => self.syms.saturating_sub(1) as f64 + 2.0 * self.half,
        } * self.sps;
        let mut sh = std::mem::take(&mut self.shaper);
        self.finish(|p| {
            let (s, c) = sin_cos(TAU * p);
            sh.take(Complex32::new(c, s), out);
        });
        sh.end(end, out);
        self.shaper = sh;
    }
}

/// Audio frequency-shift keying: phase-continuous switching between a mark and a space tone, as real audio (Bell 202 APRS 1200/2200
/// Hz, marine DSC 1300/2100 Hz, NOAA SAME 2083.3/1562.5 Hz, MPT 1327 1200/1800 Hz).
///
/// It is CPFSK with `h = |mark - space| / baud` on a carrier at the middle frequency, so the switch happens exactly at each bit
/// boundary (fractional sample positions included) with no phase jump.
///
/// ```
/// use sdrkit_algo_sim::modulate::Afsk;
/// let mut a = Afsk::new(48_000.0, 1200.0, 1200.0, 2200.0).unwrap();
/// let mut audio = Vec::new();
/// a.push(&[1, 0, 1, 1], &mut audio);
/// assert_eq!(audio.len(), 160);
/// ```
#[derive(Clone, Debug)]
pub struct Afsk {
    cpm: Cpm,
    carrier: f64,
    n: u64,
    sign: f32,
    amplitude: f32,
}

impl Afsk {
    /// Bits at `baud` on audio at `rate_hz`: 1 sends `mark_hz`, 0 sends `space_hz`. Errors unless both tones lie in `(0, rate/2)`,
    /// differ, and the baud is positive.
    pub fn new(rate_hz: f64, baud: f64, mark_hz: f64, space_hz: f64) -> Result<Self, SimError> {
        let ok = |f: f64| f > 0.0 && f < rate_hz / 2.0;
        Param::Rate.check_positive(rate_hz)?;
        Param::Baud.check_positive(baud)?;
        if !ok(mark_hz) {
            return Param::MarkHz.range(0.0, rate_hz / 2.0, mark_hz);
        }
        if !ok(space_hz) {
            return Param::SpaceHz.range(0.0, rate_hz / 2.0, space_hz);
        }
        if mark_hz == space_hz {
            return Param::SpaceHz.conflict(Param::MarkHz);
        }
        Ok(Afsk {
            cpm: Cpm::cpfsk((mark_hz - space_hz).abs() / baud, rate_hz / baud)?,
            carrier: (mark_hz + space_hz) / 2.0 / rate_hz,
            n: 0,
            sign: (mark_hz - space_hz).signum() as f32,
            amplitude: 1.0,
        })
    }

    /// Sets the peak amplitude (1 by default).
    pub fn with_amplitude(mut self, a: f32) -> Self {
        self.amplitude = a;
        self
    }

    fn emit(carrier: f64, n: &mut u64, amp: f32, out: &mut Vec<f32>, p: f64) {
        let c = (carrier * *n as f64).rem_euclid(1.0);
        *n += 1;
        let (_, cos) = sin_cos(TAU * (c + p));
        out.push(amp * cos);
    }

    /// Appends the audio of `bits` (any non-zero byte is a 1). Samples are emitted as soon as they are determined.
    pub fn push(&mut self, bits: &[u8], out: &mut Vec<f32>) {
        let (carrier, amp, sign) = (self.carrier, self.amplitude, self.sign);
        let n = &mut self.n;
        for &b in bits {
            let level = if b != 0 { sign } else { -sign };
            self.cpm
                .run(&[level], |p| Self::emit(carrier, n, amp, out, p));
        }
    }

    /// Appends the end of the last bit (the bits so far, completed); the next push continues in phase.
    pub fn flush(&mut self, out: &mut Vec<f32>) {
        let (carrier, amp) = (self.carrier, self.amplitude);
        let n = &mut self.n;
        self.cpm.finish(|p| Self::emit(carrier, n, amp, out, p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn a_start_offset_moves_the_grid_and_reset_restarts() {
        // MSK, 4 samples per symbol, held edges: a run of +1 turns a quarter cycle per symbol, so sample n has phase
        // (n + offset) / 16 cycles.
        let mut m = Cpm::msk(4.0).unwrap().with_held_edges();
        m.set_start(0.5).unwrap();
        let mut out = Vec::new();
        m.push(&[1.0; 8], &mut out);
        m.flush(&mut out);
        assert_eq!(out.len(), 32);
        for (n, z) in out.iter().enumerate() {
            let want = TAU * (n as f64 + 0.5) / 16.0;
            assert!((z.arg() as f64 - want).sin().abs() < 1e-4, "{n}");
        }
        // After a reset the next burst starts at phase 0 again.
        let first = out[0];
        m.push(&[1.0; 3], &mut out);
        m.reset();
        out.clear();
        m.push(&[1.0; 8], &mut out);
        m.flush(&mut out);
        assert!((out[0] - first).norm() < 1e-5);
        assert!(m.set_start(1.0).is_err());
        assert!(m.set_start(-0.1).is_err());
    }

    /// Instantaneous frequency in cycles per sample from the phase difference (independent of the modulator).
    fn freq(x: &[Complex32]) -> Vec<f64> {
        x.windows(2)
            .map(|w| {
                let d = w[1] * w[0].conj();
                (d.im as f64).atan2(d.re as f64) / TAU
            })
            .collect()
    }

    #[test]
    fn msk_turns_a_quarter_cycle_per_symbol_and_is_exactly_continuous() {
        // MSK at 8 sps: the frequency is +-1/4 of the symbol rate = +-1/32 cycle per sample within every symbol, and the phase
        // after k symbols is (sum a) / 4 cycles.
        let mut m = Cpm::msk(8.0).unwrap();
        let mut rng = Rng::new(3);
        let levels: Vec<f32> = (0..400)
            .map(|_| if rng.bit() == 1 { 1.0 } else { -1.0 })
            .collect();
        let mut y = Vec::new();
        m.push(&levels, &mut y);
        let f = freq(&y);
        for (n, fr) in f.iter().enumerate() {
            let k = n / 8;
            assert!((fr - levels[k] as f64 / 32.0).abs() < 1e-5, "sample {n}");
        }
        let mut cyc = 0.0f64;
        for (k, a) in levels.iter().enumerate().take(390) {
            cyc += *a as f64 / 4.0;
            let z = y[(k + 1) * 8];
            let want = TAU * cyc;
            let got = (z.im as f64).atan2(z.re as f64);
            let e = (got - want).rem_euclid(TAU);
            assert!(e.min(TAU - e) < 1e-4, "after symbol {k}");
        }
        assert!(y.iter().all(|v| (v.norm() - 1.0).abs() < 1e-5));
    }

    #[test]
    fn gmsk_deviation_and_spectrum_follow_bt() {
        // GMSK BT 0.3 at 4 sps: a long run of +1 settles at +Rs/4; an isolated symbol in alternating data peaks lower (the
        // Gaussian filter's ISI), and the phase still advances pi/2 per symbol in total.
        let mut m = Cpm::gmsk(0.3, 4.0).unwrap();
        let mut y = Vec::new();
        let mut lv = vec![1.0f32; 20];
        lv.extend([-1.0, 1.0, -1.0, 1.0, -1.0, 1.0]);
        lv.extend(vec![1.0f32; 20]);
        m.push(&lv, &mut y);
        m.flush(&mut y);
        let f = freq(&y);
        let centre = |k: usize| f[m.symbol_centre(k as u64).round() as usize];
        assert!(
            (centre(10) - 1.0 / 16.0).abs() < 1e-4,
            "steady {}",
            centre(10)
        );
        let alt = centre(22).abs();
        assert!(
            alt < 0.8 / 16.0 && alt > 0.4 / 16.0,
            "alternating peak {alt}"
        );
        let total: f64 = f.iter().sum();
        let want = lv.iter().map(|&a| a as f64).sum::<f64>() / 4.0;
        assert!(
            (total - want).abs() < 1e-3,
            "total phase {total} cycles, want {want}"
        );
    }

    #[test]
    fn four_level_fsk_by_deviation() {
        // 4800 Bd, levels +-1 and +-3 with 600 Hz per unit at 48 kHz (a C4FM-style 4FSK): a run of each level is at its
        // frequency.
        let mut m =
            Cpm::with_deviation(Shape::Rc { beta: 0.2 }, 12.0, 600.0, 4800.0, 48_000.0).unwrap();
        let mut lv = Vec::new();
        for a in [3.0f32, 1.0, -1.0, -3.0] {
            lv.extend(std::iter::repeat_n(a, 24));
        }
        let mut y = Vec::new();
        m.push(&lv, &mut y);
        m.flush(&mut y);
        let f = freq(&y);
        for (i, a) in [3.0f64, 1.0, -1.0, -3.0].iter().enumerate() {
            let k = 24 * i + 12;
            let hz = f[m.symbol_centre(k as u64).round() as usize] * 48_000.0;
            assert!(
                (hz - 600.0 * a).abs() < 0.005 * 600.0 * a.abs() + 1.0,
                "level {a}: {hz} Hz"
            );
        }
        assert!((m.h() - 0.25).abs() < 1e-12);
    }

    #[test]
    fn afsk_tones_switch_at_bit_edges_without_phase_jumps() {
        // Bell 202 at 48 kHz: 40 samples per bit; inside each bit the audio is a pure tone of the bit's frequency, and the audio's
        // analytic phase never jumps.
        let mut a = Afsk::new(48_000.0, 1200.0, 1200.0, 2200.0).unwrap();
        let bits = [1u8, 0, 0, 1, 0, 1, 1, 0];
        let mut y = Vec::new();
        a.push(&bits, &mut y);
        assert_eq!(y.len(), 320);
        for (k, &b) in bits.iter().enumerate() {
            // Least-squares fit of a cos + b sin at each tone over the middle of the bit: the bit's own tone leaves no residual,
            // the other tone leaves most of the energy.
            let resid = |f: f64| {
                let idx = k * 40 + 5..k * 40 + 35;
                let (mut cc, mut ss, mut cs, mut yc, mut ys, mut yy) =
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
                for n in idx {
                    let (sn, cn) = (TAU * f * n as f64 / 48_000.0).sin_cos();
                    let v = y[n] as f64;
                    cc += cn * cn;
                    ss += sn * sn;
                    cs += cn * sn;
                    yc += v * cn;
                    ys += v * sn;
                    yy += v * v;
                }
                let det = cc * ss - cs * cs;
                let a = (yc * ss - ys * cs) / det;
                let b = (ys * cc - yc * cs) / det;
                (yy - a * yc - b * ys) / yy
            };
            let (own, other) = if b == 1 {
                (1200.0, 2200.0)
            } else {
                (2200.0, 1200.0)
            };
            assert!(resid(own) < 1e-5, "bit {k}: residual {}", resid(own));
            assert!(resid(other) > 0.1, "bit {k}: the other tone fits too well");
        }
        // Continuity: the largest step between samples is what the higher tone allows.
        let max_step = y
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(max_step <= (TAU * 2200.0 / 48_000.0) as f32 + 1e-3);
        assert!(Afsk::new(48_000.0, 1200.0, 1200.0, 1200.0).is_err());
    }

    #[test]
    fn any_cut_returns_the_same_samples() {
        let mut rng = Rng::new(1);
        let lv: Vec<f32> = (0..300)
            .map(|_| if rng.bit() == 1 { 1.0 } else { -1.0 })
            .collect();
        let mut a = Cpm::gfsk(0.5, 0.5, 4.3).unwrap();
        let mut b = Cpm::gfsk(0.5, 0.5, 4.3).unwrap();
        let (mut ya, mut yb) = (Vec::new(), Vec::new());
        a.push(&lv, &mut ya);
        for c in lv.chunks(7) {
            b.push(c, &mut yb);
        }
        assert_eq!(ya, yb);
        assert!(Cpm::gfsk(0.0, 0.5, 4.0).is_err());
        assert!(Cpm::new(Shape::Rect, 1.0, f64::NAN, 4.0).is_err());
    }

    /// The instantaneous frequency at sample `n`, in cycles per sample, without regard to amplitude.
    fn freq_at(x: &[Complex32], n: usize) -> f64 {
        let d = x[n + 1] * x[n].conj();
        (d.im as f64).atan2(d.re as f64) / TAU
    }

    #[test]
    fn a_ramp_has_the_raised_cosine_and_linear_shapes_at_both_ends() {
        // CPFSK has unit amplitude, so |y[i]| is the envelope itself. Raised cosine over r = 16 samples: weight
        // (1 - cos(pi i / r)) / 2, so 0 at i = 0, (1 - cos(pi / 4)) / 2 = 0.1464466 at i = 4, 0.5 at i = 8, 1 from i = 16 on. The
        // fall ends one sample after the last one, so sample n - 1 - i has the weight of i + 1.
        let lv = [1.0f32, -1.0, 1.0, 1.0, -1.0, -1.0];
        let mut m = Cpm::cpfsk(0.5, 64.0)
            .unwrap()
            .with_ramp(Ramp::RaisedCosine(16.0))
            .unwrap();
        let mut y = Vec::new();
        m.push(&lv, &mut y);
        m.flush(&mut y);
        assert_eq!(y.len(), 6 * 64);
        let n = y.len();
        for (i, want) in [
            (0usize, 0.0f32),
            (4, 0.146_446_6),
            (8, 0.5),
            (12, 0.853_553_4),
            (16, 1.0),
            (100, 1.0),
        ] {
            assert!(
                (y[i].norm() - want).abs() < 1e-5,
                "start {i}: {}",
                y[i].norm()
            );
            let want_end = if i + 1 >= 16 {
                1.0
            } else {
                (1.0 - (std::f32::consts::PI * (i as f32 + 1.0) / 16.0).cos()) / 2.0
            };
            assert!(
                (y[n - 1 - i].norm() - want_end).abs() < 1e-5,
                "end {i}: {}",
                y[n - 1 - i].norm()
            );
        }
        // The ramp changes the amplitude only: the phase is the same as without it.
        let mut plain = Cpm::cpfsk(0.5, 64.0).unwrap();
        let mut z = Vec::new();
        plain.push(&lv, &mut z);
        plain.flush(&mut z);
        assert_eq!(z.len(), n);
        for i in 1..n - 1 {
            let d = y[i] * z[i].conj();
            assert!(d.im.abs() < 1e-5 * d.norm().max(1e-3) + 1e-6, "sample {i}");
        }
        // Linear over 10 samples: |y[i]| = i / 10.
        let mut l = Cpm::cpfsk(0.5, 64.0)
            .unwrap()
            .with_ramp(Ramp::Linear(10.0))
            .unwrap();
        let mut w = Vec::new();
        l.push(&lv, &mut w);
        l.flush(&mut w);
        for i in 0..12 {
            let want = (i as f32 / 10.0).min(1.0);
            assert!((w[i].norm() - want).abs() < 1e-5, "{i}");
            let want_end = ((i + 1) as f32 / 10.0).min(1.0);
            assert!((w[n - 1 - i].norm() - want_end).abs() < 1e-5, "{i}");
        }
        // The slice helper produces the same envelope on a finished burst.
        let mut s = z.clone();
        Ramp::RaisedCosine(16.0).apply(&mut s);
        for i in 0..n {
            assert!((s[i] - y[i]).norm() < 1e-5, "{i}");
        }
        assert!(Cpm::msk(4.0)
            .unwrap()
            .with_ramp(Ramp::Linear(-1.0))
            .is_err());
        assert!(Cpm::msk(4.0)
            .unwrap()
            .with_ramp(Ramp::RaisedCosine(f64::NAN))
            .is_err());
    }

    #[test]
    fn held_edges_run_from_the_start_of_symbol_0_to_the_end_of_the_last() {
        // GFSK BT 1, h 1, 32 samples per symbol (an FT4-like pulse spanning 3 symbols). With held edges a burst of N symbols has
        // N x 32 samples, and the frequency over the first and last symbol is that of their level (h a / 2 symbol rates),
        // because the levels beyond the ends are copies. Without held edges the output starts where symbol 0's pulse starts,
        // 1.5 symbols early, at the carrier.
        let sps = 32.0;
        let lv = [3.0f32, -1.0, 1.0, -3.0, 3.0, 1.0, -1.0, -3.0];
        let mut m = Cpm::gfsk(1.0, 1.0, sps).unwrap().with_held_edges();
        let mut y = Vec::new();
        m.push(&lv, &mut y);
        m.flush(&mut y);
        assert_eq!(y.len(), lv.len() * 32);
        let want = |a: f32| a as f64 / 2.0 / sps;
        // The first two samples and the centre of symbol 0 are at its tone, and so is the end of the last symbol.
        for n in [0usize, 1, 16] {
            assert!(
                (freq_at(&y, n) - want(3.0)).abs() < 0.02 * want(3.0).abs(),
                "start {n}: {}",
                freq_at(&y, n)
            );
        }
        let e = y.len() - 2;
        assert!((freq_at(&y, e) - want(-3.0)).abs() < 0.02 * want(-3.0).abs());
        let mut plain = Cpm::gfsk(1.0, 1.0, sps).unwrap();
        let mut z = Vec::new();
        plain.push(&lv, &mut z);
        assert!(freq_at(&z, 0).abs() < 0.01 * want(3.0));
        // Fractional samples per symbol: ceil(N sps) samples.
        let mut f = Cpm::gfsk(2.0, 1.0, 10.3).unwrap().with_held_edges();
        let mut v = Vec::new();
        f.push(&lv, &mut v);
        f.flush(&mut v);
        assert_eq!(v.len(), (8.0f64 * 10.3).ceil() as usize);
        // A second burst has the same length.
        let mut v2 = Vec::new();
        f.push(&lv, &mut v2);
        f.flush(&mut v2);
        assert_eq!(v2.len(), v.len());
    }

    #[test]
    fn ramped_bursts_are_the_same_for_any_cut() {
        let mut rng = Rng::new(9);
        let lv: Vec<f32> = (0..60).map(|_| (rng.below(8) as f32) * 2.0 - 7.0).collect();
        let make = || {
            Cpm::gfsk(2.0, 1.0, 12.5)
                .unwrap()
                .with_held_edges()
                .with_ramp(Ramp::RaisedCosine(1.56))
                .unwrap()
        };
        let (mut a, mut b) = (make(), make());
        let (mut ya, mut yb) = (Vec::new(), Vec::new());
        for _ in 0..2 {
            a.push(&lv, &mut ya);
            a.flush(&mut ya);
            for c in lv.chunks(7) {
                b.push(c, &mut yb);
            }
            b.flush(&mut yb);
        }
        assert_eq!(ya, yb);
        assert_eq!(ya.len(), 2 * 750);
    }
}
