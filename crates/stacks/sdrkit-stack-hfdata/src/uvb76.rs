//! The UVB-76 buzzer monitor: it times the buzz train of the AM station (about 25 buzzes a minute, 1.2 s each), finds a
//! break of the train and a long transmission (a voice message is possible). It detects and measures only: no voice is decoded
//! and no message is interpreted.
//!
//! Standard: no issuing body publishes the signal. The frequency, the buzz length (1.2 s, 0.8 s before November 2010), the
//! pause and the rate of 20 to 34 buzzes a minute are those of the Wikipedia article "UVB-76" and the Buzzer Primer of
//! priyom.org; the stack measures what it hears.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [uvb76: low pass to 12 kHz, the carrier-line gate, the envelope less the carrier, two low passes
//!        at 1.5 kHz, 10 ms levels against a floor, the buzzes, the train, the break, the long transmission] -> rows
//! ```

use crate::receiver::GEN_SECTION;
use crate::senders::{Cx, Layer, RowLog, Sender, StatusLine};
use crate::serial::gen::test_dial;
use crate::typed::{SenderRow, Uvb76Event, Uvb76Fields, SENDER_ROWS, UVB76_EVENT};
use sdrkit_algo_dsp::carrierline::CarrierLine;
use sdrkit_core::interface::{Fields as _, Seconds, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, Hz, LockProfile, Receiver, ResultsDecl,
    Schema, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StackDescriptor,
    Value, Values,
};
use sdrkit_graph::{values_of, Kept};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::sync::Arc;

/// The channel rate.
pub const RATE: f64 = 12_000.0;
/// Samples of a level block (10 ms).
const BLOCK: usize = 120;
/// Periods and lengths kept.
const KEEP: usize = 12;

/// The shortest buzz.
pub const MIN_BUZZ_S: Setting<field::Duration> =
    Setting::new("min_buzz_s", "Shortest buzz", "s", 0.15)
        .range(0.05, 1.0, 0.01)
        .help("A shorter rise of the modulation is a click and is ignored.");
/// The longest buzz.
pub const LONG_TX_S: Setting<field::Duration> = Setting::new("long_tx_s", "Longest buzz", "s", 4.0)
    .range(1.0, 20.0, 0.1)
    .help("Longer modulation is a long transmission (a voice message is possible).");
/// The carrier line share.
pub const CARRIER_SHARE: Setting<field::F64> =
    Setting::new("carrier_share", "Carrier line share", "", 0.012)
        .range(0.002, 0.1, 0.001)
        .help("The share of the channel power in the strongest five bins that shows a carrier; without a carrier nothing is timed.");
/// The period of the test signal.
pub const GEN_PERIOD_S: Setting<field::Duration> =
    Setting::new("gen_period_s", "Buzz period", "s", 2.4)
        .range(0.5, 15.0, 0.01)
        .section(GEN_SECTION)
        .advanced();
/// The buzz length of the test signal.
pub const GEN_BUZZ_S: Setting<field::Duration> =
    Setting::new("gen_buzz_s", "Buzz length", "s", 1.2)
        .range(0.1, 10.0, 0.01)
        .section(GEN_SECTION)
        .advanced();
/// The modulation depth of the test signal.
pub const GEN_DEPTH: Setting<field::F64> = Setting::new("gen_depth", "Modulation depth", "", 0.6)
    .range(0.0, 1.0, 0.01)
    .section(GEN_SECTION)
    .advanced();
/// The start of a pause of the train; 0 for none.
pub const GEN_BREAK_AT_S: Setting<field::Duration> =
    Setting::new("gen_break_at_s", "Pause at", "s", 0.0)
        .range(0.0, 3600.0, 0.1)
        .help("0: no pause.")
        .section(GEN_SECTION)
        .advanced();
/// The length of the pause.
pub const GEN_BREAK_S: Setting<field::Duration> = Setting::new("gen_break_s", "Pause", "s", 0.0)
    .range(0.0, 600.0, 0.1)
    .section(GEN_SECTION)
    .advanced();
/// The start of a long transmission; 0 for none.
pub const GEN_VOICE_AT_S: Setting<field::Duration> =
    Setting::new("gen_voice_at_s", "Voice at", "s", 0.0)
        .range(0.0, 3600.0, 0.1)
        .help("0: no long transmission.")
        .section(GEN_SECTION)
        .advanced();
/// The length of the long transmission.
pub const GEN_VOICE_S: Setting<field::Duration> =
    Setting::new("gen_voice_s", "Voice length", "s", 0.0)
        .range(0.0, 600.0, 0.1)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the UVB-76 receiver.
    pub enum Uvb76RxSetting {
        /// Shortest buzz.
        MinBuzzS = MIN_BUZZ_S,
        /// Longest buzz.
        LongTxS = LONG_TX_S,
        /// Carrier line share.
        CarrierShare = CARRIER_SHARE,
    }
}

sdrkit_core::settings! {
    /// The settings of the UVB-76 stack.
    pub enum Uvb76Setting {
        /// Shortest buzz.
        MinBuzzS = MIN_BUZZ_S,
        /// Longest buzz.
        LongTxS = LONG_TX_S,
        /// Carrier line share.
        CarrierShare = CARRIER_SHARE,
        /// Buzz period.
        GenPeriodS = GEN_PERIOD_S,
        /// Buzz length.
        GenBuzzS = GEN_BUZZ_S,
        /// Modulation depth.
        GenDepth = GEN_DEPTH,
        /// Pause at.
        GenBreakAtS = GEN_BREAK_AT_S,
        /// Pause.
        GenBreakS = GEN_BREAK_S,
        /// Voice at.
        GenVoiceAtS = GEN_VOICE_AT_S,
        /// Voice length.
        GenVoiceS = GEN_VOICE_S,
    }
}

/// What no public document confirms and what the stack does not do.
pub const NOTE_TEXT: &str = "A monitor: it times the buzz train and decodes nothing; a long transmission may be a voice \
message, which is not decoded or interpreted. The rate of about 25 buzzes a minute is a description, not a parameter. The \
thresholds (9 dB over the floor to start a buzz, 6 dB to hold it, -45 dB of the carrier at least) are the stack's. A dip \
under the hold threshold shorter than the shortest buzz does not end a buzz.";

/// The upper median of `v`.
fn median(v: &VecDeque<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let mut s: Vec<f64> = v.iter().copied().collect();
    s.sort_by(f64::total_cmp);
    Some(s[s.len() / 2])
}

/// A row as kept.
#[derive(Clone, Debug)]
struct Row {
    time: f64,
    kind: &'static str,
    value: String,
    text: String,
}

/// The UVB-76 layer.
pub struct Uvb76 {
    min_buzz: f64,
    long_tx: f64,
    share: f64,
    gate: CarrierLine,
    carrier: f64,
    lp: [f64; 2],
    acc: f64,
    count: usize,
    n: u64,
    smoothed: Option<f64>,
    floor: Option<f64>,
    on_since: Option<f64>,
    rise: Option<f64>,
    last_over: Option<f64>,
    last_start: Option<f64>,
    last_end: Option<f64>,
    periods: VecDeque<f64>,
    lengths: VecDeque<f64>,
    locked: bool,
    break_logged: bool,
    rows: RowLog<Row>,
    buzzes: i64,
    breaks: i64,
    long: i64,
}

impl Default for Uvb76 {
    fn default() -> Self {
        Self::new()
    }
}

impl Uvb76 {
    /// A layer at the default settings.
    pub fn new() -> Self {
        Uvb76 {
            min_buzz: 0.15,
            long_tx: 4.0,
            share: 0.012,
            gate: CarrierLine::new(CarrierLine::block_len(RATE, 0.4, 256, 32_768), 5),
            carrier: 0.0,
            lp: [0.0; 2],
            acc: 0.0,
            count: 0,
            n: 0,
            smoothed: None,
            floor: None,
            on_since: None,
            rise: None,
            last_over: None,
            last_start: None,
            last_end: None,
            periods: VecDeque::new(),
            lengths: VecDeque::new(),
            locked: false,
            break_logged: false,
            rows: RowLog::default(),
            buzzes: 0,
            breaks: 0,
            long: 0,
        }
    }

    fn row(&mut self, time: f64, kind: &'static str, value: String, text: String, cx: &mut Cx) {
        cx.events.push(UVB76_EVENT.event(
            cx.time,
            cx.dial,
            &Uvb76Event {
                text: &text,
                kind,
                value: &value,
            },
        ));
        self.rows.push(Row {
            time,
            kind,
            value,
            text,
        });
    }

    fn buzz(&mut self, start: f64, end: f64, cx: &mut Cx) {
        let len = end - start;
        if len < self.min_buzz {
            return;
        }
        let med_len = median(&self.lengths);
        if len > self.long_tx && (!self.locked || med_len.is_none_or(|m| len > 3.0 * m)) {
            self.long += 1;
            self.locked = false;
            self.periods.clear();
            self.row(
                cx.time,
                "Long transmission",
                format!("{len:.1} s"),
                "not a buzz (a voice message is possible; not decoded)".into(),
                cx,
            );
            self.last_start = None;
            self.last_end = Some(end);
            return;
        }
        self.buzzes += 1;
        self.break_logged = false;
        if let Some(s0) = self.last_start {
            let p = start - s0;
            if (0.5..=15.0).contains(&p) {
                self.periods.push_back(p);
                if self.periods.len() > KEEP {
                    self.periods.pop_front();
                }
            }
        }
        self.lengths.push_back(len);
        if self.lengths.len() > KEEP {
            self.lengths.pop_front();
        }
        self.last_start = Some(start);
        self.last_end = Some(end);
        if !self.locked && self.periods.len() >= 4 {
            let m = median(&self.periods).unwrap_or(0.0);
            let dev: VecDeque<f64> = self.periods.iter().map(|p| (p - m).abs()).collect();
            let spread = median(&dev).unwrap_or(f64::INFINITY);
            if spread < 0.15 * m {
                self.locked = true;
                let l = median(&self.lengths).unwrap_or(0.0);
                self.row(
                    cx.time,
                    "Buzz train",
                    format!("{:.1} /min", 60.0 / m),
                    format!("period {m:.2} s, buzz {l:.2} s"),
                    cx,
                );
            }
        }
    }

    /// One block of 10 ms. The smoothed level over the start threshold starts a buzz. The edges are timed by the blocks
    /// whose own level is over the hold threshold, at their middle: the first after a stretch under it and the last before
    /// one. A stretch under it shorter than the shortest buzz is a dip inside the buzz (the pause of the train is about
    /// 1 s); a longer one ends the buzz at the last block over.
    fn block(&mut self, cx: &mut Cx) {
        let t = self.n as f64 / RATE;
        let p = self.acc / self.count as f64;
        self.acc = 0.0;
        self.count = 0;
        if !self.gate.present(self.share) || self.carrier <= 0.0 {
            self.on_since = None;
            self.rise = None;
            self.last_over = None;
            return;
        }
        let level = 10.0 * (p / (self.carrier * self.carrier)).max(1e-15).log10();
        let s = match self.smoothed {
            None => level,
            Some(s) => s + 0.5 * (level - s),
        };
        self.smoothed = Some(s);
        let on = self.on_since.is_some();
        let floor = match self.floor {
            None => s,
            Some(f) if s < f => f + 0.5 * (s - f),
            Some(f) if !on => f + 0.0005 * (s - f),
            Some(f) => f,
        };
        self.floor = Some(floor);
        let base = floor.max(-45.0);
        let mid = t - 0.5 * BLOCK as f64 / RATE;
        if self.last_over.is_some_and(|e| mid - e >= self.min_buzz) {
            if let Some(start) = self.on_since.take() {
                let end = self.last_over.unwrap_or(mid);
                self.buzz(start, end, cx);
            }
            self.rise = None;
            self.last_over = None;
        }
        if level > base + 6.0 {
            self.rise.get_or_insert(mid);
            self.last_over = Some(mid);
        }
        if self.on_since.is_none() && s > base + 9.0 {
            self.on_since = Some(self.rise.unwrap_or(mid));
        }
        if self.locked && self.on_since.is_none() && !self.break_logged {
            if let (Some(end), Some(m)) = (self.last_end, median(&self.periods)) {
                if t - end > 3.0 * m {
                    self.break_logged = true;
                    self.breaks += 1;
                    self.row(
                        cx.time,
                        "Train break",
                        format!("{:.0} s", t - end),
                        "the buzz train stopped".into(),
                        cx,
                    );
                }
            }
        }
    }
}

impl Layer for Uvb76 {
    const NAME: &'static str = "hfdata.uvb76";
    const DECIM: usize = 2;
    const PASS_HZ: f64 = 4500.0;
    fn schema() -> Arc<Schema> {
        Uvb76RxSetting::schema()
    }
    fn declaration() -> ResultsDecl {
        ResultsDecl::new()
            .with_fields(Uvb76Fields::SHAPE.fields)
            .with_table(&SENDER_ROWS)
            .event(&UVB76_EVENT)
    }
    fn values(&self) -> Values {
        values_of([
            (Uvb76RxSetting::MinBuzzS, Value::Duration(self.min_buzz)),
            (Uvb76RxSetting::LongTxS, Value::Duration(self.long_tx)),
            (Uvb76RxSetting::CarrierShare, Value::F64(self.share)),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match Uvb76RxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Uvb76RxSetting::MinBuzzS => self.min_buzz = MIN_BUZZ_S.read(id, value)?,
            Uvb76RxSetting::LongTxS => self.long_tx = LONG_TX_S.read(id, value)?,
            Uvb76RxSetting::CarrierShare => self.share = CARRIER_SHARE.read(id, value)?,
        }
        Ok(())
    }
    fn push(&mut self, x: &[Complex32], cx: &mut Cx) {
        self.gate.push(x);
        let a = 1.0 - (-TAU * 1500.0 / RATE).exp();
        for &v in x {
            let e = f64::from(v.norm());
            self.carrier += (e - self.carrier) / RATE;
            let audio = e - self.carrier;
            self.lp[0] += a * (audio - self.lp[0]);
            self.lp[1] += a * (self.lp[0] - self.lp[1]);
            self.acc += self.lp[1] * self.lp[1];
            self.count += 1;
            self.n += 1;
            if self.count == BLOCK {
                self.block(cx);
            }
        }
    }
    fn reset(&mut self) {
        let rows = std::mem::take(&mut self.rows);
        let (b, br, l) = (self.buzzes, self.breaks, self.long);
        *self = Uvb76 {
            min_buzz: self.min_buzz,
            long_tx: self.long_tx,
            share: self.share,
            ..Uvb76::new()
        };
        self.rows = rows;
        (self.buzzes, self.breaks, self.long) = (b, br, l);
    }
    fn clear(&mut self) {
        self.rows.clear();
        self.buzzes = 0;
        self.breaks = 0;
        self.long = 0;
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset();
        if !kept.table(SENDER_ROWS.key) {
            self.rows.clear();
        }
        self.buzzes = 0;
        self.breaks = 0;
        self.long = 0;
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
                        kind: row.kind,
                        id: &row.value,
                        text: &row.text,
                        snr: None,
                    },
                );
            },
        );
        let m = median(&self.periods).filter(|_| self.locked);
        let l = median(&self.lengths).filter(|_| self.locked);
        let spread = m.map(|m| {
            let dev: VecDeque<f64> = self.periods.iter().map(|p| (p - m).abs()).collect();
            1000.0 * median(&dev).unwrap_or(0.0)
        });
        let carrier = self.gate.present(self.share);
        let state: u8 = match (carrier, self.locked) {
            (true, true) => 2,
            (true, false) => 1,
            _ => 0,
        };
        res.put(&Uvb76Fields {
            note: NOTE_TEXT,
            state: i64::from(state),
            carrier,
            buzzes: self.buzzes,
            rate_per_min: m.map(|m| (600.0 / m).round() / 10.0),
            buzz_s: l.map(Seconds),
            period_s: m.map(Seconds),
            spread_ms: spread,
            breaks: self.breaks,
            long_transmissions: self.long,
            level_dbfs,
        });
        let summary = match (m, l) {
            (Some(m), Some(l)) => format!("buzz train {:.1} per minute, buzz {l:.2} s", 60.0 / m),
            _ if self.buzzes > 0 => format!("{} buzzes heard", self.buzzes),
            _ => "no buzz heard".to_string(),
        };
        StatusLine {
            state,
            summary: format!("measures only: {summary}"),
            snr_db: None,
            offset_hz: None,
        }
    }
}

/// The schema of the stack.
pub fn schema() -> Arc<Schema> {
    Uvb76Setting::schema()
}

/// The test signal: an AM carrier of amplitude 1 at the dial, modulated `1 + depth a(t)`: a 30 Hz sawtooth from -0.7 to 0.7
/// during a buzz, a voice stand-in during a long transmission.
pub struct Uvb76Gen {
    rate: f64,
    n: u64,
    period: f64,
    buzz: f64,
    depth: f64,
    pause: (f64, f64),
    voice: (f64, f64),
    dial: f64,
}

impl Uvb76Gen {
    /// The test signal at `rate` with the settings of the stack, the dial at `dial_hz` from the centre of the samples.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rate: f64,
        period: f64,
        buzz: f64,
        depth: f64,
        pause: (f64, f64),
        voice: (f64, f64),
        dial_hz: f64,
    ) -> Self {
        Uvb76Gen {
            rate,
            n: 0,
            period,
            buzz,
            depth,
            pause,
            voice,
            dial: dial_hz,
        }
    }

    /// The modulation at time `t`.
    fn a(&self, t: f64) -> f64 {
        let within = |w: (f64, f64)| w.0 > 0.0 && t >= w.0 && t < w.0 + w.1;
        if within(self.voice) {
            return 0.5 * (TAU * 300.0 * t).sin() * (0.6 + 0.4 * (TAU * 3.1 * t).sin())
                + 0.3 * (TAU * 1100.0 * t).sin() * (TAU * 2.3 * t).sin();
        }
        if within(self.pause) || t.rem_euclid(self.period) >= self.buzz {
            return 0.0;
        }
        let saw = (30.0 * t).fract();
        1.4 * saw - 0.7
    }
}

impl Generator for Uvb76Gen {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(4500.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        self.dial
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let m = 1.0 + self.depth * self.a(t);
            let ph = (self.dial * self.n as f64 / self.rate).fract() * TAU;
            *o = Complex32::from_polar(m as f32, ph as f32);
            self.n += 1;
        }
        out.len()
    }
}

/// The test signal of the stack.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    Ok(Box::new(Uvb76Gen::new(
        cfg.rate_hz,
        s.get(&GEN_PERIOD_S),
        s.get(&GEN_BUZZ_S),
        s.get(&GEN_DEPTH),
        (s.get(&GEN_BREAK_AT_S), s.get(&GEN_BREAK_S)),
        (s.get(&GEN_VOICE_AT_S), s.get(&GEN_VOICE_S)),
        test_dial(cfg.rate_hz),
    )))
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("uvb76"),
        "UVB-76",
        Sender::new(Uvb76::new()),
        cfg,
    )
}

/// The dial of the test signal. It is not a station frequency: the host gives the frequency.
pub const TEST_SIGNAL_HZ: f64 = 6.5e6;

/// The UVB-76 buzzer monitor.
pub static UVB76: StackDescriptor = StackDescriptor {
    id: "uvb76",
    name: "UVB-76 buzzer monitor",
    group: "HF utility",
    blurb: "Times the buzz train of the UVB-76 AM station: buzz rate, length, period and spread; train breaks and long transmissions. A monitor: no voice is decoded.",
    tuning: crate::ms110::hf_tuning(TEST_SIGNAL_HZ),
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::UVB76),
    benchmarks: Some(crate::benchmarks),
};
