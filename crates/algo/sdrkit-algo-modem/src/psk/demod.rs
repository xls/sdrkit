//! The streaming PSK demodulator: the mixer, the matched filter, symbol timing and the carrier loop.

use super::{PskOrder, PskPulse};
use crate::carrier::{CarrierConfig, CarrierSync, Costas, MlTanh, PhaseDetector};
use crate::lock::Gears;
use crate::timing::{MatchedBank, RunSymbolSync, SymbolSyncConfig};
use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::interp::Cubic;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::Shape;

mod slices;

/// The phase detector of the carrier loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PskCarrier {
    /// The Costas loop of the order (2, 4 or 8 points).
    Costas,
    /// Decision-directed on the points of the order, each symbol weighed by its size. BPSK and QPSK only.
    DecisionDirected,
    /// The maximum-likelihood BPSK detector `tanh(2 Re / N0) Im` with the noise variance `n0` of unit-power symbols.
    MlTanh {
        /// The noise variance.
        n0: f32,
    },
}

/// Everything a [`PskDemodulator`] needs.
#[derive(Clone, Debug, PartialEq)]
pub struct PskDemodConfig {
    /// The order.
    pub order: PskOrder,
    /// Samples per symbol (above 1).
    pub sps: f64,
    /// The transmit pulse. The matched filter and the timing detector follow it.
    pub pulse: PskPulse,
    /// The least span of the root raised cosine matched filter, symbols.
    pub mf_span: f64,
    /// The timing loop's noise bandwidth per symbol.
    pub timing_bn_t: f64,
    /// Timing loop gears (pull-in, then tracking), if any.
    pub timing_gears: Option<Gears>,
    /// The largest clock error the timing loop follows, ppm.
    pub max_ppm: f64,
    /// The carrier loop's phase detector.
    pub carrier: PskCarrier,
    /// The carrier loop's noise bandwidth per symbol.
    pub carrier_bn_t: f64,
    /// The noise bandwidth of a frequency loop that assists the pull-in, per symbol.
    pub fll_bn_t: Option<f64>,
    /// The largest frequency the carrier loop pulls to, cycles per symbol.
    pub max_freq: f64,
    /// When the carrier loop's frequency passes this (cycles per symbol), it moves into the mixer, so the matched filter sees the
    /// signal at 0 Hz.
    pub handover: Option<f64>,
    /// Symbols at the start for which the carrier loop only turns the symbols and does not learn (the offset QPSK timing loop
    /// pairs the rails first).
    pub hold_symbols: u64,
}

impl PskDemodConfig {
    /// Defaults for `order` at `sps`: root raised cosine 0.35 over 8 symbols, timing gears 0.02 then 0.004 per symbol, clock range
    /// 60 000 ppm, a Costas loop of 0.01 per symbol with a frequency loop of 0.005, range 0.1 cycles per symbol, hand-over above
    /// 0.002, and 200 held symbols for offset QPSK.
    pub fn new(order: PskOrder, sps: f64) -> Self {
        PskDemodConfig {
            order,
            sps,
            pulse: PskPulse::Rrc(0.35),
            mf_span: 8.0,
            timing_bn_t: 0.004,
            timing_gears: Some(Gears {
                pull_in_bn_t: 0.02,
                track_bn_t: 0.004,
            }),
            max_ppm: 60_000.0,
            carrier: PskCarrier::Costas,
            carrier_bn_t: 0.01,
            fll_bn_t: Some(0.005),
            max_freq: 0.1,
            handover: Some(0.002),
            hold_symbols: if order == PskOrder::Oqpsk { 200 } else { 0 },
        }
    }
}

/// The decision-directed detector on the points of an order: `Im{y d*}` with `d` the nearest unit point, found by sign
/// tests. The error grows with the size of the symbol, so a weak symbol moves the loop less.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Decided(pub(crate) PskOrder);

impl PhaseDetector for Decided {
    fn order(&self) -> u32 {
        self.0.m() as u32
    }
    #[inline]
    fn error(&mut self, y: Complex32) -> f32 {
        self.error_and_lock(y).0
    }
    #[inline]
    fn lock_value(&self, y: Complex32) -> f32 {
        let mut d = *self;
        d.error_and_lock(y).1
    }
    #[inline]
    fn error_and_lock(&mut self, y: Complex32) -> (f32, f32) {
        let c = y * self.0.nearest(y).conj();
        let n = y.norm_sqr().sqrt();
        (c.im, if n > 0.0 { c.re / n } else { 0.0 })
    }
}

/// The carrier loop of one phase detector.
#[derive(Clone, Debug)]
enum Loop {
    C2(CarrierSync<Costas<2>>),
    C4(CarrierSync<Costas<4>>),
    C8(CarrierSync<Costas<8>>),
    Dd(CarrierSync<Decided>),
    Ml(CarrierSync<MlTanh>),
}

macro_rules! each {
    ($s:expr, $c:ident => $e:expr) => {
        match $s {
            Loop::C2($c) => $e,
            Loop::C4($c) => $e,
            Loop::C8($c) => $e,
            Loop::Dd($c) => $e,
            Loop::Ml($c) => $e,
        }
    };
}

impl Loop {
    fn new(order: PskOrder, det: PskCarrier, cfg: CarrierConfig) -> Result<Loop, ModemError> {
        Ok(match (det, order) {
            (PskCarrier::Costas, PskOrder::Bpsk) => Loop::C2(CarrierSync::new(Costas::<2>, cfg)?),
            (PskCarrier::Costas, PskOrder::Qpsk | PskOrder::Oqpsk) => {
                Loop::C4(CarrierSync::new(Costas::<4>, cfg)?)
            }
            (PskCarrier::Costas, PskOrder::Psk8) => Loop::C8(CarrierSync::new(Costas::<8>, cfg)?),
            (PskCarrier::DecisionDirected, PskOrder::Bpsk | PskOrder::Qpsk | PskOrder::Oqpsk) => {
                Loop::Dd(CarrierSync::new(Decided(order), cfg)?)
            }
            (PskCarrier::MlTanh { n0 }, PskOrder::Bpsk) => {
                Loop::Ml(CarrierSync::new(MlTanh::new(n0)?, cfg)?)
            }
            _ => return Param::Points.conflict(Param::Loop),
        })
    }
    /// Derotates the symbols of `z` in place: one loop step for the run, or with `free` no step at all.
    #[inline]
    fn run(&mut self, z: &mut [Complex32], free: bool) {
        if free {
            for v in z {
                *v = each!(self, c => c.push_free(*v));
            }
        } else {
            each!(self, c => c.push_run(z))
        }
    }
    fn freq(&self) -> f64 {
        each!(self, c => c.freq())
    }
    fn set_freq(&mut self, f: f64) {
        each!(self, c => c.set_freq(f))
    }
    fn phase(&self) -> f64 {
        each!(self, c => c.phase())
    }
    fn rotate(&mut self, cycles: f64) {
        each!(self, c => c.rotate(cycles))
    }
    fn locked(&self) -> bool {
        each!(self, c => c.locked())
    }
    fn lock_metric(&self) -> f32 {
        each!(self, c => c.lock_metric())
    }
}

/// The most symbols between two steps of the timing loop and of the carrier loop.
const RUN: usize = 8;
/// The run length times the tracking bandwidth of a loop stays under this.
const RUN_BANDWIDTH: f64 = 0.04;
/// The most symbols between two steps of the timing loop of offset QPSK: the rails of a run are split with the carrier
/// as it was at the start of the run.
const RUN_OFFSET: usize = 4;
/// Fractional delays of the matched filter bank.
const PHASES: usize = 128;
/// Samples mixed at a time.
const PIECE: usize = 256;

type Bank<const TAPS: usize> = RunSymbolSync<Complex32, MatchedBank<TAPS, PHASES>>;

/// The matched filter and the timing loop. A root raised cosine whose span fits 64 taps is filtered at the symbol instants
/// only, by a bank of matched filters; any other pulse goes through a filter at the sample rate and a cubic interpolator.
#[derive(Clone, Debug)]
enum Timing {
    B16(Box<Bank<16>>),
    B32(Box<Bank<32>>),
    B64(Box<Bank<64>>),
    Filtered(Box<Fir<Complex32>>, Box<RunSymbolSync<Complex32, Cubic>>),
}

macro_rules! timing {
    ($s:expr, $t:ident => $e:expr) => {
        match $s {
            Timing::B16($t) => $e,
            Timing::B32($t) => $e,
            Timing::B64($t) => $e,
            Timing::Filtered(_, $t) => $e,
        }
    };
}

impl Timing {
    fn new(cfg: &PskDemodConfig) -> Result<Timing, ModemError> {
        let mut t = SymbolSyncConfig::new(cfg.sps);
        t.pulse = cfg.pulse.overall();
        t.bn_t = cfg.timing_bn_t;
        t.gears = cfg.timing_gears;
        t.max_ppm = cfg.max_ppm;
        t.oqpsk = cfg.order == PskOrder::Oqpsk;
        let track = cfg.timing_gears.map_or(cfg.timing_bn_t, |g| g.track_bn_t);
        let run = if t.oqpsk {
            run_length(track).min(RUN_OFFSET)
        } else {
            run_length(track)
        };
        let taps = cfg.mf_span * cfg.sps;
        let mut timing = match cfg.pulse {
            PskPulse::Rrc(b) if taps <= 64.0 => {
                let shape = Shape::Rrc {
                    beta: b.clamp(0.01, 1.0),
                };
                if taps <= 16.0 {
                    Timing::B16(Box::new(RunSymbolSync::new(MatchedBank::new(shape, cfg.sps)?, t, run)?))
                } else if taps <= 32.0 {
                    Timing::B32(Box::new(RunSymbolSync::new(MatchedBank::new(shape, cfg.sps)?, t, run)?))
                } else {
                    Timing::B64(Box::new(RunSymbolSync::new(MatchedBank::new(shape, cfg.sps)?, t, run)?))
                }
            }
            _ => {
                let taps = cfg.pulse.matched_filter(cfg.sps, cfg.mf_span)?;
                Timing::Filtered(
                    Box::new(Fir::new(&taps)),
                    Box::new(RunSymbolSync::new(Cubic, t, run)?),
                )
            }
        };
        timing.reset();
        Ok(timing)
    }
    /// Forgets the signal. The symbol instants start at the first sample of the next input and whole symbols after it
    /// (a signal that starts with a symbol at its first sample is read at its symbols from the start).
    fn reset(&mut self) {
        match self {
            Timing::B16(t) => t.reset_to(0.0),
            Timing::B32(t) => t.reset_to(0.0),
            Timing::B64(t) => t.reset_to(0.0),
            Timing::Filtered(f, t) => {
                f.reset();
                t.reset_to(f.group_delay());
            }
        }
    }
    #[inline]
    fn need(&self) -> usize {
        timing!(self, t => t.need())
    }
    #[inline]
    fn set_carrier(&mut self, phase: f64, per_symbol: f64) {
        timing!(self, t => t.set_carrier(phase, per_symbol))
    }
    /// Takes mixed samples from the front of `x` (changed in place by the filter) until the next run of symbols is whole,
    /// and appends that run to `out`. Returns the samples taken and whether the run was made.
    #[inline]
    fn process_run(&mut self, x: &mut [Complex32], out: &mut Vec<Complex32>) -> (usize, bool) {
        match self {
            Timing::B16(t) => t.process_run(x, out),
            Timing::B32(t) => t.process_run(x, out),
            Timing::B64(t) => t.process_run(x, out),
            Timing::Filtered(f, t) => {
                let n = t.need().min(x.len());
                f.process_in_place(&mut x[..n]);
                t.process_run(&x[..n], out)
            }
        }
    }
    fn clock_ppm(&self) -> f64 {
        timing!(self, t => t.clock_ppm())
    }
    fn sps_now(&self) -> f64 {
        timing!(self, t => t.sps_now())
    }
}

/// The symbols between two loop steps for a loop of bandwidth `bn_t` per symbol: up to [`RUN`].
fn run_length(bn_t: f64) -> usize {
    ((RUN_BANDWIDTH / bn_t.max(1e-9)) as usize).clamp(1, RUN)
}

/// The streaming PSK demodulator.
///
/// The samples are mixed by the carrier estimate and matched filtered at the instants of the timing loop, a Gardner loop
/// that steers the clock once per run of up to 8 symbols when it is locked. Each run of symbols then goes through the
/// carrier loop, which takes one step per run. For offset QPSK the timing loop sees the carrier taken off, so it pairs
/// the rails. When the carrier loop's frequency passes the hand-over limit, it moves into the mixer.
///
/// The samples are taken run by run: a change of the mixer or a reset between two runs acts on the samples after those
/// that the last run needed. So the output does not depend on how the input is split across calls.
///
/// ```
/// use sdrkit_algo_modem::psk::{PskDemodConfig, PskDemodulator, PskOrder};
/// use sdrkit_algo_modem::Complex32;
/// let mut d = PskDemodulator::new(PskDemodConfig::new(PskOrder::Bpsk, 8.0)).unwrap();
/// let x: Vec<Complex32> = (0..8000)
///     .map(|i| Complex32::new(if (i / 8) % 3 == 0 { -1.0 } else { 1.0 }, 0.0))
///     .collect();
/// let mut symbols = Vec::new();
/// d.process(&x, &mut symbols);
/// // 1000 symbols went in. The first has no half-way instant before it, and the last few wait for the samples of half the
/// // matched filter and of the rest of their run.
/// assert!((990..1000).contains(&symbols.len()));
/// ```
#[derive(Clone, Debug)]
pub struct PskDemodulator {
    cfg: PskDemodConfig,
    /// The mixer, cycles per sample (minus the carrier offset).
    nco: Nco,
    timing: Timing,
    carrier: Loop,
    /// The carrier run length.
    run: usize,
    /// The symbols of the timing loop that wait for the carrier loop, from `at` on.
    syms: Vec<Complex32>,
    at: usize,
    pending: Option<(usize, f64)>,
    evm_acc: f64,
    symbols: u64,
}

impl PskDemodulator {
    /// A demodulator for `cfg`. Errors when `sps` is not above 1, a bandwidth is out of range, the pulse cannot make a matched
    /// filter, or the carrier detector does not fit the order.
    pub fn new(cfg: PskDemodConfig) -> Result<Self, ModemError> {
        if !(cfg.sps > 1.0 && cfg.sps.is_finite()) {
            return Param::Sps.at_least(1.0, cfg.sps);
        }
        Ok(PskDemodulator {
            nco: Nco::new(1.0, 0.0),
            timing: Timing::new(&cfg)?,
            carrier: Self::carrier_loop(&cfg, cfg.carrier, cfg.carrier_bn_t)?,
            run: run_length(cfg.carrier_bn_t),
            syms: Vec::with_capacity(4 * RUN),
            at: 0,
            pending: None,
            evm_acc: 1.0,
            symbols: 0,
            cfg,
        })
    }

    fn carrier_loop(cfg: &PskDemodConfig, det: PskCarrier, bn_t: f64) -> Result<Loop, ModemError> {
        let mut c = CarrierConfig::new(bn_t);
        c.fll_bn_t = cfg.fll_bn_t;
        c.max_freq = cfg.max_freq;
        Loop::new(cfg.order, det, c)
    }

    /// The configuration.
    pub fn config(&self) -> &PskDemodConfig {
        &self.cfg
    }

    /// Starts the matched filter, the timing loop and the carrier loop again, and forgets the EVM and the symbol count. The mixer
    /// keeps its frequency.
    pub fn reset(&mut self) -> Result<(), ModemError> {
        self.timing.reset();
        self.syms.clear();
        self.at = 0;
        self.carrier = Self::carrier_loop(&self.cfg, self.cfg.carrier, self.cfg.carrier_bn_t)?;
        self.run = run_length(self.cfg.carrier_bn_t);
        self.pending = None;
        self.evm_acc = 1.0;
        self.symbols = 0;
        Ok(())
    }

    /// Changes the carrier loop to `det` at `bn_t` per symbol. The new loop starts at the old one's frequency and phase. Errors as
    /// [`PskDemodulator::new`] does for the loop.
    pub fn set_carrier_loop(&mut self, det: PskCarrier, bn_t: f64) -> Result<(), ModemError> {
        let mut c = Self::carrier_loop(&self.cfg, det, bn_t)?;
        c.set_freq(self.carrier.freq());
        c.rotate(self.carrier.phase());
        self.carrier = c;
        self.run = run_length(bn_t);
        self.cfg.carrier = det;
        self.cfg.carrier_bn_t = bn_t;
        Ok(())
    }

    /// Moves the frequency the carrier loop holds into the mixer now.
    pub fn hand_over(&mut self) {
        let f = self.carrier.freq();
        self.nco.set_freq(self.nco.freq() - f / self.cfg.sps);
        self.carrier.set_freq(0.0);
    }

    /// The carrier offset the demodulator holds (mixer and loop), cycles per sample.
    pub fn carrier_offset(&self) -> f64 {
        -self.nco.freq() + self.carrier.freq() / self.cfg.sps
    }

    /// The mixer's part of the carrier offset, cycles per sample.
    pub fn mixer_offset(&self) -> f64 {
        -self.nco.freq()
    }

    /// Sets the mixer to take off `cycles_per_sample`.
    pub fn set_mixer_offset(&mut self, cycles_per_sample: f64) {
        self.nco.set_freq(-cycles_per_sample);
    }

    /// The carrier loop's frequency, cycles per symbol.
    pub fn loop_freq(&self) -> f64 {
        self.carrier.freq()
    }

    /// Sets the carrier loop's frequency, cycles per symbol.
    pub fn set_loop_freq(&mut self, cycles_per_symbol: f64) {
        self.carrier.set_freq(cycles_per_symbol);
    }

    /// The channel upstream moved its carrier by `cycles_per_sample`. The samples show it after `delay` more samples. From then
    /// on the mixer takes off that much less.
    pub fn retune_after(&mut self, delay: usize, cycles_per_sample: f64) {
        self.pending = Some((delay, cycles_per_sample));
    }

    /// True while a move of [`PskDemodulator::retune_after`] waits to reach the mixer.
    pub fn retune_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// The carrier loop's lock value: about 1 on the points, near 0 for noise.
    pub fn lock_metric(&self) -> f32 {
        self.carrier.lock_metric()
    }

    /// The root mean square distance of the symbols from their points (unit power), averaged over about 100 symbols.
    pub fn evm(&self) -> f64 {
        self.evm_acc.sqrt()
    }

    /// Symbols out since the start or the last reset.
    pub fn symbols(&self) -> u64 {
        self.symbols
    }

    /// The timing loop's clock error, ppm (positive when symbols are longer than nominal).
    pub fn clock_ppm(&self) -> f64 {
        self.timing.clock_ppm()
    }

    /// Samples per symbol the timing loop runs at now.
    pub fn sps_now(&self) -> f64 {
        self.timing.sps_now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::modulate::Linear;

    /// `n` symbols of `order` from a fixed sequence, root raised cosine 0.35 at 8 samples per symbol, the carrier at `f` cycles
    /// per sample.
    fn signal(order: PskOrder, n: usize, f: f64) -> Vec<Complex32> {
        let mut m =
            Linear::new(sdrkit_algo_dsp::pulse::Shape::Rrc { beta: 0.35 }, 16.0, 8.0).unwrap();
        if order == PskOrder::Oqpsk {
            m = m.with_offset_q();
        }
        let mut s = 7u32;
        let syms: Vec<Complex32> = (0..n)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                order.point((s >> 16) as usize)
            })
            .collect();
        let mut x = Vec::new();
        m.push(&syms, &mut x);
        for (i, v) in x.iter_mut().enumerate() {
            *v *= Complex32::from_polar(1.0, (std::f64::consts::TAU * f * i as f64) as f32);
        }
        x
    }

    #[test]
    fn locks_every_order_through_an_offset() {
        for o in PskOrder::ALL {
            let f = 0.002;
            let x = signal(o, 6000, f);
            let mut d = PskDemodulator::new(PskDemodConfig::new(o, 8.0)).unwrap();
            d.process(&x, &mut Vec::new());
            assert!(d.evm() < 0.15, "{o:?} evm {}", d.evm());
            assert!(
                (d.carrier_offset() - f).abs() < 2e-4,
                "{o:?} {}",
                d.carrier_offset()
            );
            assert!(d.lock_metric() > 0.8, "{o:?} lock {}", d.lock_metric());
        }
    }

    #[test]
    fn decision_directed_and_ml_loops_lock() {
        for (o, c) in [
            (PskOrder::Qpsk, PskCarrier::DecisionDirected),
            (PskOrder::Bpsk, PskCarrier::MlTanh { n0: 0.1 }),
        ] {
            let x = signal(o, 4000, 0.0005);
            let mut cfg = PskDemodConfig::new(o, 8.0);
            cfg.carrier = c;
            cfg.fll_bn_t = None;
            let mut d = PskDemodulator::new(cfg).unwrap();
            d.process(&x, &mut Vec::new());
            assert!(d.evm() < 0.15, "{o:?} evm {}", d.evm());
        }
        let mut cfg = PskDemodConfig::new(PskOrder::Psk8, 8.0);
        cfg.carrier = PskCarrier::DecisionDirected;
        assert!(PskDemodulator::new(cfg).is_err());
    }

    /// `n` symbols of `order` at 4 samples per symbol, root raised cosine 0.5, with the carrier at 0.0001 cycles per sample
    /// and 0.3 cycles, the clock 50 ppm fast, and noise at `es_n0_db`.
    fn impaired(order: PskOrder, n: usize, es_n0_db: f64) -> Vec<Complex32> {
        let mut m = Linear::new(sdrkit_algo_dsp::pulse::Shape::Rrc { beta: 0.5 }, 16.0, 4.0).unwrap();
        if order == PskOrder::Oqpsk {
            m = m.with_offset_q();
        }
        let mut s = 99u32;
        let syms: Vec<Complex32> = (0..n)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                order.point((s >> 16) as usize)
            })
            .collect();
        let mut clean = Vec::new();
        m.push(&syms, &mut clean);
        let mut x = Vec::new();
        sdrkit_algo_sim::clock::ClockError::new(50.0)
            .unwrap()
            .process(&clean, &mut x);
        let power = x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
        for (i, v) in x.iter_mut().enumerate() {
            let turn = std::f64::consts::TAU * (1e-4 * i as f64 + 0.3);
            *v *= Complex32::from_polar(1.0, turn as f32);
        }
        let noise = power * 4.0 / 10f64.powf(es_n0_db / 10.0);
        sdrkit_algo_sim::noise::Awgn::with_power(noise, 5)
            .unwrap()
            .process(&mut x);
        x
    }

    /// The loop numbers of both chains of the comparison.
    const BN_T: f64 = 0.004;

    /// The symbols of `x` through a matched filter at the sample rate, a Gardner loop with a cubic interpolator that takes
    /// a step per symbol, and a carrier loop that takes a step per symbol and decides on the nearest point of the
    /// constellation.
    fn step_per_symbol(order: PskOrder, x: &[Complex32]) -> Vec<Complex32> {
        use crate::carrier::DecisionDirected;
        use crate::timing::{Gardner, SymbolSync};
        use crate::{Constellation, PointSet};
        let taps = PskPulse::Rrc(0.5).matched_filter(4.0, 12.0).unwrap();
        let mut y = vec![Complex32::new(0.0, 0.0); x.len()];
        Fir::new(&taps).process(x, &mut y);
        let mut t = SymbolSyncConfig::new(4.0);
        t.pulse = PskPulse::Rrc(0.5).overall();
        t.bn_t = BN_T;
        t.max_ppm = 5000.0;
        t.oqpsk = order == PskOrder::Oqpsk;
        let mut sync = SymbolSync::new(Gardner, Cubic, t).unwrap();
        let mut c = CarrierConfig::new(BN_T);
        c.max_freq = 0.04;
        let mut out = Vec::new();
        match order {
            PskOrder::Psk8 => {
                let mut l = CarrierSync::new(Costas::<8>, c).unwrap();
                sync.process_each(&y, |z| {
                    out.push(l.push(z));
                    Some(l.phase())
                });
            }
            _ => {
                let set = if order == PskOrder::Bpsk {
                    PointSet::bpsk()
                } else {
                    PointSet::qpsk()
                };
                let det = DecisionDirected::new(Constellation::new(set).unwrap());
                let mut l = CarrierSync::new(det, c).unwrap();
                sync.process_each(&y, |z| {
                    out.push(l.push(z));
                    Some(l.phase())
                });
            }
        }
        out
    }

    /// The symbols of `x` through the demodulator with the same loop numbers.
    fn run_wise(order: PskOrder, x: &[Complex32]) -> Vec<Complex32> {
        let mut c = PskDemodConfig::new(order, 4.0);
        c.pulse = PskPulse::Rrc(0.5);
        c.mf_span = 12.0;
        c.timing_bn_t = BN_T;
        c.timing_gears = None;
        c.max_ppm = 5000.0;
        c.carrier = if order == PskOrder::Psk8 {
            PskCarrier::Costas
        } else {
            PskCarrier::DecisionDirected
        };
        c.carrier_bn_t = BN_T;
        c.fll_bn_t = None;
        c.max_freq = 0.04;
        c.handover = None;
        c.hold_symbols = 0;
        let mut out = Vec::new();
        PskDemodulator::new(c).unwrap().process(x, &mut out);
        out
    }

    /// The mean square distance of the symbols from their nearest points, and the mean square size of the symbols.
    fn spread(order: PskOrder, z: &[Complex32]) -> (f64, f64) {
        let n = z.len() as f64;
        (
            z.iter().map(|&v| (v - order.nearest(v)).norm_sqr() as f64).sum::<f64>() / n,
            z.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / n,
        )
    }

    /// The least mean square difference of the last `n` symbols of `a` and of `b`, against the mean square size of the
    /// symbols, dB, over shifts of up to `reach` symbols. The two carrier loops may lock a symmetry of the order apart, so `b` is
    /// tried at each turn of the order. Two offset QPSK receivers may also pair the rails half a symbol apart, so for that
    /// order each rail of `a` is compared with each rail of `b`, of either sign, at its own shift.
    fn difference_db(order: PskOrder, a: &[Complex32], b: &[Complex32], n: usize, reach: usize) -> f64 {
        let (a, b) = (&a[a.len() - n - 2 * reach..], &b[b.len() - n - 2 * reach..]);
        let size = a.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / a.len() as f64;
        let mean = |d: &dyn Fn(usize) -> f32| (0..n).map(|k| d(k) as f64).sum::<f64>() / n as f64;
        if order != PskOrder::Oqpsk {
            let mut least = f64::INFINITY;
            for shift in 0..=2 * reach {
                for turn in 0..order.m() {
                    let r = Complex32::from_polar(1.0, std::f32::consts::TAU * turn as f32 / order.m() as f32);
                    least = least.min(mean(&|k| (a[k + reach] - b[k + shift] * r).norm_sqr()));
                }
            }
            return 10.0 * (least / size).log10();
        }
        let rail = |z: &[Complex32], q: bool, k: usize| if q { z[k].im } else { z[k].re };
        let mut total = 0.0;
        for qa in [false, true] {
            let mut least = f64::INFINITY;
            for qb in [false, true] {
                for sign in [1.0f32, -1.0] {
                    for shift in 0..=2 * reach {
                        least = least
                            .min(mean(&|k| (rail(a, qa, k + reach) - sign * rail(b, qb, k + shift)).powi(2)));
                    }
                }
            }
            total += least;
        }
        10.0 * (total / size).log10()
    }

    #[test]
    fn the_run_wise_loops_give_the_symbols_of_a_step_per_symbol() {
        // (order, Es/N0 dB, the most the two chains may differ, dB under the symbols)
        for (o, es_n0, limit) in [
            (PskOrder::Bpsk, 7.0, -35.0),
            (PskOrder::Qpsk, 10.0, -35.0),
            (PskOrder::Oqpsk, 10.0, -25.0),
            (PskOrder::Psk8, 16.0, -35.0),
        ] {
            let x = impaired(o, 24_000, es_n0);
            let (old, new) = (step_per_symbol(o, &x), run_wise(o, &x));
            assert!(old.len() > 23_900 && new.len() > 23_900, "{o:?} {} {}", old.len(), new.len());
            let settled = 16_000;
            let d = difference_db(o, &old, &new, settled, 12);
            let (eo, so) = spread(o, &old[old.len() - settled..]);
            let (en, sn) = spread(o, &new[new.len() - settled..]);
            assert!(d < limit, "{o:?} difference {d:.1} dB");
            // The distance from the points, against the size of the symbols, is no more than 0.1 dB above that of a step
            // per symbol.
            assert!(en / sn < eo / so * 1.023, "{o:?} {} {}", en / sn, eo / so);
        }
    }

    /// `n` symbols of `order` at `sps` samples per symbol (the clock 50 ppm slow), root raised cosine 0.5, the carrier at
    /// 0.0004 cycles per symbol and 0.3 cycles, with noise at `es_n0_db`.
    fn fast_link(order: PskOrder, n: usize, sps: f64, es_n0_db: f64) -> Vec<Complex32> {
        let shape = sdrkit_algo_dsp::pulse::Shape::Rrc { beta: 0.5 };
        let mut m = Linear::new(shape, 16.0, sps * (1.0 + 50e-6)).unwrap();
        if order == PskOrder::Oqpsk {
            m = m.with_offset_q();
        }
        let mut s = 77u32;
        let syms: Vec<Complex32> = (0..n)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                order.point((s >> 16) as usize)
            })
            .collect();
        let mut x = Vec::new();
        m.push(&syms, &mut x);
        let power = x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
        for (i, v) in x.iter_mut().enumerate() {
            let turn = std::f64::consts::TAU * (4e-4 / sps * i as f64 + 0.3);
            *v *= Complex32::from_polar(1.0, turn as f32);
        }
        let noise = power * sps / 10f64.powf(es_n0_db / 10.0);
        sdrkit_algo_sim::noise::Awgn::with_power(noise, 6)
            .unwrap()
            .process(&mut x);
        x
    }

    /// The symbols of `x` through an interpolation by 2, a matched filter over 12 symbols at the doubled rate, a Gardner
    /// loop with a cubic interpolator and a carrier loop on the nearest point, each with a step per symbol.
    fn interpolated_step_per_symbol(order: PskOrder, x: &[Complex32], sps: f64) -> Vec<Complex32> {
        use crate::carrier::DecisionDirected;
        use crate::timing::{Gardner, SymbolSync};
        use crate::{Constellation, PointSet};
        let mut wide = Vec::new();
        sdrkit_algo_dsp::resample::RationalResampler::with_spec(2, 1, 0.45, 0.55, 50.0)
            .unwrap()
            .process(x, &mut wide);
        let taps = PskPulse::Rrc(0.5).matched_filter(2.0 * sps, 12.0).unwrap();
        let mut y = vec![Complex32::new(0.0, 0.0); wide.len()];
        Fir::new(&taps).process(&wide, &mut y);
        let mut t = SymbolSyncConfig::new(2.0 * sps);
        t.pulse = PskPulse::Rrc(0.5).overall();
        t.bn_t = BN_T;
        t.max_ppm = 5000.0;
        t.oqpsk = order == PskOrder::Oqpsk;
        let mut sync = SymbolSync::new(Gardner, Cubic, t).unwrap();
        let mut c = CarrierConfig::new(BN_T);
        c.max_freq = 0.04;
        let set = if order == PskOrder::Bpsk {
            PointSet::bpsk()
        } else {
            PointSet::qpsk()
        };
        let det = DecisionDirected::new(Constellation::new(set).unwrap());
        let mut l = CarrierSync::new(det, c).unwrap();
        let mut out = Vec::new();
        sync.process_each(&y, |z| {
            out.push(l.push(z));
            Some(l.phase())
        });
        out
    }

    #[test]
    fn a_fast_link_gives_the_symbols_of_the_interpolated_step_per_symbol() {
        // The links of tens of megasymbols a second: under 2 samples per symbol and a matched filter over 8 symbols, read
        // from the samples as they are. (order, samples per symbol, Es/N0 dB)
        for (o, sps, es_n0) in [
            (PskOrder::Bpsk, 45.0 / 30.72, 7.0),
            (PskOrder::Qpsk, 45.0 / 35.0, 10.0),
            (PskOrder::Oqpsk, 40.0 / 25.0, 10.0),
        ] {
            let x = fast_link(o, 24_000, sps, es_n0);
            let old = interpolated_step_per_symbol(o, &x, sps);
            let mut c = PskDemodConfig::new(o, sps);
            c.pulse = PskPulse::Rrc(0.5);
            c.mf_span = 8.0;
            c.timing_bn_t = BN_T;
            c.timing_gears = None;
            c.max_ppm = 5000.0;
            c.carrier = PskCarrier::DecisionDirected;
            c.carrier_bn_t = BN_T;
            c.fll_bn_t = None;
            c.max_freq = 0.04;
            c.handover = None;
            c.hold_symbols = 0;
            let mut new = Vec::new();
            PskDemodulator::new(c).unwrap().process(&x, &mut new);
            assert!(old.len() > 23_900 && new.len() > 23_900, "{o:?} {} {}", old.len(), new.len());
            let settled = 16_000;
            let d = difference_db(o, &old, &new, settled, 64);
            let (eo, so) = spread(o, &old[old.len() - settled..]);
            let (en, sn) = spread(o, &new[new.len() - settled..]);
            assert!(d < -25.0, "{o:?} difference {d:.1} dB");
            assert!(en / sn < eo / so * 1.023, "{o:?} {} {}", en / sn, eo / so);
        }
    }

    #[test]
    fn bad_rates_fail() {
        assert!(PskDemodulator::new(PskDemodConfig::new(PskOrder::Bpsk, 1.0)).is_err());
        assert!(PskDemodulator::new(PskDemodConfig::new(PskOrder::Bpsk, f64::NAN)).is_err());
    }
}
