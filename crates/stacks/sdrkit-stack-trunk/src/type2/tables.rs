//! `type2.tables`: the message log, the talkgroups, the sites, the patches, the calls, the system fields and the grants of
//! the control channel; and `type2.follow`, the follow decision over those grants that also ends a follow when the call's
//! updates stop.

use super::code::Osw;
use super::msg::{GrantMsg, Meaning, Message, MessageLayer, SiteMsg};
use super::plan::{ChannelPlan, Channels};
use super::rx::OswItem;
use crate::follow::{FollowPolicy, FollowSetting};
use crate::grant::Grant;
use crate::typed::*;
use crate::{typed_format, typed_port};
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    ConfigError, Event, EventDef, HostRequest, LockState, Record, Schema, Setting, SettingError,
    SettingId, SettingSet, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Decoder, Format, Io, Item, Kind, Outbox, Port,
    RunContext, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::datafile::DataFile;
use sdrkit_util_objects::shaped::{self as objpub, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The most messages logged.
pub const MESSAGE_CAP: usize = 400;
/// The most talkgroups kept.
pub const TALKGROUP_CAP: usize = 200;
/// The most calls kept.
pub const CALL_CAP: usize = 200;
/// A good word within this long shows the lock, s.
pub const LOCK_S: f64 = 5.0;
/// A call with no grant or update for this long has ended, s.
pub const CALL_END_S: f64 = 1.5;
/// The protocol name of the grants.
pub const PROTOCOL: &str = "Motorola Type II";
/// What no public document confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: the whole control channel (the sync word, the address and command masks, the check constant and masks, the parity and the interleave, every command code and field, the option bits, the site band and feature codes) and the channel plans. Inbound words, Type I fleets and SmartZone OmniLink are not decoded.";

/// The channel plan: a plan file in the host's storage.
pub const PLAN: Setting<field::Text> = Setting::new("plan", "Channel plan", "", "").help(
    "The path of a plan file in the host's storage (docs/schema/plans.json): one numbered band for each row of channel numbers. Empty for no plan: grants are listed with no frequency and never followed.",
);
/// Follow unit-to-unit calls too.
pub const FOLLOW_PRIVATE: Setting<field::Bool> = Setting::new(
    "follow_private",
    "Also follow unit-to-unit calls",
    "",
    false,
)
.help("Hand private calls to the follower too. Telephone interconnect calls are never followed.");

sdrkit_core::settings! {
    /// The settings of `type2.tables`.
    pub enum TablesSetting {
        /// The channel plan.
        Plan = PLAN,
        /// Follow unit-to-unit calls too.
        FollowPrivate = FOLLOW_PRIVATE,
    }
}

/// The end of a call: the follower lets its channel go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CallEnded {
    /// The voice channel, Hz.
    pub freq_hz: f64,
}

impl Item for CallEnded {
    const KIND: &'static str = "type2.call_ended";
}

// ---------------------------------------------------------------------------------------------------------------- held

/// One logged message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MessageRow {
    /// Signal time, s.
    pub t: f64,
    /// Wall time, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall: Option<f64>,
    /// The first word's address.
    pub address: u16,
    /// The first word is a group word.
    pub group: bool,
    /// The first word's command.
    pub command: u16,
    /// The text.
    pub text: String,
}

impl Shift for MessageRow {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// One talkgroup.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TalkgroupRow {
    /// The talkgroup.
    pub talkgroup: u16,
    /// Grants counted.
    pub calls: u32,
    /// The last calling unit.
    pub last_source: u16,
    /// The last voice channel, Hz (0 unknown).
    pub last_freq_hz: f64,
    /// The last grant or update, signal time.
    pub last_t: f64,
    /// Its last grant was digital.
    pub digital: bool,
    /// Its last grant was encrypted.
    pub encrypted: bool,
    /// It is patched.
    pub patched: bool,
}

/// One site.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SiteRow {
    /// The system.
    pub system: u16,
    /// The site.
    pub site: u8,
    /// The band code.
    pub band: u8,
    /// The feature bits.
    pub features: u8,
    /// The control channel frequency, Hz (0 unknown).
    pub cc_hz: f64,
    /// An adjacent site.
    pub adjacent: bool,
}

/// One patch member.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatchRow {
    /// The talkgroup.
    pub talkgroup: u16,
    /// The member.
    pub member: u16,
    /// Signal time it was last heard.
    pub last_t: f64,
}

/// One call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CallRow {
    /// The talkgroup or the called unit.
    pub target: u16,
    /// A group call.
    pub group: bool,
    /// The calling unit.
    pub source: u16,
    /// The voice channel, Hz (0 unknown).
    pub freq_hz: f64,
    /// The channel number.
    pub channel: u16,
    /// Digital.
    pub digital: bool,
    /// Emergency.
    pub emergency: bool,
    /// Encrypted.
    pub encrypted: bool,
    /// Telephone interconnect.
    pub interconnect: bool,
    /// Start, signal time.
    pub start: f64,
    /// The last grant or update, signal time.
    pub last: f64,
    /// It still goes on.
    pub active: bool,
}

impl Shift for CallRow {
    fn shift_times(&mut self, by_s: f64) {
        self.start += by_s;
        self.last += by_s;
    }
}

/// The counters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Counters {
    /// Good words.
    pub good: u64,
    /// Bad words.
    pub bad: u64,
    /// Information bits repaired.
    pub repaired: u64,
    /// Messages (idles not counted).
    pub messages: u64,
    /// Messages understood.
    pub known: u64,
}

/// The system as heard.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct System {
    /// The system ID.
    pub id: Option<u16>,
    /// This site.
    pub site: Option<u8>,
    /// This site's feature bits.
    pub features: Option<u8>,
    /// The control channel frequency, Hz.
    pub cc_hz: Option<f64>,
    /// Type II (true) or Type I, when a status said it.
    pub type2: Option<bool>,
    /// A wide-area site or an adjacent site was heard.
    pub smartzone: bool,
}

/// What the stack keeps and saves.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Held {
    /// The messages, newest last.
    pub messages: LogRing<MessageRow>,
    /// The talkgroups.
    pub talkgroups: Vec<TalkgroupRow>,
    /// The sites.
    pub sites: Vec<SiteRow>,
    /// The patches.
    pub patches: Vec<PatchRow>,
    /// The calls, newest last.
    pub calls: LogRing<CallRow>,
    /// The counters.
    pub counters: Counters,
    /// The system.
    pub system: System,
}

impl Default for Held {
    fn default() -> Self {
        Held {
            messages: LogRing::new(MESSAGE_CAP),
            talkgroups: Vec::new(),
            sites: Vec::new(),
            patches: Vec::new(),
            calls: LogRing::new(CALL_CAP),
            counters: Counters::default(),
            system: System::default(),
        }
    }
}

impl Shift for Held {
    fn shift_times(&mut self, by_s: f64) {
        self.messages.shift_times(by_s);
        self.calls.shift_times(by_s);
        self.talkgroups.iter_mut().for_each(|t| t.last_t += by_s);
        self.patches.iter_mut().for_each(|p| p.last_t += by_s);
    }
}

impl Tables for Held {
    const FORMAT: StateFormat = StateFormat::new("trunk.type2", 1);
    fn clear(&mut self) {
        *self = Held::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.messages.adopt(saved.messages);
        self.calls.adopt(saved.calls);
        self.talkgroups = saved.talkgroups;
        self.sites = saved.sites;
        self.patches = saved.patches;
        self.counters = saved.counters;
        self.system = saved.system;
    }
}

// --------------------------------------------------------------------------------------------------------- declarations

/// The feature bits for people.
pub fn features_text(f: u8) -> String {
    if f & 0b11_0001 == 0 {
        return "failsoft".into();
    }
    let mut v = Vec::new();
    for (bit, name) in [
        (5, "VOC"),
        (3, "digital"),
        (2, "analog"),
        (1, "CVSD"),
        (0, "active"),
    ] {
        if f >> bit & 1 == 1 {
            v.push(name);
        }
    }
    v.insert(
        usize::from(f >> 5 & 1),
        if f >> 4 & 1 == 1 {
            "wide area"
        } else {
            "site trunking"
        },
    );
    v.join(", ")
}

impl Shaped for MessageRow {
    type Row = MotorolaType2MessageRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&MotorolaType2MessageRow {
            time_s: self.t,
            address: &format!("{:04X}", self.address),
            group: self.group,
            command: &format!("{:03X}", self.command),
            text: &self.text,
        });
    }
}

impl Shaped for CallRow {
    type Row = MotorolaType2CallRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let note = if self.interconnect {
            "telephone interconnect: not followed"
        } else if self.encrypted {
            "encrypted: not followed"
        } else if self.freq_hz <= 0.0 {
            "no frequency for the channel"
        } else {
            ""
        };
        out.put(&MotorolaType2CallRow {
            talkgroup: &if self.group {
                format!("{:04X}", self.target)
            } else {
                format!("unit {}", self.target)
            },
            source_unit: i64::from(self.source),
            freq_hz: (self.freq_hz > 0.0).then_some(Hertz(self.freq_hz)),
            digital: self.digital,
            emergency: self.emergency,
            encrypted: self.encrypted,
            start_s: self.start,
            duration_s: self.last - self.start,
            state: if self.active { "granted" } else { "ended" },
            note: (!note.is_empty()).then_some(note),
        });
    }
}

/// A message as an event item.
#[derive(Clone, Debug, PartialEq)]
pub struct MessageItem(pub MessageRow);

impl Item for MessageItem {
    const KIND: &'static str = "type2.message";
    fn event_def() -> Option<&'static EventDef> {
        Some(MOTOROLA_TYPE2_MESSAGE.def())
    }
    fn write_fields(&self, f: &mut Record) {
        MotorolaType2MessageEvent {
            address: &format!("{:04X}", self.0.address),
            group: self.0.group,
            command: &format!("{:03X}", self.0.command),
            text: &self.0.text,
            family: Family::Voice,
        }
        .write(f);
    }
}

/// A grant as an event item.
#[derive(Clone, Debug, PartialEq)]
pub struct GrantItem(pub Grant, pub bool);

impl Item for GrantItem {
    const KIND: &'static str = "trunk.grant";
    fn event_def() -> Option<&'static EventDef> {
        Some(TRUNK_GRANT.def())
    }
    fn write_fields(&self, f: &mut Record) {
        let g = &self.0;
        GrantEvent {
            target: &g.talkgroup,
            source: (!g.caller.is_empty()).then_some(g.caller.as_str()),
            channel: &g.channel.to_string(),
            group: g.group,
            emergency: g.emergency,
            encrypted: g.encrypted,
            digital: self.1,
            urgency: crate::follow::urgency(g.emergency),
            family: Family::Voice,
        }
        .write(f);
    }
}

// ------------------------------------------------------------------------------------------------------------- tables

/// The `type2.tables` decoder.
pub struct Type2Tables {
    held: Held,
    layer: MessageLayer,
    plan: DataFile<ChannelPlan>,
    follow_private: bool,
    now: f64,
    started: bool,
    last_good: Option<f64>,
    locked: bool,
    inverted: bool,
    swing_hz: f32,
    snr_db: Option<f32>,
    level_dbfs: Option<f32>,
    tuned_hz: f64,
    grants: Vec<Timed<Grant>>,
    grant_items: Vec<Timed<GrantItem>>,
    messages_out: Vec<Timed<MessageItem>>,
    ends: Vec<Timed<CallEnded>>,
    pending_log: Vec<String>,
    cleared: bool,
    dirty: bool,
    summary: String,
}

impl Default for Type2Tables {
    fn default() -> Self {
        Type2Tables {
            held: Held::default(),
            layer: MessageLayer::default(),
            plan: DataFile::new(ChannelPlan::parse),
            follow_private: false,
            now: 0.0,
            started: false,
            last_good: None,
            locked: false,
            inverted: false,
            swing_hz: 0.0,
            snr_db: None,
            level_dbfs: None,
            tuned_hz: 0.0,
            grants: Vec::new(),
            grant_items: Vec::new(),
            messages_out: Vec::new(),
            ends: Vec::new(),
            pending_log: Vec::new(),
            cleared: false,
            dirty: true,
            summary: String::new(),
        }
    }
}

impl Type2Tables {
    /// What the stack holds.
    pub fn held(&self) -> &Held {
        &self.held
    }

    fn take_word(&mut self, w: Osw) {
        self.layer.word(w);
        self.read(false);
    }

    fn read(&mut self, flush: bool) {
        let plan = self.plan.get().cloned();
        let mut got = Vec::new();
        self.layer
            .read(Channels(plan.as_deref()), flush, |m| got.push(m));
        for m in got {
            self.message(m, plan.as_deref());
        }
    }

    fn message(&mut self, m: Message, plan: Option<&ChannelPlan>) {
        if m.meaning == Meaning::Idle {
            return;
        }
        let t = self.now;
        let c = &mut self.held.counters;
        c.messages += 1;
        if m.meaning != Meaning::Unknown {
            c.known += 1;
        }
        let row = MessageRow {
            t,
            wall: None,
            address: m.first.address,
            group: m.first.group,
            command: m.first.command,
            text: m.text.clone(),
        };
        self.messages_out.push(Timed {
            time: t,
            value: MessageItem(row.clone()),
        });
        self.held.messages.push(row);
        self.dirty = true;
        let ch = Channels(plan);
        match m.meaning {
            Meaning::Grant(g) => self.grant(&g, ch),
            Meaning::ControlChannel {
                channel,
                system,
                adjacent: None,
            } => {
                if m.words > 1 {
                    self.held.system.id = Some(system);
                }
                let f = ch.freq(channel).0;
                if f > 0.0 {
                    self.held.system.cc_hz = Some(f);
                }
            }
            Meaning::ControlChannel {
                adjacent: Some(true),
                ..
            } => self.held.system.smartzone = true,
            Meaning::Site(s) => self.site(&s, ch),
            Meaning::SystemId(id) => self.held.system.id = Some(id),
            Meaning::Status { opcode: 1, data } => {
                self.held.system.type2 = Some(data & 0x1000 != 0)
            }
            Meaning::Patch {
                talkgroup, member, ..
            } => {
                match self
                    .held
                    .patches
                    .iter_mut()
                    .find(|p| p.talkgroup == talkgroup && p.member == member)
                {
                    Some(p) => p.last_t = t,
                    None => self.held.patches.push(PatchRow {
                        talkgroup,
                        member,
                        last_t: t,
                    }),
                }
                for tg in self
                    .held
                    .talkgroups
                    .iter_mut()
                    .filter(|r| r.talkgroup == talkgroup || r.talkgroup == member)
                {
                    tg.patched = true;
                }
            }
            Meaning::PatchCancel(tg) => {
                self.held.patches.retain(|p| p.talkgroup != tg);
                if let Some(r) = self.held.talkgroups.iter_mut().find(|r| r.talkgroup == tg) {
                    r.patched = false;
                }
            }
            _ => {}
        }
    }

    fn site(&mut self, s: &SiteMsg, ch: Channels<'_>) {
        let cc = ch.freq(s.channel).0;
        let row = SiteRow {
            system: s.system,
            site: s.site,
            band: s.band,
            features: s.features,
            cc_hz: cc,
            adjacent: s.adjacent,
        };
        match self
            .held
            .sites
            .iter_mut()
            .find(|r| r.system == s.system && r.site == s.site)
        {
            Some(r) => *r = row,
            None => self.held.sites.push(row),
        }
        let sys = &mut self.held.system;
        sys.id = Some(s.system);
        if s.adjacent {
            sys.smartzone = true;
        } else {
            sys.site = Some(s.site);
            sys.features = Some(s.features);
            if cc > 0.0 {
                sys.cc_hz = Some(cc);
            }
            if s.features & 0b11_0001 != 0 && s.features & 0x10 != 0 {
                sys.smartzone = true;
            }
        }
    }

    fn grant(&mut self, g: &GrantMsg, ch: Channels<'_>) {
        let t = self.now;
        let (freq, width) = ch.freq(g.channel);
        let tg_row = g.group.then(|| {
            match self
                .held
                .talkgroups
                .iter()
                .position(|r| r.talkgroup == g.target)
            {
                Some(i) => i,
                None => {
                    if self.held.talkgroups.len() >= TALKGROUP_CAP {
                        // The oldest talkgroup leaves.
                        if let Some(old) = self
                            .held
                            .talkgroups
                            .iter()
                            .enumerate()
                            .min_by(|a, b| a.1.last_t.total_cmp(&b.1.last_t))
                            .map(|(i, _)| i)
                        {
                            self.held.talkgroups.remove(old);
                        }
                    }
                    self.held.talkgroups.push(TalkgroupRow {
                        talkgroup: g.target,
                        ..TalkgroupRow::default()
                    });
                    self.held.talkgroups.len() - 1
                }
            }
        });
        let digital = match (g.digital, tg_row) {
            (Some(d), _) => d,
            (None, Some(i)) => self.held.talkgroups[i].digital,
            (None, None) => false,
        };
        if let Some(i) = tg_row {
            let r = &mut self.held.talkgroups[i];
            if !g.update {
                r.calls += 1;
                r.last_source = g.source;
                r.digital = digital;
            }
            r.last_freq_hz = freq;
            r.last_t = t;
            r.encrypted = g.encrypted;
        }
        let open = self.held.calls.iter_mut().rev().find(|c| {
            c.active && c.target == g.target && c.group == g.group && c.channel == g.channel
        });
        match open {
            Some(c) => {
                c.last = t;
                if !g.update {
                    c.source = g.source;
                }
            }
            None => {
                self.held.calls.push(CallRow {
                    target: g.target,
                    group: g.group,
                    source: g.source,
                    freq_hz: freq,
                    channel: g.channel,
                    digital,
                    emergency: g.emergency,
                    encrypted: g.encrypted,
                    interconnect: g.interconnect,
                    start: t,
                    last: t,
                    active: true,
                });
            }
        }
        let grant = Grant {
            protocol: PROTOCOL,
            talkgroup: if g.group {
                format!("{:04X}", g.target)
            } else {
                g.target.to_string()
            },
            talkgroup_id: i64::from(g.target),
            caller: if g.source != 0 {
                g.source.to_string()
            } else {
                String::new()
            },
            caller_id: if g.source != 0 {
                i64::from(g.source)
            } else {
                -1
            },
            freq_hz: freq,
            channel: i32::from(g.channel),
            group: g.group,
            emergency: g.emergency,
            encrypted: g.encrypted,
            digital,
            bandwidth_hz: width,
            time: t,
            ..Grant::default()
        };
        self.grant_items.push(Timed {
            time: t,
            value: GrantItem(grant.clone(), digital),
        });
        // Interconnect calls are never followed; unit-to-unit calls only with the setting.
        if !g.interconnect && (g.group || self.follow_private) {
            self.grants.push(Timed {
                time: t,
                value: grant,
            });
        }
    }

    /// Ends the calls whose grants and updates stopped.
    fn expire(&mut self) {
        let now = self.now;
        for c in self.held.calls.iter_mut() {
            if c.active && now - c.last > CALL_END_S {
                c.active = false;
                self.dirty = true;
                if c.freq_hz > 0.0 {
                    self.ends.push(Timed {
                        time: now,
                        value: CallEnded { freq_hz: c.freq_hz },
                    });
                }
            }
        }
    }

    fn report_state(&self) -> i64 {
        if !self.started {
            0
        } else if self.last_good.is_some_and(|t| self.now - t <= LOCK_S) {
            2
        } else {
            1
        }
    }
}

impl Decoder for Type2Tables {
    type Input = Timed<OswItem>;
    fn name(&self) -> &'static str {
        "type2.tables"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<Grant>("grants"),
            Port::typed::<CallEnded>("ends"),
            Port::typed::<GrantItem>("grant_events"),
            Port::typed::<MessageItem>("messages"),
        ]
    }
    fn settings(&self) -> Arc<Schema> {
        TablesSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (TablesSetting::Plan, Value::Text(self.plan.path().into())),
            (
                TablesSetting::FollowPrivate,
                Value::Bool(self.follow_private),
            ),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match TablesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TablesSetting::Plan => {
                let path = PLAN.read(id, v)?;
                if self.plan.set_path(id, path)? {
                    self.dirty = true;
                }
            }
            TablesSetting::FollowPrivate => self.follow_private = FOLLOW_PRIVATE.read(id, v)?,
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::new(MotorolaType2Fields::SHAPE.fields).with_tables(&[
            MOTOROLA_TYPE2_MESSAGES.def(),
            MOTOROLA_TYPE2_TALKGROUPS.def(),
            MOTOROLA_TYPE2_SITES.def(),
            MOTOROLA_TYPE2_PATCHES.def(),
            MOTOROLA_TYPE2_CALLS.def(),
        ])
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.tuned_hz = input.tuned_hz;
        self.layer.clear();
        Ok(vec![
            typed_format::<Grant>(input),
            typed_format::<CallEnded>(input),
            typed_format::<GrantItem>(input),
            typed_format::<MessageItem>(input),
        ])
    }
    fn retune(&mut self, input: &Format) {
        self.tuned_hz = input.tuned_hz;
        self.layer.clear();
    }
    fn process(&mut self, x: &[Timed<OswItem>], _first: u64, out: &mut Outbox) {
        for it in x {
            self.started = true;
            self.now = self.now.max(it.time);
            match it.value {
                OswItem::Good(d) => {
                    let c = &mut self.held.counters;
                    c.good += 1;
                    c.repaired += u64::from(d.repaired);
                    self.last_good = Some(it.time);
                    self.take_word(d.osw);
                }
                OswItem::Lost => {
                    self.held.counters.bad += 1;
                    self.layer.lost();
                }
                OswItem::Level(l) => self.level_dbfs = Some(l),
                OswItem::State {
                    locked,
                    inverted,
                    swing_hz,
                    snr_db,
                } => {
                    self.locked = locked;
                    self.inverted = inverted;
                    self.swing_hz = swing_hz;
                    if snr_db.is_some() {
                        self.snr_db = snr_db;
                    }
                }
            }
        }
        self.expire();
        out.items::<Grant>(0).append(&mut self.grants);
        out.items::<CallEnded>(1).append(&mut self.ends);
        out.items::<GrantItem>(2).append(&mut self.grant_items);
        out.items::<MessageItem>(3).append(&mut self.messages_out);
    }
    fn finish(&mut self, out: &mut Outbox) {
        self.read(true);
        out.items::<Grant>(0).append(&mut self.grants);
        out.items::<GrantItem>(2).append(&mut self.grant_items);
        out.items::<MessageItem>(3).append(&mut self.messages_out);
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.now;
        let state = self.report_state();
        let h = &self.held;
        let c = &h.counters;
        let s = &h.system;
        let system = s.id.map(|id| format!("{id:04X}"));
        let features = s.features.map(features_text);
        let plan_name = self.plan.get().map(|p| p.name.clone());
        let network = if s.smartzone { "SmartZone" } else { "SmartNet" };
        p.put(&MotorolaType2Fields {
            locked: state == 2,
            rx_state: state,
            good_words: c.good as i64,
            bad_words: c.bad as i64,
            bits_repaired: c.repaired as i64,
            messages: c.messages as i64,
            messages_known: c.known as i64,
            system: system.as_deref(),
            site_id: s.site.map(i64::from),
            features: features.as_deref(),
            control_channel_hz: s.cc_hz.map(Hertz),
            system_type: s.type2.map(|t| if t { "Type II" } else { "Type I" }),
            network,
            inverted: self.inverted,
            swing_hz: f64::from(self.swing_hz),
            snr_db: self.snr_db.map(f64::from),
            level_dbfs: self.level_dbfs.map(f64::from),
            plan: plan_name.as_deref(),
            note: NOT_CONFIRMED,
        });
        if self.cleared {
            self.cleared = false;
            p.results_mut().clear_log();
        }
        if std::mem::take(&mut self.dirty) {
            objpub::log_rows(&self.held.messages, &MOTOROLA_TYPE2_MESSAGES, now, p);
            objpub::log_rows(&self.held.calls, &MOTOROLA_TYPE2_CALLS, now, p);
            let mut tgs: Vec<&TalkgroupRow> = self.held.talkgroups.iter().collect();
            tgs.sort_by(|a, b| b.last_t.total_cmp(&a.last_t));
            for (i, r) in tgs.iter().enumerate() {
                let active = self
                    .held
                    .calls
                    .iter()
                    .any(|c| c.active && c.group && c.target == r.talkgroup);
                MOTOROLA_TYPE2_TALKGROUPS.put(
                    p,
                    &format!("{i:04}"),
                    &MotorolaType2TalkgroupRow {
                        talkgroup: &format!("{:04X}", r.talkgroup),
                        calls: i64::from(r.calls),
                        last_source: (r.last_source != 0).then_some(i64::from(r.last_source)),
                        freq_hz: (r.last_freq_hz > 0.0).then_some(Hertz(r.last_freq_hz)),
                        last_s: r.last_t,
                        active,
                        digital: r.digital,
                        encrypted: r.encrypted,
                        patched: r.patched,
                    },
                );
            }
            for (i, r) in self.held.sites.iter().enumerate() {
                MOTOROLA_TYPE2_SITES.put(
                    p,
                    &format!("{i:04}"),
                    &MotorolaType2SiteRow {
                        system: &format!("{:04X}", r.system),
                        site: i64::from(r.site),
                        band: i64::from(r.band),
                        features: &features_text(r.features),
                        control_channel_hz: (r.cc_hz > 0.0).then_some(Hertz(r.cc_hz)),
                        adjacent: r.adjacent,
                    },
                );
            }
            for (i, r) in self.held.patches.iter().enumerate() {
                MOTOROLA_TYPE2_PATCHES.put(
                    p,
                    &format!("{i:04}"),
                    &MotorolaType2PatchRow {
                        talkgroup: &format!("{:04X}", r.talkgroup),
                        member: &format!("{:04X}", r.member),
                        last_s: r.last_t,
                    },
                );
            }
            for line in self.pending_log.drain(..) {
                p.results_mut().log(line);
            }
        }
        let lock_state = match state {
            0 => LockState::Off,
            2 => LockState::Locked,
            _ => LockState::Searching,
        };
        let level = self.level_dbfs;
        let active = self.held.calls.iter().filter(|c| c.active).count();
        let good = self.held.counters.good;
        let lock_word = if state == 2 { "locked" } else { "searching" };
        let no_plan = if self.plan.get().is_none() {
            "; no channel plan: calls cannot be placed"
        } else {
            ""
        };
        let r = p.results_mut();
        if state == 0 {
            crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("Type II: waiting for samples"),
            );
        } else if active > 0 {
            crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("{network}: {lock_word}, {active} calls active{no_plan}"),
            );
        } else {
            crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("{network}: {lock_word}, {good} good words{no_plan}"),
            );
        }
    }
    fn clear(&mut self) {
        self.held.clear();
        self.layer.clear();
        self.pending_log.clear();
        self.cleared = true;
        self.dirty = true;
    }
    fn attach(&mut self, ctx: &RunContext) {
        if self.plan.attach(Arc::clone(&ctx.storage)) {
            self.dirty = true;
        }
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.held.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.held.restore_state(state, self.now)?;
        self.layer.clear();
        self.cleared = true;
        self.dirty = true;
        Ok(())
    }
}

// ------------------------------------------------------------------------------------------------------------- follow

/// `type2.follow`: the follow decision of `trunk.follow` over the grants, which also lets a channel go when its call ends.
#[derive(Default)]
pub struct Type2Follow {
    policy: FollowPolicy,
    now: f64,
    requests: Vec<HostRequest>,
    events: Vec<Event>,
}

impl Component for Type2Follow {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "type2.follow",
            vec![Port::typed::<Grant>("in"), Port::typed::<CallEnded>("ends")],
            vec![
                typed_port::<HostRequest>("requests"),
                Port::new("events", Kind::Messages).rate_from(sdrkit_graph::RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FollowSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.policy = self.policy.restarted();
        Ok(vec![
            typed_format::<HostRequest>(&inputs[0]),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for g in io.items::<Grant>(0) {
            self.now = self.now.max(g.time);
            self.policy
                .grant(&g.value, g.time, &mut self.requests, &mut self.events);
        }
        for e in io.items::<CallEnded>(1) {
            self.now = self.now.max(e.time);
            self.policy.ended(
                e.value.freq_hz,
                e.time,
                &mut self.requests,
                &mut self.events,
            );
        }
        let t = self.now;
        io.items_out::<HostRequest>(0)
            .extend(self.requests.drain(..).map(|r| Timed { time: t, value: r }));
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let was_on = self.policy.on;
        self.policy.apply(id, v)?;
        if was_on && !self.policy.on {
            self.policy
                .stop(self.now, &mut self.requests, &mut self.events);
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.policy.values()
    }
}
