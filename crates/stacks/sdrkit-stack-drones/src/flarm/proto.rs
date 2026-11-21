//! The old ("V6") FLARM air protocol: Manchester chips, the sync word, the CRC, XXTEA with the per-packet key,
//! the V6 fields, position completion and velocity.

use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;
use sdrkit_util_bits::line::{Manchester, ManchesterConv};
use sdrkit_util_bits::pack::{self, BitOrder};

/// The sync word (32 data bits).
pub const SYNC: u32 = 0xF531_FAB6;
/// Payload bytes.
pub const PAYLOAD: usize = 24;
/// Frame bytes after the sync: payload and CRC.
pub const FRAME: usize = 26;
/// Chips per second.
pub const CHIP_RATE: f64 = 100e3;
/// The FSK deviation, Hz.
pub const DEVIATION: f64 = 50e3;
/// XXTEA's constant.
pub const DELTA: u32 = 0x9E37_79B9;
/// The rounds the V6 protocol uses.
pub const ROUNDS: u32 = 6;
const K1: [u32; 8] = [
    0xE432_76DF,
    0xDCA8_3759,
    0x9802_B8AC,
    0x4675_A56B,
    0xFC78_EA65,
    0x804B_90EA,
    0xB765_42CD,
    0x329D_FA32,
];

/// Aircraft type names.
pub const AIRCRAFT_TYPES: [&str; 16] = [
    "unknown",
    "glider",
    "tow plane",
    "helicopter",
    "skydiver",
    "drop plane",
    "hang glider",
    "paraglider",
    "powered aircraft",
    "jet",
    "gyrocopter",
    "balloon",
    "airship",
    "UAV",
    "reserved",
    "static object",
];
/// Address type names.
pub const ADDRESS_TYPES: [&str; 4] = ["random", "ICAO", "FLARM", "anonymous"];

/// Manchester chips of bytes, most significant bit first: a 1 is chips 0 1, a 0 is chips 1 0 (the IEEE 802.3 convention).
pub fn manchester(bytes: &[u8]) -> Vec<u8> {
    let mut bits = Vec::with_capacity(bytes.len() * 8);
    pack::unpack(bytes, BitOrder::Msb, &mut bits);
    let mut c = Vec::with_capacity(bytes.len() * 16);
    Manchester::encode(&bits, ManchesterConv::Ieee, &mut c);
    c
}

/// The CRC of the payload: CRC-16/IBM-3740.
pub fn crc(payload: &[u8]) -> u16 {
    CRC16_IBM_3740.checksum(payload)
}

/// The 26 frame bytes for a payload: the payload then its CRC, high byte first.
pub fn frame(payload: &[u8; PAYLOAD]) -> [u8; FRAME] {
    let mut f = [0u8; FRAME];
    f[..PAYLOAD].copy_from_slice(payload);
    let c = crc(payload);
    f[24] = (c >> 8) as u8;
    f[25] = c as u8;
    f
}

/// Whether a frame's CRC holds (either byte order).
pub fn crc_ok(f: &[u8; FRAME]) -> bool {
    let c = crc(&f[..PAYLOAD]);
    let hi_first = ((f[24] as u16) << 8) | f[25] as u16;
    let lo_first = ((f[25] as u16) << 8) | f[24] as u16;
    c == hi_first || c == lo_first
}

/// The chips of a whole frame: sync then the 26 bytes (64 + 416 chips).
pub fn frame_chips(payload: &[u8; PAYLOAD]) -> Vec<u8> {
    let mut b = SYNC.to_be_bytes().to_vec();
    b.extend_from_slice(&frame(payload));
    manchester(&b)
}

fn mx(sum: u32, y: u32, z: u32, p: usize, e: u32, k: &[u32; 4]) -> u32 {
    (((z >> 5) ^ (y << 2)).wrapping_add((y >> 3) ^ (z << 4)))
        ^ ((sum ^ y).wrapping_add(k[(p & 3) ^ e as usize] ^ z))
}

/// XXTEA (Corrected Block TEA) encryption of `v` in place with `rounds` rounds.
pub fn xxtea_encrypt(v: &mut [u32], k: &[u32; 4], rounds: u32) {
    let n = v.len();
    if n < 2 {
        return;
    }
    let mut sum = 0u32;
    let mut z = v[n - 1];
    for _ in 0..rounds {
        sum = sum.wrapping_add(DELTA);
        let e = (sum >> 2) & 3;
        for p in 0..n {
            let y = v[(p + 1) % n];
            v[p] = v[p].wrapping_add(mx(sum, y, z, p, e, k));
            z = v[p];
        }
    }
}

/// XXTEA decryption of `v` in place with `rounds` rounds.
pub fn xxtea_decrypt(v: &mut [u32], k: &[u32; 4], rounds: u32) {
    let n = v.len();
    if n < 2 {
        return;
    }
    let mut sum = rounds.wrapping_mul(DELTA);
    let mut y = v[0];
    for _ in 0..rounds {
        let e = (sum >> 2) & 3;
        for p in (0..n).rev() {
            let z = v[(p + n - 1) % n];
            v[p] = v[p].wrapping_sub(mx(sum, y, z, p, e, k));
            y = v[p];
        }
        sum = sum.wrapping_sub(DELTA);
    }
}

fn obscure(k: u32, s: u32) -> u32 {
    let m1 = s.wrapping_mul(k ^ (k >> 16));
    let m2 = s.wrapping_mul(m1 ^ (m1 >> 16));
    m2 ^ (m2 >> 16)
}

/// The key of a packet from `address` at UTC time `t` (seconds): it changes every 64 s.
pub fn key(t: u32, address: u32) -> [u32; 4] {
    let a = (address << 8) & 0xFF_FFFF;
    let hi = (t >> 23) & 1 == 1;
    let mut k = [0u32; 4];
    for (i, v) in k.iter_mut().enumerate() {
        let idx = if hi { i + 4 } else { i };
        *v = obscure(K1[idx] ^ ((t >> 6) ^ a), 0x045D_9F3B) ^ 0x87B5_62F4;
    }
    k
}

/// What a V6 packet says.
#[derive(Clone, Debug, PartialEq)]
pub struct V6 {
    /// The 24-bit address.
    pub address: u32,
    /// Address type (0 random, 1 ICAO, 2 FLARM, 3 anonymous).
    pub addr_type: u8,
    /// Aircraft type (0..15).
    pub aircraft_type: u8,
    /// The airborne flag.
    pub airborne: bool,
    /// The stealth flag.
    pub stealth: bool,
    /// The no-track flag.
    pub no_track: bool,
    /// GPS status (12 bits).
    pub gps: u16,
    /// Altitude above the WGS 84 ellipsoid, metres (0..8191).
    pub alt_m: u16,
    /// The low 19 bits of the latitude in 1e-7 degree / 128.
    pub lat19: u32,
    /// The low 20 bits of the longitude.
    pub lon20: u32,
    /// The speed multiplier (0..3).
    pub mult: u8,
    /// Vertical speed raw (10 bits, signed).
    pub vs_raw: i16,
    /// Four north and four east velocity samples.
    pub ns: [i8; 4],
    /// East samples.
    pub ew: [i8; 4],
}

impl V6 {
    /// Whether the packet requests not to be tracked (no-track, stealth, or an anonymous address).
    pub fn flagged(&self) -> bool {
        self.no_track || self.stealth || self.addr_type == 3
    }
    /// Speed over the ground (m/s) and track (degrees).
    pub fn velocity(&self) -> (f64, f64) {
        let ns = self.ns.iter().map(|&v| v as i32).sum::<i32>() / 4;
        let ew = self.ew.iter().map(|&v| v as i32).sum::<i32>() / 4;
        let f = (1u32 << self.mult) as f64;
        let speed = ((ns * ns + ew * ew) as f64).sqrt() * f / 4.0;
        let track = if ns == 0 && ew == 0 {
            0.0
        } else {
            (ew as f64).atan2(ns as f64).to_degrees().rem_euclid(360.0)
        };
        (speed, track)
    }
    /// Climb rate, m/s.
    pub fn climb(&self) -> f64 {
        self.vs_raw as f64 * (1u32 << self.mult) as f64 / 10.0
    }
    /// The position completed around a known position (the station).
    pub fn position(&self, near_lat: f64, near_lon: f64) -> (f64, f64) {
        (
            complete(self.lat19, 19, near_lat),
            complete(self.lon20, 20, near_lon),
        )
    }
}

/// The value encoded for a coordinate: round(deg x 1e7) / 128, rounded half away from zero.
pub fn coord_units(deg: f64) -> i64 {
    let v = (deg * 1e7).round() as i64;
    if v >= 0 {
        (v + 64) >> 7
    } else {
        -((-v + 64) >> 7)
    }
}

/// Completes the low `bits` bits of a coordinate to the value nearest to `near` (degrees).
pub fn complete(low: u32, bits: u32, near: f64) -> f64 {
    let r = coord_units(near);
    let m = 1i64 << bits;
    let mut d = (low as i64 - r).rem_euclid(m);
    if d >= m / 2 {
        d -= m;
    }
    (r + d) as f64 * 128.0 / 1e7
}

/// Modular distance of two `bits`-bit values.
pub fn mod_distance(a: u32, b: u32, bits: u32) -> u32 {
    let m = 1u32 << bits;
    let d = a.wrapping_sub(b) & (m - 1);
    d.min(m - d)
}

/// What a transmitter puts in a V6 packet.
#[derive(Clone, Debug)]
pub struct V6Tx {
    /// The 24-bit address.
    pub address: u32,
    /// Address type.
    pub addr_type: u8,
    /// Aircraft type.
    pub aircraft_type: u8,
    /// Latitude, degrees.
    pub lat: f64,
    /// Longitude, degrees.
    pub lon: f64,
    /// Altitude, metres.
    pub alt_m: f64,
    /// Speed, m/s.
    pub speed: f64,
    /// Track, degrees.
    pub track: f64,
    /// Climb, m/s.
    pub climb: f64,
    /// Airborne.
    pub airborne: bool,
    /// Stealth flag.
    pub stealth: bool,
    /// No-track flag.
    pub no_track: bool,
}

impl V6Tx {
    /// The clear fields as a [`V6`].
    pub fn fields(&self) -> V6 {
        let vn = self.speed * self.track.to_radians().cos();
        let ve = self.speed * self.track.to_radians().sin();
        let mut mult = 0u8;
        while mult < 3 {
            let f = (1u32 << mult) as f64;
            if (vn * 4.0 / f).abs().round() <= 127.0
                && (ve * 4.0 / f).abs().round() <= 127.0
                && (self.climb * 10.0 / f).abs().round() <= 511.0
            {
                break;
            }
            mult += 1;
        }
        let f = (1u32 << mult) as f64;
        let n = (vn * 4.0 / f).round().clamp(-127.0, 127.0) as i8;
        let e = (ve * 4.0 / f).round().clamp(-127.0, 127.0) as i8;
        V6 {
            address: self.address & 0xFF_FFFF,
            addr_type: self.addr_type & 7,
            aircraft_type: self.aircraft_type & 15,
            airborne: self.airborne,
            stealth: self.stealth,
            no_track: self.no_track,
            gps: 0x0FF,
            alt_m: self.alt_m.round().clamp(0.0, 8191.0) as u16,
            lat19: (coord_units(self.lat) as u32) & 0x7_FFFF,
            lon20: (coord_units(self.lon) as u32) & 0xF_FFFF,
            mult,
            vs_raw: (self.climb * 10.0 / f).round().clamp(-512.0, 511.0) as i16,
            ns: [n; 4],
            ew: [e; 4],
        }
    }
}

/// The clear payload words of a V6 packet (parity set).
pub fn pack(v: &V6, header_type: u8) -> [u32; 6] {
    let mut w = [0u32; 6];
    w[0] = (v.address & 0xFF_FFFF)
        | ((header_type as u32 & 15) << 24)
        | ((v.addr_type as u32 & 7) << 28);
    w[1] = (v.vs_raw as u32 & 0x3FF)
        | ((v.airborne as u32) << 12)
        | ((v.stealth as u32) << 13)
        | ((v.no_track as u32) << 14)
        | ((v.gps as u32 & 0xFFF) << 16)
        | ((v.aircraft_type as u32 & 15) << 28);
    w[2] = (v.lat19 & 0x7_FFFF) | ((v.alt_m as u32 & 0x1FFF) << 19);
    w[3] = (v.lon20 & 0xF_FFFF) | ((v.mult as u32 & 3) << 30);
    w[4] = u32::from_le_bytes(v.ns.map(|x| x as u8));
    w[5] = u32::from_le_bytes(v.ew.map(|x| x as u8));
    // Parity: all 24 bytes together have an even number of ones.
    let ones: u32 = w.iter().map(|x| x.count_ones()).sum();
    if ones % 2 == 1 {
        w[1] |= 1 << 15;
    }
    w
}

/// Payload bytes of words (little-endian).
pub fn words_to_bytes(w: &[u32; 6]) -> [u8; PAYLOAD] {
    let mut b = [0u8; PAYLOAD];
    for (i, x) in w.iter().enumerate() {
        b[i * 4..i * 4 + 4].copy_from_slice(&x.to_le_bytes());
    }
    b
}

/// Words of payload bytes.
pub fn bytes_to_words(b: &[u8; PAYLOAD]) -> [u32; 6] {
    let mut w = [0u32; 6];
    for (i, x) in w.iter_mut().enumerate() {
        *x = u32::from_le_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]);
    }
    w
}

/// The encrypted payload of a V6 packet at key time `t`.
pub fn encode(v: &V6, t: u32) -> [u8; PAYLOAD] {
    let mut w = pack(v, 0);
    let k = key(t, v.address);
    xxtea_encrypt(&mut w[1..], &k, ROUNDS);
    words_to_bytes(&w)
}

/// The clear header of a payload: (address, header type, address type).
pub fn header(b: &[u8; PAYLOAD]) -> (u32, u8, u8) {
    let w0 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    (
        w0 & 0xFF_FFFF,
        ((w0 >> 24) & 15) as u8,
        ((w0 >> 28) & 7) as u8,
    )
}

/// Decrypts a V6 payload at key time `t`. `None` when the parity fails.
pub fn decode(b: &[u8; PAYLOAD], t: u32) -> Option<V6> {
    let mut w = bytes_to_words(b);
    let (address, _, addr_type) = header(b);
    let k = key(t, address);
    xxtea_decrypt(&mut w[1..], &k, ROUNDS);
    let ones: u32 = w.iter().map(|x| x.count_ones()).sum();
    if ones % 2 == 1 {
        return None;
    }
    let vs = (w[1] & 0x3FF) as i16;
    let vs_raw = if vs & 0x200 != 0 { vs - 0x400 } else { vs };
    let ns = w[4].to_le_bytes().map(|x| x as i8);
    let ew = w[5].to_le_bytes().map(|x| x as i8);
    Some(V6 {
        address,
        addr_type,
        aircraft_type: ((w[1] >> 28) & 15) as u8,
        airborne: (w[1] >> 12) & 1 == 1,
        stealth: (w[1] >> 13) & 1 == 1,
        no_track: (w[1] >> 14) & 1 == 1,
        gps: ((w[1] >> 16) & 0xFFF) as u16,
        alt_m: ((w[2] >> 19) & 0x1FFF) as u16,
        lat19: w[2] & 0x7_FFFF,
        lon20: w[3] & 0xF_FFFF,
        mult: ((w[3] >> 30) & 3) as u8,
        vs_raw,
        ns,
        ew,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f1_manchester_of_the_sync() {
        let c = manchester(&SYNC.to_be_bytes());
        let mut bytes = Vec::new();
        pack::pack(&c, BitOrder::Msb, pack::Pad::Zero, &mut bytes);
        assert_eq!(bytes, [0x55, 0x99, 0xA5, 0xA9, 0x55, 0x66, 0x65, 0x96]);
    }

    #[test]
    fn f2_crc_check_value() {
        assert_eq!(crc(b"123456789"), 0x29B1);
    }

    #[test]
    fn f3_xxtea() {
        let k = [1, 2, 3, 4];
        for rounds in [6, 6 + 52 / 5] {
            let orig = [0x1111_1111u32, 0x2222_2222, 0x3333_3333, 0x4444_4444, 5];
            let mut v = orig;
            xxtea_encrypt(&mut v, &k, rounds);
            assert_ne!(v, orig);
            let enc = v;
            xxtea_decrypt(&mut v, &k, rounds);
            assert_eq!(v, orig);
            let mut w = enc;
            xxtea_decrypt(&mut w, &[1, 2, 3, 5], rounds);
            assert_ne!(w, orig);
        }
    }

    #[test]
    fn f4_keys_change_with_time_step_and_address() {
        let t = 1_800_000_123;
        assert_eq!(key(t, 0xDD1234), key(t - (t % 64), 0xDD1234));
        assert_ne!(key(t, 0xDD1234), key(t + 64, 0xDD1234));
        assert_ne!(key(t, 0xDD1234), key(t, 0xDD1235));
    }

    fn glider() -> V6Tx {
        V6Tx {
            address: 0xDD1234,
            addr_type: 2,
            aircraft_type: 1,
            lat: 54.7123,
            lon: 25.2845,
            alt_m: 1450.0,
            speed: 27.3,
            track: 215.0,
            climb: 2.1,
            airborne: true,
            stealth: false,
            no_track: false,
        }
    }

    #[test]
    fn f5_v6_round_trip() {
        let t = 1_800_000_123;
        let b = encode(&glider().fields(), t);
        assert_eq!(header(&b), (0xDD1234, 0, 2));
        let v = decode(&b, t).unwrap();
        assert_eq!(v.aircraft_type, 1);
        assert_eq!(v.alt_m, 1450);
        let (lat, lon) = v.position(54.0, 25.0);
        assert!((lat - 54.7123).abs() < 2e-5 && (lon - 25.2845).abs() < 2e-5);
        let (s, tr) = v.velocity();
        assert!((s - 27.3).abs() < 1.0 && (tr - 215.0).abs() < 4.0);
        assert!((v.climb() - 2.1).abs() < 0.5);
    }

    #[test]
    fn f6_wrong_keys_fail_parity_about_half_the_time() {
        let t = 1_800_000_123u32;
        let b = encode(&glider().fields(), t);
        let fails = (1..=64)
            .filter(|i| decode(&b, t + 64 * i).is_none())
            .count();
        assert!((16..=48).contains(&fails), "{fails}");
    }

    #[test]
    fn f7_flags_survive() {
        let mut g = glider();
        g.no_track = true;
        g.stealth = true;
        let v = decode(&encode(&g.fields(), 5000), 5000).unwrap();
        assert!(v.no_track && v.stealth && v.flagged());
    }

    #[test]
    fn f8_newer_protocol_header() {
        let mut w = pack(&glider().fields(), 2);
        w[1] ^= 0x5555;
        let b = words_to_bytes(&w);
        assert_eq!(header(&b).1, 2);
    }

    #[test]
    fn f9_position_completion() {
        for (lat, lon) in [(-33.9, 151.2), (48.1, 11.6), (0.0, -0.1), (47.2, 8.9)] {
            let g = V6Tx {
                lat,
                lon,
                ..glider()
            };
            let v = g.fields();
            let (a, b) = v.position(lat + 1.5, lon - 2.0);
            assert!(
                (a - lat).abs() < 2e-5 && (b - lon).abs() < 2e-5,
                "{lat} {lon}"
            );
        }
    }

    #[test]
    fn f10_frame_chips_and_crc() {
        let p = encode(&glider().fields(), 1);
        let c = frame_chips(&p);
        assert_eq!(c.len(), 64 + 416);
        let mut f = frame(&p);
        assert!(crc_ok(&f));
        f[5] ^= 0x10;
        assert!(!crc_ok(&f));
    }
}
