//! EM4100 LF tags (`rfid_lf`): the 40-bit identity (8-bit version, 32-bit number) that an EM4100 tag sends by load modulation
//! of a reader carrier of 100 to 150 kHz (EM Microelectronic-Marin, EM4100 data sheet). The Manchester and Biphase versions are
//! read at RF/64 and RF/32; the PSK version is not. A tag is shown only after its header, its stop bit, its ten row parities
//! and its four column parities hold; a frame with a header and a stop bit that fails a parity is counted as refused.
//!
//! The graph: one decoder component (`reader`). It mixes out the dial offset, decimates to about 32 kHz, slices the envelope
//! with the shared tracking slicer and reads half-bits with the shared interval reader, one reader for each bit rate.

pub mod frame;
pub mod gen;

use crate::family::{db20, Front, InputLevel, Log, Tally, REPORT_S};
use crate::typed::{
    RfidBitRate, RfidBitRates, RfidCoding, RfidCodings, RfidLfFields, RfidLfTagRow, RfidTagEvent,
    TagsStatusFields, RFID_LF_TAGS, RFID_TAG,
};
use gen::RfidLfGenerator;
use sdrkit_algo_modem::envtrack::{Change, EdgeTime, Gate, TrackConfig, TrackingSlicer};
use sdrkit_algo_modem::halfbit::{IntervalWindows, ManchesterHalves};
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::interface::{ReadFields as _, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::value::Choice as _;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, Receiver, Record,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, Status, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kept, Kind, Outbox, Port, RateFrom, StackGraph,
};
use std::collections::HashSet;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "rfid_lf";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 24e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 32e3;
/// The pass edge of the channel before the envelope, Hz.
pub const PASS_HZ: f64 = 5e3;
/// The carrier taken when the dial lies outside the operating range of the data sheet, Hz.
pub const NOMINAL_CARRIER_HZ: f64 = 125e3;
/// The operating range of the data sheet, Hz: a dial inside it is the carrier that the bit times are counted in.
pub const OPERATING_HZ: (f64, f64) = (100e3, 150e3);
/// Rows kept in the tag table.
pub const ROWS_KEPT: usize = 500;
/// The half-bits that one frame takes.
const WINDOW: usize = 2 * frame::FRAME_BITS;
/// The longest list of half-bits; when longer, the oldest `LIST_DROP` go.
const LIST_MAX: usize = 400;
const LIST_DROP: usize = 200;
/// A frame that ends fewer half-bits than this after the last one of the same reader is the same frame again.
const SAME_FRAME_HALVES: u64 = 100;

/// What the stack reads and what no public document confirms.
pub const NOTE_TEXT: &str = "Reads the EM4100 Manchester and Biphase versions at RF/64 and RF/32 of the reader carrier \
(EM4100 data sheet); the PSK version and other LF tag families are not read. A tag is shown only after the header, the stop \
bit, every row parity and every column parity hold. Not confirmed by a public document: the bit order within a row (the \
first bit sent is taken as the most significant), the two 16-bit halves of the number, the name EM4102 and the modulation \
depth. The receive windows, the tracking slicer and the 3 s hold are choices of this stack. Not tried on a real signal.";

/// The bit rates read.
pub const BIT_RATES: Setting<field::Choice<RfidBitRates>> =
    Setting::choice("bit_rates", "Bit rates read", RfidBitRates::Both);
/// The codes read.
pub const CODES: Setting<field::Choice<RfidCodings>> =
    Setting::choice("codes", "Codes read", RfidCodings::Both);
/// The hold time of a tag.
pub const SAME_TAG_S: Setting<field::F64> =
    Setting::new("same_tag_s", "Same tag held for", "s", 3.0)
        .range(0.5, 30.0, 0.0)
        .help("A read of the same tag within this time of its last read adds to its row.");
/// The swing gate.
pub const GATE_SIGMAS: Setting<field::F64> = Setting::new("gate_sigmas", "Swing gate", "sigma", 5.0)
    .range(3.0, 10.0, 0.0)
    .help("Level changes are decided only while the swing of the envelope is this many deviations of its noise.");

sdrkit_core::settings! {
    /// The settings of the reader component.
    pub enum ReaderSetting {
        /// The bit rates.
        BitRates = BIT_RATES,
        /// The codes.
        Codes = CODES,
        /// The hold time.
        SameTagS = SAME_TAG_S,
        /// The gate.
        GateSigmas = GATE_SIGMAS,
    }
}

sdrkit_core::settings! {
    /// The settings of the `rfid_lf` stack.
    pub enum RfidLfSetting {
        /// The bit rates.
        BitRates = BIT_RATES,
        /// The codes.
        Codes = CODES,
        /// The hold time.
        SameTagS = SAME_TAG_S,
        /// The gate.
        GateSigmas = GATE_SIGMAS,
        /// The tags of the test signal.
        GenId = gen::GEN_ID,
        /// The time of each tag.
        GenIdS = gen::GEN_ID_S,
        /// The depth.
        GenDepth = gen::GEN_DEPTH,
        /// The bit rate.
        GenRfDiv = gen::GEN_RF_DIV,
        /// The code.
        GenCoding = gen::GEN_CODING,
        /// The tag clock.
        GenBitClockPpm = gen::GEN_BIT_CLOCK_PPM,
        /// The start.
        GenStartS = gen::GEN_START_S,
        /// The polarity.
        GenInvert = gen::GEN_INVERT,
    }
}

/// The `reset` command: the trackers, the clocks and any frame in progress start again; the table and the counts stay.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Drops the trackers, the bit clocks and the half-bits so far; the table stays.");
static COMMANDS: [&Command; 1] = [&RESET];

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The carrier that the bit times count: the dial inside the operating range of the data sheet, else 125 kHz.
fn carrier_of(dial_hz: f64) -> f64 {
    if (OPERATING_HZ.0..=OPERATING_HZ.1).contains(&dial_hz) {
        dial_hz
    } else {
        NOMINAL_CARRIER_HZ
    }
}

/// The rules of the slicer.
fn slicer_config(gate_sigmas: f64) -> TrackConfig {
    TrackConfig {
        smoothing: 0.5,
        attack: 1.0,
        peak_decay_s: 0.06,
        floor_decay_s: 0.06,
        gate: Gate::Sigma(gate_sigmas),
        shut_level: Some(0),
        hysteresis: 0.22,
        edge: EdgeTime::Interpolated { within: 6.0 },
        start_s: 0.02,
    }
}

/// A frame found in a window of half-bits.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Found {
    Tag(u64, RfidCoding),
    Refused(RfidCoding),
}

/// Tests the window `w` of 128 half-bits for a frame in the codes `codes`.
fn search(w: &[u8], codes: RfidCodings) -> Option<Found> {
    let mut bits = [0u8; frame::FRAME_BITS];
    let mut refused = None;
    if codes != RfidCodings::Biphase && w.as_chunks::<2>().0.iter().all(|p| p[0] != p[1]) {
        for pol in [0u8, 1] {
            // The header first, so most windows end here.
            if w[..2 * frame::HEADER_BITS]
                .as_chunks::<2>()
                .0
                .iter()
                .any(|p| p[0] ^ pol != 1)
            {
                continue;
            }
            for (b, p) in bits.iter_mut().zip(w.as_chunks::<2>().0) {
                *b = p[0] ^ pol;
            }
            match frame::decode(&bits) {
                Ok(d) => return Some(Found::Tag(d, RfidCoding::Manchester)),
                Err(frame::Refused::Parity) => refused = Some(RfidCoding::Manchester),
                Err(frame::Refused::NoFrame) => {}
            }
        }
    }
    if codes != RfidCodings::Manchester && (1..frame::FRAME_BITS).all(|i| w[2 * i - 1] != w[2 * i])
    {
        for (b, p) in bits.iter_mut().zip(w.as_chunks::<2>().0) {
            *b = u8::from(p[0] == p[1]);
        }
        match frame::decode(&bits) {
            Ok(d) => return Some(Found::Tag(d, RfidCoding::Biphase)),
            Err(frame::Refused::Parity) => refused = refused.or(Some(RfidCoding::Biphase)),
            Err(frame::Refused::NoFrame) => {}
        }
    }
    refused.map(Found::Refused)
}

/// The half-bits of one bit rate.
#[derive(Clone, Debug)]
struct Reader {
    rate: RfidBitRate,
    halves: ManchesterHalves,
    list: Vec<u8>,
    count: u64,
    last_frame: Option<u64>,
    last_refused: Option<u64>,
    /// The code and the place (half-bit count modulo a frame) of the frames of the last read.
    place: Option<(RfidCoding, u64)>,
}

impl Reader {
    fn new(rate: RfidBitRate, carrier_hz: f64) -> Reader {
        let div = match rate {
            RfidBitRate::Rf64 => 64.0,
            RfidBitRate::Rf32 => 32.0,
        };
        Reader {
            rate,
            halves: ManchesterHalves::new(IntervalWindows::manchester(div / carrier_hz / 2.0)),
            list: Vec::with_capacity(LIST_MAX + 4),
            count: 0,
            last_frame: None,
            last_refused: None,
            place: None,
        }
    }

    fn reset(&mut self) {
        self.halves.reset();
        self.list.clear();
        self.place = None;
    }

    /// Takes the interval `dt` of level `level` and returns what the newest windows hold.
    fn edge(&mut self, dt: f64, level: u8, codes: RfidCodings) -> Option<Found> {
        let Some(n) = self.halves.edge(dt) else {
            self.list.clear();
            self.place = None;
            return None;
        };
        for _ in 0..n {
            self.list.push(level);
        }
        self.count += n as u64;
        if self.list.len() > LIST_MAX {
            self.list.drain(..LIST_DROP);
        }
        let mut refused = None;
        for lag in 0..4usize {
            let Some(end) = self.list.len().checked_sub(lag) else {
                break;
            };
            let Some(start) = end.checked_sub(WINDOW) else {
                break;
            };
            let at = self.count - lag as u64;
            match search(&self.list[start..end], codes) {
                Some(Found::Tag(d, coding)) => {
                    self.place = Some((coding, at % WINDOW as u64));
                    if self
                        .last_frame
                        .is_some_and(|l| at.abs_diff(l) < SAME_FRAME_HALVES)
                    {
                        return None;
                    }
                    self.last_frame = Some(at);
                    return Some(Found::Tag(d, coding));
                }
                Some(Found::Refused(coding)) => refused = refused.or(Some((coding, at))),
                None => {}
            }
        }
        // A refused frame counts only at the place of the frames of the last read: the same code and the same position in
        // the stream of half-bits. Elsewhere a window of the other code or of another alignment can hold a header by chance.
        let (coding, at) = refused?;
        if self.place != Some((coding, at % WINDOW as u64))
            || self
                .last_refused
                .is_some_and(|l| at.abs_diff(l) < SAME_FRAME_HALVES)
            || self
                .last_frame
                .is_some_and(|l| at.abs_diff(l) < SAME_FRAME_HALVES)
        {
            return None;
        }
        self.last_refused = Some(at);
        Some(Found::Refused(coding))
    }
}

/// A row of the tag table as the reader holds it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TagRow {
    time_s: f64,
    id: u64,
    rate: RfidBitRate,
    coding: RfidCoding,
    reads: i64,
    level_db: f64,
}

impl TagRow {
    /// The tag as 10 hex digits.
    fn tag(&self) -> String {
        format!("{:010X}", self.id)
    }

    /// The row of the table, with the text `tag`.
    fn shape<'a>(&self, tag: &'a str) -> RfidLfTagRow<'a> {
        let number = (self.id & 0xFFFF_FFFF) as i64;
        RfidLfTagRow {
            time_s: Seconds(self.time_s),
            tag,
            version: (self.id >> 32) as i64,
            number,
            number_hi: number >> 16,
            number_lo: number & 0xFFFF,
            bit_rate: self.rate,
            coding: self.coding,
            reads: self.reads,
            level_db: self.level_db,
        }
    }
}

/// The tag in the row being written.
#[derive(Clone, Copy, Debug)]
struct Current {
    key: u64,
    id: u64,
    last_s: f64,
    reads: i64,
    rate: RfidBitRate,
    coding: RfidCoding,
}

/// The `rfid_lf` reader component.
pub struct RfidLfDecoder {
    bit_rates: RfidBitRates,
    codes: RfidCodings,
    same_tag_s: f64,
    gate_sigmas: f64,
    rate: f64,
    carrier_hz: f64,
    offset_hz: f64,
    front: Option<Front>,
    slicer: Option<TrackingSlicer>,
    readers: Vec<Reader>,
    changes: Vec<Change>,
    last_change: Option<f64>,
    t: f64,
    /// The stack time at the last build of the front end: times keep running across rebuilds.
    origin: f64,
    seen: HashSet<u64>,
    reads: i64,
    refused: i64,
    current: Option<Current>,
    last_tag: Option<String>,
    tally: Tally,
    level: InputLevel,
    log: Log<TagRow>,
    events: Vec<Event>,
    status: Status,
    dial_hz: f64,
    started: bool,
}

impl Default for RfidLfDecoder {
    fn default() -> Self {
        RfidLfDecoder {
            bit_rates: RfidBitRates::Both,
            codes: RfidCodings::Both,
            same_tag_s: SAME_TAG_S.default,
            gate_sigmas: GATE_SIGMAS.default,
            rate: 0.0,
            carrier_hz: NOMINAL_CARRIER_HZ,
            offset_hz: 0.0,
            front: None,
            slicer: None,
            readers: Vec::new(),
            changes: Vec::new(),
            last_change: None,
            t: 0.0,
            origin: 0.0,
            seen: HashSet::new(),
            reads: 0,
            refused: 0,
            current: None,
            last_tag: None,
            tally: Tally::default(),
            level: InputLevel::default(),
            log: Log::default(),
            events: Vec::new(),
            status: Status::default(),
            dial_hz: 0.0,
            started: false,
        }
    }
}

impl RfidLfDecoder {
    fn build(&mut self) {
        self.origin = self.t;
        let front = Front::new(self.rate, self.offset_hz, WORK_HZ, PASS_HZ);
        self.slicer = Some(TrackingSlicer::new(
            slicer_config(self.gate_sigmas),
            front.rate(),
        ));
        self.front = Some(front);
        self.readers = match self.bit_rates {
            RfidBitRates::Both => vec![RfidBitRate::Rf64, RfidBitRate::Rf32],
            RfidBitRates::Rf64 => vec![RfidBitRate::Rf64],
            RfidBitRates::Rf32 => vec![RfidBitRate::Rf32],
        }
        .into_iter()
        .map(|r| Reader::new(r, self.carrier_hz))
        .collect();
        self.last_change = None;
    }

    fn reset(&mut self) {
        if let Some(s) = self.slicer.as_mut() {
            s.reset();
        }
        for r in &mut self.readers {
            r.reset();
        }
        self.last_change = None;
    }

    fn clear(&mut self) {
        self.reset();
        self.seen.clear();
        self.reads = 0;
        self.refused = 0;
        self.current = None;
        self.last_tag = None;
        self.tally = Tally::default();
        self.log.clear();
        self.events.clear();
    }

    fn work_rate(&self) -> f64 {
        self.front.as_ref().map_or(WORK_HZ, Front::rate)
    }

    fn read(&mut self, id: u64, rate: RfidBitRate, coding: RfidCoding, t: f64, level_db: f64) {
        self.reads += 1;
        self.tally.good(t);
        let mut row = TagRow {
            time_s: t,
            id,
            rate,
            coding,
            reads: 1,
            level_db,
        };
        match self
            .current
            .filter(|c| c.id == id && t - c.last_s <= self.same_tag_s)
        {
            Some(mut c) => {
                c.last_s = t;
                c.reads += 1;
                row.reads = c.reads;
                self.current = Some(c);
                self.log.update(c.key, row);
            }
            None => {
                self.seen.insert(id);
                let key = self.log.push(row);
                self.current = Some(Current {
                    key,
                    id,
                    last_s: t,
                    reads: 1,
                    rate,
                    coding,
                });
                let tag = row.tag();
                let r = row.shape(&tag);
                self.events.push(RFID_TAG.event(
                    t,
                    self.dial_hz,
                    &RfidTagEvent {
                        time_s: r.time_s,
                        tag: r.tag,
                        version: r.version,
                        number: r.number,
                        number_hi: r.number_hi,
                        number_lo: r.number_lo,
                        bit_rate: r.bit_rate,
                        coding: r.coding,
                        reads: r.reads,
                        level_db: r.level_db,
                    },
                ));
            }
        }
        self.last_tag = Some(row.tag());
    }

    fn change(&mut self, c: Change) {
        let r = self.work_rate();
        let t = self.origin + c.at / r;
        self.t = self.t.max(t);
        self.tally.energy(t);
        let Some(prev) = self.last_change.replace(t) else {
            return;
        };
        let dt = t - prev;
        let codes = self.codes;
        let mut found = Vec::new();
        for rd in &mut self.readers {
            if let Some(f) = rd.edge(dt, c.before, codes) {
                found.push((rd.rate, f));
            }
        }
        let level_db = db20(self.slicer.as_ref().map_or(0.0, TrackingSlicer::carrier));
        for (rate, f) in found {
            match f {
                Found::Tag(id, coding) => self.read(id, rate, coding, t, level_db),
                Found::Refused(_) => {
                    self.refused += 1;
                    self.tally.bad(t);
                }
            }
        }
    }
}

impl Decoder for RfidLfDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "rfid_lf.reader"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        ReaderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match ReaderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ReaderSetting::BitRates => self.bit_rates = BIT_RATES.read(id, value)?,
            ReaderSetting::Codes => self.codes = CODES.read(id, value)?,
            ReaderSetting::SameTagS => self.same_tag_s = SAME_TAG_S.read(id, value)?,
            ReaderSetting::GateSigmas => {
                self.gate_sigmas = GATE_SIGMAS.read(id, value)?;
                if let Some(s) = self.slicer.as_mut() {
                    s.set_gate(Gate::Sigma(self.gate_sigmas));
                }
            }
        }
        if self.rate > 0.0 && ReaderSetting::from_id(id) == Some(ReaderSetting::BitRates) {
            self.build();
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (
                ReaderSetting::BitRates,
                Value::Choice(self.bit_rates.index()),
            ),
            (ReaderSetting::Codes, Value::Choice(self.codes.index())),
            (ReaderSetting::SameTagS, Value::F64(self.same_tag_s)),
            (ReaderSetting::GateSigmas, Value::F64(self.gate_sigmas)),
        ])
    }
    fn commands(&self) -> &'static [&'static Command] {
        &COMMANDS
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        if id.index() == 0 {
            self.reset();
            Ok(None)
        } else {
            Err(CommandError::NoSuchId { id })
        }
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if !input.rate.is_finite() || input.rate < MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.dial_hz = input.tuned_hz;
        self.offset_hz = input.tuned_hz - input.centre_hz;
        self.carrier_hz = carrier_of(input.tuned_hz);
        self.started = false;
        self.build();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, input: &Format) {
        self.dial_hz = input.tuned_hz;
        self.offset_hz = input.tuned_hz - input.centre_hz;
        self.carrier_hz = carrier_of(input.tuned_hz);
        if self.rate > 0.0 {
            self.build();
        }
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.started = true;
        self.level.push(x);
        let (Some(mut front), Some(mut slicer)) = (self.front.take(), self.slicer.take()) else {
            return;
        };
        let mut changes = std::mem::take(&mut self.changes);
        changes.clear();
        let y = front.process(x);
        slicer.process_iq(y, &mut changes);
        self.t = self.origin + slicer.position() as f64 / front.rate();
        self.front = Some(front);
        self.slicer = Some(slicer);
        for c in changes.drain(..) {
            self.change(c);
        }
        self.changes = changes;
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
    fn report(&mut self, p: &mut Published) {
        self.log.publish(
            p,
            |p| p.clear_rows(&RFID_LF_TAGS),
            |p, key, r| RFID_LF_TAGS.put(p, key, &r.shape(&r.tag())),
        );
        let t = self.t;
        let s = self.slicer.as_ref();
        let carrier = s
            .map(TrackingSlicer::carrier)
            .filter(|&m| self.started && m > 0.0);
        let modulated = self.tally.recent_energy(t);
        let depth = s.filter(|_| modulated).map(|s| 100.0 * s.depth());
        let last = self.current.filter(|_| self.last_tag.is_some());
        p.put(&RfidLfFields {
            carrier_db: carrier.map(db20),
            depth_pct: depth,
            tags: self.seen.len() as i64,
            reads: self.reads,
            refused: self.refused,
            bit_rate: last.map(|c| c.rate),
            coding: last.map(|c| c.coding),
            last_tag: self.last_tag.as_deref(),
        });
        let headline = if !self.started {
            "waiting for data".to_string()
        } else if let Some(tag) = self.last_tag.as_deref() {
            format!("{} tag reads, last {} {}", self.reads, &tag[..2], &tag[2..])
        } else if modulated {
            "carrier found, no tag read yet".to_string()
        } else {
            "no load modulation seen".to_string()
        };
        let level = self.level.read();
        let st = &mut self.status;
        st.lock_state = self.tally.lock(t);
        st.level_dbfs = level.map(|l| l as f32);
        st.snr_db = None;
        st.offset_hz = None;
        st.summary = headline;
        p.put(&TagsStatusFields {
            text: &st.summary,
            state: self.tally.state(t),
            good: i64::try_from(self.tally.good).unwrap_or(i64::MAX),
            bad: i64::try_from(self.tally.bad).unwrap_or(i64::MAX),
            level_dbfs: level,
            note: NOTE_TEXT,
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        RfidLfDecoder::clear(self);
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset();
        self.tally = Tally::default();
        if !kept.table(RFID_LF_TAGS.key) {
            self.log.clear();
            self.current = None;
        }
        if !kept.field(RfidLfFields::TAGS.key) {
            self.seen.clear();
        }
        if !kept.field(RfidLfFields::READS.key) {
            self.reads = 0;
        }
        if !kept.field(RfidLfFields::REFUSED.key) {
            self.refused = 0;
        }
        if !kept.field(RfidLfFields::LAST_TAG.key) {
            self.last_tag = None;
        }
    }
}

/// The receiver: one reader component on the input.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("reader", DecoderComponent::new(RfidLfDecoder::default()))
        .input("reader")
        .output("reader.events")
        .status_from("reader")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape(ID), graph).min_rate(
        MIN_RATE_HZ,
        "The EM4100 reader needs a sample rate of at least 24 ksps",
    );
    for key in [
        BIT_RATES.key(),
        CODES.key(),
        SAME_TAG_S.key(),
        GATE_SIGMAS.key(),
    ] {
        g = g.route(key, &[&format!("reader.{key}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("reader", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("rfid_lf.reader", || {
        Box::new(DecoderComponent::new(RfidLfDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(RfidLfGenerator::new(cfg.rate_hz, &cfg.settings)))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, the tag 01 23456789 at RF/64", o, 0.0);
    c.seconds = o.seconds.max(2.0);
    c.snr_db = (!o.clean).then_some(55.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let reads = crate::number(r, RfidLfFields::READS.key);
        let last = RfidLfFields::read(r.fields()).and_then(|f| f.last_tag.map(str::to_owned));
        vec![
            CheckLine::new(RfidLfFields::READS.key, reads >= 1.0, format!("{reads}")),
            CheckLine::new(
                RfidLfFields::LAST_TAG.key,
                last.as_deref() == Some("0123456789"),
                format!("{last:?}, 0123456789 sent"),
            ),
        ]
    });
    vec![c]
}

/// The engine's checks of the test signal.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The EM4100 LF tag reader.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "EM4100 LF RFID",
    group: crate::GROUP,
    blurb: "The 40-bit identity of EM4100 tags from the load modulation on a 100 to 150 kHz reader carrier.",
    tuning: Tuning {
        carrier_at_centre: true,
        not_circular: true,
        ..Tuning::anywhere(0.0, 250e3, MIN_RATE_HZ, 10e3)
    },
    submodes: &[],
    settings: RfidLfSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: Some(crate::bench::benchmarks),
};
