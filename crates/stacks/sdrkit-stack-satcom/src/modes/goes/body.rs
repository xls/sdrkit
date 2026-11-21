//! GOES DCS message layer: the frame sync, the BCH (31,21) address, the characters with odd parity, the EOT, the
//! data offset by parity, pseudo-binary B and ASCII fields, and the channel plan.

use sdrkit_algo_fec::bch::{codes, Bch};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// The 15-bit frame sync.
pub const SYNC: [bool; 15] = [
    true, false, false, false, true, false, false, true, true, false, true, false, true, true, true,
];
/// The end of transmission character.
pub const EOT: u8 = 0x04;
/// The frequency of channel 1, Hz.
pub const CH1_HZ: f64 = 401.70175e6;
/// The channel spacing, Hz.
pub const CH_STEP_HZ: f64 = 1.5e3;
/// The number of channels.
pub const CHANNELS: usize = 266;

/// The address code, BCH (31, 21) with the generator x^10 + x^9 + x^8 + x^6 + x^5 + x^3 + 1 (it corrects 2 bits), with its working
/// buffer, so a repair allocates nothing.
#[derive(Clone, Debug)]
pub struct AddressCode {
    bch: Bch,
    word: Vec<u8>,
}

impl Default for AddressCode {
    fn default() -> Self {
        Self::new()
    }
}

impl AddressCode {
    /// The decoder.
    pub fn new() -> AddressCode {
        AddressCode {
            bch: Bch::from_code(&codes::GOES_DCS_31_21).expect("a named code builds"),
            word: Vec::with_capacity(31),
        }
    }
    /// Repairs a 31-bit address word in place. Returns the number of bits corrected, or `None` when it cannot be repaired (the
    /// bits are then left as received) or the word is not 31 bits long.
    pub fn decode(&mut self, word: &mut [bool]) -> Option<usize> {
        self.word.clear();
        self.word.extend(word.iter().map(|&b| b as u8));
        let d = self.bch.decode(&mut self.word).ok()?;
        for (w, &b) in word.iter_mut().zip(self.word.iter()) {
            *w = b != 0;
        }
        Some(d.corrected as usize)
    }
}

/// The address word of 21 data bits.
pub fn encode_address(data: u32) -> Vec<bool> {
    static ENC: OnceLock<Bch> = OnceLock::new();
    let bch =
        ENC.get_or_init(|| Bch::from_code(&codes::GOES_DCS_31_21).expect("a named code builds"));
    let mut w: Vec<u8> = (0..21).rev().map(|i| ((data >> i) & 1) as u8).collect();
    w.resize(31, 0);
    // The length is the code's own, so encoding cannot fail.
    let _ = bch.encode(&mut w);
    w.iter().map(|&b| b != 0).collect()
}

/// The channel of a frequency (0 when outside the plan).
pub fn channel_of(f: f64) -> usize {
    let n = ((f - CH1_HZ) / CH_STEP_HZ).round() as i64 + 1;
    if (1..=CHANNELS as i64).contains(&n) {
        n as usize
    } else {
        0
    }
}

/// The frequency of channel `n`, Hz.
pub fn channel_hz(n: usize) -> f64 {
    CH1_HZ + (n as f64 - 1.0) * CH_STEP_HZ
}

/// A character with its odd parity bit (bit 7).
pub fn with_parity(c: u8) -> u8 {
    let c = c & 0x7F;
    if c.count_ones().is_multiple_of(2) {
        c | 0x80
    } else {
        c
    }
}

/// Whether a byte (7 bits and its parity bit) has odd parity.
pub fn parity_ok(b: u8) -> bool {
    b.count_ones() % 2 == 1
}

/// Bits of a byte, least significant first.
pub fn byte_bits(b: u8) -> [bool; 8] {
    let mut o = [false; 8];
    for (i, v) in o.iter_mut().enumerate() {
        *v = (b >> i) & 1 == 1;
    }
    o
}

sdrkit_core::choice_enum! {
    /// The format of a report's data.
    #[derive(Default, Serialize, Deserialize)]
    pub enum DataFormat {
        /// ASCII characters.
        Ascii = ("ascii", "ASCII"),
        /// Pseudo-binary B.
        PseudoBinary = ("pseudo_binary", "Pseudo-binary B"),
        /// Another format.
        #[default]
        Other = ("other", "Other"),
    }
}

/// A decoded body.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Body {
    /// The flag word (-1 when the format has none).
    pub flags: i32,
    pub eot: bool,
    pub parity_errors: usize,
    pub chars: Vec<u8>,
    pub text: String,
    pub format: DataFormat,
    pub fields: Vec<String>,
}

/// Reads characters from the bits after the address: the data offset (0, or 9 for the extended format: a spare bit and an 8-bit
/// flag word) is chosen by the parity of the characters; characters are read up to the EOT.
pub fn read_body(bits: &[bool], extended_allowed: bool) -> Body {
    let read = |off: usize| -> (Vec<u8>, usize, bool) {
        let mut chars = Vec::new();
        let mut perr = 0;
        let mut eot = false;
        let mut i = off;
        while i + 8 <= bits.len() {
            let b = bits[i..i + 8]
                .iter()
                .enumerate()
                .fold(0u8, |v, (k, &x)| v | (x as u8) << k);
            i += 8;
            if b == EOT {
                eot = true;
                break;
            }
            if !parity_ok(b) {
                perr += 1;
            }
            chars.push(b & 0x7F);
            if chars.len() >= 4096 {
                break;
            }
        }
        (chars, perr, eot)
    };
    let a = read(0);
    let b = if extended_allowed && bits.len() >= 9 {
        Some(read(9))
    } else {
        None
    };
    // The offset whose characters hold their parity better (and which reaches an EOT) wins.
    let score = |r: &(Vec<u8>, usize, bool)| {
        (r.1 as f64 / r.0.len().max(1) as f64) - if r.2 { 0.5 } else { 0.0 }
    };
    let (use_b, (chars, perr, eot)) = match b {
        Some(b) if score(&b) < score(&a) => (true, b),
        _ => (false, a),
    };
    let flags = if use_b {
        bits[1..9]
            .iter()
            .enumerate()
            .fold(0i32, |v, (k, &x)| v | (x as i32) << k)
    } else {
        -1
    };
    let fmt = format_of(&chars);
    let text: String = chars
        .iter()
        .map(|&c| {
            if c < 0x20 || c == 0x7F {
                ' '
            } else {
                c as char
            }
        })
        .collect();
    let fields = match fmt {
        DataFormat::PseudoBinary => pseudo_binary(&chars),
        _ => ascii_fields(&text),
    };
    Body {
        flags,
        eot,
        parity_errors: perr,
        chars,
        text,
        format: fmt,
        fields,
    }
}

/// Pseudo-binary B when at least 3 characters, at least 90 % of them 0x40 or above, and fewer than 5 % look like plain numbers.
pub fn format_of(c: &[u8]) -> DataFormat {
    if c.is_empty() {
        return DataFormat::Other;
    }
    let hi = c.iter().filter(|&&x| x >= 0x40).count();
    let num = c
        .iter()
        .filter(|&&x| x == b' ' || x.is_ascii_digit() || x == b',' || x == b'.')
        .count();
    if c.len() >= 3 && hi * 10 >= c.len() * 9 && num * 20 < c.len() {
        DataFormat::PseudoBinary
    } else if c
        .iter()
        .all(|&x| (0x20..0x7F).contains(&x) || x == b'\r' || x == b'\n' || x == b'\t')
    {
        DataFormat::Ascii
    } else {
        DataFormat::Other
    }
}

/// Pseudo-binary values in groups of three characters (18 bits; also shown signed when the top bit is set).
pub fn pseudo_binary(c: &[u8]) -> Vec<String> {
    c.chunks(3)
        .filter(|g| g.len() == 3)
        .enumerate()
        .map(|(i, g)| {
            let v = g.iter().fold(0u32, |a, &x| (a << 6) | (x & 0x3F) as u32);
            if v & 0x2_0000 != 0 {
                format!("{}: {} ({} signed)", i + 1, v as i32 - (1 << 18), v)
            } else {
                format!("{}: {v}", i + 1)
            }
        })
        .take(64)
        .collect()
}

/// ASCII tokens; a label followed by a number becomes one field ("WL 12.34"); at most 64.
pub fn ascii_fields(t: &str) -> Vec<String> {
    let toks: Vec<&str> = t.split_whitespace().collect();
    let is_num = |s: &str| s.parse::<f64>().is_ok();
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() && out.len() < 64 {
        if !is_num(toks[i]) && i + 1 < toks.len() && is_num(toks[i + 1]) {
            out.push(format!("{} {}", toks[i], toks[i + 1]));
            i += 2;
        } else {
            out.push(toks[i].to_string());
            i += 1;
        }
    }
    out
}

/// The bits a platform sends after its carrier: preamble, sync, address, (spare and flags), characters, EOT.
pub fn message_bits(preamble: usize, address21: u32, flags: Option<u8>, text: &[u8]) -> Vec<bool> {
    let mut b = Vec::new();
    for i in 0..preamble {
        b.push(i % 2 == 0);
    }
    // The preamble ends with a 0 before the sync's leading 1.
    if b.last() == Some(&true) {
        b.push(false);
    }
    b.extend_from_slice(&SYNC);
    b.extend(encode_address(address21));
    if let Some(f) = flags {
        b.push(false);
        b.extend((0..8).map(|i| (f >> i) & 1 == 1));
    }
    for &c in text {
        b.extend(byte_bits(with_parity(c)));
    }
    b.extend(byte_bits(EOT));
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d1_codes_and_channels() {
        let mut code = AddressCode::new();
        let w = encode_address(0x12345);
        assert_eq!(&w[..21], &encode_address(0x12345)[..21]);
        let mut c = w.clone();
        assert_eq!(code.decode(&mut c), Some(0));
        // Every single and double error is corrected.
        for i in 0..31 {
            for j in i..31 {
                let mut e = w.clone();
                e[i] = !e[i];
                if j != i {
                    e[j] = !e[j];
                }
                assert_eq!(code.decode(&mut e), Some(if i == j { 1 } else { 2 }));
                assert_eq!(e, w);
            }
        }
        // At least 45 % of triple errors detected.
        let mut detected = 0;
        let mut total = 0;
        for i in (0..31).step_by(2) {
            for j in (i + 1..31).step_by(3) {
                for k in (j + 1..31).step_by(2) {
                    let mut e = w.clone();
                    e[i] = !e[i];
                    e[j] = !e[j];
                    e[k] = !e[k];
                    total += 1;
                    if code.decode(&mut e).is_none() {
                        detected += 1;
                    }
                }
            }
        }
        assert!(detected * 100 >= total * 45, "{detected} of {total}");
        assert_eq!(
            SYNC.iter()
                .map(|&b| if b { '1' } else { '0' })
                .collect::<String>(),
            "100010011010111"
        );
        assert!(parity_ok(EOT));
        for c in 0..128u8 {
            assert!(parity_ok(with_parity(c)));
        }
        assert_eq!(channel_of(channel_hz(1)), 1);
        assert_eq!(channel_of(channel_hz(266)), 266);
        assert_eq!(channel_of(100e6), 0);
        assert_eq!(channel_of(channel_hz(100) + 500.0), 100);
        assert_eq!(channel_of(channel_hz(100) + 800.0), 101);
    }

    #[test]
    fn d2_body_parsing() {
        let text = b"WL 12.34 TW 18.5 BV 12.6";
        // Plain and extended.
        for flags in [None, Some(0x5Au8)] {
            let bits = message_bits(0, 0, flags, text);
            let after = &bits[15 + 31..];
            let b = read_body(after, true);
            assert!(b.eot);
            assert_eq!(b.parity_errors, 0);
            assert_eq!(b.text, "WL 12.34 TW 18.5 BV 12.6");
            assert_eq!(b.flags, flags.map(|f| f as i32).unwrap_or(-1));
            assert_eq!(b.fields[0], "WL 12.34");
            assert_eq!(b.format, DataFormat::Ascii);
        }
        // Pseudo-binary: 337 = 5 * 64 + 17 in groups of three characters.
        let pb: Vec<u8> = [0u32, 5, 17, 63, 63, 52, 1, 2, 3, 4, 5, 6, 7, 8, 9]
            .iter()
            .map(|&v| 0x40 | v as u8)
            .collect();
        let b = read_body(&message_bits(0, 0, None, &pb)[46..], false);
        assert_eq!(b.format, DataFormat::PseudoBinary);
        assert_eq!(b.fields.len(), 5);
        assert_eq!(b.fields[0], "1: 337");
        assert!(b.fields[1].contains("signed"), "{}", b.fields[1]);
        // One parity error is counted.
        let mut bits = message_bits(0, 0, None, text);
        bits[46 + 8 * 3 + 2] = !bits[46 + 8 * 3 + 2];
        assert_eq!(read_body(&bits[46..], false).parity_errors, 1);
    }
}
