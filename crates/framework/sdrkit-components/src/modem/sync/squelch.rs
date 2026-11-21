//! `modem.squelch`: power, SNR and FM noise-quieting squelch.

use super::{is_retune, split_at_tags};
use sdrkit_algo_dsp::floor::{FloorTracker, MinFollower};
use sdrkit_algo_modem::squelch::{Gate, PowerSquelch, QuietingSquelch, SnrSquelch};
use sdrkit_core::publish::{field, keys, Field, Published};
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{Complex32, ConfigError, Results, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{
    Applied, Component, ComponentInfo, Every, Format, Io, Kind, Port, TagCarry, TagKind,
};

sdrkit_core::choice_enum! {
    /// What [`SquelchComponent`] measures.
    pub enum SquelchKind {
        /// Power.
        Power = ("power", "Power"),
        /// Power over the noise floor.
        Snr = ("snr", "Power over the noise floor"),
        /// FM noise quieting.
        Quieting = ("quieting", "FM noise quieting"),
    }
}

/// Whether the squelch is open.
const OPEN: Field<field::Bool> = Field::new("open", "Open", "");
/// The FM quieting ratio: voice-band over noise-band power of the discriminator output, dB.
const QUIETING_DB: Field<field::F64> = Field::new("quieting_db", "Quieting", "dB");

enum Kernel {
    Power(PowerSquelch),
    Snr(SnrSquelch),
    Quieting(QuietingSquelch),
}

impl Kernel {
    #[inline]
    fn push(&mut self, z: Complex32, re: f32) -> bool {
        match self {
            Kernel::Power(s) => s.process(std::slice::from_ref(&z)),
            Kernel::Snr(s) => s.process(std::slice::from_ref(&z)),
            Kernel::Quieting(s) => s.process(std::slice::from_ref(&re)),
        }
    }
    fn is_open(&self) -> bool {
        match self {
            Kernel::Power(s) => s.is_open(),
            Kernel::Snr(s) => s.is_open(),
            Kernel::Quieting(s) => s.is_open(),
        }
    }
}

const MODE: Setting<field::Choice<SquelchKind>> =
    Setting::choice("mode", "Squelch", SquelchKind::Power);
const THRESHOLD_DB: Setting<field::F64> =
    Setting::new("threshold_db", "Threshold", "dB", -50.0).range(-200.0, 100.0, 0.0);
const HYSTERESIS_DB: Setting<field::F64> =
    Setting::new("hysteresis_db", "Hysteresis", "dB", 3.0).range(0.0, 60.0, 0.0);
const BLOCK: Setting<field::I64> =
    Setting::new("block", "Block", "samples", 256).range(1.0, 1e6, 1.0);
const ATTACK: Setting<field::I64> =
    Setting::new("attack", "Blocks to open", "", 1).range(1.0, 1000.0, 1.0);
const HANG: Setting<field::I64> =
    Setting::new("hang", "Blocks to close", "", 4).range(0.0, 100_000.0, 1.0);
const NOISE_HZ: Setting<field::Hz> =
    Setting::new("noise_hz", "Noise band from", "Hz", 6000.0).range(3000.0, 1e6, 0.0);

sdrkit_core::settings! {
    /// The settings of [`SquelchComponent`].
    pub enum SquelchComponentSetting {
        /// Squelch.
        Mode = MODE,
        /// Threshold.
        ThresholdDb = THRESHOLD_DB,
        /// Hysteresis.
        HysteresisDb = HYSTERESIS_DB,
        /// Block.
        Block = BLOCK,
        /// Blocks to open.
        Attack = ATTACK,
        /// Blocks to close.
        Hang = HANG,
        /// Noise band from.
        NoiseHz = NOISE_HZ,
    }
}

/// The squelch: `modem.squelch` on `Iq`, `modem.squelch_real` on `Real`. It passes the signal while open and zeros while closed, deciding on blocks of
/// `block` samples through a `sdrkit_algo_modem::squelch::Gate` (open after `attack` blocks at or above `threshold_db`, closed after
/// `hang` blocks below `threshold_db - hysteresis_db`). A block of digital silence never opens it.
///
/// - `power`: the block's mean power, dBFS (`PowerSquelch`; a real sample `x` counts as power `x^2`).
/// - `snr`: the block's power over a noise floor followed while closed, dB (`SnrSquelch` with a minimum follower).
/// - `quieting` (`Real` only, a discriminator's output at 8 kHz or more): voice-band over noise-band power, dB, the noise band
///   from `noise_hz` (`QuietingSquelch`).
///
/// Tags: `Lock { locked: true }` where it opens and `Lock { locked: false }` where it closes; a `Retuned` tag closes it and starts
/// again (the noise floor is measured anew). Results: `open`, and `level_dbfs`, `snr_db` or `quieting_db` by mode. Settings rebuild it.
pub struct SquelchComponent {
    on_real: bool,
    mode: SquelchKind,
    threshold_db: f64,
    hysteresis_db: f64,
    block: usize,
    attack: u32,
    hang: u32,
    noise_hz: f64,
    rate: f64,
    kernel: Option<Kernel>,
    carry: TagCarry,
    every: Every,
    results: Published,
}

impl Default for SquelchComponent {
    /// Power measurement at -50 dBFS, 3 dB hysteresis, blocks of 256, opens after 1 block, closes after 4.
    fn default() -> Self {
        SquelchComponent {
            on_real: false,
            mode: SquelchKind::Power,
            threshold_db: THRESHOLD_DB.default,
            hysteresis_db: HYSTERESIS_DB.default,
            block: BLOCK.default as _,
            attack: ATTACK.default as _,
            hang: HANG.default as _,
            noise_hz: NOISE_HZ.default,
            rate: 0.0,
            kernel: None,
            carry: TagCarry::same_rate(),
            every: Every::default(),
            results: Published::new(&[&OPEN, &keys::LEVEL_DBFS, &keys::SNR_DB, &QUIETING_DB]),
        }
    }
}

impl SquelchComponent {
    /// A squelch that measures `mode` and opens at `threshold_db`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `threshold_db` setting refuses `threshold_db`.
    pub fn new(mode: SquelchKind, threshold_db: f64) -> Result<Self, SettingError> {
        SquelchComponentSetting::ThresholdDb.check(&Value::F64(threshold_db))?;
        Ok(SquelchComponent {
            mode,
            threshold_db,
            ..Self::default()
        })
    }
    /// `modem.squelch_real`: the squelch on `Real` input, at its defaults.
    pub fn on_real() -> Self {
        SquelchComponent {
            on_real: true,
            ..Self::default()
        }
    }
    /// The same squelch on `Real` input.
    pub fn real(self) -> Self {
        SquelchComponent {
            on_real: true,
            ..self
        }
    }
    /// Whether it is open now.
    pub fn is_open(&self) -> bool {
        self.kernel.as_ref().is_some_and(|k| k.is_open())
    }
    fn build(&self) -> Result<Kernel, ConfigError> {
        let open = self.threshold_db as f32;
        let gate = Gate::new(
            open,
            open - self.hysteresis_db as f32,
            self.attack,
            self.hang,
        )?;
        Ok(match self.mode {
            SquelchKind::Power => Kernel::Power(PowerSquelch::new(self.block, gate)?),
            SquelchKind::Snr => {
                let blocks_per_s = self.rate / self.block as f64;
                let f =
                    MinFollower::with_times(blocks_per_s, 0.2, 1.0).map_err(ConfigError::from)?;
                Kernel::Snr(SnrSquelch::new(
                    self.block,
                    FloorTracker::MinFollower(f),
                    gate,
                )?)
            }
            SquelchKind::Quieting => {
                if !self.on_real {
                    return Err(sdrkit_graph::GraphError::input_kind(0, Kind::Iq));
                }
                Kernel::Quieting(QuietingSquelch::new(
                    self.rate,
                    self.block,
                    self.noise_hz,
                    gate,
                )?)
            }
        })
    }
    fn publish(&mut self) {
        let open = self.is_open();
        self.results.set(&OPEN, open);
        match &self.kernel {
            Some(Kernel::Power(s)) => {
                if let Some(v) = s.level_db() {
                    self.results.set(&keys::LEVEL_DBFS, super::r2(v as f64));
                }
            }
            Some(Kernel::Snr(s)) => {
                if let Some(v) = s.snr_db() {
                    self.results.set(&keys::SNR_DB, super::r2(v as f64));
                }
            }
            Some(Kernel::Quieting(s)) => {
                if let Some(v) = s.ratio_db() {
                    self.results.set(&QUIETING_DB, super::r2(v as f64));
                }
            }
            None => {}
        }
    }
}

impl Component for SquelchComponent {
    fn info(&self) -> ComponentInfo {
        let (name, kind) = if self.on_real {
            ("modem.squelch_real", Kind::Real)
        } else {
            ("modem.squelch", Kind::Iq)
        };
        ComponentInfo::processor(
            name,
            vec![Port::new("in", kind)],
            vec![Port::new("out", kind).same_format_as(0)],
        )
        .in_place(0, 0)
        .manual_tags()
        .with_settings(SquelchComponentSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        super::need_rate(f.rate)?;
        self.rate = f.rate;
        self.kernel = Some(self.build()?);
        self.carry = TagCarry::same_rate();
        self.every = Every::new(f.rate, 0.1);
        self.publish();
        Ok(vec![f.clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let input = io.input(0);
        if self.kernel.is_none() {
            return;
        }
        let o0 = io.output(0).first();
        let mut changes = 0usize;
        let total;
        if self.on_real {
            let buf = io.pass_through::<f32>(0, 0);
            let n = buf.len();
            total = n;
            split_at_tags(
                &mut (&mut *self, buf, &mut changes),
                input,
                n,
                |(me, buf, ch), a, b| {
                    for (i, v) in buf[a..b].iter_mut().enumerate() {
                        let k = me.kernel.as_mut().expect("built");
                        let was = k.is_open();
                        let open = k.push(Complex32::new(*v, 0.0), *v);
                        if open != was {
                            me.carry
                                .queue(o0 + (a + i) as u64, TagKind::Lock { locked: open });
                            **ch += 1;
                        }
                        if !open {
                            *v = 0.0;
                        }
                    }
                },
                |(me, _, ch), t| {
                    if is_retune(t) {
                        if me.is_open() {
                            me.carry
                                .queue(t.at - input.first() + o0, TagKind::Lock { locked: false });
                            **ch += 1;
                        }
                        if let Ok(k) = me.build() {
                            me.kernel = Some(k);
                        }
                    }
                },
            );
        } else {
            let buf = io.pass_through::<Complex32>(0, 0);
            let n = buf.len();
            total = n;
            split_at_tags(
                &mut (&mut *self, buf, &mut changes),
                input,
                n,
                |(me, buf, ch), a, b| {
                    for (i, z) in buf[a..b].iter_mut().enumerate() {
                        let k = me.kernel.as_mut().expect("built");
                        let was = k.is_open();
                        let open = k.push(*z, z.re);
                        if open != was {
                            me.carry
                                .queue(o0 + (a + i) as u64, TagKind::Lock { locked: open });
                            **ch += 1;
                        }
                        if !open {
                            *z = Complex32::default();
                        }
                    }
                },
                |(me, _, ch), t| {
                    if is_retune(t) {
                        if me.is_open() {
                            me.carry
                                .queue(t.at - input.first() + o0, TagKind::Lock { locked: false });
                            **ch += 1;
                        }
                        if let Ok(k) = me.build() {
                            me.kernel = Some(k);
                        }
                    }
                },
            );
        }
        self.carry.forward(input, io.output(0));
        if self.every.tick(total) || changes > 0 {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SquelchComponentSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SquelchComponentSetting::Mode => self.mode = MODE.read(id, v)?,
            SquelchComponentSetting::ThresholdDb => self.threshold_db = THRESHOLD_DB.read(id, v)?,
            SquelchComponentSetting::HysteresisDb => {
                self.hysteresis_db = HYSTERESIS_DB.read(id, v)?
            }
            SquelchComponentSetting::Block => self.block = BLOCK.read(id, v)? as usize,
            SquelchComponentSetting::Attack => self.attack = ATTACK.read(id, v)? as u32,
            SquelchComponentSetting::Hang => self.hang = HANG.read(id, v)? as u32,
            SquelchComponentSetting::NoiseHz => self.noise_hz = NOISE_HZ.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([
            (
                SquelchComponentSetting::Mode,
                Value::Choice(self.mode.index()),
            ),
            (
                SquelchComponentSetting::ThresholdDb,
                Value::F64(self.threshold_db),
            ),
            (
                SquelchComponentSetting::HysteresisDb,
                Value::F64(self.hysteresis_db),
            ),
            (
                SquelchComponentSetting::Block,
                Value::I64(self.block as i64),
            ),
            (
                SquelchComponentSetting::Attack,
                Value::I64(self.attack as i64),
            ),
            (SquelchComponentSetting::Hang, Value::I64(self.hang as i64)),
            (SquelchComponentSetting::NoiseHz, Value::Hz(self.noise_hz)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}
