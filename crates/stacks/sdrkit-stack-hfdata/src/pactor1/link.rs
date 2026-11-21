//! Repeats, memory ARQ, FEC mode, sides, text and stations of PACTOR-I, and its results.

use super::proto::{self, Huffman, Packet};
use super::rx::{decode_soft, FrameKind, PactorFrame, PactorPair, FS};
use crate::typed::{
    Pactor1Fields, Pactor1FrameRow, Pactor1StationRow, PactorFrameEvent, PACTOR1_FRAMES,
    PACTOR1_REMOVE, PACTOR1_STATIONS, PACTOR1_STATION_ROW, PACTOR_FRAME,
};
use crate::util::{level_points, update_points, Origin};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::results::SeriesId;
use sdrkit_core::{
    Choice, ConfigError, LockState, Record, Results, ResultsDecl, Setting, SettingError, SettingId,
    SettingSet, StateBlob, StateError, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RunContext,
};
use sdrkit_util_objects::commands::{self as objcmd, Tables};
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

const FRAMES_CAP: usize = 300;
const STATIONS_CAP: usize = 60;
const TEXT_CAP: usize = 6000;
const LEVEL_CAP: usize = 120;
const START_LOG: &str = "PACTOR-I: 100 / 200 baud FSK, 200 Hz shift, on the upper sideband (experimental, monitor only)";

sdrkit_core::choice_enum! {
    /// The kind of a frame.
    #[derive(Serialize, Deserialize)]
    pub enum FrameType {
        /// A data packet.
        Data = ("data", "data"),
        /// A connect packet.
        Connect = ("connect", "connect"),
        /// A packet with the QRT flag.
        Qrt = ("qrt", "QRT"),
        /// Control signal 1.
        Cs1 = ("cs1", "CS1"),
        /// Control signal 2.
        Cs2 = ("cs2", "CS2"),
        /// Control signal 3.
        Cs3 = ("cs3", "CS3"),
        /// Control signal 4.
        Cs4 = ("cs4", "CS4"),
        /// A packet whose CRC failed.
        Bad = ("bad_packet", "bad packet"),
    }
}

sdrkit_core::choice_enum! {
    /// How a station was seen.
    #[derive(Serialize, Deserialize)]
    pub enum SeenAs {
        /// Called in a connect packet.
        Connect = ("connect", "called in a connect packet"),
        /// Named in the text.
        Text = ("text", "named in text"),
    }
}

/// The series of the input audio level, one point per second.
pub const LEVEL_SERIES: &str = "level";

/// What the PACTOR-I stack publishes.
pub fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(Pactor1Fields::SHAPE.fields)
        .with_table(&PACTOR1_FRAMES)
        .with_table(&PACTOR1_STATIONS)
        .series(LEVEL_SERIES, "Level", "s", "dB")
        .event(&PACTOR_FRAME)
}

/// One frame in the frame list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRow {
    /// Signal time of the first bit, seconds.
    pub t_s: f64,
    /// UTC, when the clock is known.
    pub utc: Option<f64>,
    /// The kind of frame.
    pub kind: FrameType,
    /// 100 or 200.
    pub baud: u32,
    /// The header byte (packets).
    pub header: Option<u8>,
    /// The packet number.
    pub count: u8,
    /// Huffman text.
    pub huffman: bool,
    /// The break-in flag.
    pub break_in: bool,
    /// The QRT flag.
    pub qrt: bool,
    /// The CRC passed.
    pub crc_ok: bool,
    /// Read by memory ARQ.
    pub memory_arq: bool,
    /// A repeat.
    pub repeat: bool,
    /// An unproto (FEC) repeat.
    pub fec: bool,
    /// The tones were swapped.
    pub inverted: bool,
    /// The sending side.
    pub side: u8,
    /// The decoded text (packets) or the meaning (control signals).
    pub text: String,
    /// The data bytes in hexadecimal.
    pub hex: String,
    /// The called callsign of a connect packet.
    pub call: String,
    /// SNR, dB.
    pub snr_db: f64,
    /// The lower tone, Hz.
    pub low_hz: f64,
}

impl Shift for FrameRow {
    fn shift_times(&mut self, by_s: f64) {
        self.t_s += by_s;
    }
}

impl Shaped for FrameRow {
    type Row = Pactor1FrameRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&Pactor1FrameRow {
            start: Seconds(self.t_s),
            time: self.utc.map(Time),
            kind: self.kind,
            symbol_rate_hz: Hertz(f64::from(self.baud)),
            header: self.header.map(i64::from),
            number: self.header.map(|_| i64::from(self.count)),
            huffman: self.huffman,
            break_in: self.break_in,
            qrt: self.qrt,
            crc_ok: self.crc_ok,
            memory_arq: self.memory_arq,
            repeat: self.repeat,
            fec_repeat: self.fec,
            inverted: self.inverted,
            side: i64::from(self.side),
            text: &self.text,
            data_hex: &self.hex,
            call: (!self.call.is_empty()).then_some(self.call.as_str()),
            snr_db: self.snr_db,
            pair_low: Hertz(self.low_hz),
        });
    }
}

/// One station.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationRow {
    /// The callsign.
    pub call: String,
    /// How it was seen.
    pub how: SeenAs,
}

impl Shift for StationRow {
    fn shift_times(&mut self, _: f64) {}
}

impl Shaped for StationRow {
    type Row = Pactor1StationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&Pactor1StationRow {
            call: &self.call,
            seen_as: self.how,
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// What the display keeps: the frames, the stations and the text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PactorLists {
    /// The frames, oldest first.
    pub frames: LogRing<FrameRow>,
    /// The stations, by callsign.
    pub stations: ObjectTable<String, StationRow>,
    /// The text of both sides, the newest 6000 characters.
    pub text: String,
    /// The side of the last text, if any.
    pub text_side: Option<u8>,
}

impl Default for PactorLists {
    fn default() -> Self {
        PactorLists {
            frames: LogRing::new(FRAMES_CAP),
            stations: ObjectTable::new(STATIONS_CAP),
            text: String::new(),
            text_side: None,
        }
    }
}

impl Shift for PactorLists {
    fn shift_times(&mut self, by_s: f64) {
        self.frames.shift_times(by_s);
        self.stations.shift_times(by_s);
    }
}

impl Tables for PactorLists {
    const FORMAT: StateFormat = StateFormat::new("hfdata.pactor1_lists", 1);
    fn clear(&mut self) {
        self.frames.clear();
        self.stations.clear();
        self.text.clear();
        self.text_side = None;
    }
    fn adopt(&mut self, saved: Self) {
        self.frames.adopt(saved.frames);
        self.stations.adopt(saved.stations);
        self.text = saved.text;
        self.text_side = saved.text_side;
    }
}

/// The counters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Counters {
    /// Good packets.
    pub packets_ok: u32,
    /// Failed packets.
    pub packets_bad: u32,
    /// Control signals.
    pub control_signals: u32,
    /// Repeats.
    pub repeats: u32,
    /// Memory ARQ saves.
    pub memory_arq_saves: u32,
    /// Good packets at 200 baud.
    pub fast: u32,
    /// Good packets at 100 baud.
    pub slow: u32,
    /// Changeovers.
    pub changeovers: u32,
    /// Connect packets.
    pub connects: u32,
}

/// The Huffman table of the text, one entry a line: the character or its decimal code, a tab, the code bits.
pub const HUFFMAN_TABLE: Setting<field::Text> = Setting::new("huffman_table", "Huffman table", "", "")
    .help("The full table of the public ARRL PACTOR page (Annex, section 9): one entry a line, the character or its decimal code, a tab, the code bits. Empty: the built-in partial table. Unknown codes show as ~.");

sdrkit_core::settings! {
    /// The settings of [`PactorLink`].
    pub enum PactorLinkSetting {
        /// Huffman table.
        HuffmanTable = HUFFMAN_TABLE,
    }
}

sdrkit_core::commands! {
    /// The commands of [`PactorLink`].
    pub enum PactorLinkCommand {
        /// Forget every frame, station and the text.
        Clear = objcmd::CLEAR,
        /// Forget one station.
        Remove = PACTOR1_REMOVE.command,
    }
}

/// `hfdata.pactor_link`: repeats, memory ARQ, text and stations. `pactor.frame`, `pactor.pair` and the in-band power in,
/// `pactor.frame` events out.
pub struct PactorLink {
    p: Published,
    shown: Status,
    level_series: SeriesId,
    origin: Origin,
    live: bool,
    tuned_hz: f64,
    huffman: Huffman,
    huffman_text: String,
    lists: PactorLists,
    side: u8,
    last_good: Option<(f64, u8, Vec<u8>, u8)>,
    cs_since_good: bool,
    run: u32,
    fec: bool,
    store: Option<(f64, Vec<f32>)>,
    c: Counters,
    pair: Option<f64>,
    pair_snr: f64,
    last_frame_t: Option<f64>,
    now: f64,
    level: VecDeque<f32>,
    lvl_acc: f64,
    lvl_n: u64,
    lvl_samples: u64,
    configured: bool,
}

impl Default for PactorLink {
    fn default() -> Self {
        let decl = Arc::new(declaration());
        let level_series = decl
            .series_id(LEVEL_SERIES)
            .expect("the declaration holds the level series");
        PactorLink {
            p: Published::from_decl(decl),
            shown: Status::new(LockState::Off),
            level_series,
            origin: Origin::default(),
            live: false,
            tuned_hz: 0.0,
            huffman: Huffman::default(),
            huffman_text: String::new(),
            lists: PactorLists::default(),
            side: 1,
            last_good: None,
            cs_since_good: false,
            run: 0,
            fec: false,
            store: None,
            c: Counters::default(),
            pair: None,
            pair_snr: 0.0,
            last_frame_t: None,
            now: 0.0,
            level: VecDeque::with_capacity(LEVEL_CAP),
            lvl_acc: 0.0,
            lvl_n: 0,
            lvl_samples: 0,
            configured: false,
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

impl PactorLink {
    /// The lists.
    pub fn lists(&self) -> &PactorLists {
        &self.lists
    }

    /// The counters.
    pub fn counters(&self) -> &Counters {
        &self.c
    }

    /// The text of a data packet, and whether an unknown Huffman code stopped it.
    pub fn packet_text(&self, p: &Packet) -> (String, bool) {
        if p.status.huffman {
            let mut bits = Vec::new();
            proto::bytes_to_bits(&p.data, &mut bits);
            self.huffman.decode(&bits)
        } else {
            (proto::ascii_text(&p.data), false)
        }
    }

    fn wall(&self, t: f64) -> Option<f64> {
        self.origin.wall(t)
    }

    /// Counts a station heard at `t`. A connect packet wins over a name in the text.
    fn station(&mut self, call: &str, t: f64, connect: bool) {
        let at = match self.wall(t) {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        };
        let how = if connect {
            SeenAs::Connect
        } else {
            SeenAs::Text
        };
        if let Ok(u) = self.lists.stations.upsert(call, at, || StationRow {
            call: call.into(),
            how,
        }) {
            if connect {
                u.row.how = SeenAs::Connect;
            }
        }
    }

    fn append_text(&mut self, side: u8, t: &str) {
        let l = &mut self.lists;
        if l.text_side != Some(side) {
            if !l.text.is_empty() && !l.text.ends_with('\n') {
                l.text.push('\n');
            }
            l.text.push_str(&format!("[{side}] "));
            l.text_side = Some(side);
        }
        l.text.push_str(t);
        let n = l.text.chars().count();
        if n > TEXT_CAP {
            l.text = l.text.chars().skip(n - TEXT_CAP).collect();
        }
    }

    fn good(&mut self, f: &PactorFrame, p: &Packet, memory: bool, inverted: bool) -> FrameRow {
        self.store = None;
        self.c.packets_ok += 1;
        if f.baud == 200 {
            self.c.fast += 1;
        } else {
            self.c.slow += 1;
        }
        if memory {
            self.c.memory_arq_saves += 1;
        }
        let repeat = self.last_good.as_ref().is_some_and(|(t, h, d, s)| {
            f.time - t <= 12.0 && *h == p.header && *d == p.data && *s == p.status.byte()
        });
        let fec_repeat = repeat && !self.cs_since_good;
        if !self.cs_since_good && self.last_good.is_some() {
            self.run += 1;
        } else {
            self.run = 1;
        }
        if self.run >= 3 {
            self.fec = true;
        }
        self.cs_since_good = false;
        self.last_good = Some((f.time, p.header, p.data.clone(), p.status.byte()));
        if repeat {
            self.c.repeats += 1;
        }
        let kind = if p.status.qrt {
            FrameType::Qrt
        } else if f.kind == FrameKind::Connect {
            FrameType::Connect
        } else {
            FrameType::Data
        };
        let mut call = String::new();
        let mut text = String::new();
        if kind == FrameType::Connect {
            call = proto::connect_call(&p.data).unwrap_or_default();
            self.c.connects += 1;
            if !repeat {
                self.station(&call.clone(), f.time, true);
            }
        } else if kind == FrameType::Data {
            let (t, _) = self.packet_text(p);
            text = t;
            if !repeat {
                let side = self.side;
                let tt = text.clone();
                self.append_text(side, &tt);
                // Of two stations named in one packet, the one first seen now is listed first.
                let calls: Vec<&str> = text
                    .split(|c: char| !c.is_ascii_alphanumeric())
                    .filter(|w| proto::is_callsign(w))
                    .collect();
                let (known, new): (Vec<&str>, Vec<&str>) = calls
                    .into_iter()
                    .partition(|w| self.lists.stations.contains(*w));
                for w in known.into_iter().chain(new) {
                    self.station(w, f.time, false);
                }
            }
        }
        FrameRow {
            t_s: f.time,
            utc: self.wall(f.time),
            kind,
            baud: f.baud,
            header: Some(p.header),
            count: p.status.number,
            huffman: p.status.huffman,
            break_in: p.status.break_in,
            qrt: p.status.qrt,
            crc_ok: true,
            memory_arq: memory,
            repeat,
            fec: fec_repeat,
            inverted,
            side: self.side,
            text,
            hex: hex(&p.data),
            call,
            snr_db: f.snr_db,
            low_hz: f.low_hz,
        }
    }

    /// Takes one frame. Returns its row.
    pub fn take(&mut self, f: &PactorFrame) -> FrameRow {
        self.last_frame_t = Some(f.time);
        match f.kind {
            FrameKind::Cs(i) => {
                self.c.control_signals += 1;
                self.cs_since_good = true;
                self.run = 0;
                self.fec = false;
                let side = self.side;
                if i == 2 {
                    self.c.changeovers += 1;
                    self.side = if self.side == 1 { 2 } else { 1 };
                }
                let m = proto::cs_meaning(i as usize).to_string();
                FrameRow {
                    t_s: f.time,
                    utc: self.wall(f.time),
                    kind: FrameType::from_index(FrameType::Cs1.index() + u16::from(i.min(3)))
                        .unwrap_or(FrameType::Cs1),
                    baud: 100,
                    header: None,
                    count: 0,
                    huffman: false,
                    break_in: false,
                    qrt: false,
                    crc_ok: true,
                    memory_arq: false,
                    repeat: false,
                    fec: false,
                    inverted: f.inverted,
                    side,
                    text: m,
                    hex: String::new(),
                    call: String::new(),
                    snr_db: f.snr_db,
                    low_hz: f.low_hz,
                }
            }
            FrameKind::Bad => {
                // Memory ARQ: add to an earlier failed packet of the same length within 8 s.
                if let Some((t0, prev)) = self.store.take() {
                    if f.time - t0 <= 8.0 && prev.len() == f.soft.len() {
                        let c: f32 = prev.iter().zip(&f.soft).take(8).map(|(a, b)| a * b).sum();
                        let s0 = if c >= 0.0 { 1.0 } else { -1.0 };
                        for s in [s0, -s0] {
                            let sum: Vec<f32> =
                                prev.iter().zip(&f.soft).map(|(a, b)| a + s * b).collect();
                            if let Some((p, inv)) = decode_soft(&sum) {
                                let ff = PactorFrame {
                                    kind: if p.status.qrt {
                                        FrameKind::Qrt
                                    } else {
                                        FrameKind::Data
                                    },
                                    ..f.clone()
                                };
                                return self.good(&ff, &p, true, inv);
                            }
                        }
                        let sum: Vec<f32> =
                            prev.iter().zip(&f.soft).map(|(a, b)| a + s0 * b).collect();
                        self.store = Some((f.time, sum));
                    } else {
                        self.store = Some((f.time, f.soft.clone()));
                    }
                } else {
                    self.store = Some((f.time, f.soft.clone()));
                }
                self.c.packets_bad += 1;
                FrameRow {
                    t_s: f.time,
                    utc: self.wall(f.time),
                    kind: FrameType::Bad,
                    baud: f.baud,
                    header: None,
                    count: 0,
                    huffman: false,
                    break_in: false,
                    qrt: false,
                    crc_ok: false,
                    memory_arq: false,
                    repeat: false,
                    fec: false,
                    inverted: false,
                    side: self.side,
                    text: String::new(),
                    hex: String::new(),
                    call: String::new(),
                    snr_db: f.snr_db,
                    low_hz: f.low_hz,
                }
            }
            _ => {
                let p = f.packet.clone().expect("a good frame has its packet");
                self.good(f, &p, false, f.inverted)
            }
        }
    }

    fn clear(&mut self) {
        self.lists.clear();
        self.level.clear();
        self.store = None;
        self.publish();
    }

    fn publish(&mut self) {
        let recent = self.last_frame_t.is_some_and(|t| self.now - t <= 30.0);
        let signal = recent || self.pair.is_some();
        let c = &self.c;
        let complete = self.huffman.complete();
        let now = self.now;
        let p = &mut self.p;
        p.put(&Pactor1Fields {
            signal,
            packets_recent: recent,
            packets_ok: c.packets_ok as i64,
            packets_fast: c.fast as i64,
            packets_slow: c.slow as i64,
            packets_bad: c.packets_bad as i64,
            control_signals: c.control_signals as i64,
            repeats: c.repeats as i64,
            memory_arq_saves: c.memory_arq_saves as i64,
            changeovers: c.changeovers as i64,
            connects: c.connects as i64,
            fec_mode: self.fec,
            huffman_complete: complete,
            huffman_entries: self.huffman.entries.len() as i64,
            pair_low: self.pair.map(Hertz),
            pair_high: self.pair.map(|l| Hertz(l + 200.0)),
            offset_hz: self.pair.map(|l| Hertz(l + 100.0 - 1500.0)),
            snr_db: self.pair.map(|_| self.pair_snr),
            level_dbfs: self.level.back().map(|&v| v as f64),
            text: &self.lists.text,
            clock: self.origin.source(),
        });
        shaped::log_rows(&self.lists.frames, &PACTOR1_FRAMES, now, p);
        shaped::rows(&self.lists.stations, &PACTOR1_STATIONS, now, p);
        let mode = if self.fec { "unproto / FEC" } else { "ARQ" };
        let s = &mut self.shown;
        s.summary.clear();
        if c.packets_ok > 0 {
            // Writing to a String does not fail.
            let _ = write!(s.summary, "PACTOR-I: {} packets, {mode}", c.packets_ok);
        } else if self.pair.is_some() {
            s.summary
                .push_str("PACTOR-I: tone pair found, no packet yet");
        } else {
            s.summary.push_str("PACTOR-I: no signal");
        }
        s.lock_state = if recent {
            LockState::Locked
        } else if self.pair.is_some() {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        s.snr_db = self.pair.map(|_| self.pair_snr as f32);
        s.level_dbfs = self.level.back().copied();
        s.offset_hz = self.pair.map(|l| l + 100.0 - 1500.0);
        let r = p.results_mut();
        r.set_status(s);
        update_points(r, self.level_series, true, level_points(&self.level));
    }
}

impl Component for PactorLink {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.pactor_link",
            vec![
                Port::typed::<PactorFrame>("frames"),
                Port::typed::<PactorPair>("pair"),
                Port::real("power"),
            ],
            vec![Port::new("events", Kind::Messages)],
        )
        .with_settings(PactorLinkSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.origin = Origin::new(self.live);
        self.tuned_hz = inputs[2].tuned_hz;
        if !self.configured {
            self.p.results_mut().log(START_LOG);
        }
        self.configured = true;
        self.store = None;
        self.publish();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let power = io.real(2);
        for &p in power {
            if p.is_finite() {
                self.lvl_acc += p as f64;
            }
            self.lvl_n += 1;
            self.lvl_samples += 1;
            if self.lvl_n >= FS as u64 {
                let db = 10.0 * (self.lvl_acc / self.lvl_n as f64).max(1e-15).log10();
                if self.level.len() >= LEVEL_CAP {
                    self.level.pop_front();
                }
                self.level.push_back(db as f32);
                self.lvl_acc = 0.0;
                self.lvl_n = 0;
            }
        }
        self.now = self.lvl_samples as f64 / FS;
        for p in io.items::<PactorPair>(1) {
            let p = &p.value;
            if p.locked {
                if p.new_lock || self.pair.is_none() {
                    self.p.results_mut().log(format!(
                        "PACTOR-I: tone pair {:.0} / {:.0} Hz",
                        p.low_hz,
                        p.low_hz + 200.0
                    ));
                }
                self.pair = Some(p.low_hz);
                self.pair_snr = p.snr_db;
            } else {
                self.pair = None;
            }
        }
        let mut changed = false;
        for f in io.items::<PactorFrame>(0) {
            let row = self.take(&f.value);
            let ev = PACTOR_FRAME.event(
                row.t_s,
                self.tuned_hz,
                &PactorFrameEvent {
                    kind: row.kind,
                    symbol_rate_hz: Hertz(f64::from(row.baud)),
                    header: row.header.map(i64::from),
                    number: row.header.map(|_| i64::from(row.count)),
                    source: None,
                    target: (!row.call.is_empty()).then_some(row.call.as_str()),
                    text: &row.text,
                    side: i64::from(row.side),
                    inverted: row.inverted,
                    repeat: row.repeat,
                    snr_db: row.snr_db,
                    family: Family::Message,
                },
            );
            io.messages_out(0).push(ev);
            self.lists.frames.push(row);
            changed = true;
        }
        if changed || io.tick() {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PactorLinkSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PactorLinkSetting::HuffmanTable => {
                let text = HUFFMAN_TABLE.read(id, v)?;
                self.huffman = if text.trim().is_empty() {
                    Huffman::default()
                } else {
                    Huffman::parse(text).ok_or(SettingError::Malformed { id, at: None })?
                };
                self.huffman_text = text.to_owned();
            }
        }
        self.publish();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            PactorLinkSetting::HuffmanTable,
            Value::Text(self.huffman_text.clone()),
        )])
    }
    fn commands(&self) -> &Commands {
        PactorLinkCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match PactorLinkCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            PactorLinkCommand::Clear => self.clear(),
            PactorLinkCommand::Remove => {
                let table = self
                    .p
                    .results()
                    .decl()
                    .table_id(PACTOR1_STATIONS.key)
                    .expect("the declaration holds the station table");
                objcmd::remove(
                    &mut self.lists.stations,
                    table,
                    &mut self.p,
                    args,
                    &PACTOR1_STATION_ROW,
                )?;
                self.publish();
            }
        }
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.lists.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.lists.restore_state(state, self.now)?;
        self.publish();
        Ok(())
    }
}
