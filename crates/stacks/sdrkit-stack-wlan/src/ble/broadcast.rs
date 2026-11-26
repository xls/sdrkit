//! The LE Audio broadcast sources of `ble` (the `broadcasts` table): what a source's extended and periodic advertising say
//! about its stream, read by [`sdrkit_proto_wlan_phy::ble::audio`]. The extended advertising gives the Broadcast_ID, the
//! name and the public announcement; the periodic train gives the BIGInfo (in its ACAD) and the BASE. An encrypted BIG
//! is reported as encrypted; the audio on its BIS is not received.

use crate::typed::BleBroadcastRow;
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_proto_wlan_phy::ble::audio::{Announcement, BigInfo, LC3};
use sdrkit_proto_wlan_phy::ble::Addr;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{RowOut, Shaped};
use sdrkit_util_objects::{ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};

/// The most broadcast sources kept.
pub const BROADCAST_CAP: usize = 64;

/// A broadcast source as the table keeps it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Broadcast {
    /// The source's address text (the row key).
    pub source: String,
    /// Broadcast_ID as six hex digits.
    pub broadcast_id: String,
    /// Broadcast_Name.
    pub name: String,
    /// Encrypted, from the BIGInfo or the public announcement.
    pub encrypted: Option<bool>,
    /// The BIS of the BIG (BIGInfo), or of the BASE.
    pub bis: Option<u8>,
    /// The subgroups of the BASE.
    pub subgroups: u8,
    /// The codec name ("LC3", or the coding format in hex).
    pub codec: String,
    /// The sampling frequency, Hz.
    pub sampling_hz: Option<u32>,
    /// The frame duration, µs.
    pub frame_us: Option<u32>,
    /// The octets of a codec frame.
    pub octets: Option<u16>,
    /// The audio channels: the locations of the BIS, or the BIS.
    pub channels: Option<u8>,
    /// The presentation delay, µs.
    pub presentation_delay_us: Option<u32>,
    /// The language (ISO 639-3).
    pub language: String,
    /// The program information.
    pub program: String,
    /// The quality the public announcement names.
    pub quality: String,
    /// The ISO interval, units of 1.25 ms.
    pub iso_interval: Option<u16>,
    /// The PHY of the BIS.
    pub phy: String,
}

impl Shift for Broadcast {
    fn shift_times(&mut self, _by_s: f64) {}
}

fn known(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

impl Shaped for Broadcast {
    type Row = BleBroadcastRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let bitrate = match (self.octets, self.frame_us) {
            (Some(o), Some(f)) if f > 0 => Some(f64::from(o) * 8.0 / f64::from(f) * 1e3),
            _ => None,
        };
        out.put(&BleBroadcastRow {
            source: &self.source,
            broadcast_id: known(&self.broadcast_id),
            name: known(&self.name),
            encrypted: self.encrypted,
            bis: self.bis.map(i64::from),
            subgroups: i64::from(self.subgroups),
            codec: known(&self.codec),
            sampling_hz: self.sampling_hz.map(|h| Hertz(f64::from(h))),
            frame_ms: self.frame_us.map(|f| f64::from(f) / 1e3),
            octets_per_frame: self.octets.map(i64::from),
            bitrate_kbps: bitrate,
            channels: self.channels.map(i64::from),
            presentation_delay_ms: self.presentation_delay_us.map(|d| f64::from(d) / 1e3),
            language: known(&self.language),
            program: known(&self.program),
            quality: known(&self.quality),
            iso_interval_ms: self.iso_interval.map(|i| f64::from(i) * 1.25),
            phy: known(&self.phy),
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// Takes the ACAD and the AdvData of an extended or periodic packet of `source`, stamped `at`: a row for a source that
/// announces an LE Audio broadcast, or what the packet adds to its row.
pub fn take(rows: &mut ObjectTable<String, Broadcast>, source: &Addr, acad: &[u8], data: &[u8], at: Stamp) {
    let big = BigInfo::in_acad(acad);
    let a = Announcement::parse(data);
    let any = big.is_some()
        || a.broadcast_id.is_some()
        || a.base.is_some()
        || a.public_features.is_some()
        || (a.name.is_some() && rows.get_mut(source.text().as_str()).is_some());
    if !any {
        return;
    }
    let key = source.text();
    let Ok(up) = rows.upsert(key.as_str(), at, || Broadcast {
        source: key.clone(),
        ..Broadcast::default()
    }) else {
        return;
    };
    let r = up.row;
    if let Some(id) = a.broadcast_id {
        r.broadcast_id = format!("{id:06X}");
    }
    if let Some(n) = a.name {
        r.name = n;
    }
    if let Some(f) = a.public_features {
        // The BIGInfo, once heard, is the stronger word on encryption.
        if big.is_none() && r.iso_interval.is_none() {
            r.encrypted = Some(f & 1 != 0);
        }
        r.quality = match (f & 2 != 0, f & 4 != 0) {
            (true, true) => "standard, high",
            (true, false) => "standard",
            (false, true) => "high",
            _ => "",
        }
        .into();
        if let Some(m) = a.public_metadata {
            if let Some(p) = m.program {
                r.program = p;
            }
        }
    }
    if let Some(b) = big {
        r.encrypted = Some(b.encrypted);
        r.bis = Some(b.num_bis);
        r.iso_interval = Some(b.iso_interval);
        r.phy = match b.phy {
            0 => "LE 1M",
            1 => "LE 2M",
            2 => "LE Coded",
            _ => "",
        }
        .into();
    }
    if let Some(base) = a.base {
        r.presentation_delay_us = Some(base.presentation_delay_us);
        r.subgroups = base.subgroups.len() as u8;
        if let Some(g) = base.subgroups.first() {
            r.codec = if g.coding_format == LC3 {
                "LC3".into()
            } else {
                format!("coding format {:#04x}, company {:#06x}", g.coding_format, g.company)
            };
            let c = g.bis.first().map_or(g.config, |b| b.1);
            r.sampling_hz = c.sampling_hz;
            r.frame_us = c.frame_us;
            r.octets = c.octets;
            if let Some(l) = &g.metadata.language {
                r.language.clone_from(l);
            }
            if let Some(p) = &g.metadata.program {
                r.program.clone_from(p);
            }
        }
        let all: Vec<_> = base.subgroups.iter().flat_map(|g| g.bis.iter()).collect();
        let located: u32 = all.iter().map(|b| b.1.allocation.unwrap_or(0).count_ones()).sum();
        r.channels = Some(if located > 0 { located as u8 } else { all.len() as u8 });
        if r.bis.is_none() {
            r.bis = Some(all.len() as u8);
        }
    }
}
