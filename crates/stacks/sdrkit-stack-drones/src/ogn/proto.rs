//! The OGN tracker packet: the whitening, the (208,160) code from its 48 parity checks, the header word, the position fields
//! and the range forms.
//! No public document gives the sync word, the code, the whitening or the field layout. The cipher of the whitening is TEA
//! (Wheeler and Needham, 1994).

use sdrkit_algo_fec::ldpc::{BpDecoder, InfoAt, LdpcCode};
use std::sync::OnceLock;

/// The sync word, most significant bit first.
pub const SYNC: u32 = 0x0AF3_656C;
/// The bytes of a packet.
pub const PACKET: usize = 26;
/// The bits of a packet.
pub const BITS: usize = 208;
/// The data bits of a packet.
pub const DATA_BITS: usize = 160;
/// The cycles of the whitening.
pub const WHITEN_CYCLES: u32 = 8;
/// The key of the whitening.
pub const WHITEN_KEY: [u32; 4] = [0; 4];
/// The most bits that a repair can change. A word of noise is within 4 bits of a code word about 3 times in 10 million.
pub const MAX_REPAIR: u32 = 4;
/// The iterations of the repair.
pub const REPAIR_ITERATIONS: u32 = 30;

/// The names of the address types 0 to 3.
pub const ADDRESS_TYPES: [&str; 4] = ["random", "ICAO", "FLARM", "OGN"];
/// The names of the aircraft types 0 to 15.
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
    "UFO",
    "balloon",
    "airship",
    "UAV",
    "ground support",
    "static",
];

/// The parity checks: each lists the bit positions whose sum is even. Position p is the bit of weight 2^(7 - p mod 8) of byte
/// p / 8.
pub const CHECKS: [&[u8]; 48] = [
    &[5, 7, 12, 34, 93, 122, 129, 162, 169, 181],
    &[7, 44, 48, 183, 187, 192, 195, 196, 200, 204, 205],
    &[7, 9, 32, 63, 77, 88, 142, 156, 178, 197, 199],
    &[
        7, 15, 46, 58, 66, 70, 168, 184, 185, 188, 189, 198, 201, 203,
    ],
    &[
        7, 13, 73, 76, 79, 98, 161, 164, 167, 174, 175, 176, 177, 186, 194,
    ],
    &[
        7, 19, 21, 30, 31, 38, 103, 121, 137, 143, 145, 147, 150, 151, 153, 159, 170, 172, 173,
        179, 202,
    ],
    &[
        8, 12, 29, 33, 38, 45, 46, 56, 62, 98, 130, 149, 152, 156, 163, 190, 191, 205,
    ],
    &[
        6, 12, 58, 67, 69, 94, 114, 127, 134, 135, 141, 157, 165, 176, 197, 200, 202,
    ],
    &[
        2, 12, 22, 30, 66, 76, 86, 88, 97, 115, 116, 119, 123, 154, 155, 166, 204, 207,
    ],
    &[
        1, 3, 9, 12, 15, 16, 25, 52, 55, 101, 107, 110, 118, 139, 153, 171, 175, 187,
    ],
    &[
        12, 14, 28, 36, 54, 85, 99, 113, 126, 138, 159, 160, 174, 178, 180, 185, 195,
    ],
    &[
        35, 40, 64, 66, 80, 91, 95, 96, 117, 128, 133, 159, 175, 181, 193, 197, 205,
    ],
    &[
        4, 15, 21, 26, 59, 60, 72, 76, 81, 82, 92, 100, 102, 131, 156, 160, 181, 200,
    ],
    &[
        0, 9, 39, 42, 46, 49, 54, 87, 104, 106, 109, 140, 145, 176, 181, 204,
    ],
    &[
        50, 57, 58, 65, 78, 83, 88, 98, 103, 124, 144, 158, 180, 181, 187,
    ],
    &[
        11, 38, 55, 63, 71, 74, 84, 125, 135, 166, 181, 194, 195, 201,
    ],
    &[10, 14, 15, 61, 84, 88, 90, 105, 122, 136, 150, 176, 205],
    &[
        9, 41, 47, 51, 66, 68, 75, 85, 89, 98, 122, 125, 146, 179, 200,
    ],
    &[
        11, 23, 28, 53, 58, 112, 120, 122, 148, 151, 156, 175, 204, 206,
    ],
    &[19, 20, 27, 37, 46, 74, 76, 108, 113, 122, 187, 197],
    &[56, 57, 82, 91, 104, 118, 122, 154, 164, 184, 195, 199, 202],
    &[9, 24, 30, 58, 74, 82, 93, 174, 182, 205],
    &[17, 18, 28, 43, 46, 88, 91, 93, 132, 153, 194, 200],
    &[21, 29, 55, 61, 83, 93, 111, 146, 164, 185, 197, 204],
    &[14, 35, 47, 79, 93, 141, 145, 154, 156, 187, 201],
    &[
        20, 32, 75, 93, 101, 102, 103, 106, 112, 116, 127, 130, 133, 136, 167, 189, 195,
    ],
    &[
        18, 26, 34, 65, 87, 108, 120, 127, 139, 155, 164, 178, 179, 201, 205,
    ],
    &[
        3, 24, 27, 34, 39, 43, 48, 53, 56, 60, 63, 67, 68, 79, 116, 124, 150, 185, 193,
    ],
    &[
        16, 17, 22, 29, 34, 35, 37, 41, 49, 59, 105, 135, 144, 151, 174, 189, 196, 199,
    ],
    &[
        10, 19, 25, 32, 34, 42, 62, 83, 89, 96, 115, 141, 148, 160, 182, 183, 184, 194,
    ],
    &[
        23, 34, 44, 47, 54, 61, 78, 117, 118, 131, 132, 142, 165, 166, 167, 170, 191, 203,
    ],
    &[
        4, 22, 33, 40, 48, 50, 84, 107, 109, 121, 129, 146, 157, 167, 178, 184, 206,
    ],
    &[
        6, 13, 25, 51, 63, 72, 80, 90, 104, 129, 130, 147, 155, 180, 196, 203,
    ],
    &[
        0, 2, 11, 26, 79, 85, 95, 101, 111, 129, 134, 149, 158, 170, 183, 188, 199,
    ],
    &[
        31, 32, 44, 57, 59, 64, 67, 70, 113, 123, 125, 129, 139, 140, 177, 190,
    ],
    &[3, 62, 73, 87, 99, 100, 114, 129, 142, 172, 189, 192, 207],
    &[8, 36, 48, 52, 65, 69, 92, 115, 133, 162, 172, 177, 199, 203],
    &[
        31, 42, 71, 73, 94, 97, 102, 162, 171, 178, 188, 191, 193, 196,
    ],
    &[
        16, 45, 63, 70, 81, 106, 120, 121, 128, 138, 161, 162, 165, 183, 207,
    ],
    &[13, 33, 44, 49, 60, 77, 96, 99, 108, 134, 136, 143, 162, 198],
    &[
        1, 18, 32, 39, 117, 119, 126, 144, 147, 149, 157, 162, 168, 186, 192,
    ],
    &[5, 13, 48, 75, 105, 119, 138, 142, 148, 173, 188, 190],
    &[2, 5, 8, 10, 70, 77, 107, 112, 114, 124, 131, 137, 186, 196],
    &[1, 5, 6, 17, 20, 40, 68, 78, 97, 100, 163, 177, 183, 198],
    &[0, 4, 5, 41, 44, 45, 53, 69, 73, 80, 86, 110, 168, 182],
    &[5, 23, 27, 52, 72, 89, 94, 95, 109, 123, 143, 152, 161, 192],
    &[
        24, 36, 37, 50, 64, 77, 81, 86, 90, 111, 132, 163, 169, 171, 173, 192,
    ],
    &[
        43, 51, 71, 92, 110, 126, 128, 137, 140, 142, 152, 158, 169, 198, 206,
    ],
];

/// The code of the packet.
pub fn code() -> &'static LdpcCode {
    static CODE: OnceLock<LdpcCode> = OnceLock::new();
    CODE.get_or_init(|| {
        LdpcCode::from_checks("OGN (208,160)", BITS, DATA_BITS, &CHECKS, InfoAt::Start)
            .expect("the check lists are inside the packet")
    })
}

/// Encrypts the two words (`a`, `b`) with TEA: `cycles` cycles and the key `key`.
pub fn tea_encrypt(a: &mut u32, b: &mut u32, cycles: u32, key: &[u32; 4]) {
    let mut sum = 0u32;
    for _ in 0..cycles {
        sum = sum.wrapping_add(TEA_DELTA);
        *a = a.wrapping_add(
            ((*b << 4).wrapping_add(key[0]))
                ^ b.wrapping_add(sum)
                ^ ((*b >> 5).wrapping_add(key[1])),
        );
        *b = b.wrapping_add(
            ((*a << 4).wrapping_add(key[2]))
                ^ a.wrapping_add(sum)
                ^ ((*a >> 5).wrapping_add(key[3])),
        );
    }
}

/// Decrypts the two words (`a`, `b`) of [`tea_encrypt`].
pub fn tea_decrypt(a: &mut u32, b: &mut u32, cycles: u32, key: &[u32; 4]) {
    let mut sum = TEA_DELTA.wrapping_mul(cycles);
    for _ in 0..cycles {
        *b = b.wrapping_sub(
            ((*a << 4).wrapping_add(key[2]))
                ^ a.wrapping_add(sum)
                ^ ((*a >> 5).wrapping_add(key[3])),
        );
        *a = a.wrapping_sub(
            ((*b << 4).wrapping_add(key[0]))
                ^ b.wrapping_add(sum)
                ^ ((*b >> 5).wrapping_add(key[1])),
        );
        sum = sum.wrapping_sub(TEA_DELTA);
    }
}

const TEA_DELTA: u32 = 0x9E37_79B9;

/// The five words of the 20 data bytes `b`, each least significant byte first.
pub fn words(b: &[u8]) -> [u32; 5] {
    std::array::from_fn(|w| {
        u32::from_le_bytes([b[4 * w], b[4 * w + 1], b[4 * w + 2], b[4 * w + 3]])
    })
}

/// The packet of the five words `w`, given before the whitening: the data words whitened and the check bits added.
pub fn encode(w: &[u32; 5]) -> [u8; PACKET] {
    let mut w = *w;
    let [_, a, b, c, d] = &mut w;
    tea_encrypt(a, b, WHITEN_CYCLES, &WHITEN_KEY);
    tea_encrypt(c, d, WHITEN_CYCLES, &WHITEN_KEY);
    let mut p = [0u8; PACKET];
    for (i, x) in w.iter().enumerate() {
        p[4 * i..4 * i + 4].copy_from_slice(&x.to_le_bytes());
    }
    seal(&mut p);
    p
}

/// Writes the check bits for the 20 data bytes of `p`.
pub fn seal(p: &mut [u8; PACKET]) {
    let bits = to_bits(p);
    let mut out = [0u8; BITS];
    code()
        .encode(&bits[..DATA_BITS], &mut out)
        .expect("the code has an encoder");
    *p = to_bytes(&out);
}

/// The bits of `p` in the order on the air, one for each byte.
pub fn to_bits(p: &[u8; PACKET]) -> [u8; BITS] {
    std::array::from_fn(|i| (p[i / 8] >> (7 - i % 8)) & 1)
}

/// The packet of the bits `bits` in the order on the air.
pub fn to_bytes(bits: &[u8; BITS]) -> [u8; PACKET] {
    let mut p = [0u8; PACKET];
    for (i, b) in bits.iter().enumerate() {
        p[i / 8] |= (b & 1) << (7 - i % 8);
    }
    p
}

/// The parity checks that `p` fails.
pub fn failed_checks(p: &[u8; PACKET]) -> usize {
    CHECKS
        .iter()
        .filter(|c| {
            c.iter()
                .fold(0u8, |s, &i| s ^ (p[usize::from(i) / 8] >> (7 - i % 8)))
                & 1
                != 0
        })
        .count()
}

/// The repair of a packet with failed checks, from the soft values of its bits.
#[derive(Clone, Debug)]
pub struct Repair {
    bp: BpDecoder,
    llr: [f32; BITS],
    out: [u8; BITS],
}

impl Default for Repair {
    fn default() -> Self {
        Repair {
            bp: BpDecoder::new(code()),
            llr: [0.0; BITS],
            out: [0; BITS],
        }
    }
}

impl Repair {
    /// The packet nearest the soft values `soft` (positive is a 1) and the bits changed against their signs. `None` when no
    /// packet with at most [`MAX_REPAIR`] changed bits is found.
    pub fn run(&mut self, soft: &[f32; BITS]) -> Option<([u8; PACKET], u32)> {
        let mean = soft.iter().map(|v| v.abs()).sum::<f32>() / BITS as f32;
        if !(mean > 0.0 && mean.is_finite()) {
            return None;
        }
        // The decoder takes a positive value as a 0. A sure bit gets the value 6.
        let gain = -REPAIR_GAIN / mean;
        for (l, s) in self.llr.iter_mut().zip(soft) {
            *l = (s * gain).clamp(-3.0 * REPAIR_GAIN, 3.0 * REPAIR_GAIN);
        }
        self.bp
            .decode(&self.llr, REPAIR_ITERATIONS, &mut self.out)
            .ok()?;
        let changed = self
            .out
            .iter()
            .zip(soft)
            .filter(|(b, s)| (**b != 0) != (**s > 0.0))
            .count() as u32;
        (changed <= MAX_REPAIR).then(|| (to_bytes(&self.out), changed))
    }
}

const REPAIR_GAIN: f32 = 6.0;

/// The value of a range form with a mantissa of `m` bits: the top 2 bits of the `m + 2` bits of `field` select the step.
pub fn range_value(field: u32, m: u32) -> u32 {
    let base = 1u32 << m;
    let v = field & (base - 1);
    match (field >> m) & 3 {
        0 => v,
        1 => base + 1 + 2 * v,
        2 => 3 * base + 2 + 4 * v,
        _ => 7 * base + 4 + 8 * v,
    }
}

/// The value of a signed range form of `m + 3` bits: the top bit is the sign.
pub fn signed_range_value(field: u32, m: u32) -> i32 {
    let v = range_value(field & ((1 << (m + 2)) - 1), m) as i32;
    if field >> (m + 2) & 1 != 0 {
        -v
    } else {
        v
    }
}

/// The range form with a mantissa of `m` bits that is nearest `value`.
pub fn range_field(value: u32, m: u32) -> u32 {
    let base = 1u32 << m;
    let top = base - 1;
    let (r, v) = if value < base {
        (0, value)
    } else if value < 3 * base {
        (1, value.saturating_sub(base + 1).div_ceil(2))
    } else if value < 7 * base {
        (2, (value.saturating_sub(3 * base + 2) + 2) / 4)
    } else {
        (3, (value.saturating_sub(7 * base + 4) + 4) / 8)
    };
    r << m | v.min(top)
}

/// The signed range form of `m + 3` bits that is nearest `value`.
pub fn signed_range_field(value: i32, m: u32) -> u32 {
    u32::from(value < 0) << (m + 2) | range_field(value.unsigned_abs(), m)
}

/// The header word, which is sent in the clear.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// The 24-bit address.
    pub address: u32,
    /// The address type, 0 to 3.
    pub address_type: u8,
    /// The packet is a status or information packet.
    pub other: bool,
    /// The count of relays.
    pub relay: u8,
    /// The data words are encrypted with a key of the owner.
    pub encrypted: bool,
    /// The pilot signals an emergency.
    pub emergency: bool,
}

impl Header {
    /// The header of the word `w`. `None` when the count of ones in the bits 27 to 0 is odd.
    pub fn of(w: u32) -> Option<Header> {
        (w & 0x0FFF_FFFF)
            .count_ones()
            .is_multiple_of(2)
            .then_some(Header {
                address: w & 0xFF_FFFF,
                address_type: (w >> 24 & 3) as u8,
                other: w >> 26 & 1 != 0,
                relay: (w >> 28 & 3) as u8,
                encrypted: w >> 30 & 1 != 0,
                emergency: w >> 31 != 0,
            })
    }

    /// The word of the header, with its parity bit.
    pub fn word(&self) -> u32 {
        let w = self.address & 0xFF_FFFF
            | u32::from(self.address_type & 3) << 24
            | u32::from(self.other) << 26
            | u32::from(self.relay & 3) << 28
            | u32::from(self.encrypted) << 30
            | u32::from(self.emergency) << 31;
        w | ((w & 0x07FF_FFFF).count_ones() & 1) << 27
    }

    /// The address as 6 hex digits and the name of its type.
    pub fn id(&self) -> String {
        format!(
            "{:06X} {}",
            self.address,
            ADDRESS_TYPES[usize::from(self.address_type & 3)]
        )
    }
}

/// The fields of a position packet.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PositionReport {
    /// The latitude, degrees.
    pub lat_deg: f64,
    /// The longitude, degrees.
    pub lon_deg: f64,
    /// The second of the minute.
    pub second: u8,
    /// The fix quality, 0 to 3.
    pub fix_quality: u8,
    /// The altitude, metres.
    pub alt_m: f64,
    /// The speed, m/s.
    pub speed_m_s: f64,
    /// The turn rate, degrees a second.
    pub turn_deg_s: f64,
    /// The heading, degrees.
    pub heading_deg: f64,
    /// The climb rate, m/s.
    pub climb_m_s: f64,
    /// The sender does not want to be shown by other trackers.
    pub stealth: bool,
    /// The aircraft type, 0 to 15.
    pub aircraft_type: u8,
}

/// A 24-bit two's complement value.
fn signed24(v: u32) -> i32 {
    ((v << 8) as i32) >> 8
}

impl PositionReport {
    /// The fields of the four data words `w` (after the whitening is removed).
    pub fn of(w: &[u32; 4]) -> PositionReport {
        let heading = w[3] & 0x3FF;
        PositionReport {
            lat_deg: f64::from(signed24(w[0]) * 8 + 4) / 600_000.0,
            lon_deg: f64::from(signed24(w[1]) * 16 + 8) / 600_000.0,
            second: (w[0] >> 24 & 0x3F) as u8,
            fix_quality: (w[0] >> 30) as u8,
            alt_m: f64::from(range_value(w[2] & 0x3FFF, 12)),
            speed_m_s: f64::from(range_value(w[2] >> 14 & 0x3FF, 8)) / 10.0,
            turn_deg_s: f64::from(signed_range_value(w[2] >> 24, 5)) / 10.0,
            heading_deg: f64::from((heading * 3600 + 512) / 1024) / 10.0,
            climb_m_s: f64::from(signed_range_value(w[3] >> 10 & 0x1FF, 6)) / 10.0,
            stealth: w[3] >> 19 & 1 != 0,
            aircraft_type: (w[3] >> 20 & 15) as u8,
        }
    }

    /// The four data words of the fields, before the whitening.
    pub fn words(&self) -> [u32; 4] {
        let lat = ((self.lat_deg * 600_000.0 - 4.0) / 8.0).round() as i32;
        let lon = ((self.lon_deg * 600_000.0 - 8.0) / 16.0).round() as i32;
        let heading = (self.heading_deg.rem_euclid(360.0) * 1024.0 / 360.0).round() as u32 & 0x3FF;
        [
            (lat as u32 & 0xFF_FFFF)
                | u32::from(self.second & 0x3F) << 24
                | u32::from(self.fix_quality & 3) << 30,
            lon as u32 & 0xFF_FFFF,
            range_field(self.alt_m.max(0.0).round() as u32, 12)
                | range_field((self.speed_m_s.max(0.0) * 10.0).round() as u32, 8) << 14
                | signed_range_field((self.turn_deg_s * 10.0).round() as i32, 5) << 24,
            heading
                | signed_range_field((self.climb_m_s * 10.0).round() as i32, 6) << 10
                | u32::from(self.stealth) << 19
                | u32::from(self.aircraft_type & 15) << 20,
        ]
    }

    /// The name of the aircraft type.
    pub fn aircraft(&self) -> &'static str {
        AIRCRAFT_TYPES[usize::from(self.aircraft_type & 15)]
    }
}

/// What a packet with good checks holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Content {
    /// A position.
    Position(PositionReport),
    /// A status or information packet. Its layout is not known.
    Status,
    /// A position that is encrypted with a key of the owner. It is not read.
    Encrypted,
}

/// Reads the packet `p`, which fails no check. `None` when the parity of the header is wrong.
pub fn parse(p: &[u8; PACKET]) -> Option<(Header, Content)> {
    let w = words(p);
    let head = Header::of(w[0])?;
    let content = if head.encrypted {
        Content::Encrypted
    } else if head.other {
        Content::Status
    } else {
        let [_, mut a, mut b, mut c, mut d] = w;
        tea_decrypt(&mut a, &mut b, WHITEN_CYCLES, &WHITEN_KEY);
        tea_decrypt(&mut c, &mut d, WHITEN_CYCLES, &WHITEN_KEY);
        Content::Position(PositionReport::of(&[a, b, c, d]))
    };
    Some((head, content))
}

/// The packet of a position report.
pub fn position_packet(head: &Header, r: &PositionReport) -> [u8; PACKET] {
    let [a, b, c, d] = r.words();
    encode(&[head.word(), a, b, c, d])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex<const N: usize>(s: &str) -> [u8; N] {
        let v: Vec<u8> = s
            .split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).expect("hex"))
            .collect();
        v.try_into().expect("the length")
    }

    const CLEAR_1: &str = "56 34 12 0B 88 93 3E 0C 20 77 0E 00 DC C5 1E 00 00 02 10 00";
    const AIR_1: &str =
        "56 34 12 0B BD 1A 5B 4F 14 CB CF 87 9C 6B F5 5D 11 32 55 35 BE BC 80 43 00 E1";
    const CLEAR_2: &str = "EF CD AB 0B 5C 34 D9 0C 70 84 56 00 20 C3 1E 00 00 02 70 00";
    const AIR_2: &str =
        "EF CD AB 0B D3 33 87 3D F1 3B B2 B9 02 8B D6 59 AF 9B DD 5B 5A 9E E4 FD 07 3B";
    const CLEAR_3: &str = "11 47 DD 0A 1A D4 D5 1E 9C FF 63 00 E8 03 4B 00 00 01 10 00";
    const AIR_3: &str =
        "11 47 DD 0A E4 F1 98 52 D2 9B 17 7A A2 A6 1C 77 9C B3 BB F4 FB F9 5C 85 15 2A";

    fn position(p: &[u8; PACKET]) -> (Header, PositionReport) {
        match parse(p).expect("a good header") {
            (h, Content::Position(r)) => (h, r),
            other => panic!("not a position: {other:?}"),
        }
    }

    #[test]
    fn the_check_lists_make_a_code_with_an_encoder() {
        assert_eq!(CHECKS.iter().map(|c| c.len()).sum::<usize>(), 717);
        assert!(CHECKS.iter().all(|c| (10..=21).contains(&c.len())));
        assert!(code().can_encode());
    }

    #[test]
    fn the_three_packets_encode_to_their_bytes_on_the_air() {
        for (clear, air) in [(CLEAR_1, AIR_1), (CLEAR_2, AIR_2), (CLEAR_3, AIR_3)] {
            let p = encode(&words(&hex::<20>(clear)));
            assert_eq!(p, hex::<PACKET>(air));
            assert_eq!(failed_checks(&p), 0);
        }
    }

    #[test]
    fn packet_1_decodes() {
        let p = hex::<PACKET>(AIR_1);
        let (h, r) = position(&p);
        assert_eq!(h.id(), "123456 OGN");
        assert_eq!((h.relay, h.emergency, h.encrypted), (0, false, false));
        assert!((r.lat_deg - 54.68001).abs() < 1e-5 && (r.lon_deg - 25.28001).abs() < 1e-5);
        assert_eq!((r.second, r.fix_quality), (12, 0));
        assert_eq!((r.alt_m, r.speed_m_s, r.turn_deg_s), (1500.0, 12.3, 0.0));
        assert_eq!((r.heading_deg, r.climb_m_s), (180.0, 0.0));
        assert_eq!(r.aircraft(), "glider");
        let mut bad = p;
        bad[3] ^= 1;
        assert_eq!(failed_checks(&bad), 3);
    }

    #[test]
    fn packets_2_and_3_decode() {
        let (h, r) = position(&hex::<PACKET>(AIR_2));
        assert_eq!(h.id(), "ABCDEF OGN");
        assert!((r.lat_deg + 33.89999).abs() < 1e-5 && (r.lon_deg - 151.20001).abs() < 1e-5);
        assert_eq!((r.second, r.alt_m, r.speed_m_s), (12, 800.0, 12.3));
        assert_eq!((r.heading_deg, r.aircraft()), (180.0, "paraglider"));
        let (h, r) = position(&hex::<PACKET>(AIR_3));
        assert_eq!(h.id(), "DD4711 FLARM");
        assert!((r.lat_deg + 36.84999).abs() < 1e-5 && (r.lon_deg - 174.76001).abs() < 1e-5);
        assert_eq!((r.second, r.alt_m, r.speed_m_s), (30, 1000.0, 34.5));
        assert_eq!((r.heading_deg, r.aircraft()), (90.0, "glider"));
    }

    #[test]
    fn the_range_forms_have_their_values() {
        assert_eq!(range_value(0x1064, 12), 4297);
        assert_eq!(range_value(0x3FF, 8), 3836);
        assert_eq!(signed_range_value(0x10A, 6), -10);
        for (v, m) in [
            (0, 12),
            (4095, 12),
            (4297, 12),
            (12_290, 12),
            (61_436, 12),
            (345, 8),
        ] {
            assert_eq!(range_value(range_field(v, m), m), v);
        }
        assert_eq!(signed_range_value(signed_range_field(-10, 6), 6), -10);
    }

    #[test]
    fn the_repair_returns_packet_2_with_one_to_four_wrong_bits() {
        let p = hex::<PACKET>(AIR_2);
        let bits = to_bits(&p);
        let mut fix = Repair::default();
        for wrong in 1..=4 {
            let mut soft: [f32; BITS] =
                std::array::from_fn(|i| if bits[i] != 0 { 1.0 } else { -1.0 });
            for &at in &[7usize, 44, 81, 118][..wrong] {
                soft[at] *= -0.1;
            }
            assert_eq!(
                fix.run(&soft),
                Some((p, wrong as u32)),
                "{wrong} wrong bits"
            );
        }
    }

    #[test]
    fn the_repair_refuses_noise() {
        let mut fix = Repair::default();
        let mut x = 0x1234_5678_9ABC_DEF1u64;
        let mut found = 0;
        for _ in 0..2000 {
            let soft: [f32; BITS] = std::array::from_fn(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x >> 40) as f32 / (1u64 << 23) as f32 - 1.0
            });
            found += u32::from(fix.run(&soft).is_some());
        }
        assert_eq!(found, 0);
    }

    #[test]
    fn the_header_parity_and_the_flags_decide_what_a_packet_is() {
        let clear = hex::<20>(CLEAR_1);
        let with_byte3 = |b: u8| {
            let mut c = clear;
            c[3] = b;
            encode(&words(&c))
        };
        let wrong = with_byte3(0x03);
        assert_eq!(failed_checks(&wrong), 0);
        assert_eq!(parse(&wrong), None);
        let (h, c) = parse(&with_byte3(0x4B)).expect("a header");
        assert_eq!((h.id().as_str(), c), ("123456 OGN", Content::Encrypted));
        let (h, c) = parse(&with_byte3(0x07)).expect("a header");
        assert_eq!((h.id().as_str(), c), ("123456 OGN", Content::Status));
        assert_eq!(h.word() >> 24, 0x07);
    }

    #[test]
    fn a_report_goes_through_its_words() {
        let head = Header {
            address: 0xDD4711,
            address_type: 2,
            ..Header::default()
        };
        assert_eq!(head.word(), 0x0ADD_4711);
        let r = PositionReport {
            lat_deg: -36.85,
            lon_deg: 174.76,
            second: 30,
            alt_m: 1000.0,
            speed_m_s: 34.5,
            heading_deg: 90.0,
            climb_m_s: -1.5,
            turn_deg_s: 3.0,
            aircraft_type: 1,
            ..PositionReport::default()
        };
        let (h, back) = position(&position_packet(&head, &r));
        assert_eq!(h, head);
        assert!((back.lat_deg - r.lat_deg).abs() < 2e-5 && (back.lon_deg - r.lon_deg).abs() < 3e-5);
        assert_eq!(
            (back.alt_m, back.speed_m_s, back.climb_m_s),
            (1000.0, 34.5, -1.5)
        );
        assert_eq!((back.turn_deg_s, back.heading_deg), (3.0, 90.0));
    }
}
