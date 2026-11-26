//! The VLF MSK transmitter monitor: the carrier and the bit rate of an MSK station in 10 to 30 kHz from the two lines of the
//! squared signal, and the events when a transmitter comes on, goes off or changes its rate. It measures only: the traffic
//! is encrypted and is not decoded.
//!
//! Standard: no issuing body publishes the signals of the naval VLF stations. The use of MSK at up to 300 bit/s and the
//! encryption of the traffic are those of the Wikipedia articles "Very low frequency" and "VLF Transmitter Cutler"; the
//! carriers of the stations come from the host's data file (`data/vlf/supersid_transmitters.csv`).
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [vlf_msk: low pass to 4 kHz, the spectra of the samples and of their squares over 1.024 s
//!        segments, a report every 4 s: the level, the two lines, the state, the class, the station hint] -> rows
//! ```

use crate::receiver::GEN_SECTION;
use crate::senders::{Cx, Layer, Lcg, RowLog, Sender, StatusLine};
use crate::serial::gen::test_dial;
use crate::typed::{SenderRow, VlfMskFields, VlfTransmitterEvent, SENDER_ROWS, VLF_TRANSMITTER};
use sdrkit_algo_modem::msklines::{LineReport, SquaredLines};
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_core::interface::{Fields as _, Hertz, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, Hz, LockProfile, Receiver, ResultsDecl,
    Schema, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StackDescriptor,
    Value, Values,
};
use sdrkit_graph::{values_of, Kept};
use std::f64::consts::TAU;
use std::sync::Arc;

/// The channel rate.
pub const RATE: f64 = 4000.0;

/// The time of a report.
pub const REPORT_S: Setting<field::Duration> = Setting::new("report_s", "Report time", "s", 4.0)
    .range(2.0, 16.0, 1.0)
    .help("A state must hold for two reports before it is taken.");
/// The standard rates.
pub const RATES: Setting<field::Text> =
    Setting::new("rates", "Standard rates", "bit/s", "50,75,100,150,200,300")
        .help("A measured rate within the tolerance of one of these is shown as it.");
/// The rate tolerance.
pub const RATE_TOLERANCE: Setting<field::F64> =
    Setting::new("rate_tolerance", "Rate tolerance", "%", 8.0).range(2.0, 20.0, 1.0);
/// The station hint window.
pub const HINT_HZ: Setting<field::Hz> =
    Setting::new("hint_hz", "Station hint window", "Hz", 60.0).range(0.0, 500.0, 1.0);
/// The station list.
pub const STATIONS: Setting<field::Text> = Setting::new("stations", "Station list", "", "").help(
    "frequency_hz=name pairs separated by semicolons, from the host's data file. Empty: no hint.",
);
/// The bit rate of the test signal.
pub const GEN_RATE: Setting<field::F64> = Setting::new("gen_rate", "Bit rate", "bit/s", 200.0)
    .range(25.0, 600.0, 1.0)
    .section(GEN_SECTION)
    .advanced();
/// The carrier of the test signal from the tuned frequency.
pub const GEN_FC_HZ: Setting<field::Hz> = Setting::new("gen_fc_hz", "Carrier offset", "Hz", 0.0)
    .range(-1500.0, 1500.0, 1.0)
    .section(GEN_SECTION)
    .advanced();
/// When the test signal comes on.
pub const GEN_FROM_S: Setting<field::Duration> = Setting::new("gen_from_s", "On from", "s", 0.0)
    .range(0.0, 3600.0, 0.1)
    .section(GEN_SECTION)
    .advanced();
/// When the test signal goes off; 0 for never.
pub const GEN_TO_S: Setting<field::Duration> = Setting::new("gen_to_s", "Off at", "s", 0.0)
    .range(0.0, 3600.0, 0.1)
    .help("0: on to the end.")
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::settings! {
    /// The settings of the VLF MSK receiver.
    pub enum VlfMskRxSetting {
        /// Report time.
        ReportS = REPORT_S,
        /// Standard rates.
        Rates = RATES,
        /// Rate tolerance.
        RateTolerance = RATE_TOLERANCE,
        /// Station hint window.
        HintHz = HINT_HZ,
        /// Station list.
        Stations = STATIONS,
    }
}

sdrkit_core::settings! {
    /// The settings of the VLF MSK stack.
    pub enum VlfMskSetting {
        /// Report time.
        ReportS = REPORT_S,
        /// Standard rates.
        Rates = RATES,
        /// Rate tolerance.
        RateTolerance = RATE_TOLERANCE,
        /// Station hint window.
        HintHz = HINT_HZ,
        /// Station list.
        Stations = STATIONS,
        /// Bit rate of the test signal.
        GenRate = GEN_RATE,
        /// Carrier of the test signal.
        GenFcHz = GEN_FC_HZ,
        /// On from.
        GenFromS = GEN_FROM_S,
        /// Off at.
        GenToS = GEN_TO_S,
    }
}

/// What the stack does not do and what no public document confirms.
pub const NOTE_TEXT: &str = "A monitor: it measures the carrier and the bit rate from the squared signal and decodes \
nothing (the traffic is encrypted). No public document gives the bit rate of each station; the station hint names a \
listed carrier near the measured one, a hint to check, not a proof. Not covered: GMSK or other modulations, several \
stations at once (the strongest is read), keying shorter than two reports.";

/// The class of a measure.
fn class_of(m: &LineReport, rates: &[f64], tol: f64) -> (u8, String, Option<f64>, Option<f64>) {
    match (m.line1, m.line2) {
        (Some(f1), Some(f2)) => {
            let r = (f2 - f1).abs();
            let fc = (f1 + f2) / 4.0;
            let name = match rates.iter().find(|&&s| (r - s).abs() <= tol * s) {
                Some(s) => format!("MSK {s:.0} bit/s"),
                None => format!("MSK about {r:.0} bit/s"),
            };
            (2, name, Some(r), Some(fc))
        }
        (Some(f1), None) => (1, "carrier only".into(), None, Some(f1 / 2.0)),
        _ => (0, "off air or too weak".into(), None, None),
    }
}

/// A row as kept.
#[derive(Clone, Debug)]
struct Row {
    time: f64,
    kind: &'static str,
    text: String,
}

/// The VLF MSK layer.
pub struct VlfMsk {
    report_s: f64,
    rates_text: String,
    rates: Vec<f64>,
    tol: f64,
    hint_hz: f64,
    stations_text: String,
    stations: Vec<(f64, String)>,
    lines: SquaredLines,
    since: u64,
    dial: f64,
    last: Option<LineReport>,
    on: bool,
    pending: u32,
    class: Option<String>,
    last_rate: Option<f64>,
    rows: RowLog<Row>,
    transmitters: i64,
}

impl Default for VlfMsk {
    fn default() -> Self {
        Self::new()
    }
}

impl VlfMsk {
    /// A layer at the default settings.
    pub fn new() -> Self {
        let mut v = VlfMsk {
            report_s: 4.0,
            rates_text: "50,75,100,150,200,300".to_string(),
            rates: Vec::new(),
            tol: 0.08,
            hint_hz: 60.0,
            stations_text: String::new(),
            stations: Vec::new(),
            lines: SquaredLines::new(4096),
            since: 0,
            dial: 0.0,
            last: None,
            on: false,
            pending: 0,
            class: None,
            last_rate: None,
            rows: RowLog::default(),
            transmitters: 0,
        };
        v.parse();
        v
    }

    fn parse(&mut self) {
        self.rates = self
            .rates_text
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        self.stations = self
            .stations_text
            .split(';')
            .filter_map(|p| {
                let (f, n) = p.split_once('=')?;
                Some((f.trim().parse().ok()?, n.trim().to_string()))
            })
            .collect();
    }

    /// The station near the carrier `fc` from the tuned frequency.
    fn hint(&self, fc: f64) -> Option<&str> {
        let f = self.dial + fc;
        self.stations
            .iter()
            .filter(|(s, _)| (s - f).abs() <= self.hint_hz)
            .min_by(|a, b| (a.0 - f).abs().total_cmp(&(b.0 - f).abs()))
            .map(|(_, n)| n.as_str())
    }

    fn row(&mut self, kind: &'static str, m: &LineReport, cx: &mut Cx) {
        let (_, _, rate, fc) = class_of(m, &self.rates, self.tol);
        let hint = fc.and_then(|f| self.hint(f)).map(str::to_string);
        let mut text = match kind {
            "off" => "the signal has gone".to_string(),
            _ => format!(
                "{}, carrier {:+.2} Hz from the tuned frequency, {:.1} dB over the noise",
                rate.map_or("carrier only".to_string(), |r| format!("{r:.1} bit/s")),
                fc.unwrap_or(0.0),
                m.level_db
            ),
        };
        if let (Some(h), false) = (&hint, kind == "off") {
            text.push_str(&format!(", near the listed frequency of {h}"));
        }
        cx.events.push(VLF_TRANSMITTER.event(
            cx.time,
            cx.dial,
            &VlfTransmitterEvent {
                text: &text,
                kind,
                rate,
                carrier: fc.map(Hertz),
                level: Some(m.level_db),
                hint: hint.as_deref(),
            },
        ));
        if kind == "on" {
            self.transmitters += 1;
        }
        self.rows.push(Row {
            time: cx.time,
            kind,
            text,
        });
    }

    fn measure(&mut self, m: LineReport, cx: &mut Cx) {
        let signal = m.line1.is_some();
        if signal != self.on {
            self.pending += 1;
            if self.pending >= 2 {
                self.on = signal;
                self.pending = 0;
                self.row(if signal { "on" } else { "off" }, &m, cx);
                if !signal {
                    self.class = None;
                    self.last_rate = None;
                }
            }
        } else {
            self.pending = 0;
        }
        if self.on && signal {
            let (_, class, rate, _) = class_of(&m, &self.rates, self.tol);
            let moved = match (rate, self.last_rate) {
                (Some(r), Some(l)) => (r - l).abs() > self.tol * l,
                (None, None) => false,
                _ => true,
            };
            if self.class.as_ref().is_some_and(|c| *c != class) && moved {
                self.row("changed", &m, cx);
            }
            self.class = Some(class);
            self.last_rate = rate;
        }
        self.last = Some(m);
    }
}

impl Layer for VlfMsk {
    const NAME: &'static str = "hfdata.vlf_msk";
    const DECIM: usize = 6;
    const PASS_HZ: f64 = 1700.0;
    fn schema() -> Arc<Schema> {
        VlfMskRxSetting::schema()
    }
    fn declaration() -> ResultsDecl {
        ResultsDecl::new()
            .with_fields(VlfMskFields::SHAPE.fields)
            .with_table(&SENDER_ROWS)
            .event(&VLF_TRANSMITTER)
    }
    fn values(&self) -> Values {
        values_of([
            (VlfMskRxSetting::ReportS, Value::Duration(self.report_s)),
            (VlfMskRxSetting::Rates, Value::Text(self.rates_text.clone())),
            (VlfMskRxSetting::RateTolerance, Value::F64(100.0 * self.tol)),
            (VlfMskRxSetting::HintHz, Value::Hz(self.hint_hz)),
            (
                VlfMskRxSetting::Stations,
                Value::Text(self.stations_text.clone()),
            ),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match VlfMskRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            VlfMskRxSetting::ReportS => self.report_s = REPORT_S.read(id, value)?,
            VlfMskRxSetting::Rates => self.rates_text = RATES.read(id, value)?.to_string(),
            VlfMskRxSetting::RateTolerance => self.tol = RATE_TOLERANCE.read(id, value)? / 100.0,
            VlfMskRxSetting::HintHz => self.hint_hz = HINT_HZ.read(id, value)?,
            VlfMskRxSetting::Stations => self.stations_text = STATIONS.read(id, value)?.to_string(),
        }
        self.parse();
        Ok(())
    }
    fn tuned(&mut self, dial_hz: f64) {
        self.dial = dial_hz;
    }
    fn push(&mut self, x: &[Complex32], cx: &mut Cx) {
        let every = (self.report_s * RATE) as u64;
        for &v in x {
            self.lines.push(v);
            self.since += 1;
            if self.since >= every {
                self.since = 0;
                if let Some(m) = self.lines.report(RATE, 1200.0, 20.0, 420.0, 9.0) {
                    self.measure(m, cx);
                }
            }
        }
    }
    fn reset(&mut self) {
        self.lines.reset();
        self.since = 0;
        self.last = None;
        self.on = false;
        self.pending = 0;
        self.class = None;
        self.last_rate = None;
    }
    fn clear(&mut self) {
        self.rows.clear();
        self.transmitters = 0;
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset();
        if !kept.table(SENDER_ROWS.key) {
            self.rows.clear();
        }
        self.transmitters = 0;
    }
    fn report(&mut self, res: &mut Published, level_dbfs: f64) -> StatusLine {
        self.rows.report(
            res,
            |r| r.clear_rows(&SENDER_ROWS),
            |r, key, row| {
                SENDER_ROWS.put(
                    r,
                    key,
                    &SenderRow {
                        time: Time(row.time),
                        kind: "Transmitter",
                        id: row.kind,
                        text: &row.text,
                        snr: None,
                    },
                );
            },
        );
        let m = self.last.filter(|_| self.on);
        let (kind, class, rate, fc) = m
            .map_or((0, "off air or too weak".to_string(), None, None), |m| {
                class_of(&m, &self.rates, self.tol)
            });
        let spacing = match m {
            Some(LineReport {
                line1: Some(a),
                line2: Some(b),
                ..
            }) => Some((b - a).abs()),
            _ => None,
        };
        let hint = fc.and_then(|f| self.hint(f)).map(|h| format!("likely {h}"));
        res.put(&VlfMskFields {
            note: NOTE_TEXT,
            state: i64::from(kind),
            signal_class: &class,
            carrier_hz: fc.map(Hertz),
            bit_rate: rate,
            line_spacing_hz: spacing.map(Hertz),
            level_db: m.map(|m| (m.level_db * 10.0).round() / 10.0),
            hint: hint.as_deref(),
            transmitters: self.transmitters,
            level_dbfs,
        });
        StatusLine {
            state: kind,
            summary: format!("measures only: {class}"),
            snr_db: m.map(|m| m.level_db),
            offset_hz: fc,
        }
    }
}

/// The schema of the stack.
pub fn schema() -> Arc<Schema> {
    VlfMskSetting::schema()
}

/// The test signal: an MSK carrier of amplitude 1 at `fc_hz` from the dial, random bits at `bit_rate`, on from `from_s` to
/// `to_s` (0: to the end).
pub struct MskGen {
    rate: f64,
    cpm: Cpm,
    fc: f64,
    dial: f64,
    from: f64,
    to: f64,
    bits: Lcg,
    buf: Vec<Complex32>,
    at: usize,
    n: u64,
    bit_rate: f64,
}

impl MskGen {
    /// The signal at `rate` with the dial at `dial_hz` from the centre of the samples.
    pub fn new(
        rate: f64,
        bit_rate: f64,
        fc_hz: f64,
        from_s: f64,
        to_s: f64,
        dial_hz: f64,
        seed: u64,
    ) -> Result<Self, ConfigError> {
        let cpm = Cpm::msk(rate / bit_rate).map_err(|_| ConfigError::RateNotMultiple {
            got: rate,
            of: bit_rate,
        })?;
        Ok(MskGen {
            rate,
            cpm,
            fc: fc_hz,
            dial: dial_hz,
            from: from_s,
            to: to_s,
            bits: Lcg(seed ^ 0x75F),
            buf: Vec::new(),
            at: 0,
            n: 0,
            bit_rate,
        })
    }
}

impl Generator for MskGen {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo {
            symbol_rate_hz: Some(self.bit_rate),
            ..SignalInfo::new(2000.0)
        })
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        self.dial
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            while self.at >= self.buf.len() {
                self.buf.clear();
                self.at = 0;
                let levels: Vec<f32> = (0..64)
                    .map(|_| if self.bits.below(2) == 1 { 1.0 } else { -1.0 })
                    .collect();
                self.cpm.push(&levels, &mut self.buf);
            }
            let t = self.n as f64 / self.rate;
            let on = t >= self.from && (self.to <= 0.0 || t < self.to);
            let ph = ((self.fc + self.dial) * t).fract() * TAU;
            *o = if on {
                self.buf[self.at] * Complex32::from_polar(1.0, ph as f32)
            } else {
                Complex32::default()
            };
            self.at += 1;
            self.n += 1;
        }
        out.len()
    }
}

/// The test signal of the stack.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    Ok(Box::new(MskGen::new(
        cfg.rate_hz,
        s.get(&GEN_RATE),
        s.get(&GEN_FC_HZ),
        s.get(&GEN_FROM_S),
        s.get(&GEN_TO_S),
        test_dial(cfg.rate_hz),
        cfg.seed,
    )?))
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("vlf_msk"),
        "VLF MSK",
        Sender::new(VlfMsk::new()),
        cfg,
    )
}

/// The dial of the test signal. It is not a station frequency: the host gives the frequency.
pub const TEST_SIGNAL_HZ: f64 = 15_000.0;

/// The VLF MSK transmitter monitor.
pub static VLF_MSK: StackDescriptor = StackDescriptor {
    id: "vlf_msk",
    name: "VLF MSK transmitter monitor",
    group: "HF utility",
    blurb: "Carrier and bit rate of a VLF MSK station from the two lines of the squared signal; on, off and change events, a hint of the listed station. A monitor: the traffic is not decoded.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..crate::ms110::hf_tuning(TEST_SIGNAL_HZ)
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::VLF_MSK),
    benchmarks: Some(crate::benchmarks),
};
