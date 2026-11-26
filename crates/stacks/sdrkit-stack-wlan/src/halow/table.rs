//! The `halow` network table, the frame log, the width view and what the stack publishes (the `wlan.halow_networks` component).
//! The table, the log and the names learnt from compressed SSIDs are kept in the saved state.

use super::chain::{S1gPpdu, Undecoded};
use super::mac::{self, vendors_text, FrameKind, S1gMac};
use super::phy::{max_width_mhz, rate_mbps, width_index, width_rates, Preamble, WIDTHS};
use super::plan::{class_channel_mhz, parse_numbers, Plan};
use super::BankStats;
use crate::report::{wall, PRIVACY_NOTE};
use crate::typed::{
    HalowFields, HalowFrameRow, HalowMcsRow, HalowNetworkRow, HalowPreambleRow, HalowWidthRow,
    HALOW_BY_MCS, HALOW_BY_PREAMBLE, HALOW_FRAMES, HALOW_NETWORKS, HALOW_WIDTHS, NETWORK_REMOVE,
    NETWORK_ROW,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Clock, ConfigError, HostRequest, LockState, Record, Results, Setting, SettingError,
    SettingId, SettingSet, StateBlob, StateError, Status, TableId, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kept, Kind, Port, RunContext, Timed,
};
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;
use sdrkit_util_objects::commands::{self as objcmd, Tables as _};
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Most networks kept.
pub const NETWORK_CAP: usize = 200;
/// Most log lines kept.
pub const LOG_CAP: usize = 600;
/// A frame heard within this time keeps the lock state on, s.
pub const ACTIVE_S: f64 = 5.0;
/// New networks that get a line in the text log.
const NEW_NETWORK_LINES: usize = 20;

/// The note on what the stack receives.
pub const RECEIVE_NOTE: &str = "Sub-GHz Wi-Fi HaLow (802.11ah) is for low power sensors and cameras at long range; 1, 2, 4, 8 and 16 MHz channels. 1 and 2 MHz channels work from 2.4 Msps, 4 MHz from 4 Msps, 8 MHz from 10 Msps, 16 MHz from 20 Msps.";
/// The note on what the data decoder reads.
pub const DATA_NOTE: &str = "The data field is decoded for one spatial stream with BCC coding at MCS 0 - 10. With several streams (a radio with one antenna cannot separate them), STBC, LDPC coding and A-MPDU the frame is detected and counted, its contents are not read.";

sdrkit_core::choice_enum! {
    /// Whether the receiver reads a width.
    pub enum WidthState {
        /// The width is read.
        Listening = ("listening", "listening"),
        /// The receive setting leaves the width out.
        NotSelected = ("not_selected", "not selected"),
        /// The sample rate is too low for the width.
        RateTooLow = ("rate_too_low", "sample rate too low"),
    }
}

sdrkit_core::choice_enum! {
    /// The width of the channels that the channel list and the scan use.
    pub enum ListWidth {
        /// 1 MHz channels.
        W1 = ("1", "1 MHz"),
        /// 2 MHz channels.
        W2 = ("2", "2 MHz"),
        /// 4 MHz channels.
        W4 = ("4", "4 MHz"),
        /// 8 MHz channels.
        W8 = ("8", "8 MHz"),
        /// 16 MHz channels.
        W16 = ("16", "16 MHz"),
    }
}

impl ListWidth {
    /// The width, MHz.
    pub fn mhz(self) -> u32 {
        match self {
            ListWidth::W1 => 1,
            ListWidth::W2 => 2,
            ListWidth::W4 => 4,
            ListWidth::W8 => 8,
            ListWidth::W16 => 16,
        }
    }
}

/// The channel starting frequency of the plan.
pub const START_HZ: Setting<field::Hz> =
    Setting::new("start_hz", "Channel starting frequency", "Hz", 0.0)
        .range(0.0, 1e9, 1e5)
        .help("Channel n is centred at this frequency plus n x 500 kHz.")
        .zero_means("no channel plan");
/// The help of the channel list settings.
const CHANNELS_HELP: &str = "Channel numbers apart by spaces or commas (\"1 3 5\").";
/// The 1 MHz channels of the plan.
pub const CHANNELS_1: Setting<field::Text> =
    Setting::new("channels_1", "1 MHz channels", "", "").help(CHANNELS_HELP);
/// The 2 MHz channels of the plan.
pub const CHANNELS_2: Setting<field::Text> =
    Setting::new("channels_2", "2 MHz channels", "", "").help(CHANNELS_HELP);
/// The 4 MHz channels of the plan.
pub const CHANNELS_4: Setting<field::Text> =
    Setting::new("channels_4", "4 MHz channels", "", "").help(CHANNELS_HELP);
/// The 8 MHz channels of the plan.
pub const CHANNELS_8: Setting<field::Text> =
    Setting::new("channels_8", "8 MHz channels", "", "").help(CHANNELS_HELP);
/// The 16 MHz channels of the plan.
pub const CHANNELS_16: Setting<field::Text> =
    Setting::new("channels_16", "16 MHz channels", "", "").help(CHANNELS_HELP);
/// The width of the channel list.
pub const LIST_WIDTH: Setting<field::Choice<ListWidth>> =
    Setting::choice("list_width", "Channel list width", ListWidth::W1);
/// The channel to listen on. A number that is not a channel of the plan at the list width is refused.
pub const CHANNEL_SETTING: Setting<field::I64> = Setting::new("channel", "Channel", "", 1)
    .range(1.0, 255.0, 1.0)
    .help("A channel of the plan at the list width: the host tunes to it.");
/// Scan the channels of the plan at the list width.
pub const SCAN: Setting<field::Bool> = Setting::new("scan", "Scan", "", false)
    .help("Asks the host to step through the plan's channels at the list width.");
/// The time on each channel while scanning.
pub const DWELL: Setting<field::Duration> =
    Setting::new("dwell_s", "Dwell", "s", 6.0).range(1.0, 60.0, 1.0);
/// Show only the networks whose SSID, BSSID, vendor or security holds this text.
pub const FILTER_TEXT: Setting<field::Text> = Setting::new("filter_text", "Filter", "", "")
    .help("Shows only the networks whose SSID, BSSID, vendor or security holds this text.")
    .section("Display");
/// Show only the networks at or above this level.
pub const MIN_LEVEL: Setting<field::F64> =
    Setting::new("min_level_dbfs", "Level above", "dBFS", -120.0)
        .range(-120.0, 0.0, 1.0)
        .section("Display");

sdrkit_core::settings! {
    /// The settings of [`HalowTable`].
    pub enum HalowTableSetting {
        /// The channel starting frequency.
        StartHz = START_HZ,
        /// The 1 MHz channels.
        Channels1 = CHANNELS_1,
        /// The 2 MHz channels.
        Channels2 = CHANNELS_2,
        /// The 4 MHz channels.
        Channels4 = CHANNELS_4,
        /// The 8 MHz channels.
        Channels8 = CHANNELS_8,
        /// The 16 MHz channels.
        Channels16 = CHANNELS_16,
        /// The width of the channel list.
        ListWidth = LIST_WIDTH,
        /// The channel.
        Channel = CHANNEL_SETTING,
        /// Scan the channels.
        Scan = SCAN,
        /// The dwell while scanning.
        Dwell = DWELL,
        /// The filter text.
        FilterText = FILTER_TEXT,
        /// The least level shown.
        MinLevel = MIN_LEVEL,
    }
}

sdrkit_core::commands! {
    /// The commands of [`HalowTable`].
    pub enum HalowTableCommand {
        /// Forget the networks, the log, the names and the counters.
        Clear = objcmd::CLEAR,
        /// Forget one network.
        Remove = NETWORK_REMOVE.command,
    }
}

/// One HaLow network.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HalowNetwork {
    /// BSSID.
    pub bssid: String,
    /// SSID ("" hidden or unknown).
    pub ssid: String,
    /// Hidden (an empty SSID element).
    pub hidden: bool,
    /// Compressed SSID.
    pub compressed_ssid: Option<u32>,
    /// Vendor.
    pub vendor: String,
    /// Vendor elements.
    pub vendor_elements: String,
    /// Channel number.
    pub channel: u32,
    /// Operating class.
    pub class: u8,
    /// Frequency, MHz.
    pub mhz: f64,
    /// Primary channel frequency, MHz.
    pub primary_mhz: f64,
    /// Received width of the last PPDU.
    pub width_mhz: u32,
    /// Operating width.
    pub op_width_mhz: u32,
    /// Primary width.
    pub primary_width_mhz: u32,
    /// Basic MCS map.
    pub basic_mcs: Option<u8>,
    /// MCS of the last PPDU.
    pub mcs: u8,
    /// Streams.
    pub nss: u8,
    /// Preamble.
    pub preamble: Option<Preamble>,
    /// Tone SNR.
    pub snr_db: f64,
    /// PHY rate, Mbit/s.
    pub rate_mbps: f64,
    /// Security.
    pub security: String,
    /// Beacon interval, TU.
    pub interval_tu: u16,
    /// Country.
    pub country: String,
    /// DTIM period.
    pub dtim_period: Option<u8>,
    /// RAW present.
    pub raw: bool,
    /// TWT present.
    pub twt: bool,
    /// Power constraint.
    pub power_constraint: Option<u8>,
    /// Long preamble supported.
    pub long_preamble: Option<bool>,
    /// Level.
    pub level_dbfs: f64,
    /// Maximum level.
    pub max_level_dbfs: f64,
    /// Packets.
    pub packets: u64,
    /// Beacons.
    pub beacons: u64,
    /// Last summary.
    pub last_summary: String,
    /// The compressed SSID as text ("c:1A2B3C4D"), made when it is heard.
    pub compressed_text: String,
}

impl HalowNetwork {
    fn ssid_text(&self) -> &str {
        if !self.ssid.is_empty() {
            &self.ssid
        } else if self.hidden {
            "(hidden)"
        } else if self.compressed_ssid.is_some() {
            &self.compressed_text
        } else {
            "-"
        }
    }

    fn shown(&self, text: &str, min_level: f64) -> bool {
        (text.is_empty()
            || [&self.ssid, &self.bssid, &self.vendor, &self.security]
                .iter()
                .any(|s| s.to_lowercase().contains(text)))
            && self.level_dbfs >= min_level
    }
}

impl Shift for HalowNetwork {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for HalowNetwork {
    type Row = HalowNetworkRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&HalowNetworkRow {
            ssid: self.ssid_text(),
            bssid: &self.bssid,
            vendor: &self.vendor,
            channel: i64::from(self.channel),
            freq_hz: (self.mhz > 0.0).then_some(Hertz(self.mhz * 1e6)),
            width_hz: Hertz(f64::from(self.width_mhz) * 1e6),
            op_width_hz: (self.op_width_mhz > 0)
                .then_some(Hertz(f64::from(self.op_width_mhz) * 1e6)),
            mcs: i64::from(self.mcs),
            nss: i64::from(self.nss),
            snr_db: self.snr_db,
            security: &self.security,
            // One time unit (TU) is 1024 µs (IEEE 802.11-2020 clause 3.1).
            interval: Seconds(f64::from(self.interval_tu) * 1.024e-3),
            level_dbfs: self.level_dbfs,
            beacons: self.beacons as i64,
            country: &self.country,
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// One log line.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HalowLog {
    /// Signal time.
    pub t: f64,
    /// Width.
    pub width: u32,
    /// MCS.
    pub mcs: u8,
    /// Streams.
    pub nss: u8,
    /// Preamble.
    pub preamble: Option<Preamble>,
    /// Short GI.
    pub sgi: bool,
    /// SNR.
    pub snr_db: f64,
    /// Rate.
    pub rate_mbps: f64,
    /// Length.
    pub len: usize,
    /// Decoded.
    pub decoded: bool,
    /// Level.
    pub level_dbfs: f64,
    /// Type text.
    pub kind: String,
    /// BSSID.
    pub bssid: String,
    /// Details.
    pub details: String,
}

impl Shift for HalowLog {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

impl Shaped for HalowLog {
    type Row = HalowFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&HalowFrameRow {
            signal_s: self.t,
            width_hz: Hertz(f64::from(self.width) * 1e6),
            mcs: i64::from(self.mcs),
            nss: i64::from(self.nss),
            preamble: self.preamble.map(Preamble::name),
            snr_db: self.snr_db,
            r#type: &self.kind,
            bssid: &self.bssid,
            length: self.len as i64,
            details: &self.details,
        });
    }
}

/// The networks, the log and the names learnt, kept across sessions.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The networks, by BSSID text.
    pub networks: ObjectTable<String, HalowNetwork>,
    /// The frame log.
    pub log: LogRing<HalowLog>,
    /// SSIDs by their CRC-32, for the compressed SSIDs of short beacons.
    pub names: BTreeMap<u32, String>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            networks: ObjectTable::new(NETWORK_CAP),
            log: LogRing::new(LOG_CAP),
            names: BTreeMap::new(),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.networks.shift_times(by_s);
        self.log.shift_times(by_s);
    }
}

impl objcmd::Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("wlan.halow_networks", 2);
    fn clear(&mut self) {
        self.networks.clear();
        self.log.clear();
        self.names.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.networks.adopt(saved.networks);
        self.log.adopt(saved.log);
        self.names = saved.names;
    }
}

/// The counters of the PPDUs taken since the last clear.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Counters {
    /// Good FCS.
    pub ok: u64,
    /// Bad FCS.
    pub bad: u64,
    /// Read from the SIG only.
    pub undecoded: u64,
    /// Several streams.
    pub mimo: u64,
    /// STBC.
    pub stbc: u64,
    /// LDPC.
    pub ldpc: u64,
    /// A-MPDU.
    pub agg: u64,
    /// NDP.
    pub ndp: u64,
    /// Good SIG by width.
    pub per_width: [u64; 5],
    /// By MCS.
    pub per_mcs: [u64; 11],
    /// By preamble (SHORT, LONG, 1M).
    pub per_pre: [u64; 3],
    /// Network frames.
    pub net_frames: u64,
    /// The last PPDU's carrier offset.
    pub offset_hz: f64,
    /// The running mean tone SNR.
    pub snr_db: Option<f64>,
    /// The last PPDU's signal time.
    pub last_good_s: Option<f64>,
    sec: i64,
    sec_count: u64,
    /// Frames in the last whole second.
    pub pps: u64,
}

fn undecoded_details(p: &S1gPpdu) -> String {
    if p.sig.ndp {
        return format!("{} MHz null data packet", p.width);
    }
    let n = p.nss();
    format!(
        "{} MHz MCS {}, {n} stream{}, {} {}: SIG field read, {} not decoded",
        p.width,
        p.sig.mcs,
        if n == 1 { "" } else { "s" },
        p.sig.length,
        if p.sig.aggregated { "symbols" } else { "bytes" },
        p.reason.map_or("", Undecoded::name)
    )
}

/// The network table as a component.
pub struct HalowTable {
    published: Published,
    /// The networks, the log and the names.
    pub store: Store,
    /// The counters.
    pub counters: Counters,
    stats: BankStats,
    base: BankStats,
    centre: f64,
    offset: f64,
    tuned_mhz: f64,
    hint: Option<u32>,
    plan: Plan,
    /// The channel starting frequency as the host set it, Hz.
    start_hz: f64,
    /// The texts of the channel lists, in the order of [`super::plan::WIDTHS_MHZ`].
    list_texts: [String; 5],
    list_width: ListWidth,
    channel: u32,
    scan: bool,
    dwell_s: f64,
    filter: String,
    filter_lower: String,
    min_level_dbfs: f64,
    requests: Vec<HostRequest>,
    new_logged: usize,
    lines: Vec<String>,
    status: Status,
    note: String,
    key: String,
    networks_table: TableId,
    now_s: f64,
    live: bool,
    clock: Clock,
    configured: bool,
}

impl Default for HalowTable {
    fn default() -> Self {
        let published = Published::new(HalowFields::SHAPE.fields).with_tables(&[
            HALOW_NETWORKS.def(),
            HALOW_FRAMES.def(),
            HALOW_WIDTHS.def(),
            HALOW_BY_MCS.def(),
            HALOW_BY_PREAMBLE.def(),
        ]);
        let networks_table = published
            .results()
            .decl()
            .table_id(HALOW_NETWORKS.key)
            .unwrap_or_else(|| unreachable!("the network table is declared"));
        HalowTable {
            published,
            store: Store::default(),
            counters: Counters::default(),
            stats: BankStats::default(),
            base: BankStats::default(),
            centre: 0.0,
            offset: 0.0,
            tuned_mhz: 0.0,
            hint: None,
            plan: Plan::default(),
            start_hz: START_HZ.default,
            list_texts: Default::default(),
            list_width: LIST_WIDTH.default,
            channel: u32::try_from(CHANNEL_SETTING.default).unwrap_or(1),
            scan: SCAN.default,
            dwell_s: DWELL.default,
            filter: String::new(),
            filter_lower: String::new(),
            min_level_dbfs: MIN_LEVEL.default,
            requests: Vec::new(),
            new_logged: 0,
            lines: Vec::new(),
            status: Status::default(),
            note: format!("{PRIVACY_NOTE} {RECEIVE_NOTE}"),
            key: String::new(),
            networks_table,
            now_s: 0.0,
            live: false,
            clock: Clock::fixed(0.0),
            configured: false,
        }
    }
}

impl HalowTable {
    fn count_ppdu(&mut self, p: &S1gPpdu) {
        let c = &mut self.counters;
        c.per_width[width_index(p.width)] += 1;
        c.per_mcs[(p.sig.mcs as usize).min(10)] += 1;
        if let Some(pre) = p.preamble {
            c.per_pre[pre.slot()] += 1;
        }
        c.offset_hz = p.cfo_hz;
        c.snr_db = Some(match c.snr_db {
            None => p.snr_db,
            Some(s) => s + 0.2 * (p.snr_db - s),
        });
        c.last_good_s = Some(p.time_s);
        let s = p.time_s.floor() as i64;
        if s != c.sec {
            c.pps = if s == c.sec + 1 { c.sec_count } else { 0 };
            c.sec = s;
            c.sec_count = 0;
        }
        c.sec_count += 1;
    }

    fn log_line(&mut self, p: &S1gPpdu, kind: String, bssid: String, details: String, len: usize) {
        self.store.log.push(HalowLog {
            t: p.time_s,
            width: p.width,
            mcs: p.sig.mcs,
            nss: p.nss(),
            preamble: p.preamble,
            sgi: p.sig.sgi,
            snr_db: p.snr_db,
            rate_mbps: rate_mbps(p.width, p.sig.mcs, p.nss(), p.sig.sgi),
            len,
            decoded: p.decoded,
            level_dbfs: p.level_dbfs,
            kind,
            bssid,
            details,
        });
    }

    /// Takes a PPDU: a decoded one with its MAC frame, or one read from its SIG field only.
    pub fn take(&mut self, p: &S1gPpdu) {
        if !p.decoded {
            self.take_undecoded(p);
            return;
        }
        let ok = sdrkit_proto_wlan_phy::mac::fcs_ok(&p.psdu);
        let m = if ok { mac::parse(&p.psdu) } else { None };
        self.take_frame(p, ok, m.as_ref());
    }

    fn take_undecoded(&mut self, p: &S1gPpdu) {
        self.count_ppdu(p);
        let c = &mut self.counters;
        c.undecoded += 1;
        match p.reason {
            Some(Undecoded::Ndp) => c.ndp += 1,
            Some(Undecoded::Stbc) => c.stbc += 1,
            Some(Undecoded::SpatialStreams) => c.mimo += 1,
            Some(Undecoded::Ldpc) => c.ldpc += 1,
            Some(Undecoded::Ampdu) => c.agg += 1,
            Some(Undecoded::Empty | Undecoded::Length) | None => {}
        }
        let kind = if p.sig.ndp { "NDP" } else { "PPDU" };
        self.log_line(
            p,
            kind.into(),
            String::new(),
            undecoded_details(p),
            p.sig.length as usize,
        );
    }

    fn take_frame(&mut self, p: &S1gPpdu, fcs_ok: bool, m: Option<&S1gMac>) {
        self.count_ppdu(p);
        if !fcs_ok {
            self.counters.bad += 1;
            return;
        }
        self.counters.ok += 1;
        let Some(m) = m else {
            // A good FCS on a frame that says nothing this stack reads (another protocol version).
            self.log_line(
                p,
                "frame".into(),
                String::new(),
                "frame of another protocol version".into(),
                p.psdu.len(),
            );
            return;
        };
        let kind = match m.kind {
            FrameKind::S1gBeacon => "S1G beacon".to_string(),
            FrameKind::ShortBeacon => "S1G short beacon".to_string(),
            FrameKind::Beacon => "beacon".to_string(),
            FrameKind::ProbeResp => "probe response".to_string(),
            FrameKind::Other => m.summary.clone(),
        };
        let bssid = m.bssid.clone().unwrap_or_default();
        self.log_line(p, kind, bssid.clone(), m.summary.clone(), p.psdu.len());
        // Names learnt by their CRC-32 (the compressed SSID, clause 9.4.2.207).
        if let Some(s) = &m.ssid {
            if !s.is_empty() {
                self.store
                    .names
                    .insert(CRC32_ISO_HDLC.checksum(s.as_bytes()), s.clone());
            }
        }
        let is_net = matches!(
            m.kind,
            FrameKind::S1gBeacon
                | FrameKind::ShortBeacon
                | FrameKind::Beacon
                | FrameKind::ProbeResp
        );
        if bssid.is_empty() || (!is_net && !self.store.networks.contains(bssid.as_str())) {
            return;
        }
        let at = match wall(self.live, &self.clock) {
            Some(w) => Stamp::at(p.time_s).with_wall(w),
            None => Stamp::at(p.time_s),
        };
        let vendor = {
            let b = parse_mac(&bssid);
            if sdrkit_util_ids::oui::is_locally_administered(b[0]) {
                "local"
            } else {
                sdrkit_util_ids::oui::vendor_of_address(&b).unwrap_or("")
            }
        };
        let Ok(up) = self
            .store
            .networks
            .upsert(bssid.as_str(), at, || HalowNetwork {
                bssid: bssid.clone(),
                vendor: vendor.into(),
                max_level_dbfs: f64::MIN,
                ..Default::default()
            })
        else {
            return;
        };
        if up.new && self.new_logged < NEW_NETWORK_LINES {
            self.new_logged += 1;
            self.lines.push(format!("HaLow: new network {bssid}"));
        }
        let centre = self.centre;
        let file = !self.live && centre <= 0.0;
        let n = up.row;
        n.width_mhz = p.width;
        n.mcs = p.sig.mcs;
        n.nss = p.nss();
        n.preamble = p.preamble;
        n.snr_db = p.snr_db;
        n.rate_mbps = rate_mbps(p.width, p.sig.mcs, p.nss(), p.sig.sgi);
        n.level_dbfs = p.level_dbfs;
        n.max_level_dbfs = n.max_level_dbfs.max(p.level_dbfs);
        n.packets += 1;
        n.last_summary = m.summary.clone();
        if !is_net {
            return;
        }
        self.counters.net_frames += 1;
        if matches!(
            m.kind,
            FrameKind::S1gBeacon | FrameKind::ShortBeacon | FrameKind::Beacon
        ) {
            n.beacons += 1;
        }
        if let Some(s) = &m.ssid {
            n.hidden = s.is_empty();
            if !s.is_empty() {
                n.ssid = s.clone();
            }
        }
        if let Some(c) = m.compressed_ssid {
            if n.compressed_ssid != Some(c) {
                n.compressed_text = format!("c:{c:08X}");
            }
            n.compressed_ssid = Some(c);
        }
        if let Some(op) = m.op {
            n.channel = op.channel as u32;
            n.class = op.class;
            n.op_width_mhz = op.width;
            n.primary_width_mhz = op.primary_width;
            n.basic_mcs = op.basic_mcs;
            match class_channel_mhz(op.class, op.channel as u32) {
                Some(f) => {
                    n.mhz = f;
                    n.primary_mhz = class_channel_mhz(op.class, op.primary as u32).unwrap_or(0.0);
                }
                None => {
                    n.mhz = if file {
                        0.0
                    } else {
                        (centre + p.sub_offset_hz) / 1e6
                    };
                }
            }
        } else if n.mhz == 0.0 && !file {
            n.mhz = (centre + p.sub_offset_hz) / 1e6;
        }
        if !m.vendors.is_empty() {
            n.vendor_elements = vendors_text(&m.vendors);
            if n.vendor.is_empty() || n.vendor == "local" {
                if let Some(v) = m
                    .vendors
                    .iter()
                    .find_map(|o| sdrkit_util_ids::oui::vendor_of_address(o))
                {
                    n.vendor = v.into();
                }
            }
        }
        if let Some(lp) = m.long_preamble {
            n.long_preamble = Some(lp);
        }
        if let Some(c) = &m.country {
            n.country = c.clone();
        }
        if m.dtim_period.is_some() {
            n.dtim_period = m.dtim_period;
        }
        n.raw |= m.raw;
        n.twt |= m.twt;
        if m.power_constraint.is_some() {
            n.power_constraint = m.power_constraint;
        }
        if !m.security.is_empty() && m.kind != FrameKind::ShortBeacon {
            n.security = m.security.clone();
        }
        if let Some(iv) = m.interval_tu {
            n.interval_tu = iv;
        }
    }

    fn apply_names(&mut self) {
        let names = &self.store.names;
        for (_, n) in self.store.networks.records_mut() {
            if n.ssid.is_empty() {
                if let Some(name) = n.compressed_ssid.and_then(|c| names.get(&c)) {
                    n.ssid = name.clone();
                }
            }
        }
    }

    fn clear(&mut self) {
        self.store.clear();
        self.counters = Counters::default();
        self.base = self.stats;
        self.published.clear_rows(HALOW_BY_MCS.def());
        self.published.clear_rows(HALOW_BY_PREAMBLE.def());
    }

    /// Publishes the results now.
    pub fn publish(&mut self) {
        self.apply_names();
        let now = self.now_s;
        let st = self.stats;
        let b = self.base;
        let tuned_mhz = if self.centre > 0.0 {
            (self.centre + self.offset) / 1e6
        } else {
            0.0
        };
        let hint = self.hint;
        let preambles = st.chains.preambles.saturating_sub(b.chains.preambles);
        let c = &self.counters;
        let active = c.last_good_s.is_some_and(|l| now - l <= ACTIVE_S);
        let frames = c.ok + c.bad + c.undecoded;
        let nets = self.store.networks.len();
        let status = &mut self.status;
        status.lock_state = if active {
            LockState::Locked
        } else {
            LockState::Searching
        };
        status.snr_db = c.snr_db.map(|s| s as f32);
        status.level_dbfs = st.level_dbfs.map(|l| l as f32);
        status.offset_hz = c.last_good_s.map(|_| c.offset_hz);
        status.summary.clear();
        // Writing to a String cannot fail.
        let _ = if frames > 0 {
            write!(
                status.summary,
                "{nets} networks, {} frames",
                c.ok + c.undecoded
            )
        } else if preambles > 0 {
            write!(
                status.summary,
                "{preambles} preambles, no frame decoded yet"
            )
        } else if tuned_mhz > 0.0 {
            write!(status.summary, "listening on {tuned_mhz:.1} MHz")
        } else {
            write!(status.summary, "listening for HaLow")
        };
        let max_w = max_width_mhz(st.rate_hz);
        let p = &mut self.published;
        {
            let r = p.results_mut();
            r.set_status(status);
            for line in self.lines.drain(..) {
                r.log(line);
            }
        }
        p.put(&HalowFields {
            networks: nets as i64,
            frames_ok: c.ok as i64,
            frames_bad: c.bad as i64,
            undecoded: c.undecoded as i64,
            mimo_frames: c.mimo as i64,
            stbc_frames: c.stbc as i64,
            ldpc_frames: c.ldpc as i64,
            ampdu_frames: c.agg as i64,
            ndp_frames: c.ndp as i64,
            preambles: preambles as i64,
            sig_ok: st.chains.sig_ok.saturating_sub(b.chains.sig_ok) as i64,
            sig_bad: st.chains.sig_bad.saturating_sub(b.chains.sig_bad) as i64,
            data_bad: st.chains.data_bad.saturating_sub(b.chains.data_bad) as i64,
            beacons_seen: c.net_frames as i64,
            frames_per_second: c.pps as i64,
            tuned_hz: (tuned_mhz > 0.0).then_some(Hertz(tuned_mhz * 1e6)),
            channel: hint.map(i64::from),
            max_width_hz: Hertz(f64::from(max_w) * 1e6),
            snr_db: c.snr_db,
            offset_hz: c.last_good_s.map(|_| Hertz(c.offset_hz)),
            level_dbfs: st.level_dbfs,
            scanning: self.scan,
            receive_note: &self.note,
            data_note: DATA_NOTE,
        });
        for (mcs, &n) in c.per_mcs.iter().enumerate() {
            if n > 0 {
                self.key.clear();
                let _ = write!(self.key, "{mcs}");
                HALOW_BY_MCS.put(
                    p,
                    &self.key,
                    &HalowMcsRow {
                        mcs: mcs as i64,
                        ppdus: n as i64,
                    },
                );
            }
        }
        for pre in [Preamble::Short, Preamble::Long, Preamble::OneM] {
            let n = c.per_pre[pre.slot()];
            if n > 0 {
                let key = Preamble::CHOICES[usize::from(pre.index())].key.as_ref();
                HALOW_BY_PREAMBLE.put(
                    p,
                    key,
                    &HalowPreambleRow {
                        preamble: pre,
                        ppdus: n as i64,
                    },
                );
            }
        }
        let (filter, min_level) = (&self.filter_lower, self.min_level_dbfs);
        shaped::rows_where(&self.store.networks, &HALOW_NETWORKS, now, p, |r| {
            r.record.shown(filter, min_level)
        });
        shaped::log_rows(&self.store.log, &HALOW_FRAMES, now, p);
        for (i, &w) in WIDTHS.iter().enumerate() {
            let (min, rec) = width_rates(w);
            let state = if st.width_mask & (1 << i) == 0 {
                if st.rate_hz < min * 0.999_999 {
                    WidthState::RateTooLow
                } else {
                    WidthState::NotSelected
                }
            } else {
                WidthState::Listening
            };
            self.key.clear();
            let _ = write!(self.key, "{w}");
            HALOW_WIDTHS.put(
                p,
                &self.key,
                &HalowWidthRow {
                    width_hz: Hertz(f64::from(w) * 1e6),
                    sig_ok: c.per_width[i] as i64,
                    min_rate_hz: Hertz(min),
                    rate_hz: Hertz(rec),
                    state,
                },
            );
        }
    }

    fn channel_hz(&self, number: u32) -> Option<f64> {
        self.plan
            .channels(self.list_width.mhz())
            .into_iter()
            .find(|c| c.number == number)
            .map(|c| c.mhz * 1e6)
    }

    fn request_scan(&mut self) {
        let channels: Vec<f64> = self
            .plan
            .channels(self.list_width.mhz())
            .into_iter()
            .map(|c| c.mhz * 1e6)
            .collect();
        if !channels.is_empty() {
            self.requests.push(HostRequest::Hop {
                channels,
                dwell_s: self.dwell_s,
                stop_on_lock: false,
            });
        }
    }

    fn request_channel(&mut self) {
        if let Some(hz) = self.channel_hz(self.channel) {
            self.requests
                .push(HostRequest::retune(hz, "HaLow channel chosen"));
        }
    }

    /// The nearest 1 MHz channel of the plan to the tuned frequency.
    fn update_hint(&mut self) {
        self.hint = self.plan.nearest(1, self.tuned_mhz).map(|c| c.number);
    }

    /// The plan changed: the hint follows, and a scan asks again.
    fn plan_changed(&mut self) {
        self.update_hint();
        if self.scan {
            self.request_scan();
        }
    }

    /// Sets the channel list of the width at `slot` of [`super::plan::WIDTHS_MHZ`] from its text.
    fn set_list(&mut self, id: SettingId, slot: usize, text: &str) -> Result<(), SettingError> {
        let numbers = parse_numbers(text).ok_or(SettingError::NotPermitted { id })?;
        text.clone_into(&mut self.list_texts[slot]);
        if self.plan.numbers[slot] != numbers {
            self.plan.numbers[slot] = numbers;
            self.plan_changed();
        }
        Ok(())
    }
}

/// The six bytes of a MAC address text ("AA:BB:CC:DD:EE:FF"), zeros where the text is not one.
fn parse_mac(text: &str) -> [u8; 6] {
    let mut out = [0u8; 6];
    for (o, p) in out.iter_mut().zip(text.split(':')) {
        *o = u8::from_str_radix(p, 16).unwrap_or(0);
    }
    out
}

impl Component for HalowTable {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wlan.halow_networks",
            vec![
                Port::typed::<S1gPpdu>("ppdus"),
                Port::typed::<BankStats>("stats"),
            ],
            vec![Port::typed::<HostRequest>("requests")],
        )
        .with_settings(HalowTableSetting::schema())
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
        self.stats = BankStats::default();
        self.base = BankStats::default();
        Ok(vec![Format::new(Kind::typed::<HostRequest>(), 0.0)])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.clock = ctx.clock.clone();
    }

    fn retuned(&mut self, centre_hz: f64, tuned_hz: f64) {
        self.centre = centre_hz;
        self.offset = tuned_hz - centre_hz;
        self.tuned_mhz = if centre_hz > 0.0 { tuned_hz / 1e6 } else { 0.0 };
        self.update_hint();
    }

    fn process(&mut self, io: &mut Io) {
        for s in io.items::<BankStats>(1) {
            self.stats = s.value;
            self.now_s = self.now_s.max(s.value.time_s);
        }
        for p in io.items::<S1gPpdu>(0) {
            self.take(&p.value);
        }
        let now = self.now_s;
        for r in self.requests.drain(..) {
            io.items_out::<HostRequest>(0).push(Timed {
                time: now,
                value: r,
            });
        }
        if io.tick() {
            self.publish();
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match HalowTableSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            HalowTableSetting::StartHz => {
                let hz = START_HZ.read(id, v)?;
                if hz != self.start_hz {
                    self.start_hz = hz;
                    self.plan.start = hz / 1e6;
                    self.plan_changed();
                }
            }
            HalowTableSetting::Channels1 => self.set_list(id, 0, CHANNELS_1.read(id, v)?)?,
            HalowTableSetting::Channels2 => self.set_list(id, 1, CHANNELS_2.read(id, v)?)?,
            HalowTableSetting::Channels4 => self.set_list(id, 2, CHANNELS_4.read(id, v)?)?,
            HalowTableSetting::Channels8 => self.set_list(id, 3, CHANNELS_8.read(id, v)?)?,
            HalowTableSetting::Channels16 => self.set_list(id, 4, CHANNELS_16.read(id, v)?)?,
            HalowTableSetting::ListWidth => {
                self.list_width = LIST_WIDTH.read(id, v)?;
                if self.scan {
                    self.request_scan();
                }
            }
            HalowTableSetting::Channel => {
                let c = u32::try_from(CHANNEL_SETTING.read(id, v)?)
                    .map_err(|_| SettingError::NotPermitted { id })?;
                if c != self.channel {
                    if self.channel_hz(c).is_none() {
                        return Err(SettingError::NotPermitted { id });
                    }
                    self.channel = c;
                    self.request_channel();
                }
            }
            HalowTableSetting::Scan => {
                let on = SCAN.read(id, v)?;
                if on != self.scan {
                    self.scan = on;
                    if on {
                        self.request_scan();
                    } else {
                        self.request_channel();
                    }
                }
            }
            HalowTableSetting::Dwell => {
                let d = DWELL.read(id, v)?;
                if d != self.dwell_s {
                    self.dwell_s = d;
                    if self.scan {
                        self.request_scan();
                    }
                }
            }
            HalowTableSetting::FilterText => {
                FILTER_TEXT.read(id, v)?.clone_into(&mut self.filter);
                self.filter_lower = self.filter.to_lowercase();
            }
            HalowTableSetting::MinLevel => self.min_level_dbfs = MIN_LEVEL.read(id, v)?,
        }
        if self.configured {
            self.publish();
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (HalowTableSetting::StartHz, Value::Hz(self.start_hz)),
            (
                HalowTableSetting::Channels1,
                Value::Text(self.list_texts[0].clone()),
            ),
            (
                HalowTableSetting::Channels2,
                Value::Text(self.list_texts[1].clone()),
            ),
            (
                HalowTableSetting::Channels4,
                Value::Text(self.list_texts[2].clone()),
            ),
            (
                HalowTableSetting::Channels8,
                Value::Text(self.list_texts[3].clone()),
            ),
            (
                HalowTableSetting::Channels16,
                Value::Text(self.list_texts[4].clone()),
            ),
            (
                HalowTableSetting::ListWidth,
                Value::Choice(self.list_width.index()),
            ),
            (
                HalowTableSetting::Channel,
                Value::I64(i64::from(self.channel)),
            ),
            (HalowTableSetting::Scan, Value::Bool(self.scan)),
            (HalowTableSetting::Dwell, Value::Duration(self.dwell_s)),
            (
                HalowTableSetting::FilterText,
                Value::Text(self.filter.clone()),
            ),
            (HalowTableSetting::MinLevel, Value::F64(self.min_level_dbfs)),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }

    fn commands(&self) -> &Commands {
        HalowTableCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match HalowTableCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            HalowTableCommand::Clear => self.clear(),
            HalowTableCommand::Remove => objcmd::remove(
                &mut self.store.networks,
                self.networks_table,
                &mut self.published,
                args,
                &NETWORK_ROW,
            )?,
        }
        if self.configured {
            self.publish();
        }
        Ok(None)
    }

    /// The counters, the MCS and preamble counts and the measurements start again; the networks and the frame log
    /// stay if `kept` holds them, with the names learnt from compressed SSIDs.
    fn forget(&mut self, kept: &Kept) {
        self.counters = Counters::default();
        self.base = self.stats;
        self.stats.level_dbfs = None;
        self.published.clear_rows(HALOW_BY_MCS.def());
        self.published.clear_rows(HALOW_BY_PREAMBLE.def());
        if !kept.table(HALOW_NETWORKS.key) {
            self.store.networks.clear();
            self.store.names.clear();
        }
        if !kept.table(HALOW_FRAMES.key) {
            self.store.log.clear();
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
