//! Orbcomm packets: the Fletcher check, the packet types and their fields, the ephemeris position and the channel raster.

use sdrkit_util_bits::checksum::fletcher_mod256;
use sdrkit_util_bits::reader::BitReader;
use sdrkit_util_geo::{Ecef, Ellipsoid, Position};
use serde::{Deserialize, Serialize};

pub const PACKET: usize = 12;
pub const FRAME_PACKETS: usize = 50;
pub const SYNC: [u8; 3] = [0x65, 0xA8, 0xF9];

/// The Fletcher check of a 12-byte packet: both running sums over all 12 bytes are zero.
pub fn check(p: &[u8]) -> bool {
    p.len() == PACKET && fletcher_mod256(p) == (0, 0)
}

/// Sets bytes 10 and 11 so the packet checks.
pub fn seal(p: &mut [u8; PACKET]) {
    let (a, b) = fletcher_mod256(&p[..10]);
    p[10] = ((512 - ((a as u32 + b as u32) % 256)) % 256) as u8;
    p[11] = b;
}

/// The raster channel (12.5 kHz from 137.0 MHz) nearest a frequency.
pub fn raster(f: f64) -> f64 {
    137.0e6 + ((f - 137.0e6) / 12.5e3).round() * 12.5e3
}

/// A decoded packet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Packet {
    Sync {
        satellite: u8,
        frame: u8,
    },
    Message {
        id: u8,
        part: u8,
        last: bool,
        text: String,
    },
    Uplink {
        channels: Vec<f64>,
    },
    Downlink {
        channels: Vec<f64>,
    },
    Network {
        gateway: u8,
        status: u8,
    },
    Fill,
    Ephemeris(Ephemeris),
    Elements(Elements),
    Other {
        kind: u8,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ephemeris {
    pub x_km: f64,
    pub y_km: f64,
    pub z_km: f64,
    pub seconds: u32,
    pub lat: f64,
    pub lon: f64,
    pub height_km: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Elements {
    pub satellite: u8,
    pub inclination: f64,
    pub raan: f64,
    pub arg_latitude: f64,
    pub period_min: f64,
}

impl Packet {
    pub fn type_name(&self) -> &'static str {
        match self {
            Packet::Sync { .. } => "sync",
            Packet::Message { .. } => "message",
            Packet::Uplink { .. } => "uplink",
            Packet::Downlink { .. } => "downlink",
            Packet::Network { .. } => "network",
            Packet::Fill => "fill",
            Packet::Ephemeris(_) => "ephemeris",
            Packet::Elements(_) => "elements",
            Packet::Other { .. } => "other",
        }
    }
    /// The packet log's description.
    pub fn describe(&self) -> String {
        match self {
            Packet::Sync { satellite, frame } => {
                format!("sync  satellite {satellite}  frame {frame}")
            }
            Packet::Message {
                id,
                part,
                last,
                text,
            } => {
                format!(
                    "message {id} part {part}{}  \"{text}\"",
                    if *last { " (last)" } else { "" }
                )
            }
            Packet::Uplink { channels } => format!(
                "uplink channels: {} MHz",
                channels
                    .iter()
                    .map(|c| format!("{:.4}", c / 1e6))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            Packet::Downlink { channels } => format!(
                "downlink channels: {} MHz",
                channels
                    .iter()
                    .map(|c| format!("{:.4}", c / 1e6))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            Packet::Network { gateway, status } => {
                format!("network control  gateway {gateway}  status {status:02X}")
            }
            Packet::Fill => "fill".into(),
            Packet::Ephemeris(e) => format!(
                "ephemeris  {:.2} {:.2}  {:.0} km (x {:.1} y {:.1} z {:.1} km, {} s)",
                e.lat, e.lon, e.height_km, e.x_km, e.y_km, e.z_km, e.seconds
            ),
            Packet::Elements(e) => format!(
                "elements  satellite {}  incl {:.2}  RAAN {:.2}  arg lat {:.2}  period {:.2} min",
                e.satellite, e.inclination, e.raan, e.arg_latitude, e.period_min
            ),
            Packet::Other { kind } => format!("type {kind:02X}"),
        }
    }
}

fn u16le(p: &[u8], i: usize) -> u16 {
    p[i] as u16 | (p[i + 1] as u16) << 8
}

/// Reads a packet (its check already held).
pub fn parse(p: &[u8]) -> Packet {
    match p[0] {
        0x65 => Packet::Sync {
            satellite: p[3],
            frame: p[4],
        },
        0x1A => Packet::Message {
            id: p[1],
            part: p[2] & 0x7F,
            last: p[2] & 0x80 != 0,
            text: p[3..10]
                .iter()
                .map(|&c| {
                    if (0x20..0x7F).contains(&c) {
                        c as char
                    } else {
                        ' '
                    }
                })
                .collect(),
        },
        0x1B | 0x1C => {
            let n = (p[1] as usize).min(4);
            let base = if p[0] == 0x1B { 148.0e6 } else { 137.0e6 };
            let channels = (0..n)
                .map(|i| base + u16le(p, 2 + 2 * i) as f64 * 2.5e3)
                .collect();
            if p[0] == 0x1B {
                Packet::Uplink { channels }
            } else {
                Packet::Downlink { channels }
            }
        }
        0x1D => Packet::Network {
            gateway: p[1],
            status: p[2],
        },
        0x1E => Packet::Fill,
        0x1F => {
            // Three 20-bit two's complement coordinates in 1/64 km and a 12-bit clock, least significant bit first.
            let mut r = BitReader::new_lsb(&p[1..10]);
            let x = r.i(20) as f64 / 64.0;
            let y = r.i(20) as f64 / 64.0;
            let z = r.i(20) as f64 / 64.0;
            let seconds = r.u(12) as u32;
            let (lat, lon, h) = ecef_to_wgs84(x, y, z);
            Packet::Ephemeris(Ephemeris {
                x_km: x,
                y_km: y,
                z_km: z,
                seconds,
                lat,
                lon,
                height_km: h,
            })
        }
        0x22 => Packet::Elements(Elements {
            satellite: p[1],
            inclination: u16le(p, 2) as f64 / 100.0,
            raan: u16le(p, 4) as f64 / 100.0,
            arg_latitude: u16le(p, 6) as f64 / 100.0,
            period_min: u16le(p, 8) as f64 / 100.0,
        }),
        k => Packet::Other { kind: k },
    }
}

/// Builds the bytes of a packet (sealed).
pub fn build(pk: &Packet) -> [u8; PACKET] {
    let mut p = [0u8; PACKET];
    match pk {
        Packet::Sync { satellite, frame } => {
            p[..3].copy_from_slice(&SYNC);
            p[3] = *satellite;
            p[4] = *frame;
        }
        Packet::Message {
            id,
            part,
            last,
            text,
        } => {
            p[0] = 0x1A;
            p[1] = *id;
            p[2] = (*part & 0x7F) | if *last { 0x80 } else { 0 };
            for (i, c) in text
                .bytes()
                .chain(std::iter::repeat(b' '))
                .take(7)
                .enumerate()
            {
                p[3 + i] = c;
            }
        }
        Packet::Uplink { channels } | Packet::Downlink { channels } => {
            let up = matches!(pk, Packet::Uplink { .. });
            p[0] = if up { 0x1B } else { 0x1C };
            let base = if up { 148.0e6 } else { 137.0e6 };
            p[1] = channels.len().min(4) as u8;
            for (i, c) in channels.iter().take(4).enumerate() {
                let n = ((c - base) / 2.5e3).round() as u16;
                p[2 + 2 * i] = n as u8;
                p[3 + 2 * i] = (n >> 8) as u8;
            }
        }
        Packet::Network { gateway, status } => {
            p[0] = 0x1D;
            p[1] = *gateway;
            p[2] = *status;
        }
        Packet::Fill => p[0] = 0x1E,
        Packet::Ephemeris(e) => {
            p[0] = 0x1F;
            let q = |v: f64| ((v * 64.0).round() as i32 as u32) & 0xF_FFFF;
            let mut bits = Vec::with_capacity(72);
            for (v, n) in [
                (q(e.x_km), 20),
                (q(e.y_km), 20),
                (q(e.z_km), 20),
                (e.seconds & 0xFFF, 12),
            ] {
                for i in 0..n {
                    bits.push((v >> i) & 1 == 1);
                }
            }
            for (i, b) in bits.iter().enumerate() {
                if *b {
                    p[1 + i / 8] |= 1 << (i % 8);
                }
            }
        }
        Packet::Elements(e) => {
            p[0] = 0x22;
            p[1] = e.satellite;
            for (i, v) in [e.inclination, e.raan, e.arg_latitude, e.period_min]
                .iter()
                .enumerate()
            {
                let n = (v * 100.0).round() as u16;
                p[2 + 2 * i] = n as u8;
                p[3 + 2 * i] = (n >> 8) as u8;
            }
        }
        Packet::Other { kind } => p[0] = *kind,
    }
    seal(&mut p);
    p
}

/// Earth-fixed x, y, z (km) to WGS-84 latitude, longitude (degrees) and height (km). Coordinates that are not finite give NaN.
pub fn ecef_to_wgs84(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let e = Ecef {
        x_m: x * 1e3,
        y_m: y * 1e3,
        z_m: z * 1e3,
    };
    match Ellipsoid::WGS84.from_ecef(&e) {
        Ok(p) => (p.lat_deg, p.lon_deg, p.alt_m / 1e3),
        Err(_) => (f64::NAN, f64::NAN, f64::NAN),
    }
}

impl Ephemeris {
    /// The position, when it passes the family's position check (finite, a latitude within 90 degrees, not exactly 0, 0).
    pub fn position(&self) -> Option<Position> {
        if !(self.lat.is_finite() && self.lon.is_finite() && self.height_km.is_finite()) {
            return None;
        }
        if self.lat == 0.0 && self.lon == 0.0 {
            return None;
        }
        let p = Position {
            lat_deg: self.lat,
            lon_deg: self.lon,
            alt_m: self.height_km * 1e3,
        };
        p.validate().ok().map(|_| p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o1_format() {
        let mut p = build(&Packet::Fill);
        assert!(check(&p));
        p[5] ^= 0x10;
        assert!(!check(&p));
        let e = Ephemeris {
            x_km: -4123.5,
            y_km: 5021.25,
            z_km: 2999.0,
            seconds: 1234,
            ..Default::default()
        };
        let b = build(&Packet::Ephemeris(e));
        assert!(check(&b));
        match parse(&b) {
            Packet::Ephemeris(r) => {
                assert!(
                    (r.x_km + 4123.5).abs() < 0.02
                        && (r.y_km - 5021.25).abs() < 0.02
                        && (r.z_km - 2999.0).abs() < 0.02
                );
                assert_eq!(r.seconds, 1234);
            }
            x => panic!("{x:?}"),
        }
        let (la, lo, h) = ecef_to_wgs84(6378.137 + 775.0, 0.0, 0.0);
        assert!(la.abs() < 1e-9 && lo.abs() < 1e-9 && (h - 775.0).abs() < 1e-6);
        assert!((raster(137.2881e6) - 137.2875e6).abs() < 1.0);
        assert_eq!(
            parse(&build(&Packet::Sync {
                satellite: 23,
                frame: 4
            }))
            .type_name(),
            "sync"
        );
        assert_eq!(
            parse(&build(&Packet::Elements(Elements::default()))).type_name(),
            "elements"
        );
        assert_eq!(
            parse(&build(&Packet::Other { kind: 0x99 })),
            Packet::Other { kind: 0x99 }
        );
        let ch = Packet::Downlink {
            channels: vec![137.25e6, 137.2875e6],
        };
        assert_eq!(parse(&build(&ch)), ch);
        assert_eq!(ch.describe(), "downlink channels: 137.2500 137.2875 MHz");
    }
}
