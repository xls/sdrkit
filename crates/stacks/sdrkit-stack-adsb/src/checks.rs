//! Frame checks and error correction: the format and its length, the parity, known addresses, the plausibility of
//! address/parity replies, and the repair of up to three bits. [`FrameChecker`] is the plain type, [`FrameChecks`] the component.
//!
//! Standard: ICAO Annex 10 Vol. IV 3.1.2.3.3

use crate::codes::ac13_ft;
use crate::crc::{remainder, syndromes};
use crate::demod::{Candidate, DemodOut, DemodStats, Slice};
use crate::feeds::{RemoteOut, RemoteState, SourceStatus};
use crate::message::{df_bits, header, is_ap, Frame, NON_ICAO};
use crate::settings::{CORRECTION, DROP_S, EXPIRY_S, REPAIR_UNKNOWN};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::{
    Choice, ConfigError, Record, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};
use std::collections::HashMap;

sdrkit_core::choice_enum! {
    /// The error correction setting.
    pub enum Correction {
        /// No repair.
        Off = ("off", "Off"),
        /// Single-bit errors.
        One = ("1bit", "1 bit"),
        /// Up to three bits among the least certain.
        Three = ("3bits", "Up to 3 bits"),
    }
}

/// An accepted frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Accepted {
    /// The frame (after any repair).
    pub frame: Frame,
    /// Signal time of the preamble, s (for a frame from another receiver: the time it was taken in).
    pub time: f64,
    /// Level, dBFS (`None` for another receiver's frame).
    pub level_dbfs: Option<f32>,
    /// SNR, dB.
    pub snr_db: Option<f32>,
    /// Bits flipped by the repair.
    pub repaired: u8,
    /// A trusted frame (PI format with a good or repaired parity).
    pub trusted: bool,
    /// The aircraft key.
    pub key: u32,
    /// 0 for this receiver, otherwise the number of the feed it came from.
    pub source: u16,
    /// The 12 MHz time stamp of another receiver's Beast frame, and its signal byte.
    pub remote_stamp: Option<(u64, u8)>,
}

/// The counters of the checks, with the demodulator's measurements, at a report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CheckStats {
    /// The demodulator's.
    pub demod: DemodStats,
    /// Good messages (clean or repaired), this receiver only.
    pub good: u64,
    /// Failed candidates.
    pub failed: u64,
    /// Repaired messages.
    pub repaired: u64,
    /// Loud messages.
    pub loud: u64,
    /// Good messages by DF.
    pub by_df: [u64; 32],
    /// Time of the last failed candidate with an SNR of at least 8 dB.
    pub last_strong_failure: Option<f64>,
}

/// What the checks output. (One item per message, so the size of the frame variant does not matter.)
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Msg {
    /// An accepted frame.
    Frame(Accepted),
    /// A state update from another receiver (SBS or ASTERIX).
    State {
        /// The update.
        state: RemoteState,
        /// The feed number.
        source: u16,
    },
    /// The feeds in, at a report.
    Sources(Vec<SourceStatus>),
    /// The counters, at a report.
    Stats(CheckStats),
}

impl Item for Msg {
    const KIND: &'static str = "modes.frame";
}

#[derive(Clone, Debug, Default)]
struct Book {
    last_trusted: f64,
    last_seen: f64,
    level: Option<f32>,
    trusted_alt: Option<(i32, f64)>,
}

/// The frame checker: it keeps what it needs about the aircraft heard (when the last trusted frame came, the mean level, the last
/// trusted altitude).
#[derive(Clone, Debug)]
pub struct FrameChecker {
    /// Error correction.
    pub correction: Correction,
    /// Repair frames from addresses not known.
    pub repair_unknown: bool,
    /// How long an address stays known after its last trusted frame, s.
    pub expiry_s: f64,
    /// How long the table keeps a silent aircraft, s (the drop time, or the expiry time when no drop time is set).
    pub keep_s: f64,
    book: HashMap<u32, Book>,
    accepted_until: f64,
    stats: CheckStats,
}

impl Default for FrameChecker {
    fn default() -> Self {
        FrameChecker {
            correction: Correction::Three,
            repair_unknown: false,
            expiry_s: 60.0,
            keep_s: 60.0,
            book: HashMap::new(),
            accepted_until: f64::MIN,
            stats: CheckStats::default(),
        }
    }
}

/// The most aircraft the checker (and the table) keeps.
pub const MAX_AIRCRAFT: usize = 2000;

impl FrameChecker {
    /// The counters.
    pub fn stats(&self) -> &CheckStats {
        &self.stats
    }
    /// Empties the address book and the counters ("clear results").
    pub fn clear(&mut self) {
        self.book.clear();
        self.stats = CheckStats::default();
        self.accepted_until = f64::MIN;
    }

    /// Whether an address is known at `now`.
    pub fn known(&self, key: u32, now: f64) -> bool {
        self.book.get(&key).is_some_and(|b| {
            now - b.last_trusted <= self.expiry_s && now - b.last_seen <= self.keep_s
        })
    }

    fn plausible(&self, f: &Frame, key: u32, level: Option<f32>, now: f64) -> bool {
        let Some(b) = self.book.get(&key) else {
            return false;
        };
        let df = f.df();
        if matches!(df, 4 | 5 | 20 | 21) && f.field(6, 8) >= 6 {
            return false;
        }
        if let (Some(l), Some(m)) = (level, b.level) {
            if (l - 10.0 * m.log10()).abs() > 25.0 {
                return false;
            }
        }
        if matches!(df, 0 | 4 | 16 | 20) {
            if let (Some(a), Some((ta, t))) = (ac13_ft(f.field(20, 32) as u32), b.trusted_alt) {
                if now - t <= 30.0 && (a - ta).abs() > 5000 {
                    return false;
                }
            }
        }
        true
    }

    fn key_of(f: &Frame) -> u32 {
        let mut k = f.aa();
        if f.df() == 18 && matches!(f.field(6, 8), 1 | 5) {
            k |= NON_ICAO;
        }
        k
    }

    /// Accepts a frame as it is, or `None`.
    fn accept_as_is(&self, s: &Slice, level: Option<f32>, now: f64) -> Option<(Frame, bool, u32)> {
        let bits = df_bits(s.bytes[0])?;
        let f = Frame {
            bytes: s.bytes,
            bits,
        };
        let rem = f.remainder();
        let df = s.bytes[0] >> 3;
        match df {
            17..=19 => (rem == 0).then(|| (f, true, Self::key_of(&f))),
            11 => {
                if rem == 0 || (rem < 128 && self.known(f.aa(), now)) {
                    Some((f, true, f.aa()))
                } else {
                    None
                }
            }
            _ if is_ap(f.df()) => (self.known(rem, now) && self.plausible(&f, rem, level, now))
                .then_some((f, false, rem)),
            _ => None,
        }
    }

    fn repair(&self, s: &Slice, now: f64) -> Option<(Frame, u8, u32)> {
        if self.correction == Correction::Off {
            return None;
        }
        let lens: &[u8] = match df_bits(s.bytes[0]) {
            Some(56) => &[56],
            Some(_) => &[112],
            None => &[112, 56],
        };
        let syn = syndromes();
        for &bits in lens {
            let f = Frame {
                bytes: s.bytes,
                bits,
            };
            let rem = f.remainder();
            if rem == 0 || (bits == 56 && rem < 128) {
                continue;
            }
            let fits = |g: &Frame| -> Option<u32> {
                let df = g.bytes[0] >> 3;
                let ok = if bits == 112 {
                    df == 17 || df == 18
                } else {
                    df == 11
                };
                if !ok {
                    return None;
                }
                let key = Self::key_of(g);
                (self.repair_unknown || self.known(key, now)).then_some(key)
            };
            let n = bits as usize;
            if let Some(i) = syn.locate(n, rem) {
                let mut g = f;
                g.flip(i);
                if let Some(k) = fits(&g) {
                    return Some((g, 1, k));
                }
            }
            if self.correction == Correction::Three {
                let table = syn.for_len(n);
                let mut order = [0usize; 112];
                for (i, v) in order.iter_mut().enumerate() {
                    *v = i;
                }
                let idx = &mut order[..n];
                // The bit index breaks ties, so the order is that of a stable sort by confidence.
                idx.sort_unstable_by(|&a, &b| s.conf[a].total_cmp(&s.conf[b]).then(a.cmp(&b)));
                let l16 = &idx[..16];
                for (ai, &a) in l16.iter().enumerate() {
                    for &b in &l16[ai + 1..] {
                        if table[a] ^ table[b] == rem {
                            let mut g = f;
                            g.flip(a);
                            g.flip(b);
                            if let Some(k) = fits(&g) {
                                return Some((g, 2, k));
                            }
                        }
                    }
                }
                let l12 = &idx[..12];
                for ai in 0..12 {
                    for bi in ai + 1..12 {
                        let need = rem ^ table[l12[ai]] ^ table[l12[bi]];
                        for &c in &l12[bi + 1..] {
                            if table[c] == need {
                                let mut g = f;
                                g.flip(l12[ai]);
                                g.flip(l12[bi]);
                                g.flip(c);
                                if let Some(k) = fits(&g) {
                                    return Some((g, 3, k));
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn learn(&mut self, a: &Accepted) {
        if !self.book.contains_key(&a.key) && self.book.len() >= MAX_AIRCRAFT {
            if let Some((&old, _)) = self
                .book
                .iter()
                .min_by(|x, y| x.1.last_seen.total_cmp(&y.1.last_seen))
            {
                self.book.remove(&old);
            }
        }
        let b = self.book.entry(a.key).or_default();
        b.last_seen = a.time;
        if a.trusted {
            b.last_trusted = a.time;
        }
        if let Some(l) = a.level_dbfs {
            let lin = 10f32.powf(l / 10.0);
            b.level = Some(b.level.map_or(lin, |m| m + 0.2 * (lin - m)));
        }
        if matches!(a.frame.df(), 17 | 18) {
            if let Some(
                crate::message::Es::Airborne {
                    gnss: false,
                    alt_ft: Some(alt),
                    ..
                }
                | crate::message::Es::NoPosition { alt_ft: Some(alt) },
            ) = crate::message::es(&a.frame)
            {
                b.trusted_alt = Some((alt, a.time));
            }
        }
    }

    /// Checks one slice: accepted as it is, or repaired, or refused (`None`). `level` and `snr` come from the preamble.
    pub fn check_slice(
        &mut self,
        s: &Slice,
        time: f64,
        level: Option<f32>,
        snr: Option<f32>,
    ) -> Option<Accepted> {
        let (frame, repaired, trusted, key) = match self.accept_as_is(s, level, time) {
            Some((f, t, k)) => (f, 0, t, k),
            None => {
                let (f, n, k) = self.repair(s, time)?;
                (f, n, true, k)
            }
        };
        let a = Accepted {
            frame,
            time,
            level_dbfs: level,
            snr_db: snr,
            repaired,
            trusted,
            key,
            source: 0,
            remote_stamp: None,
        };
        self.learn(&a);
        self.stats.good += 1;
        if repaired > 0 {
            self.stats.repaired += 1;
        }
        self.stats.by_df[(frame.bytes[0] >> 3) as usize] += 1;
        if level.is_some_and(|l| l > -7.0) {
            self.stats.loud += 1;
        }
        self.accepted_until = time + (7.0 + frame.bits as f64) * 1e-6;
        Some(a)
    }

    /// Checks a candidate: its attempts in order, the first that passes wins. A candidate inside a frame already accepted is
    /// dropped.
    pub fn check_candidate(&mut self, c: &Candidate) -> Option<Accepted> {
        if c.time < self.accepted_until {
            return None;
        }
        for s in &c.attempts {
            if let Some(a) = self.check_slice(s, c.time, Some(c.level_dbfs), Some(c.snr_db)) {
                return Some(a);
            }
        }
        if c.strict {
            self.stats.failed += 1;
            if c.snr_db >= 8.0 {
                self.stats.last_strong_failure = Some(c.time);
            }
        }
        None
    }

    /// Counts a failed candidate (for frames fed as sliced frames).
    pub fn count_failed(&mut self) {
        self.stats.failed += 1;
    }

    /// A frame from another receiver: only a DF17/18 (112 bits) or DF11 (56 bits) with a good parity is taken.
    pub fn check_remote(
        &mut self,
        f: &Frame,
        time: f64,
        source: u16,
        stamp: Option<(u64, u8)>,
    ) -> Option<Accepted> {
        let df = f.bytes[0] >> 3;
        let ok = (f.bits == 112 && (df == 17 || df == 18)) || (f.bits == 56 && df == 11);
        if !ok || remainder(f.data()) != 0 {
            return None;
        }
        let a = Accepted {
            frame: *f,
            time,
            level_dbfs: None,
            snr_db: None,
            repaired: 0,
            trusted: true,
            key: Self::key_of(f),
            source,
            remote_stamp: stamp,
        };
        self.learn(&a);
        Some(a)
    }

    /// An SBS or ASTERIX update from another receiver: its address counts as known.
    pub fn learn_remote_state(&mut self, key: u32, time: f64) {
        let a = Accepted {
            frame: Frame {
                bytes: [0; 14],
                bits: 56,
            },
            time,
            level_dbfs: None,
            snr_db: None,
            repaired: 0,
            trusted: true,
            key,
            source: 1,
            remote_stamp: None,
        };
        let had = self.book.get(&key).and_then(|b| b.trusted_alt);
        self.learn(&a);
        if let Some(b) = self.book.get_mut(&key) {
            b.trusted_alt = had;
        }
    }

    /// The header key of an accepted frame (for tests).
    pub fn key(f: &Frame) -> u32 {
        header(f, f.remainder()).key
    }
}

sdrkit_core::settings! {
    /// The settings of [`FrameChecks`].
    pub enum ChecksSetting {
        /// Error correction.
        Correction = CORRECTION,
        /// Repair unknown addresses.
        RepairUnknown = REPAIR_UNKNOWN,
        /// Expiry time.
        ExpiryS = EXPIRY_S,
        /// Drop time.
        DropS = DROP_S,
    }
}

sdrkit_core::commands! {
    /// The commands of [`FrameChecks`].
    pub enum ChecksCommand {
        /// Forgets the addresses heard and the counters.
        Clear = sdrkit_util_objects::commands::CLEAR,
    }
}

/// The checks as a component (`adsb.checks`): input 0 the demodulator's candidates, input 1 the feeds from other receivers.
pub struct FrameChecks {
    c: FrameChecker,
    drop_s: f64,
    time: f64,
    out: Vec<Timed<Msg>>,
}

impl Default for FrameChecks {
    fn default() -> Self {
        FrameChecks {
            c: FrameChecker::default(),
            drop_s: DROP_S.default,
            time: 0.0,
            out: Vec::new(),
        }
    }
}

impl FrameChecks {
    /// The checker.
    pub fn checker(&self) -> &FrameChecker {
        &self.c
    }
}

impl Component for FrameChecks {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "adsb.checks",
            vec![
                Port::typed::<DemodOut>("candidates"),
                Port::typed::<RemoteOut>("remote"),
            ],
            vec![Port::typed::<Msg>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(ChecksSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.c.accepted_until = f64::MIN;
        Ok(vec![Format::new(Kind::typed::<Msg>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let cands = io.items::<DemodOut>(0);
        let remote = io.items::<RemoteOut>(1);
        let mut stats = None;
        let mut outv = std::mem::take(&mut self.out);
        for r in remote {
            match &r.value {
                RemoteOut::Frame {
                    frame,
                    source,
                    stamp,
                } => {
                    if let Some(a) = self.c.check_remote(frame, self.time, *source, *stamp) {
                        outv.push(Timed {
                            time: self.time,
                            value: Msg::Frame(a),
                        });
                    }
                }
                RemoteOut::State { state, source } => {
                    let key = state.key;
                    self.c.learn_remote_state(key, self.time);
                    outv.push(Timed {
                        time: self.time,
                        value: Msg::State {
                            state: state.clone(),
                            source: *source,
                        },
                    });
                }
                RemoteOut::Sources(s) => outv.push(Timed {
                    time: self.time,
                    value: Msg::Sources(s.clone()),
                }),
            }
        }
        for c in cands {
            match &c.value {
                DemodOut::Candidate(c) => {
                    self.time = self.time.max(c.time);
                    if let Some(a) = self.c.check_candidate(c) {
                        outv.push(Timed {
                            time: a.time,
                            value: Msg::Frame(a),
                        });
                    }
                }
                DemodOut::Stats(s) => {
                    self.time = self.time.max(s.time);
                    let mut st = self.c.stats.clone();
                    st.demod = s.clone();
                    stats = Some((s.time, st));
                }
            }
        }
        let o = io.items_out::<Msg>(0);
        o.append(&mut outv);
        self.out = outv;
        if let Some((t, st)) = stats {
            o.push(Timed {
                time: t,
                value: Msg::Stats(st),
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ChecksSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ChecksSetting::Correction => self.c.correction = CORRECTION.read(id, v)?,
            ChecksSetting::RepairUnknown => self.c.repair_unknown = REPAIR_UNKNOWN.read(id, v)?,
            ChecksSetting::ExpiryS => self.c.expiry_s = EXPIRY_S.read(id, v)?,
            ChecksSetting::DropS => {
                self.drop_s = DROP_S.read(id, v)?;
                self.c.keep_s = keep_s(self.drop_s);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                ChecksSetting::Correction,
                Value::Choice(self.c.correction.index()),
            ),
            (
                ChecksSetting::RepairUnknown,
                Value::Bool(self.c.repair_unknown),
            ),
            (ChecksSetting::ExpiryS, Value::F64(self.c.expiry_s)),
            (ChecksSetting::DropS, Value::F64(self.drop_s)),
        ])
    }
    fn commands(&self) -> &Commands {
        ChecksCommand::commands()
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match ChecksCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ChecksCommand::Clear => self.c.clear(),
        }
        Ok(None)
    }
}

/// The time the table keeps a silent aircraft for a drop time: 0 keeps it for a time far beyond any session, and a drop time
/// is at least 5 s.
pub fn keep_s(drop_s: f64) -> f64 {
    if drop_s <= 0.0 {
        1e9
    } else {
        drop_s.max(5.0)
    }
}
