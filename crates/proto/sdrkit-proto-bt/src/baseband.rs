//! The basic-rate baseband: the 79 channels, the sync word of a LAP, the access code and its search, the packet header with
//! its HEC, FEC 1/3, FEC 2/3, whitening and the payload CRC.
//!
//! Bits are `u8` (0 or 1) in air order. Standard: Bluetooth Core Specification v5.4, Vol 2 Part B clauses 6.3 (access code),
//! 6.4 (header), 7.1 (HEC and CRC), 7.2 (whitening), 7.4 and 7.5 (FEC).

use sdrkit_util_bits::crc::catalog::CRC16_KERMIT;

/// The number of basic-rate channels.
pub const CHANNELS: u32 = 79;
/// The centre of channel 0, Hz.
pub const CHANNEL0_HZ: f64 = 2402e6;
/// The channel spacing, Hz.
pub const CHANNEL_SPACING_HZ: f64 = 1e6;
/// The symbol rate of the basic rate, symbols per second.
pub const SYMBOL_RATE: f64 = 1e6;
/// The LAP of the general inquiry access code.
pub const GIAC: u32 = 0x9E_8B33;
/// The LAP of the limited inquiry access code.
pub const LIAC: u32 = 0x9E_8B00;
/// The bits of an access code with its trailer.
pub const ACCESS_BITS: usize = 72;
/// The bits of a packet header on the air (18 bits sent three times).
pub const HEADER_AIR_BITS: usize = 54;
/// The packet type of an FHS packet.
pub const TYPE_FHS: u8 = 2;

/// The centre of channel `k`, Hz.
pub fn channel_hz(k: u32) -> f64 {
    CHANNEL0_HZ + f64::from(k) * CHANNEL_SPACING_HZ
}

/// The pseudo-random sequence of the sync word: bit i is p_i.
const PN: u64 = 0x8384_8D96_BBCC_54FC;
/// The generator of the (64,30) code of the sync word, highest power first: degree 34.
const SYNC_GEN: u64 = 0o260_534_236_651;

/// The value of s57..s63 (bit k = s_(57+k)) when a23 = 1.
pub const BARKER_ONE: u8 = 0x27;
/// The value of s57..s63 when a23 = 0.
pub const BARKER_ZERO: u8 = 0x58;

/// The 30 information bits of the sync word: the LAP and its Barker extension, bit i = a_i.
fn lap_bits(lap: u32) -> u64 {
    let lap = u64::from(lap & 0xFF_FFFF);
    let ext: u64 = if lap >> 23 & 1 == 0 {
        0b10_1100
    } else {
        0b01_0011
    };
    lap | ext << 24
}

/// The sync word of `lap` (the low 24 bits count): bit i is s_i, the bit sent i-th.
///
/// ```
/// use sdrkit_proto_bt::baseband::{sync_word, GIAC};
/// assert_eq!(sync_word(GIAC), 0x4E7A_2CCE_331A_3AE2);
/// assert_eq!(sync_word(GIAC).reverse_bits(), 0x475C_58CC_7334_5E72);
/// ```
pub fn sync_word(lap: u32) -> u64 {
    let x = lap_bits(lap) ^ (PN >> 34) & ((1 << 30) - 1);
    let mut r = x << 34;
    for d in (34..64).rev() {
        if r >> d & 1 == 1 {
            r ^= SYNC_GEN << (d - 34);
        }
    }
    (r & ((1 << 34) - 1) | x << 34) ^ PN
}

/// The LAP that a received sync word carries in s34..s57 (sent plain, since the PN is applied twice to those bits).
pub fn lap_of(sync: u64) -> u32 {
    (sync >> 34 & 0xFF_FFFF) as u32
}

/// The access code of `lap` in air order: preamble, sync word and, with `trailer`, the trailer (72 bits, else 68).
pub fn access_code(lap: u32, trailer: bool, out: &mut Vec<u8>) {
    let s = sync_word(lap);
    let f = (s & 1) as u8;
    out.extend([f, f ^ 1, f, f ^ 1]);
    out.extend((0..64).map(|i| (s >> i & 1) as u8));
    if trailer {
        let l = (s >> 63) as u8 ^ 1;
        out.extend([l, l ^ 1, l, l ^ 1]);
    }
}

/// An access code found in a bit string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessHit {
    /// The index of the first sync-word bit.
    pub at: usize,
    /// The LAP.
    pub lap: u32,
    /// Bits of the 64 that differ from the sync word of the LAP.
    pub errors: u32,
    /// The four bits after the sync word are a trailer.
    pub trailer: bool,
}

/// Finds the access codes in `bits`: at each position the four preamble bits alternate, s57..s63 lie within one bit of
/// [`BARKER_ONE`] or [`BARKER_ZERO`], and the 64 bits differ from the sync word of the LAP they carry in at most `max_errors`
/// places. The search goes on after the sync word of each hit.
pub fn find_access_codes(bits: &[u8], max_errors: u32, mut found: impl FnMut(AccessHit)) {
    if bits.len() < 68 {
        return;
    }
    let mut p = 4;
    while p + 64 <= bits.len() {
        let pre = &bits[p - 4..p];
        if pre[0] == pre[1] || pre[1] == pre[2] || pre[2] == pre[3] {
            p += 1;
            continue;
        }
        let mut barker = 0u8;
        for k in 0..7 {
            barker |= (bits[p + 57 + k] & 1) << k;
        }
        if (barker ^ BARKER_ONE).count_ones() > 1 && (barker ^ BARKER_ZERO).count_ones() > 1 {
            p += 1;
            continue;
        }
        let mut s = 0u64;
        for (i, &b) in bits[p..p + 64].iter().enumerate() {
            s |= u64::from(b & 1) << i;
        }
        let lap = lap_of(s);
        let errors = (s ^ sync_word(lap)).count_ones();
        if errors > max_errors {
            p += 1;
            continue;
        }
        let trailer = bits.get(p + 64..p + 68).is_some_and(|t| {
            let l = (s >> 63) as u8 ^ 1;
            t == [l, l ^ 1, l, l ^ 1]
        });
        found(AccessHit {
            at: p,
            lap,
            errors,
            trailer,
        });
        p += 64;
    }
}

/// The 10 header bits before the HEC, in air order: LT_ADDR (3), TYPE (4), FLOW, ARQN, SEQN, each least significant bit first.
pub fn header_bits(lt_addr: u8, kind: u8, flow: bool, arqn: bool, seqn: bool) -> [u8; 10] {
    let mut h = [0u8; 10];
    for (k, b) in h[..3].iter_mut().enumerate() {
        *b = lt_addr >> k & 1;
    }
    for (k, b) in h[3..7].iter_mut().enumerate() {
        *b = kind >> k & 1;
    }
    h[7] = u8::from(flow);
    h[8] = u8::from(arqn);
    h[9] = u8::from(seqn);
    h
}

/// The fields of the 10 header bits `h`: LT_ADDR, TYPE, FLOW, ARQN, SEQN.
pub fn header_fields(h: &[u8]) -> (u8, u8, bool, bool, bool) {
    let v = |r: std::ops::Range<usize>| {
        h[r.clone()]
            .iter()
            .enumerate()
            .fold(0u8, |a, (k, &b)| a | (b & 1) << k)
    };
    (v(0..3), v(3..7), h[7] == 1, h[8] == 1, h[9] == 1)
}

/// The HEC of the 10 header bits `h` (air order) with the register preset from `uap`, as the 8 bits sent (r7 first).
///
/// ```
/// use sdrkit_proto_bt::baseband::hec;
/// assert_eq!(hec(0, &[0, 0, 0, 0, 1, 0, 0, 0, 0, 0]), [1, 1, 1, 0, 0, 1, 1, 0]);
/// ```
pub fn hec(uap: u8, h: &[u8]) -> [u8; 8] {
    let mut r = uap;
    for &b in h {
        let fb = (b & 1) ^ (r >> 7);
        r = r << 1 | fb;
        if fb == 1 {
            r ^= 0b1010_0110;
        }
    }
    let mut out = [0u8; 8];
    for (k, o) in out.iter_mut().enumerate() {
        *o = r >> (7 - k) & 1;
    }
    out
}

/// Appends each bit of `bits` three times (FEC 1/3).
pub fn fec13_encode(bits: &[u8], out: &mut Vec<u8>) {
    for &b in bits {
        out.extend([b, b, b]);
    }
}

/// The majority of each triple of `coded` into `out`; returns the number of triples whose bits disagree.
pub fn fec13_decode(coded: &[u8], out: &mut Vec<u8>) -> usize {
    let mut split = 0;
    for t in coded.as_chunks::<3>().0 {
        let n = t.iter().map(|&b| usize::from(b & 1)).sum::<usize>();
        out.push(u8::from(n >= 2));
        split += usize::from(n == 1 || n == 2);
    }
    split
}

/// The generator of the (15,10) shortened Hamming code: D^5 + D^4 + D^2 + 1.
const FEC23_GEN: u16 = 0b11_0101;

/// The 5 parity bits of a block of 10 data bits, as the value whose bit 4 is the first parity bit sent.
fn fec23_parity(data: &[u8]) -> u16 {
    let mut r: u16 = 0;
    for (i, &b) in data.iter().enumerate() {
        r |= u16::from(b & 1) << (14 - i);
    }
    for k in (5..15).rev() {
        if r >> k & 1 == 1 {
            r ^= FEC23_GEN << (k - 5);
        }
    }
    r
}

/// Encodes `bits` (a multiple of 10) with FEC 2/3: each 10 bits are followed by their 5 parity bits.
pub fn fec23_encode(bits: &[u8], out: &mut Vec<u8>) {
    for d in bits.as_chunks::<10>().0 {
        out.extend_from_slice(d);
        let p = fec23_parity(d);
        out.extend((0..5).map(|j| (p >> (4 - j) & 1) as u8));
    }
}

/// The syndrome of a 15-bit block in air order: the parity of its data bits added to its received parity.
fn fec23_syndrome(block: &[u8]) -> u16 {
    let mut rx = 0u16;
    for j in 0..5 {
        rx |= u16::from(block[10 + j] & 1) << (4 - j);
    }
    fec23_parity(&block[..10]) ^ rx
}

/// Decodes FEC 2/3 blocks of `coded` (a multiple of 15) into `out`: one bit error per block is corrected. Returns the number of
/// corrected blocks, or `None` when a block has a syndrome of no single error.
pub fn fec23_decode(coded: &[u8], out: &mut Vec<u8>) -> Option<usize> {
    let mut fixed = 0;
    for block in coded.as_chunks::<15>().0 {
        let s = fec23_syndrome(block);
        let start = out.len();
        out.extend_from_slice(&block[..10]);
        if s == 0 {
            continue;
        }
        // A data error gives the parity of that data bit; a parity error gives that parity bit alone.
        if let Some(i) = (0..10).find(|&i| {
            let mut unit = [0u8; 10];
            unit[i] = 1;
            fec23_parity(&unit) == s
        }) {
            out[start + i] ^= 1;
        } else if s.count_ones() != 1 {
            return None;
        }
        fixed += 1;
    }
    Some(fixed)
}

/// The whitening sequence of the master clock bits CLK1..CLK6 (`clk` bit 0 is CLK1).
#[derive(Clone, Copy, Debug)]
pub struct Whitening {
    reg: u8,
}

impl Whitening {
    /// The LFSR of x^7 + x^4 + 1 preset with CLK1..CLK6 in positions 0 to 5 and a 1 in position 6.
    pub fn new(clk: u8) -> Whitening {
        Whitening {
            reg: clk & 0x3F | 0x40,
        }
    }

    /// The next bit of the sequence.
    pub fn next_bit(&mut self) -> u8 {
        let o = self.reg >> 6 & 1;
        self.reg = (self.reg << 1 | o) & 0x7F;
        self.reg ^= o << 4;
        o
    }

    /// XORs the sequence onto `bits`.
    pub fn apply(&mut self, bits: &mut [u8]) {
        for b in bits {
            *b ^= self.next_bit();
        }
    }
}

/// The 16 CRC bits of payload `bits` (air order) with the register preset 0, as sent: the least significant bit of the value
/// first. This is the CRC of a packet checked with UAP 0, as every inquiry response is. The preset for another UAP is not
/// given here.
pub fn crc16_uap0(bits: &[u8]) -> [u8; 16] {
    let v = CRC16_KERMIT.bits(bits);
    let mut out = [0u8; 16];
    for (k, o) in out.iter_mut().enumerate() {
        *o = (v >> k & 1) as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sync_words_of_the_worked_laps() {
        for (lap, s) in [
            (0x9E8B33, 0x4E7A2CCE331A3AE2u64),
            (0x9E8B00, 0x4E7A2C01EB45C348),
            (0xA1B2C3, 0x4E86CB0C825530FA),
            (0x3C5A96, 0xB0F16A585BDDF6EB),
            (0xE17B24, 0x4F85EC93B6951B30),
            (0x0F0E0D, 0xB03C383602260732),
        ] {
            assert_eq!(sync_word(lap), s, "{lap:06X}");
            assert_eq!(lap_of(s), lap);
            let barker = (s >> 57) as u8;
            assert_eq!(
                barker,
                if lap >> 23 & 1 == 1 {
                    BARKER_ONE
                } else {
                    BARKER_ZERO
                }
            );
        }
        assert_eq!(sync_word(GIAC).reverse_bits(), 0x475C58CC73345E72);
    }

    #[test]
    fn access_codes_are_found_with_up_to_two_errors_and_not_with_three() {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for k in 0..2000 {
            let lap = (next() & 0xFF_FFFF) as u32;
            let trailer = k % 2 == 0;
            let lead = (next() % 40) as usize;
            let mut bits: Vec<u8> = (0..lead).map(|_| (next() & 1) as u8).collect();
            access_code(lap, trailer, &mut bits);
            bits.extend((0..30).map(|_| (next() & 1) as u8));
            let at = lead + 4;
            // Errors in the parity part (s0..s33) only.
            let errors = k % 3;
            let mut damaged = bits.clone();
            let mut used = Vec::new();
            while used.len() < errors {
                let i = (next() % 34) as usize;
                if !used.contains(&i) {
                    used.push(i);
                    damaged[at + i] ^= 1;
                }
            }
            let mut hits = Vec::new();
            find_access_codes(&damaged, 2, |h| hits.push(h));
            let h = hits.iter().find(|h| h.at == at).expect("found");
            assert_eq!((h.lap, h.errors as usize), (lap, errors));
            if trailer {
                assert!(h.trailer);
            }
            // A third error, or a wrong LAP bit: not found there.
            let mut three = bits.clone();
            for i in [3, 11, 29] {
                three[at + i] ^= 1;
            }
            let mut wrong = bits.clone();
            wrong[at + 34 + (k % 23)] ^= 1;
            for b in [three, wrong] {
                let mut hits = Vec::new();
                find_access_codes(&b, 2, |h| hits.push(h));
                assert!(hits.iter().all(|h| h.at != at), "{k}");
            }
        }
    }

    #[test]
    fn random_bits_hold_no_access_code() {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let bits: Vec<u8> = (0..10_000_000)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 40 & 1) as u8
            })
            .collect();
        let mut n = 0;
        find_access_codes(&bits, 2, |_| n += 1);
        assert_eq!(n, 0);
    }

    #[test]
    fn the_fec23_parity_of_each_data_bit() {
        let want = [0x0B, 0x16, 0x07, 0x0E, 0x1C, 0x13, 0x0D, 0x1A, 0x1F, 0x15];
        for (i, &w) in want.iter().enumerate() {
            let mut d = [0u8; 10];
            d[i] = 1;
            let mut c = Vec::new();
            fec23_encode(&d, &mut c);
            let v = (0..5).fold(0u8, |a, j| a | c[10 + j] << j);
            assert_eq!(v, w, "data bit {i}");
        }
    }

    #[test]
    fn fec23_corrects_one_error_and_refuses_two() {
        let d: Vec<u8> = (0..30).map(|i| ((i * 7 + 3) % 5 % 2) as u8).collect();
        let mut c = Vec::new();
        fec23_encode(&d, &mut c);
        for e in 0..15 {
            let mut x = c.clone();
            x[15 + e] ^= 1;
            let mut out = Vec::new();
            assert_eq!(fec23_decode(&x, &mut out), Some(1));
            assert_eq!(out, d);
        }
        // Two errors in a block give either a refusal or a wrong correction; never the sent bits with 0 corrections.
        let mut x = c.clone();
        x[0] ^= 1;
        x[1] ^= 1;
        let mut out = Vec::new();
        assert_ne!(fec23_decode(&x, &mut out).map(|_| out == d), Some(true));
    }

    #[test]
    fn the_hec_of_the_fhs_header() {
        let h = header_bits(0, TYPE_FHS, false, false, false);
        assert_eq!(h, [0, 0, 0, 0, 1, 0, 0, 0, 0, 0]);
        assert_eq!(hec(0, &h), [1, 1, 1, 0, 0, 1, 1, 0]);
        assert_eq!(header_fields(&h), (0, 2, false, false, false));
    }

    #[test]
    fn whitening_is_its_own_inverse_and_has_period_127() {
        let mut w = Whitening::new(0x15);
        let first: Vec<u8> = (0..127).map(|_| w.next_bit()).collect();
        let again: Vec<u8> = (0..127).map(|_| w.next_bit()).collect();
        assert_eq!(first, again);
        let mut bits = vec![1u8; 40];
        Whitening::new(7).apply(&mut bits);
        Whitening::new(7).apply(&mut bits);
        assert_eq!(bits, vec![1u8; 40]);
    }
}
