//! Reading VDL Mode 2 frames: AVLC addresses and control, ACARS over AVLC, ISO 8208 (X.25) with CLNP, COTP, ES-IS and IDRP, and
//! XID with the VDL parameters. And building frames for the generator.
//! Standard: ICAO Doc 9776 clause 5 (AVLC), ARINC 631 (ACARS over AVLC), ISO 8208, ISO 8473, ISO 8073, ISO/IEC 13239 (XID).

use crate::messages::{adsc_lines, Located, Origin};
use sdrkit_proto_aero::acars::reading_of;
use crate::report::{hex, one_line};
use sdrkit_proto_aero::acars::{self, assemble::Quality, Message, Reading};
use sdrkit_util_bits::crc::catalog::CRC16_KERMIT;
use serde::{Deserialize, Serialize};

/// A decoded frame (every field of 3.6 - 3.9).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// Source address.
    pub src: u32,
    /// Source type.
    pub src_type: u8,
    /// Destination address.
    pub dst: u32,
    /// Destination type.
    pub dst_type: u8,
    /// The destination's A/G bit: the aircraft is on the ground.
    pub on_ground: bool,
    /// The source's C/R bit.
    pub response: bool,
    /// "I", "S" or "U".
    pub kind: String,
    /// RR, RNR, UI, XID, the XID frame name ...
    pub name: String,
    /// N(S) of an I frame.
    pub ns: Option<u8>,
    /// N(R) of an I or S frame.
    pub nr: Option<u8>,
    /// Poll / final.
    pub pf: bool,
    /// The deepest layer read: "ACARS", "X.25", "CLNP", "COTP", "XID", "AVLC".
    pub proto: String,
    /// One line.
    pub summary: String,
    /// Every decoded layer, one line each.
    pub detail: Vec<String>,
    /// The ACARS message.
    #[serde(skip)]
    pub acars: Option<Message>,
    /// The ACARS block body (mode to ETX or ETB, parity cleared), for joining.
    #[serde(skip)]
    pub acars_body: Vec<u8>,
    /// The ACARS check sequence passed.
    pub acars_crc_ok: bool,
    /// A position found.
    pub position: Option<Located>,
    /// Airports (coverage) of a GSIF.
    pub airports: String,
    /// Destination airport of a link establishment.
    pub destination_airport: String,
    /// X.25 called DTE.
    pub called_dte: String,
    /// X.25 calling DTE.
    pub calling_dte: String,
    /// X.25 call user data.
    pub call_user_data: String,
    /// Raw octets (FCS removed).
    #[serde(skip)]
    pub raw: Vec<u8>,
}

/// The name of an address type.
pub fn type_name(t: u8) -> &'static str {
    match t {
        1 => "aircraft",
        4 => "ground station",
        5 => "ground station",
        7 => "all stations",
        _ => "reserved",
    }
}

fn rev28(v: u32) -> u32 {
    let mut r = 0;
    for i in 0..28 {
        if v & (1 << i) != 0 {
            r |= 1 << (27 - i);
        }
    }
    r
}

/// Reads a 4-octet address: (address, type, status bit).
pub fn read_addr(b: &[u8]) -> (u32, u8, bool) {
    let mut raw = 0u32;
    for (i, o) in b.iter().take(4).enumerate() {
        raw |= ((*o as u32 >> 1) & 0x7F) << (7 * i);
    }
    let v = rev28(raw);
    (v & 0xFF_FFFF, ((v >> 24) & 7) as u8, (v >> 27) & 1 != 0)
}

/// Writes a 4-octet address. `end` sets the address-end bit of the last octet.
pub fn write_addr(addr: u32, typ: u8, status: bool, end: bool) -> [u8; 4] {
    let v = (addr & 0xFF_FFFF) | ((typ as u32 & 7) << 24) | ((status as u32) << 27);
    let raw = rev28(v);
    let mut o = [0u8; 4];
    for (i, x) in o.iter_mut().enumerate() {
        *x = (((raw >> (7 * i)) & 0x7F) as u8) << 1;
    }
    if end {
        o[3] |= 1;
    }
    o
}

/// The name of an XID frame from (C/R, P/F, h, r).
pub fn xid_name(cr: bool, pf: bool, h: bool, r: bool) -> &'static str {
    let i = (cr as u8) << 3 | (pf as u8) << 2 | (h as u8) << 1 | r as u8;
    match i {
        0b0001 => "XID_CMD_LCR",
        0b0010 | 0b0110 => "XID_CMD_HO",
        0b0011 => "GSIF",
        0b0100 => "XID_CMD_LE",
        0b0111 => "XID_CMD_LPM",
        0b1100 => "XID_RSP_LE",
        0b1101 => "XID_RSP_LCR",
        0b1110 => "XID_RSP_HO",
        0b1111 => "XID_RSP_LPM",
        _ => "XID",
    }
}

fn xid_description(name: &str) -> &'static str {
    match name {
        "GSIF" => "ground station information",
        "XID_CMD_LE" => "link establishment",
        "XID_RSP_LE" => "link establishment response",
        "XID_CMD_HO" => "handoff",
        "XID_RSP_HO" => "handoff response",
        "XID_CMD_LCR" | "XID_RSP_LCR" => "link connection refused",
        _ => "link parameters",
    }
}

fn s12(v: u32) -> f64 {
    let v = (v & 0xFFF) as i32;
    (if v & 0x800 != 0 { v - 0x1000 } else { v }) as f64 / 10.0
}

fn bcd(b: &[u8], start_digit: usize, n: usize) -> String {
    (0..n)
        .map(|i| {
            let d = start_digit + i;
            let o = b.get(d / 2).copied().unwrap_or(0);
            let v = if d.is_multiple_of(2) {
                o >> 4
            } else {
                o & 0x0F
            };
            char::from_digit(v as u32, 16).unwrap_or('?')
        })
        .collect()
}

/// Reads a frame (FCS removed).
pub fn read(raw: &[u8]) -> Option<Frame> {
    if raw.len() < 9 {
        return None;
    }
    let (dst, dst_type, ag) = read_addr(&raw[0..4]);
    let (src, src_type, cr) = read_addr(&raw[4..8]);
    let c = raw[8];
    let info = &raw[9..];
    let mut f = Frame {
        src,
        src_type,
        dst,
        dst_type,
        on_ground: ag,
        response: cr,
        proto: "AVLC".into(),
        raw: raw.to_vec(),
        ..Default::default()
    };
    f.detail.push(format!(
        "AVLC: {src:06X} ({}) to {dst:06X} ({}), {}, {}",
        type_name(src_type),
        type_name(dst_type),
        if ag { "on ground" } else { "airborne" },
        if cr { "response" } else { "command" }
    ));
    if c & 1 == 0 {
        f.kind = "I".into();
        f.ns = Some((c >> 1) & 7);
        f.pf = (c >> 4) & 1 != 0;
        f.nr = Some(c >> 5);
        f.name = "I".into();
        f.detail.push(format!(
            "  I frame, N(S) {}, N(R) {}, {} octets of information",
            (c >> 1) & 7,
            c >> 5,
            info.len()
        ));
    } else if c & 3 == 1 {
        f.kind = "S".into();
        f.name = ["RR", "RNR", "REJ", "SREJ"][((c >> 2) & 3) as usize].into();
        f.nr = Some(c >> 5);
        f.pf = (c >> 4) & 1 != 0;
        f.detail
            .push(format!("  S frame {}, N(R) {}", f.name, c >> 5));
        f.summary = format!("{} N(R) {}", f.name, c >> 5);
        return Some(f);
    } else {
        f.kind = "U".into();
        f.pf = (c >> 4) & 1 != 0;
        f.name = match (c >> 2) & 0x3B {
            0x00 => "UI",
            0x03 => "DM",
            0x10 => "DISC",
            0x18 => "UA",
            0x21 => "FRMR",
            0x2B => "XID",
            0x38 => "TEST",
            _ => "U?",
        }
        .into();
        f.detail.push(format!(
            "  U frame {}{}, {} octets of information",
            f.name,
            if f.pf { ", poll/final" } else { "" },
            info.len()
        ));
    }
    if (f.kind == "I" || f.name == "UI")
        && info.len() >= 3
        && info[0] == 0xFF
        && info[1] == 0xFF
        && info[2] & 0x7F == 0x01
    {
        read_acars(&mut f, &info[3..]);
    } else if f.name == "XID" {
        read_xid(&mut f, info);
    } else if f.kind == "I" {
        if info.is_empty() {
            f.summary = "I frame, no information".into();
        } else {
            read_x25(&mut f, info);
        }
    } else {
        f.summary = format!("{} frame, {} octets", f.name, info.len());
    }
    Some(f)
}

fn read_acars(f: &mut Frame, b: &[u8]) {
    f.proto = "ACARS".into();
    let mut b = b;
    if b.last() == Some(&0x7F) {
        b = &b[..b.len() - 1];
    }
    if b.len() < 16 {
        f.summary = "ACARS: cut short".into();
        f.detail.push("ACARS: cut short".into());
        return;
    }
    let body = &b[..b.len() - 2];
    let bcs = [b[b.len() - 2], b[b.len() - 1]];
    let ok = CRC16_KERMIT.checksum(body).to_le_bytes() == bcs;
    let mut stripped = body.to_vec();
    acars::strip_parity(&mut stripped);
    let Ok(block) = acars::Block::parse(&stripped) else {
        f.summary = "ACARS: cut short".into();
        return;
    };
    let q = Quality {
        bits_fixed: 0,
        bcs_ok: ok,
    };
    let m = Message::from_block(&block, q);
    f.acars_crc_ok = ok;
    describe_acars(f, &m);
    f.acars = Some(m);
    f.acars_body = stripped;
}

/// Fills a frame's summary, detail lines and position from its ACARS message (again after joining).
pub fn describe_acars(f: &mut Frame, m: &Message) {
    f.detail.retain(|l| {
        !l.starts_with("ACARS")
            && !l.starts_with("  message")
            && !l.starts_with("  reading")
            && !l.starts_with("ADS-C")
            && !l.starts_with("    ")
            && !l.starts_with("  position")
            && !l.starts_with("FANS")
            && !l.starts_with("Joined")
    });
    if m.blocks > 1 {
        f.detail.push(format!(
            "Joined message of {} blocks{}:",
            m.blocks,
            if m.incomplete {
                " (the last block did not arrive)"
            } else {
                ""
            }
        ));
    }
    f.detail.push(format!(
        "ACARS: mode {}, registration {}, label {} {}, block id {}{}",
        m.mode,
        m.registration,
        m.label,
        m.label_meaning.unwrap_or(""),
        m.block_id,
        if m.quality.bcs_ok {
            ""
        } else {
            ", check sequence FAILED"
        }
    ));
    if !m.msn.is_empty() || !m.flight.is_empty() {
        f.detail
            .push(format!("  message {}, flight {}", m.msn, m.flight));
    }
    let reading = reading_of(m).0;
    if !reading.is_empty() {
        f.detail.push(format!("  reading: {reading}"));
    }
    let meaning = m
        .label_meaning
        .map(|t| format!(" ({t})"))
        .unwrap_or_default();
    f.summary = format!(
        "ACARS {}{meaning} {}: {}",
        m.label,
        m.flight,
        one_line(&m.text, 80)
    )
    .replace("  ", " ");
    if m.flight.is_empty() {
        f.summary = format!("ACARS {}{meaning}: {}", m.label, one_line(&m.text, 80));
    }
    f.position = Located::of_message(m);
    match &m.reading {
        Reading::Adsc(a) => {
            f.detail.push("ADS-C (FANS-1/A):".into());
            for l in adsc_lines(a) {
                f.detail.push(format!("    {l}"));
            }
            if a.crc_ok {
                if a.report().is_some() {
                    f.summary = format!("ACARS {} ADS-C position report", m.label);
                } else if a.earth_reference().is_some() {
                    f.summary = format!("ACARS {} ADS-C earth reference", m.label);
                }
            }
        }
        Reading::Cpdlc(c) => {
            f.detail.push(c.description());
            for l in c.text().lines() {
                f.detail.push(format!("    {l}"));
            }
            f.summary = match c.summary() {
                "" => format!("ACARS {} FANS-1/A {}", m.label, c.kind()),
                s => format!("ACARS {} CPDLC {s}", m.label),
            };
        }
        _ => {}
    }
    if let Some(p) = &f.position {
        f.detail.push(format!("  position {}", p.summary()));
    }
}

fn read_x25(f: &mut Frame, b: &[u8]) {
    f.proto = "X.25".into();
    if b.len() < 3 {
        f.summary = "X.25: too short".into();
        f.detail.push("X.25: too short".into());
        return;
    }
    let lcn = ((b[0] as u16 & 0x0F) << 8) | b[1] as u16;
    let t = b[2];
    if t & 1 == 0 {
        let ps = (t >> 1) & 7;
        let pr = t >> 5;
        let m = (t >> 4) & 1 != 0;
        f.summary = format!("X.25 data, LCN {lcn}, P(S) {ps} P(R) {pr}");
        f.detail.push(format!(
            "X.25 data, LCN {lcn}, P(S) {ps} P(R) {pr}{}",
            if m { ", more follows" } else { "" }
        ));
        let ud = &b[3..];
        match ud.first() {
            Some(0x81) => read_clnp(f, ud),
            Some(0x82) => {
                f.detail.push(format!(
                    "ES-IS (ISO 9542 routeing exchange), {} octets",
                    ud.len()
                ));
            }
            Some(0x85) => {
                f.detail.push(format!(
                    "IDRP (ISO 10747 inter-domain routeing), {} octets",
                    ud.len()
                ));
            }
            Some(_) => f.detail.push(format!(
                "ICAO SNDCF compressed (LREF) CLNP header, {} octets",
                ud.len()
            )),
            None => {}
        }
        return;
    }
    let name = match t {
        0x0B => "call request",
        0x0F => "call accepted",
        0x13 => "clear request",
        0x17 => "clear confirmation",
        0x1B => "reset request",
        0x1F => "reset confirmation",
        0xFB => "restart request",
        0xFF => "restart confirmation",
        0x23 => "interrupt",
        0x27 => "interrupt confirmation",
        0xF1 => "diagnostic",
        _ => "",
    };
    let name = if name.is_empty() {
        match t & 0x1F {
            0x01 => format!("RR, P(R) {}", t >> 5),
            0x05 => format!("RNR, P(R) {}", t >> 5),
            0x09 => format!("REJ, P(R) {}", t >> 5),
            _ => format!("packet type 0x{t:02X}"),
        }
    } else {
        name.to_string()
    };
    f.summary = format!("X.25 {name}, LCN {lcn}");
    f.detail.push(format!("X.25 {name}, LCN {lcn}"));
    if (t == 0x0B || t == 0x0F) && b.len() > 3 {
        let al = b[3];
        let called_n = (al & 0x0F) as usize;
        let calling_n = (al >> 4) as usize;
        let ab = &b[4..];
        f.called_dte = bcd(ab, 0, called_n);
        f.calling_dte = bcd(ab, called_n, calling_n);
        let aoct = (called_n + calling_n).div_ceil(2);
        let rest = ab.get(aoct..).unwrap_or(&[]);
        let fl = rest.first().copied().unwrap_or(0) as usize;
        let cud = rest.get(1 + fl..).unwrap_or(&[]);
        f.detail.push(format!(
            "  called DTE {}, calling DTE {}, facilities {fl} octets",
            f.called_dte, f.calling_dte
        ));
        if !cud.is_empty() {
            let what = if cud[0] == 0xC1 { " (ICAO SNDCF)" } else { "" };
            f.call_user_data = format!("{}{what}", hex(cud));
            f.detail
                .push(format!("  call user data: {}", f.call_user_data));
        }
    } else if (t == 0x13 || t == 0x1B) && b.len() >= 5 {
        f.detail
            .push(format!("  cause {}, diagnostic {}", b[3], b[4]));
    }
}

fn read_clnp(f: &mut Frame, b: &[u8]) {
    if b.len() < 9 {
        f.detail.push("CLNP: too short".into());
        return;
    }
    f.proto = "CLNP".into();
    let li = b[1] as usize;
    let ver = b[2];
    let life = b[3] as f64 * 0.5;
    let ty = b[4] & 0x1F;
    let sp = b[4] & 0x80 != 0;
    let er = b[4] & 0x20 != 0;
    let seg = u16::from_be_bytes([b[5], b[6]]);
    let tname = match ty {
        0x1C => "DT (data)",
        0x01 => "ER (error report)",
        0x1E => "ERQ (echo request)",
        0x1F => "ERP (echo reply)",
        _ => "type ?",
    };
    let mut l = format!(
        "CLNP: {tname}, version {ver}, lifetime {life} s, segment {seg} octets, header {li} octets"
    );
    if sp {
        l.push_str(", segmented");
    }
    if er {
        l.push_str(", error reports wanted");
    }
    f.detail.push(l);
    let mut i = 9;
    if let Some(&dl) = b.get(i) {
        let d = b.get(i + 1..i + 1 + dl as usize).unwrap_or(&[]);
        i += 1 + dl as usize;
        let s = b.get(i).copied().unwrap_or(0) as usize;
        let sa = b.get(i + 1..i + 1 + s).unwrap_or(&[]);
        f.detail.push(format!(
            "  destination NSAP {}, source NSAP {}",
            hex(d),
            hex(sa)
        ));
    }
    if ty == 0x1C && b.len() > li {
        let t = &b[li..];
        if t.len() >= 2 {
            f.proto = "COTP".into();
            let name = match t[1] >> 4 {
                0xE => "CR (connection request)",
                0xD => "CC (connection confirm)",
                0xF => "DT (data)",
                0x8 => "DR (disconnect request)",
                0xC => "DC (disconnect confirm)",
                0x6 => "AK (acknowledgement)",
                0x1 => "ED (expedited data)",
                0x2 => "EA (expedited acknowledgement)",
                0x7 => "ER (error)",
                _ => "TPDU ?",
            };
            f.detail
                .push(format!("COTP: {name}, header {} octets", t[0]));
            if t[1] >> 4 == 0xF {
                f.detail.push(
                    "  ATN application data (CPDLC, ADS-C, CM over ATN: ASN.1 PER, not decoded)"
                        .into(),
                );
            }
        }
    }
}

fn read_xid(f: &mut Frame, b: &[u8]) {
    f.proto = "XID".into();
    let mut h = true;
    let mut r = true;
    let mut lines = Vec::new();
    if b.first() == Some(&0x82) {
        let mut i = 1;
        while i + 3 <= b.len() {
            let gid = b[i];
            let gl = u16::from_be_bytes([b[i + 1], b[i + 2]]) as usize;
            let g = b
                .get(i + 3..i + 3 + gl)
                .unwrap_or(&b[(i + 3).min(b.len())..]);
            i += 3 + gl;
            if gid != 0xF0 {
                lines.push(format!(
                    "  public parameters (group 0x{gid:02X}): {}",
                    hex(g)
                ));
                continue;
            }
            let mut j = 0;
            while j + 2 <= g.len() {
                let pid = g[j];
                let pl = g[j + 1] as usize;
                let v = g.get(j + 2..j + 2 + pl).unwrap_or(&[]);
                j += 2 + pl;
                let txt = || String::from_utf8_lossy(v).trim().to_string();
                match pid {
                    0x00 => lines.push(format!("  parameter set {}", txt())),
                    0x01 => {
                        let x = v.first().copied().unwrap_or(3);
                        h = x & 2 != 0;
                        r = x & 1 != 0;
                        lines.push(format!(
                            "  connection management h={} r={}",
                            h as u8, r as u8
                        ));
                    }
                    0x03 => {
                        let x = v.first().copied().unwrap_or(0);
                        lines.push(format!(
                            "  XID sequencing: sequence {}, retry {}",
                            x >> 4,
                            x & 0x0F
                        ));
                    }
                    0x83 => {
                        f.destination_airport = txt();
                        lines.push(format!("  destination airport {}", f.destination_airport));
                    }
                    0xC1 => {
                        let a: Vec<String> = v
                            .chunks(4)
                            .map(|c| String::from_utf8_lossy(c).to_string())
                            .collect();
                        f.airports = a.join(" ");
                        lines.push(format!("  airport coverage {}", f.airports));
                    }
                    0xC3 => lines.push(format!("  nearest airport {}", txt())),
                    0x84 | 0xC8 => {
                        if v.len() >= 3 {
                            let w = (v[0] as u32) << 16 | (v[1] as u32) << 8 | v[2] as u32;
                            let lat = s12(w >> 12);
                            let lon = s12(w);
                            let alt = (pid == 0x84)
                                .then(|| v.get(3).copied().unwrap_or(0) as f64 * 1000.0);
                            let kind = if pid == 0x84 {
                                "XID aircraft location"
                            } else {
                                "XID ground station location"
                            };
                            if let Some(mut p) = Located::new(lat, lon, kind, Origin::Vdl2Xid) {
                                p.alt_ft = alt;
                                lines.push(format!(
                                    "  {} {lat:.1} {lon:.1}{}",
                                    if pid == 0x84 {
                                        "aircraft location"
                                    } else {
                                        "ground station location"
                                    },
                                    alt.map_or(String::new(), |a| format!(" {a:.0} ft"))
                                ));
                                f.position = Some(p);
                            }
                        }
                    }
                    0xC0 => lines.push(format!("  frequency support list {}", hex(v))),
                    0x82 => lines.push(format!("  alternate ground stations {}", hex(v))),
                    0x81 => lines.push(format!("  modulation support {}", hex(v))),
                    _ => lines.push(format!("  parameter 0x{pid:02X}: {}", hex(v))),
                }
            }
        }
    }
    // The C/R bit's sense depends on who sends: a ground station's response is the aircraft's command.
    let ground = f.src_type == 4 || f.src_type == 5;
    let name = xid_name(f.response ^ ground, f.pf, h, r);
    f.name = name.into();
    f.detail
        .push(format!("XID {name}: {}", xid_description(name)));
    f.detail.extend(lines);
    f.summary = format!("XID {name} ({})", xid_description(name));
}

// ---------------------------------------------------------------------------------------------------------------------------
// Building frames, for the generator.

/// The address and control octets of a frame.
pub fn header(
    dst: u32,
    dst_type: u8,
    on_ground: bool,
    src: u32,
    src_type: u8,
    response: bool,
    control: u8,
) -> Vec<u8> {
    let mut v = Vec::with_capacity(9);
    v.extend(write_addr(dst, dst_type, on_ground, false));
    v.extend(write_addr(src, src_type, response, true));
    v.push(control);
    v
}

/// An I frame control octet.
pub fn i_control(ns: u8, nr: u8, p: bool) -> u8 {
    (ns & 7) << 1 | (p as u8) << 4 | (nr & 7) << 5
}

/// An S frame control octet (0 RR, 1 RNR, 2 REJ, 3 SREJ).
pub fn s_control(kind: u8, nr: u8, pf: bool) -> u8 {
    1 | (kind & 3) << 2 | (pf as u8) << 4 | (nr & 7) << 5
}

/// The XID control octet.
pub fn xid_control(pf: bool) -> u8 {
    // (control >> 2) & 0x3B == 0x2B: bits 2-3 = 11, bits 5-7 = 101.
    0x03 | (0x03 << 2) | (pf as u8) << 4 | (0x05 << 5)
}

/// ACARS information: FF FF then the block from SOH, its check bytes and DEL.
pub fn acars_info(body: &[u8], bcs: [u8; 2]) -> Vec<u8> {
    let mut v = vec![0xFF, 0xFF, 0x01];
    v.extend_from_slice(body);
    v.extend_from_slice(&bcs);
    v.push(0x7F);
    v
}

/// An XID information field with VDL private parameters.
pub fn xid_info(params: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let mut g = Vec::new();
    for (id, v) in params {
        g.push(*id);
        g.push(v.len() as u8);
        g.extend_from_slice(v);
    }
    let mut v = vec![0x82, 0xF0];
    v.extend((g.len() as u16).to_be_bytes());
    v.extend(g);
    v
}

/// A location parameter value (tenths of a degree, then the altitude in thousands of feet when given).
pub fn location(lat: f64, lon: f64, alt_ft: Option<f64>) -> Vec<u8> {
    let la = ((lat * 10.0).round() as i32 & 0xFFF) as u32;
    let lo = ((lon * 10.0).round() as i32 & 0xFFF) as u32;
    let w = la << 12 | lo;
    let mut v = vec![(w >> 16) as u8, (w >> 8) as u8, w as u8];
    if let Some(a) = alt_ft {
        v.push((a / 1000.0).round() as u8);
    }
    v
}

/// An X.25 call request.
pub fn x25_call(lcn: u16, called: &str, calling: &str, cud: &[u8]) -> Vec<u8> {
    let mut v = vec![0x10 | ((lcn >> 8) as u8 & 0x0F), lcn as u8, 0x0B];
    v.push(((calling.len() as u8) << 4) | called.len() as u8);
    let digits: Vec<u8> = called
        .bytes()
        .chain(calling.bytes())
        .map(|c| c - b'0')
        .collect();
    for p in digits.chunks(2) {
        v.push(p[0] << 4 | p.get(1).copied().unwrap_or(0));
    }
    v.push(0);
    v.extend_from_slice(cud);
    v
}

/// An X.25 data packet carrying CLNP DT and a COTP DT TPDU around `app` octets.
pub fn x25_data_clnp(lcn: u16, ps: u8, pr: u8, app: &[u8]) -> Vec<u8> {
    let mut v = vec![
        0x10 | ((lcn >> 8) as u8 & 0x0F),
        lcn as u8,
        (pr & 7) << 5 | (ps & 7) << 1,
    ];
    let dst = [0x47u8, 0x00, 0x27, 0x81, 0x41, 0x45, 0x55];
    let src = [0x47u8, 0x00, 0x27, 0x81, 0x41, 0x46, 0x56];
    let li = 9 + 1 + dst.len() + 1 + src.len();
    let cotp = [2u8, 0xF0, 0x80];
    let total = li + cotp.len() + app.len();
    v.extend([
        0x81,
        li as u8,
        1,
        30,
        0x1C | 0x80,
        (total >> 8) as u8,
        total as u8,
        0,
        0,
    ]);
    v.push(dst.len() as u8);
    v.extend(dst);
    v.push(src.len() as u8);
    v.extend(src);
    v.extend(cotp);
    v.extend_from_slice(app);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{adsc_envelope, AdscReport};
    use sdrkit_proto_aero::acars::{BlockSpec, End};

    #[test]
    fn addresses_round_trip() {
        for t in [1u8, 4, 5, 7, 2] {
            for st in [false, true] {
                for end in [false, true] {
                    let o = write_addr(0x4CA2B1, t, st, end);
                    assert_eq!(o[3] & 1 != 0, end);
                    assert_eq!(read_addr(&o), (0x4CA2B1, t, st));
                }
            }
        }
    }

    #[test]
    fn an_adsc_frame() {
        let r = AdscReport {
            lat_deg: 51.5,
            lon_deg: -0.4,
            alt_ft: 36000.0,
            sec_past_hour: 100.0,
            track: None,
            route: None,
            meteo: None,
        };
        let env = adsc_envelope("TESTGND", "G-EUUA", "BA0117", &r).unwrap();
        let text = format!("M01ABA0117/{env}");
        let mut b = Vec::new();
        let c = BlockSpec::new('2', "G-EUUA", b'2', End::Etx)
            .label("B6")
            .text(&text)
            .body(&mut b)
            .unwrap();
        let mut raw = header(
            0x10A000,
            4,
            false,
            0x4CA2B1,
            1,
            false,
            i_control(2, 5, false),
        );
        raw.extend(acars_info(&b, c));
        let f = read(&raw).unwrap();
        assert_eq!(
            (f.src, f.src_type, f.dst, f.dst_type),
            (0x4CA2B1, 1, 0x10A000, 4)
        );
        assert_eq!((f.ns, f.nr), (Some(2), Some(5)));
        assert!(f.acars_crc_ok);
        let a = f.acars.as_ref().unwrap();
        assert_eq!(a.registration, "G-EUUA");
        assert_eq!(a.flight, "BA0117");
        let p = f.position.as_ref().unwrap();
        assert!((p.lat - 51.5).abs() < 1e-3 && (p.lon + 0.4).abs() < 1e-3);
        assert_eq!(p.alt_ft, Some(36000.0));
        assert_eq!(f.summary, "ACARS B6 ADS-C position report");
    }

    #[test]
    fn x25_and_clnp() {
        let mut raw = header(
            0x10A000,
            4,
            false,
            0x4CA2B1,
            1,
            false,
            i_control(3, 4, false),
        );
        raw.extend(x25_data_clnp(1, 3, 4, &[1, 2, 3, 4]));
        let f = read(&raw).unwrap();
        assert!(f.summary.contains("P(S) 3 P(R) 4"), "{}", f.summary);
        assert!(f.detail.iter().any(|l| l.starts_with("CLNP: DT")));
        assert!(f.detail.iter().any(|l| l.starts_with("COTP")));
        assert_eq!(f.proto, "COTP");
        let mut raw = header(
            0x10A000,
            4,
            false,
            0x4CA2B1,
            1,
            false,
            i_control(0, 0, false),
        );
        raw.extend(x25_call(1, "1234", "56789", &[0xC1, 0x00]));
        let f = read(&raw).unwrap();
        assert_eq!(f.summary, "X.25 call request, LCN 1");
        assert!(f
            .detail
            .iter()
            .any(|l| l.contains("called DTE 1234, calling DTE 56789")));
        assert_eq!(f.call_user_data, "C100 (ICAO SNDCF)");
    }

    #[test]
    fn xid_frames() {
        let mut raw = header(0xFFFFFF, 7, false, 0x10A000, 4, true, xid_control(false));
        raw.extend(xid_info(&[
            (0x01, vec![3]),
            (0xC1, b"EGLLEGKK".to_vec()),
            (0xC8, location(51.4, -0.5, None)),
        ]));
        let f = read(&raw).unwrap();
        assert_eq!(f.name, "GSIF");
        assert_eq!(f.airports, "EGLL EGKK");
        let p = f.position.unwrap();
        assert!((p.lat - 51.4).abs() < 1e-9 && (p.lon + 0.5).abs() < 1e-9);
        let mut raw = header(0x10A000, 4, false, 0x4CA2B1, 1, false, xid_control(true));
        raw.extend(xid_info(&[
            (0x01, vec![0]),
            (0x84, location(-33.9, 151.2, Some(37000.0))),
            (0x83, b"ZZZZ".to_vec()),
        ]));
        let f = read(&raw).unwrap();
        assert_eq!(f.name, "XID_CMD_LE");
        let p = f.position.unwrap();
        assert!(
            (p.lat + 33.9).abs() < 1e-9
                && (p.lon - 151.2).abs() < 1e-9
                && p.alt_ft == Some(37000.0)
        );
    }
}
