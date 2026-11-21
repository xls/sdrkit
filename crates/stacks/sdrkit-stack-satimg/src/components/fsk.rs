//! The demodulator component of two-level FSK links (`satimg.fsk_demod`): the frequency of the signal read from the angle
//! between successive samples, a slow mean removed, a root raised cosine matched filter and a Gardner symbol clock.

use super::link::{control_port, item_format, item_port, DemodReport};
use super::{controls, Control};
use crate::psk::DemodState;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::interp::Cubic;
use sdrkit_algo_dsp::pulse::{rrc, Shape};
use sdrkit_algo_modem::timing::{Gardner, SymbolSync, SymbolSyncConfig};
use sdrkit_algo_modem::ModemError;
use sdrkit_core::{Complex32, ConfigError, SettingError, SettingId, Value, Values};
use sdrkit_graph::{no_values, Applied, Component, ComponentInfo, Format, Io, Port, Timed};
use std::collections::VecDeque;

/// Smoothing of the symbol size, for each symbol.
const AMP_ALPHA: f32 = 0.002;
/// Smoothing of the Es/N0 moments, for each symbol.
const SNR_ALPHA: f32 = 0.003;
/// Symbols kept for the constellation.
const POINTS: usize = 256;

/// The FSK demodulator: complex samples in, one soft value for each symbol out (LLRs, positive means 0; the higher frequency
/// is a 1).
#[derive(Debug)]
pub struct FskDemod {
    symbol_rate: f64,
    /// Angle a sample to frequency in units of half the symbol rate.
    scale: f32,
    alpha: f32,
    prev: Complex32,
    mean: f32,
    matched: Fir<f32>,
    data: Vec<f32>,
    sync: SymbolSync<f32, Gardner, Cubic>,
    symbols: Vec<f32>,
    amp: f32,
    m1: f32,
    m2: f32,
    count: u64,
    /// The newest symbols at unit size, on the real axis.
    pub constellation: VecDeque<[f32; 2]>,
}

impl FskDemod {
    /// A demodulator for `symbol_rate` symbols a second at `rate` samples a second, with a matched filter of `roll_off`.
    ///
    /// # Errors
    ///
    /// [`ModemError`] if there are fewer than 2 samples a symbol or the symbol clock refuses its numbers.
    pub fn new(rate: f64, symbol_rate: f64, roll_off: f64) -> Result<Self, ModemError> {
        let sps = rate / symbol_rate;
        let mut t = SymbolSyncConfig::new(sps);
        t.bn_t = 0.02;
        t.max_ppm = 5000.0;
        t.pulse = Shape::Rc { beta: roll_off };
        let sync = SymbolSync::new(Gardner, Cubic, t)?;
        // The matched filter: +-4 symbols, unit gain at DC so the levels keep their scale.
        let half = (4.0 * sps).ceil() as i64;
        let mut taps: Vec<f32> = (-half..=half)
            .map(|i| rrc(i as f64 / sps, roll_off) as f32)
            .collect();
        let sum: f32 = taps.iter().sum();
        taps.iter_mut().for_each(|t| *t /= sum.max(1e-12));
        Ok(FskDemod {
            symbol_rate,
            scale: (rate / (std::f64::consts::PI * symbol_rate)) as f32,
            alpha: (1.0 / (60.0 * sps)).min(0.5) as f32,
            prev: Complex32::new(0.0, 0.0),
            mean: 0.0,
            matched: Fir::new(&taps),
            data: Vec::new(),
            sync,
            symbols: Vec::new(),
            amp: 1.0,
            m1: 1.0,
            m2: 1.0,
            count: 0,
            constellation: VecDeque::with_capacity(POINTS),
        })
    }

    /// The symbol clock holds a lock.
    pub fn locked(&self) -> bool {
        self.sync.locked()
    }

    /// The lock metric of the symbol clock, 0 to 1.
    pub fn lock_metric(&self) -> f32 {
        self.sync.lock_metric().clamp(0.0, 1.0)
    }

    /// The carrier offset: the mean frequency, Hz.
    pub fn carrier_hz(&self) -> f64 {
        f64::from(self.mean) * self.symbol_rate / 2.0
    }

    /// The symbol clock against nominal, ppm.
    pub fn clock_ppm(&self) -> f64 {
        self.sync.clock_ppm()
    }

    /// Symbols since the start.
    pub fn symbols(&self) -> u64 {
        self.count
    }

    /// Es/N0 from the mean and the variance of the symbol sizes, not below -30 dB.
    pub fn es_n0_db(&self) -> f32 {
        let v = (self.m2 - self.m1 * self.m1).max(1e-9);
        (10.0 * (self.m1 * self.m1 / (2.0 * v)).log10()).max(-30.0)
    }

    /// Starts again: the mean, the clock and the levels.
    pub fn restart(&mut self) {
        self.mean = 0.0;
        self.sync.reset();
        self.count = 0;
    }

    /// Takes samples and appends one soft value for each symbol to `soft`.
    pub fn process(&mut self, input: &[Complex32], soft: &mut Vec<i8>) {
        self.data.clear();
        for &z in input {
            let f = (z * self.prev.conj()).arg() * self.scale;
            self.prev = z;
            self.mean += self.alpha * (f - self.mean);
            self.data.push(self.matched.process_sample(f - self.mean));
        }
        self.symbols.clear();
        self.sync.process(&self.data, &mut self.symbols);
        for &s in &self.symbols {
            self.amp += AMP_ALPHA * (s.abs() - self.amp);
            let y = s / self.amp.max(1e-6);
            self.m1 += SNR_ALPHA * (y.abs() - self.m1);
            self.m2 += SNR_ALPHA * (y * y - self.m2);
            self.count += 1;
            if self.constellation.len() == POINTS {
                self.constellation.pop_front();
            }
            self.constellation.push_back([y, 0.0]);
            soft.push((-y * 40.0).clamp(-127.0, 127.0) as i8);
        }
    }
}

/// The FSK demodulator as a component: soft symbols out as LLRs and a [`DemodReport`] at each tick.
///
/// Control items: [`Control::RestartCarrier`] and [`Control::RejectCarrier`] start it again.
pub struct FskDemodComponent {
    symbol_rate: f64,
    roll_off: f64,
    demod: Option<FskDemod>,
    rate: f64,
    soft: Vec<i8>,
    samples: u64,
}

impl FskDemodComponent {
    /// A demodulator for `symbol_rate` symbols a second with a matched filter of `roll_off`.
    pub fn new(symbol_rate: f64, roll_off: f64) -> Self {
        FskDemodComponent {
            symbol_rate,
            roll_off,
            demod: None,
            rate: 0.0,
            soft: Vec::new(),
            samples: 0,
        }
    }
}

impl Default for FskDemodComponent {
    /// A demodulator for 100 000 symbols a second, roll-off 0.5.
    fn default() -> Self {
        FskDemodComponent::new(100e3, 0.5)
    }
}

impl Component for FskDemodComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.fsk_demod",
            vec![Port::iq("in"), control_port()],
            vec![Port::llr("soft"), item_port::<DemodReport>("report")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.demod = Some(FskDemod::new(f.rate, self.symbol_rate, self.roll_off)?);
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
            let r = DemodReport {
                state: DemodState {
                    tracking: d.symbols() > 0,
                    locked: d.locked(),
                    lock_metric: d.lock_metric(),
                    carrier_hz: d.carrier_hz(),
                    es_n0_db: d.es_n0_db(),
                    symbol_rate: self.symbol_rate * (1.0 + d.clock_ppm() * 1e-6),
                    clock_ppm: d.clock_ppm(),
                    symbols: d.symbols(),
                    searches: 0,
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
