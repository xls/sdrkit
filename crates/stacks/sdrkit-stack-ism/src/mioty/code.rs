//! The mioty (TS-UNB) uplink frame: the TSMA patterns, the pilots, the CRCs, whitening, the rate 1/3 code, the spread of the
//! coded bits over the radio bursts, the extension bursts and the MAC header of the fixed MAC mode.
//! Standard: ETSI TS 103 357 V1.1.1 clause 6 (uplink, standard TSMA mode).

use sdrkit_algo_fec::conv::codes::MIOTY_K7_R3;
use sdrkit_algo_fec::conv::{Encoder, Ends, Viterbi};
use sdrkit_algo_fec::llr::Llr8;
use sdrkit_util_bits::crc::catalog::CRC8_CDMA2000;
use sdrkit_util_bits::seq::PN9_WHITENING;

/// The symbol rate, symbols per second; also the carrier spacing, Hz.
pub const SYMBOL_RATE: f64 = 2380.371;
/// Symbols of a radio burst.
pub const BURST_SYMBOLS: usize = 36;
/// The first pilot symbol of a burst.
pub const PILOT_START: usize = 12;
/// Pilot symbols of a burst.
pub const PILOT_SYMBOLS: usize = 12;
/// Radio bursts of a core frame.
pub const CORE_BURSTS: usize = 24;
/// Carriers of the core frame.
pub const CORE_CARRIERS: u16 = 24;
/// The pilot of a core burst, in symbol order.
pub const CORE_PILOT: [u8; 12] = [0, 1, 1, 1, 0, 1, 0, 0, 0, 0, 1, 0];
/// The pilot of an extension burst.
pub const EXTENSION_PILOT: [u8; 12] = [0, 1, 0, 0, 1, 1, 1, 1, 1, 0, 1, 0];
/// MPDU bytes that the core frame carries; each further byte adds one extension burst.
pub const CORE_MPDU: usize = 20;
/// Coded bits of a core frame.
pub const CORE_CODED: usize = 576;
/// The most MPDU bytes.
pub const MAX_PSI: usize = 245;
/// The polynomial of the Galois LFSR of the extension bursts.
pub const EXTENSION_POLY: u16 = 0xB4F3;
/// The carriers that an extension burst takes (its carrier is the LFSR value / 256 modulo this).
pub const EXTENSION_CARRIERS: u16 = 25;

/// An uplink pattern group: its name, the carrier of each core burst, the symbols between the pilot centres of each burst
/// and the one before (the first is 0), and the group spacing T_UPG in symbols.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UplinkPattern {
    /// "UPG1/3".
    pub name: &'static str,
    /// The carrier of each core burst.
    pub carriers: [u8; 24],
    /// Symbols from the pilot centre of the burst before (0 for burst 0).
    pub gaps: [u16; 24],
    /// The group spacing, symbols.
    pub spacing: u16,
}

macro_rules! patterns {
    ($($name:literal, [$($c:literal)*], [$($t:literal)*], $s:literal;)*) => {
        /// The 17 uplink patterns of TS 103 357 Tables 6-46 and 6-49 to 6-54.
        pub const PATTERNS: [UplinkPattern; 17] = [$(UplinkPattern {
            name: $name,
            carriers: [$($c),*],
            gaps: [0, $($t),*],
            spacing: $s,
        }),*];
    };
}

patterns! {
    "UPG1/1", [5 21 13 6 22 14 1 17 9 0 16 8 7 23 15 4 20 12 3 19 11 2 18 10], [330 387 388 330 387 354 330 387 356 330 387 432 330 387 352 330 387 467 330 387 620 330 387], 337;
    "UPG1/2", [4 20 12 1 17 9 0 16 8 6 22 14 7 23 15 2 18 10 5 21 13 3 19 11], [330 387 435 330 387 409 330 387 398 330 387 370 330 387 361 330 387 472 330 387 522 330 387], 337;
    "UPG1/3", [4 20 12 3 19 11 6 22 14 7 23 15 0 16 8 5 21 13 2 18 10 1 17 9], [330 387 356 330 387 439 330 387 413 330 387 352 330 387 485 330 387 397 330 387 444 330 387], 337;
    "UPG1/4", [6 22 14 2 18 10 7 23 15 0 16 8 1 17 9 4 20 12 5 21 13 3 19 11], [330 387 352 330 387 382 330 387 381 330 387 365 330 387 595 330 387 604 330 387 352 330 387], 337;
    "UPG1/5", [7 23 15 4 20 12 3 19 11 2 18 10 6 22 14 0 16 8 1 17 9 5 21 13], [330 387 380 330 387 634 330 387 360 330 387 393 330 387 352 330 387 373 330 387 490 330 387], 337;
    "UPG1/6", [3 19 11 6 22 14 2 18 10 0 16 8 7 23 15 1 17 9 4 20 12 5 21 13], [330 387 364 330 387 375 330 387 474 330 387 355 330 387 478 330 387 464 330 387 513 330 387], 337;
    "UPG1/7", [3 19 11 1 17 9 5 21 13 7 23 15 0 16 8 2 18 10 6 22 14 4 20 12], [330 387 472 330 387 546 330 387 501 330 387 356 330 387 359 330 387 359 330 387 364 330 387], 337;
    "UPG1/8", [0 16 8 6 22 14 3 19 11 2 18 10 4 20 12 7 23 15 5 21 13 1 17 9], [330 387 391 330 387 468 330 387 512 330 387 543 330 387 354 330 387 391 330 387 368 330 387], 337;
    "UPG2/1", [4 20 12 0 16 8 3 19 11 5 21 13 1 17 9 7 23 15 2 18 10 6 22 14], [373 319 545 373 319 443 373 319 349 373 319 454 373 319 578 373 319 436 373 319 398 373 319], 337;
    "UPG2/2", [3 19 11 7 23 15 2 18 10 5 21 13 4 20 12 0 16 8 1 17 9 6 22 14], [373 319 371 373 319 410 373 319 363 373 319 354 373 319 379 373 319 657 373 319 376 373 319], 337;
    "UPG2/3", [6 22 14 0 16 8 1 17 9 4 20 12 3 19 11 5 21 13 2 18 10 7 23 15], [373 319 414 373 319 502 373 319 433 373 319 540 373 319 428 373 319 467 373 319 409 373 319], 337;
    "UPG2/4", [3 19 11 1 17 9 4 20 12 5 21 13 2 18 10 7 23 15 6 22 14 0 16 8], [373 319 396 373 319 516 373 319 631 373 319 471 373 319 457 373 319 416 373 319 354 373 319], 337;
    "UPG2/5", [5 21 13 2 18 10 0 16 8 6 22 14 7 23 15 1 17 9 4 20 12 3 19 11], [373 319 655 373 319 416 373 319 367 373 319 400 373 319 415 373 319 342 373 319 560 373 319], 337;
    "UPG2/6", [1 17 9 3 19 11 4 20 12 6 22 14 7 23 15 5 21 13 2 18 10 0 16 8], [373 319 370 373 319 451 373 319 465 373 319 593 373 319 545 373 319 380 373 319 365 373 319], 337;
    "UPG2/7", [5 21 13 1 17 9 2 18 10 4 20 12 3 19 11 0 16 8 6 22 14 7 23 15], [373 319 393 373 319 374 373 319 344 373 319 353 373 319 620 373 319 503 373 319 546 373 319], 337;
    "UPG2/8", [3 19 11 6 22 14 5 21 13 1 17 9 7 23 15 2 18 10 0 16 8 4 20 12], [373 319 367 373 319 346 373 319 584 373 319 579 373 319 519 373 319 351 373 319 486 373 319], 337;
    "UPG3", [1 5 4 3 2 17 21 20 19 18 9 13 12 11 10 6 0 7 22 16 23 14 8 15], [66 66 66 66 66 66 66 66 66 123 66 66 66 66 60 66 66 198 66 66 255 66 66], 66;
}

impl UplinkPattern {
    /// The time of each core burst after burst 0, symbols.
    pub fn times(&self) -> [u32; 24] {
        let mut t = [0u32; 24];
        for k in 1..24 {
            t[k] = t[k - 1] + u32::from(self.gaps[k]);
        }
        t
    }
}

/// The next value of the extension LFSR: a right shift, XOR [`EXTENSION_POLY`] when the bit shifted out was 1.
pub fn lfsr_next(r: u16) -> u16 {
    let out = r & 1;
    let r = r >> 1;
    if out == 1 {
        r ^ EXTENSION_POLY
    } else {
        r
    }
}

/// The extension bursts of a frame with header CRC `hcrc` and payload CRC `pcrc`: for each, (symbols after the burst before,
/// carrier).
pub fn extension_bursts(hcrc: u8, pcrc: u8, count: usize, spacing: u16) -> Vec<(u32, u16)> {
    let mut r = 0x8000 | u16::from(hcrc & 0x7F) << 8 | u16::from(pcrc);
    (0..count)
        .map(|_| {
            r = lfsr_next(r);
            (
                u32::from(spacing) + u32::from(r % 128),
                (r >> 8) % EXTENSION_CARRIERS,
            )
        })
        .collect()
}

/// Extension bursts of an MPDU of `psi` bytes.
pub fn extension_count(psi: usize) -> usize {
    psi.saturating_sub(CORE_MPDU)
}

/// Coded bits of an MPDU of `psi` bytes.
pub fn coded_len(psi: usize) -> usize {
    3 * (24 + 8 * psi.max(CORE_MPDU) + 2 + 6)
}

fn bits_of(v: u8, out: &mut Vec<u8>) {
    out.extend((0..8).rev().map(|i| v >> i & 1));
}

/// The CRC-8 of TS-UNB (CRC-8/CDMA2000) of bits (most significant first).
pub fn crc8(bits: &[u8]) -> u8 {
    CRC8_CDMA2000.bits(bits)
}

/// The payload CRC (over the MPDU and the 2 MMODE bits) and the header CRC (over the payload CRC and PSI).
pub fn crcs(mpdu: &[u8], mmode: u8) -> (u8, u8) {
    let mut b = Vec::with_capacity(8 * mpdu.len() + 2);
    for &x in mpdu {
        bits_of(x, &mut b);
    }
    b.extend([mmode >> 1 & 1, mmode & 1]);
    let p = crc8(&b);
    let mut h = Vec::with_capacity(16);
    bits_of(p, &mut h);
    bits_of(mpdu.len() as u8, &mut h);
    (crc8(&h), p)
}

/// XORs the PN9 whitening sequence onto `bits`.
pub fn whiten(bits: &mut [u8]) {
    let mut l = PN9_WHITENING;
    for b in bits {
        *b ^= l.bit();
    }
}

/// The coded bits of `mpdu` (1 to [`MAX_PSI`] bytes) with MMODE `mmode`, in encoder output order. Returns them and the two CRCs
/// (header, payload).
pub fn encode(mpdu: &[u8], mmode: u8) -> (Vec<u8>, u8, u8) {
    let (hcrc, pcrc) = crcs(mpdu, mmode);
    let mut b = Vec::with_capacity(coded_len(mpdu.len()) / 3);
    bits_of(hcrc, &mut b);
    bits_of(pcrc, &mut b);
    bits_of(mpdu.len() as u8, &mut b);
    for k in 0..mpdu.len().max(CORE_MPDU) {
        bits_of(mpdu.get(k).copied().unwrap_or(0), &mut b);
    }
    b.extend([mmode >> 1 & 1, mmode & 1]);
    whiten(&mut b);
    b.extend([0; 6]);
    let mut enc = Encoder::new(MIOTY_K7_R3);
    let mut out = Vec::with_capacity(3 * b.len());
    enc.encode(&b, &mut out);
    (out, hcrc, pcrc)
}

/// The symbol order of the bits of a burst: for the o-th bit of an even burst, its symbol.
const EVEN: [u8; 24] = [
    11, 24, 10, 25, 9, 26, 8, 27, 7, 28, 6, 29, 5, 30, 4, 31, 3, 32, 2, 33, 1, 34, 0, 35,
];
/// The same for an odd burst.
const ODD: [u8; 24] = [
    24, 11, 25, 10, 26, 9, 27, 8, 28, 7, 29, 6, 30, 5, 31, 4, 32, 3, 33, 2, 34, 1, 35, 0,
];

/// For each coded bit (encoder output order) of a frame of `n` coded bits, its burst and its symbol.
pub fn placement(n: usize) -> Vec<(u16, u8)> {
    let se = (n - CORE_CODED) / 24;
    let g_len = 12 + se;
    let mut filled = vec![0usize; CORE_BURSTS + se];
    let mut by_k = vec![(0u16, 0u8); n];
    for (k, slot) in by_k.iter_mut().enumerate() {
        let burst = if k < 288 {
            k % 24
        } else {
            let g = (k - 288) / g_len;
            let i = (k - 288) % g_len;
            if i < 12 {
                2 * i + g % 2
            } else {
                i + 12
            }
        };
        let o = filled[burst];
        filled[burst] += 1;
        let m = if burst % 2 == 0 { EVEN[o] } else { ODD[o] };
        *slot = (burst as u16, m);
    }
    (0..n).map(|j| by_k[(j + 48) % n]).collect()
}

/// The header of a received frame: the CRCs and PSI, from soft values of the first 240 coded bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// The header CRC.
    pub hcrc: u8,
    /// The payload CRC.
    pub pcrc: u8,
    /// MPDU bytes.
    pub psi: usize,
}

/// Reads the header from LLRs of the first 240 coded bits: Viterbi from state 0 over 80 steps, dewhitening, the header CRC and
/// PSI from 1 to [`MAX_PSI`].
pub fn read_header(v: &mut Viterbi, llr: &[Llr8]) -> Option<Header> {
    let mut out = [0u8; 80];
    v.decode_only(llr.get(..240)?, Ends::Start(0), &mut out)
        .ok()?;
    let mut b = [0u8; 24];
    b.copy_from_slice(&out[..24]);
    whiten(&mut b);
    let byte = |k: usize| b[8 * k..8 * k + 8].iter().fold(0u8, |a, &x| a << 1 | x);
    let (hcrc, pcrc, psi) = (byte(0), byte(1), usize::from(byte(2)));
    if crc8(&b[8..24]) != hcrc || !(1..=MAX_PSI).contains(&psi) {
        return None;
    }
    Some(Header { hcrc, pcrc, psi })
}

/// Decodes a whole frame of `psi` bytes from the LLRs of its coded bits: terminated Viterbi, dewhitening, both CRCs. Returns
/// the MPDU and MMODE.
pub fn read_frame(v: &mut Viterbi, llr: &[Llr8], psi: usize) -> Option<(Vec<u8>, u8)> {
    let n = coded_len(psi);
    let info = n / 3 - 6;
    let mut out = vec![0u8; info];
    v.decode_only(llr.get(..n)?, Ends::Zero, &mut out).ok()?;
    whiten(&mut out);
    let byte = |k: usize| out[8 * k..8 * k + 8].iter().fold(0u8, |a, &x| a << 1 | x);
    if usize::from(byte(2)) != psi {
        return None;
    }
    let mpdu: Vec<u8> = (0..psi).map(|k| byte(3 + k)).collect();
    let m = 24 + 8 * psi.max(CORE_MPDU);
    let mmode = out[m] << 1 | out[m + 1];
    let (hcrc, pcrc) = crcs(&mpdu, mmode);
    (hcrc == byte(0) && pcrc == byte(1)).then_some((mpdu, mmode))
}

/// The MAC header of the fixed MAC mode, as read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mac {
    /// The device id: "EUI-64 0123456789ABCDEF" or "short 1234".
    pub id: String,
    /// The packet counter.
    pub counter: u32,
    /// The payload is not encrypted (attach).
    pub attach: bool,
    /// Payload bytes.
    pub payload: usize,
    /// The text: "short address 1234, counter 1, 6 payload bytes (encrypted)".
    pub text: String,
}

/// Reads the MAC header of `mpdu`: byte 0 (bit 0 the least significant: bit 1 one more header byte, bit 5 a full address,
/// bit 6 attach), the address, the 24-bit counter, the optional byte, the payload and 4 bytes of authentication code.
pub fn read_mac(mpdu: &[u8]) -> Option<Mac> {
    let b0 = *mpdu.first()?;
    let full = b0 >> 5 & 1 == 1;
    let alen = if full { 8 } else { 2 };
    let extra = usize::from(b0 >> 1 & 1);
    let head = 1 + alen + 3 + extra;
    if mpdu.len() < head + 4 {
        return None;
    }
    let addr = crate::family::hex(&mpdu[1..1 + alen]);
    let c = &mpdu[1 + alen..4 + alen];
    let counter = u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]);
    let attach = b0 >> 6 & 1 == 1;
    let payload = mpdu.len() - head - 4;
    let (id, label) = if full {
        (format!("EUI-64 {addr}"), format!("EUI-64 {addr}"))
    } else {
        (format!("short {addr}"), format!("short address {addr}"))
    };
    let mut text = format!("{label}, counter {counter}, {payload} payload bytes");
    text.push_str(if attach {
        " (attach, not encrypted)"
    } else {
        " (encrypted)"
    });
    if b0 >> 2 & 1 == 1 {
        text.push_str(", control");
    }
    if b0 >> 3 & 1 == 1 {
        text.push_str(", response wanted");
    }
    if b0 >> 7 & 1 == 1 {
        text.push_str(", acknowledgement");
    }
    Some(Mac {
        id,
        counter,
        attach,
        payload,
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_worked_crcs() {
        assert_eq!(crc8(&[0; 16]), 0xB1);
        assert_eq!(CRC8_CDMA2000.checksum(b"123456789"), 0xDA);
        let mpdu = [
            0x00, 0x12, 0x34, 0x00, 0x00, 0x01, 0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04,
        ];
        assert_eq!(crcs(&mpdu, 0), (0x22, 0xDC));
        let m = read_mac(&mpdu).unwrap();
        assert_eq!(m.id, "short 1234");
        assert_eq!(
            m.text,
            "short address 1234, counter 1, 4 payload bytes (encrypted)"
        );
    }

    #[test]
    fn whitening_starts_as_the_standard_says() {
        let mut b = [0u8; 16];
        whiten(&mut b);
        assert_eq!(b, [1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1]);
    }

    #[test]
    fn the_code_and_the_masks() {
        let mut enc = Encoder::new(MIOTY_K7_R3);
        let mut out = Vec::new();
        enc.encode(&[1, 0, 0, 0, 0, 0, 0], &mut out);
        // The impulse response of each output, newest bit first: the masks 5B, 65, 7D read from bit 0.
        for (g, mask) in [0x5Bu8, 0x65, 0x7D].iter().enumerate() {
            let r: Vec<u8> = out.iter().skip(g).step_by(3).copied().collect();
            let want: Vec<u8> = (0..7).map(|i| mask >> i & 1).collect();
            assert_eq!(r, want, "generator {g}");
        }
        assert_eq!(coded_len(20), CORE_CODED);
        assert_eq!(coded_len(21), 600);
    }

    #[test]
    fn every_symbol_of_every_burst_gets_one_bit() {
        for psi in [14usize, 20, 21, 30, 245] {
            let n = coded_len(psi);
            let p = placement(n);
            let bursts = CORE_BURSTS + extension_count(psi);
            let mut seen = vec![[0u8; 36]; bursts];
            for &(b, m) in &p {
                seen[usize::from(b)][usize::from(m)] += 1;
            }
            for s in &seen {
                for (m, &c) in s.iter().enumerate() {
                    let data = !(12..24).contains(&m);
                    assert_eq!(c, u8::from(data), "psi {psi}");
                }
            }
            // The first 240 coded bits lie in the core bursts.
            assert!(p[..240].iter().all(|&(b, _)| usize::from(b) < CORE_BURSTS));
        }
    }

    #[test]
    fn frames_come_back_through_the_decoders() {
        let mut v = Viterbi::new(MIOTY_K7_R3, 2000).unwrap();
        for psi in [14usize, 15, 20, 21, 30] {
            let mut mpdu: Vec<u8> = (0..psi).map(|k| (k * 37 + 5) as u8).collect();
            mpdu[..6].copy_from_slice(&[0, 0x12, 0x34, 0, 0, 1]);
            let (coded, hcrc, pcrc) = encode(&mpdu, 2);
            let llr: Vec<Llr8> = coded
                .iter()
                .map(|&b| if b == 1 { -60 } else { 60 })
                .collect();
            let h = read_header(&mut v, &llr).unwrap();
            assert_eq!((h.hcrc, h.pcrc, h.psi), (hcrc, pcrc, psi));
            assert_eq!(read_frame(&mut v, &llr, psi), Some((mpdu, 2)));
        }
    }

    #[test]
    fn the_patterns_use_every_carrier_once() {
        for p in &PATTERNS {
            let mut c = p.carriers;
            c.sort_unstable();
            assert_eq!(c.to_vec(), (0..24).collect::<Vec<u8>>(), "{}", p.name);
        }
        assert_eq!(PATTERNS[16].times()[23], 66 * 19 + 123 + 60 + 198 + 255);
        let ext = extension_bursts(0x22, 0xDC, 3, 337);
        assert!(ext.iter().all(|&(t, c)| (337..465).contains(&t) && c < 25));
    }
}
