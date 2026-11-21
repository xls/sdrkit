//! The PSK physical layer of every link: the front end, the carrier searches and the demodulator with its
//! soft symbols (LLRs, positive means 0).

use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fir::design_by_taps;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::level::{DataAided, M2m4};
use sdrkit_algo_modem::lock::{Gears, LockDetector};
use sdrkit_algo_modem::psk::{
    BandLocator, CarrierSearch, PskCarrier, PskDemodConfig, PskDemodulator, PskFrontEnd, PskOrder,
    PskPulse, SearchConfig, SearchKind, SearchOutcome,
};
use sdrkit_algo_modem::ModemError;
use sdrkit_core::Complex32;
use std::ops::ControlFlow;

/// The modulation of a link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modulation {
    /// Binary PSK: one bit per symbol.
    Bpsk,
    /// Quadrature PSK: I and Q at the same instant.
    Qpsk,
    /// Offset QPSK: Q half a symbol after I.
    Oqpsk,
}

impl Modulation {
    /// Bits (code symbols) per channel symbol.
    pub fn bits_per_symbol(self) -> usize {
        match self {
            Modulation::Bpsk => 1,
            _ => 2,
        }
    }
}

/// Which set of numbers a link uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkClass {
    /// Meteor, MetOp AHRPT, NOAA HRPT and FY-3: at least 4 samples a symbol, Doppler hand-over, fast loops.
    Leo,
    /// GOES, GK-2A, Elektro-L: at least 2.3 samples a symbol, slower loops, lock judged over many symbols.
    Geo,
    /// Elektro-L GGAK telemetry.
    Ggak,
    /// FY-2 S-VISSR: the maximum-likelihood BPSK detector, 1.4 samples a symbol at the least.
    Fy2,
}

/// How the carrier is searched before tracking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Search {
    /// Correlate the averaged spectrum with the raised-cosine shape (QPSK and OQPSK LEO links), within +-`range_hz`.
    Template {
        /// The search range.
        range_hz: f64,
    },
    /// The line of the signal raised to `power` (2 for BPSK, 4 for QPSK); `min_ratio` is the line's least height over the mean.
    PowerLine {
        /// 2 or 4.
        power: u32,
        /// The least height of the line over the mean of the spectrum (0 for the default ladder).
        min_ratio: f64,
    },
}

/// Which Es/N0 estimate a link reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnrEstimator {
    /// The blind M2M4 estimator from the moments of the symbols (Meteor).
    Moments,
    /// The decision-directed estimator: the gain and the error to the nearest BPSK or QPSK point.
    Decisions,
}

/// Everything the physical layer of a link needs.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkParams {
    /// The modulation.
    pub modulation: Modulation,
    /// Symbols per second.
    pub symbol_rate: f64,
    /// The root raised cosine roll-off.
    pub roll_off: f64,
    /// The set of numbers.
    pub class: LinkClass,
    /// How the carrier is found.
    pub search: Search,
    /// The Es/N0 estimate.
    pub snr: SnrEstimator,
    /// BPSK code symbols: +1 is code bit 0 (the geostationary links) rather than 1.
    pub bpsk_plus_is_zero: bool,
    /// Symbols kept for the constellation.
    pub constellation: usize,
    /// Meteor's 80k link: markers and a convolutional interleaver between the demodulator and the code.
    pub interleaved: bool,
}

impl LinkParams {
    /// The least samples a symbol after decimation.
    pub fn min_sps(&self) -> f64 {
        match self.class {
            LinkClass::Leo => 4.0,
            // GGAK is narrow: 6 samples a symbol keep the squared carrier line of an offset up to 1.5 Rs below the Nyquist limit.
            LinkClass::Ggak => 6.0,
            LinkClass::Geo => 2.3,
            LinkClass::Fy2 => 2.3,
        }
    }
    /// The decimation factor for an input rate.
    pub fn decimation(&self, rate: f64) -> usize {
        ((rate / (self.symbol_rate * self.min_sps())).floor() as usize).max(1)
    }
}

/// The front end of a link: bad samples to zero, the mixer, the anti-alias filter and decimation, and (geostationary
/// links, a capture wider than 3 Rs) the band locator that moves the mixer onto the signal.
///
/// # Errors
///
/// The [`ModemError`] of a rate or a factor that the front end cannot take.
pub fn front_end(
    rate: f64,
    offset_hz: f64,
    params: &LinkParams,
    blind_iq: bool,
) -> Result<PskFrontEnd, ModemError> {
    let factor = params.decimation(rate);
    let taps = anti_alias(rate, factor, params);
    let wide = matches!(params.class, LinkClass::Geo) && rate > 3.0 * params.symbol_rate;
    let locate = wide
        .then(|| BandLocator::new(rate, params.symbol_rate, params.roll_off).ok())
        .flatten();
    PskFrontEnd::new(rate, offset_hz, Some((&taps, factor)), locate, blind_iq)
}

/// The anti-alias filter of the front end.
pub fn anti_alias(rate: f64, factor: usize, p: &LinkParams) -> Vec<f32> {
    let out = rate / factor as f64;
    let rs = p.symbol_rate;
    let (taps, cutoff, window) = match p.class {
        LinkClass::Leo => {
            let width = rs * (1.0 + p.roll_off);
            let tw = (out - width).max(out / 5.0);
            let mut n = (3.5 * rate / tw).round() as usize;
            n = n.clamp(9, 401) | 1;
            (n, out / 2.0, Window::Hamming)
        }
        LinkClass::Geo | LinkClass::Fy2 => {
            let pass = 0.8 * rs;
            let stop = (out - 0.8 * rs).max(pass * 1.05);
            let n = ((1.8 * rate / (stop - pass)) as usize + 4 * factor) | 1;
            (n.min(1001), (pass + stop) / 2.0, Window::Blackman)
        }
        LinkClass::Ggak => ((8 * factor + 1) | 1, 0.85 * rs, Window::Hamming),
    };
    design_by_taps(taps, (cutoff / rate).clamp(1e-6, 0.5), window).unwrap_or_else(|_| vec![1.0])
}

/// What the demodulator reports.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DemodState {
    /// The coarse search is done and the loops run.
    pub tracking: bool,
    /// Carrier lock (the lock metric above the link's threshold).
    pub locked: bool,
    /// The lock metric, 0 to 1.
    pub lock_metric: f32,
    /// The carrier offset found (coarse plus hand-over plus loop), Hz from the mixer's start (the front end's offset not included).
    pub carrier_hz: f64,
    /// Es/N0 estimate, dB.
    pub es_n0_db: f32,
    /// The symbol rate measured by the timing loop.
    pub symbol_rate: f64,
    /// The symbol clock against nominal, ppm.
    pub clock_ppm: f64,
    /// Symbols since tracking started.
    pub symbols: u64,
    /// Searches started.
    pub searches: u32,
}

/// Commands to the demodulator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DemodCommand {
    /// Search the carrier again; `reject` remembers the current offset as one that never locked.
    Restart {
        /// Remember the offset as rejected.
        reject: bool,
    },
}

/// The span of the matched filter of [`PskDemod::short`], symbols.
const SHORT_SPAN: f64 = 8.0;
/// A symbol at unit level as an LLR: 40 times the rail of a point at 45 degrees.
const SOFT: f32 = 40.0 * std::f32::consts::SQRT_2;

/// Symbols in each piece the Es/N0 estimate is taken over.
const SNR_PIECE: u32 = 2048;
/// The Es/N0 estimate takes the first piece of every this many, at the least.
const SNR_STRIDE: u32 = 4;
/// The most Es/N0 estimates a second: a faster link leaves more pieces between two estimates.
const SNR_PER_SECOND: f64 = 100.0;

/// The carrier lock test of a link: the mean of cos(M phi) over about 500 symbols, with the thresholds of its class.
fn lock_detector(p: &LinkParams) -> LockDetector {
    let bpsk = p.modulation == Modulation::Bpsk;
    let (on, off) = match p.class {
        LinkClass::Leo => (0.5, 0.5),
        LinkClass::Ggak => (0.35, 0.35),
        LinkClass::Geo | LinkClass::Fy2 if bpsk => (0.35, 0.2),
        LinkClass::Geo | LinkClass::Fy2 => (0.2, 0.1),
    };
    LockDetector::new(500.0, on, off).unwrap_or_else(|_| unreachable!("constant thresholds"))
}

/// The timing loop bandwidths of the low-orbit links, per symbol: pulling in, then tracking.
const TIMING_LEO: (f64, f64) = (0.01, 0.0025);
/// The timing loop bandwidth of the other links, per symbol.
const TIMING_GEO: f64 = 0.004;
/// The carrier loop frequency (cycles per symbol) above which a low-orbit link moves it into the mixer.
const HANDOVER_CYCLES: f64 = 0.003 / std::f64::consts::TAU;

/// The PSK demodulator of a link: the carrier search, the shared demodulator with the link's loop numbers, the lock and settling
/// rules of its class, and soft symbols.
pub struct PskDemod {
    p: LinkParams,
    rate: f64,
    pre: Option<FirDecimator<Complex32>>,
    pre_buf: Vec<Complex32>,
    sps: f64,
    /// The demodulator; taken out while it runs samples, so the symbol rules can borrow the rest.
    demod: Option<PskDemodulator>,
    coarse_hz: f64,
    amp: f32,
    order: PskOrder,
    lock: LockDetector,
    // Es/N0: the first piece of SNR_PIECE symbols in every `snr_stride` produces an estimate.
    snr_stride: u32,
    moments: M2m4,
    decisions: DataAided,
    piece: u32,
    snr_db: Option<f64>,
    search: Option<CarrierSearch>,
    rejected: Vec<f64>,
    /// State reported out.
    pub state: DemodState,
    symbols: u64,
    below: u64,
    since_lock_check: u64,
    /// The newest points of the constellation, a ring written at `point_at`; `point_count` counts every point written.
    points: Vec<[f32; 2]>,
    point_at: usize,
    point_count: u64,
    settled: bool,
    ppm_avg: f64,
    // The total carrier offset (Hz) smoothed over about 0.05 s while a LEO loop is still wide.
    freq_avg: f64,
}

impl PskDemod {
    /// A demodulator for symbols at `rate` samples a second. When the rate holds more than twice the samples a symbol the link needs
    /// (a link whose symbol rate changes at run time), it first decimates by a whole factor.
    ///
    /// # Errors
    ///
    /// The [`ModemError`] of a rate below one sample a symbol, or of loop numbers the demodulator refuses.
    pub fn new(input_rate: f64, p: &LinkParams) -> Result<Self, ModemError> {
        Self::with_span(input_rate, p, false)
    }

    /// [`PskDemod::new`] with the matched filter over 8 symbols, not 12: for the links of tens of megasymbols a second,
    /// where the length of the filter sets the processor time. The tails that the shorter filter leaves out are about
    /// 40 dB under the peak of the pulse at a roll-off of 0.5.
    ///
    /// # Errors
    ///
    /// As [`PskDemod::new`].
    pub fn short(input_rate: f64, p: &LinkParams) -> Result<Self, ModemError> {
        Self::with_span(input_rate, p, true)
    }

    fn with_span(input_rate: f64, p: &LinkParams, short: bool) -> Result<Self, ModemError> {
        let f = ((input_rate / p.symbol_rate) / p.min_sps()).floor() as usize;
        let (pre, rate) = if f >= 2 {
            (
                Some(FirDecimator::new(&anti_alias(input_rate, f, p), f)),
                input_rate / f as f64,
            )
        } else {
            (None, input_rate)
        };
        let sps = rate / p.symbol_rate;
        let mut d = PskDemod {
            p: p.clone(),
            rate,
            pre,
            pre_buf: Vec::new(),
            sps,
            demod: Some(PskDemodulator::new(demod_config(p, sps, short))?),
            coarse_hz: 0.0,
            amp: 0.0,
            order: order_of(p.modulation),
            lock: lock_detector(p),
            snr_stride: SNR_STRIDE.max((p.symbol_rate / (f64::from(SNR_PIECE) * SNR_PER_SECOND)) as u32),
            moments: M2m4::psk(),
            decisions: DataAided::new(),
            piece: 0,
            snr_db: None,
            search: None,
            rejected: Vec::new(),
            state: DemodState::default(),
            symbols: 0,
            below: 0,
            since_lock_check: 0,
            points: vec![[0.0; 2]; p.constellation],
            point_at: 0,
            point_count: 0,
            settled: false,
            ppm_avg: 0.0,
            freq_avg: 0.0,
        };
        d.restart(false);
        Ok(d)
    }

    /// The samples a symbol.
    pub fn sps(&self) -> f64 {
        self.sps
    }

    /// The link's numbers.
    pub fn params(&self) -> &LinkParams {
        &self.p
    }

    /// The newest points of the constellation (up to the count of the link), oldest first.
    pub fn constellation(&self) -> impl Iterator<Item = [f32; 2]> + '_ {
        let n = self.points.len();
        let held = (self.point_count.min(n as u64)) as usize;
        let first = if held < n { 0 } else { self.point_at };
        (0..held).map(move |k| self.points[(first + k) % n])
    }

    /// Searches the carrier again (and optionally remembers the current offset as never locked).
    pub fn restart(&mut self, reject: bool) {
        if let Some(mut d) = self.demod.take() {
            self.restart_with(&mut d, reject);
            self.demod = Some(d);
        }
    }

    fn restart_with(&mut self, d: &mut PskDemodulator, reject: bool) {
        if reject && self.state.tracking {
            self.rejected.push(self.coarse_hz);
            if self.rejected.len() > 8 {
                self.rejected.remove(0);
            }
        }
        let kind = match self.p.search {
            Search::Template { range_hz } => SearchKind::Template { range_hz },
            Search::PowerLine { power, min_ratio } => SearchKind::PowerLine {
                power,
                min_db: (min_ratio > 0.0).then(|| 10.0 * min_ratio.log10()),
            },
        };
        let cfg = SearchConfig {
            kind,
            rate: self.rate,
            symbol_rate: self.p.symbol_rate,
            roll_off: self.p.roll_off,
        };
        let Ok(search) = CarrierSearch::new(cfg, &self.rejected) else {
            return;
        };
        self.search = Some(search);
        self.state.tracking = false;
        self.state.locked = false;
        self.state.searches += 1;
        self.coarse_hz = 0.0;
        d.set_mixer_offset(0.0);
        self.reset_loops(d);
    }

    /// Starts tracking at the carrier `hz` (from the mixer's start) without a search: a gate in front of the demodulator
    /// found it.
    pub fn start_at(&mut self, hz: f64) {
        let Some(mut d) = self.demod.take() else {
            return;
        };
        self.search = None;
        self.state.tracking = true;
        self.state.locked = false;
        self.state.searches += 1;
        self.coarse_hz = hz;
        d.set_mixer_offset(hz / self.rate);
        self.reset_loops(&mut d);
        self.demod = Some(d);
    }

    fn reset_loops(&mut self, d: &mut PskDemodulator) {
        self.amp = 0.0;
        self.lock = lock_detector(&self.p);
        self.moments.reset();
        self.decisions.reset();
        self.piece = 0;
        self.snr_db = None;
        self.symbols = 0;
        self.below = 0;
        self.since_lock_check = 0;
        self.settled = false;
        self.state.symbols = 0;
        self.state.lock_metric = 0.0;
        self.ppm_avg = 0.0;
        self.freq_avg = self.coarse_hz;
        let (b0, _) = self.bandwidths(false);
        let _ = d.set_carrier_loop(carrier_detector(&self.p, 1.0), b0);
        let _ = d.reset();
    }

    /// (carrier B, timing kind) at start or after settling.
    fn bandwidths(&self, settled: bool) -> (f64, f64) {
        match (self.p.class, settled) {
            (LinkClass::Leo, false) => (0.01, 1.0),
            (LinkClass::Leo, true) => (
                if self.p.snr == SnrEstimator::Moments {
                    0.004
                } else {
                    0.002
                },
                0.25,
            ),
            (_, false) => (0.01, 0.0),
            (_, true) => (
                if self.p.modulation == Modulation::Bpsk {
                    if self.p.class == LinkClass::Ggak {
                        0.004
                    } else {
                        0.008
                    }
                } else {
                    0.004
                },
                0.0,
            ),
        }
    }

    /// Applies a command.
    pub fn command(&mut self, c: DemodCommand) {
        match c {
            DemodCommand::Restart { reject } => self.restart(reject),
        }
    }

    /// The carrier offset found so far, Hz (search plus hand-over plus the loop's frequency).
    pub fn carrier_hz(&self) -> f64 {
        self.demod.as_ref().map_or(0.0, |d| d.carrier_offset() * self.rate)
    }

    /// Runs samples, appending soft code symbols as LLRs (positive means 0).
    pub fn process(&mut self, input: &[Complex32], soft: &mut Vec<i8>) {
        let mut pre_buf = std::mem::take(&mut self.pre_buf);
        let x: &[Complex32] = match &mut self.pre {
            Some(d) => {
                pre_buf.clear();
                d.process(input, &mut pre_buf);
                &pre_buf
            }
            None => input,
        };
        self.run(x, soft);
        self.pre_buf = pre_buf;
    }

    fn run(&mut self, x: &[Complex32], soft: &mut Vec<i8>) {
        let Some(mut d) = self.demod.take() else {
            return;
        };
        self.run_with(&mut d, x, soft);
        self.demod = Some(d);
    }

    fn run_with(&mut self, d: &mut PskDemodulator, x: &[Complex32], soft: &mut Vec<i8>) {
        let mut i = 0;
        while i < x.len() {
            if let Some(s) = &mut self.search {
                let Ok((used, outcome)) = s.push(&x[i..]) else {
                    // The search refused its spectrum: it stops until the next restart.
                    self.search = None;
                    return;
                };
                i += used;
                match outcome {
                    Some(SearchOutcome::Found(f)) => {
                        self.search = None;
                        self.coarse_hz = f;
                        d.set_mixer_offset(f / self.rate);
                        self.state.tracking = true;
                        self.reset_loops(d);
                    }
                    Some(SearchOutcome::NotFound) => {
                        // No line strong enough: search again from the start.
                        self.restart_with(d, false);
                        self.state.searches -= 1;
                    }
                    None => {}
                }
                continue;
            }
            // A change of the mixer at a run of symbols acts on the samples after it whatever the cuts of the input; a new
            // search takes the samples after the run that started it.
            i += d.process_with(&x[i..], |d, run| {
                self.symbols(d, run, soft);
                if self.search.is_some() {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            });
        }
    }

    /// One run of symbols: the level, the lock value, the Es/N0 pieces, the soft symbols, the constellation, the averages
    /// and the lock rules.
    #[inline]
    fn symbols(&mut self, d: &mut PskDemodulator, run: &[Complex32], soft: &mut Vec<i8>) {
        let bpsk = self.p.modulation == Modulation::Bpsk;
        // Soft symbols as LLRs, positive means 0: a positive rail is a 1 unless the link sends 0 as +1.
        let scale = if bpsk && self.p.bpsk_plus_is_zero { SOFT } else { -SOFT };
        let sat = |v: f32| v.clamp(-127.0, 127.0) as i8;
        let ppm = d.clock_ppm();
        soft.reserve(run.len() * if bpsk { 1 } else { 2 });
        for &z in run {
            let mag = z.norm_sqr().sqrt();
            self.amp = if self.amp == 0.0 {
                mag.max(1e-9)
            } else {
                self.amp + 0.002 * (mag - self.amp)
            };
            let zn = z * (1.0 / self.amp.max(1e-12));
            self.lock.update(lock_value(bpsk, zn));
            // Es/N0 over the first piece of symbols in every `snr_stride`.
            if self.piece < SNR_PIECE {
                match self.p.snr {
                    SnrEstimator::Moments => self.moments.push(zn),
                    SnrEstimator::Decisions => self.decisions.push(zn, self.order.nearest(zn)),
                }
            }
            self.piece += 1;
            if self.piece == SNR_PIECE {
                let e = match self.p.snr {
                    SnrEstimator::Moments => self.moments.estimate(),
                    SnrEstimator::Decisions => self.decisions.estimate(),
                };
                self.snr_db = Some(e.map_or(-10.0, |e| e.snr_db));
                self.moments.reset();
                self.decisions.reset();
            } else if self.piece == SNR_PIECE * self.snr_stride {
                self.piece = 0;
            }
            soft.push(sat(zn.re * scale));
            if !bpsk {
                soft.push(sat(zn.im * scale));
            }
            if !self.points.is_empty() {
                self.points[self.point_at] = [zn.re, zn.im];
                self.point_at = if self.point_at + 1 == self.points.len() { 0 } else { self.point_at + 1 };
                self.point_count += 1;
            }
        }
        // The symbol rate reported is the timing loop's clock error averaged over about 20 000 symbols: the mean of the
        // first 19 999, then a mean that forgets. The clock error is one value for the run.
        let n = run.len() as u64;
        let first = 19_999u64.saturating_sub(self.symbols).min(n);
        for _ in 0..first {
            self.symbols += 1;
            self.ppm_avg += (ppm - self.ppm_avg) / self.symbols as f64;
        }
        if n > first {
            self.ppm_avg = ppm + (self.ppm_avg - ppm) * (1.0f64 - 5e-5).powi((n - first) as i32);
            self.symbols += n - first;
        }
        self.state.lock_metric = self.lock.metric().clamp(0.0, 1.0);
        self.state.symbols = self.symbols;
        if self.p.class == LinkClass::Leo && !self.settled {
            // A mean over about 0.05 s, a run at a time.
            let k = (20.0 * run.len() as f64 / self.p.symbol_rate).min(1.0);
            self.freq_avg += k * (d.carrier_offset() * self.rate - self.freq_avg);
        }
        self.lock_rules(d, run.len() as u64);
    }

    /// The rules after a run of `n` symbols.
    fn lock_rules(&mut self, d: &mut PskDemodulator, n: u64) {
        let rs = self.p.symbol_rate;
        let settle_after = match self.p.class {
            LinkClass::Leo => (0.5 * rs) as u64,
            _ => 20_000,
        };
        if !self.settled && self.symbols >= settle_after {
            self.settled = true;
            if self.p.class == LinkClass::Leo {
                // The wide loop wanders at low Es/N0 (and can be in a slip at this instant): the narrow loop starts from the
                // smoothed offset, inside its pull-in range.
                d.set_mixer_offset(self.freq_avg / self.rate);
                d.set_loop_freq(0.0);
            }
            // The narrow carrier loop starts where the wide one is.
            let (b, _) = self.bandwidths(true);
            let n0 = self.snr_db.map_or(1.0, |db| 10f64.powf(-db / 10.0)) as f32;
            let _ = d.set_carrier_loop(carrier_detector(&self.p, n0.max(1e-3)), b);
        }
        match self.p.class {
            LinkClass::Leo => {
                self.state.locked = self.lock.locked();
            }
            LinkClass::Ggak => {
                if self.symbols >= 8000 {
                    self.state.locked = self.lock.locked();
                }
            }
            LinkClass::Geo | LinkClass::Fy2 => {
                let bpsk = self.p.modulation == Modulation::Bpsk;
                let off = if bpsk { 0.2 } else { 0.1 };
                if self.symbols >= 30_000 || self.p.class == LinkClass::Fy2 && self.symbols >= 5000
                {
                    self.state.locked = self.lock.locked();
                    if self.lock.metric() < off {
                        self.below += n;
                    } else {
                        self.below = 0;
                    }
                    if self.below > 60_000 {
                        self.restart_with(d, false);
                        return;
                    }
                }
                if !self.state.locked {
                    self.since_lock_check += n;
                    if self.since_lock_check > 120_000 && self.symbols > 120_000 {
                        self.restart_with(d, true);
                    }
                } else {
                    self.since_lock_check = 0;
                }
            }
        }
    }

    /// Fills the report.
    pub fn report(&mut self) -> &DemodState {
        self.state.es_n0_db = if self.state.tracking {
            self.snr_db.map_or(-10.0, |v| v.clamp(-10.0, 40.0)) as f32
        } else {
            -10.0
        };
        // The timing loop counts its clock error positive when symbols are longer: the report counts the symbol clock fast.
        self.state.symbol_rate = self.p.symbol_rate / (1.0 + self.ppm_avg * 1e-6);
        self.state.clock_ppm = -self.ppm_avg;
        self.state.carrier_hz = self.carrier_hz();
        &self.state
    }
}

/// The carrier loop of a link: decision-directed on the BPSK or QPSK points (each symbol weighed by its size, so weak noisy
/// symbols move the loop less), or the maximum-likelihood BPSK detector with noise variance `n0` (FY-2).
fn carrier_detector(p: &LinkParams, n0: f32) -> PskCarrier {
    match (p.modulation, p.class) {
        (Modulation::Bpsk, LinkClass::Fy2) => PskCarrier::MlTanh { n0 },
        _ => PskCarrier::DecisionDirected,
    }
}

/// The order of a modulation.
fn order_of(m: Modulation) -> PskOrder {
    match m {
        Modulation::Bpsk => PskOrder::Bpsk,
        Modulation::Qpsk => PskOrder::Qpsk,
        Modulation::Oqpsk => PskOrder::Oqpsk,
    }
}

/// The lock value of a symbol at unit level, about `cos(M phi)`: `Re{z^2} / |z|^2` for BPSK, `-Re{z^4} / |z|^4` for the
/// QPSK points at 45 degrees.
#[inline]
fn lock_value(bpsk: bool, z: Complex32) -> f32 {
    let (a, b) = (z.re * z.re, z.im * z.im);
    let n = a + b;
    if n <= 0.0 {
        0.0
    } else if bpsk {
        (a - b) / n
    } else {
        // Re{z^4} = (a - b)^2 - 4 a b.
        (4.0 * a * b - (a - b) * (a - b)) / (n * n)
    }
}

/// The demodulator numbers of a link at `sps` samples a symbol: root raised cosine over 12 symbols (8 with `short`, so
/// that the filter has 16 taps at up to 2 samples a symbol and 32 at up to 4), a Gardner loop
/// with two gears on low-orbit links, clock range 5000 ppm, a carrier loop with range 0.25 radians per symbol and no frequency
/// loop, and the Doppler hand-over of low-orbit links.
fn demod_config(p: &LinkParams, sps: f64, short: bool) -> PskDemodConfig {
    let order = order_of(p.modulation);
    let mut c = PskDemodConfig::new(order, sps);
    c.pulse = PskPulse::Rrc(p.roll_off);
    c.mf_span = if short { SHORT_SPAN } else { 12.0 };
    c.max_ppm = 5000.0;
    (c.timing_bn_t, c.timing_gears) = match p.class {
        // The lock test of the timing loop compares the energy at the symbol with the energy half way between, which is the same
        // for offset QPSK: its gears do not shift, so it runs at the tracking bandwidth.
        LinkClass::Leo if order == PskOrder::Oqpsk => (TIMING_LEO.1, None),
        LinkClass::Leo => (
            TIMING_LEO.0,
            Some(Gears {
                pull_in_bn_t: TIMING_LEO.0,
                track_bn_t: TIMING_LEO.1,
            }),
        ),
        _ => (TIMING_GEO, None),
    };
    c.carrier = carrier_detector(p, 1.0);
    c.carrier_bn_t = 0.01;
    c.fll_bn_t = None;
    c.max_freq = 0.25 / std::f64::consts::TAU;
    c.handover = (p.class == LinkClass::Leo).then_some(HANDOVER_CYCLES);
    c.hold_symbols = 0;
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anti_alias_has_unity_dc_gain() {
        let p = LinkParams {
            modulation: Modulation::Qpsk,
            symbol_rate: 72e3,
            roll_off: 0.6,
            class: LinkClass::Leo,
            search: Search::Template { range_hz: 15e3 },
            snr: SnrEstimator::Moments,
            bpsk_plus_is_zero: false,
            constellation: 256,
            interleaved: false,
        };
        assert_eq!(p.decimation(1e6), 3);
        let h = anti_alias(1e6, 3, &p);
        let s: f32 = h.iter().sum();
        assert!((s - 1.0).abs() < 1e-5);
        assert!(h.len() % 2 == 1 && h.len() >= 9 && h.len() <= 401);
    }
}
