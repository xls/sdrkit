//! Second-generation beacon messages (C/S T.018): the 202 information bits, BCH(250,202) (generator 0x1C7EB85DF3C97 over GF(2^8)
//! with x^8 + x^4 + x^3 + x^2 + 1, correcting 6 errors), the 23-hex ID (section 3.6), and the normal-mode PRN segments (an
//! x^23 + x^18 + 1 sequence per arm).

#![allow(clippy::unusual_byte_groupings)] // the groups follow the message fields

use super::t001::value_of;
use sdrkit_algo_fec::bch::{codes, Bch};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// The BCH(250, 202) code of a second-generation message, with its working buffer, so a repair allocates nothing.
///
/// Standard: C/S T.018 Appendix B.
#[derive(Clone, Debug)]
pub struct Code {
    bch: Bch,
    word: Vec<u8>,
}

impl Default for Code {
    fn default() -> Self {
        Self::new()
    }
}

impl Code {
    /// The decoder.
    pub fn new() -> Code {
        Code {
            bch: Bch::from_code(&codes::SARSAT_250_202).expect("a named code builds"),
            word: Vec::with_capacity(250),
        }
    }
    /// Repairs the 250 message bits in place. Returns the number of bits corrected, or `None` when the word cannot be repaired
    /// (the bits are then left as received) or `bits` is not 250 long.
    pub fn repair(&mut self, bits: &mut [bool]) -> Option<usize> {
        self.word.clear();
        self.word.extend(bits.iter().map(|&b| b as u8));
        let d = self.bch.decode(&mut self.word).ok()?;
        for (f, &w) in bits.iter_mut().zip(self.word.iter()) {
            *f = w != 0;
        }
        Some(d.corrected as usize)
    }
}

static ENCODER: OnceLock<Bch> = OnceLock::new();

/// Chips per arm per second, chips per channel bit, channel bits per arm.
pub const CHIP_RATE: f64 = 38_400.0;
pub const CHIPS_PER_BIT: usize = 256;
pub const BITS_PER_ARM: usize = 150;
pub const PREAMBLE_BITS: usize = 25;

/// The first 23 chips of the normal-mode I and Q sequences (chip 1 is sent as -1).
const I_SEED: u32 = 0b1000_0000_0000_0000_0000_000;
const Q_SEED: u32 = 0b0011_1111_1000_0011_0101_100;

/// The 38 400 chips of one arm (`q` false: I), as 0 / 1.
pub fn prn(q: bool) -> &'static [u8] {
    static I: OnceLock<Vec<u8>> = OnceLock::new();
    static Q: OnceLock<Vec<u8>> = OnceLock::new();
    let make = |seed: u32| {
        let mut s: Vec<u8> = (0..23).map(|i| ((seed >> (22 - i)) & 1) as u8).collect();
        while s.len() < BITS_PER_ARM * CHIPS_PER_BIT {
            let n = s.len();
            s.push(s[n - 5] ^ s[n - 23]);
        }
        s
    };
    if q {
        Q.get_or_init(|| make(Q_SEED))
    } else {
        I.get_or_init(|| make(I_SEED))
    }
}

/// The first 64 chips of an arm as hex.
pub fn prn_hex64(q: bool) -> String {
    let p = prn(q);
    (0..16)
        .map(|k| {
            let v = (0..4).fold(0u8, |a, i| (a << 1) | p[4 * k + i]);
            std::char::from_digit(v as u32, 16)
                .expect("a value below 16")
                .to_ascii_uppercase()
        })
        .collect()
}

/// A second-generation beacon to encode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SgbSpec {
    pub tac: u16,
    pub serial: u16,
    pub mid: u32,
    pub homing: bool,
    pub rls: bool,
    pub test_protocol: bool,
    pub position: Option<(f64, f64)>,
    pub vessel_id_type: u8,
    pub vessel_id: u64,
    /// 0 ELT, 1 EPIRB, 2 PLB, 3 ELT(DT).
    pub beacon_type: u8,
    pub cancel: bool,
    pub rotating_id: u8,
    pub rotating: u64,
}

/// A decoded second-generation message.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sgb {
    pub hex_id_23: String,
    pub tac: u16,
    pub serial: u16,
    pub mid: u32,
    pub country: String,
    pub beacon_type: String,
    pub homing_121_5: bool,
    pub rls: bool,
    pub test_protocol: bool,
    pub position: bool,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub vessel_id_type: u8,
    pub vessel_id: u64,
    pub cancellation: bool,
    pub rotating_field_id: u8,
    pub bits_corrected: usize,
    pub raw_hex: String,
}

fn put(b: &mut [bool], first: usize, last: usize, v: u64) {
    let n = last - first + 1;
    for i in 0..n {
        b[first - 1 + i] = (v >> (n - 1 - i)) & 1 == 1;
    }
}
fn get(b: &[bool], first: usize, last: usize) -> u64 {
    value_of(&b[first - 1..last])
}

const DEF_LAT: u64 = 0b0_1111111_000001111100000;
const DEF_LON: u64 = 0b0_11111111_111110000011111;

/// The 250 message bits (information then BCH parity).
pub fn encode(s: &SgbSpec) -> Vec<bool> {
    let mut b = vec![false; 202];
    put(&mut b, 1, 16, s.tac as u64);
    put(&mut b, 17, 30, s.serial as u64);
    put(&mut b, 31, 40, s.mid as u64);
    b[40] = s.homing;
    b[41] = s.rls;
    b[42] = s.test_protocol;
    match s.position {
        Some((lat, lon)) => {
            let la = lat.abs();
            let lo = lon.abs();
            let lf = ((la.fract() * 32768.0).round() as u64).min(32767);
            let of = ((lo.fract() * 32768.0).round() as u64).min(32767);
            put(&mut b, 44, 44, (lat < 0.0) as u64);
            put(&mut b, 45, 51, la.trunc() as u64);
            put(&mut b, 52, 66, lf);
            put(&mut b, 67, 67, (lon < 0.0) as u64);
            put(&mut b, 68, 75, lo.trunc() as u64);
            put(&mut b, 76, 90, of);
        }
        None => {
            put(&mut b, 44, 66, DEF_LAT);
            put(&mut b, 67, 90, DEF_LON);
        }
    }
    put(&mut b, 91, 93, s.vessel_id_type as u64);
    put(&mut b, 94, 137, s.vessel_id);
    put(&mut b, 138, 140, s.beacon_type as u64);
    put(&mut b, 141, 154, if s.cancel { 0 } else { 0x3FFF });
    put(&mut b, 155, 158, s.rotating_id as u64);
    put(&mut b, 159, 202, s.rotating);
    let bch = ENCODER
        .get_or_init(|| Bch::from_code(&codes::SARSAT_250_202).expect("a named code builds"));
    let mut w: Vec<u8> = b.iter().map(|&x| x as u8).collect();
    w.resize(codes::SARSAT_250_202.n, 0);
    // The length is the code's own, so encoding cannot fail.
    let _ = bch.encode(&mut w);
    b.extend(w[codes::SARSAT_250_202.k..].iter().map(|&x| x != 0));
    b
}

/// The 23-hex ID of T.018 section 3.6: 1, country, 101, TAC, serial, test protocol, vessel id type, vessel id (92 bits).
pub fn hex_id_23(b: &[bool]) -> String {
    let mut id = Vec::with_capacity(92);
    id.push(true);
    id.extend_from_slice(&b[30..40]);
    id.extend_from_slice(&[true, false, true]);
    id.extend_from_slice(&b[0..30]);
    id.push(b[42]);
    id.extend_from_slice(&b[90..137]);
    let mut s = String::new();
    for c in id.chunks(4) {
        let v = value_of(c) as u32;
        s.push(
            std::char::from_digit(v, 16)
                .expect("a value below 16")
                .to_ascii_uppercase(),
        );
    }
    s
}

/// Decodes 250 message bits with the repair of `code` (up to 6 errors). Returns `None` when the message is short, the BCH
/// fails, or the spare bits hold another value.
pub fn decode(raw: &[bool], code: &mut Code) -> Option<Sgb> {
    if raw.len() < 250 {
        return None;
    }
    let mut b = raw[..250].to_vec();
    let corrected = code.repair(&mut b)?;
    let spare = get(&b, 141, 154);
    if spare != 0x3FFF && spare != 0 {
        return None;
    }
    let mid = get(&b, 31, 40) as u32;
    let lat_bits = get(&b, 44, 66);
    let lon_bits = get(&b, 67, 90);
    let (position, lat, lon) = if lat_bits == DEF_LAT && lon_bits == DEF_LON {
        (false, None, None)
    } else {
        let mut lat = get(&b, 45, 51) as f64 + get(&b, 52, 66) as f64 / 32768.0;
        let mut lon = get(&b, 68, 75) as f64 + get(&b, 76, 90) as f64 / 32768.0;
        if b[43] {
            lat = -lat;
        }
        if b[66] {
            lon = -lon;
        }
        (true, Some(lat), Some(lon))
    };
    let bt = get(&b, 138, 140) as u8;
    let mut hex = String::new();
    for c in b.chunks(4) {
        let n = c.len();
        let v = (value_of(c) << (4 - n)) as u32;
        hex.push(
            std::char::from_digit(v, 16)
                .expect("a value below 16")
                .to_ascii_uppercase(),
        );
    }
    Some(Sgb {
        hex_id_23: hex_id_23(&b),
        tac: get(&b, 1, 16) as u16,
        serial: get(&b, 17, 30) as u16,
        mid,
        country: sdrkit_util_ids::mid::country(mid as u16)
            .unwrap_or("")
            .to_string(),
        beacon_type: match bt {
            0 => "ELT",
            1 => "EPIRB",
            2 => "PLB",
            3 => "ELT(DT)",
            _ => "?",
        }
        .into(),
        homing_121_5: b[40],
        rls: b[41],
        test_protocol: b[42],
        position,
        lat,
        lon,
        vessel_id_type: get(&b, 91, 93) as u8,
        vessel_id: get(&b, 94, 137),
        cancellation: spare == 0,
        rotating_field_id: get(&b, 155, 158) as u8,
        bits_corrected: corrected,
        raw_hex: hex,
    })
}

/// The second-generation test beacons of the generator (made-up).
pub fn test_beacons() -> Vec<SgbSpec> {
    let base = SgbSpec {
        tac: 0,
        serial: 0,
        mid: 0,
        homing: false,
        rls: false,
        test_protocol: false,
        position: None,
        vessel_id_type: 0,
        vessel_id: 0,
        beacon_type: 1,
        cancel: false,
        rotating_id: 0,
        rotating: 0,
    };
    vec![
        SgbSpec {
            tac: 4711,
            serial: 1234,
            mid: 277,
            homing: true,
            position: Some((55.7041, 20.9873)),
            beacon_type: 1,
            rotating: 0x1234_5678_9,
            ..base.clone()
        },
        SgbSpec {
            tac: 65535,
            serial: 16383,
            mid: 512,
            position: Some((-41.2865, -174.77621)),
            beacon_type: 2,
            rotating_id: 1,
            ..base.clone()
        },
        SgbSpec {
            tac: 230,
            serial: 573,
            mid: 201,
            homing: true,
            rls: true,
            vessel_id_type: 3,
            vessel_id: 0x3C6444123,
            beacon_type: 0,
            rotating_id: 2,
            ..base.clone()
        },
        SgbSpec {
            tac: 100,
            serial: 7,
            mid: 366,
            test_protocol: true,
            position: Some((37.7749, -122.4194)),
            beacon_type: 2,
            ..base
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prn_segments() {
        assert_eq!(prn_hex64(false), "8000010842128 4A1".replace(' ', ""));
        assert_eq!(prn_hex64(true), "3F8358BAD030F231");
    }

    #[test]
    fn six_errors_are_repaired() {
        let s = &test_beacons()[0];
        let good = encode(s);
        let mut b = good.clone();
        for k in 0..6 {
            b[7 + 41 * k] = !b[7 + 41 * k];
        }
        let d = decode(&b, &mut Code::new()).unwrap();
        assert_eq!(d.bits_corrected, 6);
        assert_eq!(d.tac, 4711);
    }

    #[test]
    fn hex_ids_of_the_test_beacons() {
        let want = [
            "A2B4499C4D2000000000000",
            "C017FFFFFFF000000000000",
            "9934039823D3003C6444123",
            "ADD40190007800000000000",
        ];
        for (s, w) in test_beacons().iter().zip(want) {
            let d = decode(&encode(s), &mut Code::new()).unwrap();
            assert_eq!(d.hex_id_23, w);
        }
    }
}
