//! The `ble` device table, the packet log and what the stack publishes (the `wlan.ble_devices` component). The table and
//! the log are kept in the saved state.

use super::adv::{kind_of, parse_ad, AdvInfo, DeviceKind};
use super::follow::{Follower, Role};
use super::{AdvChannel, AdvStats, Advert};
use crate::report::{hex, push_history, remove_older_rows, wall, BLE_PRIVACY_NOTE};
use crate::typed::{
    BleDeviceEvent, BleDeviceLevelRow, BleDeviceRow, BleFields, BlePacketRow, BlePduTypeRow,
    BLE_DEVICE, BLE_DEVICES, BLE_DEVICE_LEVELS, BLE_DEVICE_ROW, BLE_PACKETS, BLE_PDU_TYPES,
    BLE_REMOVE,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Clock, ConfigError, Event, HostRequest, LockState, Record, Results, Setting,
    SettingError, SettingId, SettingSet, StateBlob, StateError, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kept, Kind, Port, RunContext, Timed,
};
use sdrkit_proto_wlan_phy::ble::{channel_hz, Addr, AddrKind, Body, Coding, PduType, Phy};
use sdrkit_util_objects::commands::{self as objcmd, Tables as _};
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Most devices kept.
pub const DEVICE_CAP: usize = 400;
/// Most log entries kept.
pub const LOG_CAP: usize = 400;
/// The level history length of a device.
pub const HISTORY: usize = 24;
/// A device heard within this time keeps the lock state on, s.
pub const ACTIVE_S: f64 = 5.0;
/// New devices that get a line in the text log.
const NEW_DEVICE_LINES: usize = 20;

sdrkit_core::choice_enum! {
    /// The kind of a device address. Standard: Bluetooth Core Specification v5.4 Vol 6 Part B clause 1.3.
    #[derive(Default, Serialize, Deserialize)]
    pub enum AddressType {
        /// A public address.
        #[default]
        Public = ("public", "Public"),
        /// A random static address.
        RandomStatic = ("random_static", "Random static"),
        /// A resolvable private address.
        ResolvablePrivate = ("resolvable_private", "Resolvable private"),
        /// A non-resolvable private address.
        NonResolvablePrivate = ("non_resolvable_private", "Non-resolvable private"),
        /// A random address of the reserved kind.
        Random = ("random", "Random"),
    }
}

impl From<AddrKind> for AddressType {
    fn from(k: AddrKind) -> Self {
        match k {
            AddrKind::Public => AddressType::Public,
            AddrKind::RandomStatic => AddressType::RandomStatic,
            AddrKind::ResolvablePrivate => AddressType::ResolvablePrivate,
            AddrKind::NonResolvablePrivate => AddressType::NonResolvablePrivate,
            AddrKind::RandomReserved => AddressType::Random,
        }
    }
}

/// The advertising channel to listen on. The host tunes to it.
pub const CHANNEL_SETTING: Setting<field::Choice<AdvChannel>> =
    Setting::choice("channel", "Channel", AdvChannel::C37)
        .help("Asks the host to tune to the advertising channel.");
/// Hop through the advertising channels.
pub const HOP: Setting<field::Bool> = Setting::new("hop", "Hop", "", false).help(
    "Asks the host to step 37, 38, 39: some devices skip a channel, and interference differs between them.",
);
/// The time on each channel while hopping.
pub const HOP_SECONDS: Setting<field::Duration> = Setting::new("hop_seconds", "Dwell", "s", 3.0)
    .range(1.0, 30.0, 1.0)
    .help("Time on each channel while hopping.");
/// Show only the devices whose name, address, vendor or beacon holds this text.
pub const FILTER_TEXT: Setting<field::Text> = Setting::new("filter_text", "Filter", "", "")
    .help("Shows only the devices whose name, address, vendor or beacon holds this text.")
    .section("Display");
/// Show only the devices at or above this level.
pub const MIN_LEVEL: Setting<field::F64> =
    Setting::new("min_level_dbfs", "Level above", "dBFS", -120.0)
        .range(-120.0, 0.0, 1.0)
        .section("Display");
/// Show only the devices with a name.
pub const NAMED_ONLY: Setting<field::Bool> =
    Setting::new("named_only", "Named only", "", false).section("Display");

sdrkit_core::settings! {
    /// The settings of [`DeviceTable`].
    pub enum DeviceTableSetting {
        /// The advertising channel.
        Channel = CHANNEL_SETTING,
        /// Hop through the advertising channels.
        Hop = HOP,
        /// The dwell while hopping.
        HopSeconds = HOP_SECONDS,
        /// The filter text.
        FilterText = FILTER_TEXT,
        /// The least level shown.
        MinLevel = MIN_LEVEL,
        /// Only named devices.
        NamedOnly = NAMED_ONLY,
    }
}

sdrkit_core::commands! {
    /// The commands of [`DeviceTable`].
    pub enum DeviceTableCommand {
        /// Forget the devices, the log and the counters.
        Clear = objcmd::CLEAR,
        /// Forget one device.
        Remove = BLE_REMOVE.command,
    }
}

/// One device: an address and what its packets said.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// The address, most significant byte first.
    pub addr: String,
    /// A random address.
    pub random: bool,
    /// The kind of the address.
    pub kind: AddressType,
    /// The name.
    pub name: String,
    /// The name is the complete one.
    pub name_complete: bool,
    /// The vendor.
    pub vendor: String,
    /// The beacon text.
    pub beacon: String,
    /// The company identifier.
    pub company: Option<u16>,
    /// The first 32 bytes after the company identifier, hex.
    pub mfr_hex: String,
    /// Services joined with ", ".
    pub services: String,
    /// TX power, dBm.
    pub tx_power_dbm: Option<i8>,
    /// Flags.
    pub flags: Option<u8>,
    /// Connectable.
    pub connectable: bool,
    /// Scannable.
    pub scannable: bool,
    /// Extended advertising.
    pub extended: bool,
    /// Scanned by another device.
    pub scanned: bool,
    /// PDU types seen (bit per type code).
    pub pdu_mask: u16,
    /// Channels seen (bit 0 = 37, 1 = 38, 2 = 39).
    pub chan_mask: u8,
    /// Level of the last packet, dBFS.
    pub level_dbfs: f64,
    /// The last levels, oldest first.
    pub history: Vec<f32>,
    /// The levels heard in all.
    pub levels: u64,
    /// The text of the beacon column.
    pub beacon_text: String,
    /// The text of the channels column.
    pub channels_text: String,
    /// PHYs heard (bit 0 LE 1M, 1 LE 2M, 2 LE Coded).
    pub phys: u8,
    /// The text of the PHYs column.
    pub phys_text: String,
    /// An AuxPtr pointed to a channel outside the capture.
    pub aux_outside: bool,
    /// The advertising set id of the last extended packet.
    pub set_id: Option<u8>,
    /// The periodic advertising interval, ms.
    pub periodic_interval_ms: Option<f64>,
    /// The kind of device.
    pub device_kind: DeviceKind,
    /// The advertising data matched a rule of the tracker detector ([`crate::tracker::book::recognise`]).
    pub tracker: bool,
}

impl Device {
    /// The channels seen, as "37 38 39".
    pub fn channels(&self) -> String {
        let mut s = String::new();
        for i in 0..3u8 {
            if self.chan_mask & (1 << i) != 0 {
                if !s.is_empty() {
                    s.push(' ');
                }
                let _ = write!(s, "{}", 37 + i);
            }
        }
        s
    }

    /// The PHYs heard, as "LE 1M, LE Coded".
    pub fn phys_column(&self) -> String {
        [Phy::Le1M, Phy::Le2M, Phy::LeCoded]
            .iter()
            .enumerate()
            .filter(|(i, _)| self.phys & (1 << i) != 0)
            .map(|(_, p)| p.name())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The beacon, then the services, then the TX power, joined with "; ".
    pub fn beacon_column(&self) -> String {
        let mut parts = Vec::new();
        if !self.beacon.is_empty() {
            parts.push(self.beacon.clone());
        }
        if !self.services.is_empty() {
            parts.push(self.services.clone());
        }
        if let Some(t) = self.tx_power_dbm {
            parts.push(format!("tx {t} dBm"));
        }
        parts.join("; ")
    }

    fn matches(&self, filter: &str) -> bool {
        filter.is_empty()
            || [&self.name, &self.addr, &self.vendor, &self.beacon]
                .iter()
                .any(|s| s.to_lowercase().contains(filter))
    }
}

impl Shift for Device {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// The PHY of a packet as the log shows it: "LE 1M", "LE 2M", "LE Coded S=2" or "LE Coded S=8".
pub fn phy_text(phy: Phy, coding: Option<Coding>) -> String {
    match coding {
        Some(c) if phy == Phy::LeCoded => format!("{} {}", phy.name(), c.name()),
        _ => phy.name().to_owned(),
    }
}

/// `s`, or no value for an empty text.
fn known(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

impl Shaped for Device {
    type Row = BleDeviceRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&BleDeviceRow {
            address: &self.addr,
            address_type: self.kind,
            name: known(&self.name),
            level_dbfs: self.level_dbfs,
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
            vendor: known(&self.vendor),
            beacon: known(&self.beacon_text),
            channels: &self.channels_text,
            connectable: self.connectable,
            scanned: self.scanned,
            manufacturer_data: known(&self.mfr_hex),
            first_heard: columns::first_heard(seen).map(Time),
            phys: &self.phys_text,
            extended: self.extended,
            data_outside: self.aux_outside,
            set_id: self.set_id.map(i64::from),
            periodic_interval_ms: self.periodic_interval_ms,
            device_kind: self.device_kind,
            tracker: self.tracker,
        });
    }
}

/// One packet log entry.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    /// Signal time.
    pub t: f64,
    /// Channel from the whitening.
    pub ch: u8,
    /// PDU type name.
    pub pdu: String,
    /// Address ("" when none).
    pub addr: String,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// Payload length.
    pub len: u8,
    /// Summary.
    pub info: String,
    /// The PHY text.
    pub phy: String,
}

impl Shift for LogEntry {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

impl Shaped for LogEntry {
    type Row = BlePacketRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&BlePacketRow {
            signal_s: self.t,
            channel: i64::from(self.ch),
            pdu: &self.pdu,
            address: known(&self.addr),
            level_dbfs: self.level_dbfs,
            length: i64::from(self.len),
            details: &self.info,
            phy: &self.phy,
        });
    }
}

/// The devices and the log, kept across sessions.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The devices, by address and "r" or "p".
    pub devices: ObjectTable<String, Device>,
    /// The packet log.
    pub log: LogRing<LogEntry>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            devices: ObjectTable::new(DEVICE_CAP),
            log: LogRing::new(LOG_CAP),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.devices.shift_times(by_s);
        self.log.shift_times(by_s);
    }
}

impl objcmd::Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("wlan.ble_devices", 4);
    fn clear(&mut self) {
        self.devices.clear();
        self.log.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.devices.adopt(saved.devices);
        self.log.adopt(saved.log);
    }
}

/// The key of a device row: the address text and "r" (random) or "p" (public).
pub fn device_key(a: &Addr) -> String {
    format!("{}{}", a.text(), if a.random { "r" } else { "p" })
}

/// The counters of the packets taken since the last clear.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Counters {
    /// Good packets.
    pub good: u64,
    /// Good packets by PDU name.
    pub per_type: Vec<(&'static str, u64)>,
    /// Good packets on 37, 38 and 39.
    pub per_channel: [u64; 3],
    /// The running mean of the eye SNR.
    pub snr_db: Option<f64>,
    /// The last packet's carrier offset, Hz.
    pub offset_hz: f64,
    /// The last packet's deviation, Hz.
    pub deviation_hz: f64,
    /// The last good packet's channel.
    pub last_channel: u8,
    /// The last good packet's signal time.
    pub last_good_s: Option<f64>,
    /// Good packets on channels 0 to 36.
    pub secondary: u64,
    /// Good packets on LE 2M.
    pub two_m: u64,
    /// Good packets on LE Coded.
    pub coded: u64,
    /// Auxiliary packets reached through their pointer.
    pub aux_followed: u64,
    /// Pointers to a channel outside the capture.
    pub aux_outside: u64,
}

/// The device table and packet log as a component.
pub struct DeviceTable {
    published: Published,
    /// The devices and the log.
    pub store: Store,
    /// The counters.
    pub counters: Counters,
    follow: Follower,
    stats: AdvStats,
    base: AdvStats,
    level_dbfs: Option<f64>,
    new_logged: usize,
    lines: Vec<String>,
    events: Vec<Event>,
    requests: Vec<HostRequest>,
    status: Status,
    key: String,
    channel: AdvChannel,
    hop: bool,
    hop_seconds: f64,
    filter: String,
    filter_lower: String,
    min_level_dbfs: f64,
    named_only: bool,
    now_s: f64,
    live: bool,
    clock: Clock,
    configured: bool,
}

impl Default for DeviceTable {
    fn default() -> Self {
        DeviceTable {
            published: Published::new(BleFields::SHAPE.fields).with_tables(&[
                BLE_DEVICES.def(),
                BLE_PACKETS.def(),
                BLE_PDU_TYPES.def(),
                BLE_DEVICE_LEVELS.def(),
            ]),
            store: Store::default(),
            counters: Counters::default(),
            follow: Follower::default(),
            stats: AdvStats::default(),
            base: AdvStats::default(),
            level_dbfs: None,
            new_logged: 0,
            lines: Vec::new(),
            events: Vec::new(),
            requests: Vec::new(),
            status: Status::default(),
            key: String::new(),
            channel: CHANNEL_SETTING.default,
            hop: HOP.default,
            hop_seconds: HOP_SECONDS.default,
            filter: String::new(),
            filter_lower: String::new(),
            min_level_dbfs: MIN_LEVEL.default,
            named_only: NAMED_ONLY.default,
            now_s: 0.0,
            live: false,
            clock: Clock::fixed(0.0),
            configured: false,
        }
    }
}

fn update_from_adv(d: &mut Device, info: &AdvInfo, t: PduType, adv_mode: u8) {
    let k = kind_of(info);
    if k != DeviceKind::Unknown {
        d.device_kind = k;
    }
    if let Some(n) = &info.name {
        if info.name_complete || d.name.is_empty() || d.name.len() <= n.len() {
            d.name.clone_from(n);
            d.name_complete = info.name_complete;
        }
    }
    if let Some(v) = &info.vendor {
        d.vendor.clone_from(v);
    }
    if let Some(b) = &info.beacon {
        d.beacon.clone_from(b);
    }
    if let Some(c) = info.company {
        d.company = Some(c);
        d.mfr_hex = hex(&info.mfr[..info.mfr.len().min(32)]);
    }
    if !info.services.is_empty() {
        d.services = info.services.join(", ");
    }
    if info.tx_power.is_some() {
        d.tx_power_dbm = info.tx_power;
    }
    if info.flags.is_some() {
        d.flags = info.flags;
    }
    let ext = t == PduType::AdvExtInd;
    if matches!(t, PduType::AdvInd | PduType::AdvDirectInd) || (ext && adv_mode & 1 != 0) {
        d.connectable = true;
    }
    if matches!(t, PduType::AdvInd | PduType::AdvScanInd) || (ext && adv_mode & 2 != 0) {
        d.scannable = true;
    }
    if ext {
        d.extended = true;
    }
    d.beacon_text = d.beacon_column();
}

impl DeviceTable {
    fn row(&mut self, a: &Addr, at: Stamp) -> Option<&mut Device> {
        let key = device_key(a);
        let up = self
            .store
            .devices
            .upsert(&key, at, || Device {
                addr: a.text(),
                random: a.random,
                kind: a.kind().into(),
                ..Default::default()
            })
            .ok()?;
        if up.new && self.new_logged < NEW_DEVICE_LINES {
            self.new_logged += 1;
            let kind = AddressType::from(a.kind());
            self.lines.push(format!(
                "BLE: new device {} ({})",
                a.text(),
                AddressType::CHOICES[usize::from(kind.index())].label
            ));
        }
        Some(up.row)
    }

    /// Takes a good packet: the counters, the log and the device rows. Returns the event of the device it updated.
    pub fn take(&mut self, a: &Advert) -> Option<Event> {
        let p = a.parsed()?;
        let t = p.header.pdu_type;
        let at = match wall(self.live, &self.clock) {
            Some(w) => Stamp::at(a.time_s).with_wall(w),
            None => Stamp::at(a.time_s),
        };
        let f = self.follow.take(a, self.stats.capture);
        let role = f.as_ref().map_or(Role::Primary, |f| f.role);
        let name = role.pdu_name(t);
        let c = &mut self.counters;
        c.good += 1;
        match c.per_type.iter_mut().find(|(n, _)| *n == name) {
            Some(e) => e.1 += 1,
            None => c.per_type.push((name, 1)),
        }
        if (37..=39).contains(&a.channel) {
            c.per_channel[(a.channel - 37) as usize] += 1;
        } else {
            c.secondary += 1;
        }
        match a.phy {
            Phy::Le2M => c.two_m += 1,
            Phy::LeCoded => c.coded += 1,
            Phy::Le1M => {}
        }
        if matches!(role, Role::AuxAdv | Role::AuxChain | Role::AuxScanRsp) {
            c.aux_followed += 1;
        }
        if f.as_ref().is_some_and(|f| f.outside.is_some()) {
            c.aux_outside += 1;
        }
        c.snr_db = Some(match c.snr_db {
            None => a.snr_db,
            Some(s) => s + 0.2 * (a.snr_db - s),
        });
        c.offset_hz = a.offset_hz;
        c.deviation_hz = a.deviation_hz;
        c.last_channel = a.channel;
        c.last_good_s = Some(a.time_s);
        let owner = f.as_ref().and_then(|f| f.owner);
        let sender = super::adv::sender(&p).or(owner);
        let mut info = a.summary.clone();
        if let Some(f) = &f {
            if let Some(ch) = f.outside {
                let _ = write!(info, "; channel {ch} is outside the capture");
            }
            if f.incomplete {
                info.push_str("; data incomplete");
            }
            if f.unfollowed {
                info.push_str("; not followed");
            }
        }
        self.store.log.push(LogEntry {
            t: a.time_s,
            ch: a.channel,
            pdu: name.into(),
            addr: sender.map(|x| x.text()).unwrap_or_default(),
            level_dbfs: a.level_dbfs,
            len: p.header.length,
            info,
            phy: phy_text(a.phy, a.coding),
        });
        if let Body::ScanReq { adv_a, .. } = p.body {
            // The advertiser that the request addresses is marked, but this packet is not its own: a known row is not
            // heard again.
            let key = device_key(&adv_a);
            match self.store.devices.get_mut(key.as_str()) {
                Some(d) => d.scanned = true,
                None => {
                    if let Some(d) = self.row(&adv_a, at) {
                        d.scanned = true;
                    }
                }
            }
        }
        let s = sender?;
        let adv_mode = match p.body {
            Body::Extended { ext, .. } => ext.adv_mode,
            _ => 0,
        };
        let d = self.row(&s, at)?;
        d.level_dbfs = a.level_dbfs;
        push_history(&mut d.history, a.level_dbfs as f32, HISTORY);
        d.levels += 1;
        d.pdu_mask |= 1 << (t.code() & 15);
        if (37..=39).contains(&a.channel) {
            d.chan_mask |= 1 << (a.channel - 37);
            d.channels_text = d.channels();
        }
        d.phys |= match a.phy {
            Phy::Le1M => 1,
            Phy::Le2M => 2,
            Phy::LeCoded => 4,
        };
        d.phys_text = d.phys_column();
        match &f {
            Some(f) if f.role != Role::Primary => {
                d.extended = true;
                if let Some(data) = &f.complete {
                    update_from_adv(d, &parse_ad(data), t, adv_mode);
                    d.tracker |= crate::tracker::book::recognise(data).is_some();
                }
            }
            _ if !matches!(t, PduType::ScanReq | PduType::ConnectInd) => {
                update_from_adv(d, &a.info, t, adv_mode);
                d.tracker |= p
                    .data()
                    .is_some_and(|data| crate::tracker::book::recognise(data).is_some());
            }
            _ => {}
        }
        if let Body::Extended { ext, .. } = p.body {
            if ext.tx_power.is_some_and(|t| t != 127) {
                d.tx_power_dbm = ext.tx_power;
                d.beacon_text = d.beacon_column();
            }
        }
        if let Some(f) = &f {
            if f.outside.is_some() {
                d.aux_outside = true;
            }
            if f.set_id.is_some() {
                d.set_id = f.set_id;
            }
            if let Some(si) = f.sync {
                d.periodic_interval_ms = Some(si.interval_s() * 1e3);
            }
        }
        let freq = channel_hz(a.channel).unwrap_or(0.0);
        Some(BLE_DEVICE.event(
            a.time_s,
            freq,
            &BleDeviceEvent {
                source: &d.addr,
                text: known(&d.name),
                level_dbfs: a.level_dbfs,
                family: Family::Other,
            },
        ))
    }

    fn counters_since_clear(&self) -> (u64, u64) {
        (
            self.stats.aa_hits.saturating_sub(self.base.aa_hits),
            self.stats
                .crc_failures
                .saturating_sub(self.base.crc_failures),
        )
    }

    fn clear(&mut self) {
        self.store.clear();
        self.follow.clear();
        self.counters = Counters::default();
        self.base = self.stats;
        self.published.clear_rows(BLE_PDU_TYPES.def());
        self.published.clear_rows(BLE_DEVICE_LEVELS.def());
    }

    /// Publishes the results now.
    pub fn publish(&mut self) {
        let now = self.now_s;
        // The PDU counts are lent out for the time of the publish, so nothing is copied.
        let per_type = std::mem::take(&mut self.counters.per_type);
        let c = Counters {
            per_type: Vec::new(),
            ..self.counters
        };
        let (hits, bad) = self.counters_since_clear();
        let active = c.last_good_s.is_some_and(|l| now - l <= ACTIVE_S);
        let channel = self.stats.channel;
        let st = &mut self.status;
        st.lock_state = if active {
            LockState::Locked
        } else {
            LockState::Searching
        };
        st.snr_db = c.snr_db.map(|s| s as f32);
        st.level_dbfs = self.level_dbfs.map(|l| l as f32);
        st.offset_hz = c.last_good_s.map(|_| c.offset_hz);
        st.summary.clear();
        // Writing to a String cannot fail.
        let _ = if c.good > 0 {
            write!(
                st.summary,
                "{} devices, {} packets",
                self.store.devices.len(),
                c.good
            )
        } else if channel != 0 {
            write!(st.summary, "listening on channel {channel}")
        } else {
            write!(st.summary, "listening for advertising")
        };
        let p = &mut self.published;
        {
            let r = p.results_mut();
            r.set_status(st);
            for line in self.lines.drain(..) {
                r.log(line);
            }
        }
        p.put(&BleFields {
            devices: self.store.devices.len() as i64,
            frames_ok: c.good as i64,
            frames_bad: bad as i64,
            aa_hits: hits as i64,
            snr_db: c.snr_db,
            offset_hz: c.last_good_s.map(|_| Hertz(c.offset_hz)),
            deviation_hz: c.last_good_s.map(|_| Hertz(c.deviation_hz)),
            level_dbfs: self.level_dbfs,
            channel: (channel != 0).then_some(i64::from(channel)),
            last_channel: (c.last_channel != 0).then_some(i64::from(c.last_channel)),
            packets_37: c.per_channel[0] as i64,
            packets_38: c.per_channel[1] as i64,
            packets_39: c.per_channel[2] as i64,
            hopping: self.hop,
            privacy: BLE_PRIVACY_NOTE,
            packets_secondary: c.secondary as i64,
            packets_2m: c.two_m as i64,
            packets_coded: c.coded as i64,
            channels_in_capture: i64::from(self.stats.capture.count_ones()),
            aux_followed: c.aux_followed as i64,
            aux_outside: c.aux_outside as i64,
            periodic_trains: i64::from(self.stats.trains),
        });
        for &(name, n) in &per_type {
            BLE_PDU_TYPES.put(
                p,
                name,
                &BlePduTypeRow {
                    pdu: name,
                    packets: n as i64,
                },
            );
        }
        self.counters.per_type = per_type;
        let (filter, min_level, named_only) =
            (&self.filter_lower, self.min_level_dbfs, self.named_only);
        shaped::rows_where(&self.store.devices, &BLE_DEVICES, now, p, |r| {
            r.record.matches(filter)
                && r.record.level_dbfs >= min_level
                && (!named_only || !r.record.name.is_empty())
        });
        shaped::log_rows(&self.store.log, &BLE_PACKETS, now, p);
        self.publish_levels();
    }

    /// Writes the level history of every device to [`BLE_DEVICE_LEVELS`] and removes the rows of devices that left.
    fn publish_levels(&mut self) {
        let p = &mut self.published;
        let mut written = 0;
        for r in self.store.devices.iter() {
            let d = r.record;
            let first = d.levels - d.history.len() as u64;
            for (j, &v) in d.history.iter().enumerate() {
                let n = first + j as u64;
                self.key.clear();
                let _ = write!(self.key, "{}/{}", r.key, n % HISTORY as u64);
                BLE_DEVICE_LEVELS.put(
                    p,
                    &self.key,
                    &BleDeviceLevelRow {
                        device: r.key,
                        sample: n as i64,
                        level_dbfs: f64::from(v),
                    },
                );
                written += 1;
            }
        }
        remove_older_rows(p, BLE_DEVICE_LEVELS.def(), written);
    }

    fn request_channel(&mut self) {
        let hz = channel_hz(self.channel.number()).unwrap_or(2402e6);
        self.requests
            .push(HostRequest::retune(hz, "advertising channel chosen"));
    }

    fn request_hop(&mut self) {
        self.requests.push(HostRequest::Hop {
            channels: [37u8, 38, 39]
                .iter()
                .filter_map(|&c| channel_hz(c))
                .collect(),
            dwell_s: self.hop_seconds,
            stop_on_lock: false,
        });
    }
}

impl Component for DeviceTable {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wlan.ble_devices",
            vec![
                Port::typed::<Advert>("adverts"),
                Port::typed::<AdvStats>("stats"),
            ],
            vec![
                Port::new("events", Kind::Messages),
                Port::typed::<HostRequest>("requests"),
            ],
        )
        .with_settings(DeviceTableSetting::schema())
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A restart forgets the signal and keeps the tables: their times move back by the old signal time.
        if self.configured {
            self.store.shift_times(-self.now_s);
            if let Some(l) = &mut self.counters.last_good_s {
                *l -= self.now_s;
            }
        }
        self.configured = true;
        self.now_s = 0.0;
        self.stats = AdvStats::default();
        self.base = AdvStats::default();
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.clock = ctx.clock.clone();
    }

    fn process(&mut self, io: &mut Io) {
        for s in io.items::<AdvStats>(1) {
            self.stats = s.value;
            self.now_s = self.now_s.max(s.value.time_s);
            if s.value.level_dbfs.is_some() {
                self.level_dbfs = s.value.level_dbfs;
            }
        }
        for a in io.items::<Advert>(0) {
            if let Some(e) = self.take(&a.value) {
                self.events.push(e);
            }
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
        let now = self.now_s;
        for r in self.requests.drain(..) {
            io.items_out::<HostRequest>(1).push(Timed {
                time: now,
                value: r,
            });
        }
        if io.tick() {
            self.publish();
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DeviceTableSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DeviceTableSetting::Channel => {
                let c = CHANNEL_SETTING.read(id, v)?;
                if c != self.channel {
                    self.channel = c;
                    self.request_channel();
                }
            }
            DeviceTableSetting::Hop => {
                let on = HOP.read(id, v)?;
                if on != self.hop {
                    self.hop = on;
                    if on {
                        self.request_hop();
                    } else {
                        self.request_channel();
                    }
                }
            }
            DeviceTableSetting::HopSeconds => {
                let s = HOP_SECONDS.read(id, v)?;
                if s != self.hop_seconds {
                    self.hop_seconds = s;
                    if self.hop {
                        self.request_hop();
                    }
                }
            }
            DeviceTableSetting::FilterText => {
                FILTER_TEXT.read(id, v)?.clone_into(&mut self.filter);
                self.filter_lower = self.filter.to_lowercase();
            }
            DeviceTableSetting::MinLevel => self.min_level_dbfs = MIN_LEVEL.read(id, v)?,
            DeviceTableSetting::NamedOnly => self.named_only = NAMED_ONLY.read(id, v)?,
        }
        if self.configured {
            self.publish();
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (
                DeviceTableSetting::Channel,
                Value::Choice(self.channel.index()),
            ),
            (DeviceTableSetting::Hop, Value::Bool(self.hop)),
            (
                DeviceTableSetting::HopSeconds,
                Value::Duration(self.hop_seconds),
            ),
            (
                DeviceTableSetting::FilterText,
                Value::Text(self.filter.clone()),
            ),
            (
                DeviceTableSetting::MinLevel,
                Value::F64(self.min_level_dbfs),
            ),
            (DeviceTableSetting::NamedOnly, Value::Bool(self.named_only)),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }

    fn commands(&self) -> &Commands {
        DeviceTableCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match DeviceTableCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DeviceTableCommand::Clear => self.clear(),
            DeviceTableCommand::Remove => {
                let table = self
                    .published
                    .results()
                    .decl()
                    .table_id(BLE_DEVICES.key)
                    .ok_or(CommandError::NoSuchId { id })?;
                objcmd::remove(
                    &mut self.store.devices,
                    table,
                    &mut self.published,
                    args,
                    &BLE_DEVICE_ROW,
                )?;
            }
        }
        if self.configured {
            self.publish();
        }
        Ok(None)
    }

    /// The counters, the PDU types and the measurements start again; the devices, their levels and the packet log
    /// stay if `kept` holds them.
    fn forget(&mut self, kept: &Kept) {
        self.counters = Counters::default();
        self.base = self.stats;
        self.level_dbfs = None;
        self.published.clear_rows(BLE_PDU_TYPES.def());
        if !kept.table(BLE_DEVICES.key) {
            self.store.devices.clear();
        }
        if !kept.table(BLE_PACKETS.key) {
            self.store.log.clear();
        }
        if !kept.table(BLE_DEVICE_LEVELS.key) {
            for (_, d) in self.store.devices.records_mut() {
                d.history.clear();
            }
            self.published.clear_rows(BLE_DEVICE_LEVELS.def());
        }
        if self.configured {
            self.publish();
        }
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.now_s).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.now_s)?;
        self.publish();
        Ok(())
    }
}
