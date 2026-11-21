//! EPC Gen2 reader commands (`rfid_uhf`): the commands that a UHF RFID reader sends to its tags by pulse-interval encoding
//! of its carrier (GS1, EPC Radio-Frequency Identity Protocols Generation-2 UHF RFID, version 2.0.1, sections 6.3.1.2 and
//! 6.3.2.12): Query, QueryRep, QueryAdjust, ACK, NAK, Select, Req_RN, Read, Write, Kill, Lock and Access, with Tari, RTcal,
//! TRcal and the tag link frequency that a Query asks. A command is shown only after its timing, its length and its CRC-5 or
//! CRC-16 hold; QueryRep, QueryAdjust, ACK and NAK have no CRC in the standard and only their unique lengths and the timing
//! protect them. The tag replies are not decoded.
//!
//! The graph: one decoder component (`reader`). It mixes out the dial offset, decimates to about 1 Msps, slices the envelope
//! with the shared tracking slicer and reads the frames of pulse intervals.

pub mod command;
pub mod gen;

use crate::family::{db20, Front, InputLevel, Log, Tally, REPORT_S};
use crate::typed::{
    Gen2Command, RfidCommandEvent, RfidUhfCommandRow, RfidUhfFields, TagsStatusFields,
    RFID_COMMAND, RFID_UHF_COMMANDS,
};
use command::Decoded;
use gen::UhfGenerator;
use sdrkit_algo_modem::envtrack::{EdgeTime, Gate, TrackConfig, TrackingSlicer};
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
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "rfid_uhf";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 400e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 1e6;
/// Rows kept in the command table.
pub const ROWS_KEPT: usize = 500;

/// The low pulse of a delimiter, s (the standard gives 12.5 us within 5 %).
const DELIMITER: (f64, f64) = (9e-6, 17e-6);
/// The carrier before a delimiter, s.
const DELIMITER_CARRIER_S: f64 = 40e-6;
/// The shortest and the longest symbol, s.
const SYMBOL: (f64, f64) = (2.5e-6, 350e-6);
/// The most symbols of a frame.
const MAX_SYMBOLS: usize = 683;
/// The end of a frame without RTcal, s.
const EARLY_END_S: f64 = 400e-6;

/// What the stack reads and what no public document confirms.
pub const NOTE_TEXT: &str = "Reads the commands of an EPC Gen2 reader (version 2.0.1): a command is shown only after its \
timing, its length and its CRC hold; QueryRep, QueryAdjust, ACK and NAK carry no CRC. Tag replies, PR-ASK against DSB-ASK and \
SSB-ASK, the commands added after Access and the regional channel plans are not read; the cover-coded data and the password \
halves are never shown. Tari 6.25 us needs an input of 2.4 Msps. The receive windows (delimiter 9 to 17 us after 40 us of \
carrier, Tari 5.5 to 28 us, RTcal 2.35 to 3.2 Tari, TRcal up to 3.2 RTcal, a 0 within 0.85 to 1.15 Tari and a 1 within 1.35 \
to 2.2 Tari), the frame end and the three blocks of a pointer are choices of this stack, not of the standard. Not tried on a \
real signal.";

/// Rows for the commands without a CRC.
pub const LIST_UNCHECKED: Setting<field::Bool> =
    Setting::new("list_unchecked", "List commands that have no checksum", "", true)
        .help("QueryRep, QueryAdjust, ACK and NAK are counted either way; off, they make no row and no event.");

sdrkit_core::settings! {
    /// The settings of the reader component.
    pub enum ReaderSetting {
        /// The rows of unchecked commands.
        ListUnchecked = LIST_UNCHECKED,
    }
}

sdrkit_core::settings! {
    /// The settings of the `rfid_uhf` stack.
    pub enum RfidUhfSetting {
        /// The rows of unchecked commands.
        ListUnchecked = LIST_UNCHECKED,
        /// Tari of the test signal.
        GenTariUs = gen::GEN_TARI_US,
        /// The data-1.
        GenData1Tari = gen::GEN_DATA1_TARI,
        /// PW.
        GenPwTari = gen::GEN_PW_TARI,
        /// The depth.
        GenDepth = gen::GEN_DEPTH,
        /// TRcal.
        GenTrcalRtcal = gen::GEN_TRCAL_RTCAL,
        /// The reader clock.
        GenReaderClockPpm = gen::GEN_READER_CLOCK_PPM,
        /// The gap.
        GenGapS = gen::GEN_GAP_S,
        /// The lead.
        GenLeadS = gen::GEN_LEAD_S,
        /// The commands.
        GenCommands = gen::GEN_COMMANDS,
    }
}

/// The `reset` command: the trackers and any frame in progress start again; the table and the counts stay.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Drops the trackers and the frame in progress; the table stays.");
static COMMANDS: [&Command; 1] = [&RESET];

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

fn slicer_config() -> TrackConfig {
    TrackConfig {
        smoothing: 0.6,
        attack: 1.0,
        peak_decay_s: 0.002,
        floor_decay_s: 0.01,
        gate: Gate::Share(0.3),
        shut_level: Some(1),
        hysteresis: 0.1,
        edge: EdgeTime::Interpolated { within: 64.0 },
        start_s: 0.0,
    }
}

/// The timings of a frame that passed its checks, s.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Timing {
    tari: f64,
    rtcal: f64,
    trcal: Option<f64>,
}

/// Tari and RTcal of a frame fit the rules of a reader.
fn tari_rules(sym: &[f64]) -> bool {
    let (tari, rtcal) = (sym[0], sym[1]);
    (5.5e-6..=28e-6).contains(&tari) && rtcal >= 2.35 * tari && rtcal <= 3.2 * tari
}

/// The bits of a frame of symbol lengths, after the timing rules; `None` when a rule fails.
fn frame_bits(sym: &[f64]) -> Option<(Vec<u8>, Timing)> {
    let (tari, rtcal) = (sym[0], sym[1]);
    if !tari_rules(sym) {
        return None;
    }
    let trcal = (sym[2] > 1.05 * rtcal).then_some(sym[2]);
    if trcal.is_some_and(|t| t > 3.2 * rtcal) {
        return None;
    }
    let first = if trcal.is_some() { 3 } else { 2 };
    let mut bits = Vec::with_capacity(sym.len());
    for &s in &sym[first..] {
        let r = s / tari;
        if s > rtcal / 2.0 {
            if !(1.35..=2.2).contains(&r) {
                return None;
            }
            bits.push(1);
        } else {
            if !(0.85..=1.15).contains(&r) {
                return None;
            }
            bits.push(0);
        }
    }
    Some((bits, Timing { tari, rtcal, trcal }))
}

/// A command row as the reader holds it.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    time_s: f64,
    d: Decoded,
    level_db: f64,
}

impl Row {
    fn shape(&self) -> RfidUhfCommandRow<'_> {
        let d = &self.d;
        RfidUhfCommandRow {
            time_s: Seconds(self.time_s),
            command: d.command.unwrap_or(Gen2Command::Nak),
            session: d.session,
            q: d.q,
            slots: d.q.map(|q| 1i64 << q),
            target: d.target,
            sel: d.sel,
            coding: d.coding,
            pilot: d.pilot,
            dr: d.dr,
            updn: d.updn,
            rn16: d.rn16.as_deref(),
            bank: d.bank,
            pointer: d.pointer,
            word_count: d.word_count,
            mask: d.mask.as_deref(),
            truncate: d.truncate,
            action: d.action,
            lock_payload: d.lock_payload.as_deref(),
            level_db: self.level_db,
        }
    }
}

/// The `rfid_uhf` reader component.
pub struct RfidUhfDecoder {
    list_unchecked: bool,
    rate: f64,
    offset_hz: f64,
    front: Option<Front>,
    slicer: Option<TrackingSlicer>,
    fall: Option<f64>,
    last_rise: Option<f64>,
    frame: Option<Vec<f64>>,
    timing: Option<Timing>,
    last_trcal: Option<f64>,
    last_carrier: Option<f64>,
    t: f64,
    /// The stack time at the last build of the front end: times keep running across rebuilds.
    origin: f64,
    ok: i64,
    failed: i64,
    counts: [i64; 12],
    last: Option<Gen2Command>,
    tally: Tally,
    level: InputLevel,
    log: Log<Row>,
    events: Vec<Event>,
    status: Status,
    dial_hz: f64,
    started: bool,
}

impl Default for RfidUhfDecoder {
    fn default() -> Self {
        RfidUhfDecoder {
            list_unchecked: LIST_UNCHECKED.default,
            rate: 0.0,
            offset_hz: 0.0,
            front: None,
            slicer: None,
            fall: None,
            last_rise: None,
            frame: None,
            timing: None,
            last_trcal: None,
            last_carrier: None,
            t: 0.0,
            origin: 0.0,
            ok: 0,
            failed: 0,
            counts: [0; 12],
            last: None,
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

impl RfidUhfDecoder {
    fn build(&mut self) {
        self.origin = self.t;
        let d = ((self.rate / WORK_HZ).floor()).max(1.0);
        let pass = 300e3f64.min(0.45 * self.rate / d);
        let front = Front::new(self.rate, self.offset_hz, WORK_HZ, pass);
        self.slicer = Some(TrackingSlicer::new(slicer_config(), front.rate()));
        self.front = Some(front);
        self.reset();
    }

    fn reset(&mut self) {
        if let Some(s) = self.slicer.as_mut() {
            s.reset();
        }
        self.fall = None;
        self.last_rise = None;
        self.frame = None;
    }

    fn clear(&mut self) {
        self.reset();
        self.ok = 0;
        self.failed = 0;
        self.counts = [0; 12];
        self.last = None;
        self.timing = None;
        self.last_trcal = None;
        self.tally = Tally::default();
        self.log.clear();
        self.events.clear();
    }

    fn fail(&mut self, t: f64) {
        self.failed += 1;
        self.tally.bad(t);
    }

    fn rise(&mut self, t: f64) {
        let Some(fall) = self.fall.take() else {
            self.last_rise = Some(t);
            return;
        };
        let pw = t - fall;
        match self.frame.as_mut() {
            None => {
                let carrier = self
                    .last_rise
                    .is_none_or(|r| fall - r >= DELIMITER_CARRIER_S);
                if pw > DELIMITER.0 && pw < DELIMITER.1 && carrier {
                    self.frame = Some(Vec::with_capacity(64));
                    self.tally.energy(t);
                }
            }
            Some(sym) => {
                let s = t - self.last_rise.unwrap_or(t);
                if !(SYMBOL.0..=SYMBOL.1).contains(&s) || sym.len() >= MAX_SYMBOLS {
                    self.frame = None;
                } else {
                    sym.push(s);
                }
            }
        }
        self.last_rise = Some(t);
    }

    /// Ends the frame in progress when no rise came for its time.
    fn idle(&mut self, now: f64) {
        let (Some(sym), Some(r)) = (self.frame.as_ref(), self.last_rise) else {
            return;
        };
        let limit = if sym.len() >= 2 {
            3.5 * sym[1]
        } else {
            EARLY_END_S
        };
        if now - r > limit {
            let sym = self.frame.take().unwrap_or_default();
            self.end(&sym, now);
        }
    }

    fn end(&mut self, sym: &[f64], t: f64) {
        if sym.len() < 4 {
            return;
        }
        // A frame whose Tari and RTcal fit no reader is not counted: a pulse train of noise can look like a delimiter.
        let Some((bits, tm)) = frame_bits(sym) else {
            if tari_rules(sym) {
                self.fail(t);
            }
            return;
        };
        self.last_carrier = Some(t);
        let Ok(d) = command::decode(&bits, tm.trcal.is_some()) else {
            self.fail(t);
            return;
        };
        let Some(c) = d.command else {
            self.fail(t);
            return;
        };
        self.ok += 1;
        self.counts[usize::from(c.index())] += 1;
        self.last = Some(c);
        self.timing = Some(tm);
        if tm.trcal.is_some() {
            self.last_trcal = tm.trcal;
        }
        self.tally.good(t);
        let unchecked = matches!(
            c,
            Gen2Command::QueryRep | Gen2Command::QueryAdjust | Gen2Command::Ack | Gen2Command::Nak
        );
        if unchecked && !self.list_unchecked {
            return;
        }
        let level_db = db20(self.slicer.as_ref().map_or(0.0, TrackingSlicer::peak));
        let row = Row {
            time_s: t,
            d,
            level_db,
        };
        let r = row.shape();
        self.events.push(RFID_COMMAND.event(
            t,
            self.dial_hz,
            &RfidCommandEvent {
                time_s: r.time_s,
                command: r.command,
                session: r.session,
                q: r.q,
                slots: r.slots,
                target: r.target,
                sel: r.sel,
                coding: r.coding,
                pilot: r.pilot,
                dr: r.dr,
                updn: r.updn,
                rn16: r.rn16,
                bank: r.bank,
                pointer: r.pointer,
                word_count: r.word_count,
                mask: r.mask,
                truncate: r.truncate,
                action: r.action,
                lock_payload: r.lock_payload,
                level_db: r.level_db,
            },
        ));
        self.log.push(row);
    }
}

impl Decoder for RfidUhfDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "rfid_uhf.reader"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        ReaderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match ReaderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ReaderSetting::ListUnchecked => self.list_unchecked = LIST_UNCHECKED.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([(
            ReaderSetting::ListUnchecked,
            Value::Bool(self.list_unchecked),
        )])
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
        self.started = false;
        self.build();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, input: &Format) {
        self.dial_hz = input.tuned_hz;
        self.offset_hz = input.tuned_hz - input.centre_hz;
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
        let r = front.rate();
        for &z in front.process(x) {
            let c = slicer.push_iq(z);
            let now = self.origin + slicer.position() as f64 / r;
            if let Some(c) = c {
                let at = self.origin + c.at / r;
                if c.before == 1 {
                    self.fall = Some(at);
                } else {
                    self.rise(at);
                }
            } else if self.frame.is_some() {
                self.idle(now);
            }
            self.t = now;
        }
        self.front = Some(front);
        self.slicer = Some(slicer);
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
    fn report(&mut self, p: &mut Published) {
        self.log.publish(
            p,
            |p| p.clear_rows(&RFID_UHF_COMMANDS),
            |p, key, r| {
                RFID_UHF_COMMANDS.put(p, key, &r.shape());
            },
        );
        let t = self.t;
        let carrier = self.last_carrier.is_some_and(|c| t - c <= 2.0);
        let tm = self
            .timing
            .filter(|_| self.last_carrier.is_some_and(|c| t - c <= 8.0));
        let trcal = self.last_trcal.filter(|_| tm.is_some());
        let c = &self.counts;
        p.put(&RfidUhfFields {
            tari_us: tm.map(|m| m.tari * 1e6),
            rtcal_us: tm.map(|m| m.rtcal * 1e6),
            rtcal_tari: tm.map(|m| m.rtcal / m.tari),
            trcal_us: trcal.map(|v| v * 1e6),
            blf_dr8_khz: trcal.map(|v| 8.0 / v / 1e3),
            blf_dr643_khz: trcal.map(|v| 64.0 / 3.0 / v / 1e3),
            commands_ok: self.ok,
            commands_failed: self.failed,
            count_query: c[0],
            count_query_rep: c[1],
            count_query_adjust: c[2],
            count_ack: c[3],
            count_nak: c[4],
            count_select: c[5],
            count_req_rn: c[6],
            count_read: c[7],
            count_write: c[8],
            count_kill: c[9],
            count_lock: c[10],
            count_access: c[11],
            reader_carrier: carrier,
        });
        let headline = if !self.started {
            "waiting for data".to_string()
        } else if let Some(l) = self.last {
            format!("{} reader commands, last {}", self.ok, l.label())
        } else if carrier {
            "reader carrier, no command yet".to_string()
        } else {
            "no reader carrier".to_string()
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
        RfidUhfDecoder::clear(self);
    }
    fn forget(&mut self, kept: &Kept) {
        let log = std::mem::take(&mut self.log);
        self.clear();
        if kept.table(RFID_UHF_COMMANDS.key) {
            self.log = log;
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
        .add("reader", DecoderComponent::new(RfidUhfDecoder::default()))
        .input("reader")
        .output("reader.events")
        .status_from("reader")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE_HZ,
            "The Gen2 reader needs a sample rate of at least 400 ksps",
        )
        .route(LIST_UNCHECKED.key(), &["reader.list_unchecked"])?
        .command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("reader", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("rfid_uhf.reader", || {
        Box::new(DecoderComponent::new(RfidUhfDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(UhfGenerator::new(cfg.rate_hz, &cfg.settings)))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, the eight commands of the list", o, 0.0);
    c.seconds = o.seconds.max(1.0);
    c.snr_db = (!o.clean).then_some(30.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let ok = crate::number(r, RfidUhfFields::COMMANDS_OK.key);
        let tari = RfidUhfFields::read(r.fields()).and_then(|f| f.tari_us);
        vec![
            CheckLine::new(
                RfidUhfFields::COMMANDS_OK.key,
                ok >= 8.0,
                format!("{ok}, 8 sent in the first pass"),
            ),
            CheckLine::new(
                RfidUhfFields::TARI_US.key,
                tari.is_some_and(|t| (t - 12.5).abs() < 0.5),
                format!("{tari:?}, 12.5 sent"),
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

/// The Gen2 reader command reader.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "UHF RFID reader commands (EPC Gen2)",
    group: crate::GROUP,
    blurb: "The commands an EPC Gen2 reader sends to its tags, with their fields, Tari, RTcal, TRcal and the tag rate they ask. \
Tag replies are not decoded.",
    tuning: Tuning {
        carrier_at_centre: true,
        not_circular: true,
        ..Tuning::anywhere(0.0, 2e6, MIN_RATE_HZ, 600e3)
    },
    submodes: &[],
    settings: RfidUhfSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: None,
};
