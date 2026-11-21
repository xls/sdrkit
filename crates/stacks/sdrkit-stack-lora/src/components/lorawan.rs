//! The components of `lorawanwide`: `lorawan.fsk` (the FSK channel) and `lorawan.mac` (parsing, the frame log, the device and
//! channel tables, the counters and the results).

use super::{count, show_status};
use crate::commands::RESET;
use crate::components::channels::WidePlan;
use crate::mesh::decode::update_device;
use crate::mesh::store::MeshStore;
use crate::mesh::types::Protocol;
use crate::proto::lorawan::{self, LorawanFrame};
use crate::results::Modulation;
use crate::settings::CLOCK0;
use crate::typed::{
    LoraPacketEvent, WideChannelRow, WideFields, WideFrameRow, LORA_PACKET, MESH_DEVICES,
    MESH_TYPES, WIDE_CHANNELS, WIDE_FRAMES,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{
    ConfigError, Event, LockState, Record, Results, ResultsDecl, SettingError, SettingId,
    SettingSet, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Port,
    RateFrom, Timed,
};
use sdrkit_proto_lora_phy::bank::DetectorState;
use sdrkit_proto_lora_phy::components::BankState;
use sdrkit_proto_lora_phy::fsk::{FskFrame, FskRx, FskStats};
use sdrkit_proto_lora_phy::rx::LoraFrame;
use sdrkit_proto_lora_phy::LoraError;
use sdrkit_util_objects::commands::CLEAR;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Tables};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// A frame of the LoRaWAN FSK channel as a graph item (`lorawan.fsk_frame`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FskItem(pub FskFrame);

impl Item for FskItem {
    const KIND: &'static str = "lorawan.fsk_frame";
}

/// The FSK channel's state, at every tick (`lorawan.fsk_state`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FskState {
    /// Counters.
    pub stats: FskStats,
    /// Running (the channel fits the capture).
    pub running: bool,
}

impl Item for FskState {
    const KIND: &'static str = "lorawan.fsk_state";
}

/// `lorawan.fsk`: the FSK channel of the plan, when it fits the capture.
#[derive(Default)]
pub struct FskComp {
    freq: Option<f64>,
    rate: f64,
    centre: f64,
    rx: Option<FskRx>,
    rebuild: bool,
    out: Vec<FskFrame>,
    base: u64,
    error: Option<LoraError>,
}

impl FskComp {
    /// Why the last build of the receiver failed. `None` after a good build.
    pub fn error(&self) -> Option<LoraError> {
        self.error
    }
}

impl Component for FskComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "lorawan.fsk",
            vec![Port::iq("in"), Port::typed::<WidePlan>("plan")],
            vec![
                Port::typed::<FskItem>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<FskState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if let Some(f) = inputs.first() {
            self.rate = f.rate;
            self.centre = f.centre_hz;
        }
        self.rebuild = true;
        Ok(vec![
            Format::new(Kind::typed::<FskItem>(), 0.0),
            Format::new(Kind::typed::<FskState>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        for t in io.input(0).tags() {
            if let sdrkit_graph::TagKind::Retuned { centre_hz, .. } = t.kind {
                self.centre = centre_hz;
                self.rebuild = true;
            }
        }
        if let Some(p) = io.items::<WidePlan>(1).last() {
            if p.value.fsk_hz != self.freq {
                self.freq = p.value.fsk_hz;
                self.rebuild = true;
            }
        }
        if self.rebuild {
            self.rebuild = false;
            self.base = io.input(0).first();
            self.rx = None;
            self.error = None;
            if let Some(f) = self.freq.filter(|_| self.rate > 0.0) {
                match FskRx::new(self.rate, f - self.centre) {
                    Ok(rx) => self.rx = Some(rx),
                    Err(e) => self.error = Some(e),
                }
            }
        }
        if let Some(rx) = &mut self.rx {
            self.out.clear();
            rx.process(io.iq(0), &mut self.out);
            for mut f in self.out.drain(..) {
                f.start += self.base;
                let t = f.start as f64 / self.rate;
                io.items_out::<FskItem>(0).push(Timed {
                    time: t,
                    value: FskItem(f),
                });
            }
        }
        if io.tick() {
            let st = FskState {
                stats: self.rx.as_ref().map(|r| r.stats).unwrap_or_default(),
                running: self.rx.is_some(),
            };
            let time = io.input(0).first() as f64 / self.rate.max(1.0);
            io.items_out::<FskState>(1).push(Timed { time, value: st });
        }
    }
}

/// One row of the frame log.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WideRow {
    /// Computer clock (0 when times follow the signal).
    pub wall: f64,
    /// Signal time.
    pub time: f64,
    /// Channel centre, Hz (absolute when the radio's centre is known).
    pub channel_hz: f64,
    /// Bandwidth, Hz.
    pub bw: f64,
    /// Channel number in the plan (-1 unknown).
    pub channel: i32,
    /// SF (0 for FSK).
    pub sf: u8,
    /// FSK.
    pub fsk: bool,
    /// Inverted IQ.
    pub inverted: bool,
    /// CRC present.
    pub crc_present: bool,
    /// CRC right.
    pub crc_ok: bool,
    /// SNR, dB.
    pub snr_db: f64,
    /// Level, dBFS.
    pub level_db: f64,
    /// Carrier offset, Hz.
    pub cfo_hz: f64,
    /// Length.
    pub len: usize,
    /// Corrected codewords.
    pub corrected: u32,
    /// The parsed frame.
    pub mac: Option<LorawanFrame>,
    /// Summary.
    pub summary: String,
    /// The DevAddr or the DevEUI as the log shows it.
    #[serde(default)]
    pub device: String,
    /// The MAC commands as the log shows them.
    #[serde(default)]
    pub mac_commands: String,
    /// The message integrity code as the log shows it.
    #[serde(default)]
    pub mic: String,
}

impl Shift for WideRow {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for WideRow {
    type Row = WideFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let mac = self.mac.as_ref();
        out.put(&WideFrameRow {
            time_s: self.time,
            r#type: mac.map(|x| lorawan::mtype_name(x.mtype)),
            device: text(&self.device),
            network: mac.map(|x| x.network.as_str()),
            fcnt: mac.and_then(|x| x.fcnt).map(i64::from),
            fport: mac.and_then(|x| x.fport).map(i64::from),
            payload_bytes: mac.map(|x| i64::try_from(x.frm_len).unwrap_or(i64::MAX)),
            fctrl: mac.map(|x| x.flags.as_str()),
            mac_commands: text(&self.mac_commands),
            mic: text(&self.mic),
            modulation: if self.fsk {
                Modulation::Fsk
            } else {
                Modulation::Lora
            },
            sf: (!self.fsk).then_some(i64::from(self.sf)),
            snr_db: self.snr_db,
            freq: Hertz(self.channel_hz + self.cfo_hz),
            summary: &self.summary,
        });
    }
}

/// A text cell: `None` when the text is empty.
fn text(t: &str) -> Option<&str> {
    (!t.is_empty()).then_some(t)
}

/// Devices kept by `lorawanwide`.
pub const WIDE_DEVICES_CAP: usize = 300;
/// Frames kept by `lorawanwide`.
pub const WIDE_LOG_CAP: usize = 600;

/// What `lorawanwide` heard: the frame log, the devices and the counters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WideHeard {
    /// The frame log, oldest first.
    pub log: LogRing<WideRow>,
    /// The devices (and the type counts).
    pub store: MeshStore,
    /// Good frames.
    pub good: u64,
    /// Bad frames.
    pub bad: u64,
}

impl Default for WideHeard {
    fn default() -> Self {
        let mut store = MeshStore::default();
        store.devices.set_cap(WIDE_DEVICES_CAP);
        WideHeard {
            log: LogRing::new(WIDE_LOG_CAP),
            store,
            good: 0,
            bad: 0,
        }
    }
}

impl Shift for WideHeard {
    fn shift_times(&mut self, by_s: f64) {
        self.log.shift_times(by_s);
        self.store.shift_times(by_s);
    }
}

impl Tables for WideHeard {
    const FORMAT: StateFormat = StateFormat::new("lora.lorawan_heard", 1);
    fn clear(&mut self) {
        self.log.clear();
        self.store.clear();
        self.good = 0;
        self.bad = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.log.adopt(saved.log);
        self.store.adopt(saved.store);
        self.good = saved.good;
        self.bad = saved.bad;
    }
}

/// The `lorawan.mac` component.
pub struct LorawanMac {
    /// What was heard.
    pub heard: WideHeard,
    plan: WidePlan,
    preambles: u64,
    header_failed: u64,
    fsk: FskState,
    last_good: f64,
    now: f64,
    level: Option<f64>,
    clock0: f64,
    demods: usize,
    det: Vec<DetectorState>,
    p: Published,
    events: Vec<Event>,
    text: String,
    key: String,
}

impl Default for LorawanMac {
    fn default() -> Self {
        LorawanMac {
            heard: WideHeard::default(),
            plan: WidePlan::default(),
            preambles: 0,
            header_failed: 0,
            fsk: FskState::default(),
            last_good: -1e9,
            now: 0.0,
            level: None,
            clock0: 0.0,
            demods: 0,
            det: Vec::new(),
            p: Published::from_decl(std::sync::Arc::new(
                ResultsDecl::new()
                    .with_fields(WideFields::SHAPE.fields)
                    .with_table(WIDE_FRAMES.def())
                    .with_table(WIDE_CHANNELS.def())
                    .with_table(MESH_DEVICES.def())
                    .with_table(MESH_TYPES.def()),
            )),
            events: Vec::new(),
            text: String::new(),
            key: String::new(),
        }
    }
}

/// One frame for the log.
struct Heard<'a> {
    time: f64,
    chan: f64,
    bw: f64,
    sf: u8,
    fsk: bool,
    inverted: bool,
    crc_present: bool,
    crc_ok: bool,
    snr: f64,
    level: f64,
    cfo: f64,
    corrected: u32,
    payload: &'a [u8],
}

impl LorawanMac {
    /// Good frames.
    pub fn good(&self) -> u64 {
        self.heard.good
    }

    fn row(&mut self, f: Heard, events: &mut Vec<Event>) {
        let parsed = lorawan::parse(f.payload);
        if !f.crc_present && parsed.is_err() {
            // A frame without CRC that is not LoRaWAN is noise.
            return;
        }
        let summary;
        let mut mac = None;
        let h = &mut self.heard;
        if !f.crc_ok {
            summary = "CRC error".into();
            h.bad += 1;
        } else {
            match parsed {
                Ok(lw) => {
                    h.good += 1;
                    self.last_good = f.time;
                    h.store.count(
                        crate::mesh::types::Protocol::Lorawan,
                        lorawan::mtype_name(lw.mtype),
                    );
                    summary = lw.summary.clone();
                    let (id, new) = update_device(
                        &mut h.store,
                        &lw,
                        f.time,
                        f.chan + f.cfo,
                        f.snr,
                        f.level,
                        f.sf,
                        f.fsk,
                    );
                    if new {
                        if let Some(d) = h.store.devices.get(&id) {
                            let line =
                                format!("LoRaWAN: new device {id} ({}) ({})", d.network, d.vendor);
                            self.p.results_mut().log(line);
                        }
                    }
                    let devaddr = lw.devaddr.map(|a| format!("{a:08X}"));
                    events.push(LORA_PACKET.event(
                        f.time,
                        f.chan + f.cfo,
                        &LoraPacketEvent {
                            net: Protocol::Lorawan,
                            modulation: if f.fsk {
                                Modulation::Fsk
                            } else {
                                Modulation::Lora
                            },
                            snr_db: f.snr,
                            sf: (!f.fsk).then_some(i64::from(f.sf)),
                            payload: f.payload.into(),
                            crc_ok: true,
                            source: None,
                            target: None,
                            devaddr: devaddr.as_deref(),
                            iq_inverted: Some(f.inverted),
                            crc: None,
                            bw: None,
                            cr: None,
                            implicit: None,
                            sync: None,
                            level_dbfs: None,
                            family: keys::Family::Sensor,
                        },
                    ));
                    mac = Some(lw);
                }
                Err(e) => {
                    summary = format!("not LoRaWAN: {e}");
                    h.bad += 1;
                }
            }
        }
        let device = match mac.as_ref().map(|x| (x.devaddr, x.dev_eui)) {
            Some((Some(a), _)) => format!("{a:08X}"),
            Some((None, Some(e))) => lorawan::eui_text(e),
            _ => String::new(),
        };
        let mac_commands = mac
            .as_ref()
            .map(|x| x.mac_commands.join(", "))
            .unwrap_or_default();
        let mic = mac
            .as_ref()
            .and_then(|x| x.mic)
            .map(|x| format!("{x:08X}"))
            .unwrap_or_default();
        let channel = self
            .plan
            .channels
            .iter()
            .position(|c| (c.0 - f.chan).abs() < 1.0 && c.1 == f.bw)
            .map_or(-1, |i| i as i32);
        self.heard.log.push(WideRow {
            wall: if self.clock0 > 0.0 {
                self.clock0 + f.time
            } else {
                0.0
            },
            time: f.time,
            channel_hz: f.chan,
            bw: f.bw,
            channel,
            sf: f.sf,
            fsk: f.fsk,
            inverted: f.inverted,
            crc_present: f.crc_present,
            crc_ok: f.crc_ok,
            snr_db: f.snr,
            level_db: f.level,
            cfo_hz: f.cfo,
            len: f.payload.len(),
            corrected: f.corrected,
            mac,
            summary,
            device,
            mac_commands,
            mic,
        });
    }

    fn publish(&mut self) {
        let h = &self.heard;
        let state = self.now - self.last_good < 5.0;
        let chans = self.plan.bank.channels.len();
        let t = &mut self.text;
        t.clear();
        if h.good == 0 {
            let _ = write!(
                t,
                "listening on {chans} channels ({} demodulators)",
                self.demods
            );
        } else {
            let _ = write!(
                t,
                "{} frames, {} devices, {chans} channels",
                h.good,
                h.store.devices.len()
            );
        }
        let p = &mut self.p;
        show_status(
            p,
            if state {
                LockState::Locked
            } else {
                LockState::Searching
            },
            None,
            None,
            self.level,
            t,
        );
        p.put(&WideFields {
            level_dbfs: self.level,
            frames_ok: count(h.good),
            frames_bad: count(h.bad),
            preambles: count(self.preambles),
            header_failed: count(self.header_failed),
            fsk_syncs: count(self.fsk.stats.syncs),
            fsk_frames: count(self.fsk.stats.frames),
            fsk_crc_errors: count(self.fsk.stats.crc_errors),
            demodulators: count(self.demods),
            channels_listed: count(self.plan.channels.len()),
            channels_decoded: count(chans),
            note: &self.plan.note,
            plan: &self.plan.plan_name,
            devices: count(h.store.devices.len()),
        });
        shaped::log_rows(&h.log, &WIDE_FRAMES, self.now, p);
        let key = &mut self.key;
        for (i, c) in self.plan.bank.channels.iter().enumerate() {
            let sum = |f: &dyn Fn(&DetectorState) -> u64| {
                count(
                    self.det
                        .iter()
                        .filter(|d| c.detectors.iter().any(|x| x.tag == d.tag))
                        .map(f)
                        .sum::<u64>(),
                )
            };
            let in_band = self
                .det
                .iter()
                .any(|d| d.in_band && c.detectors.iter().any(|x| x.tag == d.tag));
            let lo = c.detectors.iter().map(|d| d.sf).min().unwrap_or(0);
            let hi = c.detectors.iter().map(|d| d.sf).max().unwrap_or(0);
            key.clear();
            let _ = write!(key, "{i:03}");
            WIDE_CHANNELS.put(
                p,
                key,
                &WideChannelRow {
                    freq: Hertz(c.freq_hz),
                    bw: Hertz(c.bw),
                    sf_min: i64::from(lo),
                    sf_max: i64::from(hi),
                    in_band,
                    preambles: sum(&|d| d.stats.preambles),
                    frames: sum(&|d| d.stats.frames),
                    crc_errors: sum(&|d| d.stats.crc_failed),
                },
            );
        }
        h.store.publish_devices(p, self.now);
    }
}

sdrkit_core::settings! {
    /// The settings of `lorawan.mac`.
    pub enum LorawanMacSetting {
        /// Computer clock at signal 0.
        Clock0 = CLOCK0,
    }
}

sdrkit_core::commands! {
    /// The commands of `lorawan.mac`.
    pub enum LorawanMacCommand {
        /// Clear.
        Clear = CLEAR,
        /// Reset.
        Reset = RESET,
    }
}

impl Component for LorawanMac {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "lorawan.mac",
            vec![
                Port::typed::<LoraFrame>("frames"),
                Port::typed::<FskItem>("fsk"),
                Port::typed::<BankState>("state"),
                Port::typed::<FskState>("fsk_state"),
                Port::typed::<WidePlan>("plan"),
            ],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(LorawanMacSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }

    fn process(&mut self, io: &mut Io) {
        if let Some(pl) = io.items::<WidePlan>(4).last() {
            let pl = pl.value.clone();
            if pl.note != self.plan.note {
                self.p.results_mut().log(format!("LoRaWAN: {}", pl.note));
            }
            if pl.bank != self.plan.bank {
                self.p.clear_rows(&WIDE_CHANNELS);
            }
            self.plan = pl;
        }
        for s in io.items::<BankState>(2) {
            self.now = s.value.time;
            self.level = Some(s.value.level_db);
            self.demods = s.value.demodulators;
            self.preambles = s.value.detectors.iter().map(|d| d.stats.preambles).sum();
            self.header_failed = s
                .value
                .detectors
                .iter()
                .map(|d| d.stats.header_failed)
                .sum();
            self.det.clone_from(&s.value.detectors);
        }
        for s in io.items::<FskState>(3) {
            self.fsk = s.value;
        }
        let mut events = std::mem::take(&mut self.events);
        for f in io.items::<LoraFrame>(0) {
            let fr = &f.value;
            self.row(
                Heard {
                    time: f.time,
                    chan: fr.channel_hz,
                    bw: fr.bw,
                    sf: fr.sf,
                    fsk: false,
                    inverted: fr.inverted,
                    crc_present: fr.crc_present,
                    crc_ok: fr.good(),
                    snr: fr.snr_db,
                    level: fr.level_db,
                    cfo: fr.cfo_hz,
                    corrected: fr.corrected,
                    payload: &fr.payload,
                },
                &mut events,
            );
        }
        let fsk_hz = self.plan.fsk_hz.unwrap_or(0.0);
        for f in io.items::<FskItem>(1) {
            let fr = &f.value.0;
            self.row(
                Heard {
                    time: f.time,
                    chan: fsk_hz,
                    bw: 100e3,
                    sf: 0,
                    fsk: true,
                    inverted: false,
                    crc_present: true,
                    crc_ok: true,
                    snr: fr.snr_db,
                    level: fr.level_db,
                    cfo: fr.offset_hz,
                    corrected: 0,
                    payload: &fr.payload,
                },
                &mut events,
            );
        }
        io.messages_out(0).append(&mut events);
        self.events = events;
        if io.tick() {
            self.publish();
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match LorawanMacSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            LorawanMacSetting::Clock0 => {
                self.clock0 = CLOCK0.read(id, v)?;
                self.heard.store.wall0 = self.clock0;
            }
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([(LorawanMacSetting::Clock0, Value::F64(self.clock0))])
    }

    fn commands(&self) -> &Commands {
        LorawanMacCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match LorawanMacCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            LorawanMacCommand::Clear => {
                self.heard.clear();
                for t in [WIDE_FRAMES.def(), MESH_DEVICES.def(), MESH_TYPES.def()] {
                    self.p.clear_rows(t);
                }
            }
            LorawanMacCommand::Reset => {
                self.heard.good = 0;
                self.heard.bad = 0;
            }
        }
        // What is shown changes at once, not at the next report.
        self.publish();
        Ok(None)
    }

    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }

    fn forget(&mut self, kept: &Kept) {
        if !kept.table(WIDE_FRAMES.key) {
            self.heard.log.clear();
            self.p.clear_rows(&WIDE_FRAMES);
        }
        if !kept.table(MESH_DEVICES.key) {
            self.heard.store.clear();
            self.p.clear_rows(&MESH_DEVICES);
            self.p.clear_rows(&MESH_TYPES);
        }
        if !kept.field("frames_ok") {
            self.heard.good = 0;
        }
        if !kept.field("frames_bad") {
            self.heard.bad = 0;
        }
        self.level = None;
        self.p.clear_rows(&WIDE_CHANNELS);
        self.publish();
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.heard.save_state(self.now).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let now = self.now;
        self.heard.restore_state(state, now)
    }
}
