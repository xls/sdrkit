//! NavIC SPS signal constants and the navigation subframe: the Gold ranging codes of the L5 and S signals, and the subframe of
//! 600 symbols (the sync word EB90, then 292 bits coded with the K = 7 code and interleaved 8 by 73), with its CRC-24Q.
//!
//! Standard: IRNSS Signal-in-Space ICD for Standard Positioning Service, version 1.1, sections 3.2 and 5.

use sdrkit_algo_fec::conv::{codes, Encoder, Ends, Viterbi};
use sdrkit_util_bits::crc::catalog::CRC24_LTE_A;

/// The L5 carrier, Hz.
pub const L5_HZ: f64 = 1176.45e6;
/// Chips of a ranging code.
pub const CODE_LEN: usize = 1023;
/// The highest PRN.
pub const MAX_PRN: u16 = 14;
/// The sync word of a subframe.
pub const SYNC: u16 = 0xEB90;
/// Symbols of a subframe.
pub const SUBFRAME_SYMBOLS: usize = 600;
/// Coded symbols of a subframe.
pub const CODED: usize = 584;
/// Bits of a subframe, tail included.
pub const BITS: usize = 292;
/// Rows of the interleaver.
const ROWS: usize = 8;
/// Columns of the interleaver.
const COLS: usize = 73;

/// The G2 start states of the L5 codes (ICD Table 7), PRN 1 first, stage 10 the first character.
const L5_G2: [&str; 14] = [
    "1110100111",
    "0000100110",
    "1000110100",
    "0101110010",
    "1110110000",
    "0001101011",
    "0000010100",
    "0100110000",
    "0010011000",
    "1101100100",
    "0001001100",
    "1101111100",
    "1011010010",
    "0111101010",
];

/// The G2 start states of the S codes (ICD Table 7).
const S_G2: [&str; 14] = [
    "0011101111",
    "0101111101",
    "1000110001",
    "0010101011",
    "1010010001",
    "0100101100",
    "0010001110",
    "0100100110",
    "1100001110",
    "1010111110",
    "1110010001",
    "1101101001",
    "0101000101",
    "0100001101",
];

/// The signal of a ranging code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    /// L5.
    L5,
    /// S.
    S,
}

/// The 1023 chips (0 or 1) of the ranging code of `prn` (1 to 14) on `band`. G1: all ones, feedback stages 3 and 10. G2: the
/// state of Table 7, feedback stages 2, 3, 6, 8, 9 and 10. A chip is stage 10 of G1 XOR stage 10 of G2; then both registers
/// shift towards stage 10 with the feedback into stage 1.
pub fn ranging_code(prn: u16, band: Band) -> Option<Vec<u8>> {
    let table = match band {
        Band::L5 => &L5_G2,
        Band::S => &S_G2,
    };
    let init = table.get(usize::from(prn).checked_sub(1)?)?;
    // Stage s (1 to 10) at index s; the first character of the table is stage 10.
    let mut g1 = [1u8; 11];
    let mut g2 = [0u8; 11];
    for (i, c) in init.bytes().enumerate() {
        g2[10 - i] = c - b'0';
    }
    let mut out = Vec::with_capacity(CODE_LEN);
    for _ in 0..CODE_LEN {
        out.push(g1[10] ^ g2[10]);
        let f1 = g1[3] ^ g1[10];
        let f2 = g2[2] ^ g2[3] ^ g2[6] ^ g2[8] ^ g2[9] ^ g2[10];
        for s in (2..=10).rev() {
            g1[s] = g1[s - 1];
            g2[s] = g2[s - 1];
        }
        g1[1] = f1;
        g2[1] = f2;
    }
    Some(out)
}

/// The fields of a subframe that the stack reads.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Subframe {
    /// The time of week count (bits 9 to 25).
    pub towc: u32,
    /// The alert flag.
    pub alert: bool,
    /// The autonav flag.
    pub autonav: bool,
    /// The subframe ID, 1 to 4.
    pub id: u8,
    /// The week number (subframe 1).
    pub wn: Option<u16>,
    /// The message ID (subframes 3 and 4).
    pub message_id: Option<u8>,
    /// The PRN ID (subframes 3 and 4).
    pub prn_id: Option<u8>,
}

/// Field `[from, from + n)` of `bits` (ICD bit numbers from 1), most significant first.
fn field(bits: &[u8], from: usize, n: usize) -> u32 {
    bits[from - 1..from - 1 + n]
        .iter()
        .fold(0u32, |a, &b| (a << 1) | u32::from(b))
}

/// Writes `v` into field `[from, from + n)` of `bits`.
fn put(bits: &mut [u8], from: usize, n: usize, v: u32) {
    for k in 0..n {
        bits[from - 1 + k] = ((v >> (n - 1 - k)) & 1) as u8;
    }
}

/// Reads the fields of 286 decoded bits (bits 1 to 286). `None` when the CRC-24Q over bits 1 to 262 fails.
pub fn read(bits: &[u8]) -> Option<Subframe> {
    if bits.len() < 286 {
        return None;
    }
    let crc = CRC24_LTE_A.bits(&bits[..262]);
    if crc != field(bits, 263, 24) {
        return None;
    }
    let id = field(bits, 28, 2) as u8 + 1;
    Some(Subframe {
        towc: field(bits, 9, 17),
        alert: bits[25] == 1,
        autonav: bits[26] == 1,
        id,
        wn: (id == 1).then(|| field(bits, 31, 10) as u16),
        message_id: (id >= 3).then(|| field(bits, 31, 6) as u8),
        prn_id: (id >= 3).then(|| field(bits, 257, 6) as u8),
    })
}

/// The 292 bits of a subframe with these fields: `fill` gives the bits the stack does not read (data and TLM), then the CRC
/// and the zero tail.
pub fn bits_of(s: &Subframe, mut fill: impl FnMut() -> u8) -> Vec<u8> {
    let mut b: Vec<u8> = (0..BITS).map(|_| fill() & 1).collect();
    put(&mut b, 9, 17, s.towc);
    b[25] = u8::from(s.alert);
    b[26] = u8::from(s.autonav);
    put(&mut b, 28, 2, u32::from(s.id.clamp(1, 4) - 1));
    b[29] = 0;
    if let Some(wn) = s.wn {
        put(&mut b, 31, 10, u32::from(wn));
    }
    if let Some(m) = s.message_id {
        put(&mut b, 31, 6, u32::from(m));
    }
    if let Some(p) = s.prn_id {
        put(&mut b, 257, 6, u32::from(p));
    }
    let crc = CRC24_LTE_A.bits(&b[..262]);
    put(&mut b, 263, 24, crc);
    b[286..].fill(0);
    b
}

/// The sent symbol index of coded symbol `k`: the coded symbols are written column by column into 8 rows by 73 columns and
/// read row by row, so sent symbol `73 r + c` is coded symbol `8 c + r`.
pub const fn sent_index(k: usize) -> usize {
    (k % ROWS) * COLS + k / ROWS
}

/// The 600 symbols (0 or 1) of a subframe of `bits` (292): the sync word, then the coded and interleaved bits.
pub fn symbols(bits: &[u8]) -> Vec<u8> {
    let mut coded = Vec::with_capacity(CODED);
    Encoder::new(codes::K7_171_133).encode(bits, &mut coded);
    let mut out: Vec<u8> = (0..16).rev().map(|k| ((SYNC >> k) & 1) as u8).collect();
    let mut sent = vec![0u8; CODED];
    for (k, &c) in coded.iter().enumerate() {
        sent[sent_index(k)] = c;
    }
    out.extend(sent);
    out
}

/// The decoder of the 584 coded symbols of a subframe.
pub struct Decoder {
    viterbi: Viterbi,
    llr: Vec<i8>,
    bits: Vec<u8>,
}

impl Default for Decoder {
    fn default() -> Self {
        Decoder {
            viterbi: Viterbi::new(codes::K7_171_133, BITS).expect("a K = 7 code"),
            llr: vec![0; CODED],
            bits: vec![0; BITS],
        }
    }
}

impl Decoder {
    /// Decodes the soft symbols after the sync word (`soft`: 584 values, positive for a 0 symbol, scaled to about 1).
    /// `None` when the CRC fails.
    pub fn decode(&mut self, soft: &[f32]) -> Option<Subframe> {
        if soft.len() != CODED {
            return None;
        }
        for k in 0..CODED {
            self.llr[k] = (soft[sent_index(k)] * 32.0).clamp(-127.0, 127.0) as i8;
        }
        self.viterbi
            .decode_only(&self.llr, Ends::Zero, &mut self.bits)
            .ok()?;
        read(&self.bits[..BITS - 6])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn octal(chips: &[u8]) -> String {
        let v = chips[..10]
            .iter()
            .fold(0u32, |a, &b| (a << 1) | u32::from(b));
        format!("{v:04o}")
    }

    #[test]
    fn the_first_chips_of_every_code() {
        let l5 = [
            "0130", "1731", "0713", "1215", "0117", "1624", "1753", "1317", "1547", "0233", "1663",
            "0203", "0455", "1025",
        ];
        let s = [
            "1420", "1202", "0716", "1524", "0556", "1323", "1561", "1331", "0361", "0501", "0156",
            "0226", "1272", "1362",
        ];
        for prn in 1..=MAX_PRN {
            let i = usize::from(prn - 1);
            assert_eq!(
                octal(&ranging_code(prn, Band::L5).unwrap()),
                l5[i],
                "L5 {prn}"
            );
            assert_eq!(octal(&ranging_code(prn, Band::S).unwrap()), s[i], "S {prn}");
        }
        let c = ranging_code(1, Band::L5).unwrap();
        let first: String = c[..24].iter().map(|b| char::from(b'0' + b)).collect();
        assert_eq!(first, "000101100000110011100010");
        assert!(ranging_code(0, Band::L5).is_none() && ranging_code(15, Band::L5).is_none());
    }

    #[test]
    fn subframes_round_trip_and_check() {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
        let one = Subframe {
            towc: 70_000,
            alert: true,
            id: 1,
            wn: Some(987),
            ..Subframe::default()
        };
        let four = Subframe {
            towc: 70_003,
            id: 4,
            message_id: Some(26),
            prn_id: Some(9),
            ..Subframe::default()
        };
        let mut dec = Decoder::default();
        for s in [&one, &four] {
            let bits = bits_of(s, || (rng.next_u32() & 1) as u8);
            assert_eq!(read(&bits[..286]).as_ref(), Some(s));
            // One flipped bit of 1 to 262 fails the CRC.
            for k in [0, 100, 261] {
                let mut b = bits.clone();
                b[k] ^= 1;
                assert!(read(&b[..286]).is_none());
            }
            let sym = symbols(&bits);
            assert_eq!(sym.len(), SUBFRAME_SYMBOLS);
            let soft: Vec<f32> = sym[16..]
                .iter()
                .map(|&b| 1.0 - 2.0 * f32::from(b))
                .collect();
            assert_eq!(dec.decode(&soft).as_ref(), Some(s));
        }
        assert_eq!(read(&bits_of(&four, || 0)[..286]).unwrap().wn, None);
        // Gaussian noise of sigma 0.6 on the symbols: 30 of 30 decode.
        let mut good = 0;
        for k in 0..30 {
            let s = Subframe {
                towc: 5000 + k,
                id: (k % 4) as u8 + 1,
                ..Subframe::default()
            };
            let bits = bits_of(&s, || (rng.next_u32() & 1) as u8);
            let sym = symbols(&bits);
            let soft: Vec<f32> = sym[16..]
                .iter()
                .map(|&b| 1.0 - 2.0 * f32::from(b) + 0.6 * rng.normal() as f32)
                .collect();
            good += usize::from(dec.decode(&soft).is_some_and(|d| d.towc == s.towc));
        }
        assert_eq!(good, 30);
    }
}
