//! DECT base stations (`dect`): the A-fields of the beacons with their R-CRC, the base station identity (RFPI) and the broadcast
//! system information (Qt). The B-field is never decoded, paging and MAC control tails are logged by type only, and nothing is sent.
//!
//! Standard: ETSI EN 300 175-2 (physical layer), EN 300 175-3 clauses 6.2 and 7.2 (A-field, R-CRC, Qt), EN 300 175-6 (RFPI).

use crate::family::{checked, hex, min_rate_message, CleanScene, REPORT_S};
use crate::typed::{
    DectBaseEvent, DectBaseRow, DectBurstRow, DectDemodFields, DectFields, DECT_BASE, DECT_BASES,
    DECT_BASE_ROW, DECT_BURSTS, DECT_REMOVE,
};
use sdrkit_algo_dsp::floor::MinFollower;
use sdrkit_algo_dsp::fm::LagDiscriminator;
use sdrkit_algo_dsp::stats::WindowSum;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst as SimBurst, Scene};
use sdrkit_core::bench::{measure, BenchResult, Benchmark};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, ReadFields, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::AutoGain;
use sdrkit_core::SignalType;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Event, GenConfig, Generator, HostRequest, LockProfile,
    LockState, Receiver, Record, Results, ResultsDecl, Setting, SettingError, SettingId,
    SettingSet, SignalInfo, StackConfig, StackDescriptor, StateBlob, StateError, Status, TableId,
    Tuning, Value, Values,
};
use sdrkit_graph::Catalog;
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Decoder, DecoderComponent, Format,
    GraphBuilder, GraphError, Io, Item, Kept, Kind, Outbox, Port, RateFrom, StackGraph, Timed,
};
use sdrkit_util_bits::crc::catalog::CRC16_DECT_R;
use sdrkit_util_bits::pack::unpack;
use sdrkit_util_bits::BitOrder;
use sdrkit_util_objects::commands;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp, Tables,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;

/// The S-field of a base station (preamble and sync), first bit sent most significant.
pub const SYNC_RFP: u32 = 0xAAAA_E98A;
/// The S-field of a handset.
pub const SYNC_PP: u32 = 0x5555_1675;
/// The bit rate, bit/s.
pub const BIT_RATE: f64 = 1.152e6;
/// The rate the demodulator runs at: 4 samples a bit.
pub const DEMOD_RATE: f64 = 4.608e6;
/// The width of the channel filter, Hz (650 kHz on each side).
pub const CHANNEL_WIDTH: f64 = 1.3e6;
/// The base stations kept.
pub const BASE_CAP: usize = 60;
/// The bursts kept in the log.
pub const LOG_CAP: usize = 300;

/// The R-CRC of the header and tail `ht` (6 bytes). EN 300 175-3 clause 6.2.5: CRC-16/DECT-R, whose last bit is inverted.
pub fn r_crc(ht: &[u8]) -> u16 {
    CRC16_DECT_R.checksum(ht)
}

/// Whether the A-field `a` (header, tail and R-CRC, 8 bytes) has a good R-CRC. Only the R-CRC itself passes.
pub fn rcrc_ok(a: &[u8; 8]) -> bool {
    r_crc(&a[..6]) == u16::from_be_bytes([a[6], a[7]])
}

/// Builds an A-field: header (TA, Q1, BA, Q2 0), the 40-bit tail and the R-CRC. With `good_crc` false the R-CRC is wrong.
pub fn afield(ta: u8, q1: bool, ba: u8, tail: u64, good_crc: bool) -> [u8; 8] {
    let mut b = [0u8; 8];
    b[0] = (ta << 5) | ((q1 as u8) << 4) | ((ba & 7) << 1);
    b[1..6].copy_from_slice(&tail.to_be_bytes()[3..]);
    let c = r_crc(&b[..6]);
    let r = if good_crc { c } else { c ^ 0x0F0F };
    b[6..].copy_from_slice(&r.to_be_bytes());
    b
}

/// The parts of an RFPI: (E, ARC, Some((EMC, FPN, RPN)) for class A, else None). EN 300 175-6 clause 5.
pub fn rfpi_split(r: u64) -> (bool, u8, Option<(u16, u32, u8)>) {
    let e = (r >> 39) & 1 != 0;
    let arc = ((r >> 36) & 7) as u8;
    if arc == 0 {
        (
            e,
            arc,
            Some((
                ((r >> 20) & 0xFFFF) as u16,
                ((r >> 3) & 0x1FFFF) as u32,
                (r & 7) as u8,
            )),
        )
    } else {
        (e, arc, None)
    }
}

/// The name of a tail type (TA).
pub fn ta_name(ta: u8) -> &'static str {
    [
        "Ct (data 0)",
        "Ct (data 1)",
        "Nt (identity)",
        "Nt (connectionless)",
        "Qt (system info)",
        "escape",
        "Mt (MAC control)",
        "Pt (paging)",
    ][(ta & 7) as usize]
}

/// The name of the Q header of a Qt tail. EN 300 175-3 clause 7.2.3.1.
pub fn q_name(qh: u8) -> &'static str {
    match qh {
        0 | 1 => "static system information",
        2 => "extended RF carriers",
        3 => "fixed part capabilities",
        4 => "extended fixed part capabilities",
        5 => "SARI list contents",
        6 => "multiframe number",
        7 => "escape",
        9 => "transmit information",
        12 => "extended fixed part capabilities part 2",
        _ => "reserved",
    }
}

/// The decoded fields of a Qt tail as (name, value) pairs. EN 300 175-3 clause 7.2.3.2 (static system information).
pub fn qt_fields(tail: u64) -> Vec<(String, String)> {
    let bits = |from: u32, n: u32| -> u64 { (tail >> (40 - from - n)) & ((1 << n) - 1) };
    let qh = bits(0, 4) as u8;
    let mut v = vec![("Q header".to_string(), q_name(qh).to_string())];
    match qh {
        0 | 1 => {
            v.push(("Normal/reverse".into(), bits(4, 1).to_string()));
            v.push(("Slot number".into(), bits(5, 4).to_string()));
            v.push(("Start position".into(), bits(9, 2).to_string()));
            v.push(("Escape".into(), bits(11, 1).to_string()));
            v.push(("Transceivers".into(), bits(12, 2).to_string()));
            v.push(("Extended RF carriers".into(), bits(14, 1).to_string()));
            v.push((
                "RF carriers available".into(),
                format!("{:010b}", bits(15, 10)),
            ));
            v.push(("Carrier number".into(), bits(25, 6).to_string()));
            v.push(("Primary scan carrier".into(), bits(31, 6).to_string()));
        }
        6 => v.push(("Multiframe number".into(), bits(16, 24).to_string())),
        _ => {}
    }
    v
}

/// One A-field with a good R-CRC.
#[derive(Clone, Debug, PartialEq)]
pub struct AField {
    /// From a base station (true) or a handset.
    pub base: bool,
    /// The 8 bytes (header, tail, R-CRC).
    pub bytes: [u8; 8],
    /// The level over S- and A-field, dBFS.
    pub level_dbfs: f32,
    /// The SNR, dB.
    pub snr_db: f32,
    /// The carrier offset from the channel, Hz.
    pub offset_hz: f32,
    /// The carrier frequency: the channel plus the offset, Hz.
    pub freq_hz: f64,
}

/// The output of the demodulator (`dect.afield`).
#[derive(Clone, Debug, PartialEq)]
pub enum Burst {
    /// A good A-field.
    Good(AField),
    /// A burst whose S-field was found (from a base station: true) but whose A-field did not pass.
    Bad(bool),
}

impl Item for Burst {
    const KIND: &'static str = "dect.afield";
}

/// The size of the history rings of the demodulator, samples (a power of two).
const RING: usize = 1 << 14;
/// The rise of the power floor per sample, dB: a factor of 1 + 2e-7.
const FLOOR_RISE_DB: f32 = 8.685_889e-7;
/// The least power floor: -80 dBFS, below the noise of any radio. Without it a signal with no noise at all triggers on the
/// ringing of the channel filter before each burst.
const MIN_FLOOR: f32 = 1e-8;
/// The share of a drop the power floor follows at each sample. Half a drop a sample follows the minimum within a few samples, and
/// it never rounds the floor to 0 (a full step `f + (x - f)` in `f32` can).
const FLOOR_ATTACK: f32 = 0.5;
/// The alignments tried before a candidate, samples.
const SEARCH_BEFORE: i64 = 80;
/// The alignments tried after a candidate, samples. The trigger fires early on a strong signal with little noise (the rise of the
/// channel filter reaches the trigger level sooner), so the S-field can end later than the candidate.
const SEARCH_AFTER: i64 = 160;

/// One alignment of the S-field: (end sample, eye, slicing level, bit errors).
type Alignment = (i64, f32, f32, u32);

/// The demodulator (`dect.demod`): the phase step over one bit, the burst trigger, the S-field and the A-field.
pub struct Demod {
    rate: f64,
    channel_hz: f64,
    /// The phase step over one bit (4 samples), Hz.
    disc: Option<LagDiscriminator>,
    f: Vec<f32>,
    p: Vec<f32>,
    pos: u64,
    pow128: WindowSum<f32>,
    floor: MinFollower,
    floor_now: f32,
    armed: bool,
    pending: VecDeque<u64>,
    busy_until: u64,
    alignments: Vec<Alignment>,
    good: u64,
    bad: u64,
}

impl Default for Demod {
    fn default() -> Self {
        Demod {
            rate: DEMOD_RATE,
            channel_hz: 0.0,
            disc: None,
            f: vec![0.0; RING],
            p: vec![0.0; RING],
            pos: 0,
            pow128: WindowSum::new(128).expect("a window of 128"),
            floor: MinFollower::new(FLOOR_ATTACK, FLOOR_RISE_DB).expect("a valid follower"),
            floor_now: f32::MAX,
            armed: true,
            pending: VecDeque::with_capacity(64),
            busy_until: 0,
            alignments: Vec::with_capacity((SEARCH_BEFORE + SEARCH_AFTER) as usize + 1),
            good: 0,
            bad: 0,
        }
    }
}

impl Demod {
    fn at(&self, t: i64) -> f32 {
        self.f[(t as usize) & (RING - 1)]
    }

    /// Fills the alignments of `sync` around `cand`, widest eye first.
    fn align(&mut self, cand: u64, sync: u32) {
        let mut al = std::mem::take(&mut self.alignments);
        al.clear();
        for sh in -SEARCH_BEFORE..=SEARCH_AFTER {
            let end = cand as i64 + sh;
            let (mut a, mut b) = (0f32, 0f32);
            for k in 0..32 {
                let v = self.at(end - 4 * (31 - k));
                if (sync >> (31 - k)) & 1 != 0 {
                    a += v;
                } else {
                    b += v;
                }
            }
            let ones = sync.count_ones() as f32;
            let (one, zero) = (a / ones, b / (32.0 - ones));
            let mid = (one + zero) / 2.0;
            let errs = (0..32)
                .filter(|&k| (self.at(end - 4 * (31 - k)) > mid) != ((sync >> (31 - k)) & 1 != 0))
                .count() as u32;
            al.push((end, one - zero, mid, errs));
        }
        al.sort_by(|x, y| y.1.total_cmp(&x.1));
        self.alignments = al;
    }

    /// Reads the burst whose trigger produced candidate `cand`.
    fn try_burst(&mut self, cand: u64, out: &mut Outbox) {
        let mut best_any: Option<(bool, u32)> = None;
        for (base, sync) in [(true, SYNC_RFP), (false, SYNC_PP)] {
            self.align(cand, sync);
            if let Some(e) = self.alignments.iter().map(|x| x.3).min() {
                best_any = match best_any {
                    Some((bb, be)) if be <= e => Some((bb, be)),
                    _ => Some((base, e)),
                };
            }
            for i in 0..self.alignments.len() {
                let (end, eye, mid, errs) = self.alignments[i];
                if eye <= 0.0 {
                    break;
                }
                let mut bytes = [0u8; 8];
                for j in 0..64i64 {
                    if self.at(end + 4 * (j + 1)) > mid {
                        bytes[(j / 8) as usize] |= 0x80 >> (j % 8);
                    }
                }
                if errs > 3 || !rcrc_ok(&bytes) {
                    continue;
                }
                let dev = eye / 2.0;
                let mut var = 0f64;
                for j in -31..65i64 {
                    let v = self.at(end + 4 * j);
                    let ideal = if v > mid { mid + dev } else { mid - dev };
                    var += ((v - ideal) as f64).powi(2);
                }
                let var = var / 96.0;
                let p0 = (end - 4 * 32).max(0);
                let p1 = end + 4 * 64;
                let lvl = (p0..p1)
                    .map(|t| self.p[(t as usize) & (RING - 1)] as f64)
                    .sum::<f64>()
                    / (p1 - p0) as f64;
                self.busy_until = p1 as u64;
                self.good += 1;
                out.items::<Burst>(0).push(Timed {
                    time: p0 as f64 / self.rate,
                    value: Burst::Good(AField {
                        base,
                        bytes,
                        level_dbfs: (10.0 * lvl.max(1e-20).log10()) as f32,
                        snr_db: (10.0 * ((dev as f64).powi(2) / var.max(1e-6)).log10()) as f32,
                        offset_hz: mid,
                        freq_hz: self.channel_hz + mid as f64,
                    }),
                });
                return;
            }
        }
        if let Some((base, e)) = best_any {
            if e <= 1 {
                self.bad += 1;
                out.items::<Burst>(0).push(Timed {
                    time: cand as f64 / self.rate,
                    value: Burst::Bad(base),
                });
            }
        }
    }
}

impl Decoder for Demod {
    type Input = Complex32;

    fn name(&self) -> &'static str {
        "dect.demod"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<Burst>("afields").rate_from(RateFrom::Fixed(0.0))]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Published {
        Published::new(DectDemodFields::SHAPE.fields)
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        let (good, bad) = (self.good, self.bad);
        *self = Demod {
            good,
            bad,
            ..Demod::default()
        };
        self.rate = input.rate;
        self.channel_hz = input.tuned_hz;
        self.disc = Some(LagDiscriminator::new(self.rate, 1.0, 4)?);
        let mut o = input.with_kind(Kind::typed::<Burst>());
        o.rate = 0.0;
        Ok(vec![o])
    }
    fn retune(&mut self, input: &Format) {
        self.channel_hz = input.tuned_hz;
        self.pending.clear();
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        let _ = first;
        for &s in x {
            let i = (self.pos as usize) & (RING - 1);
            self.f[i] = self.disc.as_mut().map_or(0.0, |d| d.process_sample(s));
            let pw = s.norm_sqr();
            self.p[i] = pw;
            let avg = (self.pow128.push(pw) / 128.0) as f32;
            self.pos += 1;
            if self.pos > 128 {
                self.floor_now = self
                    .floor
                    .push(avg.max(MIN_FLOOR))
                    .unwrap_or(self.floor_now);
                if self.armed && avg > 6.0 * self.floor_now && self.pos > self.busy_until {
                    self.armed = false;
                    // The S-field ends about 22 bits after the trigger.
                    self.pending.push_back(self.pos + 22 * 4);
                } else if !self.armed && avg < 3.0 * self.floor_now {
                    self.armed = true;
                }
            }
            while let Some(&c) = self.pending.front() {
                if self.pos < c + SEARCH_AFTER as u64 + 4 * 66 {
                    break;
                }
                self.pending.pop_front();
                self.try_burst(c, out);
            }
        }
    }
    fn report(&mut self, r: &mut Published) {
        r.put(&DectDemodFields {
            frames_ok: self.good as i64,
            frames_bad: self.bad as i64,
        });
    }
    fn clear(&mut self) {
        self.good = 0;
        self.bad = 0;
    }
    fn forget(&mut self, kept: &Kept) {
        self.pending.clear();
        if !kept.field("frames_ok") {
            self.good = 0;
        }
        if !kept.field("frames_bad") {
            self.bad = 0;
        }
    }
}

/// The DECT bands: (setting value, name, carriers in ascending frequency).
pub fn bands() -> [(&'static str, &'static str, Vec<f64>); 5] {
    let sp = 1.728e6;
    [
        (
            "eu",
            "Europe",
            (0..10).rev().map(|n| 1897.344e6 - sp * n as f64).collect(),
        ),
        (
            "us",
            "US / Canada (DECT 6.0)",
            (0..5).map(|n| 1881.792e6 + sp * (23 + n) as f64).collect(),
        ),
        (
            "br",
            "Brazil",
            (0..5).map(|n| 1881.792e6 + sp * (17 + n) as f64).collect(),
        ),
        (
            "la",
            "Latin America",
            (0..11).map(|n| 1881.792e6 + sp * (17 + n) as f64).collect(),
        ),
        (
            "ism900",
            "US ISM 900",
            (0..14).map(|n| 903.744e6 + sp * n as f64).collect(),
        ),
    ]
}

/// The band and carrier number of a frequency: the carrier whose centre is within 0.2 spacings. The carrier number counts from
/// 0 at the first entry of the band (EN 300 175-2 numbers the European carriers down from 1897.344 MHz).
pub fn carrier_of(hz: f64) -> Option<(&'static str, u32)> {
    let sp = 1.728e6;
    for (key, name, ch) in bands() {
        let n = ch.len();
        if let Some(i) = ch.iter().position(|&c| (c - hz).abs() <= 0.2 * sp) {
            let number = if key == "eu" { n - 1 - i } else { i };
            return Some((name, number as u32));
        }
    }
    None
}

/// A base station.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Base {
    /// The RFPI.
    pub rfpi: u64,
    /// The E bit.
    pub e: bool,
    /// The access rights class.
    pub arc: u8,
    /// The equipment manufacturer code (class A).
    pub emc: Option<u16>,
    /// The fixed part number (class A).
    pub fpn: Option<u32>,
    /// The radio fixed part number (class A).
    pub rpn: Option<u8>,
    /// The slots seen, one bit for each of the 24 slots.
    pub slots: u32,
    /// The level of the last beacon, dBFS.
    pub level_dbfs: f32,
    /// The carrier frequency, Hz.
    pub freq_hz: f64,
    /// Nt tails seen.
    pub nt: u64,
    /// Qt tails seen.
    pub qt: u64,
    /// The last Qt tail.
    pub last_qt: Option<u64>,
    /// The frame phase: signal time modulo 10 ms, s.
    pub phase: f64,
}

impl Shift for Base {
    fn shift_times(&mut self, by_s: f64) {
        self.phase = (self.phase + by_s).rem_euclid(0.01);
    }
}

/// A line of the burst log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BurstLine {
    /// The signal time, s.
    pub time_s: f64,
    /// From a base station (true) or a handset.
    pub base: bool,
    /// The tail type.
    pub ta: u8,
    /// The tail.
    pub tail: u64,
    /// The level, dBFS.
    pub level_dbfs: f32,
}

impl Shift for BurstLine {
    fn shift_times(&mut self, by_s: f64) {
        self.time_s += by_s;
    }
}

impl Shaped for Base {
    type Row = DectBaseRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let rfpi = format!("{:010X}", self.rfpi);
        let slots = format!("{:024b}", self.slots);
        let last_qt = self.last_qt.map(|q| format!("{q:010X}"));
        let info = self.last_qt.map(|q| {
            qt_fields(q)
                .into_iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join(", ")
        });
        out.put(&DectBaseRow {
            rfpi: &rfpi,
            arc: ["A", "B", "C", "D", "E", "?", "?", "?"][self.arc as usize & 7],
            emc: self.emc.map(i64::from),
            fpn: self.fpn.map(i64::from),
            rpn: self.rpn.map(i64::from),
            slots: &slots,
            level_dbfs: f64::from(self.level_dbfs),
            frequency: Hertz(self.freq_hz),
            last_qt: last_qt.as_deref(),
            system_info: info.as_deref(),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

impl Shaped for BurstLine {
    type Row = DectBurstRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&DectBurstRow {
            time_s: self.time_s,
            from: if self.base { "base" } else { "handset" },
            tail_type: ta_name(self.ta),
            tail: &hex(&self.tail.to_be_bytes()[3..]),
            level_dbfs: f64::from(self.level_dbfs),
        });
    }
}

/// What `dect` keeps: the base stations and the burst log.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The base stations, keyed by RFPI.
    pub bases: ObjectTable<u64, Base>,
    /// The burst log.
    pub log: LogRing<BurstLine>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            bases: ObjectTable::new(BASE_CAP),
            log: LogRing::new(LOG_CAP),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.bases.shift_times(by_s);
        self.log.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("ism.dect", 1);
    fn clear(&mut self) {
        self.bases.clear();
        self.log.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.bases.adopt(saved.bases);
        self.log.adopt(saved.log);
    }
}

sdrkit_core::choice_enum! {
    /// The DECT bands. The keys are those of [`bands`].
    pub enum DectBand {
        /// Europe.
        Eu = ("eu", "Europe"),
        /// US and Canada (DECT 6.0).
        Us = ("us", "US / Canada (DECT 6.0)"),
        /// Brazil.
        Br = ("br", "Brazil"),
        /// Latin America.
        La = ("la", "Latin America"),
        /// US ISM 900.
        Ism900 = ("ism900", "US ISM 900"),
    }
}

const BAND_SETTING: Setting<field::Choice<DectBand>> =
    Setting::choice("band", "Band", DectBand::Eu);
const SCAN: Setting<field::Bool> = Setting::new("scan", "Scan the band's carriers", "", false)
    .help("Asks the host to visit each carrier of the band in turn.");
const DWELL: Setting<field::Duration> =
    Setting::new("dwell", "Scan dwell", "s", 5.0).range(1.0, 60.0, 1.0);

sdrkit_core::settings! {
    /// The settings of [`Bases`].
    pub enum BasesSetting {
        /// The band.
        Band = BAND_SETTING,
        /// Scan the band.
        Scan = SCAN,
        /// The scan dwell.
        Dwell = DWELL,
    }
}

sdrkit_core::commands! {
    /// The commands of [`Bases`].
    pub enum BasesCommand {
        /// Forgets everything heard.
        Clear = commands::CLEAR,
        /// Forgets one base station.
        Remove = DECT_REMOVE.command,
    }
}

/// The base station table (`dect.bases`): base stations, the burst log, the status, the events and the scan request.
pub struct Bases {
    store: Store,
    published: Published,
    bases_table: TableId,
    events: Vec<Event>,
    requests: Vec<HostRequest>,
    channel_hz: f64,
    snr: f32,
    offset: f32,
    base_bursts: u64,
    handset_bursts: u64,
    good: u64,
    now: f64,
    last_good: f64,
    band: DectBand,
    scan: bool,
    dwell_s: f64,
    configured: bool,
}

impl Default for Bases {
    fn default() -> Self {
        let published = Published::from_decl(Arc::new(
            ResultsDecl::new()
                .with_fields(DectFields::SHAPE.fields)
                .with_table(&DECT_BASES)
                .with_table(&DECT_BURSTS),
        ));
        let bases_table = published
            .results()
            .decl()
            .table_id(DECT_BASES.key)
            .expect("the tables of dect.bases are declared");
        Bases {
            store: Store::default(),
            published,
            bases_table,
            events: Vec::new(),
            requests: Vec::new(),
            channel_hz: 0.0,
            snr: 0.0,
            offset: 0.0,
            base_bursts: 0,
            handset_bursts: 0,
            good: 0,
            now: 0.0,
            last_good: f64::NEG_INFINITY,
            band: DectBand::Eu,
            scan: false,
            dwell_s: 5.0,
            configured: false,
        }
    }
}

impl Bases {
    /// The base stations held.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The scan request for the band (a hop over its carriers), or a retune to the channel when the scan stops.
    fn scan_request(&mut self) {
        if !self.configured {
            return;
        }
        if self.scan {
            let channels = bands()
                .into_iter()
                .find(|b| b.0 == DectBand::CHOICES[usize::from(self.band.index())].key)
                .map(|b| b.2)
                .unwrap_or_default();
            self.requests.push(HostRequest::Hop {
                channels,
                dwell_s: self.dwell_s,
                stop_on_lock: false,
            });
        } else if self.channel_hz > 0.0 {
            self.requests
                .push(HostRequest::retune(self.channel_hz, "DECT scan stopped"));
        }
    }

    fn afield(&mut self, t: f64, a: &AField) {
        self.good += 1;
        self.now = self.now.max(t);
        self.last_good = t;
        self.snr = 0.8 * self.snr + 0.2 * a.snr_db;
        self.offset = 0.9 * self.offset + 0.1 * a.offset_hz;
        let ta = a.bytes[0] >> 5;
        let mut tb = [0u8; 8];
        tb[3..].copy_from_slice(&a.bytes[1..6]);
        let tail = u64::from_be_bytes(tb);
        if a.base {
            self.base_bursts += 1;
        } else {
            self.handset_bursts += 1;
        }
        self.store.log.push(BurstLine {
            time_s: t,
            base: a.base,
            ta,
            tail,
            level_dbfs: a.level_dbfs,
        });
        if !a.base {
            return;
        }
        let phase = t.rem_euclid(0.01);
        let key = if ta == 2 || ta == 3 {
            Some(tail)
        } else {
            self.store
                .bases
                .iter()
                .find(|r| {
                    let d = (phase - r.record.phase).rem_euclid(0.01);
                    d.min(0.01 - d) <= 150e-6
                })
                .map(|r| *r.key)
        };
        let Some(key) = key else { return };
        let Ok(up) = self.store.bases.upsert(&key, Stamp::at(t), Base::default) else {
            return;
        };
        let b = up.row;
        if up.new {
            let (e, arc, cl) = rfpi_split(tail);
            *b = Base {
                rfpi: tail,
                e,
                arc,
                emc: cl.map(|c| c.0),
                fpn: cl.map(|c| c.1),
                rpn: cl.map(|c| c.2),
                phase,
                ..Base::default()
            };
            let emc = cl.map_or("-".to_string(), |c| c.0.to_string());
            let rfpi = format!("{tail:010X}");
            self.events.push(DECT_BASE.event(
                t,
                a.freq_hz,
                &DectBaseEvent {
                    source: &rfpi,
                    text: &format!("DECT: new base station {rfpi} (manufacturer code {emc})"),
                    family: Family::Sensor,
                    rfpi: &rfpi,
                    emc: cl.map(|c| i64::from(c.0)),
                },
            ));
        }
        b.level_dbfs = a.level_dbfs;
        b.freq_hz = a.freq_hz;
        if ta == 2 || ta == 3 {
            b.nt += 1;
            b.phase = phase;
        }
        if ta == 4 {
            b.qt += 1;
            b.last_qt = Some(tail);
        }
        // The slot in the 10 ms frame: signal time modulo 10 ms in 1/24 steps.
        let slot = (phase / (0.01 / 24.0)).round() as u32 % 24;
        b.slots |= 1 << slot;
    }

    fn publish(&mut self) {
        let locked = self.good > 0 && self.now - self.last_good < 5.0;
        let n = self.store.bases.len();
        let summary = if self.good == 0 {
            "listening for DECT on the carrier".to_string()
        } else {
            format!(
                "{n} base stations, {} base / {} handset bursts",
                self.base_bursts, self.handset_bursts
            )
        };
        let carrier = carrier_of(self.channel_hz);
        let p = &mut self.published;
        p.results_mut().set_status(&Status {
            lock_state: LockState::from_level(if locked { 2 } else { 0 }),
            snr_db: (self.good > 0).then_some(self.snr),
            level_dbfs: None,
            offset_hz: (self.good > 0).then_some(f64::from(self.offset)),
            summary,
        });
        p.put(&DectFields {
            bases: n as i64,
            base_bursts: self.base_bursts as i64,
            handset_bursts: self.handset_bursts as i64,
            carrier: Hertz(self.channel_hz),
            band: carrier.map(|c| c.0),
            carrier_number: carrier.map(|c| i64::from(c.1)),
        });
        shaped::rows(&self.store.bases, &DECT_BASES, self.now, p);
        shaped::log_rows(&self.store.log, &DECT_BURSTS, self.now, p);
    }
}

impl Component for Bases {
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dect.bases",
            vec![Port::typed::<Burst>("in")],
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(BasesSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.channel_hz = inputs.first().map_or(0.0, |f| f.tuned_hz);
        self.configured = true;
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut new = false;
        for b in io.items::<Burst>(0) {
            match &b.value {
                Burst::Good(a) => self.afield(b.time, a),
                Burst::Bad(base) => {
                    if *base {
                        self.base_bursts += 1;
                    } else {
                        self.handset_bursts += 1;
                    }
                }
            }
            new = true;
        }
        let tick = io.tick();
        if tick {
            self.now += REPORT_S;
        }
        if new || tick {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
        let now = self.now;
        for r in self.requests.drain(..) {
            io.items_out::<HostRequest>(1).push(Timed {
                time: now,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BasesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BasesSetting::Band => {
                self.band = BAND_SETTING.read(id, v)?;
                if self.scan {
                    self.scan_request();
                }
            }
            BasesSetting::Scan => {
                let on = SCAN.read(id, v)?;
                let changed = on != self.scan;
                self.scan = on;
                if changed {
                    self.scan_request();
                }
            }
            BasesSetting::Dwell => {
                self.dwell_s = DWELL.read(id, v)?;
                if self.scan {
                    self.scan_request();
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (BasesSetting::Band, Value::Choice(self.band.index())),
            (BasesSetting::Scan, Value::Bool(self.scan)),
            (BasesSetting::Dwell, Value::Duration(self.dwell_s)),
        ])
    }
    fn commands(&self) -> &Commands {
        BasesCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match BasesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            BasesCommand::Clear => {
                self.store.clear();
                self.base_bursts = 0;
                self.handset_bursts = 0;
                self.good = 0;
                self.published.clear_rows(&DECT_BASES);
                self.published.clear_rows(&DECT_BURSTS);
            }
            BasesCommand::Remove => commands::remove(
                &mut self.store.bases,
                self.bases_table,
                &mut self.published,
                args,
                &DECT_BASE_ROW,
            )?,
        }
        self.publish();
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(DECT_BASES.key) {
            self.store.bases.clear();
            self.published.clear_rows(&DECT_BASES);
        }
        if !kept.table(DECT_BURSTS.key) {
            self.store.log.clear();
            self.published.clear_rows(&DECT_BURSTS);
        }
        if !kept.field("base_bursts") {
            self.base_bursts = 0;
        }
        if !kept.field("handset_bursts") {
            self.handset_bursts = 0;
        }
        self.good = 0;
        self.snr = 0.0;
        self.offset = 0.0;
        self.last_good = f64::NEG_INFINITY;
        self.publish();
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.now)?;
        self.publish();
        Ok(())
    }
}

/// The `dect` receiver: the carrier to 4.608 Msps, the demodulator and the base station table.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among
/// them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add(
            "front",
            checked(
                "front",
                sdrkit_components::dsp::Channel::new(CHANNEL_WIDTH, DEMOD_RATE),
            )?,
        )
        .set("front.exact", Value::Bool(true))
        .add("demod", DecoderComponent::new(Demod::default()))
        .add("bases", Bases::default())
        .input("front")
        .connect("front", "demod")
        .connect("demod.afields", "bases.in")
        .output("bases.events")
        .output("bases.requests")
        .clock(REPORT_S)
        .status_from("bases")
        .build()?;
    StackGraph::of(crate::typed::shape("dect"), graph)
        .min_rate(
            DEMOD_RATE,
            &min_rate_message("DECT cordless phones", DEMOD_RATE),
        )
        .route(BAND_SETTING.key(), &["bases.band"])?
        .route(SCAN.key(), &["bases.scan"])?
        .route(DWELL.key(), &["bases.dwell"])?
        .command("clear", sdrkit_graph::CommandTarget::of("bases", "clear"))
        .command("remove", sdrkit_graph::CommandTarget::of("bases", "remove"))
        .with_settings(&cfg.settings)
        .build()
}

sdrkit_core::choice_enum! {
    /// The base stations of the test signal.
    pub enum GenBases {
        /// Two bases with handsets.
        Two = ("0", "Two bases with handsets"),
        /// One base.
        One = ("1", "One base"),
        /// No base.
        None = ("2", "No base"),
    }
}

/// The base stations of the test signal.
pub const GEN_OPTION: Setting<field::Choice<GenBases>> =
    Setting::choice("gen_option", "Test signal bases", GenBases::Two)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `dect` stack.
    pub enum DectSetting {
        /// The band.
        Band = BAND_SETTING,
        /// Scan the band.
        Scan = SCAN,
        /// The scan dwell.
        Dwell = DWELL,
        /// The base stations of the test signal.
        GenOption = GEN_OPTION,
    }
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// A test base station of the generator.
#[derive(Clone, Debug)]
pub struct TestBase {
    /// The RFPI.
    pub rfpi: u64,
    /// The slot, 0 to 11.
    pub slot: u32,
    /// The level, dB.
    pub level_db: f64,
    /// The R-CRC is right (false: deliberately wrong).
    pub good_crc: bool,
    /// A handset answers 12 slots later.
    pub handset: bool,
}

/// The base stations of a generator option (0, 1 or 2).
pub fn test_bases(option: u32) -> Vec<TestBase> {
    let a = TestBase {
        rfpi: 0x0100A5C3D1,
        slot: 0,
        level_db: 0.0,
        good_crc: true,
        handset: true,
    };
    match option {
        1 => vec![TestBase {
            handset: false,
            ..a
        }],
        2 => vec![],
        _ => vec![
            a,
            TestBase {
                rfpi: 0x0100F31A22,
                slot: 5,
                level_db: -4.4,
                good_crc: true,
                handset: true,
            },
        ],
    }
}

/// The levels of one burst for the modulator: the S-field, the A-field and 320 random B-field bits.
pub fn burst_levels(sync: u32, a: &[u8; 8], rng: &mut Rng, out: &mut Vec<f32>) {
    let mut bits = Vec::with_capacity(32 + 64 + 320);
    unpack(&sync.to_be_bytes(), BitOrder::Msb, &mut bits);
    unpack(a, BitOrder::Msb, &mut bits);
    bits.extend((0..320).map(|_| rng.bit()));
    out.extend(bits.iter().map(|&b| if b != 0 { 1.0 } else { -1.0 }));
}

/// The settings of the generator.
#[derive(Clone, Debug)]
pub struct GenParams {
    /// The base stations.
    pub bases: Vec<TestBase>,
    /// The seed of the content.
    pub seed: u64,
}

/// The `dect` generator at `rate` samples a second: each base sends its beacon once every 10 ms frame in its slot (Nt with the
/// RFPI on even frames, Qt on odd frames), and its handset answers 12 slots later, GFSK BT 0.5 at ±288 kHz. The signal is
/// clean: noise and the receiver's errors come from the `gen_*` settings.
pub fn generator(rate: f64, p: GenParams) -> CleanScene {
    let rate = if rate > 0.0 && rate.is_finite() {
        rate
    } else {
        DEMOD_RATE
    };
    let info = SignalInfo::new(BIT_RATE)
        .with_symbol_rate(BIT_RATE)
        .with_bit_rate(BIT_RATE);
    let mut scene = CleanScene::new(Scene::new(rate).expect("a positive rate"), rate, info);
    let mut stream = 0u64;
    for b in p.bases {
        for base in [true, false] {
            if !base && !b.handset {
                continue;
            }
            let Ok(mut cpm) = Cpm::gfsk(0.5, 2.0 * 288e3 / BIT_RATE, rate / BIT_RATE) else {
                continue;
            };
            stream += 1;
            let mut rng = Rng::stream(p.seed, stream);
            let mut frame = 0u64;
            let mut levels = Vec::new();
            let b = b.clone();
            let source = move || {
                let slot = b.slot + if base { 0 } else { 12 };
                let t = frame as f64 * 0.01 + slot as f64 * 0.01 / 24.0 + 100.0 / rate;
                let (sync, a, level_db) = if base {
                    let a = if frame.is_multiple_of(2) {
                        afield(3, true, 0, b.rfpi, b.good_crc)
                    } else {
                        afield(4, true, 0, 0x0123456789, b.good_crc)
                    };
                    (SYNC_RFP, a, b.level_db)
                } else {
                    let tail = rng.next_u64() & 0xFF_FFFF_FFFF;
                    (SYNC_PP, afield(0, false, 0, tail, true), b.level_db - 3.0)
                };
                levels.clear();
                burst_levels(sync, &a, &mut rng, &mut levels);
                let mut samples = Vec::new();
                cpm.push(&levels, &mut samples);
                cpm.flush(&mut samples);
                frame += 1;
                Some(SimBurst {
                    start_s: t,
                    offset_hz: 0.0,
                    level_db,
                    samples,
                })
            };
            scene.scene_mut().add_source(Box::new(source));
        }
    }
    scene
}

/// The settings of the generator from a configuration.
pub fn gen_params(cfg: &GenConfig) -> GenParams {
    let option = match cfg.settings.get(&GEN_OPTION) {
        GenBases::Two => 0,
        GenBases::One => 1,
        GenBases::None => 2,
    };
    GenParams {
        bases: test_bases(option),
        seed: cfg.seed,
    }
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(generator(cfg.rate_hz, gen_params(cfg))))
}

/// The time the demodulator takes for 0.1 s of the two-base signal at 4.608 Msps (`ism.dect_demod`).
pub fn benchmark() -> Benchmark {
    Benchmark {
        name: "ism.dect_demod",
        run: |seconds| -> BenchResult {
            let mut g = generator(
                DEMOD_RATE,
                GenParams {
                    bases: test_bases(0),
                    seed: 1,
                },
            );
            let mut x = vec![Complex32::default(); (DEMOD_RATE * 0.1) as usize];
            g.generate(&mut x);
            let mut d = Demod::default();
            let _ = d.configure(&Format::iq(DEMOD_RATE, 0.0, 0.0));
            let mut out = Outbox::new(&[Kind::typed::<Burst>()]);
            let mut first = 0u64;
            measure(seconds, x.len(), 0.1, || {
                d.process(&x, first, &mut out);
                first += x.len() as u64;
                out.items::<Burst>(0).clear();
            })
        },
    }
}

/// The verify case: the two test bases.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, two bases with handsets", o, 0.0);
    c.seconds = o.seconds.max(1.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = DectFields::read(r.fields());
        let bases = f.as_ref().map_or(0, |f| f.bases);
        let handset = f.as_ref().map_or(0, |f| f.handset_bursts);
        vec![
            CheckLine::new("bases", bases >= 2, format!("{bases}, 2 expected")),
            CheckLine::new(
                "handset_bursts",
                handset >= 1,
                format!("{handset}, at least 1 expected"),
            ),
        ]
    });
    vec![c]
}

/// DECT cordless phones.
pub static DECT: StackDescriptor = StackDescriptor {
    id: "dect",
    name: "DECT cordless phones",
    group: "Digital voice",
    blurb: "DECT base stations: their identities (RFPI), slots and broadcast system information; no call content is decoded.",
    tuning: Tuning { min_hz: 900e6, max_hz: 1930e6, tune_offset_hz: 1e6, ..Tuning::anywhere(1897.344e6, 5e6, 4.608e6, 1.728e6) },
    submodes: &[],
    settings: DectSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Dect],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};
