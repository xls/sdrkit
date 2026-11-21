//! The cdma2000 Sync Channel: its coding (convolutional code, repetition, block interleaver), the capsule of the
//! signalling layer (MSG_LENGTH, PDU, CRC-30, SOM framing and reassembly), the Sync Channel Message, System Time as text and
//! the channel numbers of band classes 0, 1 and 4.
//!
//! Standard: 3GPP2 C.S0002-C (3.1.3.1.4, 3.1.3.1.5, 3.1.3.1.7, 3.1.3.3, 2.1.1.1), C.S0004 (2.1.1.5.1.2, 2.1.2.4.2, 3.1.2.3.1.3,
//! Annex A) and C.S0005 (3.7.2.3.2.26).

use sdrkit_util_bits::crc::catalog::CRC30_CDMA;
use sdrkit_util_bits::pack::{field, push_u64 as push, sign_extend, to_u64};
use serde::{Deserialize, Serialize};

/// Bits of a Sync Channel frame (26.666 ms at 1200 bit/s).
pub const FRAME_BITS: usize = 32;
/// Modulation symbols of a frame after repetition.
pub const FRAME_SYMBOLS: usize = 128;
/// Frames of a superframe (80 ms).
pub const SUPERFRAME_FRAMES: usize = 3;
/// Bits of the Sync Channel Message fields read, MSG_TYPE included.
pub const MESSAGE_BITS: usize = 181;

/// The 7-bit reversal of `i`: the read address of the 128-symbol interleaver (C.S0002-C 3.1.3.1.7.1.1, `m = 7, J = 1`).
pub fn bro7(i: usize) -> usize {
    ((i as u8).reverse_bits() >> 1) as usize
}

/// The modulation symbols of one frame from its 64 code symbols: each repeated twice (adjacent copies), then interleaved
/// (symbol `i` sent is the repeated symbol at address `BRO_7(i)`).
pub fn frame_symbols(code: &[u8]) -> [u8; FRAME_SYMBOLS] {
    let mut rep = [0u8; FRAME_SYMBOLS];
    for (j, &c) in code.iter().take(64).enumerate() {
        rep[2 * j] = c;
        rep[2 * j + 1] = c;
    }
    let mut out = [0u8; FRAME_SYMBOLS];
    for (i, o) in out.iter_mut().enumerate() {
        *o = rep[bro7(i)];
    }
    out
}

/// The 64 code-symbol soft values of one frame from its 128 received soft symbols, in sending order: deinterleaved and the two
/// copies of each code symbol added.
pub fn frame_soft(received: &[f32]) -> [f32; 64] {
    let mut rep = [0f32; FRAME_SYMBOLS];
    for (i, &v) in received.iter().take(FRAME_SYMBOLS).enumerate() {
        rep[bro7(i)] = v;
    }
    let mut out = [0f32; 64];
    for (j, o) in out.iter_mut().enumerate() {
        *o = rep[2 * j] + rep[2 * j + 1];
    }
    out
}

/// The Sync Channel Message (C.S0005 3.7.2.3.2.26): the fields of protocol revision 6. Later revisions add fields after
/// EXT_CDMA_FREQ, which are not read.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncMessage {
    /// Protocol revision of the base station.
    pub p_rev: u8,
    /// Lowest protocol revision the base station accepts.
    pub min_p_rev: u8,
    /// System identification.
    pub sid: u16,
    /// Network identification.
    pub nid: u16,
    /// The base station's pilot PN offset, units of 64 chips.
    pub pilot_pn: u16,
    /// The long code state at SYS_TIME (42 bits).
    pub lc_state: u64,
    /// System Time in units of 80 ms (36 bits).
    pub sys_time: u64,
    /// Leap seconds since the start of System Time.
    pub lp_sec: u8,
    /// Offset of local time, units of 30 minutes.
    pub ltm_off: i8,
    /// Daylight saving in effect.
    pub daylt: bool,
    /// Paging Channel rate: 0 9600 bit/s, 1 4800 bit/s, 2 and 3 reserved.
    pub prat: u8,
    /// CDMA channel number of the Primary Paging Channel.
    pub cdma_freq: u16,
    /// The channel of the Primary Paging Channel for radio configurations above 2.
    pub ext_cdma_freq: u16,
}

impl SyncMessage {
    /// The PDU: MSG_TYPE 00000001, the fields, and the fewest zero bits that make the length `8 k + 2`.
    pub fn pdu(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(192);
        push(&mut b, 1, 8);
        push(&mut b, u64::from(self.p_rev), 8);
        push(&mut b, u64::from(self.min_p_rev), 8);
        push(&mut b, u64::from(self.sid), 15);
        push(&mut b, u64::from(self.nid), 16);
        push(&mut b, u64::from(self.pilot_pn), 9);
        push(&mut b, self.lc_state, 42);
        push(&mut b, self.sys_time, 36);
        push(&mut b, u64::from(self.lp_sec), 8);
        push(&mut b, (self.ltm_off as u8 & 0x3F) as u64, 6);
        push(&mut b, u64::from(self.daylt), 1);
        push(&mut b, u64::from(self.prat), 2);
        push(&mut b, u64::from(self.cdma_freq), 11);
        push(&mut b, u64::from(self.ext_cdma_freq), 11);
        while b.len() % 8 != 2 {
            b.push(0);
        }
        b
    }

    /// The capsule: MSG_LENGTH (octets of the whole capsule), the PDU, and the CRC-30 over both.
    pub fn capsule(&self) -> Vec<u8> {
        let pdu = self.pdu();
        let mut b = Vec::with_capacity(pdu.len() + 38);
        push(&mut b, ((pdu.len() + 38) / 8) as u64, 8);
        b.extend_from_slice(&pdu);
        let crc = CRC30_CDMA.bits(&b);
        push(&mut b, u64::from(crc), 30);
        b
    }

    /// The fields of a PDU (bits, MSG_TYPE first). `None` unless it is a Sync Channel Message of at least 181 bits.
    pub fn parse(pdu: &[u8]) -> Option<SyncMessage> {
        if pdu.len() < MESSAGE_BITS {
            return None;
        }
        let mut at = 0;
        let mut u = |n: u32| {
            let v = field(pdu, at, n);
            at += n as usize;
            v
        };
        if u(8)? != 1 {
            return None;
        }
        Some(SyncMessage {
            p_rev: u(8)? as u8,
            min_p_rev: u(8)? as u8,
            sid: u(15)? as u16,
            nid: u(16)? as u16,
            pilot_pn: u(9)? as u16,
            lc_state: u(42)?,
            sys_time: u(36)?,
            lp_sec: u(8)? as u8,
            ltm_off: sign_extend(u(6)?, 6) as i8,
            daylt: u(1)? == 1,
            prat: u(2)? as u8,
            cdma_freq: u(11)? as u16,
            ext_cdma_freq: u(11)? as u16,
        })
    }

    /// The frames of the message: the capsule after SOM = 1 in the first frame and SOM = 0 after it, 31 capsule bits a frame,
    /// zeros to the end of the superframe in which the capsule ends. Each frame is 32 bits.
    pub fn frames(&self) -> Vec<[u8; FRAME_BITS]> {
        let cap = self.capsule();
        let n = cap.len().div_ceil(FRAME_BITS - 1);
        let n = n.div_ceil(SUPERFRAME_FRAMES) * SUPERFRAME_FRAMES;
        (0..n)
            .map(|f| {
                let mut fr = [0u8; FRAME_BITS];
                fr[0] = u8::from(f == 0);
                for (k, b) in fr[1..].iter_mut().enumerate() {
                    *b = cap.get(f * (FRAME_BITS - 1) + k).copied().unwrap_or(0);
                }
                fr
            })
            .collect()
    }

    /// The UTC time of SYS_TIME as text, "2026-11-14 19:33:02.00 UTC" (System Time counts from 1980-01-06 00:00:00 UTC without
    /// leap seconds, so UTC is the epoch plus SYS_TIME x 80 ms less LP_SEC).
    pub fn utc_text(&self) -> String {
        civil_text(self.unix_ms(0))
    }

    /// The local time as text, "2026-11-14 21:33:02.00" (UTC plus LTM_OFF x 30 minutes).
    pub fn local_text(&self) -> String {
        civil_text(self.unix_ms(i64::from(self.ltm_off) * 1_800_000))
            .trim_end_matches(" UTC")
            .to_string()
    }

    fn unix_ms(&self, plus_ms: i64) -> i64 {
        sdrkit_util_geo::time::gnss::GPS_EPOCH_UNIX * 1000 + self.sys_time as i64 * 80
            - i64::from(self.lp_sec) * 1000
            + plus_ms
    }
}

/// "YYYY-MM-DD hh:mm:ss.ss UTC" of a Unix time in milliseconds.
fn civil_text(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let rest = ms.rem_euclid(1000) as u32;
    match sdrkit_util_geo::time::Civil::from_unix(secs, rest * 1_000_000) {
        Ok(c) => format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:02} UTC",
            c.year,
            c.month,
            c.day,
            c.hour,
            c.minute,
            c.second,
            rest / 10
        ),
        Err(_) => String::new(),
    }
}

/// The reassembly of capsules from Sync Channel frames (C.S0004 2.1.2.4.2).
#[derive(Clone, Debug, Default)]
pub struct Reassembly {
    buf: Vec<u8>,
}

/// What one frame gave.
#[derive(Clone, Debug, PartialEq)]
pub enum Capsule {
    /// A capsule whose CRC is right: its PDU.
    Good(Vec<u8>),
    /// A capsule whose CRC is wrong.
    Bad,
}

impl Reassembly {
    /// Takes one frame of 32 bits. SOM = 1 starts a capsule; a frame with SOM = 0 and nothing held is dropped; a MSG_LENGTH below
    /// 5 drops the bits; a capsule of MSG_LENGTH octets is checked by its CRC and returned.
    pub fn push(&mut self, frame: &[u8]) -> Option<Capsule> {
        if frame.len() < FRAME_BITS {
            return None;
        }
        if frame[0] == 1 {
            self.buf.clear();
        } else if self.buf.is_empty() {
            return None;
        }
        self.buf.extend_from_slice(&frame[1..FRAME_BITS]);
        if self.buf.len() < 8 {
            return None;
        }
        let len = to_u64(&self.buf[..8]) as usize;
        if len < 5 {
            self.buf.clear();
            return None;
        }
        if self.buf.len() < 8 * len {
            return None;
        }
        let body = &self.buf[..8 * len - 30];
        let sent = to_u64(&self.buf[8 * len - 30..8 * len]) as u32;
        let ok = CRC30_CDMA.bits(body) == sent;
        let out = if ok {
            Capsule::Good(body[8..].to_vec())
        } else {
            Capsule::Bad
        };
        self.buf.clear();
        Some(out)
    }
}

/// The base station transmit centre of channel `n` in band class `bc`, MHz (C.S0002-C 2.1.1.1.1, 2.1.1.1.2 and 2.1.1.1.5;
/// worldwide channel arithmetic of the standard). `None` for another band class or a channel outside the class.
pub fn channel_mhz(bc: u8, n: u16) -> Option<f64> {
    let n = f64::from(n);
    match (bc, n as u16) {
        (0, 1..=799) => Some(870.0 + 0.030 * n),
        (0, 991..=1023) => Some(870.0 + 0.030 * (n - 1023.0)),
        (1, 0..=1199) => Some(1930.0 + 0.050 * n),
        (4, 0..=599) => Some(1840.0 + 0.050 * n),
        _ => None,
    }
}
