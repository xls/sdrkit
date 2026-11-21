//! The demodulator component of the links with phase modulation and a residual carrier.

use super::link::{control_port, item_format, item_port, DemodReport};
use super::{controls, Control};
use crate::psk::DemodState;
use sdrkit_algo_modem::pm::{PmConfig, PmDemod};
use sdrkit_core::{ConfigError, SettingError, SettingId, Value, Values};
use sdrkit_graph::{no_values, Applied, Component, ComponentInfo, Format, Io, Port, Timed};

/// The lock value above which the carrier counts as found.
pub const PM_LOCK: f32 = 0.02;

/// The residual-carrier PM demodulator: one soft value out for each symbol (a chip of a split-phase link), as LLRs (positive
/// means 0, a symbol of negative phase).
///
/// Control items: [`Control::RestartCarrier`] and [`Control::RejectCarrier`] search the carrier again.
pub struct PmDemodComponent {
    symbol_rate: f64,
    roll_off: f64,
    search_hz: f64,
    demod: Option<PmDemod>,
    rate: f64,
    soft: Vec<i8>,
    samples: u64,
}

impl PmDemodComponent {
    /// A demodulator for `symbol_rate` symbols a second with a matched filter of `roll_off`, whose carrier search covers
    /// `search_hz` to each side.
    pub fn new(symbol_rate: f64, roll_off: f64, search_hz: f64) -> Self {
        PmDemodComponent {
            symbol_rate,
            roll_off,
            search_hz,
            demod: None,
            rate: 0.0,
            soft: Vec::new(),
            samples: 0,
        }
    }
}

impl Default for PmDemodComponent {
    /// A demodulator for split-phase data at 8320 bit/s.
    fn default() -> Self {
        PmDemodComponent::new(16_640.0, 1.0, 7500.0)
    }
}

impl Component for PmDemodComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.pm_demod",
            vec![Port::iq("in"), control_port()],
            vec![Port::llr("soft"), item_port::<DemodReport>("report")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        let mut cfg = PmConfig::new(f.rate, self.symbol_rate, self.roll_off);
        cfg.search_hz = self.search_hz;
        self.demod = Some(PmDemod::new(cfg)?);
        self.samples = 0;
        let mut o = Format::llr(self.symbol_rate, None);
        o.tuned_hz = f.tuned_hz;
        Ok(vec![o, item_format::<DemodReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(d) = &mut self.demod else { return };
        for c in controls(io, 1) {
            if matches!(c, Control::RestartCarrier | Control::RejectCarrier) {
                d.restart();
            }
        }
        let x = io.iq(0);
        self.samples += x.len() as u64;
        self.soft.clear();
        d.process(x, &mut self.soft);
        io.llrs_out(0).extend_from_slice(&self.soft);
        if io.tick() {
            let lock = d.lock_value();
            let r = DemodReport {
                state: DemodState {
                    tracking: d.tracking(),
                    locked: d.tracking() && lock > PM_LOCK,
                    lock_metric: lock,
                    carrier_hz: d.carrier_hz(),
                    es_n0_db: d.es_n0_db(),
                    symbol_rate: self.symbol_rate * (1.0 + d.clock_ppm() * 1e-6),
                    clock_ppm: d.clock_ppm(),
                    symbols: d.symbols(),
                    searches: d.searches(),
                },
                symbol_rate_nominal: self.symbol_rate,
                constellation: d.constellation.iter().copied().collect(),
                time_s: self.samples as f64 / self.rate.max(1.0),
            };
            io.items_out::<DemodReport>(1).push(Timed {
                time: r.time_s,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
}
