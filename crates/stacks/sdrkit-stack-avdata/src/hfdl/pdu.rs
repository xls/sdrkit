//! HFDL PDUs: squitters (SPDU), MPDUs and their LPDUs, and the HFNPDUs (performance data, frequency data, system table,
//! ACARS). Reading and building.
//! Standard: ARINC 635-4 clause 5 (link layer) and clause 6 (HFNPDUs), ICAO Doc 9741.

use super::stations::{mask_khz, name};
use crate::messages::{Located, Origin};
use crate::report::{hex, hms, one_line};
use sdrkit_proto_aero::acars::{self, assemble::Quality, Message as AcarsMessage};
use sdrkit_util_bits::crc::catalog::{CRC16_IBM_SDLC, CRC16_KERMIT};
use sdrkit_util_bits::crc::Trailer;
use sdrkit_util_bits::{BitReader, BitWriter};
use serde::{Deserialize, Serialize};

/// The HDLC frame check sequence of HFDL (CRC-16/IBM-SDLC, sent low octet first, ARINC 635 clause 5.2).
fn fcs(data: &[u8]) -> u16 {
    CRC16_IBM_SDLC.checksum(data)
}

/// Whether `frame` (data then its FCS, low octet first) passes the frame check.
fn fcs_ok(frame: &[u8]) -> bool {
    CRC16_IBM_SDLC.verify(frame, Trailer::Little)
}

/// One message (an SPDU or an LPDU) of a burst.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// "SPDU", "Unnumbered data", "Logon request" ...
    pub kind: String,
    /// A squitter.
    pub squitter: bool,
    /// Ground to air.
    pub uplink: bool,
    /// Ground station id.
    pub gs_id: u8,
    /// Aircraft id.
    pub ac_id: Option<u8>,
    /// ICAO address (from a logon).
    pub icao: Option<u32>,
    /// HFNPDU name.
    pub hfnpdu: String,
    /// Summary.
    pub summary: String,
    /// Detail lines.
    pub detail: Vec<String>,
    /// TDMA frame index.
    pub frame_index: Option<u16>,
    /// UTC synchronised.
    pub utc_sync: bool,
    /// Frequencies in use (kHz) of the sender (squitter).
    pub active_khz: Vec<String>,
    /// The other stations of a squitter: (id, UTC, frequencies).
    pub others: Vec<(u8, bool, Vec<String>)>,
    /// System table version.
    pub system_table_version: Option<u16>,
    /// The ACARS message.
    #[serde(skip)]
    pub acars: Option<AcarsMessage>,
    /// The ACARS block's check sequence was good.
    pub acars_ok: bool,
    /// Position.
    pub position: Option<Located>,
    /// Flight id (performance or frequency data).
    pub flight: String,
    /// UTC seconds of the day given by the aircraft.
    pub utc_sec: Option<u32>,
    /// Logon confirm: the aircraft id assigned.
    pub assigned_id: Option<u8>,
    /// Logged off or denied.
    pub logoff: bool,
}

/// Why a burst produced no PDU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// Fewer than 4 octets.
    Short,
    /// A squitter whose FCS failed.
    SquitterFcs,
    /// An MPDU whose header FCS failed.
    HeaderFcs,
    /// An LPDU with a bad FCS inside a good MPDU.
    LpduFcs,
    /// An LPDU of a type not decoded.
    LpduType,
}

/// Fields of an HFDL PDU, read least significant bit first (ARINC 635 clause 5.1).
struct Bits<'a>(BitReader<'a, sdrkit_util_bits::Lsb>);
impl Bits<'_> {
    fn at(b: &[u8], bit: usize) -> Bits<'_> {
        Bits(BitReader::at_lsb(b, bit))
    }
    fn take(&mut self, n: usize) -> u32 {
        self.0.u(n as u32) as u32
    }
}

/// Writes fields least significant bit first into `buf`, from bit `pos` on (the bits there must be zero).
fn put_fields(buf: &mut [u8], pos: usize, fields: &[(u32, u32)]) {
    let mut tmp = Vec::new();
    {
        let mut w = BitWriter::new_lsb(&mut tmp);
        w.put(0, (pos % 8) as u32);
        for &(v, n) in fields {
            w.put(v as u64, n);
        }
    }
    for (o, t) in buf[pos / 8..].iter_mut().zip(&tmp) {
        *o |= t;
    }
}

fn rev8(b: u8) -> u8 {
    b.reverse_bits()
}

/// Reads a squitter (66 octets).
pub fn read_spdu(b: &[u8]) -> Result<Message, Why> {
    if b.len() < 66 || !fcs_ok(&b[..66]) {
        return Err(Why::SquitterFcs);
    }
    let rls = (b[0] >> 1) & 3;
    let ver = (b[0] >> 3) & 3;
    let iso = b[0] & 0x20 != 0;
    let change = b[0] >> 6;
    let id = b[1] & 0x7F;
    let utc = b[1] & 0x80 != 0;
    let frame = (b[2] as u16) | ((b[3] as u16 & 0x0F) << 8);
    let offset = b[3] >> 4;
    let prio = b[52] & 0x0F;
    let stv = (b[53] as u16) | ((b[54] as u16 & 0x0F) << 8);
    let mut r = Bits::at(b, 54 * 8 + 4);
    let mask = r.take(20);
    let mut others = Vec::new();
    for _ in 0..2 {
        let oid = r.take(7) as u8;
        let outc = r.take(1) != 0;
        let m = r.take(20);
        if oid != 0 {
            others.push((oid, outc, mask_khz(oid, m)));
        }
    }
    let active = mask_khz(id, mask);
    let frame_utc = hms(frame as f64 * 32.0);
    let mut m = Message {
        kind: "SPDU".into(),
        squitter: true,
        uplink: true,
        gs_id: id,
        frame_index: Some(frame),
        utc_sync: utc,
        system_table_version: Some(stv),
        summary: format!(
            "Squitter {}, frame {frame}, {} frequencies in use",
            name(id),
            active.len()
        ),
        ..Default::default()
    };
    m.detail.push(format!(
        "SPDU (squitter) from ground station {id} {}{}",
        name(id),
        if utc { ", UTC synchronised" } else { "" }
    ));
    m.detail.push(format!(
        "  version {ver}, change note: {}, RLS {rls}, ISO 8208 {}",
        [
            "none",
            "channel down",
            "upcoming frequency change",
            "frequency change"
        ][change as usize & 3],
        if iso { "supported" } else { "not supported" }
    ));
    m.detail.push(format!("  TDMA frame {frame} ({frame_utc} UTC at 32 s per frame), offset {offset}, minimum priority {prio}, system table version {stv}"));
    m.detail
        .push(format!("  frequencies in use: {} kHz", active.join(" ")));
    for (oid, outc, k) in &others {
        m.detail.push(format!(
            "  station {oid} {}{}: {} kHz",
            name(*oid),
            if *outc { " (UTC)" } else { "" },
            k.join(" ")
        ));
    }
    m.active_khz = active;
    m.others = others;
    Ok(m)
}

/// Builds a squitter.
pub fn build_spdu(
    id: u8,
    utc: bool,
    frame: u16,
    stv: u16,
    mask: u32,
    others: &[(u8, bool, u32)],
) -> Vec<u8> {
    let mut b = vec![0u8; 66];
    b[0] = 0x20 | (1 << 3);
    b[1] = (id & 0x7F) | ((utc as u8) << 7);
    b[2] = frame as u8;
    b[3] = ((frame >> 8) & 0x0F) as u8;
    b[52] = 1;
    b[53] = stv as u8;
    b[54] = ((stv >> 8) & 0x0F) as u8;
    let mut fields = vec![(mask, 20)];
    for k in 0..2 {
        let (oid, ou, m) = others.get(k).copied().unwrap_or((0, false, 0));
        fields.extend([(oid as u32, 7), (ou as u32, 1), (m, 20)]);
    }
    put_fields(&mut b[..64], 54 * 8 + 4, &fields);
    let c = fcs(&b[..64]);
    b[64] = c as u8;
    b[65] = (c >> 8) as u8;
    b
}

fn latlon(r: &mut Bits) -> (f64, f64) {
    let s = |v: u32| {
        (if v & 0x80000 != 0 {
            v as i32 - 0x100000
        } else {
            v as i32
        }) as f64
            * 180.0
            / (1 << 19) as f64
    };
    let la = r.take(20);
    let lo = r.take(20);
    (s(la), s(lo))
}

/// A latitude or longitude as the 20-bit field of the performance and frequency data (180 / 2^19 degrees per step).
fn angle20(d: f64) -> u32 {
    ((d * (1 << 19) as f64 / 180.0).round() as i32 & 0xFFFFF) as u32
}

fn read_hfnpdu(m: &mut Message, b: &[u8]) {
    if b.len() < 2 || b[0] != 0xFF {
        m.hfnpdu = "unknown".into();
        m.summary = format!("{} {}", m.kind, hex(&b[..b.len().min(12)]));
        return;
    }
    match b[1] {
        0xD1 | 0xD5 => {
            let perf = b[1] == 0xD1;
            m.hfnpdu = if perf {
                "Performance data"
            } else {
                "Frequency data"
            }
            .into();
            let flight: String = b
                .get(2..8)
                .unwrap_or(&[])
                .iter()
                .map(|&c| c as char)
                .collect::<String>()
                .trim()
                .to_string();
            let mut r = Bits::at(b, 8 * 8);
            let (lat, lon) = latlon(&mut r);
            let t2 = r.take(16);
            let utc = t2 * 2;
            m.flight = flight.clone();
            m.utc_sec = Some(utc);
            m.position = Located::new(lat, lon, m.hfnpdu.as_str(), Origin::Hfdl).map(|mut p| {
                p.sec_of_day = Some(utc as f64);
                p
            });
            m.detail.push(format!(
                "{}: flight {flight}, position {lat:.5} {lon:.5}, {} UTC",
                m.hfnpdu,
                hms(utc as f64)
            ));
            let rest = b.get(15..).unwrap_or(&[]);
            if perf && rest.len() >= 4 {
                m.detail.push(format!(
                    "  version {}, flight leg {}, ground station {}, frequency id {}",
                    rest[0], rest[1], rest[2], rest[3]
                ));
                if rest.len() >= 14 {
                    let w = |i: usize| u16::from_le_bytes([rest[i], rest[i + 1]]);
                    m.detail.push(format!(
                        "  MPDUs received 1800 {} / 1200 {} / 600 {} / 300 {}, with errors {}, SPDUs received {} missed {}",
                        w(4), w(6), w(8), w(10), w(12), rest.get(14).copied().unwrap_or(0), rest.get(15).copied().unwrap_or(0)
                    ));
                }
            } else if !perf {
                let mut i = 0;
                while i + 7 <= rest.len() {
                    let id = rest[i] & 0x7F;
                    let mut r = Bits::at(&rest[i + 1..], 0);
                    let heard = r.take(20);
                    let listen = r.take(20);
                    m.detail.push(format!(
                        "  station {id} {}: heard {} kHz, listening on {} kHz",
                        name(id),
                        mask_khz(id, heard).join(" "),
                        mask_khz(id, listen).join(" ")
                    ));
                    i += 7;
                }
            }
            m.summary = format!("{} {flight}  {lat:.3} {lon:.3}", m.hfnpdu);
        }
        0xD0 => {
            m.hfnpdu = "System table".into();
            let part = b.get(2).copied().unwrap_or(0);
            let parts = b.get(3).copied().unwrap_or(0);
            let ver = u16::from_le_bytes([
                b.get(4).copied().unwrap_or(0),
                b.get(5).copied().unwrap_or(0),
            ]);
            m.summary =
                format!("System table part {part} of {parts}, version {ver} (not reassembled)");
            m.detail.push(m.summary.clone());
        }
        0xD2 => {
            m.hfnpdu = "System table request".into();
            m.summary = m.hfnpdu.clone();
        }
        0xDE => {
            m.hfnpdu = "Delayed echo".into();
            m.summary = m.hfnpdu.clone();
        }
        0xFF => {
            m.hfnpdu = "ACARS".into();
            let mut a = &b[2..];
            if a.first().map(|c| c & 0x7F) == Some(acars::SOH) {
                a = &a[1..];
            }
            if a.last() == Some(&acars::DEL) {
                a = &a[..a.len() - 1];
            }
            if a.len() >= 15 {
                let body = &a[..a.len() - 2];
                let bcs = [a[a.len() - 2], a[a.len() - 1]];
                let ok = CRC16_KERMIT.checksum(body).to_le_bytes() == bcs;
                let mut stripped = body.to_vec();
                acars::strip_parity(&mut stripped);
                if let Ok(block) = acars::Block::parse(&stripped) {
                    let q = Quality {
                        bits_fixed: 0,
                        bcs_ok: ok,
                    };
                    let am = AcarsMessage::from_block(&block, q);
                    m.summary = format!(
                        "ACARS {}{} {}: {}",
                        am.label,
                        am.label_meaning
                            .map(|t| format!(" ({t})"))
                            .unwrap_or_default(),
                        am.flight,
                        one_line(&am.text, 80)
                    );
                    m.detail.push(format!(
                        "ACARS: registration {}, label {}, block id {}{}",
                        am.registration,
                        am.label,
                        am.block_id,
                        if ok { "" } else { ", check sequence FAILED" }
                    ));
                    m.position = Located::of_message(&am);
                    if !am.flight.is_empty() {
                        m.flight = am.flight.clone();
                    }
                    m.acars_ok = ok;
                    m.acars = Some(am);
                }
            } else {
                m.summary = "ACARS: cut short".into();
            }
        }
        t => {
            m.hfnpdu = format!("HFNPDU type 0x{t:02X}");
            m.summary = m.hfnpdu.clone();
        }
    }
}

/// The LPDU type names.
pub fn lpdu_name(t: u8) -> Option<&'static str> {
    Some(match t {
        0x0D => "Unnumbered data",
        0x1D => "Unnumbered acknowledged data",
        0x8F => "Logon request",
        0xBF => "Logon request (DLS)",
        0x4F => "Logon resume",
        0x9F => "Logon confirm",
        0x5F => "Logon resume confirm",
        0x2F => "Logon denied",
        0x3F => "Logoff request",
        _ => return None,
    })
}

fn icao_of(b: &[u8]) -> Option<u32> {
    (b.len() >= 3).then(|| (rev8(b[0]) as u32) << 16 | (rev8(b[1]) as u32) << 8 | rev8(b[2]) as u32)
}

fn read_lpdu(
    b: &[u8],
    gs: u8,
    ac: Option<u8>,
    uplink: bool,
    why: &mut Vec<Why>,
) -> Option<Message> {
    if !fcs_ok(b) || b.len() < 3 {
        why.push(Why::LpduFcs);
        return None;
    }
    let body = &b[..b.len() - 2];
    let t = body[0];
    let Some(kind) = lpdu_name(t) else {
        why.push(Why::LpduType);
        return None;
    };
    let mut m = Message {
        kind: kind.into(),
        uplink,
        gs_id: gs,
        ac_id: ac,
        ..Default::default()
    };
    m.detail.push(format!(
        "MPDU {}, ground station {gs} {}, aircraft id {}",
        if uplink { "uplink" } else { "downlink" },
        name(gs),
        ac.map_or("-".into(), |a| a.to_string())
    ));
    match t {
        0x0D | 0x1D => read_hfnpdu(&mut m, &body[1..]),
        0x8F | 0xBF | 0x4F => {
            m.icao = icao_of(&body[1..]);
            m.summary = format!("{kind} {:06X}", m.icao.unwrap_or(0));
        }
        0x9F | 0x5F => {
            m.icao = icao_of(&body[1..]);
            m.assigned_id = body.get(4).copied();
            m.summary = format!(
                "{kind} {:06X}, aircraft id {}",
                m.icao.unwrap_or(0),
                m.assigned_id.unwrap_or(0)
            );
        }
        0x2F | 0x3F => {
            m.icao = icao_of(&body[1..]);
            m.logoff = true;
            m.summary = format!(
                "{kind} {:06X}, reason {}",
                m.icao.unwrap_or(0),
                body.get(4).copied().unwrap_or(0)
            );
        }
        _ => {}
    }
    if m.detail.len() == 1 {
        m.detail.push(format!("  {}", m.summary));
    }
    Some(m)
}

/// Reads the PDU of a burst: its messages, and why nothing (or something) was lost.
pub fn read(b: &[u8]) -> (Vec<Message>, Vec<Why>) {
    let mut why = Vec::new();
    if b.len() < 4 {
        return (vec![], vec![Why::Short]);
    }
    if b[0] & 1 == 0 {
        return match read_spdu(b) {
            Ok(m) => (vec![m], why),
            Err(w) => (vec![], vec![w]),
        };
    }
    let down = b[0] & 2 != 0;
    let mut out = Vec::new();
    if down {
        let n = ((b[0] >> 2) & 0x0F) as usize;
        let gs = b[1] & 0x7F;
        let ac = b[2];
        // The header holds three octets of the aircraft's link state (not shown) between the aircraft id and the LPDU sizes. A
        // header without them is read too.
        let Some(sz) = [6usize, 3]
            .into_iter()
            .find(|&sz| b.len() >= sz + n + 2 && fcs_ok(&b[..sz + n + 2]))
        else {
            return (vec![], vec![Why::HeaderFcs]);
        };
        let mut at = sz + n + 2;
        for k in 0..n {
            let size = b[sz + k] as usize + 1;
            if at + size > b.len() {
                why.push(Why::LpduFcs);
                break;
            }
            if let Some(m) = read_lpdu(&b[at..at + size], gs, Some(ac), false, &mut why) {
                out.push(m);
            }
            at += size;
        }
    } else {
        let nac = (((b[0] >> 4) & 7) + 1) as usize;
        let gs = b[1] & 0x7F;
        let mut i = 2;
        let mut plan: Vec<(u8, Vec<usize>)> = Vec::new();
        for _ in 0..nac {
            let Some(&id) = b.get(i) else { break };
            let cnt = (b.get(i + 1).copied().unwrap_or(0) >> 4) as usize;
            let sizes: Vec<usize> = (0..cnt)
                .map(|k| b.get(i + 2 + k).copied().unwrap_or(0) as usize + 1)
                .collect();
            i += 2 + cnt;
            plan.push((id, sizes));
        }
        let hl = i + 2;
        if b.len() < hl || !fcs_ok(&b[..hl]) {
            return (vec![], vec![Why::HeaderFcs]);
        }
        let mut at = hl;
        for (id, sizes) in plan {
            for size in sizes {
                if at + size > b.len() {
                    why.push(Why::LpduFcs);
                    break;
                }
                if let Some(m) = read_lpdu(&b[at..at + size], gs, Some(id), true, &mut why) {
                    out.push(m);
                }
                at += size;
            }
        }
    }
    (out, why)
}

/// An LPDU with its FCS.
pub fn lpdu(body: &[u8]) -> Vec<u8> {
    let mut v = body.to_vec();
    let c = fcs(&v);
    v.push(c as u8);
    v.push((c >> 8) as u8);
    v
}

/// A downlink MPDU from aircraft `ac` to station `gs` carrying LPDUs (each with its FCS).
pub fn downlink(gs: u8, ac: u8, lpdus: &[Vec<u8>]) -> Vec<u8> {
    let mut h = vec![
        1 | 2 | ((lpdus.len() as u8 & 0x0F) << 2),
        gs & 0x7F,
        ac,
        0,
        0,
        0,
    ];
    for l in lpdus {
        h.push((l.len() - 1) as u8);
    }
    let c = fcs(&h);
    h.push(c as u8);
    h.push((c >> 8) as u8);
    for l in lpdus {
        h.extend_from_slice(l);
    }
    h
}

/// An uplink MPDU from station `gs` to aircraft (id, LPDUs).
pub fn uplink(gs: u8, to: &[(u8, Vec<Vec<u8>>)]) -> Vec<u8> {
    let mut h = vec![
        1 | (((to.len() as u8).saturating_sub(1) & 7) << 4),
        gs & 0x7F,
    ];
    for (id, ls) in to {
        h.push(*id);
        h.push((ls.len() as u8) << 4);
        for l in ls {
            h.push((l.len() - 1) as u8);
        }
    }
    let c = fcs(&h);
    h.push(c as u8);
    h.push((c >> 8) as u8);
    for (_, ls) in to {
        for l in ls {
            h.extend_from_slice(l);
        }
    }
    h
}

/// An unnumbered data LPDU body carrying an ACARS block.
pub fn acars_lpdu(body: &[u8], bcs: [u8; 2]) -> Vec<u8> {
    let mut v = vec![0x0D, 0xFF, 0xFF, acars::SOH];
    v.extend_from_slice(body);
    v.extend_from_slice(&bcs);
    v.push(acars::DEL);
    lpdu(&v)
}

/// A performance data LPDU.
pub fn perf_lpdu(flight: &str, lat: f64, lon: f64, utc_sec: u32, gs: u8, freq_id: u8) -> Vec<u8> {
    let mut v = vec![0x0D, 0xFF, 0xD1];
    v.extend(format!("{flight:<6}").bytes().take(6));
    let mut tail = vec![0u8; 7];
    put_fields(
        &mut tail,
        0,
        &[(angle20(lat), 20), (angle20(lon), 20), (utc_sec / 2, 16)],
    );
    v.extend(tail);
    v.extend([1, 1, gs, freq_id]);
    v.extend([0u8; 12]);
    lpdu(&v)
}

/// A logon request (`confirm`: a logon confirm assigning `ac`).
pub fn logon_lpdu(t: u8, icao: u32, ac: u8) -> Vec<u8> {
    let mut v = vec![
        t,
        rev8((icao >> 16) as u8),
        rev8((icao >> 8) as u8),
        rev8(icao as u8),
    ];
    v.push(ac);
    lpdu(&v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_proto_aero::acars::{BlockSpec, End};

    #[test]
    fn squitter_round_trip() {
        let b = build_spdu(1, true, 777, 50, 0x21, &[(7, true, 3)]);
        let (m, _) = read(&b);
        let m = &m[0];
        assert_eq!(m.gs_id, 1);
        assert_eq!(m.frame_index, Some(777));
        assert_eq!(m.active_khz, vec!["21934", "8927"]);
        assert_eq!(m.others[0], (7, true, vec!["11384".into(), "10081".into()]));
        assert_eq!(m.system_table_version, Some(50));
        assert_eq!(
            m.summary,
            "Squitter San Francisco, frame 777, 2 frequencies in use"
        );
        assert!(m.detail[2].contains("06:54:24"));
    }

    #[test]
    fn mpdus_round_trip() {
        let mut blk = Vec::new();
        let c = BlockSpec::new('2', "N123UA", b'2', End::Etx)
            .label("H1")
            .text("M01AUA0901#DFB TEST")
            .body(&mut blk)
            .unwrap();
        let p = downlink(
            1,
            23,
            &[
                perf_lpdu("UA0901", 51.5, -30.25, 40000, 1, 2),
                acars_lpdu(&blk, c),
            ],
        );
        let (ms, why) = read(&p);
        assert!(why.is_empty(), "{why:?}");
        assert_eq!(ms.len(), 2);
        assert_eq!(ms[0].hfnpdu, "Performance data");
        assert_eq!(ms[0].flight, "UA0901");
        let pos = ms[0].position.as_ref().unwrap();
        assert!((pos.lat - 51.5).abs() < 0.001 && (pos.lon + 30.25).abs() < 0.001);
        assert_eq!(ms[0].utc_sec, Some(40000));
        assert!(ms[1].acars_ok);
        assert_eq!(ms[1].acars.as_ref().unwrap().registration, "N123UA");
        let u = uplink(1, &[(23, vec![logon_lpdu(0x9F, 0x4CA2B1, 23)])]);
        let (ms, _) = read(&u);
        assert_eq!(ms[0].icao, Some(0x4CA2B1));
        assert_eq!(ms[0].assigned_id, Some(23));
        assert!(ms[0].uplink);
    }
}
