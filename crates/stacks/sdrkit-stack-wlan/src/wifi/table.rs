//! The `wifi` network table, the frame log, the per-band summary and what the stack publishes (the `wlan.wifi_networks`
//! component). The table and the log are kept in the saved state.

use super::mac::{WifiGeneration, WifiSecurity,
    combine, header, is_duplicate, parse_bss, rates_text, security, summary, Bss, BssFrame,
};
use super::plan::{centre_mhz, hop_list, parse_channels, valid, wide_centre_mhz, Band};
use super::stations::{Heard, Stations};
use super::{DsssCounters, OfdmCounters, PhyKind, PpduCounters, WifiMpdu, WifiStats};
use crate::report::{mac_text, push_history, remove_older_rows, wall};
use crate::typed::{
    WifiBandRow, WifiBeaconRow, WifiFields, WifiNetworkRow, WifiRateRow, WifiStationRow,
    NETWORK_REMOVE, NETWORK_ROW, WIFI_BANDS, WIFI_BEACONS, WIFI_NETWORKS, WIFI_OFDM_RATES,
    WIFI_STATIONS,
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
use sdrkit_util_objects::commands::{self as objcmd, Tables as _};
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Most networks kept.
pub const NETWORK_CAP: usize = 300;
/// Most log lines kept.
pub const LOG_CAP: usize = 400;
/// A frame heard within this time keeps the lock state on, s.
pub const ACTIVE_S: f64 = 5.0;
/// New networks that get a line in the text log.
const NEW_NETWORK_LINES: usize = 30;

/// The passive note.
pub const PASSIVE_NOTE: &str = "Passive: only frames sent in the clear and the headers of protected frames are read. Nothing is transmitted, joined or decrypted. One antenna cannot separate two or more spatial streams: such PPDUs, and PPDUs wider than 20 MHz, are counted from their signal fields and their data is not decoded.";

sdrkit_core::choice_enum! {
    /// The choices of the display setting `band_filter`.
    pub enum BandShown {
        /// Every band.
        All = ("all", "All"),
        /// 2.4 GHz only.
        G2 = ("2.4", "2.4 GHz"),
        /// 5 GHz only.
        G5 = ("5", "5 GHz"),
        /// 6 GHz only.
        G6 = ("6", "6 GHz"),
    }
}

impl BandShown {
    /// The band shown, `None` for every band.
    pub fn band(self) -> Option<Band> {
        match self {
            BandShown::All => None,
            BandShown::G2 => Some(Band::G2),
            BandShown::G5 => Some(Band::G5),
            BandShown::G6 => Some(Band::G6),
        }
    }
}

/// The band to listen on. Choosing a band requests the host to tune to the first channel of its hop list.
pub const BAND_SETTING: Setting<field::Choice<Band>> = Setting::choice("band", "Band", Band::G2)
    .help("Choosing a band asks the host to tune to the first channel of its hop list.");
/// The channel to listen on. A number that is not a channel of the band is refused.
pub const CHANNEL_SETTING: Setting<field::I64> = Setting::new("channel", "Channel", "", 6)
    .range(1.0, 233.0, 1.0)
    .help("A channel of the band: the host tunes to it.");
/// Hop through the hop list.
pub const HOP: Setting<field::Bool> = Setting::new("hop", "Hop", "", false).help(
    "Asks the host to step through the hop list; dwell longer than 102.4 ms (the beacon interval).",
);
/// The time on each channel while hopping.
pub const DWELL_MS: Setting<field::F64> =
    Setting::new("dwell_ms", "Dwell", "ms", 200.0).range(100.0, 2000.0, 10.0);
/// Hop only through the preferred scanning channels of 6 GHz.
pub const PSC_ONLY: Setting<field::Bool> = Setting::new(
    "psc_only",
    "6 GHz: preferred scanning channels only",
    "",
    true,
);
/// The help of the channel list settings.
const CHANNELS_HELP: &str =
    "Channel numbers apart by spaces or commas (\"1 6 11\"). Empty: every channel of the band.";
/// The 2.4 GHz channels of the hop list.
pub const CHANNELS_24: Setting<field::Text> =
    Setting::new("channels_24", "2.4 GHz channels", "", "").help(CHANNELS_HELP);
/// The 5 GHz channels of the hop list.
pub const CHANNELS_5: Setting<field::Text> =
    Setting::new("channels_5", "5 GHz channels", "", "").help(CHANNELS_HELP);
/// The 6 GHz channels of the hop list.
pub const CHANNELS_6: Setting<field::Text> =
    Setting::new("channels_6", "6 GHz channels", "", "").help(CHANNELS_HELP);
/// Show only the networks whose SSID, BSSID, vendor or security holds this text.
pub const FILTER_TEXT: Setting<field::Text> = Setting::new("filter_text", "Filter", "", "")
    .help("Shows only the networks whose SSID, BSSID, vendor or security holds this text.")
    .section("Display");
/// Show only the networks at or above this level.
pub const MIN_LEVEL: Setting<field::F64> =
    Setting::new("min_level_dbfs", "Level above", "dBFS", -120.0)
        .range(-120.0, 0.0, 1.0)
        .section("Display");
/// Show only open networks.
pub const OPEN_ONLY: Setting<field::Bool> =
    Setting::new("open_only", "Open only", "", false).section("Display");
/// Show only the networks of one band.
pub const BAND_FILTER: Setting<field::Choice<BandShown>> =
    Setting::choice("band_filter", "Band shown", BandShown::All).section("Display");

sdrkit_core::settings! {
    /// The settings of [`NetworkTable`].
    pub enum NetworkTableSetting {
        /// The band.
        Band = BAND_SETTING,
        /// The channel.
        Channel = CHANNEL_SETTING,
        /// Hop through the hop list.
        Hop = HOP,
        /// The dwell while hopping.
        DwellMs = DWELL_MS,
        /// Only the preferred scanning channels of 6 GHz.
        PscOnly = PSC_ONLY,
        /// The 2.4 GHz channels of the hop list.
        Channels24 = CHANNELS_24,
        /// The 5 GHz channels of the hop list.
        Channels5 = CHANNELS_5,
        /// The 6 GHz channels of the hop list.
        Channels6 = CHANNELS_6,
        /// The filter text.
        FilterText = FILTER_TEXT,
        /// The least level shown.
        MinLevel = MIN_LEVEL,
        /// Only open networks.
        OpenOnly = OPEN_ONLY,
        /// The band shown.
        BandFilter = BAND_FILTER,
    }
}

sdrkit_core::commands! {
    /// The commands of [`NetworkTable`].
    pub enum NetworkTableCommand {
        /// Forget the networks, the log and the counters.
        Clear = objcmd::CLEAR,
        /// Forget one network.
        Remove = NETWORK_REMOVE.command,
    }
}

/// The PHY of a frame as the tables show it: HT and VHT by their PPDU format.
fn phy_of(m: &WifiMpdu) -> PhyKind {
    match m.format {
        Some(sdrkit_proto_wlan_phy::ppdu::Format::Ht) => PhyKind::Ht,
        Some(sdrkit_proto_wlan_phy::ppdu::Format::Vht) => PhyKind::Vht,
        _ => m.phy,
    }
}

/// Writes an address into `out` (cleared first) as [`mac_text`] does.
fn mac_into(b: &[u8; 6], out: &mut String) {
    out.clear();
    for (i, x) in b.iter().enumerate() {
        if i > 0 {
            out.push(':');
        }
        sdrkit_core::base64::hex_into(std::slice::from_ref(x), out);
    }
}

/// The six bytes of an address written as "AA:BB:CC:DD:EE:FF".
fn parse_mac(text: &str) -> Option<[u8; 6]> {
    let mut b = [0u8; 6];
    let mut parts = text.split(':');
    for v in &mut b {
        *v = u8::from_str_radix(parts.next()?, 16).ok()?;
    }
    parts.next().is_none().then_some(b)
}

/// One network (one BSSID).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Network {
    /// BSSID, MAC text.
    pub bssid: String,
    /// SSID.
    pub ssid: String,
    /// Hidden.
    pub hidden: bool,
    /// The name came from a probe response.
    pub ssid_from_probe: bool,
    /// Vendor ("local" for a locally administered BSSID).
    pub vendor: String,
    /// The band.
    pub band: Option<Band>,
    /// Channel.
    pub channel: u32,
    /// Channel heard on.
    pub heard_on: u32,
    /// PHY of the last frame.
    pub phy: Option<PhyKind>,
    /// Security.
    pub security: WifiSecurity,
    /// Enterprise.
    pub enterprise: bool,
    /// Ciphers.
    pub ciphers: String,
    /// Key management.
    pub akm: String,
    /// Protected management frames.
    pub pmf: String,
    /// Rates.
    pub rates: String,
    /// Beacon interval, TU.
    pub interval_tu: u16,
    /// Capability word, hex.
    pub capability: String,
    /// Width, MHz.
    pub width_mhz: u32,
    /// Standard ("b/g/n/ax").
    pub standard: String,
    /// The newest amendment of the standard.
    pub generation: Option<WifiGeneration>,
    /// The centre frequency of the whole channel, MHz.
    pub centre_mhz: Option<f64>,
    /// Capabilities in plain words.
    pub caps: String,
    /// BSS color.
    pub bss_color: Option<u8>,
    /// TWT.
    pub twt: bool,
    /// EHT.
    pub eht: bool,
    /// Country.
    pub country: String,
    /// Power constraint, dB.
    pub power_constraint_db: Option<u8>,
    /// Mobility domain, hex.
    pub mobility_domain: String,
    /// 6 GHz AP power type.
    pub ap_power: String,
    /// FILS Discovery heard.
    pub fils: bool,
    /// Unsolicited probe responses heard.
    pub unsolicited_probe_responses: bool,
    /// Neighbours text.
    pub neighbors: String,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// The last 24 levels.
    pub history: Vec<f32>,
    /// Beacons.
    pub beacons: u64,
    /// Probe responses.
    pub probe_responses: u64,
    /// FILS frames.
    pub fils_frames: u64,
    /// The text of the details column.
    pub details_text: String,
    /// Client stations heard with this BSSID, as of the last publish.
    pub stations: u32,
}

impl Network {
    /// The details column.
    pub fn details(&self) -> String {
        let mut p: Vec<String> = Vec::new();
        if !self.caps.is_empty() {
            p.push(self.caps.clone());
        }
        if !self.akm.is_empty() {
            p.push(match self.pmf.as_str() {
                "required" => format!("{}, PMF required", self.akm),
                "capable" => format!("{}, PMF", self.akm),
                _ => self.akm.clone(),
            });
        }
        if !self.country.is_empty() {
            p.push(format!("country {}", self.country));
        }
        if let Some(c) = self.power_constraint_db {
            p.push(format!("power constraint {c} dB"));
        }
        if !self.mobility_domain.is_empty() {
            p.push(format!("mobility domain {}", self.mobility_domain));
        }
        if !self.ap_power.is_empty() {
            p.push(self.ap_power.clone());
        }
        if self.fils {
            p.push("FILS discovery".into());
        }
        if self.unsolicited_probe_responses {
            p.push("unsolicited probe responses".into());
        }
        if !self.neighbors.is_empty() {
            p.push(format!("neighbors: {}", self.neighbors));
        }
        if p.is_empty() {
            return format!("rates {}", self.rates);
        }
        p.join("; ")
    }

    /// Whether the network passes the screen filters (the text is lower case already).
    fn shown(&self, f: &Filters) -> bool {
        (f.lower.is_empty()
            || [&self.ssid, &self.bssid, &self.vendor, self.security.label()]
                .iter()
                .any(|s| s.to_lowercase().contains(&f.lower)))
            && self.level_dbfs >= f.min_level_dbfs
            && (!f.open_only || self.security.is_open())
            && f.band.is_none_or(|b| self.band == Some(b))
    }
}

impl Shift for Network {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Network {
    type Row = WifiNetworkRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&WifiNetworkRow {
            ssid: if self.ssid.is_empty() {
                "(hidden)"
            } else {
                &self.ssid
            },
            bssid: &self.bssid,
            vendor: if self.vendor == "local" {
                "(local)"
            } else {
                &self.vendor
            },
            band: self.band,
            channel: (self.channel != 0).then_some(i64::from(self.channel)),
            heard_on: (self.heard_on != 0).then_some(i64::from(self.heard_on)),
            centre_hz: self.centre_mhz.map(|f| Hertz(f * 1e6)),
            width_hz: Hertz(f64::from(self.width_mhz) * 1e6),
            generation: self.generation,
            security: self.security,
            level_dbfs: self.level_dbfs,
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
            beacons: self.beacons as i64,
            probe_responses: self.probe_responses as i64,
            // One time unit (TU) is 1024 µs (IEEE 802.11-2020 clause 3.1).
            interval: Seconds(f64::from(self.interval_tu) * 1.024e-3),
            ciphers: &self.ciphers,
            akm: &self.akm,
            pmf: &self.pmf,
            capability: &self.capability,
            details: &self.details_text,
            stations: i64::from(self.stations),
        });
    }
}

/// One log line.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LogLine {
    /// Signal time.
    pub t: f64,
    /// Channel heard on.
    pub ch: u32,
    /// BSSID.
    pub bssid: String,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// Length, FCS included.
    pub len: usize,
    /// Summary.
    pub info: String,
    /// The PPDU format name of an HT or VHT frame; empty otherwise.
    pub format: String,
    /// The MCS of an HT or VHT frame.
    pub mcs: Option<u8>,
    /// The frame came in an A-MPDU.
    pub aggregated: bool,
}

impl Shift for LogLine {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

impl Shaped for LogLine {
    type Row = WifiBeaconRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&WifiBeaconRow {
            signal_s: self.t,
            channel: (self.ch != 0).then_some(i64::from(self.ch)),
            bssid: &self.bssid,
            level_dbfs: self.level_dbfs,
            bytes: self.len as i64,
            details: &self.info,
            format: (!self.format.is_empty()).then_some(self.format.as_str()),
            mcs: self.mcs.map(i64::from),
            aggregated: self.aggregated,
        });
    }
}

/// The networks and the log, kept across sessions.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The networks, by BSSID text.
    pub networks: ObjectTable<String, Network>,
    /// The frame log.
    pub log: LogRing<LogLine>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            networks: ObjectTable::new(NETWORK_CAP),
            log: LogRing::new(LOG_CAP),
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
    const FORMAT: StateFormat = StateFormat::new("wlan.wifi_networks", 3);
    fn clear(&mut self) {
        self.networks.clear();
        self.log.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.networks.adopt(saved.networks);
        self.log.adopt(saved.log);
    }
}

/// The counters of the frames taken since the last clear.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Counters {
    /// Good frames by type: management, control, data.
    pub per_type: [u64; 3],
    /// Beacons.
    pub beacons: u64,
    /// Probe responses.
    pub probe_resps: u64,
    /// FILS Discovery frames.
    pub fils_frames: u64,
    /// Duplicates on secondary channels.
    pub dup_frames: u64,
    /// Good OFDM frames by rate (6, 9, 12, 18, 24, 36, 48, 54 Mbit/s).
    pub ofdm_rates: [u64; 8],
    /// The running mean SNR.
    pub snr_db: Option<f64>,
    /// The last frame's carrier offset.
    pub offset_hz: f64,
    /// The last frame's PHY.
    pub phy: Option<PhyKind>,
    /// The last good frame's signal time.
    pub last_good_s: Option<f64>,
}

#[derive(Clone, Debug, Default)]
struct Filters {
    text: String,
    lower: String,
    min_level_dbfs: f64,
    open_only: bool,
    band: Option<Band>,
}

/// The network table as a component.
pub struct NetworkTable {
    published: Published,
    /// The networks and the log.
    pub store: Store,
    /// The counters.
    pub counters: Counters,
    stats: WifiStats,
    base_dsss: DsssCounters,
    base_ofdm: OfdmCounters,
    base_ppdu: PpduCounters,
    /// The client stations (not in the saved state).
    pub stations: Stations,
    level_dbfs: Option<f64>,
    filters: Filters,
    band: Band,
    channel: u32,
    hop: bool,
    dwell_ms: f64,
    psc_only: bool,
    /// The channel list of each band, in the order of [`Band::ALL`]: its text and its numbers.
    lists: [(String, Vec<u32>); 3],
    requests: Vec<HostRequest>,
    new_logged: usize,
    lines: Vec<String>,
    status: Status,
    networks_table: TableId,
    now_s: f64,
    live: bool,
    clock: Clock,
    configured: bool,
    /// Text buffers of the station rows, kept so a publish allocates nothing.
    texts: [String; 3],
}

impl Default for NetworkTable {
    fn default() -> Self {
        let published = Published::new(WifiFields::SHAPE.fields).with_tables(&[
            WIFI_NETWORKS.def(),
            WIFI_STATIONS.def(),
            WIFI_BEACONS.def(),
            WIFI_OFDM_RATES.def(),
            WIFI_BANDS.def(),
        ]);
        let networks_table = published
            .results()
            .decl()
            .table_id(WIFI_NETWORKS.key)
            .unwrap_or_else(|| unreachable!("the network table is declared"));
        NetworkTable {
            published,
            store: Store::default(),
            counters: Counters::default(),
            stats: WifiStats::default(),
            base_dsss: DsssCounters::default(),
            base_ofdm: OfdmCounters::default(),
            base_ppdu: PpduCounters::default(),
            stations: Stations::default(),
            level_dbfs: None,
            filters: Filters {
                min_level_dbfs: MIN_LEVEL.default,
                open_only: OPEN_ONLY.default,
                ..Default::default()
            },
            band: BAND_SETTING.default,
            channel: u32::try_from(CHANNEL_SETTING.default).unwrap_or(6),
            hop: HOP.default,
            dwell_ms: DWELL_MS.default,
            psc_only: PSC_ONLY.default,
            lists: Default::default(),
            requests: Vec::new(),
            new_logged: 0,
            lines: Vec::new(),
            status: Status::default(),
            networks_table,
            now_s: 0.0,
            live: false,
            clock: Clock::fixed(0.0),
            configured: false,
            texts: Default::default(),
        }
    }
}

/// The data rates of the non-HT OFDM PHY, Mbit/s. Standard: IEEE Std 802.11-2020 clause 17.3.2.3, Table 17-4.
const RATES: [f64; 8] = [6.0, 9.0, 12.0, 18.0, 24.0, 36.0, 48.0, 54.0];
/// The row keys of [`WIFI_OFDM_RATES`], one for each of [`RATES`].
const RATE_KEYS: [&str; 8] = ["6", "9", "12", "18", "24", "36", "48", "54"];

/// The summary of the networks of one band.
#[derive(Default)]
struct BandSum {
    count: i64,
    beacons: i64,
    ax_be: i64,
    wpa3_owe: i64,
    open: i64,
    strongest: f64,
}

impl BandSum {
    fn add(&mut self, n: &Network) {
        if self.count == 0 {
            self.strongest = f64::MIN;
        }
        self.count += 1;
        self.beacons += n.beacons as i64;
        self.ax_be += i64::from(matches!(
            n.generation,
            Some(WifiGeneration::Ax | WifiGeneration::Be)
        ));
        self.wpa3_owe += i64::from(n.security.is_wpa3_or_owe());
        self.open += i64::from(n.security == WifiSecurity::Open);
        self.strongest = self.strongest.max(n.level_dbfs);
    }
}

impl NetworkTable {
    /// Takes a good MPDU: the counters, the network row and the log.
    pub fn take(&mut self, m: &WifiMpdu) {
        let now = m.time_s;
        let c = &mut self.counters;
        c.snr_db = match (c.snr_db, m.snr_db) {
            (None, Some(s)) => Some(s),
            (Some(a), Some(s)) => Some(a + 0.2 * (s - a)),
            (a, None) => a,
        };
        c.offset_hz = m.offset_hz;
        c.phy = Some(phy_of(m));
        c.last_good_s = Some(now);
        if let Some(i) = RATES.iter().position(|&r| r == m.rate_mbps) {
            if m.phy == super::PhyKind::Ofdm && m.format.is_none() {
                c.ofdm_rates[i] += 1;
            }
        }
        let Some(h) = header(&m.bytes) else {
            return;
        };
        if h.ftype <= 2 {
            c.per_type[h.ftype as usize] += 1;
        }
        self.stations.take(
            &m.bytes,
            Heard {
                time_s: now,
                level_dbfs: m.level_dbfs,
                channel: m.channel,
            },
        );
        let Some(b) = parse_bss(&m.bytes) else {
            return;
        };
        self.stations.access_point(b.bssid);
        if is_duplicate(&b, m.band, m.channel) {
            c.dup_frames += 1;
            return;
        }
        match b.frame {
            BssFrame::Beacon => c.beacons += 1,
            BssFrame::ProbeResp => c.probe_resps += 1,
            BssFrame::Fils => c.fils_frames += 1,
        }
        self.update_row(&b, m, now);
    }

    fn update_row(&mut self, b: &Bss, m: &WifiMpdu, now: f64) {
        let key = mac_text(&b.bssid);
        let at = match wall(self.live, &self.clock) {
            Some(w) => Stamp::at(now).with_wall(w),
            None => Stamp::at(now),
        };
        let vendor = if sdrkit_util_ids::oui::is_locally_administered(b.bssid[0]) {
            "local"
        } else {
            sdrkit_util_ids::oui::vendor_of_address(&b.bssid).unwrap_or("")
        };
        let Ok(up) = self.store.networks.upsert(key.as_str(), at, || Network {
            bssid: key.clone(),
            vendor: vendor.into(),
            hidden: true,
            ..Default::default()
        }) else {
            return;
        };
        if up.new && self.new_logged < NEW_NETWORK_LINES {
            self.new_logged += 1;
            self.lines.push(format!("Wi-Fi: new network {key}"));
        }
        let n = up.row;
        let band = m.band;
        let comb = combine(&b.el, band);
        let sec = security(&b.el, b.capability);
        match b.frame {
            BssFrame::Beacon => {
                n.beacons += 1;
                n.hidden = b.hidden;
                if !b.hidden {
                    n.ssid.clone_from(&b.ssid);
                    n.ssid_from_probe = false;
                }
            }
            BssFrame::ProbeResp => {
                n.probe_responses += 1;
                if !b.ssid.is_empty() {
                    if n.ssid.is_empty() || n.hidden {
                        n.ssid_from_probe = n.hidden;
                    }
                    n.ssid.clone_from(&b.ssid);
                }
                if b.unsolicited {
                    n.unsolicited_probe_responses = true;
                }
            }
            BssFrame::Fils => {
                n.fils_frames += 1;
                n.fils = true;
                if n.ssid.is_empty() && !b.ssid.is_empty() {
                    n.ssid.clone_from(&b.ssid);
                    n.hidden = false;
                }
            }
        }
        match comb.channel {
            Some(c) if b.frame != BssFrame::Fils => n.channel = c as u32,
            _ => {
                if n.channel == 0 && matches!(band, Some(Band::G5 | Band::G6)) {
                    n.channel = m.channel;
                }
            }
        }
        n.heard_on = m.channel;
        n.band = band;
        n.phy = Some(phy_of(m));
        if b.frame != BssFrame::Fils {
            n.security = sec.kind;
            n.enterprise = sec.enterprise;
            n.ciphers.clone_from(&sec.ciphers);
            n.akm.clone_from(&sec.akms);
            n.pmf.clone_from(&sec.pmf);
            if !b.el.rates.is_empty() {
                n.rates = rates_text(&b.el.rates);
            }
            n.interval_tu = b.interval_tu;
            n.capability = format!("{:04X}", b.capability);
            n.width_mhz = comb.width;
            n.centre_mhz = band.and_then(|bd| match comb.centre {
                Some(c) if comb.width > 20 => wide_centre_mhz(bd, u32::from(c)),
                _ => centre_mhz(bd, n.channel),
            });
            n.standard.clone_from(&comb.standard);
            n.generation = comb.generation;
            n.caps.clone_from(&comb.caps);
            n.bss_color = b.el.he_op.map(|o| o.color);
            n.twt = b.el.he.is_some_and(|h| h.twt_req || h.twt_resp);
            n.eht = b.el.eht_caps;
            if let Some(c) = &b.el.country {
                n.country.clone_from(c);
            }
            if b.el.power_constraint.is_some() {
                n.power_constraint_db = b.el.power_constraint;
            }
            if let Some(md) = b.el.mdid {
                n.mobility_domain = format!("{md:04X}");
            }
            if let Some(s) = b.el.he_op.and_then(|h| h.six) {
                n.ap_power = match s.reg {
                    0 => "indoor AP",
                    1 => "standard power AP",
                    2 => "very low power AP",
                    _ => "",
                }
                .into();
            }
            if b.el.has_rnr {
                n.neighbors =
                    b.el.rnr
                        .iter()
                        .map(|x| x.text())
                        .collect::<Vec<_>>()
                        .join("; ");
            }
        } else if n.interval_tu == 0 {
            n.interval_tu = b.interval_tu;
        }
        n.level_dbfs = m.level_dbfs;
        n.details_text = n.details();
        push_history(&mut n.history, m.level_dbfs as f32, 24);
        let info = summary(b, comb.channel, n.security.label());
        self.store.log.push(LogLine {
            t: now,
            ch: m.channel,
            bssid: key,
            level_dbfs: m.level_dbfs,
            len: m.bytes.len() + 4,
            info,
            format: m.format.map(|f| f.name().to_owned()).unwrap_or_default(),
            mcs: m.mcs,
            aggregated: m.aggregated,
        });
    }

    fn clear(&mut self) {
        self.store.clear();
        self.stations.clear();
        self.counters = Counters::default();
        self.base_dsss = self.stats.dsss;
        self.base_ofdm = self.stats.ofdm;
        self.base_ppdu = self.stats.ppdu;
        self.published.clear_rows(WIFI_OFDM_RATES.def());
        self.published.clear_rows(WIFI_BANDS.def());
    }

    /// Publishes the results now.
    pub fn publish(&mut self) {
        let now = self.now_s;
        let st = self.stats;
        let d = st.dsss.since(self.base_dsss);
        let o = st.ofdm.since(self.base_ofdm);
        let pp = st.ppdu.since(&self.base_ppdu);
        let c = &self.counters;
        let active = c.last_good_s.is_some_and(|l| now - l <= ACTIVE_S);
        let nets = self.store.networks.len();
        let status = &mut self.status;
        status.lock_state = if active {
            LockState::Locked
        } else {
            LockState::Searching
        };
        status.snr_db = c.snr_db.map(|s| s as f32);
        status.level_dbfs = self.level_dbfs.map(|l| l as f32);
        status.offset_hz = c.last_good_s.map(|_| c.offset_hz);
        status.summary.clear();
        // Writing to a String cannot fail.
        let _ = if nets > 0 {
            write!(status.summary, "{nets} networks, {} beacons", c.beacons)
        } else if st.channel != 0 {
            write!(status.summary, "listening on channel {}", st.channel)
        } else {
            write!(status.summary, "listening for beacons")
        };
        if !self.stations.is_empty() {
            let _ = write!(status.summary, ", {} stations", self.stations.len());
        }
        if st.rate_hz > 0.0 && st.rate_hz < super::OFDM_RATE_HZ * 0.999_999 {
            let msps = st.rate_hz / 1e6;
            let _ = if matches!(st.band, None | Some(Band::G2)) {
                write!(
                    status.summary,
                    "; at {msps:.1} Msps only 802.11b DSSS frames are received, OFDM (802.11a/g/n/ac/ax) needs 20 Msps"
                )
            } else {
                write!(
                    status.summary,
                    "; nothing is received at {msps:.1} Msps in this band: OFDM needs 20 Msps"
                )
            };
        }
        let skipped: u64 = pp.not_decoded.iter().sum();
        if skipped > 0 {
            let _ = write!(status.summary, "; {skipped} PPDUs seen and not decoded");
            let streams = pp.not_decoded_for(sdrkit_proto_wlan_phy::ppdu::Undecoded::Streams);
            if streams > 0 {
                let _ = write!(
                    status.summary,
                    " ({streams} with two or more spatial streams: one antenna cannot separate them)"
                );
            } else if let Some((info, why)) = pp.last {
                let _ = write!(status.summary, " (last: {}, {})", info.format.name(), why.text());
            }
        }
        let fc = self.stations.counts;
        for (_, n) in self.store.networks.records_mut() {
            n.stations = parse_mac(&n.bssid).map_or(0, |b| self.stations.of_bss(&b) as u32);
        }
        let p = &mut self.published;
        {
            let r = p.results_mut();
            r.set_status(status);
            for line in self.lines.drain(..) {
                r.log(line);
            }
        }
        p.put(&WifiFields {
            networks: nets as i64,
            frames_ok: (d.frames + o.frames + pp.mpdus) as i64,
            frames_bad: (d.bad_fcs + o.bad_fcs + pp.bad_fcs) as i64,
            dsss_sfds: d.sfds as i64,
            dsss_headers_ok: d.headers_ok as i64,
            dsss_headers_bad: d.headers_bad as i64,
            dsss_cck_headers: d.other_rate as i64,
            ofdm_preambles: o.preambles as i64,
            ofdm_signal_ok: o.signal_ok as i64,
            ofdm_signal_bad: o.signal_bad as i64,
            mgmt_frames: c.per_type[0] as i64,
            control_frames: c.per_type[1] as i64,
            data_frames: c.per_type[2] as i64,
            beacons: c.beacons as i64,
            probe_responses: c.probe_resps as i64,
            fils_frames: c.fils_frames as i64,
            duplicate_frames: c.dup_frames as i64,
            band: st.band,
            channel: (st.channel != 0).then_some(i64::from(st.channel)),
            sub_channels: i64::from(st.sub_channels),
            phy: c.phy,
            snr_db: c.snr_db,
            offset_hz: c.last_good_s.map(|_| Hertz(c.offset_hz)),
            level_dbfs: self.level_dbfs,
            hopping: self.hop,
            passive: PASSIVE_NOTE,
            stations: self.stations.len() as i64,
            ppdu_ht: pp.seen[0] as i64,
            ppdu_vht: pp.seen[1] as i64,
            ppdu_he_su: pp.seen[2] as i64,
            ppdu_he_er_su: pp.seen[3] as i64,
            ppdu_he_mu: pp.seen[4] as i64,
            ppdu_he_tb: pp.seen[5] as i64,
            ppdu_he_bad_sig: pp.seen[6] as i64,
            ppdu_eht: pp.seen[7] as i64,
            ppdu_decoded: pp.decoded as i64,
            ppdu_not_decoded_streams: pp.not_decoded[0] as i64,
            ppdu_not_decoded_width: pp.not_decoded[1] as i64,
            ppdu_not_decoded_ldpc: pp.not_decoded[2] as i64,
            ppdu_not_decoded_stbc: pp.not_decoded[3] as i64,
            ppdu_not_decoded_multi_user: pp.not_decoded[4] as i64,
            ppdu_not_decoded_trigger_based: pp.not_decoded[5] as i64,
            ppdu_not_decoded_he_data: pp.not_decoded[6] as i64,
            ppdu_not_decoded_eht_data: pp.not_decoded[7] as i64,
            ppdu_not_decoded_signal_field: pp.not_decoded[8] as i64,
            ppdu_not_decoded_mcs: pp.not_decoded[9] as i64,
            ppdu_not_decoded_length: pp.not_decoded[10] as i64,
            ppdu_not_decoded_data: pp.not_decoded[11] as i64,
            ppdu_not_decoded_other: pp.not_decoded[12] as i64,
            ppdu_not_decoded_busy: pp.not_decoded[13] as i64,
            ampdu_bad_delimiters: pp.bad_delimiters as i64,
            probe_requests: fc.probe_requests as i64,
            authentications: fc.authentications as i64,
            associations: fc.associations as i64,
            departures: fc.departures as i64,
            actions: fc.actions as i64,
            rts: fc.rts as i64,
            cts: fc.cts as i64,
            acks: fc.acks as i64,
            block_acks: fc.block_acks as i64,
        });
        for (i, (&rate, &n)) in RATES.iter().zip(&c.ofdm_rates).enumerate() {
            if n > 0 {
                WIFI_OFDM_RATES.put(
                    p,
                    RATE_KEYS[i],
                    &WifiRateRow {
                        rate_mbps: rate,
                        frames: n as i64,
                    },
                );
            }
        }
        let f = &self.filters;
        shaped::rows_where(&self.store.networks, &WIFI_NETWORKS, now, p, |r| {
            r.record.shown(f)
        });
        shaped::log_rows(&self.store.log, &WIFI_BEACONS, now, p);
        let wall_now = wall(self.live, &self.clock);
        let mut written = 0;
        let [key, bssid, ssids] = &mut self.texts;
        for s in self.stations.iter() {
            mac_into(&s.addr, key);
            if let Some(b) = s.bssid {
                mac_into(&b, bssid);
            }
            ssids.clear();
            for (i, x) in s.ssids.iter().enumerate() {
                if i > 0 {
                    ssids.push_str(", ");
                }
                ssids.push_str(x);
            }
            WIFI_STATIONS.put(
                p,
                key,
                &WifiStationRow {
                    address: key,
                    vendor: s.vendor,
                    randomised: s.randomised,
                    bssid: s.bssid.map(|_| bssid.as_str()),
                    link: s.link.into(),
                    last_heard: wall_now
                        .zip(s.last_heard_s)
                        .map(|(w, t)| Time(w - (now - t))),
                    level_dbfs: s.level_dbfs,
                    sent: s.sent as i64,
                    received: s.received as i64,
                    probes: s.probes as i64,
                    data: s.data as i64,
                    null_data: s.null_data as i64,
                    protected: s.protected as i64,
                    ssids,
                    generation: WifiGeneration::of_number(s.generation),
                    max_rate_mbps: s.max_rate_mbps,
                    channel: (s.channel != 0).then_some(i64::from(s.channel)),
                },
            );
            written += 1;
        }
        remove_older_rows(p, WIFI_STATIONS.def(), written);
        for b in Band::ALL {
            let mut sum = BandSum::default();
            for n in self.store.networks.iter().map(|r| r.record) {
                if n.band == Some(b) {
                    sum.add(n);
                }
            }
            if sum.count == 0 {
                continue;
            }
            WIFI_BANDS.put(
                p,
                Band::CHOICES[usize::from(b.index())].key.as_ref(),
                &WifiBandRow {
                    band: b,
                    networks: sum.count,
                    beacons: sum.beacons,
                    ax_be: sum.ax_be,
                    wpa3_owe: sum.wpa3_owe,
                    open: sum.open,
                    strongest_dbfs: sum.strongest,
                },
            );
        }
    }

    /// The hop list of a band.
    fn hops(&self, band: Band) -> Vec<u32> {
        hop_list(
            band,
            &self.lists[usize::from(band.index())].1,
            self.psc_only,
        )
    }

    /// Sets the channel list of a band from its text.
    fn set_list(&mut self, id: SettingId, band: Band, text: &str) -> Result<(), SettingError> {
        let numbers = parse_channels(band, text).ok_or(SettingError::NotPermitted { id })?;
        let slot = &mut self.lists[usize::from(band.index())];
        text.clone_into(&mut slot.0);
        if slot.1 != numbers {
            slot.1 = numbers;
            if band == self.band {
                self.hop_changed();
            }
        }
        Ok(())
    }

    fn hop_channels(&self) -> Vec<f64> {
        self.hops(self.band)
            .into_iter()
            .filter_map(|c| centre_mhz(self.band, c))
            .map(|mhz| mhz * 1e6)
            .collect()
    }

    fn request_channel(&mut self) {
        if let Some(mhz) = centre_mhz(self.band, self.channel) {
            self.requests
                .push(HostRequest::retune(mhz * 1e6, "Wi-Fi channel chosen"));
        }
    }

    fn request_hop(&mut self) {
        let channels = self.hop_channels();
        if !channels.is_empty() {
            self.requests.push(HostRequest::Hop {
                channels,
                dwell_s: self.dwell_ms / 1e3,
                stop_on_lock: false,
            });
        }
    }

    fn hop_changed(&mut self) {
        if self.hop {
            self.request_hop();
        }
    }
}

impl Component for NetworkTable {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wlan.wifi_networks",
            vec![
                Port::typed::<WifiMpdu>("mpdus"),
                Port::typed::<WifiStats>("stats"),
            ],
            vec![Port::typed::<HostRequest>("requests")],
        )
        .with_settings(NetworkTableSetting::schema())
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
        self.stats = WifiStats::default();
        self.base_dsss = DsssCounters::default();
        self.base_ofdm = OfdmCounters::default();
        self.base_ppdu = PpduCounters::default();
        Ok(vec![Format::new(Kind::typed::<HostRequest>(), 0.0)])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.clock = ctx.clock.clone();
    }

    fn process(&mut self, io: &mut Io) {
        for s in io.items::<WifiStats>(1) {
            self.stats = s.value;
            self.now_s = self.now_s.max(s.value.time_s);
            if s.value.level_dbfs.is_some() {
                self.level_dbfs = s.value.level_dbfs;
            }
        }
        for m in io.items::<WifiMpdu>(0) {
            self.take(&m.value);
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
        match NetworkTableSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            NetworkTableSetting::Band => {
                let b = BAND_SETTING.read(id, v)?;
                if b != self.band {
                    self.band = b;
                    if let Some(&c) = self.hops(b).first() {
                        self.channel = c;
                        self.request_channel();
                    }
                    self.hop_changed();
                }
            }
            NetworkTableSetting::Channel => {
                let c = u32::try_from(CHANNEL_SETTING.read(id, v)?)
                    .map_err(|_| SettingError::NotPermitted { id })?;
                if !valid(self.band, c) {
                    return Err(SettingError::NotPermitted { id });
                }
                if c != self.channel {
                    self.channel = c;
                    self.request_channel();
                }
            }
            NetworkTableSetting::Hop => {
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
            NetworkTableSetting::DwellMs => {
                let d = DWELL_MS.read(id, v)?;
                if d != self.dwell_ms {
                    self.dwell_ms = d;
                    self.hop_changed();
                }
            }
            NetworkTableSetting::PscOnly => {
                let on = PSC_ONLY.read(id, v)?;
                if on != self.psc_only {
                    self.psc_only = on;
                    self.hop_changed();
                }
            }
            NetworkTableSetting::Channels24 => {
                self.set_list(id, Band::G2, CHANNELS_24.read(id, v)?)?
            }
            NetworkTableSetting::Channels5 => {
                self.set_list(id, Band::G5, CHANNELS_5.read(id, v)?)?
            }
            NetworkTableSetting::Channels6 => {
                self.set_list(id, Band::G6, CHANNELS_6.read(id, v)?)?
            }
            NetworkTableSetting::FilterText => {
                FILTER_TEXT.read(id, v)?.clone_into(&mut self.filters.text);
                self.filters.lower = self.filters.text.to_lowercase();
            }
            NetworkTableSetting::MinLevel => self.filters.min_level_dbfs = MIN_LEVEL.read(id, v)?,
            NetworkTableSetting::OpenOnly => self.filters.open_only = OPEN_ONLY.read(id, v)?,
            NetworkTableSetting::BandFilter => self.filters.band = BAND_FILTER.read(id, v)?.band(),
        }
        if self.configured {
            self.publish();
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        let shown = match self.filters.band {
            None => BandShown::All,
            Some(Band::G2) => BandShown::G2,
            Some(Band::G5) => BandShown::G5,
            Some(Band::G6) => BandShown::G6,
        };
        values_of([
            (NetworkTableSetting::Band, Value::Choice(self.band.index())),
            (
                NetworkTableSetting::Channel,
                Value::I64(i64::from(self.channel)),
            ),
            (NetworkTableSetting::Hop, Value::Bool(self.hop)),
            (NetworkTableSetting::DwellMs, Value::F64(self.dwell_ms)),
            (NetworkTableSetting::PscOnly, Value::Bool(self.psc_only)),
            (
                NetworkTableSetting::Channels24,
                Value::Text(self.lists[0].0.clone()),
            ),
            (
                NetworkTableSetting::Channels5,
                Value::Text(self.lists[1].0.clone()),
            ),
            (
                NetworkTableSetting::Channels6,
                Value::Text(self.lists[2].0.clone()),
            ),
            (
                NetworkTableSetting::FilterText,
                Value::Text(self.filters.text.clone()),
            ),
            (
                NetworkTableSetting::MinLevel,
                Value::F64(self.filters.min_level_dbfs),
            ),
            (
                NetworkTableSetting::OpenOnly,
                Value::Bool(self.filters.open_only),
            ),
            (
                NetworkTableSetting::BandFilter,
                Value::Choice(shown.index()),
            ),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }

    fn commands(&self) -> &Commands {
        NetworkTableCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match NetworkTableCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            NetworkTableCommand::Clear => self.clear(),
            NetworkTableCommand::Remove => objcmd::remove(
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

    /// The counters, the rates and the measurements start again; the networks, the bands and the frame log stay if
    /// `kept` holds them.
    fn forget(&mut self, kept: &Kept) {
        self.counters = Counters::default();
        self.base_dsss = self.stats.dsss;
        self.base_ofdm = self.stats.ofdm;
        self.base_ppdu = self.stats.ppdu;
        self.level_dbfs = None;
        self.published.clear_rows(WIFI_OFDM_RATES.def());
        if !kept.table(WIFI_NETWORKS.key) {
            self.store.networks.clear();
        }
        if !kept.table(WIFI_BEACONS.key) {
            self.store.log.clear();
        }
        if !kept.table(WIFI_BANDS.key) {
            self.published.clear_rows(WIFI_BANDS.def());
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
