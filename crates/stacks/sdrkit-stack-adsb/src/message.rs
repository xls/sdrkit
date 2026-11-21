//! Mode S downlink messages: the header fields of every downlink format and the type codes of the extended squitter ME field.
//! Comm-B registers are in [`crate::commb`].
//!
//! Standard: ICAO Annex 10 Vol. IV 3.1.2; RTCA DO-260B 2.2.3

use crate::codes::{ac12_ft, ac13_ft, callsign, squawk};
use crate::crc;
use sdrkit_util_geo::cpr::CprFrame;

/// A frame as received: 7 or 14 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Frame {
    /// The bytes (the first `len()` are used).
    pub bytes: [u8; 14],
    /// 56 or 112.
    pub bits: u8,
}

impl Frame {
    /// A frame from its bytes (7 or 14).
    pub fn new(b: &[u8]) -> Option<Frame> {
        if b.len() != 7 && b.len() != 14 {
            return None;
        }
        let mut bytes = [0u8; 14];
        bytes[..b.len()].copy_from_slice(b);
        Some(Frame {
            bytes,
            bits: (b.len() * 8) as u8,
        })
    }
    /// A frame from hex text.
    pub fn from_hex(s: &str) -> Option<Frame> {
        let s = s.trim();
        if !s.len().is_multiple_of(2) || !s.is_ascii() {
            return None;
        }
        let v: Option<Vec<u8>> = (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
            .collect();
        Frame::new(&v?)
    }
    /// The bytes in use.
    pub fn data(&self) -> &[u8] {
        &self.bytes[..self.bits as usize / 8]
    }
    /// The downlink format (bits 1 to 5; 24 for every extended length message).
    pub fn df(&self) -> u8 {
        df_of(self.bytes[0])
    }
    /// The CRC remainder.
    pub fn remainder(&self) -> u32 {
        crc::remainder(self.data())
    }
    /// The 24 bits after the first byte (bits 9 to 32).
    pub fn aa(&self) -> u32 {
        ((self.bytes[1] as u32) << 16) | ((self.bytes[2] as u32) << 8) | self.bytes[3] as u32
    }
    /// Upper-case hex.
    pub fn hex(&self) -> String {
        self.data().iter().map(|b| format!("{b:02X}")).collect()
    }
    /// Bits `a` to `b` (1-based, inclusive, at most 64 bits).
    pub fn field(&self, a: usize, b: usize) -> u64 {
        let mut v = 0u64;
        for i in a..=b {
            v = (v << 1) | ((self.bytes[(i - 1) / 8] >> (7 - (i - 1) % 8)) & 1) as u64;
        }
        v
    }
    /// The 56-bit ME or MB field (bits 33 to 88) of a long frame.
    pub fn me(&self) -> u64 {
        let mut v = 0u64;
        for &b in &self.bytes[4..11] {
            v = (v << 8) | b as u64;
        }
        v
    }
    /// Flips bit `i` (0-based).
    pub fn flip(&mut self, i: usize) {
        self.bytes[i / 8] ^= 0x80 >> (i % 8);
    }
}

/// The downlink format of a first byte (DF 24 and up all read as 24: the extended length messages).
pub fn df_of(b0: u8) -> u8 {
    let df = b0 >> 3;
    if df >= 24 {
        24
    } else {
        df
    }
}

/// The length a downlink format has, `None` for formats that are not searched for.
pub fn df_bits(b0: u8) -> Option<u8> {
    let df = b0 >> 3;
    match df {
        0 | 4 | 5 | 11 => Some(56),
        16..=21 => Some(112),
        24..=27 => Some(112),
        _ => None,
    }
}

/// Whether a format carries its address in the parity (address/parity).
pub fn is_ap(df: u8) -> bool {
    matches!(df, 0 | 4 | 5 | 16 | 20 | 21 | 24)
}

/// The key of an aircraft: the 24-bit address, with bit 24 set for a non-ICAO address (DF18 CF 1 or 5).
pub const NON_ICAO: u32 = 1 << 24;

/// The emitter category names (DO-260B), for screens.
pub fn category_name(set: char, code: u8) -> &'static str {
    match (set, code) {
        (_, 0) => "no information",
        ('A', 1) => "light",
        ('A', 2) => "small",
        ('A', 3) => "large",
        ('A', 4) => "high vortex large",
        ('A', 5) => "heavy",
        ('A', 6) => "high performance",
        ('A', 7) => "rotorcraft",
        ('B', 1) => "glider",
        ('B', 2) => "lighter than air",
        ('B', 3) => "parachutist",
        ('B', 4) => "ultralight",
        ('B', 6) => "unmanned aircraft",
        ('B', 7) => "space vehicle",
        ('C', 1) => "surface emergency vehicle",
        ('C', 2) => "surface service vehicle",
        ('C', 3) => "point obstacle",
        _ => "reserved",
    }
}

/// The emergency state names (TC 28).
pub fn emergency_name(e: u8) -> &'static str {
    match e {
        0 => "No emergency",
        1 => "General emergency",
        2 => "Medical",
        3 => "Minimum fuel",
        4 => "No communications",
        5 => "Unlawful interference",
        6 => "Downed aircraft",
        _ => "Reserved",
    }
}

/// The surface movement code as a speed in kt (the lower edge of its range); `None` for no information and the reserved codes.
pub fn movement_kt(m: u8) -> Option<f64> {
    Some(match m {
        1 => 0.0,
        2..=8 => 0.125 * (m - 1) as f64,
        9..=12 => 1.0 + 0.25 * (m - 9) as f64,
        13..=38 => 2.0 + 0.5 * (m - 13) as f64,
        39..=93 => 15.0 + (m - 39) as f64,
        94..=108 => 70.0 + 2.0 * (m - 94) as f64,
        109..=123 => 100.0 + 5.0 * (m - 109) as f64,
        124 => 175.0,
        _ => return None,
    })
}

/// The movement code for a ground speed (for the generator).
pub fn encode_movement(kt: f64) -> u8 {
    let k = kt.max(0.0);
    if k < 0.125 {
        1
    } else if k < 1.0 {
        (1.0 + k / 0.125).floor().min(8.0) as u8
    } else if k < 2.0 {
        (9.0 + (k - 1.0) / 0.25).floor() as u8
    } else if k < 15.0 {
        (13.0 + (k - 2.0) / 0.5).floor() as u8
    } else if k < 70.0 {
        (39.0 + (k - 15.0)).floor() as u8
    } else if k < 100.0 {
        (94.0 + (k - 70.0) / 2.0).floor() as u8
    } else if k < 175.0 {
        (109.0 + (k - 100.0) / 5.0).floor() as u8
    } else {
        124
    }
}

sdrkit_core::choice_enum! {
    /// The kind of a speed.
    #[derive(serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum SpeedKind {
        /// Ground speed.
        Ground = ("ground", "Ground speed"),
        /// Indicated airspeed.
        Ias = ("ias", "Indicated airspeed"),
        /// True airspeed.
        Tas = ("tas", "True airspeed"),
    }
}

/// The decoded ME field of an extended squitter.
#[derive(Clone, Debug, PartialEq)]
pub enum Es {
    /// TC 1 to 4.
    Identification {
        /// 'A' (TC 4) to 'D' (TC 1).
        set: char,
        /// 0 to 7.
        code: u8,
        /// Trailing spaces removed.
        callsign: crate::codes::Callsign,
    },
    /// TC 5 to 8.
    Surface {
        /// Type code.
        tc: u8,
        /// Movement code.
        movement: u8,
        /// Ground speed from the movement code.
        speed_kt: Option<f64>,
        /// Ground track when its status bit is set.
        track_deg: Option<f64>,
        /// CPR.
        cpr: CprFrame,
    },
    /// TC 9 to 18 and 20 to 22.
    Airborne {
        /// Type code.
        tc: u8,
        /// Surveillance status.
        ss: u8,
        /// Barometric altitude (TC 9 to 18) or GNSS height (TC 20 to 22).
        alt_ft: Option<i32>,
        /// TC 20 to 22.
        gnss: bool,
        /// Navigation integrity from the type code.
        nic: Option<u8>,
        /// CPR.
        cpr: CprFrame,
    },
    /// TC 0.
    NoPosition {
        /// The barometric altitude, when the message carries one.
        alt_ft: Option<i32>,
    },
    /// TC 19.
    Velocity {
        /// Subtype 1 to 4.
        subtype: u8,
        /// Ground speed, IAS or TAS.
        speed_kt: Option<f64>,
        /// Its kind.
        speed_kind: SpeedKind,
        /// Track (subtypes 1, 2) or magnetic heading (3, 4).
        heading_deg: Option<f64>,
        /// Whether `heading_deg` is a track.
        is_track: bool,
        /// Vertical rate, ft/min.
        vrate_fpm: Option<i32>,
        /// Barometric source of the vertical rate.
        vrate_baro: bool,
        /// GNSS minus barometric altitude, ft.
        gnss_diff_ft: Option<i32>,
    },
    /// TC 28 subtype 1.
    Status {
        /// Emergency state 0 to 7.
        emergency: u8,
        /// Mode A code, when not 0.
        squawk: Option<u16>,
    },
    /// TC 29 subtype 1.
    TargetState {
        /// Selected altitude, ft.
        sel_alt_ft: Option<i32>,
        /// FMS (true) or MCP/FCU (false).
        sel_alt_fms: bool,
        /// Barometric setting, hPa.
        baro_hpa: Option<f64>,
        /// Selected heading, 0 to 360.
        sel_heading_deg: Option<f64>,
        /// NACp.
        nacp: u8,
        /// Mode bits when valid: autopilot, VNAV, altitude hold, approach, LNAV.
        modes: Option<[bool; 5]>,
    },
    /// TC 31.
    OpStatus {
        /// 0 airborne, 1 surface.
        subtype: u8,
        /// ADS-B version.
        version: u8,
        /// NIC supplement A.
        nic_a: Option<u8>,
        /// NACp.
        nacp: Option<u8>,
        /// SIL.
        sil: Option<u8>,
        /// NIC baro.
        nic_baro: Option<u8>,
        /// GVA.
        gva: Option<u8>,
    },
    /// TC 23 to 27, 30 and other subtypes: counted, not decoded.
    Other {
        /// Type code.
        tc: u8,
    },
}

impl Es {
    /// Decodes a 56-bit ME field.
    pub fn decode(me: u64) -> Es {
        let f = |a: u32, b: u32| (me >> (56 - b)) & ((1u64 << (b - a + 1)) - 1);
        let tc = f(1, 5) as u8;
        let cpr = CprFrame {
            odd: f(22, 22) == 1,
            lat: f(23, 39) as u32,
            lon: f(40, 56) as u32,
        };
        match tc {
            1..=4 => Es::Identification {
                set: (b'A' + (4 - tc)) as char,
                code: f(6, 8) as u8,
                callsign: callsign(f(9, 56)),
            },
            5..=8 => {
                let movement = f(6, 12) as u8;
                let track_deg = (f(13, 13) == 1).then(|| f(14, 20) as f64 * 360.0 / 128.0);
                Es::Surface {
                    tc,
                    movement,
                    speed_kt: movement_kt(movement),
                    track_deg,
                    cpr,
                }
            }
            9..=18 | 20..=22 => {
                let nic = match tc {
                    9 => Some(11),
                    10 => Some(10),
                    11 => Some(9),
                    12 => Some(7),
                    13 => Some(6),
                    14 => Some(5),
                    15 => Some(4),
                    16 => Some(3),
                    17 => Some(1),
                    18 => Some(0),
                    _ => None,
                };
                Es::Airborne {
                    tc,
                    ss: f(6, 7) as u8,
                    alt_ft: ac12_ft(f(9, 20) as u32),
                    gnss: tc >= 20,
                    nic,
                    cpr,
                }
            }
            0 => Es::NoPosition {
                alt_ft: ac12_ft(f(9, 20) as u32),
            },
            19 => {
                let st = f(6, 8) as u8;
                let vr = f(38, 46) as i32;
                let vrate_fpm =
                    (vr != 0).then(|| (vr - 1) * 64 * if f(37, 37) == 1 { -1 } else { 1 });
                let gd = f(50, 56) as i32;
                let gnss_diff_ft =
                    (gd != 0).then(|| (gd - 1) * 25 * if f(49, 49) == 1 { -1 } else { 1 });
                let vrate_baro = f(36, 36) == 1;
                match st {
                    1 | 2 => {
                        let mult = if st == 2 { 4.0 } else { 1.0 };
                        let (ew, ns) = (f(15, 24) as f64, f(26, 35) as f64);
                        let (speed_kt, heading_deg) = if ew == 0.0 || ns == 0.0 {
                            (None, None)
                        } else {
                            let ve = (ew - 1.0) * mult * if f(14, 14) == 1 { -1.0 } else { 1.0 };
                            let vn = (ns - 1.0) * mult * if f(25, 25) == 1 { -1.0 } else { 1.0 };
                            let mut t = ve.atan2(vn).to_degrees();
                            if t < 0.0 {
                                t += 360.0;
                            }
                            (Some(ve.hypot(vn)), Some(t))
                        };
                        Es::Velocity {
                            subtype: st,
                            speed_kt,
                            speed_kind: SpeedKind::Ground,
                            heading_deg,
                            is_track: true,
                            vrate_fpm,
                            vrate_baro,
                            gnss_diff_ft,
                        }
                    }
                    3 | 4 => {
                        let mult = if st == 4 { 4.0 } else { 1.0 };
                        let heading_deg =
                            (f(14, 14) == 1).then(|| f(15, 24) as f64 * 360.0 / 1024.0);
                        let a = f(26, 35) as f64;
                        let speed_kt = (a != 0.0).then_some((a - 1.0) * mult);
                        let speed_kind = if f(25, 25) == 1 {
                            SpeedKind::Tas
                        } else {
                            SpeedKind::Ias
                        };
                        Es::Velocity {
                            subtype: st,
                            speed_kt,
                            speed_kind,
                            heading_deg,
                            is_track: false,
                            vrate_fpm,
                            vrate_baro,
                            gnss_diff_ft,
                        }
                    }
                    _ => Es::Other { tc },
                }
            }
            28 if f(6, 8) == 1 => {
                let id = f(12, 24) as u32;
                Es::Status {
                    emergency: f(9, 11) as u8,
                    squawk: (id != 0).then(|| squawk(id)),
                }
            }
            29 if f(6, 7) == 1 => {
                let sa = f(10, 20) as i32;
                let bs = f(21, 29) as f64;
                let sel_heading_deg = (f(30, 30) == 1).then(|| {
                    let raw = f(31, 39) as i32;
                    let v = if raw >= 256 { raw - 512 } else { raw };
                    let mut d = v as f64 * 180.0 / 256.0;
                    if d < 0.0 {
                        d += 360.0;
                    }
                    d
                });
                let modes = (f(47, 47) == 1).then(|| {
                    [
                        f(48, 48) == 1,
                        f(49, 49) == 1,
                        f(50, 50) == 1,
                        f(52, 52) == 1,
                        f(54, 54) == 1,
                    ]
                });
                Es::TargetState {
                    sel_alt_ft: (sa != 0).then(|| (sa - 1) * 32),
                    sel_alt_fms: f(9, 9) == 1,
                    baro_hpa: (bs != 0.0).then_some(800.0 + (bs - 1.0) * 0.8),
                    sel_heading_deg,
                    nacp: f(40, 43) as u8,
                    modes,
                }
            }
            31 if f(6, 8) <= 1 => {
                let subtype = f(6, 8) as u8;
                let version = f(41, 43) as u8;
                let v12 = version == 1 || version == 2;
                Es::OpStatus {
                    subtype,
                    version,
                    nic_a: v12.then(|| f(44, 44) as u8),
                    nacp: v12.then(|| f(45, 48) as u8),
                    sil: v12.then(|| f(51, 52) as u8),
                    nic_baro: (v12 && subtype == 0).then(|| f(53, 53) as u8),
                    gva: (version == 2 && subtype == 0).then(|| f(49, 50) as u8),
                }
            }
            _ => Es::Other { tc },
        }
    }

    /// The type code.
    pub fn tc(&self, me: u64) -> u8 {
        let _ = self;
        (me >> 51) as u8
    }
}

/// The header of any downlink format.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    /// Downlink format (24 for every ELM).
    pub df: u8,
    /// The aircraft key (see [`NON_ICAO`]).
    pub key: u32,
    /// CA (DF11, 17), CF (DF18), AF (DF19), FS (DF4, 5, 20, 21), VS (DF0, 16).
    pub code: u8,
    /// Barometric altitude of the AC field (DF0, 4, 16, 20).
    pub alt_ft: Option<i32>,
    /// Squawk of the ID field (DF5, 21).
    pub squawk: Option<u16>,
    /// Interrogator code of a DF11 (its remainder when below 128).
    pub ic: Option<u8>,
    /// Type code of an extended squitter whose ME is decoded.
    pub tc: Option<u8>,
}

/// Decodes the header fields. `remainder` is the frame's CRC remainder (the address of an AP format).
pub fn header(f: &Frame, remainder: u32) -> Header {
    let df = f.df();
    let code = f.field(6, 8) as u8;
    let mut h = Header {
        df,
        key: f.aa(),
        code,
        alt_ft: None,
        squawk: None,
        ic: None,
        tc: None,
    };
    match df {
        0 | 16 => {
            h.code = f.field(6, 6) as u8;
            h.key = remainder;
            h.alt_ft = ac13_ft(f.field(20, 32) as u32);
        }
        4 | 20 => {
            h.key = remainder;
            h.alt_ft = ac13_ft(f.field(20, 32) as u32);
        }
        5 | 21 => {
            h.key = remainder;
            h.squawk = Some(squawk(f.field(20, 32) as u32));
        }
        11 => h.ic = Some((remainder & 0x7F) as u8),
        17 => h.tc = Some(f.field(33, 37) as u8),
        18 => {
            if code == 1 || code == 5 {
                h.key |= NON_ICAO;
            }
            if matches!(code, 0 | 1 | 2 | 3 | 5 | 6) {
                h.tc = Some(f.field(33, 37) as u8);
            }
        }
        19 => {
            if code == 0 {
                h.tc = Some(f.field(33, 37) as u8);
            }
        }
        24 => h.key = remainder,
        _ => {}
    }
    h
}

/// The ME of a frame whose extended squitter layout is decoded (DF17, DF18 CF 0, 1, 2, 3, 5, 6, DF19 AF 0).
pub fn es(f: &Frame) -> Option<Es> {
    let code = f.field(6, 8);
    match f.df() {
        17 => Some(Es::decode(f.me())),
        18 if matches!(code, 0 | 1 | 2 | 3 | 5 | 6) => Some(Es::decode(f.me())),
        19 if code == 0 => Some(Es::decode(f.me())),
        _ => None,
    }
}

/// Flight status: (on the ground, alert, SPI); `None` for "on the ground" when the status does not say.
pub fn flight_status(fs: u8) -> (Option<bool>, bool, bool) {
    let ground = match fs {
        0 | 2 => Some(false),
        1 | 3 => Some(true),
        _ => None,
    };
    (ground, matches!(fs, 2..=4), matches!(fs, 4 | 5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identification_and_velocity() {
        let f = Frame::from_hex("8D4840D6202CC371C32CE0576098").unwrap();
        assert_eq!(f.remainder(), 0);
        match es(&f).unwrap() {
            Es::Identification {
                set,
                code,
                callsign,
            } => assert_eq!((set, code, callsign.as_str()), ('A', 0, "KLM1023")),
            e => panic!("{e:?}"),
        }
        let f = Frame::from_hex("8D485020994409940838175B284F").unwrap();
        match es(&f).unwrap() {
            Es::Velocity {
                speed_kt,
                heading_deg,
                vrate_fpm,
                gnss_diff_ft,
                vrate_baro,
                ..
            } => {
                assert!((speed_kt.unwrap() - 159.2).abs() < 0.1);
                assert!((heading_deg.unwrap() - 182.88).abs() < 0.01);
                assert_eq!(
                    (vrate_fpm, gnss_diff_ft, vrate_baro),
                    (Some(-832), Some(550), false)
                );
            }
            e => panic!("{e:?}"),
        }
    }
}
