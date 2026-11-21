//! Reading VDL4 bursts: the burst header, the synchronisation burst's CPR position (global from an even and an odd report
//! within 10 s, else local to the receiving station), and DLS.
//! Standard: ICAO Doc 9816 Part 2 clause 1.4 (synchronisation burst), ETSI EN 302 842-2.

use super::Burst;
use crate::messages::{Located, Origin};
use crate::report::hex;
use sdrkit_core::{ConfigError, Values};
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, RunContext, Timed,
};
use sdrkit_util_bits::{BitReader, BitWriter};
use sdrkit_util_geo::cpr::{self, CprFormat, CprFrame};
use sdrkit_util_geo::Position;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A read burst.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Signal time.
    pub time_s: f64,
    /// Channel.
    pub freq_hz: f64,
    /// Level.
    pub level_db: f32,
    /// SNR.
    pub snr_db: f32,
    /// Offset.
    pub offset_hz: f32,
    /// Source address.
    pub addr: u32,
    /// Source type.
    pub addr_type: u8,
    /// Autonomous (a/d bit 0).
    pub autonomous: bool,
    /// Version.
    pub version: u8,
    /// Burst id.
    pub burst_id: u8,
    /// Message id.
    pub msg_id: u8,
    /// Its name.
    pub msg_name: String,
    /// Ground station (type 4 or 5).
    pub uplink: bool,
    /// Aircraft (type 1).
    pub downlink: bool,
    /// Summary.
    pub summary: String,
    /// Detail lines.
    pub detail: Vec<String>,
    /// Position.
    pub position: Option<Located>,
    /// DLS information in hex.
    pub dls_hex: String,
    /// DLS printable text.
    pub dls_text: String,
}

impl Item for Message {
    const KIND: &'static str = "vdl4.message";
}

/// The name of an address type.
pub fn type_name(t: u8) -> &'static str {
    match t {
        0 => "unknown",
        1 => "aircraft",
        4 | 5 => "ground station",
        7 => "all stations",
        _ => "reserved",
    }
}

/// The name of a message id.
pub fn msg_name(id: u8) -> &'static str {
    match id {
        1 => "SYNC",
        3 => "DLS",
        7 => "UCTRL",
        8 => "GEN_REQ",
        9 => "GEN_CONF",
        _ => "VSS",
    }
}

/// The header fields: (autonomous, version, type, address, burst id, message id). Information from octet 6.
pub fn read_header(f: &[u8]) -> Option<(bool, u8, u8, u32, u8, u8)> {
    if f.len() < 6 {
        return None;
    }
    // The fields are sent least significant bit first (EN 302 842-2 clause 6.4).
    let mut b = BitReader::new_lsb(f);
    let ad = b.u(1);
    let _res = b.u(1);
    let ver = b.u(3) as u8;
    let typ = b.u(3) as u8;
    let addr = b.u(24) as u32;
    let bid = b.u(8) as u8;
    let mid = b.u(5) as u8;
    Some((ad == 0, ver, typ, addr, bid, mid))
}

/// Builds a header and information (for the generator).
pub fn write(
    autonomous: bool,
    typ: u8,
    addr: u32,
    burst_id: u8,
    msg_id: u8,
    info: &[u8],
) -> Vec<u8> {
    let mut v = Vec::with_capacity(6 + info.len());
    {
        let mut w = BitWriter::new_lsb(&mut v);
        w.put(!autonomous as u64, 1);
        w.put(0, 1);
        w.put(1, 3);
        w.put(typ as u64, 3);
        w.put(addr as u64, 24);
        w.put(burst_id as u64, 8);
        w.put(msg_id as u64, 5);
        w.put(0, 3);
    }
    v.extend_from_slice(info);
    v
}

/// The synchronisation burst information (8 octets) for a CPR report and an altitude (25 ft units).
pub fn sync_info(yz: u32, xz: u32, odd: bool, alt_ft: f64) -> Vec<u8> {
    let alt = ((alt_ft / 25.0).round() as u32).min(0xFFF);
    vec![
        (odd as u8) << 3,
        yz as u8,
        ((yz >> 8) & 0x0F) as u8 | (((alt >> 8) & 0x0F) as u8) << 4,
        alt as u8,
        xz as u8,
        ((xz >> 8) & 0x3F) as u8,
        0,
        0,
    ]
}

/// The last even and odd CPR report of an address: (time, latitude code, longitude code).
type CprPair = [Option<(f64, u32, u32)>; 2];

/// Reads the bursts.
#[derive(Default)]
pub struct Reader {
    /// The receiving station's position, for local decoding.
    pub station: Option<Position>,
    last: HashMap<u32, CprPair>,
    out: Vec<Message>,
}

impl Reader {
    /// Reads one burst.
    pub fn read(&mut self, b: &Burst) -> Option<Message> {
        let (auto, ver, typ, addr, bid, mid) = read_header(&b.frame)?;
        let info = &b.frame[6..];
        let name = msg_name(mid);
        let mut m = Message {
            time_s: b.time_s,
            freq_hz: b.freq_hz,
            level_db: b.level_db,
            snr_db: b.snr_db,
            offset_hz: b.offset_hz,
            addr,
            addr_type: typ,
            autonomous: auto,
            version: ver,
            burst_id: bid,
            msg_id: mid,
            msg_name: name.into(),
            uplink: typ == 4 || typ == 5,
            downlink: typ == 1,
            ..Default::default()
        };
        m.detail.push(format!(
            "VDL4 burst: {addr:07X} ({}), {}, msg {name} (id {mid}), burst id {bid}",
            type_name(typ),
            if auto { "autonomous" } else { "directed" }
        ));
        match mid {
            1 if info.len() >= 8 => {
                let odd = (info[0] >> 3) & 1 != 0;
                let yz = info[1] as u32 | ((info[2] & 0x0F) as u32) << 8;
                let alt = (((info[2] >> 4) as u32) << 8 | info[3] as u32) as f64 * 25.0;
                let xz =
                    info[4] as u32 | ((info[5] & 0x3F) as u32) << 8 | ((info[6] & 3) as u32) << 14;
                let xz = xz & 0x3FFF;
                m.summary = format!("SYNC {addr:06X}");
                m.detail.push(format!(
                    "  CPR {} lat code {yz}, lon code {xz}, altitude {alt:.0} ft",
                    if odd { "odd" } else { "even" }
                ));
                let e = self.last.entry(addr).or_insert([None, None]);
                e[odd as usize] = Some((b.time_s, yz, xz));
                let frame = |o: bool, y: u32, x: u32| CprFrame {
                    odd: o,
                    lat: y,
                    lon: x,
                };
                let fmt = CprFormat::VDL4;
                let pos = match (e[0], e[1]) {
                    (Some(ev), Some(od)) if (ev.0 - od.0).abs() <= 10.0 => cpr::decode_global(
                        &fmt,
                        &frame(false, ev.1, ev.2),
                        &frame(true, od.1, od.2),
                        odd,
                        None,
                    )
                    .ok(),
                    _ => self
                        .station
                        .and_then(|s| cpr::decode_local(&fmt, &frame(odd, yz, xz), &s).ok()),
                };
                if let Some(p) = pos {
                    let (lat, lon) = (p.lat_deg, p.lon_deg);
                    if let Some(mut p) = Located::new(lat, lon, "VDL4 sync CPR", Origin::Vdl4Sync) {
                        if (1.0..=49_999.0).contains(&alt) {
                            p.alt_ft = Some(alt);
                        }
                        m.detail.push(format!("  position {lat:.5} {lon:.5}"));
                        m.position = Some(p);
                    }
                }
            }
            3 => {
                m.dls_hex = hex(info);
                m.dls_text = info
                    .iter()
                    .skip(2)
                    .filter(|c| (0x20..0x7F).contains(*c))
                    .map(|&c| c as char)
                    .collect();
                m.summary = if m.dls_text.trim().is_empty() {
                    format!("DLS {}", &m.dls_hex[..m.dls_hex.len().min(24)])
                } else {
                    format!("DLS \"{}\"", m.dls_text)
                };
                m.detail.push(format!("  DLS {}", m.dls_hex));
            }
            _ => m.summary = format!("{name} {} octets", info.len()),
        }
        Some(m)
    }
}

impl Component for Reader {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.vdl4_reader",
            vec![Port::typed::<Burst>("in")],
            vec![Port::typed::<Message>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.station = ctx.station.position;
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.last.clear();
        Ok(vec![Format::new(Kind::typed::<Message>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut v = std::mem::take(&mut self.out);
        for b in io.items::<Burst>(0) {
            if let Some(m) = self.read(&b.value) {
                v.push(m);
            }
        }
        if !v.is_empty() {
            let o = io.items_out::<Message>(0);
            for m in v.drain(..) {
                o.push(Timed {
                    time: m.time_s,
                    value: m,
                });
            }
        }
        self.out = v;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m3_sync_burst_parses() {
        let c = cpr::encode(&CprFormat::VDL4, &Position::new(51.47, -0.45), false).unwrap();
        let f = write(
            true,
            1,
            0x4CA2B1,
            7,
            1,
            &sync_info(c.lat, c.lon, false, 35_000.0),
        );
        let mut r = Reader {
            station: Some(Position::new(51.4, -0.4)),
            ..Default::default()
        };
        let m = r
            .read(&Burst {
                frame: f,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(m.msg_id, 1);
        assert_eq!(m.msg_name, "SYNC");
        assert_eq!(m.addr, 0x4CA2B1);
        let p = m.position.unwrap();
        assert!((p.lat - 51.47).abs() < 0.01 && (p.lon + 0.45).abs() < 0.01);
        assert_eq!(p.alt_ft, Some(35_000.0));
        assert_eq!(m.summary, "SYNC 4CA2B1");
    }
}
