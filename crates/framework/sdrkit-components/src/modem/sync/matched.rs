//! `modem.matched_filter`: the receive filter matched to a pulse, with optional decimation.

use super::{shape, PulseShape};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::pulse::{taps, Norm};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Port, RateFrom, TagCarry,
};

const PULSE: Setting<field::Choice<PulseShape>> =
    Setting::choice("pulse", "Pulse", PulseShape::Rrc);
const BETA: Setting<field::F64> = Setting::new("beta", "Roll-off", "", 0.35).range(0.0, 1.0, 0.0);
const BT: Setting<field::F64> = Setting::new("bt", "Gaussian BT", "", 0.5).range(0.05, 10.0, 0.0);
const SPAN: Setting<field::F64> =
    Setting::new("span", "Span", "symbols", 8.0).range(1.0, 64.0, 0.0);
const SPS: Setting<field::F64> =
    Setting::new("sps", "Samples per symbol", "", 4.0).range(1.0, 1024.0, 0.0);
const DECIMATE: Setting<field::I64> =
    Setting::new("decimate", "Decimation", "", 1).range(1.0, 1024.0, 1.0);

sdrkit_core::settings! {
    /// The settings of [`MatchedFilter`].
    pub enum MatchedFilterSetting {
        /// The pulse shape.
        Pulse = PULSE,
        /// The roll-off of `rrc` and `rc`.
        Beta = BETA,
        /// The bandwidth-time product of `gaussian`.
        Bt = BT,
        /// The length in symbols.
        Span = SPAN,
        /// Input samples per symbol.
        Sps = SPS,
        /// Keep one output in this many.
        Decimate = DECIMATE,
    }
}

/// The matched filter (`modem.matched_filter`): `Iq` in, `Iq` out at the input rate divided by `decimate`.
///
/// The taps are the pulse sampled at `sps` input samples per symbol over `span` symbols (`sdrkit_algo_dsp::pulse::taps`), scaled to unit
/// energy so white noise keeps its power; a `FirDecimator` filters and keeps one output in `decimate`, computing only those. Tags
/// are moved to the output item made after the tagged input.
///
/// Settings (each rebuilds the filter): `pulse` (`rrc`, `rc`, `gaussian`, `half_sine`, `rect`), `beta` (roll-off of `rrc` and
/// `rc`), `bt` (of `gaussian`), `span` (symbols), `sps` (input samples per symbol, fractional allowed), `decimate` (a whole
/// number; the output then has `sps / decimate` samples per symbol).
pub struct MatchedFilter {
    pulse: PulseShape,
    beta: f64,
    bt: f64,
    span: f64,
    sps: f64,
    decimate: usize,
    fir: Option<FirDecimator<Complex32>>,
    carry: TagCarry,
}

impl Default for MatchedFilter {
    /// RRC, roll-off 0.35, 8 symbols, 4 samples per symbol, no decimation.
    fn default() -> Self {
        MatchedFilter {
            pulse: PULSE.default,
            beta: BETA.default,
            bt: BT.default,
            span: SPAN.default,
            sps: SPS.default,
            decimate: DECIMATE.default as usize,
            fir: None,
            carry: TagCarry::same_rate(),
        }
    }
}

impl MatchedFilter {
    /// A filter matched to an RRC pulse of roll-off `beta` at `sps` samples per symbol, keeping one output in `decimate`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if a setting refuses its argument.
    pub fn rrc(beta: f64, sps: f64, decimate: usize) -> Result<Self, SettingError> {
        MatchedFilterSetting::Beta.check(&Value::F64(beta))?;
        MatchedFilterSetting::Sps.check(&Value::F64(sps))?;
        MatchedFilterSetting::Decimate.check(&Value::I64(decimate as i64))?;
        Ok(MatchedFilter {
            beta,
            sps,
            decimate,
            ..Self::default()
        })
    }
}

impl Component for MatchedFilter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "modem.matched_filter",
            vec![Port::iq("in")],
            vec![Port::iq("out").rate_from(RateFrom::DividedBySetting {
                input: 0,
                by: MatchedFilterSetting::Decimate.id(),
            })],
        )
        .manual_tags()
        .with_settings(MatchedFilterSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        super::need_rate(f.rate)?;
        let s = shape(self.pulse, self.beta, self.bt)?;
        let h = taps(s, self.sps, self.span, Norm::Energy)?;
        self.fir = Some(FirDecimator::new(&h, self.decimate));
        let out_rate = f.rate / self.decimate as f64;
        self.carry = TagCarry::new(f.rate, out_rate);
        Ok(vec![Format {
            bandwidth_hz: f.bandwidth_hz.min(out_rate),
            ..f.with_rate(out_rate)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let input = io.input(0);
        let Some(fir) = &mut self.fir else { return };
        fir.process(x, io.iq_out(0));
        self.carry.forward(input, io.output(0));
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match MatchedFilterSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MatchedFilterSetting::Pulse => self.pulse = PULSE.read(id, v)?,
            MatchedFilterSetting::Beta => self.beta = BETA.read(id, v)?,
            MatchedFilterSetting::Bt => self.bt = BT.read(id, v)?,
            MatchedFilterSetting::Span => self.span = SPAN.read(id, v)?,
            MatchedFilterSetting::Sps => self.sps = SPS.read(id, v)?,
            MatchedFilterSetting::Decimate => self.decimate = DECIMATE.read(id, v)? as usize,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        use sdrkit_core::Choice;
        values_of([
            (
                MatchedFilterSetting::Pulse,
                Value::Choice(self.pulse.index()),
            ),
            (MatchedFilterSetting::Beta, Value::F64(self.beta)),
            (MatchedFilterSetting::Bt, Value::F64(self.bt)),
            (MatchedFilterSetting::Span, Value::F64(self.span)),
            (MatchedFilterSetting::Sps, Value::F64(self.sps)),
            (
                MatchedFilterSetting::Decimate,
                Value::I64(self.decimate as i64),
            ),
        ])
    }
}
