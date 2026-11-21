//! The channel plan of the mesh receiver: which LoRa settings are searched, for which protocols, and how they map
//! onto channels and detectors. Every frequency is a setting of the host; a protocol without one listens at the dial.

use super::types::{Protocol, TxChannel};
use crate::proto::{hamlora, lorawan, meshcore, meshtastic};
use sdrkit_proto_lora_phy::bank::{BankChannel, BankPlan};
use sdrkit_proto_lora_phy::channel;
use sdrkit_proto_lora_phy::params::{fmt_khz, Ldro, LoraParams};
use sdrkit_proto_lora_phy::rx::DetectorConfig;
use sdrkit_proto_meshtastic::{ChannelNumTooHigh, SlotChoice};
pub use sdrkit_proto_meshtastic::{SlotGrid, SlotRow, SLOTS_CAP};
use serde::{Deserialize, Serialize};

/// The protocols that a stack searches (bit mask): "mesh", "meshtastic", "meshcore", "loraaprs", "meshcom", "lorawan".
pub fn submode_mask(sub: &str) -> u32 {
    match sub {
        "meshtastic" => 1,
        "meshcore" => 2,
        "loraaprs" => 4,
        "meshcom" => 8,
        "lorawan" => 16,
        _ => 15,
    }
}

/// A channel of the host's list: one network of MeshCore, LoRa APRS or MeshCom.
#[derive(Clone, Debug, PartialEq)]
pub struct ListChannel {
    /// Its name ("EU/UK (Narrow)").
    pub label: String,
    /// The protocol of its frames.
    pub protocol: Protocol,
    /// Centre, Hz.
    pub freq_hz: f64,
    /// The setting.
    pub params: LoraParams,
}

/// The most entries of a channel list.
pub const MAX_LIST: usize = 256;

/// The setting of `protocol` with `sf`, `bw`, `cr` and the preamble and sync word of the protocol.
fn protocol_params(protocol: Protocol, sf: u8, bw: f64, cr: u8) -> LoraParams {
    match protocol {
        Protocol::MeshCore => meshcore::params(sf, bw, cr),
        Protocol::MeshCom => {
            let d = hamlora::meshcom_params();
            hamlora::params(sf, bw, cr, d.preamble, d.sync)
        }
        _ => {
            let d = hamlora::lora_aprs_params();
            hamlora::params(sf, bw, cr, d.preamble, d.sync)
        }
    }
}

/// A sync word as "0x12" or "18".
fn parse_sync(t: &str) -> Option<u8> {
    match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(h) => u8::from_str_radix(h, 16).ok(),
        None => t.parse().ok(),
    }
}

/// One entry of the list: `label=protocol/Hz/bandwidth Hz/SF/CR[/preamble[/sync]]`.
fn parse_entry(e: &str) -> Option<ListChannel> {
    let (label, v) = e.rsplit_once('=')?;
    let label = label.trim();
    let mut f = v.split('/').map(str::trim);
    let protocol = match f.next()? {
        "meshcore" => Protocol::MeshCore,
        "loraaprs" => Protocol::LoraAprs,
        "meshcom" => Protocol::MeshCom,
        _ => return None,
    };
    let hz: f64 = f.next()?.parse().ok()?;
    let bw: f64 = f.next()?.parse().ok()?;
    let sf: u8 = f.next()?.parse().ok()?;
    let cr: u8 = f.next()?.parse().ok()?;
    if label.is_empty()
        || !(hz.is_finite() && hz > 0.0 && bw.is_finite() && bw > 0.0)
        || !(5..=12).contains(&sf)
        || !(5..=8).contains(&cr)
    {
        return None;
    }
    let mut params = protocol_params(protocol, sf, bw, cr);
    if let Some(t) = f.next() {
        params.preamble = t.parse().ok().filter(|&n: &u16| n >= 6)?;
    }
    if let Some(t) = f.next() {
        params.sync = parse_sync(t)?;
    }
    if f.next().is_some() {
        return None;
    }
    Some(ListChannel {
        label: label.into(),
        protocol,
        freq_hz: hz.round(),
        params,
    })
}

/// The channels of the text of the setting `channels`. The entries are separated by ";"; each is a label, "=", the protocol
/// (`meshcore`, `loraaprs` or `meshcom`), and "/" before each of the frequency in Hz, the bandwidth in Hz, the spreading
/// factor, the coding rate (5 to 8) and, when they differ from the protocol's, the preamble and the sync word. `None` when an
/// entry does not parse or there are more than [`MAX_LIST`].
pub fn parse_channels(text: &str) -> Option<Vec<ListChannel>> {
    let v = text
        .split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(parse_entry)
        .collect::<Option<Vec<_>>>()?;
    (v.len() <= MAX_LIST).then_some(v)
}

/// The custom LoRa setting.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomSetting {
    /// Frequency, Hz (0 = the dial).
    pub freq_hz: f64,
    /// Bandwidth, Hz.
    pub bw: f64,
    /// SF.
    pub sf: u8,
    /// CR.
    pub cr: u8,
    /// Sync word.
    pub sync: u8,
    /// Preamble.
    pub preamble: u16,
}

impl CustomSetting {
    /// The starting values of a stack, at the dial.
    pub fn for_submode(sub: &str) -> Self {
        let (bw, sf, cr, sync, pre) = match sub {
            "meshcore" => (62.5e3, 8, 8, 0x12, 32),
            "loraaprs" => (125e3, 12, 5, 0x12, 8),
            "meshcom" => (250e3, 11, 6, 0x2B, 8),
            "lorawan" => (125e3, 7, 5, 0x34, 8),
            _ => (250e3, 11, 5, 0x2B, 16),
        };
        CustomSetting {
            freq_hz: 0.0,
            bw,
            sf,
            cr,
            sync,
            preamble: pre,
        }
    }
}

/// Everything the plan is built from.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshConfig {
    /// The stack.
    pub submode: String,
    /// Protocols (bit mask, 3.1).
    pub mask: u32,
    /// Every other Meshtastic preset of the band too.
    pub preset_search: bool,
    /// The frequency of the stack's primary channel, Hz (0: the tuned frequency).
    pub dial_hz: f64,
    /// The band of the Meshtastic slots. With both edges 0 each preset lies at the dial.
    pub mt_band: meshtastic::SlotBand,
    /// Primary Meshtastic preset.
    pub mt_preset: String,
    /// The primary channel's own name ("" = the preset's).
    pub mt_channel_name: String,
    /// Slot override (1-based, 0 automatic).
    pub mt_channel_num: u32,
    /// Frequency override, Hz (0 none).
    pub mt_override_hz: f64,
    /// Frequency offset, Hz.
    pub mt_offset_hz: f64,
    /// The channels of MeshCore, LoRa APRS and MeshCom. A protocol without one listens at the dial.
    pub channels: Vec<ListChannel>,
    /// The LoRaWAN channels. Without one, LoRaWAN listens at the dial.
    pub lw_channels: Vec<lorawan::PlanChannel>,
    /// The custom setting replaces the plan.
    pub custom: bool,
    /// The custom setting.
    pub custom_setting: CustomSetting,
    /// The lost-sample search.
    pub jump_search: bool,
    /// Its expected jump, chips.
    pub jump_chips: f64,
}

impl MeshConfig {
    /// The defaults of a stack.
    pub fn for_submode(sub: &str) -> Self {
        MeshConfig {
            submode: sub.into(),
            mask: submode_mask(sub),
            preset_search: false,
            dial_hz: 0.0,
            mt_band: meshtastic::SlotBand::default(),
            mt_preset: "LongFast".into(),
            mt_channel_name: String::new(),
            mt_channel_num: 0,
            mt_override_hz: 0.0,
            mt_offset_hz: 0.0,
            channels: Vec::new(),
            lw_channels: Vec::new(),
            custom: false,
            custom_setting: CustomSetting::for_submode(sub),
            jump_search: true,
            jump_chips: 11.7,
        }
    }
    /// The dial the stack names itself: the one set; else, for a Meshtastic stack with a band, its primary slot.
    pub fn own_dial(&self) -> Option<f64> {
        if self.dial_hz > 0.0 {
            return Some(self.dial_hz);
        }
        let banded = self.mt_band.start_hz > 0.0 || self.mt_band.end_hz > 0.0;
        if banded && matches!(self.submode.as_str(), "mesh" | "meshtastic") {
            return self.mt_slot_hz(0.0);
        }
        None
    }
    /// The dial for a session tuned to `tuned_hz`: the stack's own, else the tuned frequency, else 0.
    pub fn dial(&self, tuned_hz: Option<f64>) -> f64 {
        self.own_dial().or(tuned_hz).unwrap_or(0.0)
    }
    /// The primary Meshtastic preset.
    pub fn primary(&self) -> &'static meshtastic::Preset {
        meshtastic::preset(&self.mt_preset).unwrap_or(&meshtastic::PRESETS[0])
    }
    /// The band of the slots of preset `p` at the dial `dial`: the band set, else one channel of `p` centred on the dial.
    pub fn slot_band(&self, p: &meshtastic::Preset, dial: f64) -> meshtastic::SlotBand {
        if self.mt_band.start_hz > 0.0 || self.mt_band.end_hz > 0.0 {
            return self.mt_band;
        }
        let at = meshtastic::SlotBand {
            start_hz: dial,
            end_hz: dial,
        };
        let half = meshtastic::preset_bw(p, &at) / 2.0;
        meshtastic::SlotBand {
            start_hz: dial - half,
            end_hz: dial + half,
        }
    }
    /// The slots of the primary Meshtastic preset at the dial `dial`, in the band set or one channel at the dial; `None`
    /// when the stack does not listen to Meshtastic or the custom setting replaces the plan.
    pub fn slot_grid(&self, dial: f64) -> Option<SlotGrid> {
        if self.custom || self.mask & 1 == 0 {
            return None;
        }
        let p = self.primary();
        let band = self.slot_band(p, dial);
        Some(SlotGrid::new(
            p,
            &band,
            &SlotChoice {
                channel_name: &self.mt_channel_name,
                channel_num: self.mt_channel_num,
                offset_hz: self.mt_offset_hz,
                override_hz: self.mt_override_hz,
            },
        ))
    }
    /// Checks that `channel_num` names a slot of the primary preset in the band set; any number passes without a band.
    ///
    /// # Errors
    ///
    /// [`ChannelNumTooHigh`] for a number above the slots of the band.
    pub fn check_channel_num(&self, channel_num: u32) -> Result<(), ChannelNumTooHigh> {
        let banded = self.mt_band.start_hz > 0.0 || self.mt_band.end_hz > 0.0;
        if !banded {
            return Ok(());
        }
        meshtastic::check_channel_num(self.primary(), &self.mt_band, channel_num)
    }
    /// The primary Meshtastic slot at the dial `dial`, Hz.
    pub fn mt_slot_hz(&self, dial: f64) -> Option<f64> {
        let p = self.primary();
        meshtastic::slot_hz(
            p,
            &self.slot_band(p, dial),
            &self.mt_channel_name,
            self.mt_channel_num,
            self.mt_offset_hz,
            self.mt_override_hz,
        )
    }
    /// The listed channels of `protocol`, else one at the dial `dial` with the protocol's setting.
    pub fn channels_of(&self, protocol: Protocol, dial: f64) -> Vec<ListChannel> {
        let v: Vec<ListChannel> = self
            .channels
            .iter()
            .filter(|c| c.protocol == protocol)
            .cloned()
            .collect();
        if !v.is_empty() {
            return v;
        }
        let (label, params) = match protocol {
            Protocol::MeshCore => ("MeshCore", meshcore::params(8, 62.5e3, 8)),
            Protocol::MeshCom => ("MeshCom", hamlora::meshcom_params()),
            _ => ("LoRa APRS", hamlora::lora_aprs_params()),
        };
        vec![ListChannel {
            label: label.into(),
            protocol,
            freq_hz: dial.round(),
            params,
        }]
    }
    /// The LoRaWAN channels, else the one at the dial `dial`.
    pub fn lorawan_channels(&self, dial: f64) -> Vec<lorawan::PlanChannel> {
        if self.lw_channels.is_empty() {
            vec![lorawan::default_channel(dial.round())]
        } else {
            self.lw_channels.clone()
        }
    }
}

/// One LoRa setting of the plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanEntry {
    /// The protocols its frames are offered to.
    pub mask: u32,
    /// Its name ("LongFast", "LongFast + MeshCom 868", "EU/UK (Narrow)").
    pub name: String,
    /// Centre, Hz.
    pub freq_hz: f64,
    /// The setting.
    pub params: LoraParams,
    /// Its channel fits the capture.
    pub in_band: bool,
    /// The detector that serves it (an index into [`Plan::detectors`]).
    pub detector: usize,
}

impl PlanEntry {
    /// The protocol that names it (the first of the mask).
    pub fn protocol(&self) -> Protocol {
        Protocol::of_mask(self.mask)
            .next()
            .unwrap_or(Protocol::Meshtastic)
    }
}

/// The plan: the settings, their detectors and the bank that runs them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    /// The settings.
    pub entries: Vec<PlanEntry>,
    /// Per detector (its tag), the entries it serves.
    pub detectors: Vec<Vec<usize>>,
    /// The bank.
    pub bank: BankPlan,
    /// "Mesh: listening to ...".
    pub line: String,
    /// The dial, in words ("869.525 MHz").
    #[serde(default)]
    pub name: String,
    /// The dial, Hz.
    #[serde(default)]
    pub dial_hz: f64,
    /// The protocols searched (bit mask).
    #[serde(default)]
    pub mask: u32,
    /// The custom LoRa setting replaces the channels.
    #[serde(default)]
    pub custom: bool,
    /// The frequency slots of the primary Meshtastic preset; `None` without Meshtastic or with the custom setting.
    #[serde(default)]
    pub slots: Option<SlotGrid>,
}

impl Plan {
    /// The channel a client of the protocol with mask bit `bit` sends on, or `None` if the session listens to no channel of
    /// that protocol. Meshtastic sends on its primary channel only (the first entry of the protocol); another protocol
    /// sends on its first channel inside the capture.
    pub fn tx_channel(&self, bit: u32) -> Option<TxChannel> {
        let mut of = self.entries.iter().filter(|e| e.mask & bit != 0);
        let e = if bit == 1 {
            of.next().filter(|e| e.in_band)
        } else {
            of.find(|e| e.in_band)
        }?;
        Some(TxChannel {
            params: e.params.clone(),
            freq_hz: e.freq_hz,
        })
    }
    /// The session listens to a channel centred on `freq_hz`.
    pub fn listens_to(&self, freq_hz: f64) -> bool {
        self.entries
            .iter()
            .any(|e| e.in_band && (e.freq_hz - freq_hz).abs() <= 1.0)
    }
    /// The index of a frame's entry: the detector's entry whose sync word matches, else its first entry.
    pub fn entry_index(&self, tag: u32, sync: u8) -> Option<usize> {
        let list = self.detectors.get(tag as usize)?;
        list.iter()
            .copied()
            .find(|&i| self.entries.get(i).is_some_and(|e| e.params.sync == sync))
            .or_else(|| list.first().copied())
            .filter(|&i| i < self.entries.len())
    }
    /// The entry of a frame: the detector's entry whose sync word matches.
    pub fn entry_of(&self, tag: u32, sync: u8) -> Option<&PlanEntry> {
        let list = self.detectors.get(tag as usize)?;
        list.iter()
            .map(|&i| &self.entries[i])
            .find(|e| e.params.sync == sync)
            .or_else(|| list.first().map(|&i| &self.entries[i]))
    }
}

fn push(v: &mut Vec<PlanEntry>, mask: u32, name: &str, freq: f64, params: LoraParams) {
    let freq = freq.round();
    // Shared settings: the same frequency, SF, BW and sync word make one entry.
    if let Some(e) = v.iter_mut().find(|e| {
        (e.freq_hz - freq).abs() <= 1.0
            && e.params.sf == params.sf
            && e.params.bw == params.bw
            && e.params.sync == params.sync
    }) {
        e.mask |= mask;
        if !e.name.split(" + ").any(|n| n == name) {
            e.name = format!("{} + {name}", e.name);
        }
        return;
    }
    v.push(PlanEntry {
        mask,
        name: name.into(),
        freq_hz: freq,
        params,
        in_band: false,
        detector: 0,
    });
}

/// The protocols that use a sync word.
pub fn protocols_of_sync(sync: u8) -> u32 {
    match sync {
        0x2B => 1 | 8,
        0x12 => 2 | 4,
        0x34 => 16,
        _ => 31,
    }
}

#[allow(clippy::needless_range_loop)]
/// Builds the plan for a session tuned to `tuned_hz` and a capture at `rate` centred on `centre_hz`.
pub fn build(cfg: &MeshConfig, tuned_hz: Option<f64>, centre_hz: f64, rate: f64) -> Plan {
    let mut v: Vec<PlanEntry> = Vec::new();
    let dial = cfg.dial(tuned_hz);
    let fits = |f: f64, bw: f64| channel::in_band(rate, f - centre_hz, bw);
    if cfg.custom {
        let c = &cfg.custom_setting;
        let freq = if c.freq_hz > 0.0 { c.freq_hz } else { dial };
        let mut mask = protocols_of_sync(c.sync) & cfg.mask;
        if mask == 0 {
            mask = protocols_of_sync(c.sync);
        }
        let mut p = LoraParams::new(c.sf, c.bw, c.cr, c.preamble.max(6), c.sync);
        p.ldro = Ldro::Auto;
        push(&mut v, mask, "Custom", freq, p);
    } else {
        if cfg.mask & 1 != 0 {
            let primary = cfg.primary();
            let mut list = vec![primary];
            if cfg.preset_search {
                list.extend(
                    meshtastic::PRESETS
                        .iter()
                        .filter(|p| p.name != primary.name),
                );
            }
            for p in list {
                let band = cfg.slot_band(p, dial);
                let f = if p.name == primary.name {
                    cfg.mt_slot_hz(dial)
                } else {
                    meshtastic::slot_hz(p, &band, "", 0, 0.0, 0.0)
                };
                if let Some(f) = f {
                    push(&mut v, 1, p.name, f, meshtastic::lora_params(p, &band));
                }
            }
        }
        for protocol in [Protocol::MeshCore, Protocol::LoraAprs, Protocol::MeshCom] {
            if cfg.mask & protocol.bit() != 0 {
                for c in cfg.channels_of(protocol, dial) {
                    push(&mut v, protocol.bit(), &c.label, c.freq_hz, c.params);
                }
            }
        }
        if cfg.mask & 16 != 0 {
            for c in cfg.lorawan_channels(dial) {
                if !c.uplink || !fits(c.hz, c.bw) {
                    continue;
                }
                for sf in c.sf_min..=c.sf_max {
                    let p = LoraParams::new(sf, c.bw, 5, 8, 0x34);
                    let name = format!("LoRaWAN {} {}", fmt_mhz(c.hz), fmt_khz(c.bw));
                    push(&mut v, 16, &name, c.hz, p);
                }
            }
        }
    }
    // Detectors: one per channel (frequency, bandwidth) and SF, accepting every sync word of its entries.
    let mut chans: Vec<BankChannel> = Vec::new();
    let mut detectors: Vec<Vec<usize>> = Vec::new();
    for i in 0..v.len() {
        let e = &v[i];
        let inb = fits(e.freq_hz, e.params.bw);
        v[i].in_band = inb;
        let e = &v[i];
        let ci = match chans
            .iter()
            .position(|c| (c.freq_hz - e.freq_hz).abs() <= 1.0 && c.bw == e.params.bw)
        {
            Some(ci) => ci,
            None => {
                chans.push(BankChannel {
                    freq_hz: e.freq_hz,
                    bw: e.params.bw,
                    detectors: Vec::new(),
                });
                chans.len() - 1
            }
        };
        let ch = &mut chans[ci];
        let di = match ch.detectors.iter().position(|d| d.sf == e.params.sf) {
            Some(k) => {
                let d = &mut ch.detectors[k];
                if !d.syncs.contains(&e.params.sync) {
                    d.syncs.push(e.params.sync);
                }
                d.tag as usize
            }
            None => {
                let mut d = DetectorConfig::new(e.params.sf, e.params.bw, e.params.sync);
                d.preamble = e.params.preamble;
                d.ldro = e.params.ldro;
                d.jump_search = cfg.jump_search;
                d.jump_chips = cfg.jump_chips;
                d.tag = detectors.len() as u32;
                detectors.push(Vec::new());
                ch.detectors.push(d);
                detectors.len() - 1
            }
        };
        detectors[di].push(i);
        v[i].detector = di;
    }
    let line = plan_line(&v);
    Plan {
        entries: v,
        detectors,
        bank: BankPlan {
            channels: chans,
            fft_from: 3,
        },
        line,
        name: String::new(),
        dial_hz: dial,
        mask: cfg.mask,
        custom: cfg.custom,
        slots: cfg.slot_grid(dial),
    }
}

/// "868.1" style.
pub fn fmt_mhz(hz: f64) -> String {
    let s = format!("{:.6}", hz / 1e6);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

fn plan_line(v: &[PlanEntry]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut lw: Vec<(f64, f64, u8, u8)> = Vec::new();
    for e in v.iter().filter(|e| e.in_band) {
        if e.mask == 16 {
            match lw
                .iter_mut()
                .find(|c| c.0 == e.freq_hz && c.1 == e.params.bw)
            {
                Some(c) => {
                    c.2 = c.2.min(e.params.sf);
                    c.3 = c.3.max(e.params.sf);
                }
                None => lw.push((e.freq_hz, e.params.bw, e.params.sf, e.params.sf)),
            }
        } else {
            parts.push(format!(
                "{} {} MHz SF{}",
                e.name,
                fmt_mhz(e.freq_hz),
                e.params.sf
            ));
        }
    }
    for (f, bw, lo, hi) in lw {
        let sfs = if lo == hi {
            format!("SF{lo}")
        } else {
            format!("SF{lo}-{hi}")
        };
        parts.push(format!(
            "LoRaWAN {} MHz {} kHz {sfs}",
            fmt_mhz(f),
            fmt_khz(bw)
        ));
    }
    if parts.is_empty() {
        "Mesh: listening to nothing (no channel in the captured band)".into()
    } else {
        format!("Mesh: listening to {}", parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_custom_setting_replaces_the_plan() {
        let mut cfg = MeshConfig::for_submode("loraaprs");
        cfg.custom = true;
        let p = build(&cfg, Some(100e6), 100e6 + 300e3, 2e6);
        assert_eq!(p.entries.len(), 1);
        assert_eq!(p.entries[0].mask, 4);
        assert_eq!(p.entries[0].params.sf, 12);
        assert_eq!(p.entries[0].freq_hz, 100e6);
    }

    #[test]
    fn without_channels_each_protocol_listens_at_the_dial() {
        let cfg = MeshConfig::for_submode("mesh");
        let p = build(&cfg, Some(100e6), 100e6 + 300e3, 2e6);
        assert!(p.entries.iter().all(|e| e.freq_hz == 100e6 && e.in_band));
        // MeshCom shares LongFast's setting; MeshCore and LoRa APRS have their own.
        assert_eq!(p.entries.len(), 3);
        assert_eq!(p.entries[0].mask, 9);
        assert_eq!(p.dial_hz, 100e6);
    }

    #[test]
    fn the_channel_list_parses() {
        let v = parse_channels(
            "EU/UK (Narrow)=meshcore/869618000/62500/8/8; X=meshcom/433175000/250000/11/6/32/0x2B",
        )
        .unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].label, "EU/UK (Narrow)");
        assert_eq!((v[0].params.preamble, v[0].params.sync), (32, 0x12));
        assert_eq!(
            (v[1].protocol, v[1].params.preamble),
            (Protocol::MeshCom, 32)
        );
        assert!(parse_channels("").unwrap().is_empty());
        assert!(parse_channels("=meshcore/1/1/8/8").is_none());
        assert!(parse_channels("a=lorawan/1/1/8/8").is_none());
        assert!(parse_channels("a=meshcore/1/1/13/8").is_none());
    }

    #[test]
    fn a_band_without_a_dial_dials_the_primary_slot() {
        let mut cfg = MeshConfig::for_submode("meshtastic");
        cfg.mt_band = meshtastic::SlotBand {
            start_hz: 100e6,
            end_hz: 102e6,
        };
        let slot = cfg.mt_slot_hz(0.0).unwrap();
        assert!(slot > 100e6 && slot < 102e6);
        assert_eq!(cfg.own_dial(), Some(slot));
        cfg.dial_hz = 101e6;
        assert_eq!(cfg.own_dial(), Some(101e6));
        assert_eq!(MeshConfig::for_submode("meshcore").own_dial(), None);
    }
}
