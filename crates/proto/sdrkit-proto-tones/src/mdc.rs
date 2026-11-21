//! The MDC-1200 burst: 1200 bit/s in the audio of an analogue FM voice channel, tones of 1200 and 1800 Hz. There is no
//! public standard. This module holds the encoder, the test signal and the decoder.
//!
//! A burst is 7 bytes 0x55, the sync 07 09 2A 44 6F and a block of 112 bits. A bit that differs from the bit before it is one
//! and a half cycles of 1800 Hz, an equal bit is one cycle of 1200 Hz. A block is 14 bytes: operation code, argument, unit id
//! (high, low), the check value of these four (low, high), 0x00, and seven parity bytes. Its bits are sent in seven rows of 16.
//! A double packet (operation codes 0x35 and 0x55) has a second block right after the first.

use crate::{Param, TonesError};
use num_complex::Complex32;
use sdrkit_algo_modem::fsk::ToneCorrelator;
use std::f64::consts::TAU;

/// Bits a second.
pub const BIT_RATE: f64 = 1200.0;
/// The tone of a bit equal to the bit before it, Hz.
pub const SAME_HZ: f64 = 1200.0;
/// The tone of a bit that differs from the bit before it, Hz.
pub const CHANGE_HZ: f64 = 1800.0;
/// The sync bytes, most significant bit first.
pub const SYNC: [u8; 5] = [0x07, 0x09, 0x2A, 0x44, 0x6F];
/// The bytes of a block.
pub const BLOCK_BYTES: usize = 14;
/// The bits of a block.
pub const BLOCK_BITS: usize = 112;
/// The lowest audio rate of the decoder, Hz.
pub const MIN_RATE_HZ: f64 = 8000.0;
/// The sync as tone decisions (1 is 1800 Hz), first decision in the highest bit, after a bit 1.
pub const SYNC_DECISIONS: u64 = 0x84_8D_BF_66_58;
/// The decoders with bit boundaries one eighth of a bit apart.
const PHASES: usize = 8;

/// A packet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Packet {
    /// The operation code.
    pub op: u8,
    /// The argument.
    pub arg: u8,
    /// The unit id.
    pub unit: u16,
    /// A second block followed.
    pub double: bool,
    /// The four data bytes of the second block. Zero when it failed its check.
    pub extra: [u8; 4],
}

impl Packet {
    /// A single packet.
    pub fn new(op: u8, arg: u8, unit: u16) -> Packet {
        Packet {
            op,
            arg,
            unit,
            ..Packet::default()
        }
    }
    /// A double packet with the four bytes `extra`.
    pub fn double(op: u8, arg: u8, unit: u16, extra: [u8; 4]) -> Packet {
        Packet {
            op,
            arg,
            unit,
            double: true,
            extra,
        }
    }
    /// What the packet means, in words.
    pub fn text(&self) -> String {
        match (self.op, self.arg) {
            (0x01, 0x80) => "PTT ID (start of transmission)".into(),
            (0x01, 0x00) => "PTT ID (end of transmission)".into(),
            (0x01, _) => "PTT ID".into(),
            (op, arg) => format!(
                "op 0x{op:02X} arg 0x{arg:02X}{}",
                if self.double { " (double packet)" } else { "" }
            ),
        }
    }
}

/// True for an operation code that a second block follows.
pub fn is_double(op: u8) -> bool {
    op == 0x35 || op == 0x55
}

/// The check value of `bytes`: CRC-16 with x^16 + x^12 + x^5 + 1, bits entered least significant first, start 0, inverted.
pub fn check_value(bytes: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &b in bytes {
        crc ^= u16::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0x8408
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// The 14 bytes of a block with the four data bytes `data`, before interleaving.
pub fn block_bytes(data: [u8; 4]) -> [u8; BLOCK_BYTES] {
    let mut b = [0u8; BLOCK_BYTES];
    b[..4].copy_from_slice(&data);
    let crc = check_value(&data);
    b[4] = crc as u8;
    b[5] = (crc >> 8) as u8;
    let mut register = 0u8;
    for i in 0..7 {
        for j in 0..8 {
            let bit = (b[i] >> j) & 1;
            register = ((register << 1) | bit) & 0x7F;
            let parity = (register ^ (register >> 2) ^ (register >> 5) ^ (register >> 6)) & 1;
            b[7 + i] |= parity << j;
        }
    }
    b
}

/// The 112 bits of a block in the order sent.
pub fn block_bits(data: [u8; 4]) -> [u8; BLOCK_BITS] {
    let b = block_bytes(data);
    let mut out = [0u8; BLOCK_BITS];
    for n in 0..BLOCK_BITS {
        out[16 * (n % 7) + n / 7] = (b[n / 8] >> (n % 8)) & 1;
    }
    out
}

/// The 14 bytes of a block from its 112 bits in the order sent.
fn deinterleave(bits: &[u8; BLOCK_BITS]) -> [u8; BLOCK_BYTES] {
    let mut b = [0u8; BLOCK_BYTES];
    for n in 0..BLOCK_BITS {
        b[n / 8] |= bits[16 * (n % 7) + n / 7] << (n % 8);
    }
    b
}

/// The bits of a burst of `p` in the order sent, after `lead` bytes 0x55.
pub fn burst_bits(p: &Packet, lead: usize) -> Vec<u8> {
    let mut bits = Vec::with_capacity(8 * (lead + SYNC.len()) + 2 * BLOCK_BITS);
    let msb_first =
        |bits: &mut Vec<u8>, byte: u8| bits.extend((0..8).rev().map(|k| (byte >> k) & 1));
    for _ in 0..lead {
        msb_first(&mut bits, 0x55);
    }
    for b in SYNC {
        msb_first(&mut bits, b);
    }
    bits.extend(block_bits([p.op, p.arg, (p.unit >> 8) as u8, p.unit as u8]));
    if p.double && is_double(p.op) {
        bits.extend(block_bits(p.extra));
    }
    bits
}

/// The audio of a burst of `p` at `rate` with the amplitude `amplitude`, after `lead` bytes 0x55 (7 is usual): each bit a
/// sine of 1200 or 1800 Hz, the phase continuous. Bit `i` ends at sample `round((i + 1) * rate / 1200)`.
pub fn burst_audio(p: &Packet, rate: f64, amplitude: f32, lead: usize) -> Vec<f32> {
    let bits = burst_bits(p, lead);
    let mut out = Vec::with_capacity((bits.len() as f64 * rate / BIT_RATE) as usize + 1);
    let mut phase = 0.0f64;
    let mut before = 0u8;
    for (i, &b) in bits.iter().enumerate() {
        let hz = if b == before { SAME_HZ } else { CHANGE_HZ };
        before = b;
        let end = ((i + 1) as f64 * rate / BIT_RATE).round() as usize;
        while out.len() < end {
            phase = (phase + hz / rate).fract();
            out.push(amplitude * (TAU * phase).sin() as f32);
        }
    }
    out
}

/// One of the eight decoders: its next bit boundary and what it reads.
#[derive(Clone)]
struct Phase {
    next: f64,
    recent: u64,
    /// The decisions of the block being read, `None` while it searches for the sync.
    block: Option<Vec<u8>>,
    /// The bit before the block and the first packet of a double packet.
    before: u8,
    first: Option<Packet>,
}

/// A decoded packet and when its burst ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decoded {
    /// The packet.
    pub packet: Packet,
    /// The signal time of its last bit, s.
    pub end_s: f64,
}

/// The burst decoder: real audio in, packets out. It has no clock recovery: eight decoders run with bit boundaries one eighth
/// of a bit apart, and a burst is reported once.
pub struct BurstDecoder {
    rate: f64,
    corr: ToneCorrelator,
    count: u64,
    phases: Vec<Phase>,
    last: Option<Decoded>,
    check_failures: u64,
}

impl BurstDecoder {
    /// A decoder for audio at `rate`.
    ///
    /// # Errors
    ///
    /// [`TonesError`] if the rate is below 8 kHz.
    pub fn new(rate: f64) -> Result<Self, TonesError> {
        if !(rate >= MIN_RATE_HZ && rate.is_finite()) {
            return Err(TonesError::BadParameter(Param::MdcRate));
        }
        let window = ((rate / BIT_RATE).round() as usize).max(4);
        let corr = ToneCorrelator::new(&[SAME_HZ / rate, CHANGE_HZ / rate], window)
            .map_err(|_| TonesError::BadParameter(Param::MdcRate))?;
        let per_bit = rate / BIT_RATE;
        Ok(BurstDecoder {
            rate,
            corr,
            count: 0,
            phases: (0..PHASES)
                .map(|k| Phase {
                    next: per_bit * (1.0 + k as f64 / PHASES as f64),
                    recent: 0,
                    block: None,
                    before: 1,
                    first: None,
                })
                .collect(),
            last: None,
            check_failures: 0,
        })
    }

    /// Blocks whose check value was wrong.
    pub fn check_failures(&self) -> u64 {
        self.check_failures
    }

    /// Forgets the burst in progress.
    pub fn reset(&mut self) {
        self.corr.reset();
        for p in &mut self.phases {
            p.recent = 0;
            p.block = None;
            p.first = None;
        }
    }

    /// Reports `packet`, unless it is the burst reported last.
    fn report(&mut self, packet: Packet, on: &mut impl FnMut(Decoded)) {
        let end_s = self.count as f64 / self.rate;
        if self.last.is_some_and(|l| {
            l.packet
                == Packet {
                    extra: l.packet.extra,
                    ..packet
                }
                && end_s - l.end_s <= 0.05
        }) {
            return;
        }
        let d = Decoded { packet, end_s };
        self.last = Some(d);
        on(d);
    }

    /// Takes the tone decision `tone` of decoder `k`.
    fn decision(&mut self, k: usize, tone: u8, on: &mut impl FnMut(Decoded)) {
        let p = &mut self.phases[k];
        let Some(block) = p.block.as_mut() else {
            p.recent = ((p.recent << 1) | u64::from(tone)) & 0xFF_FFFF_FFFF;
            if (p.recent ^ SYNC_DECISIONS).count_ones() <= 3 {
                p.block = Some(Vec::with_capacity(BLOCK_BITS));
                p.before = 1;
                p.first = None;
                p.recent = 0;
            }
            return;
        };
        block.push(tone);
        if block.len() < BLOCK_BITS {
            return;
        }
        let mut bits = [0u8; BLOCK_BITS];
        let mut before = p.before;
        for (b, &d) in bits.iter_mut().zip(block.iter()) {
            before ^= d;
            *b = before;
        }
        let bytes = deinterleave(&bits);
        let good = check_value(&bytes[..4]) == u16::from_le_bytes([bytes[4], bytes[5]]);
        let first = p.first.take();
        p.block = None;
        match (first, good) {
            (Some(mut packet), good) => {
                if good {
                    packet.extra.copy_from_slice(&bytes[..4]);
                } else {
                    self.check_failures += 1;
                }
                self.report(packet, on);
            }
            (None, true) => {
                let mut packet =
                    Packet::new(bytes[0], bytes[1], u16::from_be_bytes([bytes[2], bytes[3]]));
                if is_double(packet.op) {
                    packet.double = true;
                    p.first = Some(packet);
                    p.before = before;
                    p.block = Some(Vec::with_capacity(BLOCK_BITS));
                } else {
                    self.report(packet, on);
                }
            }
            (None, false) => self.check_failures += 1,
        }
    }

    /// Decodes `audio` and calls `on` for each packet.
    pub fn process(&mut self, audio: &[f32], mut on: impl FnMut(Decoded)) {
        let per_bit = self.rate / BIT_RATE;
        for &v in audio {
            self.corr.push(Complex32::new(v, 0.0));
            self.count += 1;
            for k in 0..PHASES {
                if (self.count as f64) < self.phases[k].next {
                    continue;
                }
                self.phases[k].next += per_bit;
                let sums = self.corr.sums();
                let tone = u8::from(sums[1].norm_sqr() > sums[0].norm_sqr());
                self.decision(k, tone, &mut on);
            }
        }
    }
}
