//! `file_transfer`: one file over the air in frames of LDPC blocks (n = 7680), each with its own CRC-32, and a hash of the
//! whole file. Three widths (1 MHz, 500 kHz, 200 kHz at 800, 400 and 160 ksymbols a second) and ten profiles from BPSK rate
//! 1/4 to 1024-QAM rate 9/10. A frame is a Zadoff-Chu preamble of 512 symbols (the root names the width), a header of 122
//! bits (rate 1/2 convolutional code, BPSK), and the scrambled blocks with a pilot before every 31 block symbols. The
//! session layer ([`session`]) moves the file in bursts of up to 16 frames.

pub mod rx;
pub mod session;
pub mod stack;

pub use stack::FILE_TRANSFER;

use crate::family::{self, ldpc, push_bits, push_bytes, qam, read_bits};
use sdrkit_algo_dsp::blackman_rational::BlackmanRational;
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_fec::conv::codes::K7_171_133;
use sdrkit_algo_fec::conv::Encoder;
use sdrkit_algo_modem::zadoff_chu::zadoff_chu;
use sdrkit_core::Complex32;
use sdrkit_util_bits::xorshift::Xorshift64;
use std::sync::OnceLock;

/// The LDPC code length.
pub const N: usize = 7680;
/// Samples a symbol at the internal rate.
pub const SPS: usize = 4;
/// The base rate of the transmit path, samples a second.
pub const BASE_RATE: f64 = 3_200_000.0;
/// Length of the Zadoff-Chu sequence.
pub const ZC_LEN: usize = 251;
/// Preamble symbols: the last 10 of the sequence, then the sequence twice.
pub const PREAMBLE: usize = 512;
/// Header information bits (89 field bits, the CRC-16 and 17 zero bits).
pub const HEADER_INFO: usize = 122;
/// Header field bits.
pub const HEADER_FIELDS: usize = 89;
/// Header symbols.
pub const HEADER_SYMBOLS: usize = 256;
/// Symbols of a frame before its payload.
pub const FIXED: usize = PREAMBLE + HEADER_SYMBOLS;
/// The most blocks of a control or acknowledgement frame.
pub const MAX_CONTROL_BLOCKS: usize = 5;
/// The most frames of a burst.
pub const MAX_FRAMES: usize = 16;
/// The level of the default transmitter.
pub const TX_LEVEL: f32 = 0.35;
/// The high word of the block scrambler seed.
const SCRAMBLE_KEY: u64 = 0x4B41_4654_0000_0000;
/// The multiplier of the block scrambler seed.
const SCRAMBLE_MUL: u64 = 0xD6E8_FEB8_6659_FD93;
/// The state of the pilot generator.
const PILOT_SEED: u64 = 0xA5C3_961F_2B4D_7E81;

/// A channel width.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Width {
    /// 1 MHz: 800 ksymbols a second.
    W1M = 0,
    /// 500 kHz: 400 ksymbols a second.
    W500K = 1,
    /// 200 kHz: 160 ksymbols a second.
    W200K = 2,
}

/// The three widths in code order.
pub const WIDTHS: [Width; 3] = [Width::W1M, Width::W500K, Width::W200K];

impl Width {
    /// The width of code 0, 1 or 2.
    pub fn of(code: u8) -> Option<Width> {
        WIDTHS.get(usize::from(code)).copied()
    }

    /// The width code.
    pub fn code(self) -> u8 {
        self as u8
    }

    /// Symbols a second.
    pub fn symbol_rate(self) -> f64 {
        match self {
            Width::W1M => 800_000.0,
            Width::W500K => 400_000.0,
            Width::W200K => 160_000.0,
        }
    }

    /// The internal rate, 4 samples a symbol.
    pub fn rate(self) -> f64 {
        SPS as f64 * self.symbol_rate()
    }

    /// The most symbols of a data frame.
    pub fn budget(self) -> usize {
        match self {
            Width::W1M | Width::W500K => 32_000,
            Width::W200K => 16_000,
        }
    }

    /// The Zadoff-Chu root of the preamble.
    pub fn root(self) -> i64 {
        match self {
            Width::W1M => 13,
            Width::W500K => 11,
            Width::W200K => 7,
        }
    }

    /// The factor from the internal rate to [`BASE_RATE`].
    pub fn factor(self) -> u64 {
        match self {
            Width::W1M => 1,
            Width::W500K => 2,
            Width::W200K => 5,
        }
    }

    /// The occupied bandwidth, Hz.
    pub fn bandwidth(self) -> f64 {
        1.25 * self.symbol_rate()
    }

    /// The name of the width.
    pub fn name(self) -> &'static str {
        match self {
            Width::W1M => "1 MHz",
            Width::W500K => "500 kHz",
            Width::W200K => "200 kHz",
        }
    }
}

/// One profile: a modulation and an LDPC rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    /// Bits a symbol.
    pub bits: usize,
    /// The LDPC rate index of the family.
    pub rate: usize,
    /// The Es/N0 of a frame error rate near 1e-2, dB.
    pub required_db: f64,
}

/// The ten profiles by id.
pub const PROFILES: [Profile; 10] = [
    Profile { bits: 1, rate: 0, required_db: -0.75 },
    Profile { bits: 2, rate: 0, required_db: 2.25 },
    Profile { bits: 2, rate: 2, required_db: 1.5 },
    Profile { bits: 2, rate: 4, required_db: 4.1 },
    Profile { bits: 4, rate: 4, required_db: 10.0 },
    Profile { bits: 6, rate: 4, required_db: 14.9 },
    Profile { bits: 8, rate: 4, required_db: 19.8 },
    Profile { bits: 8, rate: 5, required_db: 21.8 },
    Profile { bits: 10, rate: 5, required_db: 26.9 },
    Profile { bits: 10, rate: 6, required_db: 28.9 },
];

/// The profile of control and acknowledgement frames.
pub const CONTROL_PROFILE: usize = 1;

impl Profile {
    /// The profile of id `id`; `None` above 9.
    pub fn of(id: usize) -> Option<Profile> {
        PROFILES.get(id).copied()
    }

    /// Information bits of a block: the payload and its CRC-32.
    pub fn k(&self) -> usize {
        ldpc::base(N, self.rate).map_or(0, |b| b.k)
    }

    /// Payload bytes of a block.
    pub fn payload(&self) -> usize {
        self.k() / 8 - 4
    }

    /// File bytes of a data block.
    pub fn data(&self) -> usize {
        self.payload() - 6
    }

    /// Symbols of a block.
    pub fn symbols(&self) -> usize {
        N / self.bits
    }
}

/// Pilots of `d` block symbols: one before every 31.
pub fn pilots_of(d: usize) -> usize {
    d.div_ceil(31)
}

/// Symbols of a frame of `blocks` blocks at `profile`.
pub fn frame_symbols(profile: &Profile, blocks: usize) -> usize {
    let d = blocks * profile.symbols();
    FIXED + d + pilots_of(d)
}

/// The most blocks of a data frame of `profile` in `width` (at least 1).
pub fn max_blocks(profile: &Profile, width: Width) -> usize {
    let mut nb = 1;
    while frame_symbols(profile, nb + 1) <= width.budget() {
        nb += 1;
    }
    nb
}

/// The callsign alphabet; the code of a character is its index plus 1.
const ALPHABET: &[u8; 39] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ/-.";

/// Whether every character of `call` is in the callsign alphabet (any case).
pub fn callsign_chars_ok(call: &str) -> bool {
    call.bytes().all(|c| ALPHABET.contains(&c.to_ascii_uppercase()))
}

/// The 36-bit callsign field: six 6-bit codes, upper case, 38 for a character outside the alphabet, 0 after the end.
pub fn callsign_code(call: &str) -> u64 {
    let b = call.as_bytes();
    (0..6).fold(0u64, |v, i| {
        let c = match b.get(i) {
            None => 0,
            Some(c) => ALPHABET.iter().position(|&a| a == c.to_ascii_uppercase()).map_or(38, |p| p as u64 + 1),
        };
        (v << 6) | c
    })
}

/// The callsign of a 36-bit field: codes up to the first 0, a code above 39 skipped.
pub fn callsign_of(code: u64) -> String {
    let mut s = String::new();
    for i in (0..6).rev() {
        let c = ((code >> (6 * i)) & 63) as usize;
        if c == 0 {
            break;
        }
        if c <= 39 {
            s.push(char::from(ALPHABET[c - 1]));
        }
    }
    s
}

/// The kind of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameKind {
    /// File data.
    Data = 0,
    /// A session message.
    Control = 1,
    /// An acknowledgement message.
    Ack = 2,
}

impl FrameKind {
    fn of(code: u64) -> Option<FrameKind> {
        match code {
            0 => Some(FrameKind::Data),
            1 => Some(FrameKind::Control),
            2 => Some(FrameKind::Ack),
            _ => None,
        }
    }
}

/// The header of a frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameHeader {
    /// The width.
    pub width: Width,
    /// The profile id, 0 to 9.
    pub profile: u8,
    /// The kind.
    pub kind: FrameKind,
    /// The session id.
    pub session: u16,
    /// The frame number (burst number << 4 | index for data frames).
    pub frame: u16,
    /// Blocks, at least 1.
    pub blocks: u8,
    /// The frame index in the burst, 0 to 15.
    pub index: u8,
    /// The last frame of the burst.
    pub last: bool,
    /// The sending station.
    pub callsign: String,
}

impl FrameHeader {
    /// The 122 information bits: the fields, the CRC-16 of the fields and 17 zero bits.
    pub fn bits(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(HEADER_INFO);
        push_bits(&mut b, u64::from(self.width.code()), 2);
        push_bits(&mut b, u64::from(self.profile & 15), 4);
        push_bits(&mut b, self.kind as u64, 2);
        push_bits(&mut b, u64::from(self.session), 16);
        push_bits(&mut b, u64::from(self.frame), 16);
        push_bits(&mut b, u64::from(self.blocks), 8);
        push_bits(&mut b, u64::from(self.index & 15), 4);
        push_bits(&mut b, u64::from(self.last), 1);
        push_bits(&mut b, callsign_code(&self.callsign), 36);
        let crc = family::crc16(&family::pack(&b));
        push_bits(&mut b, u64::from(crc), 16);
        b.resize(HEADER_INFO, 0);
        b
    }

    /// The profile of the header.
    pub fn profile(&self) -> Profile {
        PROFILES[usize::from(self.profile).min(9)]
    }

    /// Whether the block count is allowed: 1 to 5 for a control or acknowledgement frame, 1 to the most of the profile and
    /// width for a data frame.
    pub fn blocks_ok(&self) -> bool {
        let nb = usize::from(self.blocks);
        nb >= 1
            && match self.kind {
                FrameKind::Data => nb <= max_blocks(&self.profile(), self.width),
                _ => nb <= MAX_CONTROL_BLOCKS,
            }
    }
}

/// Reads 122 header bits: `None` when the CRC fails, or the width, profile or kind is out of range. The 17 zero bits are not
/// checked; [`FrameHeader::blocks_ok`] and the width of the preamble are the receiver's other checks.
pub fn parse_header(bits: &[u8]) -> Option<FrameHeader> {
    if bits.len() < HEADER_FIELDS + 16 {
        return None;
    }
    if family::crc16(&family::pack(&bits[..HEADER_FIELDS])) != read_bits(bits, HEADER_FIELDS, 16) as u16 {
        return None;
    }
    let profile = read_bits(bits, 2, 4) as u8;
    if profile > 9 {
        return None;
    }
    Some(FrameHeader {
        width: Width::of(read_bits(bits, 0, 2) as u8)?,
        profile,
        kind: FrameKind::of(read_bits(bits, 6, 2))?,
        session: read_bits(bits, 8, 16) as u16,
        frame: read_bits(bits, 24, 16) as u16,
        blocks: read_bits(bits, 40, 8) as u8,
        index: read_bits(bits, 48, 4) as u8,
        last: bits[52] == 1,
        callsign: callsign_of(read_bits(bits, 53, 36)),
    })
}

/// The 256 coded header bits: rate 1/2, K = 7, terminated.
pub fn code_header(bits: &[u8]) -> Vec<u8> {
    let mut enc = Encoder::new(K7_171_133);
    let mut out = Vec::with_capacity(HEADER_SYMBOLS);
    enc.encode(bits, &mut out);
    enc.terminate(&mut out);
    out
}

/// The seed of the block scrambler of block `block` of frame `frame`.
pub fn scrambler_seed(frame: u16, block: usize) -> u64 {
    let s = SCRAMBLE_KEY ^ (u64::from(frame) << 24) ^ ((block as u64) << 8) ^ Xorshift64::GOLDEN;
    let s = s.wrapping_mul(SCRAMBLE_MUL);
    if s == 0 {
        1
    } else {
        s
    }
}

/// XORs the information bits `bits` of block `block` of frame `frame` with the scrambler sequence.
pub fn scramble(bits: &mut [u8], frame: u16, block: usize) {
    let mut g = Xorshift64::new(scrambler_seed(frame, block));
    for _ in 0..4 {
        g.step();
    }
    let mut w = 0;
    for (i, b) in bits.iter_mut().enumerate() {
        if i % 64 == 0 {
            w = g.step();
        }
        *b ^= ((w >> (i % 64)) & 1) as u8;
    }
}

/// The first `n` pilots (+1 or -1).
pub fn pilots(n: usize) -> &'static [f32] {
    static P: OnceLock<Vec<f32>> = OnceLock::new();
    let all = P.get_or_init(|| {
        // Enough for the longest frame of any width.
        let mut g = Xorshift64::new(PILOT_SEED);
        let mut w = 0;
        (0..2048)
            .map(|k| {
                if k % 64 == 0 {
                    w = g.step();
                }
                if (w >> (k % 64)) & 1 == 1 {
                    -1.0
                } else {
                    1.0
                }
            })
            .collect()
    });
    &all[..n.min(all.len())]
}

/// The 512 preamble symbols of `width`.
pub fn preamble(width: Width) -> &'static [Complex32] {
    static P: OnceLock<[Vec<Complex32>; 3]> = OnceLock::new();
    let all = P.get_or_init(|| {
        WIDTHS.map(|w| {
            let z = zadoff_chu(w.root(), ZC_LEN);
            let mut p = Vec::with_capacity(PREAMBLE);
            p.extend_from_slice(&z[ZC_LEN - 10..]);
            p.extend_from_slice(&z);
            p.extend_from_slice(&z);
            p
        })
    });
    &all[width as usize]
}

/// The 256 BPSK header symbols of `h`.
pub fn header_symbols(h: &FrameHeader) -> Vec<Complex32> {
    code_header(&h.bits()).iter().map(|&b| Complex32::new(1.0 - 2.0 * f32::from(b), 0.0)).collect()
}

/// The 7680 code bits of block `block` of frame `frame` that carries `content` (zero padded or cut to the payload size of
/// `profile`): the CRC-32 is appended little-endian, the information bits are scrambled, then LDPC encoded.
pub fn block_bits(profile: &Profile, frame: u16, block: usize, content: &[u8]) -> Vec<u8> {
    let code = ldpc::code(N, profile.rate).expect("a code of the family");
    let p = profile.payload();
    let mut bytes = content[..content.len().min(p)].to_vec();
    bytes.resize(p, 0);
    let crc = family::crc32(&bytes);
    bytes.extend_from_slice(&crc.to_le_bytes());
    let mut info = Vec::with_capacity(code.base.k);
    push_bytes(&mut info, &bytes);
    scramble(&mut info, frame, block);
    let mut word = vec![0u8; N];
    code.encode(&info, &mut word);
    word
}

/// The symbols of one frame with header `h` and the block contents `blocks`.
pub fn frame(h: &FrameHeader, blocks: &[Vec<u8>]) -> Vec<Complex32> {
    let p = h.profile();
    let set = qam::point_set(p.bits).expect("a family set");
    let mut data = Vec::with_capacity(blocks.len() * p.symbols());
    for (b, content) in blocks.iter().enumerate() {
        qam::map(&set, p.bits, &block_bits(&p, h.frame, b, content), &mut data);
    }
    let mut s = Vec::with_capacity(frame_symbols(&p, blocks.len()));
    s.extend_from_slice(preamble(h.width));
    s.extend(header_symbols(h));
    let pil = pilots(pilots_of(data.len()));
    for (g, chunk) in data.chunks(31).enumerate() {
        s.push(Complex32::new(pil[g], 0.0));
        s.extend_from_slice(chunk);
    }
    s
}

/// The transmit filter: RRC, roll-off 0.25, 4 samples a symbol, 20 symbols (81 taps), sum of squares 4.
pub fn tx_taps() -> &'static [f32] {
    static H: OnceLock<Vec<f32>> = OnceLock::new();
    H.get_or_init(|| {
        let h = taps(Shape::Rrc { beta: 0.25 }, SPS as f64, 20.0, Norm::Energy).expect("RRC taps");
        h.iter().map(|v| v * 2.0).collect()
    })
}

/// Pulse-shapes `symbols` at 4 samples a symbol: the full convolution, `4 N + 80` samples, symbol `i` at sample `4 i + 40`.
pub fn shape(symbols: &[Complex32]) -> Vec<Complex32> {
    let h = tx_taps();
    if symbols.is_empty() {
        return Vec::new();
    }
    let mut x = vec![Complex32::new(0.0, 0.0); SPS * symbols.len() + h.len() - 1];
    for (k, &v) in symbols.iter().enumerate() {
        for (j, &t) in h.iter().enumerate() {
            x[SPS * k + j] += v * t;
        }
    }
    x
}

/// The reduced ratio `(up, down)` from rate `from` to rate `to` (whole samples a second).
pub fn ratio(from: f64, to: f64) -> (u64, u64) {
    let (a, b) = (from.round() as u64, to.round() as u64);
    let g = gcd(a, b).max(1);
    (b / g, a / g)
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// `x` at 4 samples a symbol of `width`, converted to `rate` by the rational resampler of the family; unchanged when the
/// rates are equal.
pub fn to_rate(x: &[Complex32], width: Width, rate: f64) -> Vec<Complex32> {
    let (up, down) = ratio(width.rate(), rate);
    if up == down {
        return x.to_vec();
    }
    BlackmanRational::convert(x, up, down).expect("a ratio of whole rates")
}

/// One frame to send: its header and the contents of its blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TxFrame {
    /// The header.
    pub header: FrameHeader,
    /// The block contents, each at most the payload size of the profile.
    pub blocks: Vec<Vec<u8>>,
}

/// The samples of frames sent back to back as one stream, at `rate`, at the transmit level.
pub fn frames_at(frames: &[TxFrame], rate: f64) -> Vec<Complex32> {
    let Some(first) = frames.first() else {
        return Vec::new();
    };
    let width = first.header.width;
    let mut s = Vec::new();
    for f in frames {
        s.extend(frame(&f.header, &f.blocks));
    }
    let mut y = to_rate(&shape(&s), width, rate);
    for v in &mut y {
        *v *= TX_LEVEL;
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|v| format!("{v:02x}")).collect()
    }

    fn header_vector() -> FrameHeader {
        FrameHeader {
            width: Width::W500K,
            profile: 7,
            kind: FrameKind::Ack,
            session: 0xBEEF,
            frame: 0x1234,
            blocks: 5,
            index: 9,
            last: true,
            callsign: "LY1A/P".into(),
        }
    }

    #[test]
    fn profile_and_frame_tables() {
        let k = [1920, 1920, 3840, 5760, 5760, 5760, 5760, 6400, 6400, 6912];
        let p = [236, 236, 476, 716, 716, 716, 716, 796, 796, 860];
        let s = [7680, 3840, 3840, 3840, 1920, 1280, 960, 960, 768, 768];
        let nmax = [(3, 1), (7, 3), (7, 3), (7, 3), (15, 7), (23, 11), (31, 15), (31, 15), (39, 19), (39, 19)];
        let full = [24_552, 28_516, 28_516, 28_516, 30_498, 31_158, 31_488, 31_488, 31_687, 31_687];
        for (i, pr) in PROFILES.iter().enumerate() {
            assert_eq!((pr.k(), pr.payload(), pr.data(), pr.symbols()), (k[i], p[i], p[i] - 6, s[i]), "{i}");
            assert_eq!((pr.payload() + 4) * 8, pr.k());
            assert!(pr.payload() >= 7 && pr.payload() * 8 + 32 <= pr.k());
            assert_eq!(max_blocks(pr, Width::W1M), nmax[i].0, "{i}");
            assert_eq!(max_blocks(pr, Width::W500K), nmax[i].0, "{i}");
            assert_eq!(max_blocks(pr, Width::W200K), nmax[i].1, "{i}");
            assert_eq!(frame_symbols(pr, nmax[i].0), full[i], "{i}");
        }
        assert_eq!(frame_symbols(&PROFILES[0], 1), 8696);
        assert!((8696.0 / Width::W200K.symbol_rate() * 1e3 - 54.35).abs() < 0.01);
    }

    #[test]
    fn header_vector_and_round_trip() {
        let h = header_vector();
        let b = h.bits();
        assert_eq!(b.len(), HEADER_INFO);
        assert_eq!(hex(&family::pack(&b)), "5ebeef1234059ad1845cad42d2000000");
        let mut c = Vec::new();
        push_bits(&mut c, callsign_code("LY1A/P"), 36);
        assert_eq!(hex(&family::pack(&c)), "5a308b95a0");
        assert_eq!(hex(&family::pack(&code_header(&b))), "38a1b84b403355028cb6773897314aa42357ffaf4a3a9cb926a981c000000000");
        let lower = FrameHeader { callsign: "ly1a/p".into(), ..h.clone() };
        assert_eq!(parse_header(&lower.bits()), Some(h.clone()));
        let mut bad = b.clone();
        bad[10] ^= 1;
        assert_eq!(parse_header(&bad), None);
        assert_eq!(callsign_of(callsign_code("A_B")), "A-B");
        assert_eq!(callsign_code("0"), 1 << 30);
        assert_eq!(callsign_of(callsign_code("ABCDEFGH")), "ABCDEF");
    }

    #[test]
    fn scrambler_and_pilot_vectors() {
        assert_eq!(scrambler_seed(0, 0), 0xb00b_5f9a_35b3_010f);
        assert_eq!(scrambler_seed(0x12, 3), 0x73d5_34f8_edab_ba0f);
        let p: String = pilots(64).iter().map(|&v| if v < 0.0 { '1' } else { '0' }).collect();
        assert_eq!(p, "0011110000100110001101111000010100011101000001010111000110001100");
    }

    #[test]
    fn preamble_vector() {
        let p = preamble(Width::W1M);
        let near = |z: Complex32, re: f32, im: f32| (z.re - re).abs() < 1e-5 && (z.im - im).abs() < 1e-5;
        assert!(near(p[0], -0.485_478, -0.874_249), "{:?}", p[0]);
        assert!(near(p[1], 0.659_150, 0.752_012), "{:?}", p[1]);
        assert!(near(p[10], 1.0, 0.0));
        assert!(near(p[11], 0.947_515, -0.319_710), "{:?}", p[11]);
        assert_eq!(p[10..261], p[261..512]);
    }

    #[test]
    fn block_vectors() {
        let mut content = vec![0x00, 0x10, 0x00, 0x00, 0x64, 0x00];
        content.extend(0..100u8);
        let pr = PROFILES[2];
        let mut padded = content.clone();
        padded.resize(pr.payload(), 0);
        assert_eq!(family::crc32(&padded), 0xbfe9_e8b2);
        let w = family::pack(&block_bits(&pr, 0x0010, 1, &content));
        assert_eq!(hex(&w[..32]), "8520ae356dfc0f3a1d745432fbb50c68aea9336f426fbc1ce5394c2809d6541b");
        assert_eq!(hex(&w[w.len() - 16..]), "e72cc1c700502d451662d313cf619d8a");
        let w = family::pack(&block_bits(&PROFILES[0], 0, 0, &[]));
        assert_eq!(hex(&w[..32]), "4a8e652a307060de0c949b9fc73a893546eb41c348c7bcaf32522313353da782");
    }

    #[test]
    fn transmit_filter_and_frame_layout() {
        let h = tx_taps();
        assert_eq!(h.len(), 81);
        assert!((h[40] - 1.068_329).abs() < 1e-5 && (h[41] - 0.943_182).abs() < 1e-5, "{} {}", h[40], h[41]);
        let e: f32 = h.iter().map(|v| v * v).sum();
        assert!((e - 4.0).abs() < 1e-4);
        let hd = FrameHeader { width: Width::W200K, profile: 0, kind: FrameKind::Data, blocks: 1, ..header_vector() };
        let s = frame(&hd, &[vec![1, 2, 3]]);
        assert_eq!(s.len(), 8696);
        assert_eq!(s[FIXED], Complex32::new(1.0, 0.0));
        assert_eq!(s[FIXED + 32].re, pilots(2)[1]);
        let x = shape(&s);
        assert_eq!(x.len(), 4 * s.len() + 80);
        let p = x.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / x.len() as f64;
        assert!((p - 1.0).abs() < 0.03, "{p}");
        assert_eq!(ratio(640_000.0, 1_250_000.0), (125, 64));
        assert_eq!(ratio(3_200_000.0, 3_200_000.0), (1, 1));
    }
}
