//! `reticulum`: the search plan where the session is tuned (`reticulum.channels`), the packet log and the destination table
//! (`reticulum.decode`), and the test signal. Data stays encrypted: the stack shows headers and checks announces.

use super::{count, show_status};
use crate::components::channels::{Input, Planner};
use crate::gen::{lora_burst, SceneGen, AMP};
use crate::proto::reticulum::{self as rns, hex, Identity, PacketType, SplitStore};
use crate::typed::{
    RnsAnnounceEvent, RnsDestinationEvent, RnsDestinationRow, RnsFields, RnsPacketRow,
    RNS_ANNOUNCE, RNS_DESTINATION, RNS_DESTINATIONS, RNS_PACKETS,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Clock, ConfigError, Event, GenConfig, LockState, Record, Results, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Port, RateFrom,
    RunContext,
};
use sdrkit_proto_lora_phy::bank::{BankChannel, BankPlan};
use sdrkit_proto_lora_phy::components::BankState;
use sdrkit_proto_lora_phy::params::{fmt_khz, LoraParams};
use sdrkit_proto_lora_phy::rx::{DetectorConfig, LoraFrame};
use sdrkit_util_objects::commands::CLEAR;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    fingerprint, Dedup, Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp, Tables,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

/// The radio is tuned this far above the dial: a 250 kHz channel then still fits the capture at 1 Msps.
pub const TUNE_OFFSET_HZ: f64 = 250e3;
/// Packets kept in the log.
pub const PACKET_CAP: usize = 300;
/// Destinations kept.
pub const DESTINATION_CAP: usize = 100;
/// The bandwidths of the search, Hz.
pub const SEARCH_BW: [f64; 3] = [125e3, 250e3, 62.5e3];
/// The spreading factors of the search.
pub const SEARCH_SF: std::ops::RangeInclusive<u8> = 7..=11;

sdrkit_core::choice_enum! {
    /// The bandwidth of the fixed plan. The keys are the bandwidths in Hz.
    pub enum FixedBw {
        /// 62.5 kHz.
        Bw62_5 = ("62500", "62.5 kHz"),
        /// 125 kHz.
        Bw125 = ("125000", "125 kHz"),
        /// 250 kHz.
        Bw250 = ("250000", "250 kHz"),
        /// 500 kHz.
        Bw500 = ("500000", "500 kHz"),
    }
}

impl FixedBw {
    /// The bandwidth, Hz.
    pub fn hz(self) -> f64 {
        match self {
            FixedBw::Bw62_5 => 62.5e3,
            FixedBw::Bw125 => 125e3,
            FixedBw::Bw250 => 250e3,
            FixedBw::Bw500 => 500e3,
        }
    }
}

/// Search the bandwidths 125, 250 and 62.5 kHz with the spreading factors 7 to 11.
pub const AUTO_PLAN: Setting<field::Bool> = Setting::new(
    "auto_plan",
    "Search bandwidth and spreading factor",
    "",
    true,
)
.section("LoRa");
/// The bandwidth when the search is off.
pub const BW: Setting<field::Choice<FixedBw>> =
    Setting::choice("bw", "Bandwidth", FixedBw::Bw125).section("LoRa");
/// The spreading factor when the search is off.
pub const SF: Setting<field::I64> = Setting::new("sf", "Spreading factor", "", 8)
    .section("LoRa")
    .range(5.0, 12.0, 1.0);
/// The test signal sends one announce in two radio frames.
pub const GEN_SPLIT: Setting<field::Bool> =
    Setting::new("gen_split", "Test signal: a split packet", "", true)
        .section("Test signal")
        .advanced();
/// The test signal sends one announce with a wrong signature.
pub const GEN_TAMPERED: Setting<field::Bool> = Setting::new(
    "gen_tampered",
    "Test signal: an announce with a wrong signature",
    "",
    false,
)
.section("Test signal")
.advanced();

sdrkit_core::settings! {
    /// The settings of `reticulum.channels`.
    pub enum RnsPlanSetting {
        /// Search bandwidth and spreading factor.
        AutoPlan = AUTO_PLAN,
        /// Bandwidth.
        Bw = BW,
        /// Spreading factor.
        Sf = SF,
    }
}

sdrkit_core::settings! {
    /// The settings of `reticulum`.
    pub enum RnsSetting {
        /// Search bandwidth and spreading factor.
        AutoPlan = AUTO_PLAN,
        /// Bandwidth.
        Bw = BW,
        /// Spreading factor.
        Sf = SF,
        /// A split packet in the test signal.
        GenSplit = GEN_SPLIT,
        /// An announce with a wrong signature in the test signal.
        GenTampered = GEN_TAMPERED,
    }
}

/// The plan of `reticulum` as an item (`reticulum.plan`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RnsPlan {
    /// The bank.
    pub bank: BankPlan,
    /// What is searched, in words.
    pub line: String,
}

impl Item for RnsPlan {
    const KIND: &'static str = "reticulum.plan";
}

/// The planner of `reticulum`: the detectors where the session is tuned. Each network selects its own frequency, bandwidth
/// and spreading factor.
pub struct RnsPlanner {
    auto_plan: bool,
    bw: FixedBw,
    sf: u8,
}

impl Default for RnsPlanner {
    fn default() -> Self {
        RnsPlanner {
            auto_plan: true,
            bw: FixedBw::Bw125,
            sf: 8,
        }
    }
}

impl Planner for RnsPlanner {
    type Item = RnsPlan;
    type Setting = RnsPlanSetting;
    const NAME: &'static str = "reticulum.channels";

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<bool, SettingError> {
        match RnsPlanSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RnsPlanSetting::AutoPlan => self.auto_plan = AUTO_PLAN.read(id, v)?,
            RnsPlanSetting::Bw => self.bw = BW.read(id, v)?,
            RnsPlanSetting::Sf => {
                self.sf =
                    u8::try_from(SF.read(id, v)?).map_err(|_| SettingError::NotPermitted { id })?;
            }
        }
        Ok(false)
    }

    fn values(&self) -> Values {
        values_of([
            (RnsPlanSetting::AutoPlan, Value::Bool(self.auto_plan)),
            (RnsPlanSetting::Bw, Value::Choice(self.bw.index())),
            (RnsPlanSetting::Sf, Value::I64(i64::from(self.sf))),
        ])
    }

    /// The detectors at the dial: the search (a bandwidth is left out when twice the bandwidth is more than the rate), or the
    /// one bandwidth and spreading factor of the fixed plan.
    fn plan(&self, input: &Input) -> (BankPlan, RnsPlan) {
        let dial = input.dial();
        let (bws, sfs): (Vec<f64>, Vec<u8>) = if self.auto_plan {
            (
                SEARCH_BW
                    .iter()
                    .copied()
                    .filter(|bw| 2.0 * bw <= input.rate)
                    .collect(),
                SEARCH_SF.collect(),
            )
        } else {
            (vec![self.bw.hz()], vec![self.sf])
        };
        let mut tag = 0u32;
        let mut channels = Vec::new();
        for &bw in &bws {
            let detectors = sfs
                .iter()
                .map(|&sf| {
                    let mut d = DetectorConfig::new(sf, bw, rns::SYNC);
                    d.preamble = 8;
                    d.tag = tag;
                    tag += 1;
                    d
                })
                .collect();
            channels.push(BankChannel {
                freq_hz: dial,
                bw,
                detectors,
            });
        }
        let widths = bws
            .iter()
            .map(|b| format!("{} kHz", fmt_khz(*b)))
            .collect::<Vec<_>>()
            .join(", ");
        let line = if self.auto_plan {
            format!("SF7 - SF11, {widths}")
        } else {
            format!("SF{}, {widths}", self.sf)
        };
        let bank = BankPlan {
            channels,
            fft_from: 3,
        };
        let item = RnsPlan {
            bank: bank.clone(),
            line,
        };
        (bank, item)
    }
}

/// One row of the packet log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PacketRow {
    /// Wall-clock time, when known.
    pub wall: Option<f64>,
    /// Signal time, seconds.
    pub time: f64,
    /// The destination hash in hex. Empty for a packet of an authenticated interface.
    pub destination: String,
    /// The packet type.
    pub kind: PacketType,
    /// The content in one line.
    pub content: String,
    /// The frequency, Hz.
    pub freq_hz: f64,
    /// The SNR, dB.
    pub snr_db: f64,
    /// The data is encrypted.
    pub encrypted: bool,
    /// The first 64 bytes.
    pub head: Vec<u8>,
    /// The bandwidth and the spreading factor of the detector.
    pub setting: String,
}

impl Shift for PacketRow {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for PacketRow {
    type Row = RnsPacketRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&RnsPacketRow {
            utc: self.wall.map(Time),
            time_s: self.time,
            destination: &self.destination,
            r#type: self.kind,
            content: &self.content,
            freq_hz: Hertz(self.freq_hz),
            snr_db: self.snr_db,
            encrypted: self.encrypted,
            hex: Arc::from(self.head.as_slice()),
            setting: &self.setting,
        });
    }
}

/// One destination of the table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DestinationRow {
    /// The destination hash in hex.
    pub destination: String,
    /// The type of the last packet.
    pub kind: PacketType,
    /// The content of the last packet.
    pub content: String,
    /// The identity hash of the last good announce, in hex.
    pub identity: String,
    /// The application text of the last good announce.
    pub app_text: String,
}

impl Shift for DestinationRow {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for DestinationRow {
    type Row = RnsDestinationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&RnsDestinationRow {
            destination: &self.destination,
            r#type: self.kind,
            content: &self.content,
            packets: columns::heard(seen),
            identity: &self.identity,
            app_text: &self.app_text,
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// What `reticulum.decode` keeps across sessions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heard {
    /// The packet log, oldest first.
    pub packets: LogRing<PacketRow>,
    /// The destinations by their hash in hex.
    pub destinations: ObjectTable<String, DestinationRow>,
    /// Good packets.
    pub good: u64,
    /// Packets that failed a rule.
    pub bad: u64,
}

impl Default for Heard {
    fn default() -> Self {
        Heard {
            packets: LogRing::new(PACKET_CAP),
            destinations: ObjectTable::new(DESTINATION_CAP),
            good: 0,
            bad: 0,
        }
    }
}

impl Shift for Heard {
    fn shift_times(&mut self, by_s: f64) {
        self.packets.shift_times(by_s);
        self.destinations.shift_times(by_s);
    }
}

impl Tables for Heard {
    const FORMAT: StateFormat = StateFormat::new("lora.reticulum", 1);
    fn clear(&mut self) {
        self.packets.clear();
        self.destinations.clear();
        self.good = 0;
        self.bad = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.packets.adopt(saved.packets);
        self.destinations.adopt(saved.destinations);
        self.good = saved.good;
        self.bad = saved.bad;
    }
}

sdrkit_core::commands! {
    /// The commands of `reticulum.decode`.
    pub enum RnsCommand {
        /// Forgets the tables and the counters.
        Clear = CLEAR,
    }
}

/// The `reticulum.decode` component.
pub struct RnsDecode {
    /// What was heard.
    pub heard: Heard,
    p: Published,
    plan: RnsPlan,
    stores: BTreeMap<u32, SplitStore>,
    recent: Dedup<u64>,
    now: f64,
    level: Option<f64>,
    preambles: u64,
    last_preamble: f64,
    last_good: f64,
    last: Option<(f64, f64)>,
    live: bool,
    clock: Clock,
    events: Vec<Event>,
    text: String,
}

impl Default for RnsDecode {
    fn default() -> Self {
        RnsDecode {
            heard: Heard::default(),
            p: crate::typed::results("reticulum"),
            plan: RnsPlan::default(),
            stores: BTreeMap::new(),
            recent: Dedup::new(0.25, 12),
            now: 0.0,
            level: None,
            preambles: 0,
            last_preamble: -1e9,
            last_good: -1e9,
            last: None,
            live: false,
            clock: Clock::fixed(0.0),
            events: Vec::new(),
            text: String::new(),
        }
    }
}

impl RnsDecode {
    /// Takes one Reticulum packet heard at signal time `t` on `freq_hz` by the detector `setting`. Returns whether it is good.
    pub fn take(
        &mut self,
        t: f64,
        freq_hz: f64,
        packet: &[u8],
        snr_db: f64,
        setting: &str,
    ) -> bool {
        let Ok(p) = rns::parse(packet) else {
            self.heard.bad += 1;
            return false;
        };
        self.heard.good += 1;
        self.last_good = t;
        let wall = self.live.then(|| self.clock.now());
        let destination = if p.access {
            String::new()
        } else {
            hex(&p.destination)
        };
        let content = p.content();
        self.heard.packets.push(PacketRow {
            wall,
            time: t,
            destination: destination.clone(),
            kind: p.kind(),
            content: content.clone(),
            freq_hz,
            snr_db,
            encrypted: p.encrypted(),
            head: packet[..packet.len().min(64)].to_vec(),
            setting: setting.to_string(),
        });
        if p.access {
            return true;
        }
        let stamp = match wall {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        };
        let Ok(up) = self
            .heard
            .destinations
            .upsert(&destination, stamp, || DestinationRow {
                destination: destination.clone(),
                kind: p.kind(),
                content: String::new(),
                identity: String::new(),
                app_text: String::new(),
            })
        else {
            return true;
        };
        if up.new {
            self.events.push(RNS_DESTINATION.event(
                t,
                freq_hz,
                &RnsDestinationEvent {
                    source: &destination,
                    text: &format!("Reticulum: new destination {destination}"),
                    family: Family::Message,
                },
            ));
        }
        let row = up.row;
        row.kind = p.kind();
        row.content = content;
        if let Some(a) = &p.announce {
            row.identity = hex(&a.identity);
            row.app_text = a.text.clone().unwrap_or_default();
            self.events.push(RNS_ANNOUNCE.event(
                t,
                freq_hz,
                &RnsAnnounceEvent {
                    source: &destination,
                    identity: &row.identity,
                    text: &row.app_text,
                    family: Family::Message,
                },
            ));
        }
        true
    }

    fn publish(&mut self) {
        let h = &self.heard;
        let state = if self.now - self.last_good < 8.0 {
            LockState::Locked
        } else if self.now - self.last_preamble < 8.0 {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        let t = &mut self.text;
        t.clear();
        if h.good > 0 {
            let _ = write!(t, "{} frames from {} link(s)", h.good, h.destinations.len());
        } else if self.preambles > 0 {
            let _ = write!(t, "{} LoRa preambles, no frame decoded yet", self.preambles);
        } else {
            t.push_str("No signal");
        }
        let p = &mut self.p;
        show_status(
            p,
            state,
            self.last.map(|l| l.0),
            self.last.map(|l| l.1),
            self.level,
            t,
        );
        p.put(&RnsFields {
            frames_ok: count(h.good),
            frames_bad: count(h.bad),
            preambles: count(self.preambles),
            snr_db: self.last.map(|l| l.0),
            level_dbfs: self.level,
            offset_hz: self.last.map(|l| Hertz(l.1)),
            searching: &self.plan.line,
            destinations_heard: count(h.destinations.len()),
        });
        shaped::log_rows(&h.packets, &RNS_PACKETS, self.now, p);
        shaped::rows(&h.destinations, &RNS_DESTINATIONS, self.now, p);
    }
}

impl Component for RnsDecode {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "reticulum.decode",
            vec![
                Port::typed::<LoraFrame>("frames"),
                Port::typed::<BankState>("state"),
                Port::typed::<RnsPlan>("plan"),
            ],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.clock = ctx.clock.clone();
    }

    fn process(&mut self, io: &mut Io) {
        if let Some(pl) = io.items::<RnsPlan>(2).last() {
            if pl.value.line != self.plan.line {
                self.p
                    .results_mut()
                    .log(format!("Reticulum: searching {}", pl.value.line));
            }
            if pl.value.bank != self.plan.bank {
                self.stores.clear();
            }
            self.plan = pl.value.clone();
        }
        for s in io.items::<BankState>(1) {
            self.now = s.value.time;
            self.level = Some(s.value.level_db);
            let n: u64 = s.value.detectors.iter().map(|d| d.stats.preambles).sum();
            if n > self.preambles {
                self.last_preamble = s.value.time;
            }
            self.preambles = n;
        }
        for f in io.items::<LoraFrame>(0) {
            let fr = &f.value;
            // A frame without a good CRC is not counted. A second detector that found the same frame is dropped.
            if !(fr.crc_present && fr.crc_ok) || self.recent.seen(fingerprint(&fr.payload), f.time)
            {
                continue;
            }
            let store = self.stores.entry(fr.tag).or_default();
            let Some(packet) = store.take(f.time, &fr.payload) else {
                continue;
            };
            let setting = format!("{} kHz SF{}", fmt_khz(fr.bw), fr.sf);
            if self.take(
                f.time,
                fr.channel_hz + fr.cfo_hz,
                &packet,
                fr.snr_db,
                &setting,
            ) {
                self.last = Some((fr.snr_db, fr.cfo_hz));
            }
        }
        if io.tick() {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }

    fn apply(&mut self, id: SettingId, _v: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }

    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }

    fn commands(&self) -> &Commands {
        RnsCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match RnsCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RnsCommand::Clear => {
                self.heard.clear();
                self.stores.clear();
                self.last = None;
                self.last_good = -1e9;
                self.p.clear_rows(&RNS_PACKETS);
                self.p.clear_rows(&RNS_DESTINATIONS);
            }
        }
        self.publish();
        Ok(None)
    }

    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }

    fn forget(&mut self, kept: &Kept) {
        let h = &mut self.heard;
        if !kept.table(RNS_PACKETS.key) {
            h.packets.clear();
            self.p.clear_rows(&RNS_PACKETS);
        }
        if !kept.table(RNS_DESTINATIONS.key) {
            h.destinations.clear();
            self.p.clear_rows(&RNS_DESTINATIONS);
        }
        if !kept.field("frames_ok") {
            h.good = 0;
        }
        if !kept.field("frames_bad") {
            h.bad = 0;
        }
        if !kept.field("preambles") {
            self.preambles = 0;
        }
        self.level = None;
        self.last = None;
        self.last_preamble = -1e9;
        self.last_good = -1e9;
        self.publish();
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.heard.save_state(self.now).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let now = self.now;
        self.heard.restore_state(state, now)?;
        self.publish();
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The test signal.

/// The LoRa setting of an RNode radio: CR 4/5, preamble 18, sync 0x12, CRC on.
pub fn tx_params(bw_hz: f64, sf: u8) -> LoraParams {
    let mut p = LoraParams::new(sf, bw_hz, 5, 18, rns::SYNC);
    p.crc = true;
    p
}

/// The three identities of the test scene and the names of their destinations.
pub fn scene_identities() -> [(Identity, &'static str); 3] {
    [
        (Identity::new([0x31; 32], [0x41; 32]), "lxmf.delivery"),
        (Identity::new([0x32; 32], [0x42; 32]), "nomadnetwork.node"),
        (Identity::new([0x33; 32], [0x43; 32]), "sdrkit.test"),
    ]
}

/// The Reticulum packets of cycle `cycle` of the test scene, each with whether it goes in two radio frames: announces of three
/// destinations (one with application text, one with a ratchet key, one split), encrypted data packets and a proof.
pub fn scene_packets(cycle: u32, split: bool, tampered: bool) -> Vec<(Vec<u8>, bool)> {
    let ids = scene_identities();
    let c = cycle.to_le_bytes();
    let random = |k: u8| -> [u8; 10] { [k, c[0], c[1], c[2], c[3], 0x5A, k, 0x11, 0x22, 0x33] };
    let dest = |i: usize| ids[i].0.destination(ids[i].1);
    let mut v = vec![
        (
            ids[0].0.announce(ids[0].1, 0, random(1), None, b"node one"),
            false,
        ),
        (rns::build(0x00, 1, &dest(0), 0, &[0xC5 ^ c[0]; 48]), false),
        (
            ids[1]
                .0
                .announce(ids[1].1, 1, random(2), Some([0x77; 32]), b""),
            false,
        ),
        (rns::build(0x03, 0, &dest(0), 0, &[0x3A; 64]), false),
        (
            ids[2]
                .0
                .announce(ids[2].1, 2, random(3), None, b"a split announce"),
            split,
        ),
        (rns::build(0x00, 2, &dest(1), 0, &[0x99; 32]), false),
    ];
    if tampered {
        let mut a = ids[0].0.announce(ids[0].1, 0, random(4), None, b"node one");
        a[120] ^= 0x40;
        v.push((a, false));
    }
    v
}

/// The test signal of `reticulum`: the scene at the bandwidth and spreading factor given, the channel [`TUNE_OFFSET_HZ`] below
/// the centre of the samples.
pub fn generator(rate: f64, bw_hz: f64, sf: u8, split: bool, tampered: bool) -> SceneGen {
    let p = tx_params(bw_hz, sf);
    let gap = 0.12;
    let cycle_s: f64 = scene_packets(0, split, tampered)
        .iter()
        .map(|(packet, two)| {
            rns::radio_frames(packet, 0, *two, 100)
                .iter()
                .map(|f| p.airtime(f.len()) + gap)
                .sum::<f64>()
        })
        .sum::<f64>()
        + 0.4;
    let mut seq = 0u8;
    SceneGen::new(
        rate,
        cycle_s,
        SignalInfo::new(bw_hz),
        Box::new(move |cycle, t0, bursts| {
            let mut t = t0 + 0.1;
            for (packet, two) in scene_packets(cycle, split, tampered) {
                seq = (seq + 1) & 0x0F;
                for f in rns::radio_frames(&packet, seq, two, 100) {
                    bursts.extend(lora_burst(
                        &p,
                        &f,
                        t,
                        -TUNE_OFFSET_HZ + 700.0,
                        AMP,
                        0.0,
                        rate,
                    ));
                    t += p.airtime(f.len()) + gap;
                }
            }
        }),
    )
}

/// The generator of `reticulum` from a configuration whose settings follow [`RnsSetting`]: the scene at the bandwidth and the
/// spreading factor of the settings `bw` and `sf`.
pub fn from_config(cfg: &GenConfig) -> SceneGen {
    let s = &cfg.settings;
    generator(
        cfg.rate_hz,
        s.get(&BW).hz(),
        u8::try_from(s.get(&SF)).unwrap_or(8),
        s.get(&GEN_SPLIT),
        s.get(&GEN_TAMPERED),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_search_has_fifteen_detectors_and_leaves_out_a_bandwidth_that_does_not_fit() {
        let p = RnsPlanner::default();
        let at = |rate| Input {
            rate,
            centre_hz: 868.3e6,
            tuned_hz: Some(868.0e6),
        };
        let n = |b: &BankPlan| b.channels.iter().map(|c| c.detectors.len()).sum::<usize>();
        let (bank, item) = p.plan(&at(1e6));
        assert_eq!(n(&bank), 15);
        assert_eq!(item.line, "SF7 - SF11, 125 kHz, 250 kHz, 62.5 kHz");
        assert_eq!(n(&p.plan(&at(400e3)).0), 10);
        let fixed = RnsPlanner {
            auto_plan: false,
            bw: FixedBw::Bw500,
            sf: 12,
        };
        let (bank, item) = fixed.plan(&at(2e6));
        assert_eq!(n(&bank), 1);
        assert_eq!(item.line, "SF12, 500 kHz");
    }
}
