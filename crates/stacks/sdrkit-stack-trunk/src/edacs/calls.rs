//! EDACS calls, talkgroups and the site, the power scan of the voice channels, and the choice of the analog call to follow:
//! `edacs.scan` and `edacs.calls`. The host follows the call (`HostRequest::Follow`).

use super::msg::{Command, EdacsMessage};
use super::rx::EdacsFrame;
use crate::follow::{
    urgency, FollowAction, FOLLOW, FOLLOW_GROUPS, VOICE_BANDWIDTH_HZ, VOICE_SIGNAL,
};
use crate::front::Level;
use crate::grant::Grant;
use crate::typed::*;
use crate::{set_status, typed_format, typed_port};
use num_complex::Complex32;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::{Fields, Hertz, Seconds, Time};
use sdrkit_core::publish::{field, keys, Published};
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, ConfigError, Event, HostRequest, LockState,
    Record, Results, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};
use sdrkit_util_ids::lists::IdList;
use sdrkit_util_ids::names::IdKind;
use sdrkit_util_objects::calls::{Call, CallEvent, CallLog, SameCall};
use sdrkit_util_objects::commands::CLEAR;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{log_rows, rows, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, ObjectTable, Seen, Stamp};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Parses the voice channel table: one line per channel, MHz: "frequency" (LCN = line number) or "LCN frequency", LCN 1 to 64,
/// frequencies of 1 MHz and up. Empty lines and lines that start with `#` are skipped.
///
/// # Errors
///
/// The byte position of the first line that is not a channel.
pub fn parse_table(text: &str) -> Result<BTreeMap<u32, f64>, usize> {
    let mut m = BTreeMap::new();
    let mut at = 0;
    for (i, line) in text.split_inclusive('\n').enumerate() {
        let start = at;
        at += line.len();
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let mut parts = l
            .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .filter(|p| !p.is_empty());
        let parsed = match (parts.next(), parts.next(), parts.next()) {
            (Some(f), None, None) => f.parse::<f64>().ok().map(|f| ((i + 1) as u32, f)),
            (Some(n), Some(f), None) => match (n.parse::<u32>(), f.parse::<f64>()) {
                (Ok(n), Ok(f)) => Some((n, f)),
                _ => None,
            },
            _ => None,
        };
        match parsed {
            Some((n, f)) if (1..=64).contains(&n) && f >= 1.0 => {
                m.insert(n, f * 1e6);
            }
            _ => return Err(start),
        }
    }
    Ok(m)
}

/// One channel of the scan.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScanRow {
    /// LCN.
    pub lcn: u32,
    /// Hz.
    pub freq_hz: f64,
    /// Inside the capture.
    pub in_capture: bool,
    /// Power (linear, 1 = full scale).
    pub power: f64,
    /// The floor.
    pub floor: f64,
    /// A carrier now.
    pub active: bool,
    /// Signal time of the last activity, s (-1: never).
    pub last_active: f64,
}

/// The scan's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScanReport {
    /// Signal time, s.
    pub time: f64,
    /// The capture's centre is known.
    pub centre_known: bool,
    /// The table's channels.
    pub rows: Vec<ScanRow>,
}

impl Item for ScanReport {
    const KIND: &'static str = "edacs.scan";
}

#[derive(Clone, Debug, Default)]
struct ScanState {
    p: f64,
    started: bool,
    floor: f64,
    above_since: Option<f64>,
    active: bool,
    last_active: f64,
}

/// The voice channel table, as text.
pub const LCN_TABLE: Setting<field::Text> = Setting::new("lcn_table", "Voice channel table", "", "")
    .help("One line per channel, MHz: \"frequency\" (LCN = line number) or \"LCN frequency\". LCN 1 to 64.");

sdrkit_core::settings! {
    /// The settings of `edacs.scan`.
    pub enum ScanSetting {
        /// The voice channel table.
        LcnTable = LCN_TABLE,
    }
}

/// `edacs.scan`: a Hann-windowed FFT of the raw input every N samples. For each channel of the table: the power, a floor and the
/// activity.
pub struct EdacsScan {
    table: BTreeMap<u32, f64>,
    text: String,
    rate: f64,
    centre: f64,
    n: usize,
    fft: Option<Fft>,
    win: Vec<f32>,
    wsum: f64,
    buf: Vec<Complex32>,
    fill: usize,
    states: BTreeMap<u32, ScanState>,
    t: f64,
    since_report: f64,
}

impl Default for EdacsScan {
    fn default() -> Self {
        EdacsScan {
            table: BTreeMap::new(),
            text: String::new(),
            rate: 0.0,
            centre: 0.0,
            n: 256,
            fft: None,
            win: Vec::new(),
            wsum: 1.0,
            buf: Vec::new(),
            fill: 0,
            states: BTreeMap::new(),
            t: 0.0,
            since_report: 0.0,
        }
    }
}

impl EdacsScan {
    fn rows(&self) -> Vec<ScanRow> {
        self.table
            .iter()
            .map(|(&lcn, &f)| {
                let s = self.states.get(&lcn).cloned().unwrap_or_default();
                ScanRow {
                    lcn,
                    freq_hz: f,
                    in_capture: self.centre > 0.0 && (f - self.centre).abs() < 0.42 * self.rate,
                    power: s.p,
                    floor: s.floor,
                    active: s.active,
                    last_active: if s.last_active > 0.0 {
                        s.last_active
                    } else {
                        -1.0
                    },
                }
            })
            .collect()
    }
    fn analyse(&mut self) {
        let Some(fft) = self.fft.as_mut() else { return };
        for (z, w) in self.buf.iter_mut().zip(&self.win) {
            *z *= *w;
        }
        fft.process(&mut self.buf);
        let n = self.n as f64;
        let dt = n / self.rate;
        let half = ((6250.0 / self.rate * n).round() as i64).max(1);
        for (&lcn, &f) in &self.table {
            let in_cap = self.centre > 0.0 && (f - self.centre).abs() < 0.42 * self.rate;
            let st = self.states.entry(lcn).or_default();
            if !in_cap {
                st.active = false;
                continue;
            }
            let k0 = ((f - self.centre) / self.rate * n).round() as i64;
            let mut p = 0.0;
            for k in k0 - half..=k0 + half {
                let i = k.rem_euclid(self.n as i64) as usize;
                p += f64::from(self.buf[i].norm_sqr());
            }
            p /= n * self.wsum;
            if st.started {
                st.p = 0.5 * st.p + 0.5 * p;
            } else {
                st.p = p;
                st.floor = p;
                st.started = true;
            }
            if st.p < st.floor {
                st.floor = st.p;
            } else if !st.active {
                st.floor *= 10f64.powf(0.1 * dt);
            }
            if st.active {
                if st.p < st.floor * 2.5 {
                    st.active = false;
                    st.above_since = None;
                } else {
                    st.last_active = self.t;
                }
            } else if st.p > st.floor * 6.3 && st.p > 1e-9 {
                let since = *st.above_since.get_or_insert(self.t);
                if self.t - since >= 0.06 {
                    st.active = true;
                    st.last_active = self.t;
                }
            } else {
                st.above_since = None;
            }
        }
    }
}

impl Component for EdacsScan {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "edacs.scan",
            vec![Port::iq("in")],
            vec![typed_port::<ScanReport>("out")],
        )
        .with_settings(ScanSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.centre = f.centre_hz;
        let mut n = 256;
        while (n as f64) < f.rate / 500.0 && n < 8192 {
            n *= 2;
        }
        self.n = n;
        self.fft = Some(Fft::forward(n));
        self.win = vec![0.0; n];
        Window::Hann.fill_symmetric(&mut self.win);
        self.wsum = self.win.iter().map(|w| f64::from(w * w)).sum();
        self.buf = vec![Complex32::new(0.0, 0.0); n];
        self.fill = 0;
        self.states.clear();
        self.t = 0.0;
        Ok(vec![typed_format::<ScanReport>(f)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        if self.table.is_empty() {
            self.t += x.len() as f64 / self.rate.max(1.0);
            self.since_report += x.len() as f64 / self.rate.max(1.0);
        } else {
            for &z in x {
                self.buf[self.fill] = z;
                self.fill += 1;
                if self.fill == self.n {
                    self.fill = 0;
                    self.t += self.n as f64 / self.rate;
                    self.analyse();
                    self.since_report += self.n as f64 / self.rate;
                }
            }
        }
        if self.since_report >= 0.05 {
            self.since_report = 0.0;
            let report = ScanReport {
                time: self.t,
                centre_known: self.centre > 0.0,
                rows: self.rows(),
            };
            io.items_out::<ScanReport>(0).push(Timed {
                time: self.t,
                value: report,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ScanSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ScanSetting::LcnTable => {
                let txt = LCN_TABLE.read(id, v)?;
                let t =
                    parse_table(txt).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
                txt.clone_into(&mut self.text);
                self.table = t;
                self.states.clear();
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(ScanSetting::LcnTable, Value::Text(self.text.clone()))])
    }
}

/// The record of one call in the call log.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EdacsCall {
    /// Hz (0 when not in the table).
    pub freq_hz: f64,
    /// Group or called unit (-1 unknown).
    pub gid: i64,
    /// Unit or caller (-1 unknown).
    pub lid: i64,
    /// A group call.
    pub group: bool,
    /// Emergency.
    pub emergency: bool,
    /// Digital (not decoded).
    pub digital: bool,
    /// Started from carrier activity, not a message.
    pub from_activity: bool,
    /// Never announced.
    pub carrier_only: bool,
    /// Followed now.
    pub followed: bool,
    /// Last announced (message) time.
    pub announced: f64,
}

impl SameCall for EdacsCall {
    fn same_call(&self, other: &Self) -> bool {
        self.gid == other.gid && self.group == other.group
    }
}

/// One talkgroup.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Talkgroup {
    /// Group.
    pub gid: i64,
    /// Calls.
    pub calls: u32,
    /// Last unit (-1).
    pub last_unit: i64,
    /// Last LCN.
    pub last_lcn: i32,
    /// Last frequency.
    pub freq_hz: f64,
    /// Active now.
    pub active: bool,
}

/// A good frame within this long keeps the lock, s.
const LOCK_S: f64 = 5.0;
/// The calls the log keeps.
const CALL_CAP: usize = 200;

sdrkit_core::choice_enum! {
    /// The data polarity of the control channel.
    pub enum Polarity {
        /// Normal.
        Normal = ("normal", "Normal"),
        /// Inverted.
        Inverted = ("inverted", "Inverted"),
    }
}

sdrkit_core::choice_enum! {
    /// What the follower does now, or why it cannot follow.
    pub enum FollowState {
        /// Nothing to say.
        Idle = ("idle", "Idle"),
        /// Listening to a call.
        Listening = ("listening", "Listening"),
        /// Listening to a call whose carrier is not up yet.
        Waiting = ("waiting", "Waiting for the carrier"),
        /// No voice channel table: the voice channels cannot be placed.
        NoTable = ("no_table", "No voice channel table"),
        /// The capture's frequency is not known (file input).
        NoCentre = ("no_centre", "The capture's frequency is not known"),
        /// A call is on a channel outside the capture.
        OutsideCapture = ("outside_capture", "A call is outside the captured span"),
    }
}

/// One good frame of the list.
#[derive(Clone, Debug, PartialEq)]
struct FrameRow {
    frame: EdacsFrame,
    text: String,
}

impl Shaped for FrameRow {
    type Row = EdacsFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let f = &self.frame;
        out.put(&EdacsFrameRow {
            a: i64::from(f.a),
            b: i64::from(f.b),
            err_a: i64::from(f.err_a),
            err_b: i64::from(f.err_b),
            time_s: f.time,
            text: &self.text,
        });
    }
}

impl Shaped for Talkgroup {
    type Row = EdacsTalkgroupRow;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&EdacsTalkgroupRow {
            gid: self.gid,
            calls: i64::from(self.calls),
            last_unit: (self.last_unit >= 0).then_some(self.last_unit),
            last_lcn: i64::from(self.last_lcn),
            freq_hz: (self.freq_hz > 0.0).then_some(Hertz(self.freq_hz)),
            active: self.active,
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// Also follow carriers that no message announced.
pub const FOLLOW_CARRIERS: Setting<field::Bool> =
    Setting::new("follow_carriers", "Also follow carriers", "", false)
        .help("Also follow carriers that no message announced.");

sdrkit_core::settings! {
    /// The settings of `edacs.calls`.
    pub enum CallsSetting {
        /// Follow calls.
        Follow = FOLLOW,
        /// Also follow carriers.
        FollowCarriers = FOLLOW_CARRIERS,
        /// The groups followed.
        FollowGroups = FOLLOW_GROUPS,
    }
}

const TALKGROUP_ROW: sdrkit_core::settings::Argument<sdrkit_core::publish::field::Text> =
    sdrkit_util_objects::commands::row_of(EDACS_TALKGROUPS.def().key);
const REMOVE: sdrkit_core::command::Command =
    sdrkit_util_objects::commands::remove_command(&[&TALKGROUP_ROW]);

sdrkit_core::commands! {
    /// The commands of `edacs.calls`.
    pub enum CallsCommand {
        /// Empties the frames, words, calls and talkgroups.
        Clear = CLEAR,
        /// Forgets one talkgroup.
        Remove = REMOVE,
    }
}

/// The counters and the site.
#[derive(Clone, Debug, PartialEq)]
pub struct EdacsState {
    /// Good frames.
    pub frames_good: u64,
    /// Bad frames.
    pub frames_bad: u64,
    /// The data polarity.
    pub polarity: Polarity,
    /// Site (-1 until a site ID).
    pub site: i32,
    /// Area (-1).
    pub area: i32,
    /// Control channel LCN (-1).
    pub control_lcn: i32,
    /// Highest LCN seen in a message.
    pub highest_lcn: i32,
    /// Messages (good frames).
    pub words: u64,
    /// Read as a known type.
    pub words_known: u64,
    /// dBFS.
    pub level_dbfs: f32,
    /// Hz.
    pub swing_hz: f32,
    /// Followed LCN (-1).
    pub following: i32,
    /// What the follower does.
    pub note: FollowState,
}

impl Default for EdacsState {
    fn default() -> Self {
        EdacsState {
            frames_good: 0,
            frames_bad: 0,
            polarity: Polarity::Normal,
            site: -1,
            area: -1,
            control_lcn: -1,
            highest_lcn: -1,
            words: 0,
            words_known: 0,
            level_dbfs: -200.0,
            swing_hz: 0.0,
            following: -1,
            note: FollowState::Idle,
        }
    }
}

/// `edacs.calls`: the frames and words tables, site, calls, talkgroups, the scan rows, the follow decision and the grants.
pub struct EdacsCalls {
    p: Published,
    d: EdacsState,
    follow: bool,
    follow_carriers: bool,
    groups: IdList,
    groups_text: String,
    calls: CallLog<i32, EdacsCall>,
    call_events: Vec<CallEvent<i32>>,
    talkgroups: ObjectTable<i64, Talkgroup>,
    frames: LogRing<FrameRow>,
    words: BTreeMap<u32, u32>,
    words_dirty: bool,
    scan: ScanReport,
    now: f64,
    last_good: Option<f64>,
    following: Option<u64>,
    since_report: f64,
    configured: bool,
    tuned: f64,
    followed_freq: Option<f64>,
    followed: Option<(i32, i64, i64, bool)>,
    dirty_from: u64,
    requests: Vec<HostRequest>,
    events: Vec<Event>,
    grants: Vec<Timed<Grant>>,
    key: String,
    summary: String,
}

impl Default for EdacsCalls {
    fn default() -> Self {
        EdacsCalls {
            p: Published::new(EdacsFields::SHAPE.fields).with_tables(&[
                EDACS_CALLS.def(),
                EDACS_TALKGROUPS.def(),
                EDACS_CHANNELS.def(),
                EDACS_FRAMES.def(),
                EDACS_WORDS.def(),
            ]),
            d: EdacsState::default(),
            follow: false,
            follow_carriers: false,
            groups: IdList::new(IdKind::Decimal),
            groups_text: String::new(),
            calls: CallLog::new(f64::INFINITY, 64, CALL_CAP),
            call_events: Vec::new(),
            talkgroups: ObjectTable::new(200),
            frames: LogRing::new(400),
            words: BTreeMap::new(),
            words_dirty: false,
            scan: ScanReport::default(),
            now: 0.0,
            last_good: None,
            following: None,
            since_report: 0.0,
            configured: false,
            tuned: 0.0,
            followed_freq: None,
            followed: None,
            dirty_from: u64::MAX,
            requests: Vec::new(),
            events: Vec::new(),
            grants: Vec::new(),
            key: String::new(),
            summary: String::new(),
        }
    }
}

/// The text of a radio identity, `None` when unknown.
fn ident(v: i64) -> Option<String> {
    (v >= 0).then(|| v.to_string())
}

impl EdacsCalls {
    fn freq_of(&self, lcn: i32) -> f64 {
        self.row(lcn).map_or(0.0, |r| r.freq_hz)
    }
    fn row(&self, lcn: i32) -> Option<&ScanRow> {
        self.scan.rows.iter().find(|r| r.lcn as i32 == lcn)
    }
    /// Marks call `id` and the calls after it for publishing.
    fn touch(&mut self, id: u64) {
        self.dirty_from = self.dirty_from.min(id);
    }
    /// The event of a call, at the voice channel's frequency (the control channel's when the table does not give it).
    fn call_event(&self, c: &Call<i32, EdacsCall>, at: f64) -> Event {
        let r = &c.record;
        let f = if r.freq_hz > 0.0 {
            r.freq_hz
        } else {
            self.tuned
        };
        let (source, target) = (ident(r.lid), ident(r.gid));
        EDACS_CALL.event(
            at,
            f,
            &EdacsCallEvent {
                end: c.end,
                source: source.as_deref(),
                target: target.as_deref(),
                channel: &c.channel.to_string(),
                group: r.group,
                urgency: urgency(r.emergency),
                carrier_only: r.carrier_only,
                duration: Seconds(c.duration_s()),
                family: keys::Family::Voice,
            },
        )
    }
    /// Turns the starts and ends of the call log into log lines, talkgroups and events.
    fn take_call_events(&mut self) {
        let pending = std::mem::take(&mut self.call_events);
        for e in &pending {
            let id = match *e {
                CallEvent::Started { id, .. } | CallEvent::Ended { id, .. } => id,
            };
            self.touch(id);
            let Some(c) = self.calls.get(id).cloned() else {
                continue;
            };
            let r = &c.record;
            let who = if r.group { "group" } else { "unit" };
            let gid = ident(r.gid);
            let gid = gid.as_deref().unwrap_or("?");
            match *e {
                CallEvent::Started { .. } => {
                    let only = if r.carrier_only {
                        " (carrier only)"
                    } else {
                        ""
                    };
                    self.p
                        .results_mut()
                        .log(format!("EDACS call: LCN {}{only}, {who} {gid}", c.channel));
                    if !r.carrier_only {
                        self.talkgroup(c.channel, r, c.start.t);
                    }
                    let ev = self.call_event(&c, c.start.t);
                    self.events.push(ev);
                }
                CallEvent::Ended { .. } => {
                    self.p.results_mut().log(format!(
                        "EDACS call ended: LCN {}, {who} {gid}, {:.1} s",
                        c.channel,
                        c.duration_s()
                    ));
                    if self.following == Some(id) {
                        self.following = None;
                    }
                    if let Some(c) = self.calls.get_mut(id) {
                        c.record.followed = false;
                    }
                    let ev = self.call_event(&c, c.last_s);
                    self.events.push(ev);
                }
            }
        }
        self.call_events = pending;
        self.call_events.clear();
    }
    fn talkgroup(&mut self, lcn: i32, c: &EdacsCall, at: f64) {
        if !c.group || c.gid < 0 {
            return;
        }
        if let Ok(up) = self.talkgroups.upsert(&c.gid, Stamp::at(at), || Talkgroup {
            gid: c.gid,
            last_unit: -1,
            ..Default::default()
        }) {
            let t = up.row;
            t.calls += 1;
            if c.lid >= 0 {
                t.last_unit = c.lid;
            }
            t.last_lcn = lcn;
            t.freq_hz = c.freq_hz;
        }
    }
    fn take_message(&mut self, m: &EdacsMessage) {
        self.d.words += 1;
        if m.command != Command::Unknown {
            self.d.words_known += 1;
        }
        if (1..=25).contains(&m.lcn) && m.command != Command::Unknown {
            self.d.highest_lcn = self.d.highest_lcn.max(m.lcn);
        }
        let t = m.time;
        match m.command {
            Command::SiteId => {
                self.d.site = m.site;
                self.d.area = m.area;
                if (1..=25).contains(&m.lcn) {
                    self.d.control_lcn = m.lcn;
                }
            }
            Command::Group | Command::Individual => {
                let lcn = m.lcn;
                let group = m.command == Command::Group;
                let (gid, lid) = if group {
                    (m.group, m.unit)
                } else {
                    (m.called, m.unit)
                };
                let freq = self.freq_of(lcn);
                self.grants.push(Timed {
                    time: t,
                    value: Grant {
                        protocol: "EDACS",
                        talkgroup: gid.to_string(),
                        talkgroup_id: gid,
                        caller: ident(lid).unwrap_or_default(),
                        caller_id: lid,
                        freq_hz: freq,
                        channel: lcn,
                        group,
                        emergency: m.emergency,
                        encrypted: false,
                        time: t,
                        ..Default::default()
                    },
                });
                let mut upgraded = None;
                if let Some(c) = self.calls.frame(lcn, t) {
                    let r = &mut c.record;
                    if r.from_activity {
                        r.from_activity = false;
                        r.carrier_only = false;
                        r.gid = gid;
                        r.lid = lid;
                        r.group = group;
                        r.emergency |= m.emergency;
                        r.digital = m.digital;
                        r.announced = t;
                        upgraded = Some((c.id, r.clone()));
                    } else if r.gid == gid && r.group == group {
                        r.announced = t;
                        r.emergency |= m.emergency;
                        r.digital = m.digital;
                        if lid >= 0 {
                            r.lid = lid;
                        }
                        let id = c.id;
                        self.touch(id);
                        return;
                    }
                }
                if let Some((id, r)) = upgraded {
                    self.touch(id);
                    self.talkgroup(lcn, &r, t);
                    return;
                }
                let events = &mut self.call_events;
                self.calls.begin(
                    lcn,
                    Stamp::at(t),
                    false,
                    EdacsCall {
                        freq_hz: freq,
                        gid,
                        lid,
                        group,
                        emergency: m.emergency,
                        digital: m.digital,
                        announced: t,
                        ..Default::default()
                    },
                    |e| events.push(e),
                );
                self.take_call_events();
            }
            Command::CallEnd
                if self
                    .calls
                    .call_on(m.lcn)
                    .is_some_and(|c| !c.record.from_activity) =>
            {
                let events = &mut self.call_events;
                self.calls.end(m.lcn, t, |e| events.push(e));
                self.take_call_events();
            }
            _ => {}
        }
    }
    fn report(&mut self) {
        let now = self.now;
        let ctl = self.d.control_lcn;
        // Activity: a carrier with no call starts one, a call on an active channel is refreshed, and a quiet one ends after the
        // hold.
        for i in 0..self.scan.rows.len() {
            let r = &self.scan.rows[i];
            if !r.active || r.lcn as i32 == ctl {
                continue;
            }
            let (lcn, freq_hz) = (r.lcn as i32, r.freq_hz);
            if self.calls.frame(lcn, now).is_none() {
                let events = &mut self.call_events;
                self.calls.begin(
                    lcn,
                    Stamp::at(now),
                    false,
                    EdacsCall {
                        freq_hz,
                        gid: -1,
                        lid: -1,
                        group: true,
                        from_activity: true,
                        carrier_only: true,
                        ..Default::default()
                    },
                    |e| events.push(e),
                );
            }
        }
        self.take_call_events();
        let mut quiet = Vec::new();
        for c in self.calls.active() {
            let row = self.row(c.channel);
            let active = row.is_some_and(|r| r.active);
            let in_cap = row.is_some_and(|r| r.in_capture);
            let hold = if c.record.from_activity || in_cap {
                1.5
            } else {
                4.0
            };
            if !active && now - c.last_s > hold {
                quiet.push(c.channel);
            }
        }
        for lcn in quiet {
            let events = &mut self.call_events;
            self.calls.lost(lcn, |e| events.push(e));
        }
        self.take_call_events();
        let active: Vec<u64> = self.calls.active().map(|c| c.id).collect();
        for id in active {
            self.touch(id);
        }
        self.decide_follow();
        for (gid, t) in self.talkgroups.records_mut() {
            t.active = self
                .calls
                .active()
                .any(|c| c.record.group && c.record.gid == *gid);
        }
    }
    fn qualifies(&self, c: &Call<i32, EdacsCall>) -> bool {
        let r = &c.record;
        c.is_active()
            && r.freq_hz > 0.0
            && !r.digital
            && (!r.from_activity || self.follow_carriers)
            && (self.groups.is_empty()
                || (r.group && r.gid >= 0 && self.groups.contains_number(r.gid as u64)))
            && self.row(c.channel).is_some_and(|row| row.in_capture)
    }
    fn decide_follow(&mut self) {
        let now = self.now;
        if let Some(id) = self.following {
            let keep = self.follow
                && self.calls.get(id).is_some_and(|c| {
                    let r = self.row(c.channel);
                    let carrier = r.is_some_and(|r| {
                        r.active || (r.last_active >= 0.0 && now - r.last_active <= 0.4)
                    });
                    c.is_active() && (carrier || now - c.start.t <= 1.0)
                });
            if !keep {
                if let Some(c) = self.calls.get_mut(id) {
                    c.record.followed = false;
                }
                self.touch(id);
                self.following = None;
            }
        }
        if self.follow && self.following.is_none() {
            let pick = self
                .calls
                .newest_first()
                .find(|c| self.qualifies(c) && self.row(c.channel).is_some_and(|r| r.active))
                .or_else(|| {
                    self.calls.newest_first().find(|c| {
                        self.qualifies(c)
                            && !c.record.from_activity
                            && now - c.record.announced < 1.0
                    })
                })
                .map(|c| c.id);
            if let Some(id) = pick {
                self.following = Some(id);
                if let Some(c) = self.calls.get_mut(id) {
                    c.record.followed = true;
                }
                self.touch(id);
            }
        }
        self.d.following = self
            .following
            .and_then(|id| self.calls.get(id))
            .map_or(-1, |c| c.channel);
        self.d.note = if self.d.following >= 0 {
            if self.row(self.d.following).is_some_and(|r| r.active) {
                FollowState::Listening
            } else {
                FollowState::Waiting
            }
        } else if self.scan.rows.is_empty() {
            FollowState::NoTable
        } else if !self.scan.centre_known {
            FollowState::NoCentre
        } else if self
            .calls
            .active()
            .any(|c| self.row(c.channel).is_some_and(|r| !r.in_capture))
        {
            FollowState::OutsideCapture
        } else {
            FollowState::Idle
        };
    }
    /// Requests the host to follow the call chosen now and to release the one before.
    fn request_follow(&mut self) {
        let now = self.now;
        let chosen = self.following.and_then(|id| self.calls.get(id));
        let freq = chosen.map(|c| c.record.freq_hz);
        if freq == self.followed_freq {
            return;
        }
        let chosen = chosen.map(|c| (c.channel, c.record.clone()));
        if let Some(f) = self.followed_freq.take() {
            self.requests.push(HostRequest::Release { freq_hz: f });
            let (lcn, gid, lid, emergency) = self.followed.take().unwrap_or((-1, -1, -1, false));
            self.events.push(follow_event(
                now,
                f,
                FollowAction::Release,
                lcn,
                gid,
                lid,
                emergency,
            ));
        }
        if let Some((lcn, c)) = chosen {
            let who = if c.group { "group" } else { "unit" };
            let talkgroup = if c.group {
                u32::try_from(c.gid).ok()
            } else {
                None
            };
            self.requests.push(HostRequest::Follow {
                freq_hz: c.freq_hz,
                signal: Some(VOICE_SIGNAL),
                talkgroup,
                bandwidth_hz: VOICE_BANDWIDTH_HZ,
                label: format!("EDACS {who} {}, LCN {lcn}", c.gid),
                max_s: 1800.0,
                settings: Vec::new(),
            });
            self.events.push(follow_event(
                now,
                c.freq_hz,
                FollowAction::Follow,
                lcn,
                c.gid,
                c.lid,
                c.emergency,
            ));
            self.followed_freq = Some(c.freq_hz);
            self.followed = Some((lcn, c.gid, c.lid, c.emergency));
        }
    }
    fn publish(&mut self) {
        let locked = self.last_good.is_some_and(|t| self.now - t <= LOCK_S);
        let active = self.calls.active().count();
        let lock_state = if !self.configured {
            LockState::Off
        } else if locked {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let lock = if locked { "locked" } else { "no lock" };
        let level = Some(self.d.level_dbfs);
        let r = self.p.results_mut();
        if !self.configured {
            set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("EDACS: waiting for samples"),
            );
        } else if self.d.frames_good == 0 {
            set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("EDACS: listening ({:.1} dBFS)", self.d.level_dbfs),
            );
        } else if active > 0 {
            let following = if self.d.following >= 0 {
                ", following"
            } else {
                ""
            };
            set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("EDACS: {lock}, {active} call(s) on air{following}"),
            );
        } else {
            set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("EDACS: {lock}, {} good frames", self.d.frames_good),
            );
        }
        let opt = |v: i32| (v >= 0).then_some(i64::from(v));
        let ctl = self.d.control_lcn;
        let p = &mut self.p;
        p.put(&EdacsFields {
            locked,
            frames_ok: self.d.frames_good as i64,
            frames_bad: self.d.frames_bad as i64,
            polarity: self.d.polarity,
            site: opt(self.d.site),
            area: opt(self.d.area),
            control_lcn: opt(self.d.control_lcn),
            highest_lcn: opt(self.d.highest_lcn),
            words: self.d.words as i64,
            words_known: self.d.words_known as i64,
            level_dbfs: f64::from(self.d.level_dbfs),
            swing_hz: Hertz(f64::from(self.d.swing_hz)),
            following_lcn: opt(self.d.following),
            following: self.d.note,
        });
        let now = self.now;
        let from = self.dirty_from;
        for c in self.calls.iter().filter(|c| c.id >= from) {
            self.key.clear();
            // Writing into a `String` cannot fail.
            let _ = write!(self.key, "{:08}", c.id);
            let r = &c.record;
            let dur = if c.is_active() {
                (now - c.start.t).max(0.0)
            } else {
                c.duration_s()
            };
            let channel = c.channel.to_string();
            EDACS_CALLS.put(
                p,
                &self.key,
                &EdacsCallRow {
                    source: ident(r.lid).as_deref(),
                    target: ident(r.gid).as_deref(),
                    channel: &channel,
                    freq_hz: (r.freq_hz > 0.0).then_some(Hertz(r.freq_hz)),
                    group: r.group,
                    start_s: c.start.t,
                    duration: Seconds(dur),
                    urgency: urgency(r.emergency),
                    digital: r.digital,
                    carrier_only: r.carrier_only,
                    end: c.end,
                    followed: r.followed,
                },
            );
        }
        self.dirty_from = self.calls.active().map(|c| c.id).min().unwrap_or(u64::MAX);
        rows(&self.talkgroups, &EDACS_TALKGROUPS, now, p);
        for c in &self.scan.rows {
            self.key.clear();
            // Writing into a `String` cannot fail.
            let _ = write!(self.key, "{:03}", c.lcn);
            EDACS_CHANNELS.put(
                p,
                &self.key,
                &EdacsChannelRow {
                    lcn: i64::from(c.lcn),
                    freq_hz: (c.freq_hz > 0.0).then_some(Hertz(c.freq_hz)),
                    level_db: 10.0 * c.power.max(1e-20).log10(),
                    floor_db: 10.0 * c.floor.max(1e-20).log10(),
                    in_capture: c.in_capture,
                    carrier: c.active,
                    control: c.lcn as i32 == ctl,
                },
            );
        }
        log_rows(&self.frames, &EDACS_FRAMES, now, p);
        if !std::mem::take(&mut self.words_dirty) {
            return;
        }
        let mut words: Vec<(u32, u32)> = self.words.iter().map(|(&w, &c)| (w, c)).collect();
        words.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        words.truncate(EDACS_WORDS.cap);
        let stale: Vec<String> = p
            .results()
            .decl()
            .table_id(EDACS_WORDS.key)
            .and_then(|id| p.results().table(id))
            .map(|t| {
                t.rows_newest_first()
                    .filter(|(_, r)| {
                        r.at(&EdacsWordRow::WORD)
                            .is_none_or(|w| !words.iter().any(|(x, _)| i64::from(*x) == w))
                    })
                    .map(|(k, _)| k.to_owned())
                    .collect()
            })
            .unwrap_or_default();
        for k in &stale {
            p.remove_row(&EDACS_WORDS, k);
        }
        for (w, c) in words {
            self.key.clear();
            // Writing into a `String` cannot fail.
            let _ = write!(self.key, "{w:07X}");
            EDACS_WORDS.put(
                p,
                &self.key,
                &EdacsWordRow {
                    word: i64::from(w),
                    count: i64::from(c),
                },
            );
        }
    }
    /// Empties the frames, words, calls and talkgroups (the frame counters stay).
    pub fn clear(&mut self) {
        self.words.clear();
        self.frames.clear();
        self.calls.clear();
        self.talkgroups.clear();
        self.d.words = 0;
        self.d.words_known = 0;
        self.following = None;
        for t in [
            EDACS_FRAMES.def(),
            EDACS_WORDS.def(),
            EDACS_CALLS.def(),
            EDACS_TALKGROUPS.def(),
        ] {
            self.p.clear_rows(t);
        }
    }
}

/// A follow decision on the call on `lcn`.
fn follow_event(
    t: f64,
    freq_hz: f64,
    action: FollowAction,
    lcn: i32,
    gid: i64,
    lid: i64,
    emergency: bool,
) -> Event {
    let (source, target) = (ident(lid), ident(gid));
    TRUNK_FOLLOW.event(
        t,
        freq_hz,
        &FollowEvent {
            action,
            reason: None,
            source: source.as_deref(),
            target: target.as_deref(),
            channel: (lcn >= 0).then(|| lcn.to_string()).as_deref(),
            urgency: urgency(emergency),
            family: keys::Family::Voice,
        },
    )
}

impl Component for EdacsCalls {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "edacs.calls",
            vec![
                Port::typed::<EdacsFrame>("frames"),
                Port::typed::<EdacsMessage>("messages"),
                Port::typed::<ScanReport>("scan"),
                Port::typed::<Level>("level"),
            ],
            vec![
                typed_port::<Grant>("grants"),
                typed_port::<HostRequest>("requests"),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(CallsSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let (f, fc) = (self.follow, self.follow_carriers);
        let (g, gt) = (self.groups.clone(), std::mem::take(&mut self.groups_text));
        *self = EdacsCalls::default();
        self.follow = f;
        self.follow_carriers = fc;
        self.groups = g;
        self.groups_text = gt;
        self.configured = true;
        self.tuned = inputs[0].tuned_hz;
        self.publish();
        Ok(vec![
            typed_format::<Grant>(&inputs[0]),
            typed_format::<HostRequest>(&inputs[0]),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(l) = io.items::<Level>(3).last() {
            self.d.level_dbfs = l.value.dbfs;
            self.now = self.now.max(l.time);
        }
        let mut scan_seen = false;
        for s in io.items::<ScanReport>(2) {
            self.scan.clone_from(&s.value);
            self.now = self.now.max(s.time);
            scan_seen = true;
        }
        // Messages arrive with their frames (same pass, same time).
        let msgs = io.items::<EdacsMessage>(1);
        let mut mi = 0;
        for f in io.items::<EdacsFrame>(0) {
            let fr = &f.value;
            self.now = self.now.max(f.time);
            self.d.swing_hz = fr.swing;
            if !fr.good {
                self.d.frames_bad += 1;
                continue;
            }
            self.d.frames_good += 1;
            self.last_good = Some(f.time);
            self.d.polarity = if fr.inverted {
                Polarity::Inverted
            } else {
                Polarity::Normal
            };
            self.words_dirty = true;
            *self.words.entry(fr.a).or_insert(0) += 1;
            if fr.b != 0 {
                *self.words.entry(fr.b).or_insert(0) += 1;
            }
            let text = if mi < msgs.len() {
                let m = &msgs[mi].value;
                mi += 1;
                self.take_message(m);
                m.text.clone()
            } else {
                String::new()
            };
            self.p
                .results_mut()
                .log(format!("EDACS frame {:07X} {:07X}", fr.a, fr.b));
            self.frames.push(FrameRow { frame: *fr, text });
        }
        self.since_report += if scan_seen { 0.05 } else { 0.0 };
        if self.since_report >= 0.25 - 1e-9 {
            self.since_report = 0.0;
            self.report();
        } else if scan_seen {
            self.decide_follow();
        }
        self.request_follow();
        io.items_out::<Grant>(0).append(&mut self.grants);
        let t = self.now;
        io.items_out::<HostRequest>(1)
            .extend(self.requests.drain(..).map(|r| Timed { time: t, value: r }));
        io.messages_out(2).append(&mut self.events);
        if io.tick() {
            if !scan_seen && self.scan.rows.is_empty() {
                self.report();
            }
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match CallsSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CallsSetting::Follow => {
                self.follow = FOLLOW.read(id, v)?;
                if !self.follow {
                    self.following = None;
                }
            }
            CallsSetting::FollowCarriers => self.follow_carriers = FOLLOW_CARRIERS.read(id, v)?,
            CallsSetting::FollowGroups => {
                let s = FOLLOW_GROUPS.read(id, v)?;
                self.groups = crate::follow::parse_groups(id, s)?;
                s.clone_into(&mut self.groups_text);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (CallsSetting::Follow, Value::Bool(self.follow)),
            (
                CallsSetting::FollowCarriers,
                Value::Bool(self.follow_carriers),
            ),
            (
                CallsSetting::FollowGroups,
                Value::Text(self.groups_text.clone()),
            ),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn commands(&self) -> &Commands {
        CallsCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match CallsCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            CallsCommand::Clear => self.clear(),
            CallsCommand::Remove => {
                let row = args.arg(&TALKGROUP_ROW)?;
                if !sdrkit_util_objects::rows::remove_row(&mut self.talkgroups, row) {
                    return Err(self.p.no_such_row(&EDACS_TALKGROUPS, row));
                }
                self.p.remove_row(&EDACS_TALKGROUPS, row);
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_lines() {
        let t = parse_table("856.0\n856.025\n5 857.1\n").unwrap();
        assert_eq!(t.get(&1), Some(&856.0e6));
        assert_eq!(t.get(&2), Some(&856.025e6));
        assert_eq!(t.get(&5), Some(&857.1e6));
        assert_eq!(
            parse_table("856.0\n856.025\n5 857.1\nnonsense\n0.5\n"),
            Err(22)
        );
        assert_eq!(parse_table("856.0\n0.5\n"), Err(6));
    }
}
