//! The burst, ping and beacon stacks: FSK441, JT6M, ISCAT and PI4. Each reads the 8 kHz audio of the family front end without
//! its slow AGC and decodes from a sliding window of its own, with no slot clock. They share the decode rows, the stations, the decode and
//! spot events, the kept audio and the receiver graph of this module.

pub mod fsk441;
pub mod iscat;
pub mod jt6m;
pub mod pi4;

use crate::components::FrontComponent;
use crate::front::AUDIO_RATE;
use crate::typed::{ScatterFields, ScatterStationRow, SCATTER_STATIONS};
use sdrkit_core::interface::Time;
use sdrkit_core::publish::{field, keys, Published, TableDef};
use sdrkit_core::settings::DeclareSetting;
use sdrkit_core::{Clock, ConfigError, Event, LockState, Receiver, StackConfig, Status};
use sdrkit_graph::{Format, GraphBuilder, Kind, Outbox, Port, RateFrom, StackGraph};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The section of the test-signal settings.
pub const GEN_SECTION: &str = "Test signal";

/// The text of a test signal (empty: the stack's own test text).
pub const GEN_TEXT: sdrkit_core::Setting<field::Text> =
    sdrkit_core::Setting::new("gen_text", "Test text", "", "")
        .section(GEN_SECTION)
        .help("The message of the test signal. Empty: the stack's own test text.")
        .advanced();

/// The test text of the stacks of this module.
pub const TEST_TEXT: &str = "CQ K0TST FN20";

/// The decode rows kept.
pub const ROWS_CAP: usize = 1000;

/// The stations kept.
pub const STATIONS_CAP: usize = 1000;

/// A station of the station table: the newest decode that held its callsign and the count of such decodes.
#[derive(Clone, Copy, Debug)]
struct Heard {
    utc: f64,
    count: i64,
    snr_db: f64,
}

/// The callsigns of a decoded text, each once, in order. A 6-character word that is a Maidenhead locator is no
/// callsign.
pub fn callsigns(text: &str) -> Vec<&str> {
    let mut calls: Vec<&str> = Vec::new();
    for w in text.split_whitespace() {
        let locator = w.len() == 6 && sdrkit_util_geo::maidenhead::decode(w).is_ok();
        if !locator && sdrkit_util_text::callsign::is_callsign(w) && !calls.contains(&w) {
            calls.push(w);
        }
    }
    calls
}

/// A decode of a stack of this module, with the values `extra` that only this stack has.
#[derive(Clone, Debug)]
pub struct ScatterDecode<X> {
    /// Signal time of its start, seconds from the first sample.
    pub time_s: f64,
    /// The stack or variant ("FSK441", "ISCAT-B").
    pub label: String,
    /// The decoded text.
    pub text: String,
    /// The audio frequency the stack reports, Hz.
    pub hz: f64,
    /// SNR, dB (the stack says in 2500 Hz or not).
    pub snr_db: f64,
    /// The stack's own values.
    pub extra: X,
}

/// The values of a decode that one stack of this module adds, and how the stack shows a decode.
pub trait Shown: Clone + std::fmt::Debug + Send + Sized + 'static {
    /// The decode table of the stack.
    fn table() -> &'static TableDef;
    /// Writes the row `key` of the decode `d`, which began at `utc`.
    fn row(d: &ScatterDecode<Self>, utc: f64, key: &str, p: &mut Published);
    /// The decode event of `d`, which began at `utc` on the radio frequency `rf_hz`, with the first callsign of its
    /// text.
    fn event(d: &ScatterDecode<Self>, utc: f64, rf_hz: f64, callsign: Option<&str>) -> Event;
}

/// The decode rows, the events and the counters of a stack of this module.
pub struct Board<X: Shown> {
    rows: VecDeque<ScatterDecode<X>>,
    new_rows: usize,
    serial: u64,
    total: i64,
    stations: BTreeMap<String, Heard>,
    /// The stations whose rows are to be written at the next report.
    changed: BTreeSet<String>,
    /// The stations dropped for room, whose rows are to be removed at the next report.
    dropped: Vec<String>,
    clock: Option<Clock>,
    utc0: Option<f64>,
    dial_hz: f64,
    cleared: bool,
    /// The status shown.
    pub status: Status,
}

impl<X: Shown> Default for Board<X> {
    fn default() -> Self {
        Board::new()
    }
}

impl<X: Shown> Board<X> {
    /// An empty board.
    pub fn new() -> Self {
        Board {
            rows: VecDeque::new(),
            new_rows: 0,
            serial: 0,
            total: 0,
            stations: BTreeMap::new(),
            changed: BTreeSet::new(),
            dropped: Vec::new(),
            clock: None,
            utc0: None,
            dial_hz: 0.0,
            cleared: false,
            status: Status::default(),
        }
    }

    /// Takes the host's clock.
    pub fn attach(&mut self, clock: &Clock) {
        self.clock = Some(clock.clone());
    }

    /// Takes the dial frequency of the input.
    pub fn configure(&mut self, input: &Format) {
        self.dial_hz = input.tuned_hz;
    }

    /// Notes that the sample at signal time `t` came now: the UTC of signal time 0 is fixed at the first call.
    pub fn saw(&mut self, t: f64) {
        if self.utc0.is_none() {
            let now = self.clock.as_ref().map_or(0.0, Clock::now);
            self.utc0 = Some(now - t);
        }
    }

    /// The UTC of signal time `t` (seconds since 1970; signal time when no clock was given).
    pub fn utc(&self, t: f64) -> f64 {
        self.utc0.unwrap_or(0.0) + t
    }

    /// Decodes since the last Clear.
    pub fn total(&self) -> i64 {
        self.total
    }

    /// Adds a decode: its row, its event, the stations of its callsigns ([`callsigns`]) and a spot event for each
    /// callsign heard for the first time.
    pub fn decode(&mut self, out: &mut Outbox, d: ScatterDecode<X>) {
        let utc = self.utc(d.time_s);
        let rf = self.dial_hz + d.hz;
        let calls = callsigns(&d.text);
        out.messages(0)
            .push(X::event(&d, utc, rf, calls.first().copied()));
        for c in calls {
            let heard = self.stations.entry(c.to_owned()).or_insert(Heard {
                utc,
                count: 0,
                snr_db: d.snr_db,
            });
            let first = heard.count == 0;
            *heard = Heard {
                utc,
                count: heard.count + 1,
                snr_db: d.snr_db,
            };
            self.changed.insert(c.to_owned());
            if first {
                out.messages(0).push(crate::table::spot_event(
                    &d.label,
                    utc,
                    rf,
                    d.snr_db.round() as i32,
                    c,
                    "",
                ));
            }
        }
        self.keep_room();
        self.total += 1;
        self.rows.push_back(d);
        self.new_rows += 1;
        while self.rows.len() > ROWS_CAP {
            self.rows.pop_front();
        }
    }

    /// Drops the stations heard longest ago while there are more than [`STATIONS_CAP`].
    fn keep_room(&mut self) {
        while self.stations.len() > STATIONS_CAP {
            let Some(oldest) = self
                .stations
                .iter()
                .min_by(|a, b| a.1.utc.total_cmp(&b.1.utc))
                .map(|(k, _)| k.clone())
            else {
                return;
            };
            self.stations.remove(&oldest);
            self.changed.remove(&oldest);
            self.dropped.push(oldest);
        }
    }

    /// The decodes kept, oldest first.
    pub fn rows(&self) -> impl Iterator<Item = &ScatterDecode<X>> {
        self.rows.iter()
    }

    /// Writes the new rows, the count, the note and the status.
    pub fn report(&mut self, p: &mut Published, note: &str) {
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(X::table());
            p.clear_rows(SCATTER_STATIONS.def());
        }
        for c in self.dropped.drain(..) {
            p.remove_row(SCATTER_STATIONS.def(), &c);
        }
        for c in std::mem::take(&mut self.changed) {
            if let Some(h) = self.stations.get(&c) {
                SCATTER_STATIONS.put(
                    p,
                    &c,
                    &ScatterStationRow {
                        callsign: &c,
                        last_heard: Some(Time(h.utc)),
                        heard: Some(h.count),
                        snr_db: h.snr_db,
                    },
                );
            }
        }
        let start = self.rows.len().saturating_sub(self.new_rows);
        let first_serial = self.serial;
        for (k, d) in self.rows.iter().skip(start).enumerate() {
            let key = format!("{}", first_serial + k as u64);
            let utc = self.utc0.unwrap_or(0.0) + d.time_s;
            X::row(d, utc, &key, p);
        }
        self.serial += self.new_rows as u64;
        self.new_rows = 0;
        p.put(&ScatterFields {
            decodes_total: self.total,
            note,
            level_dbfs: None,
        });
        p.results_mut().set_status(&self.status);
    }

    /// Forgets the rows, the counters and the stations; keeps the clock. The tables are emptied at the next report.
    pub fn clear(&mut self) {
        self.rows.clear();
        self.new_rows = 0;
        self.total = 0;
        self.stations.clear();
        self.changed.clear();
        self.dropped.clear();
        self.cleared = true;
    }

    /// Sets the lock state and the summary.
    pub fn set_state(&mut self, state: LockState, summary: std::fmt::Arguments<'_>) {
        use std::fmt::Write;
        self.status.lock_state = state;
        self.status.summary.clear();
        let _ = self.status.summary.write_fmt(summary);
    }
}

/// The SNR in 2500 Hz of a tone whose power is `ratio` times the mean power of the other tones of the same symbols, with tones
/// `spacing_hz` apart: 10·log10(ratio − 1) + 10·log10(spacing / 2500), at least −35 dB.
pub fn snr_2500(ratio: f64, spacing_hz: f64) -> f64 {
    let r = (ratio - 1.0).max(1e-4);
    (10.0 * r.log10() + 10.0 * (spacing_hz / 2500.0).log10()).max(-35.0)
}

/// The tune offset of the stacks of this module, Hz: the radio is tuned this far above the dial.
pub const TUNE_OFFSET_HZ: f64 = 20e3;

/// Where a test signal at `rate` puts the dial, Hz from the centre of its samples: [`TUNE_OFFSET_HZ`] below the centre, where a
/// receiver of the stack looks; at the centre when the rate is too low to hold the offset and the audio band.
pub fn dial_in_samples(rate: f64) -> f64 {
    if rate / 2.0 >= TUNE_OFFSET_HZ + 4000.0 {
        -TUNE_OFFSET_HZ
    } else {
        0.0
    }
}

/// The audio kept by a receiver: the newest samples, with the index of the first since the start of the stream.
#[derive(Clone, Debug, Default)]
pub struct Kept {
    buf: Vec<f32>,
    first: u64,
}

impl Kept {
    /// Appends samples (a sample that is not finite becomes 0).
    pub fn push(&mut self, x: &[f32]) {
        self.buf
            .extend(x.iter().map(|&v| if v.is_finite() { v } else { 0.0 }));
    }
    /// The index of the first sample kept.
    pub fn first(&self) -> u64 {
        self.first
    }
    /// The index after the newest sample.
    pub fn end(&self) -> u64 {
        self.first + self.buf.len() as u64
    }
    /// Drops the samples before index `from`.
    pub fn drop_before(&mut self, from: u64) {
        if from <= self.first {
            return;
        }
        let n = ((from - self.first) as usize).min(self.buf.len());
        self.buf.drain(..n);
        self.first += n as u64;
    }
    /// The samples from index `a` to `b`, cut to what is kept.
    pub fn span(&self, a: u64, b: u64) -> &[f32] {
        let a = a.clamp(self.first, self.end());
        let b = b.clamp(a, self.end());
        &self.buf[(a - self.first) as usize..(b - self.first) as usize]
    }
    /// Every sample kept.
    pub fn all(&self) -> &[f32] {
        &self.buf
    }
    /// Forgets everything and starts again at index `first`.
    pub fn reset(&mut self, first: u64) {
        self.buf.clear();
        self.first = first;
    }
}

/// The outputs of a decoder of this module: the events.
pub fn outputs() -> Vec<Port> {
    vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
}

/// The formats of [`outputs`].
pub fn formats() -> Vec<Format> {
    vec![Format::new(Kind::Messages, 0.0)]
}

/// Checks that the input is the 8 kHz audio of the front end.
///
/// # Errors
///
/// [`ConfigError::RateNotMultiple`] for another rate.
pub fn check_rate(input: &Format) -> Result<(), ConfigError> {
    if (input.rate - AUDIO_RATE).abs() > 1e-6 {
        return Err(ConfigError::RateNotMultiple {
            got: input.rate,
            of: AUDIO_RATE,
        });
    }
    Ok(())
}

/// The settings of a stack: its own, the front end's and its test-signal settings.
pub fn settings(
    own: &[&'static dyn DeclareSetting],
    generator: &[&'static dyn DeclareSetting],
) -> Vec<&'static dyn DeclareSetting> {
    let mut all: Vec<&'static dyn DeclareSetting> = own.to_vec();
    all.push(&crate::components::DEMOD);
    all.push(&crate::components::LISTEN);
    all.extend_from_slice(generator);
    all
}

/// The receiver of a stack of this module: the front end without its AGC, then `dec`. The decoder's settings keep their keys;
/// `demod` and `listen` go to the front end.
///
/// # Errors
///
/// [`ConfigError`] for a value of `cfg` that a component refuses.
pub fn receiver<D>(
    id: &'static str,
    dec: D,
    cfg: &StackConfig,
) -> Result<Box<dyn Receiver>, ConfigError>
where
    D: sdrkit_graph::Decoder<Input = f32>,
{
    let own = dec.settings();
    let b = GraphBuilder::new()
        .catalog(&sdrkit_graph::Catalog::of(cfg)?)
        .add("front", FrontComponent::plain())
        .add("dec", sdrkit_graph::DecoderComponent::new(dec))
        .input("front.in")
        .connect("front.audio", "dec.in")
        .output("dec.events")
        .output("front.listen")
        .status_from("dec")
        .clock(0.25)
        .reconfigure_on_retune();
    let mut g = StackGraph::of(crate::typed::shape(id), b.build()?)
        .min_rate(
            crate::front::MIN_RATE,
            "Weak-signal stacks need a sample rate of at least 8 ksps",
        )
        .route("demod", &["front.demod"])?
        .route("listen", &["front.listen"])?;
    for s in own.settings() {
        let k = &s.field.key;
        g = g.route(k, &[&format!("dec.{k}")])?;
    }
    Ok(Box::new(
        g.command("clear", sdrkit_graph::CommandTarget::every("clear"))
            .with_settings(&cfg.settings)
            .build()?,
    ))
}

/// The next power of two at or above `n`.
pub fn next_pow2(n: usize) -> usize {
    n.max(1).next_power_of_two()
}

/// The median of `v` (0 for none). Reorders `v`.
pub fn median(v: &mut [f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let m = v.len() / 2;
    let (_, x, _) = v.select_nth_unstable_by(m, f32::total_cmp);
    *x
}

/// The texts of the decode events among `events`.
pub fn decodes_of(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e.kind() == "wsjt.decode")
        .filter_map(|e| e.get(&keys::TEXT).map(str::to_owned))
        .collect()
}
