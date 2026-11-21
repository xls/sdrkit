//! The Bearer Control layer of the BGAN forward link (ETSI TS 102 744-3-1): PDU headers and CRC (5.1.4, 5.1.7), SDUs (5.3),
//! the broadcast SDUs (BulletinBoard 5.4.3, ReturnSchedule 5.4.4, SpotBeamMap 5.4.10, GPSEphemeris 5.4.12) and the AVPs (5.7).
//! Only broadcast PDUs are read. PDUs addressed to a terminal and connection PDUs are counted; nothing of them is kept.
//! The writers make the PDUs of the test signal.

use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
use sdrkit_util_bits::reader::{BitReader, BitWriter};

/// The most content octets of a PDU without a length field.
pub const MAX_CONTENT: usize = 256;

/// SDU type numbers towards the terminal (5.3.2.4).
pub mod sdu_type {
    /// A connection PDU.
    pub const CONN_PDU: u8 = 0;
    /// BulletinBoard.
    pub const BULLETIN: u8 = 1;
    /// ReturnSchedule.
    pub const RETURN_SCHEDULE: u8 = 2;
    /// StatusAckList.
    pub const STATUS_ACK: u8 = 3;
    /// SpecificAVPList.
    pub const SPECIFIC_AVP: u8 = 6;
    /// AVPList.
    pub const AVP_LIST: u8 = 7;
    /// SpotBeamMap.
    pub const SPOT_BEAM_MAP: u8 = 8;
    /// BearerTables.
    pub const BEARER_TABLES: u8 = 9;
    /// GPSEphemeris.
    pub const GPS_EPHEMERIS: u8 = 10;
    /// SystemInfoIndex.
    pub const SYSTEM_INFO_INDEX: u8 = 12;
    /// BearerTableUpdate.
    pub const BEARER_TABLE_UPDATE: u8 = 13;
}

/// The name of SDU type `t`.
pub fn sdu_name(t: u8) -> &'static str {
    match t {
        0 => "connection PDU",
        1 => "bulletin board",
        2 => "return schedule",
        3 => "status ack list",
        6 => "specific AVP list",
        7 => "AVP list",
        8 => "spot beam map",
        9 => "bearer tables",
        10 => "GPS ephemeris",
        12 => "system info index",
        13 => "bearer table update",
        _ => "reserved",
    }
}

/// One entry of a ForwardBearerCodeRate AVP (5.7.16).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeRate {
    /// The block uses the higher modulation (16-QAM on a bearer with dynamic modulation).
    pub higher: bool,
    /// The block number (8 added when it is not above the previous one).
    pub block: u8,
    /// The coding rate step from R, -8 to 7 (table 5.20).
    pub rate: i8,
}

/// An AVP of a broadcast list (5.7).
#[derive(Clone, Debug, PartialEq)]
pub enum Avp {
    /// SatelliteLocation (A9, AD): the satellite id, the longitude in 0.1 degree east, and the primary and alternate forward
    /// channel numbers of AD.
    SatelliteLocation {
        /// Satellite id.
        sat: u8,
        /// Longitude, 0.1 degree, east positive.
        lon: i16,
        /// Primary and alternate forward channel numbers.
        channels: Option<(u16, u16)>,
    },
    /// BeamInfo (21): beam type (0 global, 1 regional, 2 narrow) and the current spot beam.
    BeamInfo {
        /// Beam type.
        beam_type: u8,
        /// Spot beam id.
        spot: u8,
    },
    /// PLMNInfo (F2, F3).
    Plmn {
        /// The mobile country code.
        mcc: String,
        /// The mobile network code.
        mnc: String,
        /// The PLMN index of F3.
        index: Option<u8>,
    },
    /// PrimaryBearer (1C to 1F).
    PrimaryBearer {
        /// Spot beam id.
        spot: u8,
        /// Forward channel number.
        channel: u16,
        /// f-bearer.
        f_bearer: u8,
        /// FBearerType.
        bearer_type: u8,
        /// PLMN index when included.
        plmn_index: Option<u8>,
    },
    /// UTCDateAndTime (D4): the GPS time of frame number 0.
    UtcTime {
        /// Year.
        year: u16,
        /// Month.
        month: u8,
        /// Day.
        day: u8,
        /// Hours.
        hour: u8,
        /// Minutes.
        minute: u8,
        /// Seconds.
        second: u8,
        /// Hundredths of a second.
        centis: u8,
    },
    /// SatelliteStateVectors (AC): position in units of 488 m and the interval in units of 512 frames.
    StateVectors {
        /// x, y, z.
        xyz: [i32; 3],
        /// Interval.
        interval: u8,
    },
    /// ForwardBearerCodeRate (90 to 97).
    CodeRates(Vec<CodeRate>),
    /// AccessControl (AA).
    AccessControl,
    /// NASSystemInfo (F8 to FF) with its length.
    NasSystemInfo(usize),
    /// SubbandCentreFrequencyOffsets (19): forward and return offsets in units of 1.25 kHz.
    SubbandOffsets {
        /// Forward.
        forward: u8,
        /// Return.
        ret: u8,
    },
    /// Any other AVP: its type octet and length.
    Other {
        /// The type octet.
        code: u8,
        /// Value octets.
        len: usize,
    },
}

/// A BulletinBoard (5.4.3).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bulletin {
    /// rnc-id.
    pub rnc: u8,
    /// net-ver.
    pub net_ver: u8,
    /// The 12-bit frame number.
    pub frame: u16,
    /// f-bearer.
    pub f_bearer: u8,
    /// bct-id.
    pub bct_id: u8,
    /// The spot beam when present.
    pub spot: Option<u8>,
    /// The AVPs that follow.
    pub avps: Vec<Avp>,
}

/// A broadcast SDU as read.
#[derive(Clone, Debug, PartialEq)]
pub enum Sdu {
    /// BulletinBoard.
    Bulletin(Bulletin),
    /// ReturnSchedule: the return channel number and the slot plans.
    ReturnSchedule {
        /// Return channel number.
        channel: u16,
        /// The 4-bit slot plans.
        plans: Vec<u8>,
    },
    /// AVPList.
    AvpList(Vec<Avp>),
    /// SpotBeamMap header.
    SpotBeamMap {
        /// Version.
        version: u8,
        /// Map number.
        number: u8,
        /// Total number of spots.
        spots: u16,
    },
    /// BearerTables: its size.
    BearerTables(usize),
    /// GPSEphemeris: the PRN.
    GpsEphemeris {
        /// PRN code.
        prn: u8,
    },
    /// SystemInfoIndex: the items it lists.
    SystemInfoIndex(Vec<u8>),
    /// Another type: its number and size (counted, not read).
    Other {
        /// SDU type.
        kind: u8,
        /// Payload octets.
        len: usize,
    },
}

/// What one FEC block held.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlockContent {
    /// Only zero fill.
    pub empty: bool,
    /// PDUs whose CRC checked.
    pub pdus_ok: u32,
    /// PDUs whose CRC failed (the rest of the block is skipped).
    pub pdus_bad: u32,
    /// Broadcast PDUs.
    pub broadcast: u32,
    /// Connection PDUs (tbcn-id).
    pub connection: u32,
    /// PDUs addressed to a terminal (common-signalling or extended addresses).
    pub addressed: u32,
    /// The broadcast SDUs read.
    pub sdus: Vec<Sdu>,
}

/// The header of a PDU at the start of `b`: (header octets, content octets or `None` to fill, the PDU kind). `None` when the
/// form cannot be measured (a common-signalling address of unknown length, the reserved address type).
fn header(b: &[u8]) -> Option<(usize, Option<usize>, PduKind)> {
    let h = *b.first()?;
    let len_present = h >> 6 & 1 == 1;
    let addr = h >> 4 & 3;
    let (hl, kind) = match addr {
        0 => (1, PduKind::Broadcast),
        1 => (2, PduKind::Connection),
        2 if h & 0x08 == 0 => (4, PduKind::Extended),
        _ => return None,
    };
    if len_present {
        let l = *b.get(hl)? as usize;
        Some((hl + 1, Some(l), kind))
    } else {
        Some((hl, None, kind))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PduKind {
    Broadcast,
    Connection,
    Extended,
}

/// The extent of the PDU at the start of `b`: (header octets, content octets), the CRC after them; `None` when it cannot be
/// measured or does not fit.
fn extent(b: &[u8]) -> Option<(usize, usize, PduKind)> {
    let (hl, l, kind) = header(b)?;
    let content = match l {
        Some(l) => l,
        None => b.len().checked_sub(hl + 2)?.min(MAX_CONTENT),
    };
    (hl + content + 2 <= b.len()).then_some((hl, content, kind))
}

/// The CRC of a PDU (5.1.7): CRC-16/IBM-SDLC over the header and content, sent low octet first.
pub fn pdu_check(pdu: &[u8]) -> bool {
    let n = pdu.len();
    n >= 3 && CRC16_IBM_SDLC.checksum(&pdu[..n - 2]) == u16::from_le_bytes([pdu[n - 2], pdu[n - 1]])
}

/// The stop test of the turbo decoder: the octets are zero fill, or begin with a PDU whose CRC checks.
pub fn block_check(b: &[u8]) -> bool {
    if b.iter().all(|&x| x == 0) {
        return true;
    }
    extent(b).is_some_and(|(hl, c, _)| pdu_check(&b[..hl + c + 2]))
}

/// Reads the PDUs of a decoded block.
pub fn read_block(b: &[u8]) -> BlockContent {
    let mut out = BlockContent {
        empty: b.iter().all(|&x| x == 0),
        ..Default::default()
    };
    let mut at = 0;
    while at < b.len() && b[at..].iter().any(|&x| x != 0) {
        let rest = &b[at..];
        if rest[0] >> 4 & 3 == 2 && rest[0] & 0x08 != 0 {
            // A common-signalling address: counted, and the rest of the block skipped.
            out.addressed += 1;
            break;
        }
        let Some((hl, content, kind)) = extent(rest) else {
            out.pdus_bad += 1;
            break;
        };
        let pdu = &rest[..hl + content + 2];
        if !pdu_check(pdu) {
            out.pdus_bad += 1;
            break;
        }
        out.pdus_ok += 1;
        match kind {
            PduKind::Broadcast => {
                out.broadcast += 1;
                if rest[0] & 0x80 != 0 {
                    read_sdus(&pdu[hl..hl + content], &mut out.sdus);
                }
            }
            PduKind::Connection => out.connection += 1,
            PduKind::Extended => out.addressed += 1,
        }
        at += pdu.len();
    }
    out
}

/// Reads the chain of SDUs of a broadcast PDU's content.
fn read_sdus(mut b: &[u8], out: &mut Vec<Sdu>) {
    while let Some(&h) = b.first() {
        let more = h & 0x80 != 0;
        let (kind, len, hl) = if h & 0x40 != 0 {
            let Some(&l) = b.get(1) else { return };
            (h & 0x3F, l as usize + 1, 2)
        } else {
            (h >> 3 & 7, (h & 7) as usize + 1, 1)
        };
        let Some(p) = b.get(hl..hl + len) else {
            return;
        };
        out.push(read_sdu(kind, p));
        b = &b[hl + len..];
        if !more {
            return;
        }
    }
}

/// Reads one broadcast SDU of type `kind` from its payload.
pub fn read_sdu(kind: u8, p: &[u8]) -> Sdu {
    let mut r = BitReader::new(p);
    match kind {
        sdu_type::BULLETIN if p.len() >= 4 => {
            let rnc = r.u(8) as u8;
            let net_ver = r.u(4) as u8;
            let frame = r.u(12) as u16;
            let present = r.bit();
            let f_bearer = r.u(3) as u8;
            let bct_id = r.u(4) as u8;
            let spot = (present && p.len() >= 5).then(|| r.u(8) as u8);
            let used = if spot.is_some() { 5 } else { 4 };
            Sdu::Bulletin(Bulletin {
                rnc,
                net_ver,
                frame,
                f_bearer,
                bct_id,
                spot,
                avps: read_avps(&p[used..]),
            })
        }
        sdu_type::RETURN_SCHEDULE if p.len() >= 3 => {
            let channel = r.u(16) as u16;
            let type_included = r.bit();
            let _res_plan = r.bit();
            let n = r.u(2) as usize + 1;
            if type_included {
                r.skip(8);
            }
            let plans = (0..n).map(|_| r.u(4) as u8).collect();
            Sdu::ReturnSchedule { channel, plans }
        }
        sdu_type::AVP_LIST => Sdu::AvpList(read_avps(p)),
        sdu_type::SPOT_BEAM_MAP if p.len() >= 4 => Sdu::SpotBeamMap {
            version: p[0],
            number: p[1],
            spots: u16::from_be_bytes([p[2], p[3]]),
        },
        sdu_type::BEARER_TABLES => Sdu::BearerTables(p.len()),
        sdu_type::GPS_EPHEMERIS if p.len() >= 3 => Sdu::GpsEphemeris { prn: p[2] },
        sdu_type::SYSTEM_INFO_INDEX => Sdu::SystemInfoIndex(p.to_vec()),
        _ => Sdu::Other {
            kind,
            len: p.len(),
        },
    }
}

/// Reads a list of AVPs; a truncated last AVP is dropped.
pub fn read_avps(mut b: &[u8]) -> Vec<Avp> {
    let mut out = Vec::new();
    while let Some(&code) = b.first() {
        let len = (code & 7) as usize + 1;
        let Some(v) = b.get(1..1 + len) else { break };
        out.push(read_avp(code, v));
        b = &b[1 + len..];
    }
    out
}

/// Reads one AVP of type octet `code` from its value.
pub fn read_avp(code: u8, v: &[u8]) -> Avp {
    let mut r = BitReader::new(v);
    let digit = |d: u8| char::from_digit(u32::from(d), 10).unwrap_or('?');
    match code {
        0xA9 | 0xAD => {
            let sat = r.u(4) as u8;
            let lon = r.i(12) as i16;
            let channels = (code == 0xAD).then(|| (r.u(16) as u16, r.u(16) as u16));
            Avp::SatelliteLocation { sat, lon, channels }
        }
        0x21 => {
            r.skip(5);
            Avp::BeamInfo {
                beam_type: r.u(3) as u8,
                spot: r.u(8) as u8,
            }
        }
        0xF2 | 0xF3 => {
            let d: Vec<u8> = v[..3].iter().flat_map(|&o| [o >> 4, o & 15]).collect();
            let mcc: String = [d[0], d[1], d[2]].iter().map(|&x| digit(x)).collect();
            let mut mnc: String = [d[3], d[4]].iter().map(|&x| digit(x)).collect();
            if d[5] != 0xF {
                mnc.push(digit(d[5]));
            }
            let index = (code == 0xF3).then(|| v[3] >> 4);
            Avp::Plmn { mcc, mnc, index }
        }
        0x1C..=0x1F => {
            let spot = r.u(8) as u8;
            let channel = r.u(16) as u16;
            let inc = r.bit();
            let f_bearer = r.u(3) as u8;
            let bearer_type = r.u(4) as u8;
            let plmn_index = inc.then(|| r.u(8) as u8);
            Avp::PrimaryBearer {
                spot,
                channel,
                f_bearer,
                bearer_type,
                plmn_index,
            }
        }
        0xD4 => Avp::UtcTime {
            year: 2000 + r.u(7) as u16,
            month: r.u(4) as u8,
            day: r.u(5) as u8,
            hour: r.u(5) as u8,
            minute: r.u(6) as u8,
            second: r.u(6) as u8,
            centis: r.u(7) as u8,
        },
        0xAC => Avp::StateVectors {
            xyz: [r.i(11) as i32, r.i(11) as i32, r.i(14) as i32],
            interval: r.u(4) as u8,
        },
        0x90..=0x97 => {
            let mut prev: Option<u8> = None;
            let mut add = 0u8;
            let list = v
                .iter()
                .map(|&o| {
                    let raw = o >> 4 & 7;
                    if prev.is_some_and(|p| raw <= p) {
                        add = 8;
                    }
                    prev = Some(raw);
                    CodeRate {
                        higher: o & 0x80 != 0,
                        block: raw + add,
                        rate: ((o & 15) as i8) << 4 >> 4,
                    }
                })
                .collect();
            Avp::CodeRates(list)
        }
        0xAA => Avp::AccessControl,
        0xF8..=0xFF => Avp::NasSystemInfo(v.len()),
        0x19 => Avp::SubbandOffsets {
            forward: v[0],
            ret: v[1],
        },
        _ => Avp::Other { code, len: v.len() },
    }
}

/// The name of an AVP.
pub fn avp_name(a: &Avp) -> &'static str {
    match a {
        Avp::SatelliteLocation { .. } => "satellite location",
        Avp::BeamInfo { .. } => "beam info",
        Avp::Plmn { .. } => "PLMN info",
        Avp::PrimaryBearer { .. } => "primary bearer",
        Avp::UtcTime { .. } => "UTC date and time",
        Avp::StateVectors { .. } => "satellite state vectors",
        Avp::CodeRates(_) => "forward bearer code rate",
        Avp::AccessControl => "access control",
        Avp::NasSystemInfo(_) => "NAS system info",
        Avp::SubbandOffsets { .. } => "sub-band centre offsets",
        Avp::Other { .. } => "AVP",
    }
}

/// The beam type name (0 global, 1 regional, 2 narrow).
pub fn beam_type_name(t: u8) -> &'static str {
    match t {
        0 => "global",
        1 => "regional",
        2 => "narrow",
        _ => "reserved",
    }
}

// ----------------------------------------------------------------------------------------------------------------------------
// Writers (the test signal)
// ----------------------------------------------------------------------------------------------------------------------------

/// An AVP of type number `kind` (5 bits) with `value` (1 to 8 octets).
pub fn avp(kind: u8, value: &[u8]) -> Vec<u8> {
    let mut v = vec![(kind << 3) | (value.len().clamp(1, 8) as u8 - 1)];
    v.extend_from_slice(value);
    v
}

/// SatelliteLocation A9 of satellite `sat` at `lon` tenths of a degree east.
pub fn avp_satellite(sat: u8, lon: i16) -> Vec<u8> {
    let mut v = Vec::new();
    let mut w = BitWriter::new(&mut v);
    w.put(u64::from(sat & 15), 4);
    w.put_signed(i64::from(lon), 12);
    avp(0xA9 >> 3, &v)
}

/// BeamInfo of beam type `t` and spot `spot`.
pub fn avp_beam(t: u8, spot: u8) -> Vec<u8> {
    avp(0x21 >> 3, &[t & 7, spot])
}

/// PLMNInfo F2 of a three-digit MCC and a two- or three-digit MNC.
pub fn avp_plmn(mcc: &str, mnc: &str) -> Vec<u8> {
    let d = |s: &str, i: usize| s.as_bytes().get(i).map_or(0xF, |c| c.wrapping_sub(b'0') & 15);
    avp(
        0xF2 >> 3,
        &[
            d(mcc, 0) << 4 | d(mcc, 1),
            d(mcc, 2) << 4 | d(mnc, 0),
            d(mnc, 1) << 4 | d(mnc, 2),
        ],
    )
}

/// PrimaryBearer (1C, 5 octets) of a spot, a forward channel, f-bearer and FBearerType, with a class control octet.
pub fn avp_primary(spot: u8, channel: u16, f_bearer: u8, bearer_type: u8) -> Vec<u8> {
    let c = channel.to_be_bytes();
    avp(
        0x1C >> 3,
        &[spot, c[0], c[1], (f_bearer & 7) << 4 | (bearer_type & 15), 0],
    )
}

/// UTCDateAndTime D4.
pub fn avp_time(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8, centis: u8) -> Vec<u8> {
    let mut v = Vec::new();
    let mut w = BitWriter::new(&mut v);
    for (x, n) in [
        (u64::from(year.saturating_sub(2000)), 7),
        (u64::from(month), 4),
        (u64::from(day), 5),
        (u64::from(hour), 5),
        (u64::from(minute), 6),
        (u64::from(second), 6),
        (u64::from(centis), 7),
    ] {
        w.put(x, n);
    }
    avp(0xD4 >> 3, &v)
}

/// ForwardBearerCodeRate with one octet per entry.
pub fn avp_code_rates(list: &[CodeRate]) -> Vec<u8> {
    let v: Vec<u8> = list
        .iter()
        .take(8)
        .map(|c| u8::from(c.higher) << 7 | (c.block & 7) << 4 | (c.rate as u8 & 15))
        .collect();
    avp(0x90 >> 3, &v)
}

/// An SDU of `kind` with `payload`: the short form for types 0 to 7 with 1 to 8 octets, else the long form. `more` sets the
/// bit that announces another SDU.
pub fn sdu(kind: u8, payload: &[u8], more: bool) -> Vec<u8> {
    let m = u8::from(more) << 7;
    let mut v = if kind < 8 && (1..=8).contains(&payload.len()) {
        vec![m | kind << 3 | (payload.len() as u8 - 1)]
    } else {
        vec![m | 0x40 | (kind & 0x3F), (payload.len().clamp(1, 256) - 1) as u8]
    };
    v.extend_from_slice(payload);
    v
}

/// A BulletinBoard payload with its AVPs.
pub fn bulletin(b: &Bulletin, avps: &[Vec<u8>]) -> Vec<u8> {
    let mut v = Vec::new();
    let mut w = BitWriter::new(&mut v);
    w.put(u64::from(b.rnc), 8);
    w.put(u64::from(b.net_ver & 15), 4);
    w.put(u64::from(b.frame & 0xFFF), 12);
    w.put(u64::from(b.spot.is_some()), 1);
    w.put(u64::from(b.f_bearer & 7), 3);
    w.put(u64::from(b.bct_id & 15), 4);
    if let Some(s) = b.spot {
        w.put(u64::from(s), 8);
    }
    for a in avps {
        v.extend_from_slice(a);
    }
    v
}

/// A ReturnSchedule payload: the return channel and up to four slot plans.
pub fn return_schedule(channel: u16, plans: &[u8]) -> Vec<u8> {
    let plans = &plans[..plans.len().clamp(1, 4)];
    let mut v = Vec::new();
    let mut w = BitWriter::new(&mut v);
    w.put(u64::from(channel), 16);
    w.put(0, 1);
    w.put(0, 1);
    w.put(plans.len() as u64 - 1, 2);
    for &p in plans {
        w.put(u64::from(p & 15), 4);
    }
    v
}

/// A broadcast PDU with a length field holding `content` (the SDUs), with its CRC.
pub fn broadcast_pdu(content: &[u8]) -> Vec<u8> {
    let mut v = vec![0xC9, content.len().min(255) as u8];
    v.extend_from_slice(&content[..content.len().min(255)]);
    let c = CRC16_IBM_SDLC.checksum(&v);
    v.extend_from_slice(&c.to_le_bytes());
    v
}

/// A connection PDU to `tbcn` (12 bits) with a length field and `content`, with its CRC.
pub fn connection_pdu(tbcn: u16, content: &[u8]) -> Vec<u8> {
    let mut v = vec![
        0x50 | (tbcn >> 8 & 15) as u8,
        tbcn as u8,
        content.len().min(255) as u8,
    ];
    v.extend_from_slice(&content[..content.len().min(255)]);
    let c = CRC16_IBM_SDLC.checksum(&v);
    v.extend_from_slice(&c.to_le_bytes());
    v
}
