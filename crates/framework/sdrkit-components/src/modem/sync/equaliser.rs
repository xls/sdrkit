//! `modem.equaliser`: the adaptive equaliser.

use super::points::{ConstellationName, CONSTELLATION};
use super::{is_retune, points, split_at_tags};
use sdrkit_algo_modem::equalizer::{AdaptiveEq, EqConfig, Mode};
use sdrkit_core::publish::{field, Field, Published};
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{Complex32, ConfigError, Results, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Every, Format, Io, Port, TagCarry};

sdrkit_core::choice_enum! {
    /// How [`Equaliser`] updates its taps.
    pub enum EqualiserUpdate {
        /// Constant modulus, then decision directed.
        Cma = ("cma", "Constant modulus, then decision directed"),
        /// Decision-directed LMS.
        DdLms = ("dd_lms", "Decision-directed LMS"),
        /// Decision-directed normalised LMS.
        Nlms = ("nlms", "Decision-directed normalised LMS"),
    }
}

/// The smoothed decision error of the equaliser, dB.
const MSE_DB: Field<field::F64> = Field::new("mse_db", "Decision error", "dB");
/// Times the equaliser diverged and reset itself.
const RESETS: Field<field::I64> = Field::new("resets", "Resets", "");

const TAPS: Setting<field::I64> = Setting::new("taps", "Taps", "", 11).range(1.0, 1024.0, 1.0);
const SPS: Setting<field::I64> =
    Setting::new("sps", "Samples per symbol", "", 1).range(1.0, 2.0, 1.0);
const UPDATE: Setting<field::Choice<EqualiserUpdate>> =
    Setting::choice("update", "Adaptation", EqualiserUpdate::Cma);
const FEEDBACK: Setting<field::I64> =
    Setting::new("feedback", "Feedback taps", "", 0).range(0.0, 256.0, 1.0);
const MU: Setting<field::F64> = Setting::new("mu", "Step size", "", 1e-3).range(1e-7, 1.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`Equaliser`].
    pub enum EqualiserSetting {
        /// Taps.
        Taps = TAPS,
        /// Samples per symbol.
        Sps = SPS,
        /// Adaptation.
        Update = UPDATE,
        /// Feedback taps.
        Feedback = FEEDBACK,
        /// Step size.
        Mu = MU,
        /// Constellation.
        Constellation = CONSTELLATION,
    }
}

/// The adaptive equaliser (`modem.equaliser`): `Symbols` at `sps` samples per symbol (1, or 2 for a fractionally spaced
/// equaliser fed by `modem.symbol_sync` with two outputs per symbol) in, one equalised `Symbols` per symbol out.
///
/// Wraps `sdrkit_algo_modem::equalizer::AdaptiveEq` with `taps` feed-forward taps (in input samples, the centre one starting at 1) and
/// `feedback` decision-feedback taps (0 for a linear equaliser), adapting with step `mu` by `update`:
///
/// - `cma`: blind constant modulus with the radius of `constellation`, switching to decision directed once the decision error is
///   small (and back when it grows);
/// - `dd_lms`: decision directed (on `constellation`) with LMS;
/// - `nlms`: decision directed with normalised LMS (`mu` in `(0, 1]` divided by the power in the taps).
///
/// The output lags the input by about half the taps. A `Retuned` tag resets the taps. Results: `mse_db` (the smoothed decision
/// error), `resets` (divergences). Settings rebuild the equaliser.
pub struct Equaliser {
    taps: usize,
    sps: usize,
    update: EqualiserUpdate,
    feedback: usize,
    mu: f64,
    constellation: ConstellationName,
    eq: Option<AdaptiveEq>,
    carry: TagCarry,
    every: Every,
    results: Published,
}

impl Default for Equaliser {
    /// 11 taps, symbol spaced, CMA handing over to DD, no feedback, `mu` 0.001, QPSK.
    fn default() -> Self {
        Equaliser {
            taps: TAPS.default as _,
            sps: SPS.default as _,
            update: EqualiserUpdate::Cma,
            feedback: FEEDBACK.default as _,
            mu: MU.default,
            constellation: ConstellationName::Qpsk,
            eq: None,
            carry: TagCarry::default(),
            every: Every::default(),
            results: Published::new(&[&MSE_DB, &RESETS]),
        }
    }
}

impl Equaliser {
    fn publish(&mut self) {
        let (mse, resets) = self
            .eq
            .as_ref()
            .map_or((0.0, 0), |e| (e.mse() as f64, e.resets() as i64));
        if mse > 0.0 && mse.is_finite() {
            self.results.set(&MSE_DB, super::r2(10.0 * mse.log10()));
        }
        self.results.set(&RESETS, resets);
    }
}

impl Component for Equaliser {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "modem.equaliser",
            vec![Port::symbols("in")],
            vec![Port::symbols("out")],
        )
        .manual_tags()
        .with_settings(EqualiserSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        super::need_rate(f.rate)?;
        let mut cfg = EqConfig::new(self.taps, self.sps);
        cfg.feedback = self.feedback;
        cfg.mu = self.mu as f32;
        match self.update {
            EqualiserUpdate::Cma => {}
            EqualiserUpdate::DdLms => {
                cfg.mode = Mode::Dd;
                cfg.dd_switch_mse = None;
            }
            EqualiserUpdate::Nlms => {
                cfg.mode = Mode::Dd;
                cfg.dd_switch_mse = None;
                cfg.normalised = true;
            }
        }
        let c = points::constellation(self.constellation)?;
        self.eq = Some(AdaptiveEq::new(cfg, c).map_err(ConfigError::from)?);
        let out_rate = f.rate / self.sps as f64;
        self.carry = TagCarry::new(f.rate, out_rate);
        self.every = Every::new(out_rate, 0.1);
        self.publish();
        Ok(vec![f.with_rate(out_rate)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.samples::<Complex32>(0);
        let input = io.input(0);
        if self.eq.is_none() {
            return;
        }
        let out = io.samples_out::<Complex32>(0);
        let start = out.len();
        let mut st = (&mut self.eq, out);
        split_at_tags(
            &mut st,
            input,
            x.len(),
            |(eq, out), a, b| {
                if let Some(e) = eq.as_mut() {
                    e.process(&x[a..b], out);
                }
            },
            |(eq, _), t| {
                if is_retune(t) {
                    if let Some(e) = eq.as_mut() {
                        e.reset();
                    }
                }
            },
        );
        let n = st.1.len() - start;
        self.carry.forward(input, io.output(0));
        if self.every.tick(n) {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match EqualiserSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            EqualiserSetting::Taps => self.taps = TAPS.read(id, v)? as usize,
            EqualiserSetting::Sps => self.sps = SPS.read(id, v)? as usize,
            EqualiserSetting::Update => self.update = UPDATE.read(id, v)?,
            EqualiserSetting::Feedback => self.feedback = FEEDBACK.read(id, v)? as usize,
            EqualiserSetting::Mu => self.mu = MU.read(id, v)?,
            EqualiserSetting::Constellation => self.constellation = CONSTELLATION.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([
            (EqualiserSetting::Taps, Value::I64(self.taps as i64)),
            (EqualiserSetting::Sps, Value::I64(self.sps as i64)),
            (EqualiserSetting::Update, Value::Choice(self.update.index())),
            (EqualiserSetting::Feedback, Value::I64(self.feedback as i64)),
            (EqualiserSetting::Mu, Value::F64(self.mu)),
            (
                EqualiserSetting::Constellation,
                Value::Choice(self.constellation.index()),
            ),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}
