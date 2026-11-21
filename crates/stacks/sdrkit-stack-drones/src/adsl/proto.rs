//! The ADS-L packet: bit fields, the exponential coding, the network packet with its scrambling and parity, and the Traffic,
//! Status and Traffic Uplink payloads.
//! Standard: EASA ADS-L 4 SRD-860 Issue 2, clauses B.3, E, F and G.

use crate::flarm::proto::{xxtea_decrypt, xxtea_encrypt};
use sdrkit_algo_fec::rs::{Basis, Rs, RsCode, SymbolOrder};
use sdrkit_util_bits::crc::catalog::CRC24_MODE_S;
use sdrkit_util_bits::crc::CrcSpec;

/// The rounds of the scrambling (clause E.2).
pub const SCRAMBLE_ROUNDS: u32 = 6;
/// The public key of key index 0.
pub const PUBLIC_KEY: [u32; 4] = [0; 4];
/// The parity bytes of the Reed-Solomon form.
pub const RS_PARITY: usize = 32;
/// The bytes of a Traffic packet: network header, 20 bytes of data, CRC-24.
pub const TRAFFIC_PACKET: usize = 24;
/// The bytes of a Status packet.
pub const STATUS_PACKET: usize = 16;
/// The bytes of a Traffic payload.
pub const TRAFFIC_PAYLOAD: usize = 15;
/// The bytes after the parity when the signature flag is set: a time stamp and an Ed25519 signature.
pub const SIGNATURE: usize = 68;
/// The fixed header of an O-band LDR transmission, before the network packet.
pub const LDR_HEADER: [u8; 6] = [0x00, 0x00, 0x00, 0x00, 0x18, 0x71];

/// The Reed-Solomon code of clause E.3.2 at its full length: field 0x11D, the roots α^(11 (121 + i)), i = 0 to 31.
pub const RS_CODE: RsCode = RsCode {
    name: "ADS-L RS(255,223)",
    m: 8,
    poly: 0x11D,
    n: 255,
    k: 223,
    fcr: 121,
    prim: 11,
    order: SymbolOrder::HighFirst,
    basis: Basis::Conventional,
};

/// The CRC-8 at the end of an O-band LDR transmission: polynomial 0x07, start value 0x71.
pub const LDR_CRC8: CrcSpec =
    CrcSpec::new("CRC-8/ADS-L-LDR", 8, 0x07, 0x71, false, false, 0x00, 0x1F);

/// The `width` bits at bit offset `offset` of `b`: bit n is in byte n / 8 at weight 2^(n mod 8) (clause B.3).
pub fn bits(b: &[u8], offset: usize, width: usize) -> u32 {
    (0..width).fold(0, |v, i| {
        let n = offset + i;
        let bit = b.get(n / 8).map_or(0, |x| (x >> (n % 8)) & 1);
        v | u32::from(bit) << i
    })
}

/// Writes the `width` low bits of `value` at bit offset `offset` of `b`.
pub fn put_bits(b: &mut [u8], offset: usize, width: usize, value: u32) {
    for i in 0..width {
        let n = offset + i;
        if let Some(x) = b.get_mut(n / 8) {
            *x = (*x & !(1 << (n % 8))) | (((value >> i) & 1) as u8) << (n % 8);
        }
    }
}

/// The value of an exponential code with a base of `n` bits under a 2-bit exponent (clause G.1.6).
pub fn exp_decode(code: u32, n: u32) -> u32 {
    let e = (code >> n) & 3;
    let base = code & ((1 << n) - 1);
    (((1 << n) + base) << e) - (1 << n)
}

/// The exponential code of `value` with a base of `n` bits. A value above the range of `top` gets the code `top`.
pub fn exp_encode_to(value: u32, n: u32, top: u32) -> u32 {
    let one = 1u32 << n;
    let e = (0..4u32)
        .rev()
        .find(|e| value >= (one << e) - one)
        .unwrap_or(0);
    let base = (value + one - (one << e)) >> e;
    if base >= one {
        top
    } else {
        ((e << n) | base).min(top)
    }
}

/// The exponential code of `value` with a base of `n` bits: the largest code below the all-ones code for a value above the
/// range.
pub fn exp_encode(value: u32, n: u32) -> u32 {
    exp_encode_to(value, n, (4 << n) - 2)
}

/// The names of the address mapping tables.
pub fn amt_name(amt: u8) -> &'static str {
    match amt {
        0 => "random",
        5 => "ICAO",
        6 => "FLARM",
        7 => "OGN",
        8 => "FANET",
        9..=63 => "manufacturer",
        _ => "reserved",
    }
}

/// The aircraft categories of clause G.1.3.
pub const CATEGORIES: [&str; 18] = [
    "no information",
    "light fixed wing",
    "small to heavy fixed wing",
    "light rotorcraft",
    "glider",
    "lighter than air",
    "ultralight",
    "paraglider",
    "parachutist",
    "eVTOL",
    "gyrocopter",
    "UAS open category",
    "UAS specific category",
    "UAS certified category",
    "model plane",
    "heavy rotorcraft",
    "hang-glider",
    "paramotor",
];

/// The name of category `c`.
pub fn category_name(c: u8) -> &'static str {
    CATEGORIES
        .get(usize::from(c))
        .copied()
        .unwrap_or("reserved")
}

/// The flight states.
pub const FLIGHT_STATES: [&str; 4] = ["undefined", "on ground", "airborne", "reserved"];

/// The emergency states.
pub const EMERGENCIES: [&str; 8] = [
    "undefined",
    "no emergency",
    "general emergency",
    "lifeguard / medical",
    "no communications",
    "unlawful interference",
    "downed aircraft",
    "reserved",
];

/// The first byte of a network packet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetHeader {
    /// The protocol version: 0 Issue 1, 1 Issue 2.
    pub version: u8,
    /// A signature follows the parity.
    pub signed: bool,
    /// The key index: 0 the public key, 1 and 2 reserved, 3 not scrambled.
    pub key_index: u8,
    /// The parity is Reed-Solomon, not CRC-24.
    pub reed_solomon: bool,
}

impl NetHeader {
    /// The header of byte `b`.
    pub fn of(b: u8) -> NetHeader {
        NetHeader {
            version: b & 0x0F,
            signed: b & 0x10 != 0,
            key_index: (b >> 5) & 3,
            reed_solomon: b & 0x80 != 0,
        }
    }
    /// The byte of the header.
    pub fn byte(&self) -> u8 {
        self.version & 0x0F
            | u8::from(self.signed) << 4
            | (self.key_index & 3) << 5
            | u8::from(self.reed_solomon) << 7
    }
}

/// Scrambles (or, with `back`, descrambles) the ADS-L data in place: XXTEA with 6 rounds and the zero key over the data as
/// 32-bit little-endian words. Bytes after the last whole word stay.
pub fn scramble(data: &mut [u8], back: bool) {
    let mut w: Vec<u32> = data
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    if back {
        xxtea_decrypt(&mut w, &PUBLIC_KEY, SCRAMBLE_ROUNDS);
    } else {
        xxtea_encrypt(&mut w, &PUBLIC_KEY, SCRAMBLE_ROUNDS);
    }
    for (c, v) in data.as_chunks_mut::<4>().0.iter_mut().zip(&w) {
        c.copy_from_slice(&v.to_le_bytes());
    }
}

/// The CRC-24 of `b` (the Mode S CRC), as the three bytes sent, most significant first.
pub fn crc24(b: &[u8]) -> [u8; 3] {
    let c = CRC24_MODE_S.checksum(b).to_be_bytes();
    [c[1], c[2], c[3]]
}

/// Whether the last three bytes of `packet` are the CRC-24 of the bytes before them.
pub fn crc24_ok(packet: &[u8]) -> bool {
    packet.len() > 3 && crc24(&packet[..packet.len() - 3]) == packet[packet.len() - 3..]
}

/// The CRC syndrome of a packet with CRC-24: 0 for a good packet. The CRC is linear, so the syndrome of a packet with wrong
/// bits is the XOR of the syndromes of those bits alone.
pub fn crc24_syndrome(packet: &[u8]) -> u32 {
    let n = packet.len().saturating_sub(3);
    let got = packet[n..].iter().fold(0u32, |v, &b| v << 8 | u32::from(b));
    CRC24_MODE_S.checksum(&packet[..n]) ^ got
}

/// The Reed-Solomon coder for a packet of `len` bytes (data and 32 parity bytes). `None` for a length outside 33 to 255.
pub fn rs_for(len: usize) -> Option<Rs> {
    if !(RS_PARITY + 1..=255).contains(&len) {
        return None;
    }
    Rs::new(RS_CODE.shortened(len as u16)).ok()
}

/// The network packet of ADS-L data `data` (header and payload, a whole number of 32-bit words): the header byte, the data
/// (scrambled unless the key index is 3) and the parity.
pub fn network_packet(head: NetHeader, data: &[u8]) -> Vec<u8> {
    let mut p = Vec::with_capacity(1 + data.len() + RS_PARITY);
    p.push(head.byte());
    p.extend_from_slice(data);
    if head.key_index != 3 {
        scramble(&mut p[1..], false);
    }
    if head.reed_solomon {
        let n = p.len();
        p.resize(n + RS_PARITY, 0);
        if let Some(rs) = rs_for(p.len()) {
            // A word of the right length always encodes.
            let _ = rs.encode_word(&mut p);
        }
    } else {
        let c = crc24(&p);
        p.extend_from_slice(&c);
    }
    p
}

/// The address of ADS-L data: the mapping table and the 24 bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Address {
    /// The address mapping table, 0 to 63.
    pub amt: u8,
    /// The 24-bit address.
    pub address: u32,
}

/// The header of ADS-L data (5 bytes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DataHeader {
    /// The payload type: 2 Traffic, 3 Status, 4 Traffic Uplink, 5 FIS-B Uplink, 6 Remote Identification.
    pub payload_type: u8,
    /// The sender.
    pub address: Address,
    /// Another station sent the packet again for the sender.
    pub relay: bool,
}

impl DataHeader {
    /// The header of the first 5 bytes of `data`.
    pub fn of(data: &[u8]) -> DataHeader {
        DataHeader {
            payload_type: bits(data, 0, 8) as u8,
            address: Address {
                amt: bits(data, 8, 6) as u8,
                address: bits(data, 14, 24),
            },
            relay: bits(data, 39, 1) != 0,
        }
    }
    /// The 5 bytes of the header.
    pub fn bytes(&self) -> [u8; 5] {
        let mut b = [0u8; 5];
        put_bits(&mut b, 0, 8, u32::from(self.payload_type));
        put_bits(&mut b, 8, 6, u32::from(self.address.amt));
        put_bits(&mut b, 14, 24, self.address.address);
        put_bits(&mut b, 39, 1, u32::from(self.relay));
        b
    }
}

/// A Traffic payload (clause G.1). A field that the sender marked invalid is `None`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Traffic {
    /// The time of the position fix: quarter seconds since the full hour, modulo 60.
    pub timestamp: u8,
    /// The flight state: 0 undefined, 1 on ground, 2 airborne.
    pub flight_state: u8,
    /// The aircraft category.
    pub category: u8,
    /// The emergency status.
    pub emergency: u8,
    /// Latitude and longitude, degrees.
    pub position: Option<(f64, f64)>,
    /// The ground speed, m/s.
    pub speed_m_s: Option<f64>,
    /// The altitude above the WGS-84 ellipsoid, metres.
    pub alt_m: Option<f64>,
    /// The vertical rate, m/s, up positive.
    pub climb_m_s: Option<f64>,
    /// The ground track, degrees clockwise from true north.
    pub track_deg: Option<f64>,
    /// The source integrity level, 0 to 3.
    pub sil: u8,
    /// The design assurance, 0 to 3.
    pub design_assurance: u8,
    /// The navigation integrity category, 0 to 12.
    pub nic: u8,
    /// The horizontal position accuracy category, 0 to 7.
    pub hfom: u8,
    /// The vertical position accuracy category, 0 to 3.
    pub vfom: u8,
    /// The velocity accuracy category, 0 to 3.
    pub velocity_accuracy: u8,
}

/// A 24-bit two's complement number.
fn s24(v: u32) -> i32 {
    ((v << 8) as i32) >> 8
}

impl Traffic {
    /// The payload of 15 bytes `p`.
    pub fn of(p: &[u8]) -> Traffic {
        let (lat, lon) = (bits(p, 16, 24), bits(p, 40, 24));
        let (la, lo) = (f64::from(s24(lat)) / 93206.0, f64::from(s24(lon)) / 46603.0);
        let position =
            (lat != 0x80_0000 && lon != 0x80_0000 && la.abs() <= 90.0 && lo.abs() <= 180.0)
                .then_some((la, lo));
        let speed = bits(p, 64, 8);
        let speed_m_s = (speed != 0xFF).then(|| f64::from(exp_decode(speed, 6)) * 0.25);
        let alt = bits(p, 72, 14);
        let vr = bits(p, 86, 9);
        // "Minus zero" (0x100) is sent by some trackers for "no value".
        let climb_m_s = (vr != 0x1FF && vr != 0x100).then(|| {
            let v = f64::from(exp_decode(vr & 0xFF, 6)) * 0.125;
            if vr & 0x100 != 0 {
                -v
            } else {
                v
            }
        });
        Traffic {
            timestamp: bits(p, 0, 6) as u8,
            flight_state: bits(p, 6, 2) as u8,
            category: bits(p, 8, 5) as u8,
            emergency: bits(p, 13, 3) as u8,
            position,
            speed_m_s,
            alt_m: (alt != 0x3FFF).then(|| f64::from(exp_decode(alt, 12)) - 320.0),
            climb_m_s,
            track_deg: speed_m_s
                .filter(|v| *v > 0.0)
                .map(|_| f64::from(bits(p, 95, 9)) * 360.0 / 512.0),
            sil: bits(p, 104, 2) as u8,
            design_assurance: bits(p, 106, 2) as u8,
            nic: bits(p, 108, 4) as u8,
            hfom: bits(p, 112, 3) as u8,
            vfom: bits(p, 115, 2) as u8,
            velocity_accuracy: bits(p, 117, 2) as u8,
        }
    }

    /// The 15 bytes of the payload. A field that is `None` gets its invalid code.
    pub fn bytes(&self) -> [u8; TRAFFIC_PAYLOAD] {
        let mut p = [0u8; TRAFFIC_PAYLOAD];
        put_bits(&mut p, 0, 6, u32::from(self.timestamp % 60));
        put_bits(&mut p, 6, 2, u32::from(self.flight_state));
        put_bits(&mut p, 8, 5, u32::from(self.category));
        put_bits(&mut p, 13, 3, u32::from(self.emergency));
        let (lat, lon) = match self.position {
            Some((la, lo)) => (
                (la * 93206.0).round() as i32 as u32,
                (lo * 46603.0).round() as i32 as u32,
            ),
            None => (0x80_0000, 0x80_0000),
        };
        put_bits(&mut p, 16, 24, lat);
        put_bits(&mut p, 40, 24, lon);
        let speed = self
            .speed_m_s
            .map_or(0xFF, |v| exp_encode((v.max(0.0) * 4.0).round() as u32, 6));
        put_bits(&mut p, 64, 8, speed);
        let alt = self.alt_m.map_or(0x3FFF, |a| {
            exp_encode((a + 320.0).round().max(0.0) as u32, 12)
        });
        put_bits(&mut p, 72, 14, alt);
        let vr = self.climb_m_s.map_or(0x1FF, |v| {
            let units = (v.abs() * 8.0).round() as u32;
            // Zero has no sign. The largest negative code is one below the invalid code.
            if v < 0.0 && units != 0 {
                0x100 | exp_encode_to(units, 6, 0xFE)
            } else {
                exp_encode_to(units, 6, 0xFF)
            }
        });
        put_bits(&mut p, 86, 9, vr);
        let track = self.track_deg.map_or(0, |t| {
            (t.rem_euclid(360.0) * 512.0 / 360.0).round() as u32 % 512
        });
        put_bits(&mut p, 95, 9, track);
        put_bits(&mut p, 104, 2, u32::from(self.sil));
        put_bits(&mut p, 106, 2, u32::from(self.design_assurance));
        put_bits(&mut p, 108, 4, u32::from(self.nic));
        put_bits(&mut p, 112, 3, u32::from(self.hfom));
        put_bits(&mut p, 115, 2, u32::from(self.vfom));
        put_bits(&mut p, 117, 2, u32::from(self.velocity_accuracy));
        p
    }
}

/// A Status payload (clause G.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// The highest protocol version of the sender.
    pub version: u8,
    /// The receive class on the M-band: 0 none, 1 occasional, 2 partial, 3 full.
    pub rx_m: u8,
    /// The receive class on O-band LDR.
    pub rx_ldr: u8,
    /// The receive class on O-band HDR.
    pub rx_hdr: u8,
    /// ADS-L 4 Mobile: 0 none, 1 offline, 2 online.
    pub mobile: u8,
    /// The other systems received, as a bit map.
    pub rx_systems: u8,
    /// The other systems sent, as a bit map.
    pub tx_systems: u8,
    /// The transponder: 0 none, 1 Mode C, 2 Mode S.
    pub transponder: u8,
    /// The sender processes Traffic Uplink.
    pub traffic_uplink: bool,
    /// The sender processes FIS-B Uplink.
    pub fisb_uplink: bool,
}

impl Status {
    /// The payload of 7 bytes `p`.
    pub fn of(p: &[u8]) -> Status {
        Status {
            version: bits(p, 0, 4) as u8,
            rx_m: bits(p, 4, 2) as u8,
            rx_ldr: bits(p, 6, 2) as u8,
            rx_hdr: bits(p, 8, 2) as u8,
            mobile: bits(p, 10, 2) as u8,
            rx_systems: bits(p, 12, 8) as u8,
            tx_systems: bits(p, 20, 8) as u8,
            transponder: bits(p, 28, 2) as u8,
            traffic_uplink: bits(p, 30, 1) != 0,
            fisb_uplink: bits(p, 31, 1) != 0,
        }
    }

    /// The 7 bytes of the payload.
    pub fn bytes(&self) -> [u8; 7] {
        let mut p = [0u8; 7];
        put_bits(&mut p, 0, 4, u32::from(self.version));
        put_bits(&mut p, 4, 2, u32::from(self.rx_m));
        put_bits(&mut p, 6, 2, u32::from(self.rx_ldr));
        put_bits(&mut p, 8, 2, u32::from(self.rx_hdr));
        put_bits(&mut p, 10, 2, u32::from(self.mobile));
        put_bits(&mut p, 12, 8, u32::from(self.rx_systems));
        put_bits(&mut p, 20, 8, u32::from(self.tx_systems));
        put_bits(&mut p, 28, 2, u32::from(self.transponder));
        put_bits(&mut p, 30, 1, u32::from(self.traffic_uplink));
        put_bits(&mut p, 31, 1, u32::from(self.fisb_uplink));
        p
    }

    /// The capabilities in words.
    pub fn text(&self) -> String {
        const SYSTEMS: [&str; 7] = [
            "FLARM",
            "PilotAware",
            "FANET",
            "OGN",
            "ADS-B 1090",
            "UAT 978",
            "Mode S",
        ];
        let names = |map: u8| -> String {
            let v: Vec<&str> = SYSTEMS
                .iter()
                .enumerate()
                .filter(|(i, _)| map >> i & 1 != 0)
                .map(|(_, n)| *n)
                .collect();
            if v.is_empty() {
                "none".into()
            } else {
                v.join(" ")
            }
        };
        format!(
            "version {}, receives M {} LDR {} HDR {}, hears {}, sends {}, transponder {}",
            self.version,
            self.rx_m,
            self.rx_ldr,
            self.rx_hdr,
            names(self.rx_systems),
            names(self.tx_systems),
            ["none", "Mode C", "Mode S", "reserved"][usize::from(self.transponder & 3)]
        )
    }
}

/// ADS-L data of one payload: the 5 header bytes and the payload.
pub fn data(head: DataHeader, payload: &[u8]) -> Vec<u8> {
    let mut d = head.bytes().to_vec();
    d.extend_from_slice(payload);
    d
}

/// The data of a Traffic Uplink from `ground` with these aircraft (at most 10), padded to a whole number of words.
pub fn uplink_data(ground: Address, aircraft: &[(Address, Traffic)]) -> Vec<u8> {
    let mut d = DataHeader {
        payload_type: 4,
        address: ground,
        relay: false,
    }
    .bytes()
    .to_vec();
    for (a, t) in aircraft.iter().take(10) {
        let mut e = [0u8; 4];
        put_bits(&mut e, 0, 6, u32::from(a.amt));
        put_bits(&mut e, 6, 24, a.address);
        d.extend_from_slice(&e);
        d.extend_from_slice(&t.bytes());
    }
    while !d.len().is_multiple_of(4) {
        d.push(0);
    }
    d
}

/// The entries of a Traffic Uplink payload: the address and the Traffic payload of each aircraft. An entry of address 0
/// without a position is padding and is left out.
pub fn uplink_entries(payload: &[u8]) -> Vec<(Address, Traffic)> {
    payload
        .as_chunks::<{ 4 + TRAFFIC_PAYLOAD }>()
        .0
        .iter()
        .take(10)
        .map(|e| {
            (
                Address {
                    amt: bits(e, 0, 6) as u8,
                    address: bits(e, 6, 24),
                },
                Traffic::of(&e[4..]),
            )
        })
        .filter(|(a, t)| a.address != 0 || t.position.is_some())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    /// The Traffic data of the worked packet: FLARM DD4711, a glider at 54.7 N 25.3 E.
    pub fn a1_data() -> Vec<u8> {
        let t = Traffic {
            timestamp: 0,
            flight_state: 2,
            category: 4,
            emergency: 1,
            position: Some((54.7, 25.3)),
            speed_m_s: Some(31.5),
            alt_m: Some(1234.0),
            climb_m_s: Some(-1.25),
            track_deg: Some(247.5),
            hfom: 6,
            vfom: 2,
            velocity_accuracy: 2,
            ..Traffic::default()
        };
        data(
            DataHeader {
                payload_type: 2,
                address: Address {
                    amt: 6,
                    address: 0xDD4711,
                },
                relay: false,
            },
            &t.bytes(),
        )
    }

    #[test]
    fn numbers_are_little_endian_and_fields_count_bits_from_bit_0() {
        let mut b = [0u8; 3];
        put_bits(&mut b, 0, 24, 1_328_922);
        assert_eq!(b, [0x1A, 0x47, 0x14]);
        let mut b = [0u8; 3];
        put_bits(&mut b, 0, 14, 0x12AA);
        put_bits(&mut b, 14, 10, 0x355);
        assert_eq!(b, [0xAA, 0x52, 0xD5]);
        assert_eq!((bits(&b, 0, 14), bits(&b, 14, 10)), (0x12AA, 0x355));
    }

    #[test]
    fn the_exponential_coding_gives_the_examples_of_the_standard() {
        // Altitude: metres + 320, a base of 12 bits.
        for (m, code) in [
            (0u32, 0x0000),
            (320, 0x0140),
            (1320, 0x0528),
            (61424, 0x3FFE),
        ] {
            assert_eq!(exp_encode(m, 12), code, "altitude {m}");
            assert_eq!(exp_decode(code, 12), m);
        }
        assert_eq!(exp_encode(70000, 12), 0x3FFE);
        // Ground speed in 0.25 m/s, a base of 6 bits.
        for (u, code) in [(0u32, 0x00), (1, 0x01), (3, 0x03), (480, 0xC4), (944, 0xFE)] {
            assert_eq!(exp_encode(u, 6), code, "speed {u}");
            assert_eq!(exp_decode(code, 6), u);
        }
        assert_eq!(exp_encode(1000, 6), 0xFE);
        // Vertical rate in 0.125 m/s with a sign bit.
        let vr = |v: f64| {
            bits(
                &Traffic {
                    climb_m_s: Some(v),
                    ..Traffic::default()
                }
                .bytes(),
                86,
                9,
            )
        };
        for (v, code) in [
            (0.0, 0x000),
            (0.125, 0x001),
            (-0.125, 0x101),
            (10.0, 0x048),
            (-10.0, 0x148),
            (119.0, 0x0FF),
            (150.0, 0x0FF),
            (-118.0, 0x1FE),
            (-150.0, 0x1FE),
        ] {
            assert_eq!(vr(v), code, "vertical rate {v}");
        }
    }

    #[test]
    fn the_crcs_have_their_known_values() {
        let f = hex("8D 48 40 D6 20 2C C3 71 C3 2C E0");
        assert_eq!(crc24(&f), [0x57, 0x60, 0x98]);
        assert_eq!(LDR_CRC8.compute(&[0x00]), 0x50);
        assert_eq!(LDR_CRC8.compute(b"123456789"), LDR_CRC8.check);
    }

    #[test]
    fn the_worked_traffic_packet_has_the_bytes_of_the_specification() {
        let d = a1_data();
        assert_eq!(
            d,
            hex("02 46 C4 51 37 80 24 80 CB 4D B0 FD 11 5F 12 86 42 B0 00 56")
        );
        let p = &d[5..];
        assert_eq!(
            (bits(p, 16, 24), bits(p, 40, 24), bits(p, 64, 8)),
            (0x4DCB80, 0x11FDB0, 0x5F)
        );
        assert_eq!(
            (bits(p, 72, 14), bits(p, 86, 9), bits(p, 95, 9)),
            (0x0612, 0x10A, 0x160)
        );
        let head = NetHeader {
            version: 1,
            ..NetHeader::default()
        };
        let packet = network_packet(head, &d);
        assert_eq!(
            packet,
            hex("01 57 A8 E2 6B 5A 73 BA 8D 52 77 B4 7E 4B 6D D6 EF F3 5E DC DA F0 65 E0")
        );
        assert!(crc24_ok(&packet));
        assert_eq!(LDR_CRC8.compute(&packet), 0x82);
        let clear = network_packet(
            NetHeader {
                key_index: 3,
                ..head
            },
            &d,
        );
        assert_eq!(
            clear,
            hex("61 02 46 C4 51 37 80 24 80 CB 4D B0 FD 11 5F 12 86 42 B0 00 56 E1 74 0D")
        );
        let mut back = packet[1..21].to_vec();
        scramble(&mut back, true);
        assert_eq!(back, d);
        let t = Traffic::of(&back[5..]);
        let (lat, lon) = t.position.unwrap();
        assert!((lat - 54.7).abs() < 2e-5 && (lon - 25.3).abs() < 4e-5);
        assert!((t.alt_m.unwrap() - 1234.0).abs() <= 2.0);
        assert!((t.speed_m_s.unwrap() * 3.6 - 113.4).abs() < 0.6);
        assert!((t.climb_m_s.unwrap() + 1.25).abs() < 0.07);
        assert!((t.track_deg.unwrap() - 247.5).abs() < 0.5);
        assert_eq!((t.category, t.flight_state, t.emergency), (4, 2, 1));
        assert_eq!((t.hfom, t.vfom, t.velocity_accuracy), (6, 2, 2));
        let h = DataHeader::of(&back);
        assert_eq!(
            (h.payload_type, h.address.amt, h.address.address),
            (2, 6, 0xDD4711)
        );
    }

    #[test]
    fn the_worked_status_packet_has_the_bytes_of_the_specification() {
        let d = data(
            DataHeader {
                payload_type: 3,
                address: Address {
                    amt: 6,
                    address: 0xDD4711,
                },
                relay: false,
            },
            &Status {
                version: 1,
                rx_m: 3,
                ..Status::default()
            }
            .bytes(),
        );
        assert_eq!(d, hex("03 46 C4 51 37 31 00 00 00 00 00 00"));
        let packet = network_packet(
            NetHeader {
                version: 1,
                ..NetHeader::default()
            },
            &d,
        );
        assert_eq!(
            packet,
            hex("01 A4 94 85 9E 3A AB 97 D0 D0 01 9C F7 27 3C 20")
        );
        let s = Status::of(&d[5..]);
        assert_eq!((s.version, s.rx_m, s.rx_ldr), (1, 3, 0));
    }

    #[test]
    fn invalid_codes_give_no_values() {
        let none = Traffic::default();
        let t = Traffic::of(&none.bytes());
        assert_eq!(
            (t.position, t.alt_m, t.speed_m_s, t.climb_m_s, t.track_deg),
            (None, None, None, None, None)
        );
        let mut p = Traffic {
            speed_m_s: Some(10.0),
            track_deg: Some(90.0),
            climb_m_s: Some(1.0),
            ..none
        }
        .bytes();
        assert!(Traffic::of(&p).track_deg.is_some());
        put_bits(&mut p, 86, 9, 0x100);
        assert_eq!(Traffic::of(&p).climb_m_s, None);
        // The track is not valid at speed 0.
        put_bits(&mut p, 64, 8, 0);
        assert_eq!(Traffic::of(&p).track_deg, None);
    }

    #[test]
    fn fields_survive_the_packet_for_many_values() {
        let mut worst = [0.0f64; 6];
        for i in 0..2000u32 {
            let f = f64::from(i);
            let t = Traffic {
                timestamp: (i % 60) as u8,
                flight_state: (i % 3) as u8,
                category: (i % 18) as u8,
                emergency: (i % 7) as u8,
                position: Some((-89.0 + f * 0.089, -179.0 + f * 0.179)),
                speed_m_s: Some(f * 0.118),
                alt_m: Some(-300.0 + f * 30.0),
                climb_m_s: Some(-118.0 + f * 0.1185),
                track_deg: Some(f * 0.18),
                sil: (i % 4) as u8,
                design_assurance: (i % 4) as u8,
                nic: (i % 13) as u8,
                hfom: (i % 8) as u8,
                vfom: (i % 4) as u8,
                velocity_accuracy: (i % 4) as u8,
            };
            let r = Traffic::of(&t.bytes());
            assert_eq!(
                (r.timestamp, r.flight_state, r.category, r.emergency),
                (t.timestamp, t.flight_state, t.category, t.emergency)
            );
            assert_eq!(
                (r.sil, r.nic, r.hfom, r.vfom),
                (t.sil, t.nic, t.hfom, t.vfom)
            );
            let (a, b) = (r.position.unwrap(), t.position.unwrap());
            worst[0] = worst[0].max((a.0 - b.0).abs());
            worst[1] = worst[1].max((a.1 - b.1).abs());
            // The step of an exponential code doubles with each exponent: the error stays below one step.
            let step =
                |units: f64, n: u32| f64::from(1u32 << (exp_encode(units.round() as u32, n) >> n));
            let s = t.speed_m_s.unwrap();
            assert!(
                (r.speed_m_s.unwrap() - s).abs() <= 0.25 * step(s * 4.0, 6),
                "speed {s}"
            );
            let alt = t.alt_m.unwrap();
            assert!(
                (r.alt_m.unwrap() - alt).abs() <= step(alt + 320.0, 12),
                "altitude {alt}"
            );
            let c = t.climb_m_s.unwrap();
            assert!(
                (r.climb_m_s.unwrap() - c).abs() <= 0.125 * step(c.abs() * 8.0, 6),
                "climb {c}"
            );
            if s > 0.2 {
                let d = (r.track_deg.unwrap() - t.track_deg.unwrap()).abs();
                worst[5] = worst[5].max(d.min(360.0 - d));
            }
        }
        assert!(worst[0] < 1.1e-5 && worst[1] < 2.2e-5, "{worst:?}");
        assert!(worst[5] < 0.4, "{worst:?}");
    }

    #[test]
    fn reed_solomon_corrects_sixteen_bytes_and_refuses_twenty() {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
        for len in [53usize, 100, 255] {
            let mut rs = rs_for(len).unwrap();
            let mut word: Vec<u8> = (0..len).map(|_| rng.next_u64() as u8).collect();
            rs.encode_word(&mut word).unwrap();
            let sent = word.clone();
            let mut at: Vec<usize> = (0..len).collect();
            for i in 0..16 {
                let j = i + rng.below((len - i) as u64) as usize;
                at.swap(i, j);
                word[at[i]] ^= 1 + rng.below(255) as u8;
            }
            assert_eq!(rs.decode(&mut word, &[]).unwrap().corrected, 16, "{len}");
            assert_eq!(word, sent);
            // With 20 wrong bytes the decoder never returns another block as good.
            for _ in 0..200 {
                let mut w = sent.clone();
                for i in 0..20 {
                    let j = i + rng.below((len - i) as u64) as usize;
                    at.swap(i, j);
                    w[at[i]] ^= 1 + rng.below(255) as u8;
                }
                if rs.decode(&mut w, &[]).is_ok() {
                    assert_eq!(w, sent, "{len}");
                }
            }
        }
        assert!(rs_for(32).is_none() && rs_for(256).is_none());
    }

    #[test]
    fn an_uplink_holds_its_aircraft_and_padding() {
        let t = |lat: f64| Traffic {
            position: Some((lat, 10.0)),
            flight_state: 2,
            ..Traffic::default()
        };
        let list: Vec<(Address, Traffic)> = (1..=3)
            .map(|i| {
                (
                    Address {
                        amt: 5,
                        address: 0x500000 + i,
                    },
                    t(50.0 + f64::from(i)),
                )
            })
            .collect();
        let d = uplink_data(
            Address {
                amt: 7,
                address: 0xF00001,
            },
            &list,
        );
        assert_eq!(d.len(), 64);
        assert_eq!(DataHeader::of(&d).payload_type, 4);
        let e = uplink_entries(&d[5..]);
        assert_eq!(e.len(), 3);
        assert_eq!(e[2].0.address, 0x500003);
        assert!((e[2].1.position.unwrap().0 - 53.0).abs() < 2e-5);
        let packet = network_packet(
            NetHeader {
                version: 1,
                reed_solomon: true,
                ..NetHeader::default()
            },
            &d,
        );
        assert_eq!(packet.len(), 1 + 64 + 32);
        assert!(rs_for(packet.len()).unwrap().check(&packet).unwrap());
    }
}
