//! `modem.snr`: SNR, MER and EVM of a symbol stream.

use super::points::{ConstellationName, CONSTELLATION};
use super::{is_retune, points, split_at_tags};
use sdrkit_algo_modem::level::{DataAided, Evm, M2m4};
use sdrkit_algo_modem::Constellation;
use sdrkit_core::publish::field;
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{Complex32, ConfigError, Results, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Every, Format, Io, Port};

sdrkit_core::choice_enum! {
    /// The estimator of [`SnrMeter`].
    pub enum SnrMethod {
        /// Blind (M2M4).
        M2m4 = ("m2m4", "Blind (M2M4)"),
        /// Decision directed.
        Dd = ("dd", "Decision directed"),
    }
}

const METHOD: Setting<field::Choice<SnrMethod>> =
    Setting::choice("method", "Estimator", SnrMethod::M2m4);
const INTERVAL_S: Setting<field::F64> =
    Setting::new("interval_s", "Interval", "s", 0.2).range(0.001, 60.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`SnrMeter`].
    pub enum SnrMeterSetting {
        /// Estimator.
        Method = METHOD,
        /// Constellation.
        Constellation = CONSTELLATION,
        /// Interval.
        IntervalS = INTERVAL_S,
    }
}

/// The SNR meter (`modem.snr`), a sink for `Symbols`: publishes `snr_db`, `mer_db` and `evm_pct`.
///
/// Each estimate covers the symbols of one interval of `interval_s` seconds of signal, then starts again (so a stream cut into
/// pieces of any size produces the same reports). The SNR is symbol energy over complex noise variance (`Es/N0` at one sample per
/// symbol), from `sdrkit_algo_modem::level::M2m4` (`m2m4`: blind, from the moments, for the kurtosis of `constellation`) or
/// `DataAided::push_decided` (`dd`: decisions on `constellation`; reads high below the SNR where decisions fail). EVM and MER come
/// from `Evm` with decisions on `constellation`. A `Retuned` tag starts a new interval. Settings: `method`, `constellation`,
/// `interval_s`.
pub struct SnrMeter {
    method: SnrMethod,
    constellation: ConstellationName,
    interval_s: f64,
    c: Option<Constellation>,
    m2m4: Option<M2m4>,
    da: DataAided,
    evm: Evm,
    every: Every,
    results: Published,
}

impl Default for SnrMeter {
    /// Blind, QPSK, every 0.2 s.
    fn default() -> Self {
        SnrMeter {
            method: SnrMethod::M2m4,
            constellation: ConstellationName::Qpsk,
            interval_s: INTERVAL_S.default,
            c: None,
            m2m4: None,
            da: DataAided::new(),
            evm: Evm::new(),
            every: Every::default(),
            results: Published::new(&[&keys::SNR_DB, &keys::MER_DB, &keys::EVM_PCT]),
        }
    }
}

impl SnrMeter {
    fn restart(&mut self) {
        if let Some(m) = &mut self.m2m4 {
            m.reset();
        }
        self.da.reset();
        self.evm.reset();
    }
    fn publish(&mut self) {
        let snr = if self.method == SnrMethod::M2m4 {
            self.m2m4.as_ref().and_then(|m| m.estimate())
        } else {
            self.da.estimate()
        };
        if let Some(s) = snr {
            self.results.set(&keys::SNR_DB, super::r2(s.snr_db));
        }
        if let Some(m) = self.evm.mer_db() {
            self.results.set(&keys::MER_DB, super::r2(m));
        }
        if let Some(e) = self.evm.evm_percent() {
            self.results.set(&keys::EVM_PCT, super::r2(e));
        }
        self.restart();
    }
}

impl Component for SnrMeter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("modem.snr", vec![Port::symbols("in")])
            .with_settings(SnrMeterSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        super::need_rate(f.rate)?;
        let c = points::constellation(self.constellation)?;
        self.m2m4 = Some(M2m4::for_constellation(&c).map_err(ConfigError::from)?);
        self.c = Some(c);
        self.every = Every::new(f.rate, self.interval_s);
        self.restart();
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.samples::<Complex32>(0);
        let input = io.input(0);
        if self.c.is_none() {
            return;
        }
        split_at_tags(
            self,
            input,
            x.len(),
            |me, a, b| {
                for &z in &x[a..b] {
                    if let (Some(c), Some(m)) = (&me.c, &mut me.m2m4) {
                        m.push(z);
                        me.da.push_decided(z, c);
                        me.evm.push_decided(z, c);
                    }
                    if me.every.tick(1) {
                        me.publish();
                    }
                }
            },
            |me, t| {
                if is_retune(t) {
                    me.restart();
                    me.every.reset();
                }
            },
        );
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SnrMeterSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SnrMeterSetting::Method => self.method = METHOD.read(id, v)?,
            SnrMeterSetting::Constellation => self.constellation = CONSTELLATION.read(id, v)?,
            SnrMeterSetting::IntervalS => self.interval_s = INTERVAL_S.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([
            (SnrMeterSetting::Method, Value::Choice(self.method.index())),
            (
                SnrMeterSetting::Constellation,
                Value::Choice(self.constellation.index()),
            ),
            (SnrMeterSetting::IntervalS, Value::F64(self.interval_s)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}
