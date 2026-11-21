//! `arq_packet`: burst packets at 80 ksymbols a second, 100 kHz wide. A burst is a ramp, a preamble of two Zadoff-Chu
//! sequences, a header of 80 bits (rate 1/2 convolutional code, BPSK, sent 4 times), the scrambled LDPC blocks (n = 2304) of
//! one of nine profiles from BPSK rate 1/4 to 1024-QAM rate 5/6 with a pilot after every 31 data symbols, and a ramp. A
//! selective-repeat layer ([`arq`]) moves one message over bursts, and an acknowledgement burst says which blocks arrived.

pub mod arq;
pub mod rx;
pub mod stack;

pub use stack::ARQ_PACKET;

use crate::family::{self, ldpc, push_bits, push_bytes, qam, read_bits};
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_dsp::sinc_table::SincConverter;
use sdrkit_algo_fec::conv::codes::K7_171_133;
use sdrkit_algo_fec::conv::Encoder;
use sdrkit_algo_modem::zadoff_chu::zadoff_chu;
use sdrkit_core::Complex32;
use sdrkit_util_bits::xorshift::Xorshift64;
use std::sync::OnceLock;

/// Symbols a second.
pub const SYMBOL_RATE: f64 = 80_000.0;
/// The rate at which a burst is made, samples a second.
pub const FS_TX: f64 = 640_000.0;
/// Samples a symbol at [`FS_TX`].
pub const SPS_TX: usize = 8;
/// Symbols of each ramp.
pub const RAMP: usize = 8;
/// Length of each Zadoff-Chu half of the preamble.
pub const ZC_LEN: usize = 127;
/// The roots of the two preamble halves.
pub const ROOTS: [i64; 2] = [41, 86];
/// Preamble symbols.
pub const PREAMBLE: usize = 2 * ZC_LEN;
/// Header bits with the CRC.
pub const HEADER_BITS: usize = 80;
/// Coded header bits of one copy.
pub const HEADER_CODED: usize = 172;
/// Header symbols (4 copies).
pub const HEADER_SYMBOLS: usize = 4 * HEADER_CODED;
/// Known symbols at the start of a burst after the ramp: preamble and header.
pub const KNOWN: usize = PREAMBLE + HEADER_SYMBOLS;
/// Symbols of a burst besides its payload.
pub const OVERHEAD: usize = 2 * RAMP + KNOWN;
/// The LDPC code length.
pub const N: usize = 2304;
/// The most blocks a header can carry.
pub const MAX_BLOCKS: usize = 1023;
/// The scrambler key.
pub const SCRAMBLE_KEY: u64 = 0x4B41_5050_5F53_4352;
/// The level of a transmitted burst.
pub const TX_LEVEL: f32 = 0.35;

/// Header flag: an acknowledgement burst.
pub const FLAG_ACK: u8 = 8;
/// Header flag: the burst holds the block with the last byte of the message.
pub const FLAG_FINAL: u8 = 4;
/// Header flag: the burst holds a block sent before.
pub const FLAG_RETRANSMIT: u8 = 2;
/// Header flag: the sender waits for an acknowledgement.
pub const FLAG_ACK_REQUEST: u8 = 1;

/// One profile: a modulation and an LDPC rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    /// Bits a symbol.
    pub bits: usize,
    /// The LDPC rate index of the family.
    pub rate: usize,
    /// The Es/N0 of a block error rate near 1e-2, dB.
    pub required_db: f64,
}

/// The nine profiles by id.
pub const PROFILES: [Profile; 9] = [
    Profile { bits: 1, rate: 0, required_db: -1.0 },
    Profile { bits: 2, rate: 0, required_db: 2.0 },
    Profile { bits: 2, rate: 2, required_db: 1.5 },
    Profile { bits: 2, rate: 4, required_db: 4.1 },
    Profile { bits: 4, rate: 2, required_db: 6.3 },
    Profile { bits: 4, rate: 4, required_db: 10.0 },
    Profile { bits: 6, rate: 3, required_db: 13.3 },
    Profile { bits: 8, rate: 4, required_db: 19.5 },
    Profile { bits: 10, rate: 5, required_db: 26.6 },
];

impl Profile {
    /// The profile of id `id`; `None` above 8.
    pub fn of(id: usize) -> Option<Profile> {
        PROFILES.get(id).copied()
    }

    /// Information bits of a block: the payload and its CRC-16.
    pub fn k(&self) -> usize {
        ldpc::base(N, self.rate).map_or(0, |b| b.k)
    }

    /// Payload bytes of a block.
    pub fn payload(&self) -> usize {
        (self.k() - 16) / 8
    }

    /// Data symbols of a block.
    pub fn symbols(&self) -> usize {
        N.div_ceil(self.bits)
    }

    /// Message bytes of an ARQ block: `min(payload - 4, 255)`.
    pub fn data(&self) -> usize {
        (self.payload() - 4).min(255)
    }
}

/// Payload symbols of `d` data symbols: one pilot after every 31.
pub fn payload_symbols(d: usize) -> usize {
    d + d / 31
}

/// Symbols of a burst of `blocks` blocks at `profile`.
pub fn burst_symbols(profile: &Profile, blocks: usize) -> usize {
    OVERHEAD + payload_symbols(blocks * profile.symbols())
}

/// The most blocks of `profile` in `ms` milliseconds of air time, by the receiver's rule (31/32 of the symbols after the fixed
/// part).
pub fn max_blocks(profile: &Profile, ms: f64) -> usize {
    let s = (ms * 1e-3 * SYMBOL_RATE).floor();
    if s <= OVERHEAD as f64 {
        return 0;
    }
    ((s - OVERHEAD as f64) * 31.0 / 32.0 / profile.symbols() as f64).floor() as usize
}

const ALPHABET: &[u8; 37] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789/";

/// Whether `call` can be sent as it is: 1 to 7 characters of A-Z, 0-9 and /, any case.
pub fn callsign_ok(call: &str) -> bool {
    (1..=7).contains(&call.len()) && call.bytes().all(|c| ALPHABET.contains(&c.to_ascii_uppercase()))
}

/// The 40-bit code of a callsign: 7 base-37 digits, the first character most significant, padded with "/"; a character
/// outside the alphabet counts as "/".
pub fn callsign_code(call: &str) -> u64 {
    let b = call.as_bytes();
    (0..7).fold(0u64, |v, i| {
        let c = b.get(i).map_or(b'/', |c| c.to_ascii_uppercase());
        let d = ALPHABET.iter().position(|&a| a == c).unwrap_or(36) as u64;
        v * 37 + d
    })
}

/// The callsign of a code without the "/" at its end; `None` for a code of 37^7 or more.
pub fn callsign_of(code: u64) -> Option<String> {
    if code >= 37u64.pow(7) {
        return None;
    }
    let mut v = code;
    let mut s = [0u8; 7];
    for i in (0..7).rev() {
        s[i] = ALPHABET[(v % 37) as usize];
        v /= 37;
    }
    Some(String::from_utf8_lossy(&s).trim_end_matches('/').to_string())
}

/// The header of a burst.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// The sending station.
    pub callsign: String,
    /// The profile id, 0 to 8.
    pub profile: u8,
    /// Blocks, 1 to 1023.
    pub blocks: u16,
    /// The sequence number, 0 to 63.
    pub seq: u8,
    /// The flags ([`FLAG_ACK`] and the others).
    pub flags: u8,
}

impl Header {
    /// The 80 header bits: the 64 field bits and their CRC-16.
    pub fn bits(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(HEADER_BITS);
        push_bits(&mut b, callsign_code(&self.callsign), 40);
        push_bits(&mut b, u64::from(self.profile & 15), 4);
        push_bits(&mut b, u64::from(self.blocks & 1023), 10);
        push_bits(&mut b, u64::from(self.seq & 63), 6);
        push_bits(&mut b, u64::from(self.flags & 15), 4);
        let crc = family::crc16(&family::pack(&b));
        push_bits(&mut b, u64::from(crc), 16);
        b
    }

    /// Whether this is an acknowledgement burst.
    pub fn is_ack(&self) -> bool {
        self.flags & FLAG_ACK != 0
    }

    /// The profile of the header.
    pub fn profile(&self) -> Profile {
        PROFILES[usize::from(self.profile).min(8)]
    }

    /// Whether a receiver takes the header: a profile of 0 to 8, at least one block, an acknowledgement at profile 0 with one
    /// block, and a data burst of at most 1 s of air time.
    pub fn plausible(&self) -> bool {
        let Some(p) = Profile::of(usize::from(self.profile)) else {
            return false;
        };
        if self.blocks == 0 {
            return false;
        }
        if self.is_ack() {
            return self.profile == 0 && self.blocks == 1;
        }
        usize::from(self.blocks) <= max_blocks(&p, 1000.0).max(1)
    }
}

/// Reads 80 header bits: `None` when the CRC fails or the callsign code is invalid. [`Header::plausible`] holds the other
/// checks of a receiver.
pub fn parse_header(bits: &[u8]) -> Option<Header> {
    if bits.len() < HEADER_BITS || family::crc16(&family::pack(&bits[..64])) != read_bits(bits, 64, 16) as u16 {
        return None;
    }
    Some(Header {
        callsign: callsign_of(read_bits(bits, 0, 40))?,
        profile: read_bits(bits, 40, 4) as u8,
        blocks: read_bits(bits, 44, 10) as u16,
        seq: read_bits(bits, 54, 6) as u8,
        flags: read_bits(bits, 60, 4) as u8,
    })
}

/// The 172 coded bits of a header: rate 1/2, K = 7, terminated.
pub fn code_header(bits: &[u8]) -> Vec<u8> {
    let mut enc = Encoder::new(K7_171_133);
    let mut out = Vec::with_capacity(HEADER_CODED);
    enc.encode(bits, &mut out);
    enc.terminate(&mut out);
    out
}

/// The 2304 scrambler bits of block `b`: bit `i` is bit `i mod 64` (from the least significant) of the `i / 64`-th step.
pub fn scrambler(b: usize) -> Vec<u8> {
    let mut g = Xorshift64::new(Xorshift64::GOLDEN.wrapping_mul(b as u64 + 1) ^ SCRAMBLE_KEY);
    let mut out = Vec::with_capacity(N);
    for _ in 0..N / 64 {
        let w = g.step();
        out.extend((0..64).map(|i| ((w >> i) & 1) as u8));
    }
    out
}

/// The preamble: root 41, then root 86, length 127 each.
pub fn preamble() -> &'static [Complex32] {
    static P: OnceLock<Vec<Complex32>> = OnceLock::new();
    P.get_or_init(|| ROOTS.iter().flat_map(|&u| zadoff_chu(u, ZC_LEN)).collect())
}

/// The 688 header symbols of `h`.
pub fn header_symbols(h: &Header) -> Vec<Complex32> {
    let c = code_header(&h.bits());
    (0..HEADER_SYMBOLS).map(|i| Complex32::new(1.0 - 2.0 * f32::from(c[i % HEADER_CODED]), 0.0)).collect()
}

/// The 2304 (2310 at 1024-QAM) scrambled code bits of block `b` that carries `payload` (the payload bytes of the profile).
pub fn block_bits(profile: &Profile, b: usize, payload: &[u8]) -> Vec<u8> {
    let code = ldpc::code(N, profile.rate).expect("a code of the family");
    let mut info = Vec::with_capacity(code.base.k);
    push_bytes(&mut info, payload);
    push_bits(&mut info, u64::from(family::crc16(payload)), 16);
    let mut word = vec![0u8; N];
    code.encode(&info, &mut word);
    for (w, s) in word.iter_mut().zip(scrambler(b)) {
        *w ^= s;
    }
    word.resize(profile.symbols() * profile.bits, 0);
    word
}

/// The symbols of a burst with header `h` and the blocks `payloads` (each cut or padded with zeros to the payload size).
pub fn burst(h: &Header, payloads: &[Vec<u8>]) -> Vec<Complex32> {
    let p = h.profile();
    let set = qam::point_set(p.bits).expect("a family set");
    let mut data = Vec::with_capacity(payloads.len() * p.symbols());
    for (b, pl) in payloads.iter().enumerate() {
        let mut bytes = pl.clone();
        bytes.resize(p.payload(), 0);
        qam::map(&set, p.bits, &block_bits(&p, b, &bytes), &mut data);
    }
    let mut s = Vec::with_capacity(burst_symbols(&p, payloads.len()));
    let pre = preamble();
    for k in 0..RAMP {
        let w = ((std::f32::consts::FRAC_PI_2 * (k as f32 + 0.5) / RAMP as f32).sin()).powi(2);
        s.push(pre[0] * w);
    }
    s.extend_from_slice(pre);
    s.extend(header_symbols(h));
    for (i, &d) in data.iter().enumerate() {
        s.push(d);
        if i % 31 == 30 {
            s.push(Complex32::new(1.0, 0.0));
        }
    }
    for k in 0..RAMP {
        let w = ((std::f32::consts::FRAC_PI_2 * (k as f32 + 0.5) / RAMP as f32).cos()).powi(2);
        s.push(Complex32::new(w, 0.0));
    }
    s
}

/// The RRC of the transmitter: roll-off 0.25, 8 samples a symbol, 12 symbols (97 taps), sum of squares 8.
pub fn tx_taps() -> &'static [f32] {
    static H: OnceLock<Vec<f32>> = OnceLock::new();
    H.get_or_init(|| {
        let h = taps(Shape::Rrc { beta: 0.25 }, SPS_TX as f64, 12.0, Norm::Energy).expect("RRC taps");
        h.iter().map(|v| v * 8f32.sqrt()).collect()
    })
}

/// Pulse-shapes `symbols` at 640 ksps: symbol `k` peaks at sample `8 k + 48`.
pub fn shape(symbols: &[Complex32]) -> Vec<Complex32> {
    let h = tx_taps();
    let mut x = vec![Complex32::new(0.0, 0.0); (symbols.len().max(1) - 1) * SPS_TX + h.len()];
    for (k, &v) in symbols.iter().enumerate() {
        for (j, &t) in h.iter().enumerate() {
            x[SPS_TX * k + j] += v * t;
        }
    }
    x
}

/// The samples of one burst at `rate`, at the transmit level: made at 640 ksps and converted when the rate differs by more
/// than 0.5 Hz.
pub fn burst_at(h: &Header, payloads: &[Vec<u8>], rate: f64) -> Vec<Complex32> {
    let x = shape(&burst(h, payloads));
    let mut y = if (rate - FS_TX).abs() > 0.5 {
        SincConverter::convert(&x, FS_TX, rate).unwrap_or_default()
    } else {
        x
    };
    for v in &mut y {
        *v *= TX_LEVEL;
    }
    y
}

/// The 70 payload bytes of an acknowledgement: the SNR code (`round(Es/N0) + 10`, 0 to 63) in 6 bits, then a bitmap of 512
/// blocks, then zeros.
pub fn ack_payload(esn0_db: f64, bitmap: &[bool]) -> Vec<u8> {
    let code = (esn0_db.round() + 10.0).clamp(0.0, 63.0) as u64;
    let mut b = Vec::with_capacity(560);
    push_bits(&mut b, code, 6);
    b.extend((0..512).map(|i| u8::from(bitmap.get(i).copied().unwrap_or(false))));
    b.resize(560, 0);
    family::pack(&b)
}

/// The SNR (dB) and the 512-entry bitmap of acknowledgement payload bytes.
pub fn parse_ack(payload: &[u8]) -> (i32, Vec<bool>) {
    let mut bits = Vec::with_capacity(payload.len() * 8);
    push_bytes(&mut bits, payload);
    bits.resize(560, 0);
    let snr = read_bits(&bits, 0, 6) as i32 - 10;
    (snr, bits[6..518].iter().map(|&b| b == 1).collect())
}

/// The header of an acknowledgement of burst `seq` from station `call`.
pub fn ack_header(call: &str, seq: u8) -> Header {
    Header {
        callsign: call.to_string(),
        profile: 0,
        blocks: 1,
        seq,
        flags: FLAG_ACK,
    }
}

/// A callsign the transmitter may send: the setting, or the demonstration callsign when it is empty; `None` when it has more
/// than 7 characters or another character than A-Z, 0-9 and /.
pub fn tx_callsign(setting: &str) -> Option<String> {
    let s = setting.trim();
    if s.is_empty() {
        return Some(family::DEMO_CALLSIGN.to_string());
    }
    callsign_ok(s).then(|| s.to_ascii_uppercase())
}

/// One way: every burst of `passes` passes of `message` at `profile` from `call`, at `rate`, with 30 ms of silence before and
/// after each burst and `max(1, 0.4 rate)` samples after each pass. Sequence numbers start at `seq` + 1. Returns the samples
/// and the next sequence number.
pub fn one_way(message: &[u8], call: &str, profile: usize, passes: usize, rate: f64, seq: u8) -> (Vec<Complex32>, u8) {
    let p = PROFILES[profile.min(8)];
    let per_burst = max_blocks(&p, 1000.0).clamp(1, MAX_BLOCKS);
    let c = p.data();
    let gap = (0.03 * rate).floor() as usize;
    let mut seq = seq;
    let mut out = Vec::new();
    for _ in 0..passes.max(1) {
        let blocks: Vec<arq::Block> = arq::cut(message, 0, message.len(), c);
        for chunk in blocks.chunks(per_burst) {
            seq = (seq + 1) % 64;
            let last = chunk.iter().any(|b| b.last);
            let h = Header {
                callsign: call.to_string(),
                profile: profile as u8,
                blocks: chunk.len() as u16,
                seq,
                flags: if last { FLAG_FINAL } else { 0 },
            };
            let payloads: Vec<Vec<u8>> = chunk.iter().map(|b| b.encode(p.payload())).collect();
            out.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), gap));
            out.extend(burst_at(&h, &payloads, rate));
            out.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), gap));
        }
        out.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), ((0.4 * rate) as usize).max(1)));
    }
    (out, seq)
}

/// The air time of [`one_way`] by the formula of the description: per pass the burst symbols over the symbol rate, 60 ms a
/// burst and 0.4 s.
pub fn one_way_seconds(len: usize, profile: usize, passes: usize) -> f64 {
    let p = PROFILES[profile.min(8)];
    let per_burst = max_blocks(&p, 1000.0).clamp(1, MAX_BLOCKS);
    let blocks = len.div_ceil(p.data()).max(1);
    let bursts = blocks.div_ceil(per_burst);
    let symbols: usize = (0..bursts).map(|i| burst_symbols(&p, (blocks - i * per_burst).min(per_burst))).sum();
    (symbols as f64 / SYMBOL_RATE + bursts as f64 * 0.06 + 0.4) * passes as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|v| format!("{v:02X}")).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn profile_table() {
        let ks = [576, 576, 1152, 1728, 1152, 1728, 1536, 1728, 1920];
        let pls = [70, 70, 142, 214, 142, 214, 190, 214, 238];
        let sy = [2304, 1152, 1152, 1152, 576, 576, 384, 288, 231];
        for (i, p) in PROFILES.iter().enumerate() {
            assert_eq!((p.k(), p.payload(), p.symbols()), (ks[i], pls[i], sy[i]), "{i}");
        }
        assert_eq!(max_blocks(&PROFILES[0], 1000.0), 33);
        assert_eq!(max_blocks(&PROFILES[2], 100.0), 5);
        assert_eq!(max_blocks(&PROFILES[6], 1000.0), 199);
        assert_eq!(payload_symbols(2304) - 2304, 74);
    }

    #[test]
    fn callsign_codes() {
        assert_eq!(callsign_code("LY1ABC"), 29_937_849_293);
        assert_eq!(callsign_code("N0CALL"), 35_161_150_023);
        assert_eq!(callsign_of(callsign_code("ly1abc")).as_deref(), Some("LY1ABC"));
        assert_eq!(callsign_of(37u64.pow(7)), None);
        assert!(callsign_ok("DL1XYZ") && !callsign_ok("DL1XYZAB") && !callsign_ok("A_B") && !callsign_ok(""));
    }

    #[test]
    fn header_vectors() {
        let h = Header { callsign: "LY1ABC".into(), profile: 2, blocks: 3, seq: 17, flags: 1 };
        let b = h.bits();
        assert_eq!(hex(&family::pack(&b)), "06 F8 6F 53 CD 20 0D 11 14 93");
        assert_eq!(hex(&family::pack(&code_header(&b))), "00 35 EF B9 99 EF 6E C6 E9 BC 69 81 C0 D7 64 CF CF F7 5B AD 3D B0");
        let a = ack_header("LY2XYZ", 33);
        assert_eq!(hex(&family::pack(&a.bits())), "06 F8 9E 31 EB 00 06 18 46 12");
        // Round trip, and one flipped bit fails the CRC.
        let h = Header { callsign: "DL1XYZ".into(), profile: 7, blocks: 321, seq: 45, flags: 5 };
        let mut b = h.bits();
        assert_eq!(parse_header(&b), Some(h.clone()));
        assert!(!h.plausible(), "321 blocks is more than 1 s at profile 7");
        b[10] ^= 1;
        assert_eq!(parse_header(&b), None);
    }

    #[test]
    fn scrambler_vectors() {
        let first = |b| hex(&family::pack(&scrambler(b)[..64]));
        assert_eq!(first(0), "9F 3C 30 55 B0 3C 11 E4");
        assert_eq!(first(1), "70 57 98 6D FF 25 A5 C2");
        assert_eq!(first(2), "C2 2A F5 EA B0 6B D8 64");
        assert_eq!(first(1022), "0B EE F1 49 6E 06 5A 61");
    }

    #[test]
    fn ack_payload_vector() {
        let bm = [false, true, true, false, true, true, false, true, true, false];
        let p = ack_payload(12.0, &bm);
        assert_eq!(p.len(), 70);
        assert_eq!(&p[..2], &[0x59, 0xB6]);
        assert!(p[2..].iter().all(|&v| v == 0));
        assert_eq!(ack_payload(80.0, &[])[0] >> 2, 63);
        assert_eq!(ack_payload(-40.0, &[])[0] >> 2, 0);
        for snr in [-10.0, -3.0, 0.0, 17.0, 53.0] {
            for n in [1usize, 7, 64, 512] {
                let bm: Vec<bool> = (0..n).map(|i| (i * 7 + 3) % 5 < 2).collect();
                let (s, back) = parse_ack(&ack_payload(snr, &bm));
                assert!((f64::from(s) - snr).abs() < 0.01);
                assert_eq!(&back[..n], &bm[..]);
                assert!(back[n..].iter().all(|v| !v));
            }
        }
    }

    #[test]
    fn block_vectors() {
        let crc16 = [0x7DFB, 0x7DFB, 0xAA94, 0x6B28, 0xAA94, 0x6B28, 0xEF5C, 0x6B28, 0x8FD8];
        let cw = [0x56E1B00F, 0x56E1B00F, 0xCAC31BBB, 0x99A6FF21, 0xCAC31BBB, 0x99A6FF21, 0x170458A3, 0x99A6FF21, 0xD0C270D1];
        let sc = [0x0C572661, 0x0C572661, 0x90758DD5, 0xC310694F, 0x90758DD5, 0xC310694F, 0x4DB2CECD, 0xC310694F, 0x8A74E6BF];
        let one = [3336, 2147, 2147, 2147, 1552, 1552, 1354, 1255, 1196];
        let three = [8092, 4525, 4525, 4525, 2741, 2741, 2147, 1849, 1673];
        let ms100 = [2, 5, 5, 5, 11, 11, 17, 23, 29];
        let s1 = [33, 66, 66, 66, 132, 132, 199, 265, 331];
        for (i, p) in PROFILES.iter().enumerate() {
            let pl: Vec<u8> = (0..p.payload()).map(|v| v as u8).collect();
            assert_eq!(family::crc16(&pl), crc16[i], "{i}");
            let bits = block_bits(p, 0, &pl);
            let scr = scrambler(0);
            let word: Vec<u8> = bits[..N].iter().zip(&scr).map(|(a, b)| a ^ b).collect();
            assert_eq!(family::crc32(&family::pack(&word)), cw[i], "{i}");
            assert_eq!(family::crc32(&family::pack(&bits[..N])), sc[i], "{i}");
            assert_eq!(burst_symbols(p, 1), one[i]);
            assert_eq!(burst_symbols(p, 3), three[i]);
            assert_eq!(max_blocks(p, 100.0), ms100[i]);
            assert_eq!(max_blocks(p, 1000.0), s1[i]);
        }
    }

    #[test]
    fn example_burst() {
        let h = Header { callsign: "LY1ABC".into(), profile: 2, blocks: 3, seq: 17, flags: 1 };
        let pl = vec![0x55u8; 300];
        let payloads: Vec<Vec<u8>> = pl.chunks(142).map(<[u8]>::to_vec).collect();
        let s = burst(&h, &payloads);
        assert_eq!(s.len(), 4525);
        assert_eq!(s[981], Complex32::new(1.0, 0.0));
        let pre = preamble();
        assert!((pre[1].re + 0.441_826).abs() < 1e-5 && (pre[1].im + 0.897_101).abs() < 1e-5);
        assert_eq!(pre[127], Complex32::new(1.0, 0.0));
        assert!((tx_taps()[48] - 1.068_407).abs() < 1e-5);
        let x = shape(&s);
        assert_eq!(x.len(), 36_289);
        let p = x.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / x.len() as f64;
        assert!((p - 0.995).abs() < 0.01, "{p}");
        // The converter's sample counts of the description.
        for (rate, before, with) in [(320e3, 18_128, 18_163), (250e3, 14_159, 14_194), (1e6, 56_676, 56_732)] {
            let mut c = SincConverter::new(FS_TX, rate).unwrap();
            let mut y = Vec::new();
            c.process(&x, &mut y);
            assert_eq!(y.len(), before, "{rate}");
            c.flush(&mut y);
            assert_eq!(y.len(), with, "{rate}");
        }
    }
}
