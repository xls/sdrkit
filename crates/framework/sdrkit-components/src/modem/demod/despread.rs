//! `modem.despread`: direct-sequence spread spectrum to symbols, with code acquisition and an optional delay-locked loop.

use super::pieces;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_modem::dsss::{
    chips_from_bits, ChipDespreader, CodeAcquisition, CodeTracker, Correlations, TrackerConfig,
    BARKER_11,
};
use sdrkit_core::publish::field;
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{Complex32, ConfigError, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Every, Format, Io, Kind, Port};

sdrkit_core::choice_enum! {
    /// The spreading code of [`Despread`].
    pub enum SpreadingCode {
        /// Barker 11 (IEEE 802.11 DSSS).
        Barker11 = ("barker_11", "Barker 11 (IEEE 802.11 DSSS)"),
        /// GPS C/A (PRN).
        GpsCa = ("gps_ca", "GPS C/A (PRN)"),
        /// Custom chips.
        Custom = ("custom", "Custom chips"),
    }
}

/// Samples worked on at a time (the scratch buffers' size).
const PIECE: usize = 4096;

/// Chips given as text: `+` and `-`, or `0` and `1` (0 is +1, as `dsss::chips_from_bits`).
fn parse_chips(s: &str) -> Option<Vec<f32>> {
    let t: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).collect();
    if t.is_empty() {
        return None;
    }
    if t.iter().all(|c| *c == '+' || *c == '-') {
        return Some(
            t.iter()
                .map(|&c| if c == '+' { 1.0 } else { -1.0 })
                .collect(),
        );
    }
    if t.iter().all(|c| *c == '0' || *c == '1') {
        let bits: Vec<u8> = t.iter().map(|&c| (c == '1') as u8).collect();
        let mut out = Vec::new();
        chips_from_bits(&bits, &mut out);
        return Some(out);
    }
    None
}

/// The despreader at work.
#[derive(Debug)]
enum Engine {
    Idle,
    /// Gathering periods for the acquisition (their samples are kept and despread once it is done).
    Acquire {
        acq: Box<CodeAcquisition>,
        buf: Vec<Complex32>,
        periods: usize,
    },
    /// A fixed code phase and carrier.
    Fixed {
        desp: Box<ChipDespreader>,
        nco: Nco,
        mixed: Vec<Complex32>,
    },
    /// The delay-locked loop.
    Dll {
        tracker: Box<CodeTracker>,
        corr: Vec<Correlations>,
    },
}

sdrkit_core::choice_enum! {
    /// The code tracking of [`Despread`].
    pub enum CodeTracking {
        /// None.
        None = ("none", "None"),
        /// Delay-locked loop.
        Dll = ("dll", "Delay-locked loop"),
    }
}

const CODE: Setting<field::Choice<SpreadingCode>> =
    Setting::choice("code", "Code", SpreadingCode::Barker11);
const PRN: Setting<field::I64> = Setting::new("prn", "PRN", "", 1)
    .range(1.0, 32.0, 1.0)
    .help("The GPS satellite's code number.");
const CHIPS: Setting<field::Text> =
    Setting::new("chips", "Chips", "", "").help("A custom code: + and -, or 0 and 1 (0 is +1).");
const CHIP_RATE_HZ: Setting<field::Hz> =
    Setting::new("chip_rate_hz", "Chip rate", "Hz", 1e6).range(1.0, 1e9, 0.0);
const PERIODS_PER_SYMBOL: Setting<field::I64> =
    Setting::new("periods_per_symbol", "Periods per symbol", "", 1)
        .range(1.0, 10_000.0, 1.0)
        .help("Code periods summed into one symbol (20 for the GPS navigation message).");
const TRACKING: Setting<field::Choice<CodeTracking>> =
    Setting::choice("tracking", "Code tracking", CodeTracking::None);
const ACQUIRE_PERIODS: Setting<field::I64> = Setting::new("acquire_periods", "Acquisition periods", "", 0).range(0.0, 1000.0, 1.0).help("Code periods searched for the code phase and carrier first; 0 takes the code to start at once.");
const MAX_SHIFT: Setting<field::I64> = Setting::new("max_shift", "Carrier search", "", 0)
    .range(0.0, 64.0, 1.0)
    .help("Carrier offsets searched, in bins of one over the code period.");
const BANDWIDTH: Setting<field::F64> = Setting::new("bandwidth", "Loop bandwidth", "", 0.01)
    .range(1e-4, 0.4, 0.0)
    .help("Noise bandwidth of the delay-locked loop per code period.");

sdrkit_core::settings! {
    /// The settings of [`Despread`].
    pub enum DespreadSetting {
        /// Code.
        Code = CODE,
        /// PRN.
        Prn = PRN,
        /// Chips.
        Chips = CHIPS,
        /// Chip rate.
        ChipRateHz = CHIP_RATE_HZ,
        /// Periods per symbol.
        PeriodsPerSymbol = PERIODS_PER_SYMBOL,
        /// Code tracking.
        Tracking = TRACKING,
        /// Acquisition periods.
        AcquirePeriods = ACQUIRE_PERIODS,
        /// Carrier search.
        MaxShift = MAX_SHIFT,
        /// Loop bandwidth.
        Bandwidth = BANDWIDTH,
    }
}

/// `modem.despread`: DSSS (`Iq`) to one complex symbol per `periods_per_symbol` code periods (`Symbols`, the mean of the period
/// correlations: an aligned unit-amplitude signal produces 1).
///
/// The code is `barker_11` (`sdrkit_algo_modem::dsss::BARKER_11`), `gps_ca` of PRN `prn` (`sdrkit_util_bits::seq::gps_ca`) or `custom`
/// (`chips`, as `+-` or `01`), at `chip_rate_hz` (any whole or fractional number of samples per chip, at least one).
///
/// With `acquire_periods` above 0 the code phase and the carrier offset are first found by FFT correlation over that many periods
/// (`dsss::CodeAcquisition`, carrier offsets within `max_shift` bins of one over the period, a quarter bin apart; `max_shift` 0
/// searches no carrier offset); those samples are then despread
/// too, so no symbol is lost. With 0 the code is taken to start at the first sample. Then `tracking` `none` despreads at that code
/// phase and carrier (`dsss::ChipDespreader`), and `dll` follows the code with early, prompt and late correlators
/// (`dsss::CodeTracker`, loop bandwidth `bandwidth` per period, wiping the carrier found). The carrier's phase is left in the
/// symbols for a carrier loop to remove. A `Retuned` or `Discontinuity` tag starts the acquisition again. Publishes `offset_hz`
/// (the carrier offset found) and `locked` (acquired).
#[derive(Debug)]
pub struct Despread {
    code_name: SpreadingCode,
    prn: u8,
    chips: String,
    chip_rate_hz: f64,
    periods_per_symbol: u32,
    dll: bool,
    acquire_periods: u32,
    max_shift: u32,
    bandwidth: f64,
    code: Vec<f32>,
    spc: f64,
    rate: f64,
    engine: Engine,
    periods: Vec<Complex32>,
    acc: Complex32,
    count: u32,
    freq: f64,
    acquired: bool,
    results: Published,
    every: Every,
}

impl Default for Despread {
    fn default() -> Self {
        Despread {
            code_name: SpreadingCode::Barker11,
            prn: PRN.default as _,
            chips: String::new(),
            chip_rate_hz: CHIP_RATE_HZ.default,
            periods_per_symbol: PERIODS_PER_SYMBOL.default as _,
            dll: false,
            acquire_periods: ACQUIRE_PERIODS.default as _,
            max_shift: MAX_SHIFT.default as _,
            bandwidth: BANDWIDTH.default,
            code: Vec::new(),
            spc: 1.0,
            rate: 0.0,
            engine: Engine::Idle,
            periods: Vec::new(),
            acc: Complex32::default(),
            count: 0,
            freq: 0.0,
            acquired: false,
            results: Published::new(&[&keys::OFFSET_HZ, &keys::LOCKED]),
            every: Every::default(),
        }
    }
}

impl Despread {
    fn the_code(&self) -> Result<Vec<f32>, ConfigError> {
        match self.code_name {
            SpreadingCode::Barker11 => Ok(BARKER_11.iter().map(|&c| c as f32).collect()),
            SpreadingCode::GpsCa => {
                let mut bits = [0u8; 1023];
                sdrkit_util_bits::seq::gps_ca(self.prn, &mut bits).map_err(crate::stage)?;
                let mut c = Vec::with_capacity(1023);
                chips_from_bits(&bits, &mut c);
                Ok(c)
            }
            SpreadingCode::Custom => {
                parse_chips(&self.chips).ok_or(ConfigError::Setting(SettingError::Required {
                    id: DespreadSetting::Chips.id(),
                }))
            }
        }
    }

    /// The engine after an acquisition at `code_phase` samples into each period and carrier `freq` (cycles per sample), or at
    /// the stream start.
    fn running(&self, code_phase: f64, freq: f64) -> Result<Engine, ConfigError> {
        if self.dll {
            let mut cfg = TrackerConfig::new(self.bandwidth);
            cfg.carrier_to_code = 0.0;
            let mut t = CodeTracker::new(&self.code, self.spc, cfg).map_err(ConfigError::from)?;
            // The next sample lies `period - code_phase` samples into the period that started before it.
            let chips = self.code.len() as f64;
            t.set_code_phase((-code_phase / self.spc).rem_euclid(chips));
            t.set_carrier(freq);
            Ok(Engine::Dll {
                tracker: Box::new(t),
                corr: Vec::with_capacity(PIECE),
            })
        } else {
            let mut d = ChipDespreader::new(&self.code, self.spc).map_err(ConfigError::from)?;
            d.align(code_phase);
            Ok(Engine::Fixed {
                desp: Box::new(d),
                nco: Nco::new(1.0, -freq),
                mixed: vec![Complex32::default(); PIECE],
            })
        }
    }

    fn start(&mut self) -> Result<(), ConfigError> {
        self.acc = Complex32::default();
        self.count = 0;
        self.acquired = false;
        self.engine = if self.acquire_periods > 0 {
            // With no carrier search there is one hypothesis, at 0 Hz; otherwise each bin is searched in quarters.
            let sub = if self.max_shift == 0 { 1 } else { 4 };
            let acq = CodeAcquisition::new(&self.code, self.spc, self.max_shift, sub)
                .map_err(ConfigError::from)?;
            let n = acq.period() * self.acquire_periods as usize;
            Engine::Acquire {
                acq: Box::new(acq),
                buf: Vec::with_capacity(n),
                periods: self.acquire_periods as usize,
            }
        } else {
            self.freq = 0.0;
            self.acquired = true;
            self.running(0.0, 0.0)?
        };
        Ok(())
    }

    /// Despreads `x`, appending the period correlations to `self.periods`.
    fn run(&mut self, x: &[Complex32]) {
        match &mut self.engine {
            Engine::Idle => {}
            Engine::Acquire { acq, buf, periods } => {
                let need = acq.period() * *periods;
                let take = (need - buf.len()).min(x.len());
                buf.extend_from_slice(&x[..take]);
                if buf.len() < need {
                    return;
                }
                let n = acq.period();
                for p in buf.chunks_exact(n) {
                    // Each piece is exactly one period, so this cannot fail.
                    let _ = acq.accumulate(p);
                }
                let Some(found) = acq.result() else { return };
                let held = std::mem::take(buf);
                self.freq = found.freq;
                self.acquired = true;
                match self.running(found.code_phase, found.freq) {
                    Ok(e) => self.engine = e,
                    Err(_) => {
                        self.engine = Engine::Idle;
                        return;
                    }
                }
                // Despread what the acquisition saw, then the rest of `x`.
                for p in held.chunks(PIECE) {
                    self.run(p);
                }
                self.run(&x[take..]);
            }
            Engine::Fixed { desp, nco, mixed } => {
                for p in x.chunks(PIECE) {
                    let m = &mut mixed[..p.len()];
                    nco.mix(p, m);
                    desp.process(m, &mut self.periods);
                }
            }
            Engine::Dll { tracker, corr } => {
                for p in x.chunks(PIECE) {
                    corr.clear();
                    tracker.process(p, corr);
                    self.periods.extend(corr.iter().map(|c| c.prompt));
                }
            }
        }
    }
}

impl Component for Despread {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "modem.despread",
            vec![Port::iq("in")],
            vec![Port::symbols("out")],
        )
        .with_settings(DespreadSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.code = self.the_code()?;
        self.spc = f.rate / self.chip_rate_hz;
        if self.spc.is_nan() || self.spc < 1.0 {
            return Err(ConfigError::SettingAtRate {
                id: DespreadSetting::ChipRateHz.id(),
                rate_hz: f.rate,
            });
        }
        self.rate = f.rate;
        self.periods = Vec::with_capacity(PIECE + 64);
        self.start()?;
        self.every = Every::new(f.rate, 0.25);
        let symbol_rate =
            self.chip_rate_hz / (self.code.len() as f64 * self.periods_per_symbol as f64);
        Ok(vec![Format {
            bandwidth_hz: symbol_rate,
            ..f.with_kind(Kind::Symbols).with_rate(symbol_rate)
        }])
    }

    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(0);
        let x = io.iq(0);
        for (a, b, reset) in pieces(io.input(0), n) {
            if reset && self.start().is_err() {
                self.engine = Engine::Idle;
            }
            for p in x[a..b].chunks(PIECE) {
                self.periods.clear();
                self.run(p);
                let per = self.periods_per_symbol;
                let out = io.samples_out::<Complex32>(0);
                for &c in &self.periods {
                    self.acc += c;
                    self.count += 1;
                    if self.count == per {
                        out.push(self.acc / per as f32);
                        self.acc = Complex32::default();
                        self.count = 0;
                    }
                }
            }
        }
        if self.every.tick(n) {
            self.results.set(&keys::LOCKED, self.acquired);
            if self.acquired {
                self.results.set(&keys::OFFSET_HZ, self.freq * self.rate);
            } else {
                self.results.none(&keys::OFFSET_HZ);
            }
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DespreadSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DespreadSetting::Code => self.code_name = CODE.read(id, v)?,
            DespreadSetting::Prn => self.prn = PRN.read(id, v)? as u8,
            DespreadSetting::Chips => {
                let t = CHIPS.read(id, v)?;
                if !t.trim().is_empty() && parse_chips(t).is_none() {
                    return Err(SettingError::Malformed { id, at: None });
                }
                self.chips = t.to_owned();
            }
            DespreadSetting::ChipRateHz => self.chip_rate_hz = CHIP_RATE_HZ.read(id, v)?,
            DespreadSetting::PeriodsPerSymbol => {
                self.periods_per_symbol = PERIODS_PER_SYMBOL.read(id, v)? as u32
            }
            DespreadSetting::Tracking => self.dll = TRACKING.read(id, v)? == CodeTracking::Dll,
            DespreadSetting::AcquirePeriods => {
                self.acquire_periods = ACQUIRE_PERIODS.read(id, v)? as u32
            }
            DespreadSetting::MaxShift => self.max_shift = MAX_SHIFT.read(id, v)? as u32,
            DespreadSetting::Bandwidth => self.bandwidth = BANDWIDTH.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (DespreadSetting::Code, Value::Choice(self.code_name.index())),
            (DespreadSetting::Prn, Value::I64(self.prn as i64)),
            (DespreadSetting::Chips, Value::Text(self.chips.clone())),
            (DespreadSetting::ChipRateHz, Value::Hz(self.chip_rate_hz)),
            (
                DespreadSetting::PeriodsPerSymbol,
                Value::I64(self.periods_per_symbol as i64),
            ),
            (
                DespreadSetting::Tracking,
                Value::Choice(
                    if self.dll {
                        CodeTracking::Dll
                    } else {
                        CodeTracking::None
                    }
                    .index(),
                ),
            ),
            (
                DespreadSetting::AcquirePeriods,
                Value::I64(self.acquire_periods as i64),
            ),
            (DespreadSetting::MaxShift, Value::I64(self.max_shift as i64)),
            (DespreadSetting::Bandwidth, Value::F64(self.bandwidth)),
        ])
    }

    fn results(&self) -> Option<&sdrkit_core::Results> {
        Some(self.results.results())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chips_parse_both_ways() {
        assert_eq!(parse_chips("+-+"), Some(vec![1.0, -1.0, 1.0]));
        assert_eq!(parse_chips("0 1 1"), Some(vec![1.0, -1.0, -1.0]));
        assert_eq!(parse_chips("+0"), None);
        assert_eq!(parse_chips(""), None);
    }
}
