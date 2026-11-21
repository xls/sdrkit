//! `modem.symbol_sync`: symbol timing recovery.

use super::{is_retune, shape, split_at_tags, PulseShape};
use sdrkit_algo_dsp::interp::{Cubic, Interpolator, Linear, Polyphase, MAX_TAPS};
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_modem::timing::{
    EarlyLate, Gardner, MatchedInterp, MuellerMuller, SymbolSync, SymbolSyncConfig, TimingDetector,
    ZeroCrossing,
};
use sdrkit_core::publish::{field, keys, Published};
use sdrkit_core::{
    Choice, Complex32, ConfigError, Results, Setting, SettingError, SettingId, SettingSet, Value,
    Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Every, Format, Io, Kind, Port, TagCarry, TagKind,
};

sdrkit_core::choice_enum! {
    /// A timing error detector of [`SymbolSyncComponent`].
    pub enum TimingDetectorKind {
        /// Gardner.
        Gardner = ("gardner", "Gardner"),
        /// Mueller and Muller.
        Mm = ("mm", "Mueller and Muller"),
        /// Early-late.
        EarlyLate = ("early_late", "Early-late"),
        /// Zero crossing.
        ZeroCrossing = ("zero_crossing", "Zero crossing"),
    }
}

sdrkit_core::choice_enum! {
    /// An interpolator of [`SymbolSyncComponent`].
    pub enum InterpKind {
        /// Linear.
        Linear = ("linear", "Linear"),
        /// Cubic.
        Cubic = ("cubic", "Cubic"),
        /// Polyphase.
        Polyphase = ("polyphase", "Polyphase"),
        /// Matched polyphase.
        Matched = ("matched", "Matched polyphase"),
    }
}

/// Phases of the polyphase banks.
const PHASES: usize = 64;

/// One monomorphised synchroniser behind a per-call interface (the per-sample work stays inside the generic code).
trait SyncCore: Send {
    /// Runs `x`, appending symbols to `syms`, one timing error per symbol to `errs`, and `(symbol index, locked)` at each change
    /// of the lock test to `locks`.
    fn run(
        &mut self,
        x: &[Complex32],
        syms: &mut Vec<Complex32>,
        errs: &mut Vec<f32>,
        locks: &mut Vec<(usize, bool)>,
    );
    fn reset(&mut self);
    fn clock_ppm(&self) -> f64;
    fn locked(&self) -> bool;
}

impl<D, I> SyncCore for SymbolSync<Complex32, D, I>
where
    D: TimingDetector<Complex32> + Send,
    I: Interpolator + Send,
{
    fn run(
        &mut self,
        x: &[Complex32],
        syms: &mut Vec<Complex32>,
        errs: &mut Vec<f32>,
        locks: &mut Vec<(usize, bool)>,
    ) {
        for v in x {
            let (before, was) = (self.symbols(), self.locked());
            self.process(std::slice::from_ref(v), syms);
            let after = self.symbols();
            if after != before {
                let e = self.last_error();
                for _ in before..after {
                    errs.push(e);
                }
            }
            let now = self.locked();
            if now != was {
                locks.push((syms.len().saturating_sub(1), now));
            }
        }
    }
    fn reset(&mut self) {
        SymbolSync::reset(self)
    }
    fn clock_ppm(&self) -> f64 {
        SymbolSync::clock_ppm(self)
    }
    fn locked(&self) -> bool {
        SymbolSync::locked(self)
    }
}

fn boxed<D, I>(det: D, interp: I, cfg: SymbolSyncConfig) -> Result<Box<dyn SyncCore>, ConfigError>
where
    D: TimingDetector<Complex32> + Send + 'static,
    I: Interpolator + Send + 'static,
{
    Ok(Box::new(SymbolSync::new(det, interp, cfg)?))
}

fn with_interp<D>(
    det: D,
    interp: InterpKind,
    matched: Shape,
    span: f64,
    cfg: SymbolSyncConfig,
) -> Result<Box<dyn SyncCore>, ConfigError>
where
    D: TimingDetector<Complex32> + Send + 'static,
{
    match interp {
        InterpKind::Linear => boxed(det, Linear, cfg),
        InterpKind::Cubic => boxed(det, Cubic, cfg),
        InterpKind::Polyphase => boxed(det, Polyphase::<16, PHASES>::design(0.8, 60.0)?, cfg),
        InterpKind::Matched => {
            // The matched bank covers the pulse's span: the smallest of 8, 16, 32 or 64 taps that holds it.
            let need = (span * cfg.sps).ceil() as usize;
            match need {
                0..=8 => boxed(det, MatchedInterp::<8, PHASES>::new(matched, cfg.sps)?, cfg),
                9..=16 => boxed(
                    det,
                    MatchedInterp::<16, PHASES>::new(matched, cfg.sps)?,
                    cfg,
                ),
                17..=32 => boxed(
                    det,
                    MatchedInterp::<32, PHASES>::new(matched, cfg.sps)?,
                    cfg,
                ),
                33..=MAX_TAPS => boxed(
                    det,
                    MatchedInterp::<64, PHASES>::new(matched, cfg.sps)?,
                    cfg,
                ),
                _ => Err(SettingError::Conflict {
                    id: SymbolSyncComponentSetting::Span.id(),
                    with: SymbolSyncComponentSetting::Sps.id(),
                }
                .into()),
            }
        }
    }
}

const SPS: Setting<field::F64> =
    Setting::new("sps", "Samples per symbol", "", 4.0).range(1.01, 1024.0, 0.0);
const DETECTOR: Setting<field::Choice<TimingDetectorKind>> =
    Setting::choice("detector", "Timing detector", TimingDetectorKind::Gardner);
const INTERP: Setting<field::Choice<InterpKind>> =
    Setting::choice("interp", "Interpolator", InterpKind::Cubic);
const BANDWIDTH: Setting<field::F64> = Setting::new("bandwidth", "Loop bandwidth", "", 0.01)
    .range(1e-5, 0.25, 0.0)
    .help("Loop noise bandwidth per symbol (0.005 to 0.02 is usual).");
const DAMPING: Setting<field::F64> =
    Setting::new("damping", "Damping", "", std::f64::consts::FRAC_1_SQRT_2).range(0.1, 4.0, 0.0);
const MAX_PPM: Setting<field::F64> =
    Setting::new("max_ppm", "Clock range", "ppm", 2000.0).range(0.0, 100_000.0, 0.0);
const OQPSK: Setting<field::Bool> = Setting::new("oqpsk", "Offset QPSK", "", false);
const OUTPUTS_PER_SYMBOL: Setting<field::I64> =
    Setting::new("outputs_per_symbol", "Outputs per symbol", "", 1).range(1.0, 2.0, 1.0);
const PULSE: Setting<field::Choice<PulseShape>> =
    Setting::choice("pulse", "Pulse at the detector", PulseShape::Rc);
const MATCHED_PULSE: Setting<field::Choice<PulseShape>> =
    Setting::choice("matched_pulse", "Matched filter pulse", PulseShape::Rrc);
const BETA: Setting<field::F64> = Setting::new("beta", "Roll-off", "", 0.35).range(0.0, 1.0, 0.0);
const BT: Setting<field::F64> = Setting::new("bt", "Gaussian BT", "", 0.5).range(0.05, 10.0, 0.0);
const SPAN: Setting<field::F64> =
    Setting::new("span", "Matched filter span", "symbols", 8.0).range(1.0, 64.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`SymbolSyncComponent`].
    pub enum SymbolSyncComponentSetting {
        /// Samples per symbol.
        Sps = SPS,
        /// Timing detector.
        Detector = DETECTOR,
        /// Interpolator.
        Interp = INTERP,
        /// Loop bandwidth.
        Bandwidth = BANDWIDTH,
        /// Damping.
        Damping = DAMPING,
        /// Clock range.
        MaxPpm = MAX_PPM,
        /// Offset QPSK.
        Oqpsk = OQPSK,
        /// Outputs per symbol.
        OutputsPerSymbol = OUTPUTS_PER_SYMBOL,
        /// Pulse at the detector.
        Pulse = PULSE,
        /// Matched filter pulse.
        MatchedPulse = MATCHED_PULSE,
        /// Roll-off.
        Beta = BETA,
        /// Gaussian BT.
        Bt = BT,
        /// Matched filter span.
        Span = SPAN,
    }
}

/// Symbol timing recovery (`modem.symbol_sync`): `Iq` samples in, `Symbols` out at the symbol rate times `outputs_per_symbol`,
/// and the timing error (`Real`, one per symbol, at the symbol rate) on a second output for display or tuning.
///
/// Wraps `sdrkit_algo_modem::timing::SymbolSync`: the symbols are read at fractional instants through the interpolator, the detector
/// returns one error per symbol, and a second-order loop of noise bandwidth `bandwidth` (per symbol) and `damping` steers the
/// instants; the loop's integrator holds the clock error, clamped to `max_ppm`. The symbols come out at unit mean power.
///
/// Settings (each rebuilds the loop):
///
/// - `sps`: input samples per symbol (above 1, fractional allowed).
/// - `detector`: `gardner`, `mm` (Mueller and Muller with sign decisions; needs the carrier), `early_late`, `zero_crossing`.
/// - `interp`: `linear`, `cubic`, `polyphase` (a 16-tap windowed-sinc bank), or `matched` (a polyphase bank of the `matched_pulse`
///   that filters and interpolates in one dot product per output, so no separate matched filter is needed).
/// - `bandwidth` (loop noise bandwidth per symbol), `damping`, `max_ppm` (clock range).
/// - `oqpsk` (Q read half a symbol after I), `outputs_per_symbol` (1, or 2 for the T/2 stream of a fractionally spaced equaliser:
///   mid-point, then symbol).
/// - `pulse`, `beta`, `bt`: the overall pulse at the detector (the crate's `SymbolSyncConfig::pulse`: `rc` for an RRC link), from
///   which the detector gain is measured so the loop has the requested bandwidth.
/// - `matched_pulse` and `span` (symbols): the filter of the `matched` interpolator (it shares `beta` and `bt`).
///
/// Tags: a `Retuned` tag resets the loop at its sample; `Lock { locked }` marks each change of the lock test at the symbol where
/// it happens; every input tag moves to the symbol (and error) made after its sample. Results: `clock_ppm`, `locked`.
pub struct SymbolSyncComponent {
    sps: f64,
    detector: TimingDetectorKind,
    interp: InterpKind,
    bandwidth: f64,
    damping: f64,
    max_ppm: f64,
    oqpsk: bool,
    outputs_per_symbol: usize,
    pulse: PulseShape,
    matched_pulse: PulseShape,
    beta: f64,
    bt: f64,
    span: f64,
    core: Option<Box<dyn SyncCore>>,
    syms: Vec<Complex32>,
    errs: Vec<f32>,
    locks: Vec<(usize, bool)>,
    carry_syms: TagCarry,
    carry_errs: TagCarry,
    every: Every,
    results: Published,
}

impl Default for SymbolSyncComponent {
    /// 4 samples per symbol, Gardner, cubic, bandwidth 0.01, damping 0.707, +-2000 ppm, RC 0.35 overall pulse.
    fn default() -> Self {
        SymbolSyncComponent {
            sps: SPS.default,
            detector: TimingDetectorKind::Gardner,
            interp: InterpKind::Cubic,
            bandwidth: BANDWIDTH.default,
            damping: DAMPING.default,
            max_ppm: MAX_PPM.default,
            oqpsk: OQPSK.default,
            outputs_per_symbol: OUTPUTS_PER_SYMBOL.default as usize,
            pulse: PulseShape::Rc,
            matched_pulse: PulseShape::Rrc,
            beta: BETA.default,
            bt: BT.default,
            span: SPAN.default,
            core: None,
            syms: Vec::new(),
            errs: Vec::new(),
            locks: Vec::new(),
            carry_syms: TagCarry::default(),
            carry_errs: TagCarry::default(),
            every: Every::default(),
            results: Published::new(&[&keys::CLOCK_PPM, &keys::LOCKED]),
        }
    }
}

impl SymbolSyncComponent {
    /// A synchroniser at `sps` samples per symbol with the default detector and interpolator.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `sps` setting refuses `sps`.
    pub fn new(sps: f64) -> Result<Self, SettingError> {
        SymbolSyncComponentSetting::Sps.check(&Value::F64(sps))?;
        Ok(SymbolSyncComponent {
            sps,
            ..Self::default()
        })
    }
    /// The clock error found, ppm (0 before configure).
    pub fn clock_ppm(&self) -> f64 {
        self.core.as_ref().map_or(0.0, |c| c.clock_ppm())
    }
    /// Whether the lock test passes.
    pub fn locked(&self) -> bool {
        self.core.as_ref().is_some_and(|c| c.locked())
    }
    fn build(&self) -> Result<Box<dyn SyncCore>, ConfigError> {
        let pulse = shape(self.pulse, self.beta, self.bt)?;
        let matched = shape(self.matched_pulse, self.beta, self.bt)?;
        let mut cfg = SymbolSyncConfig::new(self.sps);
        cfg.bn_t = self.bandwidth;
        cfg.zeta = self.damping;
        cfg.max_ppm = self.max_ppm;
        cfg.oqpsk = self.oqpsk;
        cfg.two_per_symbol = self.outputs_per_symbol == 2;
        cfg.pulse = pulse;
        let (i, s) = (self.interp, self.span);
        match self.detector {
            TimingDetectorKind::Gardner => with_interp(Gardner, i, matched, s, cfg),
            TimingDetectorKind::Mm => with_interp(MuellerMuller, i, matched, s, cfg),
            TimingDetectorKind::EarlyLate => {
                with_interp(EarlyLate::<Complex32>::default(), i, matched, s, cfg)
            }
            TimingDetectorKind::ZeroCrossing => with_interp(ZeroCrossing, i, matched, s, cfg),
        }
    }
    fn publish(&mut self) {
        let (ppm, locked) = (self.clock_ppm(), self.locked());
        self.results.set(&keys::CLOCK_PPM, super::r2(ppm));
        self.results.set(&keys::LOCKED, locked);
    }
}

impl Component for SymbolSyncComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "modem.symbol_sync",
            vec![Port::iq("in")],
            vec![Port::symbols("out"), Port::real("error")],
        )
        .manual_tags()
        .with_settings(SymbolSyncComponentSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        super::need_rate(f.rate)?;
        self.core = Some(self.build()?);
        let sym_rate = f.rate / self.sps;
        let out_rate = sym_rate * self.outputs_per_symbol as f64;
        self.carry_syms = TagCarry::new(f.rate, out_rate);
        self.carry_errs = TagCarry::new(f.rate, sym_rate);
        // Room for one call of the graph's largest piece, so steady state allocates nothing.
        self.syms = Vec::with_capacity(1024);
        self.errs = Vec::with_capacity(1024);
        self.locks = Vec::with_capacity(16);
        self.every = Every::new(sym_rate, 0.1);
        self.publish();
        Ok(vec![
            Format {
                bandwidth_hz: sym_rate,
                ..f.with_kind(Kind::Symbols).with_rate(out_rate)
            },
            Format {
                bandwidth_hz: sym_rate,
                ..Format::new(Kind::Real, sym_rate)
            },
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let input = io.input(0);
        let (s0, e0) = (io.output(0).first(), io.output(1).first());
        if self.core.is_none() {
            return;
        }
        self.syms.clear();
        self.errs.clear();
        self.locks.clear();
        split_at_tags(
            self,
            input,
            x.len(),
            |me, a, b| {
                if let Some(c) = &mut me.core {
                    c.run(&x[a..b], &mut me.syms, &mut me.errs, &mut me.locks);
                }
            },
            |me, t| {
                let (ps, pe) = (s0 + me.syms.len() as u64, e0 + me.errs.len() as u64);
                if is_retune(t) {
                    if let Some(c) = &mut me.core {
                        if c.locked() {
                            me.locks.push((me.syms.len(), false));
                        }
                        c.reset();
                    }
                }
                me.carry_syms.queue(ps, t.kind.clone());
                me.carry_errs.queue(pe, t.kind.clone());
            },
        );
        for &(at, locked) in &self.locks {
            self.carry_syms
                .queue(s0 + at as u64, TagKind::Lock { locked });
        }
        let n = self.errs.len();
        io.samples_out::<Complex32>(0).extend_from_slice(&self.syms);
        io.real_out(1).extend_from_slice(&self.errs);
        self.carry_syms.flush(io.output(0));
        self.carry_errs.flush(io.output(1));
        if self.every.tick(n) {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SymbolSyncComponentSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SymbolSyncComponentSetting::Sps => self.sps = SPS.read(id, v)?,
            SymbolSyncComponentSetting::Detector => self.detector = DETECTOR.read(id, v)?,
            SymbolSyncComponentSetting::Interp => self.interp = INTERP.read(id, v)?,
            SymbolSyncComponentSetting::Bandwidth => self.bandwidth = BANDWIDTH.read(id, v)?,
            SymbolSyncComponentSetting::Damping => self.damping = DAMPING.read(id, v)?,
            SymbolSyncComponentSetting::MaxPpm => self.max_ppm = MAX_PPM.read(id, v)?,
            SymbolSyncComponentSetting::Oqpsk => self.oqpsk = OQPSK.read(id, v)?,
            SymbolSyncComponentSetting::OutputsPerSymbol => {
                self.outputs_per_symbol = OUTPUTS_PER_SYMBOL.read(id, v)? as usize
            }
            SymbolSyncComponentSetting::Pulse => self.pulse = PULSE.read(id, v)?,
            SymbolSyncComponentSetting::MatchedPulse => {
                self.matched_pulse = MATCHED_PULSE.read(id, v)?
            }
            SymbolSyncComponentSetting::Beta => self.beta = BETA.read(id, v)?,
            SymbolSyncComponentSetting::Bt => self.bt = BT.read(id, v)?,
            SymbolSyncComponentSetting::Span => self.span = SPAN.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([
            (SymbolSyncComponentSetting::Sps, Value::F64(self.sps)),
            (
                SymbolSyncComponentSetting::Detector,
                Value::Choice(self.detector.index()),
            ),
            (
                SymbolSyncComponentSetting::Interp,
                Value::Choice(self.interp.index()),
            ),
            (
                SymbolSyncComponentSetting::Bandwidth,
                Value::F64(self.bandwidth),
            ),
            (
                SymbolSyncComponentSetting::Damping,
                Value::F64(self.damping),
            ),
            (SymbolSyncComponentSetting::MaxPpm, Value::F64(self.max_ppm)),
            (SymbolSyncComponentSetting::Oqpsk, Value::Bool(self.oqpsk)),
            (
                SymbolSyncComponentSetting::OutputsPerSymbol,
                Value::I64(self.outputs_per_symbol as i64),
            ),
            (
                SymbolSyncComponentSetting::Pulse,
                Value::Choice(self.pulse.index()),
            ),
            (
                SymbolSyncComponentSetting::MatchedPulse,
                Value::Choice(self.matched_pulse.index()),
            ),
            (SymbolSyncComponentSetting::Beta, Value::F64(self.beta)),
            (SymbolSyncComponentSetting::Bt, Value::F64(self.bt)),
            (SymbolSyncComponentSetting::Span, Value::F64(self.span)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}
