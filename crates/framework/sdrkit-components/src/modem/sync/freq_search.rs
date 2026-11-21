//! `modem.freq_search`: feed-forward carrier frequency search, and the shift that removes the offset found.

use super::{is_retune, split_at_tags};
use sdrkit_algo_dsp::fft::{fft_shift, Fft};
use sdrkit_algo_dsp::floor::BinMedian;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_modem::freq::{centroid, lag_phase, PowerLine};
use sdrkit_core::publish::field;
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::EventDef;
use sdrkit_core::Field;
use sdrkit_core::Record;
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{Complex32, ConfigError, Results, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Item, Port, TagCarry, Timed};

sdrkit_core::choice_enum! {
    /// The estimator of [`FreqSearch`].
    pub enum FreqMethod {
        /// M-th power line.
        PowerLine = ("power_line", "M-th power line"),
        /// Line pair (MSK, OQPSK).
        LinePair = ("line_pair", "Line pair (MSK, OQPSK)"),
        /// Lag autocorrelation.
        Lag = ("lag", "Lag autocorrelation"),
        /// Spectral centroid.
        Centroid = ("centroid", "Spectral centroid"),
    }
}

/// One frequency estimate of `modem.freq_search`, made from one block of `fft` samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FreqEstimate {
    /// The carrier offset found, Hz from 0 Hz in the input stream (the output is shifted by minus this from the next block on).
    pub offset_hz: f64,
    /// The line's power over the median noise of the bins searched, dB (`power_line`, `line_pair`); `None` for the others.
    pub snr_db: Option<f32>,
    /// The measured spacing of the two lines, Hz (`line_pair`: the symbol rate of MSK or OQPSK); `None` for the others.
    pub spacing_hz: Option<f64>,
    /// The stream index of the block's first sample.
    pub start: u64,
}

const SPACING: Field<field::Hz> = Field::new("spacing_hz", "Line spacing", "Hz");
const START: Field<field::I64> = Field::new("start", "First sample", "");

static FREQ_ESTIMATE: EventDef = EventDef::new(
    "modem.freq_estimate",
    "Frequency estimate",
    &[&keys::OFFSET_HZ, &keys::SNR_DB, &SPACING, &START],
);

impl Item for FreqEstimate {
    const KIND: &'static str = "modem.freq_estimate";
    fn event_def() -> Option<&'static EventDef> {
        Some(&FREQ_ESTIMATE)
    }
    fn write_fields(&self, r: &mut Record) {
        r.set(&keys::OFFSET_HZ, self.offset_hz);
        r.opt(&keys::SNR_DB, self.snr_db.map(f64::from));
        r.opt(&SPACING, self.spacing_hz);
        r.set(&START, self.start as i64);
    }
}

/// The estimator chosen in `configure`.
enum Method {
    Line(PowerLine),
    Pair(PowerLine, f64),
    Lag(usize),
    Centroid(Fft, BinMedian, f64),
}

const METHOD: Setting<field::Choice<FreqMethod>> =
    Setting::choice("method", "Method", FreqMethod::PowerLine);
const M: Setting<field::I64> = Setting::new("m", "Power", "", 4)
    .range(1.0, 16.0, 1.0)
    .help("1 for a tone, 2 for BPSK, 4 for QPSK and square QAM, 8 for 8PSK.");
const SPAN_HZ: Setting<field::Hz> = Setting::new("span_hz", "Search span", "Hz", 0.0)
    .range(0.0, 1e9, 0.0)
    .help("Carriers within this much of 0 Hz; 0 for the whole unambiguous range.");
const FFT: Setting<field::I64> =
    Setting::new("fft", "Block", "samples", 4096).range(64.0, 65536.0, 1.0);
const LAG: Setting<field::I64> = Setting::new("lag", "Lag", "samples", 1).range(1.0, 65535.0, 1.0);
const SPACING_HZ: Setting<field::Hz> =
    Setting::new("spacing_hz", "Line spacing", "Hz", 0.0).range(0.0, 1e9, 0.0);

sdrkit_core::settings! {
    /// The settings of [`FreqSearch`].
    pub enum FreqSearchSetting {
        /// Method.
        Method = METHOD,
        /// Power.
        M = M,
        /// Search span.
        SpanHz = SPAN_HZ,
        /// Block.
        Fft = FFT,
        /// Lag.
        Lag = LAG,
        /// Line spacing.
        SpacingHz = SPACING_HZ,
    }
}

/// The frequency search (`modem.freq_search`): `Iq` in, the same `Iq` shifted by minus the latest estimate out, and a typed
/// [`FreqEstimate`] per block.
///
/// The input is cut into blocks of `fft` samples (counted from the start or the last retune). Each block is raised to the power `m`
/// (1 for a tone, 2 for BPSK, 4 for QPSK and square QAM, 8 for 8PSK), which removes the modulation, and searched by `method`:
///
/// - `power_line`: the strongest line of `x^m` within `span_hz` (`sdrkit_algo_modem::freq::PowerLine::estimate`);
/// - `line_pair`: the two lines `spacing_hz` apart (`PowerLine::estimate_pair`; MSK, OQPSK, GMSK with `m = 2`, spacing the symbol
///   rate);
/// - `lag`: the phase of the autocorrelation of `x^m` at `lag` samples (`lag_phase`; unambiguous within `rate / (2 lag m)`);
/// - `centroid`: the power-weighted mean frequency of the spectrum of `x^m` above its median, within `span_hz` (`centroid`).
///
/// The estimate of one block shifts the output from the next block on (an `sdrkit_algo_dsp::nco::Nco`, phase continuous). A block whose
/// search fails (silence, no line in the span) keeps the previous estimate and returns no item. A `Retuned` tag forgets the
/// estimate and starts a new block. Results: `offset_hz`. Settings: `method`, `m`, `span_hz` (0 for the whole unambiguous range),
/// `fft` (a power of two, 64 to 65536), `lag`, `spacing_hz`; each rebuilds the search.
pub struct FreqSearch {
    method: FreqMethod,
    m: u32,
    span_hz: f64,
    fft: usize,
    lag: usize,
    spacing_hz: f64,
    rate: f64,
    est: Option<Method>,
    nco: Option<Nco>,
    block: Vec<Complex32>,
    scratch: Vec<Complex32>,
    power: Vec<f32>,
    block_start: u64,
    offset_hz: f64,
    carry: TagCarry,
    results: Published,
}

impl Default for FreqSearch {
    /// The 4th-power line over the whole range, blocks of 4096.
    fn default() -> Self {
        FreqSearch {
            method: FreqMethod::PowerLine,
            m: M.default as _,
            span_hz: SPAN_HZ.default,
            fft: FFT.default as _,
            lag: LAG.default as _,
            spacing_hz: SPACING_HZ.default,
            rate: 0.0,
            est: None,
            nco: None,
            block: Vec::new(),
            scratch: Vec::new(),
            power: Vec::new(),
            block_start: 0,
            offset_hz: 0.0,
            carry: TagCarry::same_rate(),
            results: Published::new(&[&keys::OFFSET_HZ]),
        }
    }
}

impl FreqSearch {
    /// A search with `method`, raising the signal to the power `m`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `m` setting refuses `m`.
    pub fn new(method: FreqMethod, m: u32) -> Result<Self, SettingError> {
        FreqSearchSetting::M.check(&Value::I64(i64::from(m)))?;
        Ok(FreqSearch {
            method,
            m,
            ..Self::default()
        })
    }
    /// The latest estimate, Hz.
    pub fn offset_hz(&self) -> f64 {
        self.offset_hz
    }

    /// Searches the full block; returns the estimate in cycles per sample, its SNR and line spacing (cycles per sample).
    fn search(&mut self) -> Option<(f64, Option<f32>, Option<f64>)> {
        let m = self.m;
        match self.est.as_mut()? {
            Method::Line(pl) => pl
                .estimate(&self.block)
                .ok()
                .map(|l| (l.freq, Some(l.snr_db), None)),
            Method::Pair(pl, sp) => pl
                .estimate_pair(&self.block, *sp)
                .ok()
                .map(|l| (l.freq, Some(l.snr_db), Some(l.spacing))),
            Method::Lag(lag) => {
                self.scratch.clear();
                self.scratch.extend(self.block.iter().map(|z| pow(*z, m)));
                lag_phase(&self.scratch, *lag)
                    .ok()
                    .map(|f| (f / m as f64, None, None))
            }
            Method::Centroid(fft, floor, span) => {
                self.scratch.clear();
                self.scratch.extend(self.block.iter().map(|z| pow(*z, m)));
                if self
                    .scratch
                    .iter()
                    .any(|z| !(z.re.is_finite() && z.im.is_finite()))
                {
                    return None;
                }
                fft.process(&mut self.scratch);
                self.power.clear();
                self.power.extend(self.scratch.iter().map(|z| z.norm_sqr()));
                fft_shift(&mut self.power);
                let n = self.power.len();
                let half = ((*span * m as f64 * n as f64).floor() as usize).min(n / 2);
                let (lo, hi) = (n / 2 - half, (n / 2 + half + 1).min(n));
                let fl = floor.estimate(self.power.iter().copied())?;
                let bin = centroid(&self.power, lo, hi, fl)?;
                Some(((bin - (n / 2) as f64) / n as f64 / m as f64, None, None))
            }
        }
    }

    fn block_done(&mut self, items: &mut Vec<Timed<FreqEstimate>>) {
        if let Some((f, snr, spacing)) = self.search() {
            self.offset_hz = f * self.rate;
            if let Some(n) = &mut self.nco {
                n.set_freq(-self.offset_hz);
            }
            self.results
                .set(&keys::OFFSET_HZ, super::r2(self.offset_hz));
            items.push(Timed {
                time: self.block_start as f64 / self.rate,
                value: FreqEstimate {
                    offset_hz: self.offset_hz,
                    snr_db: snr,
                    spacing_hz: spacing.map(|s| s * self.rate),
                    start: self.block_start,
                },
            });
        }
        self.block.clear();
    }

    fn restart(&mut self) {
        self.block.clear();
        self.offset_hz = 0.0;
        if let Some(n) = &mut self.nco {
            n.set_freq(0.0);
        }
    }
}

/// `z^m` by repeated multiplication.
#[inline]
fn pow(z: Complex32, m: u32) -> Complex32 {
    let mut w = Complex32::new(1.0, 0.0);
    for _ in 0..m {
        w *= z;
    }
    w
}

impl Component for FreqSearch {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "modem.freq_search",
            vec![Port::iq("in")],
            vec![Port::iq("out"), Port::typed::<FreqEstimate>("estimate")],
        )
        .manual_tags()
        .with_settings(FreqSearchSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        super::need_rate(f.rate)?;
        self.rate = f.rate;
        let full = 0.5 / self.m as f64;
        let span = if self.span_hz > 0.0 {
            self.span_hz / f.rate
        } else {
            full
        };
        if span > full + 1e-12 {
            return Err(ConfigError::SettingAtRate {
                id: FreqSearchSetting::SpanHz.id(),
                rate_hz: f.rate,
            });
        }
        let span = span.min(full);
        self.est = Some(match self.method {
            FreqMethod::PowerLine | FreqMethod::LinePair => {
                let mut pl = PowerLine::new(self.m, self.fft)?;
                pl.set_span(span)?;
                if self.method == FreqMethod::PowerLine {
                    Method::Line(pl)
                } else {
                    let sp = self.spacing_hz / f.rate;
                    if !(sp > 0.0 && sp < 0.5) {
                        return Err(ConfigError::SettingAtRate {
                            id: FreqSearchSetting::SpacingHz.id(),
                            rate_hz: f.rate,
                        });
                    }
                    Method::Pair(pl, sp)
                }
            }
            FreqMethod::Lag => {
                if self.lag >= self.fft {
                    return Err(SettingError::Conflict {
                        id: FreqSearchSetting::Lag.id(),
                        with: FreqSearchSetting::Fft.id(),
                    }
                    .into());
                }
                Method::Lag(self.lag)
            }
            FreqMethod::Centroid => {
                let mut floor = BinMedian::new(1)?;
                floor.reserve(self.fft);
                Method::Centroid(Fft::forward(self.fft), floor, span)
            }
        });
        self.nco = Some(Nco::new(f.rate, 0.0));
        self.block = Vec::with_capacity(self.fft);
        self.scratch = Vec::with_capacity(self.fft);
        self.power = Vec::with_capacity(self.fft);
        self.block_start = 0;
        self.offset_hz = 0.0;
        self.carry = TagCarry::same_rate();
        self.results.set(&keys::OFFSET_HZ, 0.0);
        Ok(vec![
            f.clone(),
            Format::new(sdrkit_graph::Kind::typed::<FreqEstimate>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let input = io.input(0);
        if self.est.is_none() {
            return;
        }
        let first = input.first();
        // The items are written to a vector of the output port's own, taken out for the call so both outputs can be written.
        let mut items = std::mem::take(io.items_out::<FreqEstimate>(1));
        let out = io.iq_out(0);
        split_at_tags(
            &mut (&mut *self, out, &mut items),
            input,
            x.len(),
            |(me, out, items), a, b| {
                let mut i = a;
                while i < b {
                    if me.block.is_empty() {
                        me.block_start = first + i as u64;
                    }
                    let take = (me.fft - me.block.len()).min(b - i);
                    let seg = &x[i..i + take];
                    me.block.extend_from_slice(seg);
                    let s = out.len();
                    out.extend_from_slice(seg);
                    if let Some(n) = &mut me.nco {
                        n.mix_in_place(&mut out[s..]);
                    }
                    i += take;
                    if me.block.len() == me.fft {
                        me.block_done(items);
                    }
                }
            },
            |(me, _, _), t| {
                if is_retune(t) {
                    me.restart();
                }
            },
        );
        *io.items_out::<FreqEstimate>(1) = items;
        self.carry.forward(input, io.output(0));
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FreqSearchSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FreqSearchSetting::Method => self.method = METHOD.read(id, v)?,
            FreqSearchSetting::M => self.m = M.read(id, v)? as u32,
            FreqSearchSetting::SpanHz => self.span_hz = SPAN_HZ.read(id, v)?,
            FreqSearchSetting::Fft => {
                let n = FFT.read(id, v)? as usize;
                if !n.is_power_of_two() {
                    return Err(SettingError::NotPermitted { id });
                }
                self.fft = n;
            }
            FreqSearchSetting::Lag => self.lag = LAG.read(id, v)? as usize,
            FreqSearchSetting::SpacingHz => self.spacing_hz = SPACING_HZ.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([
            (
                FreqSearchSetting::Method,
                Value::Choice(self.method.index()),
            ),
            (FreqSearchSetting::M, Value::I64(self.m as i64)),
            (FreqSearchSetting::SpanHz, Value::Hz(self.span_hz)),
            (FreqSearchSetting::Fft, Value::I64(self.fft as i64)),
            (FreqSearchSetting::Lag, Value::I64(self.lag as i64)),
            (FreqSearchSetting::SpacingHz, Value::Hz(self.spacing_hz)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}
