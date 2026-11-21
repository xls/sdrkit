//! SBAS L1 messages: the message check (preambles, type, CRC-24Q) and the content decoder of the `sbas` stack. The corrections
//! are decoded and shown, never applied. Bits are counted from 0 over the 250 message bits.
//!
//! Standard: RTCA DO-229, Appendix A.

use crate::system::System;
use sdrkit_util_bits::crc::catalog::CRC24_LTE_A;
use sdrkit_util_bits::HardBits;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

/// The three preambles, one per message in turn.
pub const PREAMBLES: [u8; 3] = [0x53, 0x9A, 0xC6];

/// Checks 250 bits as a message: a preamble, a type 0 to 63 (always true for 6 bits), and the CRC. Returns (type, preamble index).
pub fn check(bits: &[u8]) -> Option<(u8, usize)> {
    if bits.len() < 250 {
        return None;
    }
    let pre = HardBits::at(bits, 0).u(8) as u8;
    let idx = PREAMBLES.iter().position(|&p| p == pre)?;
    if CRC24_LTE_A.bits(&bits[..226]) != HardBits::at(bits, 226).u(24) as u32 {
        return None;
    }
    Some((HardBits::at(bits, 8).u(6) as u8, idx))
}

/// The UDRE variance (m²) of an index, or `None` for 14 (not monitored) and 15 (do not use).
pub fn udre_variance(i: u8) -> Option<f64> {
    const T: [f64; 14] = [
        0.052, 0.0924, 0.1444, 0.283, 0.4678, 0.8315, 1.2992, 1.8709, 2.5465, 3.326, 5.1968,
        20.787, 230.9661, 2078.695,
    ];
    T.get(i as usize).copied()
}

/// The GIVE variance (m²) of an index, or `None` for 15 (not monitored).
pub fn give_variance(i: u8) -> Option<f64> {
    const T: [f64; 15] = [
        0.0084, 0.0333, 0.0749, 0.1331, 0.2079, 0.2994, 0.4075, 0.5322, 0.6735, 0.8315, 1.1974,
        1.8709, 3.326, 20.787, 187.0826,
    ];
    T.get(i as usize).copied()
}

/// The fast-correction degradation factor ai (m/s²) of an index 0 to 15.
pub fn degradation_factor(i: u8) -> f64 {
    const T: [f64; 16] = [
        0.0, 0.00005, 0.00009, 0.00012, 0.00015, 0.0002, 0.0003, 0.00045, 0.0006, 0.0009, 0.0015,
        0.0021, 0.0027, 0.0033, 0.0046, 0.0058,
    ];
    T[(i & 15) as usize]
}

/// The satellite that a PRN mask bit (1 to 210) stands for, or `None` for a reserved bit.
pub fn mask_bit_sat(bit: u16) -> Option<(System, u16)> {
    match bit {
        1..=37 => Some((System::Gps, bit)),
        38..=61 => Some((System::Glonass, bit - 37)),
        120..=138 => Some((System::Sbas, bit)),
        193..=202 => Some((System::Qzss, bit)),
        _ => None,
    }
}

/// The name of a message type, or `None` for a type that this decoder does not decode.
pub fn type_label(t: u8) -> Option<&'static str> {
    Some(match t {
        0 => "Don't use (test)",
        1 => "PRN mask",
        2..=5 => "Fast corrections",
        6 => "Integrity information",
        7 => "Fast correction degradation",
        9 => "GEO navigation message",
        10 => "Degradation parameters",
        12 => "SBAS network time / UTC offset",
        17 => "GEO almanacs",
        18 => "Ionospheric grid point masks",
        24 => "Mixed fast / long-term",
        25 => "Long-term satellite corrections",
        26 => "Ionospheric delay corrections",
        27 => "SBAS service message",
        28 => "Clock-ephemeris covariance matrix",
        62 => "Internal test message",
        63 => "Null message",
        _ => return None,
    })
}

/// The name of a message type for people.
pub fn type_name(t: u8) -> String {
    match type_label(t) {
        Some(l) => l.into(),
        None => format!("type {t} (not decoded)"),
    }
}

/// A fast-correction slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FastSlot {
    /// Pseudorange correction, m (`None` when only a UDREI is held).
    pub prc: Option<f64>,
    pub udrei: u8,
    pub iodf: u8,
    pub iodp: u8,
    /// Signal time of the message, s.
    pub time: f64,
}

/// The PRN mask.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PrnMask {
    pub iodp: u8,
    /// The mask bit numbers (1 to 210) of the slots, in slot order.
    pub slots: Vec<u16>,
}

/// GEO navigation (type 9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GeoNav {
    pub iodn: u32,
    pub t0: f64,
    pub ura: u32,
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    pub acc: [f64; 3],
    pub af0: f64,
    pub af1: f64,
}

/// Type 12.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SbasTime {
    pub a1: f64,
    pub a0: f64,
    pub tot: f64,
    pub wnt: u32,
    pub dtls: i32,
    pub wnlsf: u32,
    pub dn: u32,
    pub dtlsf: i32,
    pub utc_id: u32,
    pub tow: u32,
    pub week: u32,
    pub glonass: bool,
}

/// One GEO almanac entry (type 17).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GeoAlmanac {
    pub data_id: u32,
    pub prn: u32,
    pub health: u32,
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    pub t0: f64,
}

/// A long-term correction of a slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LongTerm {
    pub slot: u32,
    pub iode: u32,
    pub dpos: [f64; 3],
    pub daf0: f64,
    pub velocity: bool,
    pub dvel: [f64; 3],
    pub daf1: f64,
    pub t0: f64,
    pub iodp: u32,
}

/// An ionospheric band: its mask and delays.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IonoBand {
    pub iodi: u8,
    pub nbands: u8,
    /// Grid point numbers (0 to 200) in the mask, in order.
    pub points: Vec<u16>,
    /// (delay m or None for not monitored, GIVEI) per masked point.
    pub delays: Vec<Option<(Option<f64>, u8)>>,
    pub blocks_seen: u16,
}

/// The ionosphere summary of a band.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IonoSummary {
    pub band: u8,
    pub iodi: u8,
    pub points: usize,
    pub blocks_needed: usize,
    pub blocks_seen: usize,
    pub values: usize,
    pub not_monitored: usize,
    pub min_m: f64,
    pub mean_m: f64,
    pub max_m: f64,
    pub worst_givei: u8,
}

/// Everything decoded from one SBAS satellite.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SbasSat {
    pub prn: u16,
    pub messages: u64,
    pub bad: u64,
    pub types_seen: u64,
    pub first_s: f64,
    pub last_s: f64,
    pub last_type: u8,
    pub fast_messages: u64,
    pub long_term_messages: u64,
    pub mask: Option<PrnMask>,
    pub fast: Vec<Option<FastSlot>>,
    pub iodf6: Option<[u8; 4]>,
    pub tlat: Option<f64>,
    pub ai: Option<(u8, Vec<u8>)>,
    pub geo: Option<GeoNav>,
    pub time: Option<SbasTime>,
    pub almanac: Vec<GeoAlmanac>,
    pub degradation: Option<BTreeMap<String, u32>>,
    pub iono: BTreeMap<u8, IonoBand>,
    pub long_term: BTreeMap<u32, LongTerm>,
    pub test_message: bool,
}

impl SbasSat {
    fn new(prn: u16) -> SbasSat {
        SbasSat {
            prn,
            fast: vec![None; 52],
            ..Default::default()
        }
    }
    /// Slots with a correction, of them usable (UDREI 0 to 13), not monitored (14), do not use (15).
    pub fn fast_summary(&self) -> (usize, usize, usize, usize) {
        let mut s = (0, 0, 0, 0);
        for f in self.fast.iter().flatten().filter(|f| f.prc.is_some()) {
            s.0 += 1;
            match f.udrei {
                0..=13 => s.1 += 1,
                14 => s.2 += 1,
                _ => s.3 += 1,
            }
        }
        s
    }
    /// The ionosphere summary per band.
    pub fn iono_summary(&self) -> Vec<IonoSummary> {
        self.iono
            .iter()
            .map(|(&band, b)| {
                let mut s = IonoSummary {
                    band,
                    iodi: b.iodi,
                    points: b.points.len(),
                    blocks_needed: b.points.len().div_ceil(15),
                    blocks_seen: b.blocks_seen.count_ones() as usize,
                    min_m: f64::INFINITY,
                    max_m: f64::NEG_INFINITY,
                    ..Default::default()
                };
                let mut sum = 0.0;
                for (d, g) in b.delays.iter().flatten() {
                    s.worst_givei = s.worst_givei.max(*g);
                    match d {
                        Some(m) if *g < 15 => {
                            s.values += 1;
                            sum += m;
                            s.min_m = s.min_m.min(*m);
                            s.max_m = s.max_m.max(*m);
                        }
                        _ => s.not_monitored += 1,
                    }
                }
                if s.values > 0 {
                    s.mean_m = sum / s.values as f64;
                } else {
                    s.min_m = 0.0;
                    s.max_m = 0.0;
                }
                s
            })
            .collect()
    }
}

/// One message line of the log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MessageLine {
    pub time_s: f64,
    pub prn: u16,
    pub msg_type: u8,
    pub text: String,
}

/// The content decoder: the state of every SBAS satellite heard, the message log and the totals.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SbasContent {
    pub sats: BTreeMap<u16, SbasSat>,
    pub log: VecDeque<MessageLine>,
    pub total: u64,
    pub bad: u64,
    pub per_type: BTreeMap<u8, u64>,
}

fn fu(b: &[u8], s: usize, l: usize) -> u32 {
    HardBits::at(b, s).u(l as u32) as u32
}
fn fs(b: &[u8], s: usize, l: usize) -> i64 {
    HardBits::at(b, s).i(l as u32)
}
fn p2(e: i32) -> f64 {
    2f64.powi(e)
}

/// The most message lines kept.
pub const LOG_CAP: usize = 200;

impl SbasContent {
    /// Forgets everything decoded (the satellites stay tracked by the receiver).
    pub fn clear(&mut self) {
        *self = SbasContent::default();
    }

    /// Counts a message lost from `prn` (a gap, or a failed CRC).
    pub fn bad_message(&mut self, prn: u16) {
        self.bad += 1;
        self.sats
            .entry(prn)
            .or_insert_with(|| SbasSat::new(prn))
            .bad += 1;
    }

    /// Decodes a message with a good CRC (250 bits) from `prn` received at signal time `t`. Returns its log line.
    pub fn add(&mut self, prn: u16, bits: &[u8], t: f64) -> String {
        let ty = fu(bits, 8, 6) as u8;
        self.total += 1;
        *self.per_type.entry(ty).or_default() += 1;
        let sat = self.sats.entry(prn).or_insert_with(|| SbasSat::new(prn));
        if sat.messages == 0 {
            sat.first_s = t;
        }
        sat.messages += 1;
        sat.last_s = t;
        sat.last_type = ty;
        sat.types_seen |= 1u64 << ty;
        let text = decode_into(sat, ty, bits, t);
        self.log.push_back(MessageLine {
            time_s: t,
            prn,
            msg_type: ty,
            text: text.clone(),
        });
        while self.log.len() > LOG_CAP {
            self.log.pop_front();
        }
        text
    }
}

/// Takes `n` corrections of a block laid out for `layout` (13 for types 2 to 5, 6 for type 24): PRCs then UDREIs.
#[allow(clippy::too_many_arguments)]
fn fast_block(
    sat: &mut SbasSat,
    first_slot: usize,
    n: usize,
    layout: usize,
    bits: &[u8],
    base: usize,
    iodf: u8,
    iodp: u8,
    t: f64,
) {
    for i in 0..n {
        let prc = fs(bits, base + 12 * i, 12) as f64 * 0.125;
        let udrei = fu(bits, base + 12 * layout + 4 * i, 4) as u8;
        let slot = first_slot + i;
        if slot < sat.fast.len() {
            sat.fast[slot] = Some(FastSlot {
                prc: Some(prc),
                udrei,
                iodf,
                iodp,
                time: t,
            });
        }
    }
}

fn long_term_half(sat: &mut SbasSat, b: &[u8], s: usize) -> String {
    sat.long_term_messages += 1;
    let code = b[s];
    if code == 0 {
        let iodp = fu(b, s + 103, 2);
        let mut parts = Vec::new();
        for k in 0..2 {
            let o = s + 1 + 51 * k;
            let slot = fu(b, o, 6);
            if slot == 0 {
                continue;
            }
            let lt = LongTerm {
                slot,
                iode: fu(b, o + 6, 8),
                dpos: [
                    fs(b, o + 14, 9) as f64 * 0.125,
                    fs(b, o + 23, 9) as f64 * 0.125,
                    fs(b, o + 32, 9) as f64 * 0.125,
                ],
                daf0: fs(b, o + 41, 10) as f64 * p2(-31),
                velocity: false,
                iodp,
                ..Default::default()
            };
            parts.push(format!(
                "slot {} IODE {} dx {:.3} dy {:.3} dz {:.3} m",
                slot, lt.iode, lt.dpos[0], lt.dpos[1], lt.dpos[2]
            ));
            sat.long_term.insert(slot, lt);
        }
        parts.join(", ")
    } else {
        let slot = fu(b, s + 1, 6);
        let lt = LongTerm {
            slot,
            iode: fu(b, s + 7, 8),
            dpos: [
                fs(b, s + 15, 11) as f64 * 0.125,
                fs(b, s + 26, 11) as f64 * 0.125,
                fs(b, s + 37, 11) as f64 * 0.125,
            ],
            daf0: fs(b, s + 48, 11) as f64 * p2(-31),
            velocity: true,
            dvel: [
                fs(b, s + 59, 8) as f64 * p2(-11),
                fs(b, s + 67, 8) as f64 * p2(-11),
                fs(b, s + 75, 8) as f64 * p2(-11),
            ],
            daf1: fs(b, s + 83, 8) as f64 * p2(-39),
            t0: fu(b, s + 91, 13) as f64 * 16.0,
            iodp: fu(b, s + 104, 2),
        };
        if slot != 0 {
            sat.long_term.insert(slot, lt);
        }
        format!("slot {slot} IODE {} with velocity", lt.iode)
    }
}

fn decode_into(sat: &mut SbasSat, ty: u8, b: &[u8], t: f64) -> String {
    match ty {
        0 | 2..=5 => {
            let iodf = fu(b, 14, 2) as u8;
            let iodp = fu(b, 16, 2) as u8;
            let blk = if ty == 0 { 0 } else { ty as usize - 2 };
            let n = if blk == 3 { 12 } else { 13 };
            // The block of type 5 has room for 13 but 51 slots exist; its last value is ignored.
            fast_block(sat, blk * 13 + 1, n, 13, b, 18, iodf, iodp, t);
            sat.fast_messages += 1;
            if ty == 0 {
                sat.test_message = true;
            }
            format!(
                "{}: slots {} to {}, IODF {iodf}, IODP {iodp}{}",
                type_name(ty),
                blk * 13 + 1,
                blk * 13 + n,
                if ty == 0 { " (test message)" } else { "" }
            )
        }
        1 => {
            let mut slots = Vec::new();
            for i in 0..210 {
                if b[14 + i] == 1 {
                    slots.push(i as u16 + 1);
                }
            }
            let iodp = fu(b, 224, 2) as u8;
            let n = slots.len();
            sat.mask = Some(PrnMask { iodp, slots });
            format!("PRN mask: {n} satellites, IODP {iodp}")
        }
        6 => {
            let iodfs = [
                fu(b, 14, 2) as u8,
                fu(b, 16, 2) as u8,
                fu(b, 18, 2) as u8,
                fu(b, 20, 2) as u8,
            ];
            sat.iodf6 = Some(iodfs);
            let mut applied = 0;
            for i in 0..51 {
                let udrei = fu(b, 22 + 4 * i, 4) as u8;
                let slot = i + 1;
                let want = iodfs[(i / 13).min(3)];
                match &mut sat.fast[slot] {
                    Some(f) if f.prc.is_some() => {
                        if f.iodf == want || want == 3 {
                            f.udrei = udrei;
                            applied += 1;
                        }
                    }
                    other => {
                        *other = Some(FastSlot {
                            prc: None,
                            udrei,
                            iodf: want,
                            iodp: 0,
                            time: t,
                        });
                    }
                }
            }
            format!("Integrity information: IODFs {iodfs:?}, {applied} UDREIs applied")
        }
        7 => {
            let tlat = fu(b, 14, 4) as f64;
            let iodp = fu(b, 18, 2) as u8;
            let ai: Vec<u8> = (0..51).map(|i| fu(b, 22 + 4 * i, 4) as u8).collect();
            sat.tlat = Some(tlat);
            sat.ai = Some((iodp, ai));
            format!("Fast correction degradation: latency {tlat} s, IODP {iodp}")
        }
        9 => {
            let g = GeoNav {
                iodn: fu(b, 14, 8),
                t0: fu(b, 22, 13) as f64 * 16.0,
                ura: fu(b, 35, 4),
                pos: [
                    fs(b, 39, 30) as f64 * 0.08,
                    fs(b, 69, 30) as f64 * 0.08,
                    fs(b, 99, 25) as f64 * 0.4,
                ],
                vel: [
                    fs(b, 124, 17) as f64 * 0.000625,
                    fs(b, 141, 17) as f64 * 0.000625,
                    fs(b, 158, 18) as f64 * 0.004,
                ],
                acc: [
                    fs(b, 176, 10) as f64 * 0.0000125,
                    fs(b, 186, 10) as f64 * 0.0000125,
                    fs(b, 196, 10) as f64 * 0.0000625,
                ],
                af0: fs(b, 206, 12) as f64 * p2(-31),
                af1: fs(b, 218, 8) as f64 * p2(-40),
            };
            let r = (g.pos[0].powi(2) + g.pos[1].powi(2) + g.pos[2].powi(2)).sqrt();
            let lon = g.pos[1].atan2(g.pos[0]).to_degrees();
            sat.geo = Some(g);
            format!(
                "GEO navigation: IODN {}, t0 {} s, radius {:.0} m, longitude {:.2} deg",
                g.iodn, g.t0, r, lon
            )
        }
        10 => {
            let names: [(&str, usize); 16] = [
                ("brrc", 10),
                ("cltc_lsb", 10),
                ("cltc_v1", 10),
                ("iltc_v1", 9),
                ("cltc_v0", 10),
                ("iltc_v0", 9),
                ("cgeo_lsb", 10),
                ("cgeo_v", 10),
                ("igeo", 9),
                ("cer", 6),
                ("ciono_step", 10),
                ("iiono", 9),
                ("ciono_ramp", 10),
                ("rss_udre", 1),
                ("rss_iono", 1),
                ("ccovariance", 7),
            ];
            let mut m = BTreeMap::new();
            let mut s = 14;
            for (n, l) in names {
                m.insert(n.to_string(), fu(b, s, l));
                s += l;
            }
            let text = format!(
                "Degradation parameters: Brrc {}, Cer {}, Ciono_ramp {}",
                m["brrc"], m["cer"], m["ciono_ramp"]
            );
            sat.degradation = Some(m);
            text
        }
        12 => {
            let tm = SbasTime {
                a1: fs(b, 14, 24) as f64 * p2(-50),
                a0: fs(b, 38, 32) as f64 * p2(-30),
                tot: fu(b, 70, 8) as f64 * 4096.0,
                wnt: fu(b, 78, 8),
                dtls: fs(b, 86, 8) as i32,
                wnlsf: fu(b, 94, 8),
                dn: fu(b, 102, 8),
                dtlsf: fs(b, 110, 8) as i32,
                utc_id: fu(b, 118, 3),
                tow: fu(b, 121, 20),
                week: fu(b, 141, 10),
                glonass: b[151] == 1,
            };
            sat.time = Some(tm);
            format!(
                "Network time: week {}, TOW {}, leap seconds {} (next {}), UTC identifier {}",
                tm.week, tm.tow, tm.dtls, tm.dtlsf, tm.utc_id
            )
        }
        17 => {
            let t0 = fu(b, 215, 11) as f64 * 64.0;
            sat.almanac.clear();
            for k in 0..3 {
                let o = 14 + 67 * k;
                let prn = fu(b, o + 2, 8);
                if prn == 0 {
                    continue;
                }
                sat.almanac.push(GeoAlmanac {
                    data_id: fu(b, o, 2),
                    prn,
                    health: fu(b, o + 10, 8),
                    pos: [
                        fs(b, o + 18, 15) as f64 * 2600.0,
                        fs(b, o + 33, 15) as f64 * 2600.0,
                        fs(b, o + 48, 9) as f64 * 26_000.0,
                    ],
                    vel: [
                        fs(b, o + 57, 3) as f64 * 10.0,
                        fs(b, o + 60, 3) as f64 * 10.0,
                        fs(b, o + 63, 4) as f64 * 60.0,
                    ],
                    t0,
                });
            }
            let prns: Vec<String> = sat.almanac.iter().map(|a| a.prn.to_string()).collect();
            format!("GEO almanacs: PRN {}", prns.join(", "))
        }
        18 => {
            let nb = fu(b, 14, 4) as u8;
            let band = fu(b, 18, 4) as u8;
            let iodi = fu(b, 22, 2) as u8;
            if band > 10 {
                return format!("Ionospheric grid point mask: band {band} (not defined)");
            }
            let points: Vec<u16> = (0..201)
                .filter(|&i| b[24 + i] == 1)
                .map(|i| i as u16)
                .collect();
            let n = points.len();
            let e = sat.iono.entry(band).or_default();
            if e.iodi != iodi || e.nbands != nb || e.points != points {
                e.delays = vec![None; n];
                e.blocks_seen = 0;
            }
            e.iodi = iodi;
            e.nbands = nb;
            e.points = points;
            format!("Ionospheric grid point mask: band {band} of {nb}, IODI {iodi}, {n} points")
        }
        24 => {
            let iodp = fu(b, 110, 2) as u8;
            let blk = fu(b, 112, 2) as usize;
            let iodf = fu(b, 114, 2) as u8;
            fast_block(sat, blk * 13 + 1, 6, 6, b, 14, iodf, iodp, t);
            sat.fast_messages += 1;
            let lt = long_term_half(sat, b, 120);
            format!("Mixed fast / long-term: block {blk}, IODF {iodf}, IODP {iodp}; {lt}")
        }
        25 => {
            let a = long_term_half(sat, b, 14);
            let c = long_term_half(sat, b, 120);
            format!("Long-term corrections: {a}; {c}")
        }
        26 => {
            let band = fu(b, 14, 4) as u8;
            let block = fu(b, 18, 4) as usize;
            let iodi = fu(b, 217, 2) as u8;
            let Some(e) = sat.iono.get_mut(&band).filter(|e| e.iodi == iodi) else {
                return format!("Ionospheric delays: band {band} block {block}, IODI {iodi} (no matching mask, not taken)");
            };
            let mut taken = 0;
            for i in 0..15 {
                let idx = block * 15 + i;
                if idx >= e.points.len() {
                    break;
                }
                let raw = fu(b, 22 + 13 * i, 9);
                let givei = fu(b, 31 + 13 * i, 4) as u8;
                let d = (raw != 511).then_some(raw as f64 * 0.125);
                e.delays[idx] = Some((d, givei));
                taken += 1;
            }
            if block < 16 {
                e.blocks_seen |= 1 << block;
            }
            format!("Ionospheric delays: band {band} block {block}, IODI {iodi}, {taken} points")
        }
        27 | 28 | 62 | 63 => type_name(ty),
        _ => type_name(ty),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_util_bits::pack::put_u64;

    #[test]
    fn sbas_tables() {
        assert_eq!(udre_variance(0), Some(0.052));
        assert_eq!(udre_variance(13), Some(2078.695));
        assert_eq!(udre_variance(14).unwrap_or(0.0), 0.0);
        assert_eq!(give_variance(14), Some(187.0826));
        assert_eq!(give_variance(15).unwrap_or(0.0), 0.0);
        assert_eq!(degradation_factor(15), 0.0058);
        assert_eq!(mask_bit_sat(5), Some((System::Gps, 5)));
        assert_eq!(mask_bit_sat(38), Some((System::Glonass, 1)));
        assert_eq!(mask_bit_sat(131), Some((System::Sbas, 131)));
        assert_eq!(mask_bit_sat(195), Some((System::Qzss, 195)));
        assert_eq!(mask_bit_sat(100), None);
        assert_eq!(udre_variance(15), None);
    }

    fn message(pre: usize, ty: u8, fill: impl Fn(&mut [u8])) -> Vec<u8> {
        let mut m = vec![0u8; 250];
        put_u64(&mut m, 0, PREAMBLES[pre] as u64, 8).unwrap();
        put_u64(&mut m, 8, ty as u64, 6).unwrap();
        fill(&mut m);
        let c = CRC24_LTE_A.bits(&m[..226]);
        put_u64(&mut m, 226, c as u64, 24).unwrap();
        m
    }

    #[test]
    fn crc_and_preambles() {
        let m = message(0, 63, |_| {});
        assert_eq!(check(&m), Some((63, 0)));
        let mut bad = m.clone();
        bad[100] ^= 1;
        assert_eq!(check(&bad), None);
        assert_eq!(check(&message(1, 63, |_| {})), Some((63, 1)));
        assert_eq!(check(&message(2, 63, |_| {})), Some((63, 2)));
    }

    #[test]
    fn iono_needs_its_mask() {
        let mut c = SbasContent::default();
        let t26 = message(0, 26, |m| {
            for i in 0..15 {
                put_u64(m, 22 + 13 * i, 40 + i as u64, 9).unwrap();
            }
        });
        c.add(130, &t26, 0.0);
        assert!(c.sats[&130].iono.is_empty());
        let t18 = message(1, 18, |m| {
            put_u64(m, 14, 2, 4).unwrap();
            for i in 0..30 {
                m[24 + i] = 1;
            }
        });
        c.add(130, &t18, 1.0);
        c.add(130, &t26, 2.0);
        let s = &c.sats[&130].iono_summary()[0];
        assert_eq!((s.points, s.values, s.blocks_needed), (30, 15, 2));
        // All ones with GIVEI 15: not monitored.
        let t26b = message(2, 26, |m| {
            put_u64(m, 18, 1, 4).unwrap();
            for i in 0..15 {
                put_u64(m, 22 + 13 * i, 511, 9).unwrap();
                put_u64(m, 31 + 13 * i, 15, 4).unwrap();
            }
        });
        c.add(130, &t26b, 3.0);
        let s = &c.sats[&130].iono_summary()[0];
        assert_eq!((s.values, s.not_monitored), (15, 15));
    }
}
