//! `modem.fsk_demod`: M-FSK by tone correlators or by a discriminator and a level slicer.

use super::{parse_list, pieces};
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_algo_modem::fsk::{
    DiscriminatorConfig, DiscriminatorLevels, FskLlr, LabelMap, LevelSlicer, MfskConfig, MfskDemod,
    TrackedFskLlr,
};
use sdrkit_core::publish::field;
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{ConfigError, Schema, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::SlicePool;
use sdrkit_graph::{
    Applied, Component, ComponentInfo, Every, Format, Io, Item, Kind, Port, RateFrom, Timed,
};
use std::sync::{Arc, OnceLock};

/// The tone energies of one symbol (output `energies` of `modem.fsk_demod` with the correlator): `|S_t|^2` of each tone, in the
/// order of the tone list. It flows only inside a graph.
#[derive(Clone, Debug, PartialEq)]
pub struct FskEnergies {
    /// The symbol's number since the start (or the last reset).
    pub symbol: u64,
    /// One energy per tone (shared, reused once every reader has dropped it).
    pub energies: Arc<[f32]>,
}

impl Item for FskEnergies {
    const KIND: &'static str = "fsk.energies";
}

/// Input samples per piece of the discriminator front end (`fsk::DiscriminatorLevels` works in pieces of this size), for
/// sizing the scratch buffer.
const PIECE: usize = 4096;

sdrkit_core::choice_enum! {
    /// How [`FskDemod`] distinguishes the tones.
    pub enum FskDetector {
        /// Tone correlators.
        Correlator = ("correlator", "Tone correlators"),
        /// Discriminator and level slicer.
        Discriminator = ("discriminator", "Discriminator and level slicer"),
    }
}

sdrkit_core::choice_enum! {
    /// The bits each tone or level of [`FskDemod`] carries.
    pub enum BitLabels {
        /// Natural (tone t is t).
        Natural = ("natural", "Natural (tone t is t)"),
        /// Gray.
        Gray = ("gray", "Gray"),
        /// Binary, first tone is 1.
        Binary = ("binary", "Binary, first tone is 1"),
        /// C4FM dibits.
        C4fm = ("c4fm", "C4FM dibits"),
    }
}

impl BitLabels {
    /// The label map of `sdrkit-algo-modem`.
    fn label_map(self) -> LabelMap {
        match self {
            BitLabels::Natural => LabelMap::Natural,
            BitLabels::Gray => LabelMap::Gray,
            BitLabels::Binary => LabelMap::Binary,
            BitLabels::C4fm => LabelMap::C4fm,
        }
    }
}

/// The working demodulator, built in `configure`.
#[derive(Debug)]
enum Engine {
    Idle,
    Tones {
        demod: Box<MfskDemod>,
        llr: Box<TrackedFskLlr>,
    },
    Levels {
        levels: Box<DiscriminatorLevels>,
        slicer: Box<LevelSlicer>,
    },
}

const DETECTOR: Setting<field::Choice<FskDetector>> = Setting::choice("detector", "Detector", FskDetector::Correlator).help("Tone correlators (any number of tones, on IQ), or a discriminator and level slicer (2- or 4-level FSK).");
const TONES_HZ: Setting<field::Text> = Setting::new("tones_hz", "Tones", "", "").help("The tone frequencies in Hz, separated by commas, first tone first (for the correlator). Empty: levels tones shift_hz apart around 0 Hz.");
const SHIFT_HZ: Setting<field::Hz> = Setting::new("shift_hz", "Tone spacing", "Hz", 1000.0)
    .range(1.0, 1e7, 0.0)
    .help("Spacing of adjacent tones when tones_hz is empty.");
const DEVIATION_HZ: Setting<field::Hz> = Setting::new("deviation_hz", "Deviation", "Hz", 500.0)
    .range(1.0, 1e7, 0.0)
    .help("Peak deviation of the outermost level (discriminator on IQ).");
const LEVELS: Setting<field::I64> = Setting::new("levels", "Tones or levels", "", 2)
    .range(2.0, 256.0, 1.0)
    .help("Number of tones or levels: a power of two.");
const BAUD: Setting<field::Hz> =
    Setting::new("baud", "Symbol rate", "Hz", 1200.0).range(0.1, 1e7, 0.0);
const MAP: Setting<field::Choice<BitLabels>> =
    Setting::choice("map", "Bit labels", BitLabels::Natural)
        .help("The bits each tone (in list order) or level (from the lowest) carries.");
const CLOCK_BANDWIDTH: Setting<field::F64> =
    Setting::new("clock_bandwidth", "Clock loop bandwidth", "", 0.01)
        .range(1e-4, 0.2, 0.0)
        .help("Noise bandwidth of the symbol clock loop per symbol.");
const MAX_PPM: Setting<field::F64> =
    Setting::new("max_ppm", "Clock range", "ppm", 2000.0).range(0.0, 50_000.0, 0.0);
const LLR_SCALE: Setting<field::F64> = Setting::new("llr_scale", "Soft bit scale", "", 4.0)
    .range(0.01, 127.0, 0.0)
    .help("i8 units per natural-log unit of the soft bits.");

sdrkit_core::settings! {
    /// The settings of [`FskDemod`].
    pub enum FskDemodSetting {
        /// Detector.
        Detector = DETECTOR,
        /// Tones.
        TonesHz = TONES_HZ,
        /// Tone spacing.
        ShiftHz = SHIFT_HZ,
        /// Deviation.
        DeviationHz = DEVIATION_HZ,
        /// Tones or levels.
        Levels = LEVELS,
        /// Symbol rate.
        Baud = BAUD,
        /// Bit labels.
        Map = MAP,
        /// Clock loop bandwidth.
        ClockBandwidth = CLOCK_BANDWIDTH,
        /// Clock range.
        MaxPpm = MAX_PPM,
        /// Soft bit scale.
        LlrScale = LLR_SCALE,
    }
}

/// The settings of `modem.fsk_demod_real`: those of [`FskDemodSetting`], with `detector` fixed to the discriminator.
fn real_schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    Arc::clone(SCHEMA.get_or_init(|| {
        let mut settings = FskDemodSetting::schema().settings().to_vec();
        let detector = &mut settings[FskDemodSetting::Detector.id().index()];
        detector.default = Value::Choice(FskDetector::Discriminator.index());
        detector.fixed = true;
        Arc::new(Schema::new(settings).expect("the settings of FskDemod make a schema"))
    }))
}

/// `modem.fsk_demod` (on `Iq`) and `modem.fsk_demod_real` (on `Real`, discriminator only): M-FSK to soft bits.
///
/// - **correlator** (`sdrkit_algo_modem::fsk::MfskDemod`, `Iq` only): one sliding DFT per tone over a symbol, with the symbol clock at
///   the instant of clearest decision. If a `Real` source feeds it, the graph inserts its analytic adapter. The tones are `tones_hz`
///   (any number that is a power of two) or `levels` tones `shift_hz` apart centred on 0 Hz. Output `energies` carries the tone energies of every
///   symbol, and the LLRs are the exact non-coherent ones (`fsk::FskLlr`), with the amplitude and noise estimated blind and
///   smoothed over about 16 symbols.
/// - **discriminator**: the FM discriminator (`Iq`, scaled by `deviation_hz`, the outermost level's deviation) or the input itself
///   (`Real`: a frequency already, from an FM demodulator), a one-symbol integrate-and-dump, the zero-crossing symbol synchroniser
///   and the `levels`-level slicer (`fsk::LevelSlicer`, which follows the centre and spacing). Output `levels` carries the
///   symbol-rate levels (unit mean power).
///
/// `map` labels the tones (in list order) or the levels (from the lowest frequency): `natural`, `gray`, `binary` (2: the lower is
/// bit 1, as `LevelSlicer::binary`) or `c4fm` (4: the P25/DMR dibits of `LevelSlicer::c4fm`). Bits leave most significant first.
/// Publishes `clock_ppm` and `snr_db` (per tone or per level) four times a second of signal.
#[derive(Debug)]
pub struct FskDemod {
    on_real: bool,
    detector: FskDetector,
    tones_hz: String,
    shift_hz: f64,
    deviation_hz: f64,
    levels: u32,
    baud: f64,
    map: BitLabels,
    clock_bandwidth: f64,
    max_ppm: f64,
    llr_scale: f64,
    engine: Engine,
    rate: f64,
    symbols: u64,
    bits: usize,
    raw: Vec<f32>,
    soft: Vec<f32>,
    hard: Vec<i8>,
    pool: SlicePool<f32>,
    results: Published,
    every: Every,
    snr: f32,
}

impl Default for FskDemod {
    fn default() -> Self {
        FskDemod {
            on_real: false,
            detector: FskDetector::Correlator,
            tones_hz: String::new(),
            shift_hz: SHIFT_HZ.default,
            deviation_hz: DEVIATION_HZ.default,
            levels: LEVELS.default as _,
            baud: BAUD.default,
            map: BitLabels::Natural,
            clock_bandwidth: CLOCK_BANDWIDTH.default,
            max_ppm: MAX_PPM.default,
            llr_scale: LLR_SCALE.default,
            engine: Engine::Idle,
            rate: 0.0,
            symbols: 0,
            bits: 1,
            raw: Vec::new(),
            soft: Vec::new(),
            hard: Vec::new(),
            pool: SlicePool::default(),
            results: Published::new(&[&keys::CLOCK_PPM, &keys::SNR_DB]),
            every: Every::default(),
            snr: 0.0,
        }
    }
}

impl FskDemod {
    /// The tone frequencies in Hz for an input of `kind`.
    fn tone_list(&self) -> Result<Vec<f64>, ConfigError> {
        let m = self.levels as usize;
        let tones = FskDemodSetting::TonesHz.id();
        let t = parse_list(&self.tones_hz).ok_or(SettingError::Malformed {
            id: tones,
            at: None,
        })?;
        if !t.is_empty() {
            if t.len() != m {
                return Err(SettingError::Conflict {
                    id: tones,
                    with: FskDemodSetting::Levels.id(),
                }
                .into());
            }
            return Ok(t);
        }
        Ok((0..m)
            .map(|k| (k as f64 - (m as f64 - 1.0) / 2.0) * self.shift_hz)
            .collect())
    }

    fn build(&self, rate: f64, real: bool) -> Result<Engine, ConfigError> {
        let m = self.levels as usize;
        let sps = rate / self.baud;
        let labels = self.map.label_map().labels(m)?;
        match self.detector {
            FskDetector::Correlator => {
                let tones = self.tone_list()?;
                if tones.iter().any(|f| f.abs() >= rate / 2.0) {
                    return Err(ConfigError::SettingAtRate {
                        id: FskDemodSetting::TonesHz.id(),
                        rate_hz: rate,
                    });
                }
                let mut cfg = MfskConfig::from_hz(&tones, self.baud, rate);
                cfg.bn_t = self.clock_bandwidth;
                cfg.max_ppm = self.max_ppm;
                let demod = MfskDemod::new(cfg).map_err(ConfigError::from)?;
                let llr = FskLlr::new(&labels).map_err(ConfigError::from)?;
                Ok(Engine::Tones {
                    demod: Box::new(demod),
                    llr: Box::new(TrackedFskLlr::new(llr, 16.0).map_err(ConfigError::from)?),
                })
            }
            FskDetector::Discriminator => {
                let dev = if real {
                    None
                } else {
                    Some(self.deviation_hz / rate)
                };
                let mut cfg = DiscriminatorConfig::new(sps, dev);
                cfg.bn_t = self.clock_bandwidth;
                cfg.max_ppm = self.max_ppm;
                let levels = DiscriminatorLevels::new(cfg).map_err(ConfigError::from)?;
                // The synchroniser produces unit mean power: levels +-1, +-3 ... scaled by 1 / rms.
                let rms = (((m * m - 1) as f64) / 3.0).sqrt();
                let slicer = LevelSlicer::new(&labels, 0.01, (1.0 / rms) as f32)
                    .map_err(ConfigError::from)?;
                Ok(Engine::Levels {
                    levels: Box::new(levels),
                    slicer: Box::new(slicer),
                })
            }
        }
    }

    fn reset(&mut self) {
        match &mut self.engine {
            Engine::Idle => {}
            Engine::Tones { demod, llr } => {
                demod.reset();
                llr.reset();
            }
            Engine::Levels { levels, .. } => levels.reset(),
        }
    }

    /// Demodulates input items `a..b` into the outputs.
    fn run(&mut self, io: &mut Io, a: usize, b: usize) {
        let real = self.on_real;
        self.raw.clear();
        match &mut self.engine {
            Engine::Idle => return,
            Engine::Tones { demod, .. } => demod.process(&io.iq(0)[a..b], &mut self.raw),
            Engine::Levels { levels, .. } => {
                if real {
                    levels.process_real(&io.real(0)[a..b], &mut self.raw);
                } else {
                    levels.process_iq(&io.iq(0)[a..b], &mut self.raw);
                }
            }
        }
        let bits = self.bits;
        let q = Quantizer::default();
        let scale = self.llr_scale as f32;
        let baud = self.baud;
        match &mut self.engine {
            Engine::Idle => {}
            Engine::Tones { llr, .. } => {
                let m = llr.llr().tones();
                for e in self.raw.chunks_exact(m) {
                    // A symbol that is not a number has its bits erased (LLRs 0).
                    let _ = llr.symbol(e, &mut self.soft[..bits]);
                    self.snr = llr.snr();
                    let time = self.symbols as f64 / baud;
                    let item = FskEnergies {
                        symbol: self.symbols,
                        energies: self.pool.exact(e.len(), |s| s.copy_from_slice(e)),
                    };
                    io.items_out::<FskEnergies>(1)
                        .push(Timed { time, value: item });
                    self.symbols += 1;
                    // Both slices hold `bits` values and the scale is positive, so this cannot fail.
                    let _ = q.quantize(&self.soft[..bits], scale, &mut self.hard[..bits]);
                    io.llrs_out(0).extend_from_slice(&self.hard[..bits]);
                }
            }
            Engine::Levels { slicer, .. } => {
                for &v in &self.raw {
                    self.soft.clear();
                    self.soft.resize(bits, 0.0);
                    if slicer.slice(v, &mut self.soft).is_err() {
                        self.soft.fill(0.0);
                    }
                    let sp = slicer.spacing();
                    self.snr = sp * sp / slicer.noise_var().max(f32::MIN_POSITIVE);
                    self.symbols += 1;
                    // Both slices hold `bits` values and the scale is positive, so this cannot fail.
                    let _ = q.quantize(&self.soft, scale, &mut self.hard[..bits]);
                    io.llrs_out(0).extend_from_slice(&self.hard[..bits]);
                }
                io.real_out(2).extend_from_slice(&self.raw);
            }
        }
    }

    fn publish(&mut self) {
        let ppm = match &self.engine {
            Engine::Idle => return,
            Engine::Tones { demod, .. } => demod.clock_ppm(),
            Engine::Levels { levels, .. } => levels.clock_ppm(),
        };
        self.results.set(&keys::CLOCK_PPM, ppm);
        if self.snr > 0.0 && self.snr.is_finite() {
            self.results
                .set(&keys::SNR_DB, 10.0 * (self.snr as f64).log10());
        } else {
            self.results.none(&keys::SNR_DB);
        }
    }
}

impl FskDemod {
    /// `modem.fsk_demod_real`: the discriminator demodulator on `Real` input that is a frequency already (the output of an FM
    /// demodulator). It refuses the correlator.
    pub fn on_real() -> Self {
        FskDemod {
            on_real: true,
            detector: FskDetector::Discriminator,
            ..Self::default()
        }
    }
}

impl Component for FskDemod {
    fn info(&self) -> ComponentInfo {
        let (name, kind) = if self.on_real {
            ("modem.fsk_demod_real", Kind::Real)
        } else {
            ("modem.fsk_demod", Kind::Iq)
        };
        ComponentInfo::processor(
            name,
            vec![Port::new("in", kind)],
            vec![
                Port::llr("out"),
                Port::typed::<FskEnergies>("energies").rate_from(RateFrom::Fixed(0.0)),
                Port::real("levels").rate_from(RateFrom::Setting(FskDemodSetting::Baud.id())),
            ],
        )
        .with_settings(if self.on_real {
            real_schema()
        } else {
            FskDemodSetting::schema()
        })
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let real = self.on_real;
        let m = self.levels as usize;
        if !(f.rate > 0.0 && f.rate / self.baud >= 2.0) {
            return Err(ConfigError::SettingAtRate {
                id: FskDemodSetting::Baud.id(),
                rate_hz: f.rate,
            });
        }
        self.engine = self.build(f.rate, real)?;
        self.rate = f.rate;
        self.bits = m.trailing_zeros() as usize;
        self.symbols = 0;
        self.snr = 0.0;
        self.soft = vec![0.0; self.bits];
        self.hard = vec![0; self.bits];
        self.raw =
            Vec::with_capacity(4 * m * (PIECE as f64 / (f.rate / self.baud)).ceil() as usize + 64);
        self.pool = SlicePool::with_ready(m, 64);
        self.every = Every::new(f.rate, 0.25);
        Ok(vec![
            Format::llr(self.baud * self.bits as f64, Some(self.llr_scale as f32)),
            Format::new(Kind::typed::<FskEnergies>(), 0.0),
            Format {
                bandwidth_hz: self.baud,
                ..f.with_kind(Kind::Real).with_rate(self.baud)
            },
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(0);
        for (a, b, reset) in pieces(io.input(0), n) {
            if reset {
                self.reset();
            }
            if b > a {
                self.run(io, a, b);
            }
        }
        if self.every.tick(n) {
            self.publish();
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FskDemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FskDemodSetting::Detector => {
                let detector = DETECTOR.read(id, v)?;
                if self.on_real && detector == FskDetector::Correlator {
                    return Err(SettingError::NotPermitted { id });
                }
                self.detector = detector;
            }
            FskDemodSetting::TonesHz => {
                let t = TONES_HZ.read(id, v)?;
                if parse_list(t).is_none() {
                    return Err(SettingError::Malformed { id, at: None });
                }
                self.tones_hz = t.to_owned();
            }
            FskDemodSetting::ShiftHz => self.shift_hz = SHIFT_HZ.read(id, v)?,
            FskDemodSetting::DeviationHz => self.deviation_hz = DEVIATION_HZ.read(id, v)?,
            FskDemodSetting::Levels => {
                let m = LEVELS.read(id, v)?;
                if !(m as u64).is_power_of_two() {
                    return Err(SettingError::NotPermitted { id });
                }
                self.levels = m as u32;
            }
            FskDemodSetting::Baud => self.baud = BAUD.read(id, v)?,
            FskDemodSetting::Map => self.map = MAP.read(id, v)?,
            FskDemodSetting::ClockBandwidth => {
                self.clock_bandwidth = CLOCK_BANDWIDTH.read(id, v)?
            }
            FskDemodSetting::MaxPpm => self.max_ppm = MAX_PPM.read(id, v)?,
            FskDemodSetting::LlrScale => self.llr_scale = LLR_SCALE.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        let present = [
            (
                FskDemodSetting::Detector,
                Value::Choice(self.detector.index()),
            ),
            (FskDemodSetting::TonesHz, Value::Text(self.tones_hz.clone())),
            (FskDemodSetting::ShiftHz, Value::Hz(self.shift_hz)),
            (FskDemodSetting::DeviationHz, Value::Hz(self.deviation_hz)),
            (FskDemodSetting::Levels, Value::I64(self.levels as i64)),
            (FskDemodSetting::Baud, Value::Hz(self.baud)),
            (FskDemodSetting::Map, Value::Choice(self.map.index())),
            (
                FskDemodSetting::ClockBandwidth,
                Value::F64(self.clock_bandwidth),
            ),
            (FskDemodSetting::MaxPpm, Value::F64(self.max_ppm)),
            (FskDemodSetting::LlrScale, Value::F64(self.llr_scale)),
        ];
        if !self.on_real {
            return values_of(present);
        }
        let mut v = Values::defaults(&real_schema());
        for (s, value) in present
            .iter()
            .filter(|(s, _)| *s != FskDemodSetting::Detector)
        {
            v.set(s.id(), value)
                .expect("the component holds only values its schema takes");
        }
        v
    }

    fn results(&self) -> Option<&sdrkit_core::Results> {
        Some(self.results.results())
    }
}
