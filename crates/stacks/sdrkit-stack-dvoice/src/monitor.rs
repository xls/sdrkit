//! The voice monitor: which call is heard, the ID lists and labels, the codec plug-ins that turn speech frames into audio,
//! the calls heard and the stations heard. Inputs `speech` and `call`, output `audio` (mono, at the rate of the codec).

use crate::decoder::{HANG_S, LISTEN};
use crate::items::{CallItem, EncState, Ident};
use crate::results::ident_text;
use crate::typed::{HeardRow, HistoryRow, MonitorFields, HEARD, HISTORY};
use sdrkit_core::codec::{CodecError, CodecSet};
use sdrkit_core::command::{Command, CommandError, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Seconds};
use sdrkit_core::media::{Codec, MediaFormat, Packaging};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    CommandId, ConfigError, FourCc, Record, Results, ResultsDecl, Setting, SettingError, SettingId,
    SettingSet, Value, Values,
};
use sdrkit_graph::codec::GuardedCodec;
use sdrkit_graph::media::SpeechPacket;
use sdrkit_graph::{
    values_of, Applied, ChannelsFrom, Component, ComponentInfo, Format, Io, Port, RateFrom,
    RunContext,
};
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;

/// The sample rate of the speech codecs, Hz.
pub const SPEECH_RATE: f64 = 8_000.0;

/// One entry of an ID list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdMatch {
    /// One number.
    Number(u64),
    /// Numbers from the first to the second.
    Range(u64, u64),
    /// One callsign.
    Call(String),
    /// Callsigns that start with this text.
    Prefix(String),
}

/// An ID list: numbers, ranges "100-199", callsigns and prefixes "LY*", apart by commas, semicolons, spaces or new lines. "TG"
/// before a number is left out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IdList(pub Vec<IdMatch>);

impl IdList {
    /// Reads an ID list.
    ///
    /// # Errors
    ///
    /// The byte position of the first entry with a character that is not a letter, a digit, `/`, `-` or a final `*`.
    pub fn parse(text: &str) -> Result<IdList, usize> {
        let mut out = Vec::new();
        let mut start = None;
        let ends = text
            .char_indices()
            .chain(std::iter::once((text.len(), ' ')));
        for (i, c) in ends {
            let sep = c == ',' || c == ';' || c.is_whitespace();
            if !sep {
                start.get_or_insert(i);
                continue;
            }
            let Some(s) = start.take() else {
                continue;
            };
            let t = &text[s..i];
            if t.eq_ignore_ascii_case("tg") {
                continue;
            }
            let body = t.strip_suffix('*').unwrap_or(t);
            if let Some(bad) = body
                .char_indices()
                .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '/' || *c == '-'))
            {
                return Err(s + bad.0);
            }
            out.push(Self::entry(t));
        }
        Ok(IdList(out))
    }

    fn entry(t: &str) -> IdMatch {
        // "TG91" is "91".
        let t = match t.get(..2) {
            Some(p)
                if p.eq_ignore_ascii_case("tg")
                    && t.len() > 2
                    && t[2..].chars().all(|c| c.is_ascii_digit() || c == '-') =>
            {
                &t[2..]
            }
            _ => t,
        };
        if let Some((a, b)) = t.split_once('-') {
            if let (Ok(a), Ok(b)) = (a.parse::<u64>(), b.parse::<u64>()) {
                return IdMatch::Range(a.min(b), a.max(b));
            }
        }
        if let Ok(n) = t.parse::<u64>() {
            IdMatch::Number(n)
        } else if let Some(p) = t.strip_suffix('*') {
            IdMatch::Prefix(p.to_ascii_uppercase())
        } else {
            IdMatch::Call(t.to_ascii_uppercase())
        }
    }

    /// Returns true when the list is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns true when `id` is on the list.
    pub fn matches_id(&self, id: &Ident) -> bool {
        if self.is_empty() {
            return false;
        }
        let text = id.text().trim().to_ascii_uppercase();
        let num = id.number().or_else(|| text.parse().ok());
        self.0.iter().any(|m| match m {
            IdMatch::Number(n) => num == Some(*n),
            IdMatch::Range(a, b) => num.is_some_and(|x| x >= *a && x <= *b),
            IdMatch::Call(c) => {
                !text.is_empty()
                    && (text == *c || text.split(['/', '-', ' ']).next() == Some(c.as_str()))
            }
            IdMatch::Prefix(p) => !text.is_empty() && text.starts_with(p.as_str()),
        })
    }

    /// Returns true when the target or the source of a call is on the list.
    pub fn matches(&self, src: &Ident, dst: &Ident) -> bool {
        self.matches_id(dst) || self.matches_id(src)
    }
}

/// Names for targets and source radios.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Labels {
    /// Names of targets, by the target in upper case.
    pub targets: BTreeMap<String, String>,
    /// Names of source radios, by the radio in upper case.
    pub radios: BTreeMap<String, String>,
}

fn clean_name(s: &str) -> String {
    let s: String = s
        .chars()
        .filter(|c| *c != '"')
        .map(|c| {
            if c == ',' || c == ';' || c == '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut cut = s.len().min(64);
    while !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s[..cut].to_string()
}

impl Labels {
    /// Reads lines "91=Worldwide", which name targets, and "RID 3120123=W1ABC Joe", which name source radios. Empty lines are
    /// left out.
    ///
    /// # Errors
    ///
    /// The byte position of the first line that is not empty and has no `=`.
    pub fn parse(text: &str) -> Result<Labels, usize> {
        let mut l = Labels::default();
        let mut at = 0;
        for line in text.split_inclusive('\n') {
            let start = at;
            at += line.len();
            let line = line.trim_end_matches(['\n', '\r']);
            if line.trim().is_empty() {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                return Err(start);
            };
            let k = k.trim();
            let v = clean_name(v);
            if let Some(r) = k.strip_prefix("RID").or_else(|| k.strip_prefix("rid")) {
                l.radios.insert(r.trim().to_ascii_uppercase(), v);
            } else {
                let k = k.trim_start_matches("TG").trim();
                l.targets.insert(k.to_ascii_uppercase(), v);
            }
        }
        Ok(l)
    }

    /// Imports a radio ID list (CSV with RADIO_ID, CALLSIGN, FIRST_NAME, LAST_NAME, CITY, STATE, COUNTRY. Comma, semicolon or
    /// tab apart, with or without a header). It returns the count of lines taken.
    pub fn import_radio_ids(&mut self, text: &str) -> usize {
        let mut taken = 0;
        let mut cols: Option<BTreeMap<String, usize>> = None;
        for (n, line) in text.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let sep = if line.contains('\t') {
                '\t'
            } else if line.matches(';').count() > line.matches(',').count() {
                ';'
            } else {
                ','
            };
            let cells: Vec<String> = line
                .split(sep)
                .map(|c| c.trim().trim_matches('"').to_string())
                .collect();
            if n == 0
                && cells.iter().any(|c| {
                    c.eq_ignore_ascii_case("RADIO_ID") || c.eq_ignore_ascii_case("CALLSIGN")
                })
            {
                cols = Some(
                    cells
                        .iter()
                        .enumerate()
                        .map(|(i, c)| (c.to_ascii_uppercase(), i))
                        .collect(),
                );
                continue;
            }
            let get = |name: &str, default: usize| -> String {
                let i = cols
                    .as_ref()
                    .and_then(|c| c.get(name).copied())
                    .unwrap_or(default);
                cells.get(i).cloned().unwrap_or_default()
            };
            let id = get("RADIO_ID", 0);
            if id.parse::<u64>().is_err() {
                continue;
            }
            let name = [
                get("CALLSIGN", 1),
                get("FIRST_NAME", 2),
                get("LAST_NAME", 3),
            ]
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
            self.radios.insert(id, clean_name(&name));
            taken += 1;
        }
        taken
    }

    /// Imports a talkgroup list: `id,name`, `id;name`, `id<TAB>name` or `id=name` lines. It returns the count of lines taken.
    pub fn import_talkgroups(&mut self, text: &str) -> usize {
        let mut taken = 0;
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            let Some(i) = line.find([',', ';', '\t', '=']) else {
                continue;
            };
            let (id, name) = (line[..i].trim().trim_matches('"'), &line[i + 1..]);
            if id.parse::<u64>().is_err() {
                continue;
            }
            self.targets.insert(id.to_string(), clean_name(name));
            taken += 1;
        }
        taken
    }

    /// The radio with its name, "1234567  W1ABC Joe Smith", when its name is known.
    pub fn show_radio(&self, id: &Ident) -> String {
        let t = id.text();
        match self.radios.get(&t.to_ascii_uppercase()) {
            Some(n) => format!("{t}  {n}"),
            None => t,
        }
    }

    /// The name of target `id`.
    pub fn target_label(&self, id: &Ident) -> Option<&String> {
        if self.targets.is_empty() {
            return None;
        }
        self.targets.get(&id.text().to_ascii_uppercase())
    }
}

sdrkit_core::choice_enum! {
    /// Why a call is or is not heard.
    pub enum Verdict {
        /// It plays.
        Play = ("play", "Play"),
        /// Another call is playing.
        Busy = ("busy", "another call is playing"),
        /// A call of higher priority is playing.
        LowerPriority = ("lower_priority", "Lower priority"),
        /// It is on the mute list.
        Muted = ("muted", "Muted"),
        /// It is not on the allow list.
        Filtered = ("filtered", "Filtered"),
        /// Another target is held.
        NotHeld = ("not_held", "Not held"),
        /// Its bit error rate is above the error squelch.
        Squelched = ("squelched", "Squelched"),
        /// It is encrypted and is never decoded.
        Encrypted = ("encrypted", "never decoded"),
        /// No codec plug-in for its codec.
        NoCodec = ("no_codec", "No codec"),
    }
}

/// A call as the monitor lists it.
#[derive(Clone, Debug)]
pub struct HeardCall {
    /// The call id.
    pub call_id: u64,
    /// Who sends.
    pub src: Ident,
    /// Who is called.
    pub dst: Ident,
    /// The name of the target.
    pub label: String,
    /// Signal time of the start, seconds.
    pub start_s: f64,
    /// Signal time of the last frame, seconds.
    pub end_s: f64,
    /// Why it is heard or not.
    pub verdict: Verdict,
    /// Frames played.
    pub played_frames: u32,
    /// It is encrypted.
    pub encrypted: bool,
    /// It is on air.
    pub active: bool,
}

/// One row of the stations heard (per source radio or callsign).
#[derive(Clone, Debug, Default)]
pub struct HeardStation {
    /// The source.
    pub src: String,
    /// Calls heard.
    pub calls: u32,
    /// Time on air, seconds.
    pub airtime_s: f64,
    /// Signal time of the end of the last call, seconds.
    pub last_s: f64,
    /// The mean bit error rate of its calls.
    pub mean_ber: f32,
    /// One of its calls was an emergency call.
    pub emergency: bool,
}

/// Frames with a higher bit error rate are not played. 0 turns it off.
pub const SQUELCH_PCT: Setting<field::F64> = Setting::new("squelch_pct", "Error squelch", "%", 0.0)
    .range(0.0, 50.0, 1.0)
    .zero_means("Off")
    .help("Frames with a higher bit error rate are not played.")
    .section("Voice");
/// Play only calls on the allow list.
pub const ALLOW_ONLY: Setting<field::Bool> = Setting::new("allow_only", "Allowed only", "", false)
    .help("Play only calls on the allow list.")
    .section("Voice");
/// The allow list.
pub const ALLOW: Setting<field::Text> = Setting::new("allow", "Allow list", "", "")
    .help("Numbers, ranges 100-199, callsigns and prefixes LY*.")
    .section("Voice");
/// The mute list.
pub const MUTE_LIST: Setting<field::Text> = Setting::new("mute_list", "Mute list", "", "")
    .help("Numbers, ranges 100-199, callsigns and prefixes LY*.")
    .section("Voice");
/// A call on this list takes the sound at once.
pub const PRIORITY: Setting<field::Text> = Setting::new("priority", "Priority list", "", "")
    .help("A call on this list takes the sound at once.")
    .section("Voice");
/// Only these targets are played.
pub const HOLD: Setting<field::Text> = Setting::new("hold", "Hold", "", "")
    .help("Only this target is played.")
    .section("Voice");
/// Names of targets and radios.
pub const LABELS: Setting<field::Text> = Setting::new("labels", "Labels", "", "")
    .help("Lines 91=Worldwide, and RID 3120123=W1ABC Joe.")
    .section("Voice");
/// The held target is let go after this long without a call on it. 0 holds it until it is released.
pub const HOLD_TIMEOUT_S: Setting<field::F64> =
    Setting::new("hold_timeout_s", "Hold timeout", "s", 0.0)
        .range(0.0, 3600.0, 1.0)
        .help("The hold lets go after this long without a call on it (0: until released).")
        .section("Voice");

sdrkit_core::settings! {
    /// The settings of the voice monitor.
    pub enum MonitorSetting {
        /// The error squelch.
        SquelchPct = SQUELCH_PCT,
        /// Play only calls on the allow list.
        AllowOnly = ALLOW_ONLY,
        /// The allow list.
        Allow = ALLOW,
        /// The mute list.
        MuteList = MUTE_LIST,
        /// The priority list.
        Priority = PRIORITY,
        /// The targets held.
        Hold = HOLD,
        /// The names of targets and radios.
        Labels = LABELS,
        /// The hold timeout.
        HoldTimeoutS = HOLD_TIMEOUT_S,
        /// A call is over after this long without a frame.
        HangS = HANG_S,
        /// Speech is played.
        Listen = LISTEN,
    }
}

/// Plays only the target of the call that plays now, or of the newest call.
pub const HOLD_CURRENT: Command = Command::new("hold_current", "Hold current")
    .help("Plays only the target of the call that plays now, or of the newest call.");
/// Lets the held target go.
pub const RELEASE: Command = Command::new("release", "Release").help("Lets the held target go.");

sdrkit_core::commands! {
    /// The commands of the voice monitor.
    pub enum MonitorCommand {
        /// Forgets the calls and stations heard.
        Clear = sdrkit_util_objects::commands::CLEAR,
        /// Holds the target of the call that plays now.
        HoldCurrent = HOLD_CURRENT,
        /// Lets the held target go.
        Release = RELEASE,
    }
}

/// The settings of the monitor in force.
#[derive(Clone, Debug)]
pub struct MonitorSettings {
    /// The error squelch, percent.
    pub squelch_pct: f32,
    /// Play only calls on the allow list.
    pub allow_only: bool,
    /// The allow list, as typed and as read.
    pub allow: (String, IdList),
    /// The mute list, as typed and as read.
    pub mute_list: (String, IdList),
    /// The priority list, as typed and as read.
    pub priority: (String, IdList),
    /// The targets held, as typed and as read.
    pub hold: (String, IdList),
    /// The names, as typed and as read.
    pub labels: (String, Labels),
    /// The hold timeout, seconds.
    pub hold_timeout_s: f64,
    /// The hang time, seconds.
    pub hang_s: f64,
    /// Speech is played.
    pub listen: bool,
}

impl Default for MonitorSettings {
    fn default() -> Self {
        MonitorSettings {
            squelch_pct: SQUELCH_PCT.default as f32,
            allow_only: ALLOW_ONLY.default,
            allow: Default::default(),
            mute_list: Default::default(),
            priority: Default::default(),
            hold: Default::default(),
            labels: Default::default(),
            hold_timeout_s: HOLD_TIMEOUT_S.default,
            hang_s: HANG_S.default,
            listen: LISTEN.default,
        }
    }
}

struct Playing {
    call_id: u64,
    priority: bool,
    last_s: f64,
}

/// The id of the monitor in the graph of a stack.
pub const ID: &str = "monitor";

/// The most calls the monitor lists.
pub const HISTORY_CAP: usize = 500;
/// The most stations the monitor lists.
pub const HEARD_CAP: usize = 200;
/// The most calls the monitor keeps to judge the frames that come.
const CALLS_KEPT: usize = 1000;

/// Adds the fields and tables of the monitor to the declaration `d` of a stack.
pub fn declare(d: ResultsDecl) -> ResultsDecl {
    d.with_fields(MonitorFields::SHAPE.fields)
        .with_table(&HISTORY)
        .with_table(&HEARD)
}

/// The voice monitor component (`dvoice.monitor`).
pub struct VoiceMonitor {
    codecs: CodecSet,
    settings: MonitorSettings,
    calls: BTreeMap<u64, CallItem>,
    history: VecDeque<HeardCall>,
    heard: BTreeMap<String, HeardStation>,
    playing: Option<Playing>,
    hold_target: Option<Ident>,
    hold_last_s: f64,
    decoders: BTreeMap<(u8, u64, FourCc), GuardedCodec>,
    results: Published,
    now: f64,
    no_codec: Option<Codec>,
    notice: String,
    changed: bool,
    cleared: bool,
    key: String,
    text: String,
}

impl Default for VoiceMonitor {
    fn default() -> Self {
        VoiceMonitor::new()
    }
}

impl VoiceMonitor {
    /// A monitor. Its codec plug-ins come from the run context when it is attached.
    pub fn new() -> VoiceMonitor {
        VoiceMonitor {
            codecs: CodecSet::default(),
            settings: MonitorSettings::default(),
            calls: BTreeMap::new(),
            history: VecDeque::with_capacity(HISTORY_CAP + 1),
            heard: BTreeMap::new(),
            playing: None,
            hold_target: None,
            hold_last_s: 0.0,
            decoders: BTreeMap::new(),
            results: Published::new(MonitorFields::SHAPE.fields).with_tables(&[&HISTORY, &HEARD]),
            now: 0.0,
            no_codec: None,
            notice: String::new(),
            changed: true,
            cleared: false,
            key: String::new(),
            text: String::new(),
        }
    }

    /// The calls heard, newest first.
    pub fn history(&self) -> impl Iterator<Item = &HeardCall> {
        self.history.iter()
    }

    fn verdict(
        &mut self,
        call_id: u64,
        frame_ber: f32,
        fourcc: FourCc,
        layout: &str,
    ) -> Option<Verdict> {
        let c = self.calls.get(&call_id)?;
        let s = &self.settings;
        let (src, dst) = (&c.record.src, &c.record.dst);
        if matches!(c.record.encryption.state, EncState::Encrypted) {
            return Some(Verdict::Encrypted);
        }
        if s.mute_list.1.matches(src, dst) {
            return Some(Verdict::Muted);
        }
        if s.allow_only && !s.allow.1.matches(src, dst) {
            return Some(Verdict::Filtered);
        }
        // Hold: the list, or the held target.
        if let Some(t) = &self.hold_target {
            if s.hold_timeout_s > 0.0 && self.now - self.hold_last_s > s.hold_timeout_s {
                self.hold_target = None;
            } else if dst != t {
                return Some(Verdict::NotHeld);
            } else {
                self.hold_last_s = self.now;
            }
        } else if !s.hold.1.is_empty() && !s.hold.1.matches_id(dst) {
            return Some(Verdict::NotHeld);
        }
        if s.squelch_pct > 0.0 && frame_ber * 100.0 > s.squelch_pct {
            return Some(Verdict::Squelched);
        }
        let prio = s.priority.1.matches(src, dst);
        if let Some(p) = &self.playing {
            let same = p.call_id == c.id || self.same_call(p.call_id, c);
            let quiet = self.now - p.last_s > s.hang_s;
            if !same && !quiet {
                if prio && !p.priority {
                    // Takes the sound at once.
                } else if p.priority && !prio {
                    return Some(Verdict::LowerPriority);
                } else {
                    return Some(Verdict::Busy);
                }
            }
        }
        if self.codecs.find(fourcc, layout).is_none() {
            return Some(Verdict::NoCodec);
        }
        Some(Verdict::Play)
    }

    /// A group call sent by several sites or slots is heard once: the same target and source within 1 s.
    fn same_call(&self, playing_id: u64, c: &CallItem) -> bool {
        self.calls.get(&playing_id).is_some_and(|p| {
            p.record.dst == c.record.dst
                && p.record.src == c.record.src
                && (c.start.t - p.start.t).abs() < 1.0
        })
    }

    fn note_call(&mut self, c: &CallItem) {
        self.changed = true;
        if let Some(h) = self.history.iter_mut().find(|h| h.call_id == c.id) {
            h.end_s = c.last_s;
            h.active = c.is_active();
            h.encrypted = c.record.encryption.is_encrypted();
            if h.encrypted {
                h.verdict = Verdict::Encrypted;
            }
        } else if c.record.kind.is_voice() || c.record.voice_frames > 0 {
            let encrypted = c.record.encryption.is_encrypted();
            self.history.push_front(HeardCall {
                call_id: c.id,
                src: c.record.src.clone(),
                dst: c.record.dst.clone(),
                label: self
                    .settings
                    .labels
                    .1
                    .target_label(&c.record.dst)
                    .cloned()
                    .unwrap_or_default(),
                start_s: c.start.t,
                end_s: c.last_s,
                verdict: if encrypted {
                    Verdict::Encrypted
                } else {
                    Verdict::Play
                },
                played_frames: 0,
                encrypted,
                active: c.is_active(),
            });
            self.history.truncate(HISTORY_CAP);
        }
        // The stations heard: one row per source.
        if !c.is_active() && !c.record.src.is_none() {
            let src = c.record.src.text();
            let h = self.heard.entry(src.clone()).or_default();
            if h.src.is_empty() {
                h.src = src;
            }
            let n = h.calls as f32;
            h.calls += 1;
            h.airtime_s += c.duration_s().max(0.0);
            h.last_s = c.last_s;
            h.mean_ber = (h.mean_ber * n + c.record.ber()) / (n + 1.0);
            h.emergency |= c.record.emergency;
            if self.heard.len() > HEARD_CAP {
                if let Some(oldest) = self
                    .heard
                    .iter()
                    .min_by(|a, b| a.1.last_s.total_cmp(&b.1.last_s))
                    .map(|x| x.0.clone())
                {
                    self.heard.remove(&oldest);
                }
            }
        }
        self.calls.insert(c.id, c.clone());
        while self.calls.len() > CALLS_KEPT {
            if self.calls.pop_first().is_none() {
                break;
            }
        }
    }

    fn play(&mut self, f: &SpeechPacket, out: &mut Vec<f32>) {
        let key = (f.channel.slot, f.channel.carrier_hz as u64, f.fourcc);
        if !self.decoders.contains_key(&key) {
            let Some(entry) = self.codecs.find(f.fourcc, f.packaging.key()).cloned() else {
                return;
            };
            let Some(codec) = Codec::from_fourcc(f.fourcc) else {
                return;
            };
            let format = MediaFormat::new(codec, Packaging::Raw);
            self.decoders
                .insert(key, GuardedCodec::start(&entry, &format));
        }
        let Some(codec) = self.decoders.get_mut(&key) else {
            return;
        };
        match codec.decode(f.payload(), f.erasure) {
            Ok(pcm) => {
                // Every speech codec of the family outputs 8 kHz. Speech at another rate is not played.
                if pcm.rate_hz == 0 || f64::from(pcm.rate_hz) == SPEECH_RATE {
                    out.extend(pcm.samples.iter().map(|&s| f32::from(s) / 32768.0));
                }
            }
            Err(e @ (CodecError::Timeout | CodecError::Crashed)) => {
                self.decoders.remove(&key);
                self.codecs.disable(f.fourcc, &e);
            }
            Err(CodecError::Frame | CodecError::Unsupported) => {}
        }
    }

    fn publish(&mut self) {
        let p = &mut self.results;
        if self.cleared {
            self.cleared = false;
            for key in [HISTORY.key, HEARD.key] {
                if let Some(id) = p.results().decl().table_id(key) {
                    if let Some(mut t) = p.results_mut().table_mut(id) {
                        t.clear();
                    }
                }
            }
        }
        if let Some(pl) = &self.playing {
            self.key.clear();
            // Writing to a `String` cannot fail.
            let _ = write!(self.key, "{}", pl.call_id);
        }
        for n in self.codecs.take_notices() {
            self.notice = n;
        }
        if let Some(c) = self.no_codec.take() {
            self.notice.clear();
            let _ = write!(
                self.notice,
                "{} decoder not installed: identities and messages are shown, no sound",
                c.name()
            );
        }
        p.put(&MonitorFields {
            playing: self.playing.as_ref().map(|_| self.key.as_str()),
            held: self
                .hold_target
                .as_ref()
                .map(|t| ident_text(&mut self.text, t)),
            codec: (!self.notice.is_empty()).then_some(self.notice.as_str()),
        });
        if !self.changed {
            return;
        }
        self.changed = false;
        let mut dst = String::new();
        for h in self.history.iter().rev() {
            self.key.clear();
            let _ = write!(self.key, "{}", h.call_id);
            ident_text(&mut dst, &h.dst);
            let row = HistoryRow {
                call: &self.key,
                start_s: h.start_s,
                src: ident_text(&mut self.text, &h.src),
                dst: &dst,
                label: &h.label,
                verdict: h.verdict,
                played_frames: i64::from(h.played_frames),
                encrypted: h.encrypted,
                active: h.active,
            };
            HISTORY.put(p, &self.key, &row);
        }
        for h in self.heard.values() {
            let row = HeardRow {
                src: &h.src,
                calls: i64::from(h.calls),
                airtime: Seconds(h.airtime_s),
                last_s: h.last_s,
                mean_ber: f64::from(h.mean_ber),
                emergency: h.emergency,
            };
            HEARD.put(p, &h.src, &row);
        }
        let gone: Vec<String> = p
            .results()
            .decl()
            .table_id(HEARD.key)
            .and_then(|id| p.results().table(id))
            .map(|t| {
                t.rows_newest_first()
                    .filter(|(k, _)| !self.heard.contains_key(*k))
                    .map(|(k, _)| k.to_owned())
                    .collect()
            })
            .unwrap_or_default();
        for k in &gone {
            p.remove_row(&HEARD, k);
        }
    }

    fn clear(&mut self) {
        self.calls.clear();
        self.history.clear();
        self.heard.clear();
        self.playing = None;
        self.changed = true;
        self.cleared = true;
    }

    fn set_list(
        id: SettingId,
        v: &Value,
        setting: &Setting<field::Text>,
        slot: &mut (String, IdList),
    ) -> Result<(), SettingError> {
        let text = setting.read(id, v)?;
        let list =
            IdList::parse(text).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
        *slot = (text.to_owned(), list);
        Ok(())
    }
}

impl Component for VoiceMonitor {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvoice.monitor",
            vec![
                Port::typed::<SpeechPacket>("speech"),
                Port::typed::<CallItem>("call"),
            ],
            vec![Port::audio("audio")
                .rate_from(RateFrom::Fixed(SPEECH_RATE))
                .channels_from(ChannelsFrom::Fixed(1))],
        )
        .with_settings(MonitorSetting::schema())
    }

    /// Takes the codec plug-ins of the host from the run context. The decoders made with the plug-ins before are dropped.
    fn attach(&mut self, ctx: &RunContext) {
        self.codecs = (*ctx.codecs).clone();
        self.decoders.clear();
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let mut f = Format::audio(SPEECH_RATE, 1);
        f.bandwidth_hz = 4_000.0;
        Ok(vec![f])
    }

    fn process(&mut self, io: &mut Io) {
        for c in io.items::<CallItem>(1) {
            self.note_call(&c.value);
        }
        let frames = io.items::<SpeechPacket>(0);
        let mut audio = std::mem::take(&mut io.audio_out(0).samples);
        for f in frames {
            let f = &f.value;
            self.now = f.packet.time;
            let frame_ber = f.bit_errors.map_or(0.0, |e| {
                f32::from(e) / (f.payload().len().max(1) * 8) as f32
            });
            let Some(v) = self.verdict(f.call_id, frame_ber, f.fourcc, f.packaging.key()) else {
                continue;
            };
            if let Some(h) = self.history.iter_mut().find(|h| h.call_id == f.call_id) {
                if h.verdict != v {
                    h.verdict = v;
                    self.changed = true;
                }
            }
            match v {
                Verdict::Play => {
                    let prio = self.calls.get(&f.call_id).is_some_and(|c| {
                        self.settings
                            .priority
                            .1
                            .matches(&c.record.src, &c.record.dst)
                    });
                    self.playing = Some(Playing {
                        call_id: f.call_id,
                        priority: prio,
                        last_s: f.packet.time,
                    });
                    if !self.settings.listen {
                        continue;
                    }
                    self.play(f, &mut audio);
                    if let Some(h) = self.history.iter_mut().find(|h| h.call_id == f.call_id) {
                        h.played_frames += 1;
                        self.changed = true;
                    }
                }
                Verdict::NoCodec => {
                    if self.notice.is_empty() {
                        self.no_codec = Codec::from_fourcc(f.fourcc);
                    }
                }
                Verdict::Busy
                | Verdict::LowerPriority
                | Verdict::Muted
                | Verdict::Filtered
                | Verdict::NotHeld
                | Verdict::Squelched
                | Verdict::Encrypted => {}
            }
        }
        io.audio_out(0).samples = audio;
        if io.tick() {
            self.publish();
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let s = &mut self.settings;
        match MonitorSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MonitorSetting::SquelchPct => s.squelch_pct = SQUELCH_PCT.read(id, v)? as f32,
            MonitorSetting::AllowOnly => s.allow_only = ALLOW_ONLY.read(id, v)?,
            MonitorSetting::Allow => Self::set_list(id, v, &ALLOW, &mut s.allow)?,
            MonitorSetting::MuteList => Self::set_list(id, v, &MUTE_LIST, &mut s.mute_list)?,
            MonitorSetting::Priority => Self::set_list(id, v, &PRIORITY, &mut s.priority)?,
            MonitorSetting::Hold => Self::set_list(id, v, &HOLD, &mut s.hold)?,
            MonitorSetting::Labels => {
                let text = LABELS.read(id, v)?;
                let l = Labels::parse(text)
                    .map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
                s.labels = (text.to_owned(), l);
            }
            MonitorSetting::HoldTimeoutS => s.hold_timeout_s = HOLD_TIMEOUT_S.read(id, v)?,
            MonitorSetting::HangS => s.hang_s = HANG_S.read(id, v)?,
            MonitorSetting::Listen => s.listen = LISTEN.read(id, v)?,
        }
        self.changed = true;
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        let s = &self.settings;
        values_of([
            (
                MonitorSetting::SquelchPct,
                Value::F64(f64::from(s.squelch_pct)),
            ),
            (MonitorSetting::AllowOnly, Value::Bool(s.allow_only)),
            (MonitorSetting::Allow, Value::Text(s.allow.0.clone())),
            (MonitorSetting::MuteList, Value::Text(s.mute_list.0.clone())),
            (MonitorSetting::Priority, Value::Text(s.priority.0.clone())),
            (MonitorSetting::Hold, Value::Text(s.hold.0.clone())),
            (MonitorSetting::Labels, Value::Text(s.labels.0.clone())),
            (MonitorSetting::HoldTimeoutS, Value::F64(s.hold_timeout_s)),
            (MonitorSetting::HangS, Value::F64(s.hang_s)),
            (MonitorSetting::Listen, Value::Bool(s.listen)),
        ])
    }

    fn commands(&self) -> &Commands {
        MonitorCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match MonitorCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            MonitorCommand::Clear => self.clear(),
            MonitorCommand::HoldCurrent => {
                self.hold_target = self
                    .playing
                    .as_ref()
                    .and_then(|p| self.calls.get(&p.call_id))
                    .or_else(|| self.calls.values().next_back())
                    .map(|c| c.record.dst.clone());
                self.hold_last_s = self.now;
            }
            MonitorCommand::Release => self.hold_target = None,
        }
        self.publish();
        Ok(None)
    }

    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_lists_parse_numbers_ranges_callsigns_and_prefixes() {
        let l = IdList::parse("TG 91, 100-199\nLY*  W1ABC;TG3100").unwrap();
        assert!(l.matches_id(&Ident::Number(91)));
        assert!(l.matches_id(&Ident::Number(150)));
        assert!(!l.matches_id(&Ident::Number(200)));
        assert!(l.matches_id(&Ident::Text("LY2XX".into())));
        assert!(l.matches_id(&Ident::Text("w1abc".into())));
        assert!(l.matches_id(&Ident::Number(3100)));
        assert!(l.matches(&Ident::Number(5), &Ident::Number(91)));
        assert_eq!(IdList::parse("91, 9#2"), Err(5));
    }

    #[test]
    fn labels_and_imports() {
        let mut l = Labels::parse("91=Worldwide\nRID 3120123=W1ABC Joe").unwrap();
        assert_eq!(l.target_label(&Ident::Number(91)).unwrap(), "Worldwide");
        assert_eq!(l.show_radio(&Ident::Number(3120123)), "3120123  W1ABC Joe");
        let csv = "RADIO_ID,CALLSIGN,FIRST_NAME,LAST_NAME,CITY,STATE,COUNTRY\r\n1234567,W1ABC,\"Joe\",Smith,X,Y,Z\r\nbad,line\r\n";
        assert_eq!(l.import_radio_ids(csv), 1);
        assert_eq!(
            l.show_radio(&Ident::Number(1234567)),
            "1234567  W1ABC Joe Smith"
        );
        assert_eq!(l.import_radio_ids("2345678;K1XYZ;Ann;Lee"), 1);
        assert_eq!(
            l.import_talkgroups("91,World wide\n3100\tUSA\nx=y\n9=Local, net"),
            3
        );
        assert_eq!(l.target_label(&Ident::Number(9)).unwrap(), "Local net");
        assert_eq!(Labels::parse("91=A\nno name here\n"), Err(5));
    }
}
