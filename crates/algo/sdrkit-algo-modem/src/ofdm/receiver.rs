//! The streaming OFDM receiver: [`CpSync`] finds the symbols in a stream of samples and passes them on whole, with the carrier
//! offset removed; [`OfdmReceiver`] turns whole symbols into equalised cells with their channel state. Both are driven by an
//! [`OfdmLayout`] and keep their state between calls, so the output does not depend on how the input is split.

use super::{
    equalise, integer_cfo, CfoMetric, ChannelEstimator, FftFramer, LagCorrelator, LagPeak,
    OfdmLayout, PilotLs, TimeFreq,
};
use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::loopfilter::LoopFilter;
use sdrkit_algo_dsp::nco::Nco;

/// Symbols without a prefix peak before [`CpSync`] drops the lock.
const MAX_MISSES: u64 = 8;
/// Prefix lengths after a symbol's end before [`CpSync`] cuts it (its prefix peak is reported once the correlation has fallen
/// again).
const DELAY: u64 = 2;
/// Most prefix peaks one step of input can close.
const MAX_PEAKS: usize = 64;

/// Settings of [`CpSync`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CpSyncConfig {
    /// The normalised prefix correlation (0 to 1] above which a symbol is found; the peak closes below half of it.
    pub threshold: f32,
    /// The largest whole-carrier offset searched at lock, in carriers.
    pub max_shift: u32,
}

impl Default for CpSyncConfig {
    /// Threshold 0.7, offsets up to 2 carriers.
    fn default() -> Self {
        CpSyncConfig {
            threshold: 0.7,
            max_shift: 2,
        }
    }
}

/// What [`CpSync::process`] did, in output order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CpSyncEvent {
    /// The lock was gained (just before the first symbol of the lock, at `out_at`) or lost (where the output stood then).
    Lock {
        /// Gained or lost.
        locked: bool,
        /// The output index it happened at.
        out_at: usize,
    },
    /// A whole symbol, prefix first, was appended at `out[out_at..out_at + len]`.
    Symbol {
        /// The input position (samples since the receiver was made) of its first sample.
        start: u64,
        /// Its length, prefix included.
        len: usize,
        /// Where it starts in the output.
        out_at: usize,
        /// Its number since the lock was gained (0 is the first: a frame start).
        index: u64,
        /// How far after `start` the symbol truly begins, in samples (`-0.5` to `0.5`, and always 0 without the clock loop of
        /// [`CpSync::enable_clock_loop`]). Give it to `FftFramer::set_timing`, which takes it off as a phase slope.
        timing: f64,
    },
}

/// Cyclic-prefix symbol synchronisation for OFDM in a stream (the receiver of `modem.ofdm_sync`).
///
/// The prefix is found by a [`LagCorrelator`] (lag = FFT size, window = prefix), which also measures the fractional carrier
/// offset. At the first symbol found, the whole-carrier offset is measured over that symbol's FFT ([`integer_cfo`] with
/// [`CfoMetric::Power`] over the used carriers that are not nulls, within `max_shift` carriers) and the stream locks: from then on
/// symbols are cut on a fixed grid, kept by the following prefix peaks (moved when one lands more than a quarter of the prefix
/// away), and the fractional offset is smoothed over them (one tenth of each new measurement). After 8 symbols
/// with no peak the lock is dropped.
///
/// Symbols leave whole, prefix first, with the offset removed by a phase continuous over the stream. A symbol leaves two prefix
/// lengths after its last sample: every decision then happens at a fixed input position, so the output does not depend on how
/// the input is cut. Two symbols of history are kept; nothing is allocated once the buffers have grown to their working size.
///
/// ```
/// use sdrkit_algo_modem::ofdm::{CpSync, CpSyncConfig, CpSyncEvent, OfdmLayout};
/// use sdrkit_algo_modem::Complex32;
/// let layout = OfdmLayout::ieee80211a();
/// // Thirty symbols of a repeating pseudo-random pattern, each with its 16-sample cyclic prefix, after some silence.
/// let mut x = vec![Complex32::default(); 200];
/// for s in 0..30u32 {
///     let body: Vec<Complex32> = (0..64u32)
///         .map(|i| {
///             let v = (i * 97 + s * 31) % 13;
///             Complex32::new(v as f32 - 6.0, ((i * 7 + s) % 5) as f32 - 2.0)
///         })
///         .collect();
///     x.extend_from_slice(&body[48..]);
///     x.extend_from_slice(&body);
/// }
/// let mut sync = CpSync::new(&layout, CpSyncConfig::default()).unwrap();
/// let (mut out, mut events) = (Vec::new(), Vec::new());
/// sync.process(&x, &mut out, &mut events);
/// assert!(sync.locked());
/// assert!(matches!(events[0], CpSyncEvent::Lock { locked: true, out_at: 0 }));
/// assert!(matches!(events[1], CpSyncEvent::Symbol { start: 200, len: 80, out_at: 0, index: 0, .. }));
/// ```
#[derive(Clone, Debug)]
pub struct CpSync {
    layout: OfdmLayout,
    cfg: CpSyncConfig,
    longest: usize,
    corr: LagCorrelator,
    framer: FftFramer,
    known: Vec<(i32, Complex32)>,
    cells: Vec<Complex32>,
    peaks: Vec<LagPeak>,
    hist: Vec<Complex32>,
    /// Input position of `hist[0]`; `hist_start + hist.len()` is the position of the next sample.
    hist_start: u64,
    /// Input position of the first sample given to the correlator.
    corr_origin: u64,
    locked: bool,
    next_sym: u64,
    /// Symbols cut so far in this lock (picks the prefix of each in the layout's cycle).
    sym_count: u64,
    /// Start of the last symbol a prefix peak confirmed.
    confirmed: u64,
    frac: f64,
    shift: i32,
    nco: Nco,
    frame_start: bool,
    clock: Option<ClockLoop>,
}

/// The clock loop of a [`CpSync`]: the exact start of the next symbol and a second-order loop on its timing errors.
#[derive(Clone, Debug)]
struct ClockLoop {
    lf: LoopFilter,
    /// The exact input position of the next symbol to cut.
    grid: f64,
    /// The largest clock error, as a fraction.
    max: f64,
    /// Whether the prefix peaks feed the loop.
    from_peaks: bool,
}

impl CpSync {
    /// A synchroniser for `layout`. Errors for a threshold outside `(0, 1]`.
    pub fn new(layout: &OfdmLayout, cfg: CpSyncConfig) -> Result<Self, ModemError> {
        if !(cfg.threshold > 0.0 && cfg.threshold <= 1.0) {
            return Param::Threshold.range(0.0, 1.0, cfg.threshold);
        }
        let (nfft, cp) = (layout.nfft(), layout.cp(0));
        let nulls = layout.nulls();
        let known = (0..layout.used())
            .map(|i| layout.carrier(i))
            .filter(|c| !nulls.contains(c))
            .map(|c| (c, Complex32::new(1.0, 0.0)))
            .collect();
        let longest = layout.longest_symbol();
        Ok(CpSync {
            layout: layout.clone(),
            cfg,
            longest,
            corr: LagCorrelator::new(nfft, cp, cfg.threshold, 0.5 * cfg.threshold)?,
            framer: FftFramer::new(layout, 0)?,
            known,
            cells: vec![Complex32::default(); layout.used()],
            peaks: Vec::with_capacity(MAX_PEAKS),
            hist: Vec::with_capacity(4 * longest + 65_536),
            hist_start: 0,
            corr_origin: 0,
            locked: false,
            next_sym: 0,
            sym_count: 0,
            confirmed: 0,
            frac: 0.0,
            shift: 0,
            nco: Nco::new(1.0, 0.0),
            frame_start: false,
            clock: None,
        })
    }

    /// Turns on the clock loop: from then on the symbol grid is kept in fractions of a sample and moved by a second-order loop of
    /// noise bandwidth `bn_t` per symbol (damping 1/sqrt(2)) on the timing error of every prefix peak, measured to a fraction of a
    /// sample by a triangle fit through the largest prefix correlation near the peak and its neighbours. The loop's integrator is the sampling clock
    /// error, held within `max_ppm`. The grid never jumps: symbols are cut at the nearest whole sample, and each
    /// [`CpSyncEvent::Symbol`] says how far the true start lies after the cut (`timing`). [`CpSync::timing_error`] feeds the loop a
    /// better measurement when the caller has one (the timing slope of `phase_fit` over the pilots). Errors for `bn_t` outside
    /// `(0, 0.5)` or `max_ppm` outside `[0, 10000]`.
    pub fn enable_clock_loop(&mut self, bn_t: f64, max_ppm: f64) -> Result<(), ModemError> {
        if !(0.0..=10_000.0).contains(&max_ppm) {
            return Param::MaxPpm.range(0.0, 10_000.0, max_ppm);
        }
        let mut lf = LoopFilter::new(bn_t, std::f64::consts::FRAC_1_SQRT_2)?;
        let len = self.layout.symbol_len(0) as f64;
        lf.set_clamp(max_ppm * 1e-6 * len);
        self.clock = Some(ClockLoop {
            lf,
            grid: self.next_sym as f64,
            max: max_ppm * 1e-6,
            from_peaks: true,
        });
        Ok(())
    }

    /// Whether the prefix peaks feed the clock loop (on by default). A caller that feeds a finer measurement through
    /// [`CpSync::timing_error`] turns them off. The peaks then only keep the lock.
    pub fn set_peak_timing(&mut self, on: bool) {
        if let Some(c) = &mut self.clock {
            c.from_peaks = on;
        }
    }

    /// The sampling clock error the loop has found, in ppm (positive: symbols longer than nominal). 0 without the clock loop.
    pub fn clock_ppm(&self) -> f64 {
        self.clock.as_ref().map_or(0.0, |c| {
            c.lf.integrator() / self.layout.symbol_len(0) as f64 * 1e6
        })
    }

    /// Feeds the clock loop a timing error of the symbols being cut, in samples (positive: they start later than the grid), for
    /// example the timing slope `phase_fit` measures over the pilots. Ignored without the clock loop, when not locked, or for a value
    /// that is not finite.
    pub fn timing_error(&mut self, samples: f64) {
        if self.locked && samples.is_finite() {
            self.loop_update(samples);
        }
    }

    fn loop_update(&mut self, err: f64) {
        let Some(c) = &mut self.clock else { return };
        let len = self.layout.symbol_len(0) as f64;
        let e = err.clamp(-0.5 * len, 0.5 * len);
        let v = c.lf.update(e);
        c.grid += v - c.lf.integrator();
        self.next_sym = c.grid.round().max(0.0) as u64;
    }

    /// Where the prefix correlation `|sum x[i] conj(x[i - nfft])|` (over the prefix-long window ending at a position) peaks near
    /// `end`, the end the lag correlator reported: the largest value within half a prefix either side, refined by a triangle
    /// through it and its neighbours (a prefix correlation rises and falls linearly about its peak for one path). Returns the
    /// offset from `end` in samples, or 0 when the history does not hold the stretch.
    fn peak_offset(&self, end: u64) -> f64 {
        let (nfft, cp) = (self.layout.nfft() as u64, self.layout.cp(0) as u64);
        let reach = (cp / 2).max(1);
        let (Some(lo), hi) = (end.checked_sub(reach), end + reach) else {
            return 0.0;
        };
        // The first sample of the first window, and the history must hold every sample up to `hi`.
        let Some(first) = (lo + 1).checked_sub(cp + nfft) else {
            return 0.0;
        };
        if first < self.hist_start || hi >= self.position() {
            return 0.0;
        }
        let h = &self.hist;
        let at = |abs: u64| (abs - self.hist_start) as usize;
        let n = nfft as usize;
        let term = |i: usize| {
            let p = h[i + n] * h[i].conj();
            (p.re as f64, p.im as f64)
        };
        // Window ending at `lo`: products for i in first ..= first + cp - 1 (the later sample at i + nfft).
        let (mut re, mut im) = (0.0f64, 0.0f64);
        let a = at(first);
        for i in a..a + cp as usize {
            let (r, m) = term(i);
            re += r;
            im += m;
        }
        let count = (hi - lo + 1) as usize;
        let mut best = (f64::NEG_INFINITY, 0usize);
        let mut vals = [0f64; 3];
        let mut prev = 0f64;
        let mut after_best = None;
        for k in 0..count {
            if k > 0 {
                let (r, m) = term(a + cp as usize + k - 1);
                let (r0, m0) = term(a + k - 1);
                re += r - r0;
                im += m - m0;
            }
            let v = (re * re + im * im).sqrt();
            if v > best.0 {
                best = (v, k);
                vals = [prev, v, 0.0];
                after_best = None;
            } else if after_best.is_none() && k == best.1 + 1 {
                after_best = Some(v);
            }
            prev = v;
        }
        let k = best.1;
        let whole = k as f64 - reach as f64;
        let (Some(r), true) = (after_best, k > 0) else {
            return whole;
        };
        vals[2] = r;
        let (l, c) = (vals[0], vals[1]);
        let lo_v = l.min(r);
        if c <= lo_v {
            return whole;
        }
        whole + ((r - l) / (2.0 * (c - lo_v))).clamp(-0.5, 0.5)
    }

    /// The layout.
    pub fn layout(&self) -> &OfdmLayout {
        &self.layout
    }

    /// Whether the stream is locked.
    pub fn locked(&self) -> bool {
        self.locked
    }

    /// The carrier offset removed, in cycles per sample (fractional plus whole carriers over the FFT size).
    pub fn freq(&self) -> f64 {
        self.frac + self.shift as f64 / self.layout.nfft() as f64
    }

    /// The input position of the next sample (samples taken since the receiver was made).
    pub fn position(&self) -> u64 {
        self.hist_start + self.hist.len() as u64
    }

    /// Starts afresh at the current position (after a retune or a gap): forgets the history and the lock. Returns whether a
    /// lock was dropped.
    pub fn reset(&mut self) -> bool {
        let was = self.locked;
        self.locked = false;
        self.frame_start = false;
        self.corr.reset();
        self.hist_start = self.position();
        self.corr_origin = self.hist_start;
        self.hist.clear();
        was
    }

    /// Takes the next input samples: appends every symbol that becomes due to `out` and what happened to `events` (module
    /// documentation). The input is worked through in steps that end at fixed stream positions (the next symbol's due point when
    /// locked, multiples of the prefix otherwise), so the result does not depend on the cuts.
    pub fn process(
        &mut self,
        x: &[Complex32],
        out: &mut Vec<Complex32>,
        events: &mut Vec<CpSyncEvent>,
    ) {
        let cp = self.layout.cp(0) as u64;
        let mut i = 0;
        while i < x.len() {
            let end = self.position();
            let boundary = if self.locked {
                self.due(self.layout.symbol_len(self.sym_count) as u64)
            } else {
                (end / cp + 1) * cp
            };
            let take = (boundary.saturating_sub(end).max(1) as usize).min(x.len() - i);
            let step = &x[i..i + take];
            self.hist.extend_from_slice(step);
            self.peaks.clear();
            let peaks = &mut self.peaks;
            self.corr.process(step, |p| {
                if peaks.len() < peaks.capacity() {
                    peaks.push(p)
                }
            });
            for k in 0..self.peaks.len() {
                let p = self.peaks[k];
                self.on_peak(p);
            }
            self.emit(out, events);
            // Keep two symbols of history, and everything from the next symbol to cut.
            let end = self.position();
            let mut keep = end.saturating_sub(2 * self.longest as u64);
            if self.locked {
                keep = keep.min(self.next_sym);
            }
            if keep > self.hist_start {
                self.hist.drain(..(keep - self.hist_start) as usize);
                self.hist_start = keep;
            }
            i += take;
        }
    }

    /// The input position after which the next symbol (of `len` samples) is cut.
    fn due(&self, len: u64) -> u64 {
        self.next_sym + len + DELAY * self.layout.cp(0) as u64
    }

    /// The whole-carrier offset of the symbol whose prefix starts at input position `start`.
    fn integer_offset(&mut self, start: u64) -> i32 {
        let rel = (start - self.hist_start) as usize;
        self.framer.set_freq(self.frac);
        if self
            .framer
            .demodulate(&self.hist[rel..], self.layout.cp(0), start, &mut self.cells)
            .is_err()
        {
            return 0;
        }
        integer_cfo(
            self.framer.bins(),
            &self.known,
            self.cfg.max_shift as i32,
            CfoMetric::Power,
        )
        .map_or(0, |c| c.shift)
    }

    fn on_peak(&mut self, p: LagPeak) {
        let (nfft, cp, len) = (
            self.layout.nfft() as u64,
            self.layout.cp(0) as u64,
            self.layout.symbol_len(0) as i64,
        );
        let end = self.corr_origin + p.end;
        let Some(start) = (end + 1).checked_sub(nfft + cp) else {
            return;
        };
        if !self.locked {
            if start < self.hist_start {
                return;
            }
            self.frac = p.freq;
            self.next_sym = start;
            self.shift = self.integer_offset(start);
            self.locked = true;
            self.sym_count = 0;
            self.confirmed = start;
            self.frame_start = true;
            if self.clock.is_some() {
                let f = self.peak_offset(end);
                if let Some(c) = &mut self.clock {
                    c.lf.reset();
                    c.lf.set_clamp(c.max * len as f64);
                    c.grid = start as f64 + f;
                }
            }
            return;
        }
        if let Some((grid, from_peaks)) = self.clock.as_ref().map(|c| (c.grid, c.from_peaks)) {
            // Where the peak lands against the exact grid, within half a symbol.
            let measured = start as f64 + self.peak_offset(end);
            let d = (measured - grid).rem_euclid(len as f64);
            let d = if d > len as f64 / 2.0 {
                d - len as f64
            } else {
                d
            };
            if d.abs() <= (cp / 2) as f64 {
                self.confirmed = self.confirmed.max(start);
                self.frac += 0.1 * (p.freq - self.frac);
                if from_peaks {
                    self.loop_update(d);
                }
            }
            return;
        }
        // Where the peak lands on the grid of symbols, within half a symbol.
        let mut d = (start as i64 - self.next_sym as i64).rem_euclid(len);
        if d > len / 2 {
            d -= len;
        }
        if d.unsigned_abs() <= cp / 2 {
            self.confirmed = self.confirmed.max(start);
            self.frac += 0.1 * (p.freq - self.frac);
            if d.unsigned_abs() > cp / 4 {
                self.next_sym = (self.next_sym as i64 + d) as u64;
            }
        }
    }

    /// Cuts every symbol whose samples have all arrived and whose due point has passed.
    fn emit(&mut self, out: &mut Vec<Complex32>, events: &mut Vec<CpSyncEvent>) {
        let hist_end = self.position();
        while self.locked {
            let len = self.layout.symbol_len(self.sym_count) as u64;
            if self.next_sym < self.hist_start || self.due(len) > hist_end {
                break;
            }
            if self.next_sym > self.confirmed + MAX_MISSES * len {
                self.locked = false;
                events.push(CpSyncEvent::Lock {
                    locked: false,
                    out_at: out.len(),
                });
                break;
            }
            let rel = (self.next_sym - self.hist_start) as usize;
            let f = self.freq();
            let o0 = out.len();
            out.resize(o0 + len as usize, Complex32::default());
            self.nco.set_freq(-f);
            self.nco
                .set_phase_cycles((-f * self.next_sym as f64).rem_euclid(1.0));
            self.nco
                .mix(&self.hist[rel..rel + len as usize], &mut out[o0..]);
            if self.frame_start {
                self.frame_start = false;
                events.push(CpSyncEvent::Lock {
                    locked: true,
                    out_at: o0,
                });
            }
            let timing = self
                .clock
                .as_ref()
                .map_or(0.0, |c| c.grid - self.next_sym as f64);
            events.push(CpSyncEvent::Symbol {
                start: self.next_sym,
                len: len as usize,
                out_at: o0,
                index: self.sym_count,
                timing,
            });
            self.sym_count += 1;
            match &mut self.clock {
                Some(c) => {
                    // Each symbol lasts its nominal length plus the clock error the loop holds (per nominal symbol).
                    let nominal = self.layout.symbol_len(0) as f64;
                    c.grid += len as f64 * (1.0 + c.lf.integrator() / nominal);
                    self.next_sym = c.grid.round().max(0.0) as u64;
                }
                None => self.next_sym += len,
            }
        }
    }
}

/// The channel estimators of this module chosen at run time (by name, for settings), dispatched by a `match` rather than `dyn`.
#[derive(Clone, Debug)]
pub enum Estimator {
    /// [`PilotLs`], raw or smoothed.
    PilotLs(PilotLs),
    /// [`TimeFreq`].
    TimeFreq(TimeFreq),
}

impl Estimator {
    /// The names [`by_name`](Self::by_name) accepts: `pilot_ls` ([`PilotLs`] raw), `pilot_ls_smooth` ([`PilotLs`] smoothed over
    /// neighbouring pilots), `time_freq` ([`TimeFreq`]).
    pub const NAMES: [&'static str; 3] = ["pilot_ls", "pilot_ls_smooth", "time_freq"];

    /// The estimator called `name` for `layout`. Errors for an unknown name, or when `layout` does not suit it.
    pub fn by_name(name: &str, layout: &OfdmLayout) -> Result<Self, ModemError> {
        match name {
            "pilot_ls" => Ok(Estimator::PilotLs(PilotLs::new(layout, false))),
            "pilot_ls_smooth" => Ok(Estimator::PilotLs(PilotLs::new(layout, true))),
            "time_freq" => Ok(Estimator::TimeFreq(TimeFreq::new(layout)?)),
            _ => Param::Estimator.unknown_name(),
        }
    }
}

impl ChannelEstimator for Estimator {
    fn delay(&self) -> usize {
        match self {
            Estimator::PilotLs(e) => e.delay(),
            Estimator::TimeFreq(e) => e.delay(),
        }
    }
    fn push(&mut self, symbol: u64, cells: &[Complex32]) -> Result<bool, ModemError> {
        match self {
            Estimator::PilotLs(e) => e.push(symbol, cells),
            Estimator::TimeFreq(e) => e.push(symbol, cells),
        }
    }
    fn output_symbol(&self) -> u64 {
        match self {
            Estimator::PilotLs(e) => e.output_symbol(),
            Estimator::TimeFreq(e) => e.output_symbol(),
        }
    }
    fn cells(&self) -> &[Complex32] {
        match self {
            Estimator::PilotLs(e) => e.cells(),
            Estimator::TimeFreq(e) => e.cells(),
        }
    }
    fn h(&self) -> &[Complex32] {
        match self {
            Estimator::PilotLs(e) => e.h(),
            Estimator::TimeFreq(e) => e.h(),
        }
    }
    fn noise(&self) -> f32 {
        match self {
            Estimator::PilotLs(e) => e.noise(),
            Estimator::TimeFreq(e) => e.noise(),
        }
    }
    fn reset(&mut self) {
        match self {
            Estimator::PilotLs(e) => e.reset(),
            Estimator::TimeFreq(e) => e.reset(),
        }
    }
}

/// One equalised OFDM symbol from [`OfdmReceiver`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OfdmSymbol<'a> {
    /// The symbol's number in the layout's cycle (counted from the first symbol given at the last restart).
    pub symbol: u64,
    /// The input position (samples since the receiver was made) of the symbol's first sample.
    pub start: u64,
    /// Every used carrier (`layout.first() ..= layout.last()`, nulls and reserved carriers included), equalised by zero forcing.
    pub cells: &'a [Complex32],
    /// The weight of each cell, `|H|^2 / noise` (0 for a cell in a deep fade: an erasure), for
    /// `Constellation::demap_block_weighted`.
    pub weights: &'a [f32],
    /// The noise variance per cell the estimator measured (`E|n|^2`, before equalisation; at least 1e-9).
    pub noise: f32,
    /// The mean `|H|^2` over the noise.
    pub snr: f32,
}

/// Whole OFDM symbols (from [`CpSync`], back to back, prefix first) to equalised cells: each symbol goes through an
/// [`FftFramer`] (window `backoff` samples early, its phase ramp removed), the channel estimator `E` and [`equalise`] (zero
/// forcing with channel state information). The receiver of `modem.ofdm_demod`, and of any OFDM stack that keeps only its tables.
///
/// The symbols are numbered from `first_symbol` (the phase of the pilot pattern) at every [`restart`](Self::restart), which a
/// frame start, a retune or a gap calls. An estimator with a delay outputs each symbol's cells that many symbols later, with the
/// start of the symbol they belong to. The samples of a symbol may arrive in any number of calls.
///
/// ```
/// use sdrkit_algo_modem::ofdm::{Estimator, OfdmLayout, OfdmReceiver};
/// use sdrkit_algo_modem::Complex32;
/// let layout = OfdmLayout::ieee80211a();
/// let est = Estimator::by_name("pilot_ls", &layout).unwrap();
/// let mut rx = OfdmReceiver::new(&layout, est, 2, 0).unwrap();
/// let x = vec![Complex32::new(0.1, 0.0); 3 * 80]; // three symbols
/// let mut got = Vec::new();
/// rx.process(&x[..100], |s| got.push((s.symbol, s.start, s.cells.len())));
/// rx.process(&x[100..], |s| got.push((s.symbol, s.start, s.cells.len())));
/// assert_eq!(got, [(0, 0, 53), (1, 80, 53), (2, 160, 53)]);
/// ```
#[derive(Clone, Debug)]
pub struct OfdmReceiver<E: ChannelEstimator> {
    layout: OfdmLayout,
    framer: FftFramer,
    est: E,
    buf: Vec<Complex32>,
    cells: Vec<Complex32>,
    z: Vec<Complex32>,
    w: Vec<f32>,
    /// The start of each symbol still inside the estimator, by symbol number modulo its length.
    starts: Vec<u64>,
    first_symbol: u64,
    symbol: u64,
    /// Input position of the next sample, and of the first sample of the symbol being gathered.
    position: u64,
    start: u64,
    snr: f32,
}

impl<E: ChannelEstimator> OfdmReceiver<E> {
    /// A receiver for `layout` with estimator `est`, the FFT window `backoff` samples early (at most the shortest prefix), and
    /// symbols numbered from `first_symbol`.
    pub fn new(
        layout: &OfdmLayout,
        est: E,
        backoff: usize,
        first_symbol: u64,
    ) -> Result<Self, ModemError> {
        let used = layout.used();
        let depth = est.delay() + 1;
        Ok(OfdmReceiver {
            layout: layout.clone(),
            framer: FftFramer::new(layout, backoff)?,
            est,
            buf: Vec::with_capacity(layout.longest_symbol()),
            cells: vec![Complex32::default(); used],
            z: vec![Complex32::default(); used],
            w: vec![0.0; used],
            starts: vec![0; depth],
            first_symbol,
            symbol: first_symbol,
            position: 0,
            start: 0,
            snr: 0.0,
        })
    }

    /// The layout.
    pub fn layout(&self) -> &OfdmLayout {
        &self.layout
    }

    /// The channel estimator.
    pub fn estimator(&self) -> &E {
        &self.est
    }

    /// The mean `|H|^2` over the noise of the last symbol given out (0 before the first).
    pub fn snr(&self) -> f32 {
        self.snr
    }

    /// The input position (samples taken since the receiver was made) of the next sample.
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Starts the symbol count again at the first symbol, drops a part-gathered symbol and clears the estimator. The next sample
    /// is the first of a symbol.
    pub fn restart(&mut self) {
        self.buf.clear();
        self.symbol = self.first_symbol;
        self.est.reset();
    }

    /// Takes the next samples, calling `on_symbol` for every symbol whose equalised cells are ready.
    pub fn process(&mut self, x: &[Complex32], mut on_symbol: impl FnMut(&OfdmSymbol<'_>)) {
        let mut i = 0;
        while i < x.len() {
            let len = self.layout.symbol_len(self.symbol);
            if self.buf.is_empty() {
                self.start = self.position;
            }
            let take = (len - self.buf.len()).min(x.len() - i);
            self.buf.extend_from_slice(&x[i..i + take]);
            i += take;
            self.position += take as u64;
            if self.buf.len() == len {
                self.symbol_done(&mut on_symbol);
                self.symbol += 1;
                self.buf.clear();
            }
        }
    }

    fn symbol_done(&mut self, on_symbol: &mut impl FnMut(&OfdmSymbol<'_>)) {
        let depth = self.starts.len() as u64;
        self.starts[(self.symbol % depth) as usize] = self.start;
        let cp = self.layout.cp(self.symbol);
        if self
            .framer
            .demodulate(&self.buf, cp, 0, &mut self.cells)
            .is_err()
        {
            return;
        }
        if !matches!(self.est.push(self.symbol, &self.cells), Ok(true)) {
            return;
        }
        let noise = self.est.noise().max(1e-9);
        if equalise(
            self.est.cells(),
            self.est.h(),
            noise,
            &mut self.z,
            &mut self.w,
        )
        .is_err()
        {
            return;
        }
        let h = self.est.h();
        let h2 = h.iter().map(|v| v.norm_sqr()).sum::<f32>() / h.len().max(1) as f32;
        self.snr = h2 / noise;
        let symbol = self.est.output_symbol();
        on_symbol(&OfdmSymbol {
            symbol,
            start: self.starts[(symbol % depth) as usize],
            cells: &self.z,
            weights: &self.w,
            noise,
            snr: self.snr,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::noise::Awgn;
    use sdrkit_algo_sim::rng::Rng;

    /// 802.11a symbols: pilots from the layout, QPSK data, nulls empty; returns the samples and the sent cells of each symbol.
    fn wifi_signal(
        symbols: usize,
        lead: usize,
        seed: u64,
    ) -> (Vec<Complex32>, Vec<Vec<Complex32>>) {
        let layout = OfdmLayout::ieee80211a();
        let n = layout.nfft();
        let mut rng = Rng::new(seed);
        let mut x = vec![Complex32::default(); lead];
        let mut sent = Vec::new();
        let mut ifft = sdrkit_algo_dsp::fft::Fft::inverse(n);
        for s in 0..symbols as u64 {
            let mut cells = vec![Complex32::default(); layout.used()];
            for &d in layout.data(s) {
                let (a, b) = (rng.bit(), rng.bit());
                cells[d as usize] = Complex32::new(1.0 - 2.0 * a as f32, 1.0 - 2.0 * b as f32)
                    * std::f32::consts::FRAC_1_SQRT_2;
            }
            for p in layout.pilots(s) {
                cells[p.index as usize] = p.value;
            }
            let mut f = vec![Complex32::default(); n];
            for (i, &c) in cells.iter().enumerate() {
                f[layout.bin(i)] = c;
            }
            ifft.process(&mut f);
            let sc = 1.0 / (n as f32).sqrt();
            x.extend(f[n - layout.cp(s)..].iter().map(|v| v * sc));
            x.extend(f.iter().map(|v| v * sc));
            sent.push(cells);
        }
        (x, sent)
    }

    /// Mixes `x` up by `f` cycles per sample and adds noise of power `nv`.
    fn impair(x: &mut [Complex32], f: f64, nv: f64, seed: u64) {
        for (i, v) in x.iter_mut().enumerate() {
            let p = std::f64::consts::TAU * ((f * i as f64) % 1.0);
            *v *= Complex32::new(p.cos() as f32, p.sin() as f32);
        }
        Awgn::with_power(nv, seed).unwrap().process(x);
    }

    fn run_sync(x: &[Complex32], cuts: &[usize]) -> (Vec<Complex32>, Vec<CpSyncEvent>, CpSync) {
        let layout = OfdmLayout::ieee80211a();
        let mut sync = CpSync::new(&layout, CpSyncConfig::default()).unwrap();
        let (mut out, mut ev) = (Vec::new(), Vec::new());
        let (mut at, mut k) = (0, 0);
        while at < x.len() {
            let step = cuts[k % cuts.len()].min(x.len() - at);
            sync.process(&x[at..at + step], &mut out, &mut ev);
            at += step;
            k += 1;
        }
        (out, ev, sync)
    }

    #[test]
    fn cp_sync_locks_removes_the_offset_and_ignores_cuts() {
        let lead = 300;
        let (mut x, _) = wifi_signal(60, lead, 1);
        // One whole carrier and 0.3 of one: 1.3 / 64 cycles per sample.
        let f = 1.3 / 64.0;
        impair(&mut x, f, 0.01, 2);
        let (out, ev, sync) = run_sync(&x, &[x.len()]);
        assert!(sync.locked());
        assert!(
            (sync.freq() - f).abs() < 0.02 / 64.0,
            "{} vs {f}",
            sync.freq() * 64.0
        );
        assert!(matches!(
            ev[0],
            CpSyncEvent::Lock {
                locked: true,
                out_at: 0
            }
        ));
        let CpSyncEvent::Symbol {
            start,
            len,
            out_at,
            index,
            ..
        } = ev[1]
        else {
            panic!("{:?}", ev[1]);
        };
        assert_eq!((len, out_at, index), (80, 0, 0));
        assert!(start.abs_diff(lead as u64) <= 4, "{start}");
        // Every symbol but the last two (not due yet) leaves, back to back.
        let symbols = ev
            .iter()
            .filter(|e| matches!(e, CpSyncEvent::Symbol { .. }))
            .count();
        assert!((57..=60).contains(&symbols), "{symbols}");
        assert_eq!(out.len(), symbols * 80);
        for cuts in [&[1usize][..], &[7, 1, 333], &[80], &[4096, 3]] {
            let (o2, e2, _) = run_sync(&x, cuts);
            assert_eq!((o2, e2), (out.clone(), ev.clone()), "{cuts:?}");
        }
    }

    #[test]
    fn cp_sync_drops_the_lock_when_the_signal_stops() {
        let (mut x, _) = wifi_signal(20, 200, 3);
        x.extend(vec![Complex32::default(); 80 * 20]);
        Awgn::with_power(0.01, 4).unwrap().process(&mut x);
        let (out, ev, mut sync) = run_sync(&x, &[x.len()]);
        assert!(!sync.locked());
        let lost = ev
            .iter()
            .position(|e| matches!(e, CpSyncEvent::Lock { locked: false, .. }))
            .unwrap();
        // The lock goes after eight symbols with no prefix peak: twenty sent, up to eight more cut from the noise.
        let before = ev[..lost]
            .iter()
            .filter(|e| matches!(e, CpSyncEvent::Symbol { .. }))
            .count();
        assert!((20..=29).contains(&before), "{before}");
        assert_eq!(
            ev[lost],
            CpSyncEvent::Lock {
                locked: false,
                out_at: out.len()
            }
        );
        assert!(!sync.reset());
        assert_eq!(sync.position(), x.len() as u64);
        // A reset while locked reports the lost lock.
        let (y, _) = wifi_signal(10, 100, 5);
        let mut s2 = CpSync::new(&OfdmLayout::ieee80211a(), CpSyncConfig::default()).unwrap();
        s2.process(&y, &mut Vec::new(), &mut Vec::new());
        assert!(s2.locked() && s2.reset() && !s2.locked());
        assert!(CpSync::new(
            &OfdmLayout::ieee80211a(),
            CpSyncConfig {
                threshold: 0.0,
                max_shift: 2
            }
        )
        .is_err());
    }

    #[test]
    fn receiver_returns_the_sent_cells_from_the_synchronised_stream() {
        let lead = 250;
        let (mut x, sent) = wifi_signal(40, lead, 7);
        impair(&mut x, 0.6 / 64.0, 0.003, 8);
        let (out, ev, _) = run_sync(&x, &[x.len()]);
        let layout = OfdmLayout::ieee80211a();
        let est = Estimator::by_name("pilot_ls_smooth", &layout).unwrap();
        let mut rx = OfdmReceiver::new(&layout, est, 4, 0).unwrap();
        let mut got: Vec<(u64, u64, f32)> = Vec::new();
        rx.process(&out, |s| {
            let k = s.symbol as usize;
            let mut e = 0f32;
            for &d in layout.data(s.symbol) {
                e += (s.cells[d as usize] - sent[k][d as usize]).norm_sqr();
            }
            got.push((s.symbol, s.start, e / layout.data(s.symbol).len() as f32));
        });
        let starts: Vec<usize> = ev
            .iter()
            .filter_map(|e| match e {
                CpSyncEvent::Symbol { out_at, .. } => Some(*out_at),
                _ => None,
            })
            .collect();
        assert_eq!(got.len(), starts.len());
        for (k, &(sym, start, evm)) in got.iter().enumerate() {
            assert_eq!((sym, start), (k as u64, starts[k] as u64));
            assert!(evm < 0.05, "symbol {k}: {evm}");
        }
        assert!(rx.snr() > 30.0, "{}", rx.snr());
        // The cells do not depend on how the input is split.
        let make = || {
            OfdmReceiver::new(
                &layout,
                Estimator::by_name("pilot_ls_smooth", &layout).unwrap(),
                4,
                0,
            )
            .unwrap()
        };
        let mut whole = Vec::new();
        make().process(&out, |s| whole.extend_from_slice(s.cells));
        for cut in [1, 13, 80, 1000] {
            let mut r = make();
            let mut cells = Vec::new();
            for p in out.chunks(cut) {
                r.process(p, |s| cells.extend_from_slice(s.cells));
            }
            assert_eq!(cells, whole, "{cut}");
        }
    }

    #[test]
    fn delayed_estimates_carry_the_start_of_their_own_symbol() {
        let layout = OfdmLayout::dvbt_2k(32).unwrap();
        let len = layout.symbol_len(0);
        let est = Estimator::by_name("time_freq", &layout).unwrap();
        assert_eq!(est.delay(), 3);
        let mut rx = OfdmReceiver::new(&layout, est, 16, 0).unwrap();
        let x: Vec<Complex32> = (0..10 * len)
            .map(|i| Complex32::new(((i * 7) % 13) as f32 - 6.0, ((i * 5) % 11) as f32 - 5.0))
            .collect();
        let mut got = Vec::new();
        rx.process(&x, |s| got.push((s.symbol, s.start)));
        assert!(!got.is_empty());
        for &(sym, start) in &got {
            assert_eq!(start, sym * len as u64);
        }
        // A restart numbers from the first symbol again and clears the estimator.
        rx.restart();
        got.clear();
        rx.process(&x[..2 * len], |s| got.push((s.symbol, s.start)));
        assert!(got.is_empty());
        assert!(Estimator::by_name("magic", &layout).is_err());
        assert!(OfdmReceiver::new(
            &layout,
            Estimator::by_name("pilot_ls", &layout).unwrap(),
            100,
            0
        )
        .is_err());
    }

    /// 802.11a-like symbols sampled by a clock `ppm` fast: each sample `n` is the continuous-time signal at `n (1 + ppm 1e-6)`
    /// sample periods. Returns the samples and the sent cells.
    fn wifi_clock(
        symbols: usize,
        lead: usize,
        ppm: f64,
        seed: u64,
    ) -> (Vec<Complex32>, Vec<Vec<Complex32>>) {
        let layout = OfdmLayout::ieee80211a();
        let n = layout.nfft() as f64;
        let mut rng = Rng::new(seed);
        let mut sent = Vec::new();
        for s in 0..symbols as u64 {
            let mut cells = vec![Complex32::default(); layout.used()];
            for &d in layout.data(s) {
                let (a, b) = (rng.bit(), rng.bit());
                cells[d as usize] = Complex32::new(1.0 - 2.0 * a as f32, 1.0 - 2.0 * b as f32)
                    * std::f32::consts::FRAC_1_SQRT_2;
            }
            for p in layout.pilots(s) {
                cells[p.index as usize] = p.value;
            }
            sent.push(cells);
        }
        let total = ((symbols * 80) as f64 / (1.0 + ppm * 1e-6)) as usize - 2;
        let mut x = vec![Complex32::default(); lead];
        for m in 0..total {
            let t = m as f64 * (1.0 + ppm * 1e-6);
            let s = (t / 80.0).floor() as usize;
            // Time from the start of the symbol's useful part (the prefix repeats its end).
            let u = t - s as f64 * 80.0 - 16.0;
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, c) in sent[s].iter().enumerate() {
                if *c == Complex32::default() {
                    continue;
                }
                let k = layout.carrier(i) as f64;
                let (sn, cs) = (std::f64::consts::TAU * k * u / n).sin_cos();
                re += c.re as f64 * cs - c.im as f64 * sn;
                im += c.re as f64 * sn + c.im as f64 * cs;
            }
            x.push(Complex32::new(
                (re / n.sqrt()) as f32,
                (im / n.sqrt()) as f32,
            ));
        }
        (x, sent)
    }

    #[test]
    fn the_clock_loop_follows_a_sampling_clock_error_without_jumps() {
        // 200 ppm fast: the symbols drift 0.016 samples each, 8 samples over the run.
        let (x, _) = wifi_clock(600, 300, 200.0, 7);
        let layout = OfdmLayout::ieee80211a();
        let mut sync = CpSync::new(&layout, CpSyncConfig::default()).unwrap();
        sync.enable_clock_loop(0.01, 1000.0).unwrap();
        let (mut out, mut events) = (Vec::new(), Vec::new());
        for c in x.chunks(997) {
            sync.process(c, &mut out, &mut events);
        }
        assert!(sync.locked());
        assert!(
            (sync.clock_ppm() + 200.0).abs() < 60.0,
            "{}",
            sync.clock_ppm()
        );
        let syms: Vec<(u64, usize, f64)> = events
            .iter()
            .filter_map(|e| match *e {
                CpSyncEvent::Symbol {
                    start,
                    out_at,
                    timing,
                    ..
                } => Some((start, out_at, timing)),
                _ => None,
            })
            .collect();
        assert!(syms.len() > 550);
        // The exact starts advance smoothly: never a whole-sample jump.
        for w in syms[50..].windows(2) {
            let step = (w[1].0 as f64 + w[1].2) - (w[0].0 as f64 + w[0].2);
            assert!((step - 80.0 / (1.0 + 200e-6)).abs() < 0.25, "{step}");
            assert!(w[0].2.abs() <= 0.5 + 1e-9);
        }
        // The exact starts (cut plus fraction) follow the true symbol starts, 300 + s 80 / (1 + 200e-6). The test signal changes
        // symbol at the first whole sample after a start, so the prefix correlation sees each start half a sample late on average.
        // each peak is good to about 2 samples only (a 16-sample prefix of random data), and the loop averages that out.
        let truth = |start: f64| {
            let s = ((start - 300.0) * (1.0 + 200e-6) / 80.0).round();
            300.0 + s * 80.0 / (1.0 + 200e-6)
        };
        let tail = &syms[syms.len() - 300..];
        let errs: Vec<f64> = tail
            .iter()
            .map(|&(st, _, t)| st as f64 + t - truth(st as f64 + t))
            .collect();
        let bias = errs.iter().sum::<f64>() / errs.len() as f64;
        let rms = (errs.iter().map(|e| e * e).sum::<f64>() / errs.len() as f64).sqrt();
        assert!(
            (bias - 0.5).abs() < 0.3 && rms < 1.0,
            "bias {bias} rms {rms}"
        );
        // A caller with a better measurement (the pilots' timing slope) feeds it in. Here it is the truth itself.
        let mut fed = CpSync::new(&layout, CpSyncConfig::default()).unwrap();
        fed.enable_clock_loop(0.01, 1000.0).unwrap();
        fed.set_peak_timing(false);
        let (mut out2, mut ev2) = (Vec::new(), Vec::new());
        let mut last = Vec::new();
        for c in x.chunks(80) {
            ev2.clear();
            fed.process(c, &mut out2, &mut ev2);
            for e in &ev2 {
                if let CpSyncEvent::Symbol { start, timing, .. } = *e {
                    let at = start as f64 + timing;
                    fed.timing_error(truth(at) - at);
                    last.push(at - truth(at));
                }
            }
        }
        let tail = &last[last.len() - 300..];
        let rms = (tail.iter().map(|e| e * e).sum::<f64>() / tail.len() as f64).sqrt();
        assert!(rms < 0.1, "fed rms {rms}");
        assert!(sync.enable_clock_loop(0.7, 100.0).is_err());
        assert!(sync.enable_clock_loop(0.01, -1.0).is_err());
    }
}
