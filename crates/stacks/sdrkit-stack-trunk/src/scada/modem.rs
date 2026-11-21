//! The SCADA modem (`scada.modem`): two FM front ends at 96 000 samples/s (channel ±8 kHz and ±16 kHz), the audio FSK path at
//! 24 000 samples/s, seven FSK chains side by side (audio FSK 1200 Bd; 2-FSK and 4-FSK at 4800, 9600 and 19 200 bit/s), on
//! each chain the asynchronous framers (8E1, 8O1, 8N1, normal and inverted) and the HDLC framers (NRZ normal and inverted,
//! NRZI), the frame search in each burst, one message from the frames that several chains and framers read, and the
//! locked bursts that gave no frame ("not recognised").
//!
//! Modbus over Serial Line V1.02 (async character format, frame gaps); ISO/IEC 13239 (HDLC). The carriage of async bytes on
//! direct FSK and the 4-FSK dibit order are as the specification assumes them.

use super::frame::{find_ascii, find_frames, whole_burst, Found, Protocol};
use crate::front::NfmFront;
use num_complex::Complex32;
use sdrkit_algo_dsp::approx::atan2;
use sdrkit_algo_dsp::iir::Butterworth4;
use sdrkit_core::{ConfigError, Values};
use sdrkit_graph::{no_values, Decoder, Format, Item, Outbox, Port, Timed};
use sdrkit_util_bits::hdlc::HdlcDeframer;
use std::f64::consts::TAU;

/// The rate of the two front ends, Hz.
pub const WORK: f64 = 96_000.0;
/// The rate of the audio FSK path, Hz.
pub const AUDIO_RATE: f64 = 24_000.0;
/// The audio FSK path mixes down by this, Hz: the centre of both tone pairs.
pub const AUDIO_CENTRE_HZ: f64 = 1700.0;
/// Idle bits that end an async burst.
const BURST_IDLE_BITS: u32 = 28;
/// An idle gap that splits a burst for the search of a damaged frame, bits.
const STRETCH_GAP_BITS: u32 = 14;
/// The most bytes of a burst; the last [`BURST_KEEP`] are kept after a cut.
const BURST_MAX: usize = 1024;
/// The bytes kept after a cut.
const BURST_KEEP: usize = 300;
/// The bits of a locked stretch kept for a "not recognised" row.
const STRETCH_BITS: usize = 4096;
/// A frame read by several chains and framers within this time is one message, s.
const SAME_FRAME_S: f64 = 0.4;
/// A frame waits this long for the readings of other chains and framers, s.
const HOLD_S: f64 = 0.3;
/// A locked stretch is "not recognised" when no frame, good or damaged, came from 0.3 s before it to this long after it, s.
const UNKNOWN_AFTER_S: f64 = 0.5;
/// Status items, every this many seconds.
const STATUS_EVERY_S: f64 = 0.25;

/// The seven chains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainKind {
    /// Audio FSK at 1200 Bd (Bell 202 or V.23).
    Afsk1200,
    /// 2-FSK, 4800 bit/s.
    Fsk2x4800,
    /// 2-FSK, 9600 bit/s.
    Fsk2x9600,
    /// 2-FSK, 19 200 bit/s.
    Fsk2x19200,
    /// 4-FSK, 4800 bit/s.
    Fsk4x4800,
    /// 4-FSK, 9600 bit/s.
    Fsk4x9600,
    /// 4-FSK, 19 200 bit/s.
    Fsk4x19200,
}

/// Which signal a chain reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Input {
    Audio,
    Narrow,
    Wide,
}

impl ChainKind {
    /// Every chain.
    pub const ALL: [ChainKind; 7] = [
        ChainKind::Afsk1200,
        ChainKind::Fsk2x4800,
        ChainKind::Fsk2x9600,
        ChainKind::Fsk2x19200,
        ChainKind::Fsk4x4800,
        ChainKind::Fsk4x9600,
        ChainKind::Fsk4x19200,
    ];

    /// Symbols a second.
    pub fn symbol_rate(self) -> f64 {
        match self {
            ChainKind::Afsk1200 => 1200.0,
            ChainKind::Fsk2x4800 => 4800.0,
            ChainKind::Fsk2x9600 | ChainKind::Fsk4x19200 => 9600.0,
            ChainKind::Fsk2x19200 => 19_200.0,
            ChainKind::Fsk4x4800 => 2400.0,
            ChainKind::Fsk4x9600 => 4800.0,
        }
    }

    /// Bits a symbol.
    pub fn bits(self) -> usize {
        match self {
            ChainKind::Fsk4x4800 | ChainKind::Fsk4x9600 | ChainKind::Fsk4x19200 => 2,
            _ => 1,
        }
    }

    fn input(self) -> Input {
        match self {
            ChainKind::Afsk1200 => Input::Audio,
            ChainKind::Fsk2x4800 | ChainKind::Fsk4x4800 => Input::Narrow,
            _ => Input::Wide,
        }
    }

    /// The start half-swing and its limits, Hz.
    fn half_swing(self) -> (f32, f32, f32) {
        match self {
            ChainKind::Afsk1200 => (450.0, 250.0, 800.0),
            ChainKind::Fsk2x4800 => (2000.0, 800.0, 4500.0),
            ChainKind::Fsk2x9600 => (3000.0, 1200.0, 6000.0),
            ChainKind::Fsk2x19200 => (4800.0, 2000.0, 9000.0),
            ChainKind::Fsk4x4800 => (1800.0, 800.0, 4000.0),
            ChainKind::Fsk4x9600 => (2400.0, 1000.0, 5500.0),
            ChainKind::Fsk4x19200 => (4000.0, 1600.0, 8000.0),
        }
    }

    /// The name of a direct FSK chain ("4-FSK 9600 bit/s (4800 symbols/s)").
    fn direct_name(self) -> &'static str {
        match self {
            ChainKind::Afsk1200 => "AFSK 1200 Bd",
            ChainKind::Fsk2x4800 => "2-FSK 4800 bit/s",
            ChainKind::Fsk2x9600 => "2-FSK 9600 bit/s",
            ChainKind::Fsk2x19200 => "2-FSK 19200 bit/s",
            ChainKind::Fsk4x4800 => "4-FSK 4800 bit/s (2400 symbols/s)",
            ChainKind::Fsk4x9600 => "4-FSK 9600 bit/s (4800 symbols/s)",
            ChainKind::Fsk4x19200 => "4-FSK 19200 bit/s (9600 symbols/s)",
        }
    }
}

/// A character format or HDLC line code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Framing {
    /// 8 data bits, no parity, one stop bit.
    A8N1,
    /// 8 data bits, no parity, two stop bits.
    A8N2,
    /// 8 data bits, even parity.
    A8E1,
    /// 8 data bits, odd parity.
    A8O1,
    /// 7 data bits, even parity (Modbus ASCII).
    A7E1,
    /// 7 data bits, odd parity.
    A7O1,
    /// 7 data bits, no parity, two stop bits.
    A7N2,
    /// HDLC, NRZ.
    HdlcNrz,
    /// HDLC, NRZI.
    HdlcNrzi,
}

impl Framing {
    /// Its name.
    pub fn label(self) -> &'static str {
        match self {
            Framing::A8N1 => "8N1",
            Framing::A8N2 => "8N2",
            Framing::A8E1 => "8E1",
            Framing::A8O1 => "8O1",
            Framing::A7E1 => "7E1",
            Framing::A7O1 => "7O1",
            Framing::A7N2 => "7N2",
            Framing::HdlcNrz => "HDLC NRZ",
            Framing::HdlcNrzi => "HDLC NRZI",
        }
    }

    /// How strict it is: of the framings that read the same frame, the strictest names the link.
    fn strictness(self) -> u8 {
        match self {
            Framing::HdlcNrz | Framing::HdlcNrzi => 4,
            Framing::A8E1 | Framing::A8O1 | Framing::A7E1 | Framing::A7O1 => 3,
            Framing::A8N2 | Framing::A7N2 => 2,
            Framing::A8N1 => 1,
        }
    }
}

/// Parity of an async framer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parity {
    None,
    Even,
    Odd,
}

/// A byte of a burst.
#[derive(Clone, Copy, Debug, Default)]
struct RxByte {
    v: u8,
    /// A framing or parity error.
    bad: bool,
    locked: bool,
    /// Idle bits between the stop bit before it and its start bit.
    idle_before: u32,
    eye: f32,
}

/// What a framer read: a frame and how.
#[derive(Clone, Debug)]
struct Cand {
    t: f64,
    chain: usize,
    framing: Framing,
    inverted: bool,
    found: Found,
    /// The frame came through HDLC and holds neither protocol.
    raw_hdlc: bool,
    eye: f32,
}

/// An async framer: start bit 0, data least significant first, parity, stop bit 1.
struct AsyncFramer {
    parity: Parity,
    inverted: bool,
    /// The bit of the character (0 the first data bit) and its value; `None` while idle.
    at: Option<(u8, u16)>,
    char_locked: bool,
    char_eye: f32,
    idle: u32,
    bytes: Vec<RxByte>,
    values: Vec<u8>,
    found: Vec<Found>,
}

impl AsyncFramer {
    fn new(parity: Parity, inverted: bool) -> AsyncFramer {
        AsyncFramer {
            parity,
            inverted,
            at: None,
            char_locked: false,
            char_eye: 0.0,
            idle: 0,
            bytes: Vec::with_capacity(BURST_MAX),
            values: Vec::with_capacity(BURST_MAX),
            found: Vec::with_capacity(8),
        }
    }

    fn data_bits(&self) -> u8 {
        8
    }

    /// One bit at time `t`.
    fn bit(&mut self, b: u8, t: f64, locked: bool, eye: f32, chain: usize, out: &mut Vec<Cand>) {
        let b = b ^ u8::from(self.inverted);
        let n = self.data_bits();
        let parity_bits = u8::from(self.parity != Parity::None);
        match self.at {
            None => {
                if b == 1 {
                    self.idle = self.idle.saturating_add(1);
                    if self.idle == BURST_IDLE_BITS {
                        self.flush(t, chain, out);
                    }
                } else {
                    self.at = Some((0, 0));
                    self.char_locked = locked;
                    self.char_eye = eye;
                }
            }
            Some((k, v)) => {
                if k < n + parity_bits {
                    self.at = Some((k + 1, v | u16::from(b) << k));
                    self.char_eye = self.char_eye.max(eye);
                    return;
                }
                // The stop bit.
                let data = (v & 0xFF) as u8;
                let ones = (v & 0x1FF).count_ones();
                let parity_ok = match self.parity {
                    Parity::None => true,
                    Parity::Even => ones % 2 == 0,
                    Parity::Odd => ones % 2 == 1,
                };
                self.at = None;
                self.bytes.push(RxByte {
                    v: data,
                    bad: b != 1 || !parity_ok,
                    locked: self.char_locked || locked,
                    idle_before: self.idle,
                    eye: self.char_eye,
                });
                self.idle = 0;
                if self.bytes.len() >= BURST_MAX {
                    self.search(t, chain, out);
                    let cut = self.bytes.len() - BURST_KEEP;
                    self.bytes.drain(..cut);
                }
            }
        }
    }

    /// The chain lost its lock: the burst ends.
    fn lock_lost(&mut self, t: f64, chain: usize, out: &mut Vec<Cand>) {
        self.flush(t, chain, out);
        self.at = None;
    }

    fn framing(&self) -> Framing {
        match self.parity {
            Parity::None => Framing::A8N1,
            Parity::Even => Framing::A8E1,
            Parity::Odd => Framing::A8O1,
        }
    }

    /// Searches the burst for frames and empties it.
    fn flush(&mut self, t: f64, chain: usize, out: &mut Vec<Cand>) {
        self.search(t, chain, out);
        self.bytes.clear();
    }

    /// Searches the burst for frames. A burst in which the chain never locked is not searched.
    fn search(&mut self, t: f64, chain: usize, out: &mut Vec<Cand>) {
        if self.bytes.len() < 4 || !self.bytes.iter().any(|b| b.locked) {
            return;
        }
        self.values.clear();
        self.values.extend(self.bytes.iter().map(|b| b.v));
        self.found.clear();
        find_frames(&self.values, &mut self.found);
        if self.parity == Parity::None {
            find_ascii(&self.values, &mut self.found);
        }
        let mut good = 0;
        let own = self.framing();
        let no_parity = self.parity == Parity::None;
        for f in self.found.drain(..) {
            let bytes = &self.bytes[f.start..f.start + f.len];
            if bytes.iter().any(|b| b.bad) {
                continue;
            }
            // A good frame counts when the chain locked before its end; a damaged one only from a fully locked stretch.
            let counts = if f.crc_ok {
                self.bytes[..f.start + f.len].iter().any(|b| b.locked)
            } else {
                bytes.iter().all(|b| b.locked)
            };
            if !counts {
                continue;
            }
            good += 1;
            let framing = match f.protocol {
                Protocol::ModbusAscii => ascii_framing(bytes),
                _ if no_parity && two_stops(bytes) => Framing::A8N2,
                _ => own,
            };
            let eye = bytes.iter().map(|b| b.eye).sum::<f32>() / bytes.len() as f32;
            out.push(Cand {
                t,
                chain,
                framing,
                inverted: self.inverted,
                found: f,
                raw_hdlc: false,
                eye,
            });
        }
        if good > 0 {
            return;
        }
        // No good frame: the whole burst, then each stretch between idle gaps, as an unknown or damaged Modbus frame.
        let mut starts: [usize; 16] = [0; 16];
        let mut nstarts = 1;
        for (k, b) in self.bytes.iter().enumerate().skip(1) {
            if b.idle_before >= STRETCH_GAP_BITS && nstarts < starts.len() {
                starts[nstarts] = k;
                nstarts += 1;
            }
        }
        let mut pieces: [(usize, usize); 17] = [(0, 0); 17];
        pieces[0] = (0, self.bytes.len());
        for i in 0..nstarts {
            let end = if i + 1 < nstarts {
                starts[i + 1]
            } else {
                self.bytes.len()
            };
            pieces[i + 1] = (starts[i], end);
        }
        let count = if nstarts > 1 { nstarts + 1 } else { 1 };
        for &(a, e) in &pieces[..count] {
            let bytes = &self.bytes[a..e];
            if bytes.iter().any(|b| b.bad) {
                continue;
            }
            let Some(mut f) = whole_burst(&self.values[a..e]) else {
                continue;
            };
            // A damaged frame only from a fully locked stretch; a good one when the chain locked during it.
            let locked = if f.crc_ok {
                bytes.iter().any(|b| b.locked)
            } else {
                bytes.iter().all(|b| b.locked)
            };
            if !locked {
                continue;
            }
            f.start = a;
            let framing = if self.parity == Parity::None && two_stops(bytes) {
                Framing::A8N2
            } else {
                self.framing()
            };
            let eye = bytes.iter().map(|b| b.eye).sum::<f32>() / bytes.len().max(1) as f32;
            out.push(Cand {
                t,
                chain,
                framing,
                inverted: self.inverted,
                found: f,
                raw_hdlc: false,
                eye,
            });
            return;
        }
    }
}

/// Every byte but the last has at least one more idle bit after its stop bit: two stop bits.
fn two_stops(bytes: &[RxByte]) -> bool {
    bytes.len() > 1 && bytes[1..].iter().all(|b| b.idle_before >= 1)
}

/// The character format of a Modbus ASCII frame read as 8N1: bit 7 is the parity bit (even or odd) or the first of two stop
/// bits.
fn ascii_framing(bytes: &[RxByte]) -> Framing {
    let even = bytes.iter().all(|b| b.v.count_ones() % 2 == 0);
    let odd = bytes.iter().all(|b| b.v.count_ones() % 2 == 1);
    if even {
        Framing::A7E1
    } else if odd {
        Framing::A7O1
    } else {
        Framing::A7N2
    }
}

/// An HDLC framer of one line code.
struct HdlcFramer {
    framing: Framing,
    inverted: bool,
    last_level: u8,
    d: HdlcDeframer,
    found: Vec<Found>,
}

impl HdlcFramer {
    fn new(framing: Framing, inverted: bool) -> HdlcFramer {
        HdlcFramer {
            framing,
            inverted,
            last_level: 0,
            d: HdlcDeframer::x25(4, 598),
            found: Vec::with_capacity(4),
        }
    }

    fn bit(
        &mut self,
        level: u8,
        t: f64,
        locked: bool,
        eye: f32,
        chain: usize,
        out: &mut Vec<Cand>,
    ) {
        let b = match self.framing {
            Framing::HdlcNrzi => {
                let b = u8::from(level == self.last_level);
                self.last_level = level;
                b
            }
            _ => level ^ u8::from(self.inverted),
        };
        let Some(end) = self.d.push(b) else {
            return;
        };
        if !end.fcs_ok || !locked {
            return;
        }
        let bytes = self.d.frame();
        self.found.clear();
        find_frames(bytes, &mut self.found);
        let raw = self.found.is_empty();
        if raw {
            self.found.push(Found {
                protocol: Protocol::ModbusRtu,
                start: 0,
                len: bytes.len(),
                bytes: bytes.to_vec(),
                crc_ok: true,
                note: Some("HDLC frame, FCS good, content not Modbus RTU or DNP3".into()),
            });
        }
        for f in self.found.drain(..) {
            out.push(Cand {
                t,
                chain,
                framing: self.framing,
                inverted: self.inverted,
                found: f,
                raw_hdlc: raw,
                eye,
            });
        }
    }
}

/// A locked stretch of a chain.
struct Stretch {
    t0: f64,
    bits: Vec<u8>,
    nbits: usize,
    changes: usize,
    symbols: usize,
    eye_sum: f32,
    last: u8,
    /// The last 8 bits, newest in bit 0, and for each lag of 1 to 8 bits the bits equal to the bit that lag earlier.
    history: u8,
    agree: [u32; 8],
}

/// A locked stretch that may be "not recognised".
#[derive(Clone, Debug)]
struct Unknown {
    t0: f64,
    t1: f64,
    chain: usize,
    eye: f32,
    bits: Vec<u8>,
    nbits: usize,
}

/// What a chain measures on its clean symbols.
#[derive(Clone, Copy, Debug, Default)]
struct Measured {
    centre: f32,
    half: f32,
    /// The audio chain: the mean value of mark and space symbols at the second discriminator.
    mark: f32,
    space: f32,
    have: bool,
}

/// One FSK chain: moving mean, clock, levels, eye, framers.
struct Chain {
    kind: ChainKind,
    index: usize,
    rate: f64,
    step: f64,
    ma: Vec<f32>,
    ma_at: usize,
    sum: f32,
    prev: f32,
    phase: f64,
    pending_cross: Option<f64>,
    last_level: i8,
    centre: f32,
    half: f32,
    window: [f32; 24],
    window_len: usize,
    window_at: usize,
    eye: [f32; 24],
    eye_at: usize,
    eye_sum: f32,
    good_run: u32,
    locked: bool,
    symbols: u64,
    n: u64,
    clean: bool,
    measured: Measured,
    asyncs: Vec<AsyncFramer>,
    hdlcs: Vec<HdlcFramer>,
    stretch: Stretch,
}

impl Chain {
    fn new(kind: ChainKind, index: usize, rate: f64) -> Chain {
        let sps = rate / kind.symbol_rate();
        let frac = if kind.bits() == 2 { 0.5 } else { 0.75 };
        let len = ((frac * sps).round() as usize).max(1);
        let (start, _, _) = kind.half_swing();
        let senses: &[bool] = if kind == ChainKind::Afsk1200 {
            &[false]
        } else {
            &[false, true]
        };
        let mut asyncs = Vec::new();
        for &inv in senses {
            for p in [Parity::Even, Parity::Odd, Parity::None] {
                asyncs.push(AsyncFramer::new(p, inv));
            }
        }
        Chain {
            kind,
            index,
            rate,
            step: kind.symbol_rate() / rate,
            ma: vec![0.0; len],
            ma_at: 0,
            sum: 0.0,
            prev: 0.0,
            phase: 0.0,
            pending_cross: None,
            last_level: 0,
            centre: 0.0,
            half: start,
            window: [0.0; 24],
            window_len: 0,
            window_at: 0,
            eye: [1.0; 24],
            eye_at: 0,
            eye_sum: 24.0,
            good_run: 0,
            locked: false,
            symbols: 0,
            n: 0,
            clean: false,
            measured: Measured::default(),
            asyncs,
            hdlcs: vec![
                HdlcFramer::new(Framing::HdlcNrz, false),
                HdlcFramer::new(Framing::HdlcNrz, true),
                HdlcFramer::new(Framing::HdlcNrzi, false),
            ],
            stretch: Stretch {
                t0: 0.0,
                bits: Vec::with_capacity(STRETCH_BITS / 8),
                nbits: 0,
                changes: 0,
                symbols: 0,
                eye_sum: 0.0,
                last: 0,
                history: 0,
                agree: [0; 8],
            },
        }
    }

    /// The carrier came on: the level window starts again, and the eye error starts at the lock threshold, so the
    /// preamble of the burst alone decides the lock.
    fn carrier_start(&mut self) {
        if self.locked {
            return;
        }
        self.window_len = 0;
        self.window_at = 0;
        self.eye = [0.3; 24];
        self.eye_sum = 0.3 * 24.0;
        self.good_run = 0;
    }

    fn eye_mean(&self) -> f32 {
        self.eye_sum / self.eye.len() as f32
    }

    /// One sample of the chain's input.
    fn sample(&mut self, x: f32, cands: &mut Vec<Cand>, unknowns: &mut Vec<Unknown>) {
        self.sum += x - self.ma[self.ma_at];
        self.ma[self.ma_at] = x;
        self.ma_at = (self.ma_at + 1) % self.ma.len();
        if self.n.is_multiple_of(65_536) {
            // The running sum is summed again now and then, so rounding does not build up.
            self.sum = self.ma.iter().sum();
        }
        self.n += 1;
        let v = self.sum / self.ma.len() as f32;
        let before = self.phase;
        self.phase += self.step;
        let (dp, d) = (self.prev - self.centre, v - self.centre);
        if (dp < 0.0) != (d < 0.0) && dp != d {
            let f = f64::from(dp / (dp - d));
            let mut at = before + f * self.step;
            if at >= 0.5 {
                at -= 1.0;
            }
            // 2-FSK pulls at every crossing; 4-FSK only at a crossing between opposite symbols, applied when the symbol after
            // it is known.
            if self.kind.bits() == 1 {
                let k = if self.locked { 0.85 } else { 0.6 };
                self.phase -= at * (1.0 - k);
            } else {
                self.pending_cross = Some(at);
            }
        }
        if self.phase >= 0.5 {
            let f = ((0.5 - before) / (self.phase - before)).clamp(0.0, 1.0) as f32;
            let value = self.prev + f * (v - self.prev);
            self.phase -= 1.0;
            self.symbol(value, cands, unknowns);
        }
        self.prev = v;
    }

    /// One symbol read at the clock.
    fn symbol(&mut self, v: f32, cands: &mut Vec<Cand>, unknowns: &mut Vec<Unknown>) {
        let (_, lo, hi) = self.kind.half_swing();
        let four = self.kind.bits() == 2;
        let audio = self.kind == ChainKind::Afsk1200;
        // While unlocked, direct FSK takes its levels from the extremes of the last 16 symbols (24 for 4-FSK), or of those
        // since the carrier came on.
        let span = if four { 24 } else { 16 };
        self.window[self.window_at] = v;
        self.window_at = (self.window_at + 1) % span;
        self.window_len = (self.window_len + 1).min(span);
        if !self.locked && !audio && self.window_len >= 4 {
            let w = &self.window[..self.window_len];
            let (mut mn, mut mx) = (f32::INFINITY, f32::NEG_INFINITY);
            for &x in w {
                mn = mn.min(x);
                mx = mx.max(x);
            }
            self.centre = 0.5 * (mx + mn);
            self.half = (0.5 * (mx - mn)).clamp(lo, hi);
        }
        let y = (v - self.centre) / self.half;
        let (level, ideal): (i8, f32) = if four {
            if y > 2.0 / 3.0 {
                (3, 1.0)
            } else if y > 0.0 {
                (1, 1.0 / 3.0)
            } else if y > -2.0 / 3.0 {
                (-1, -1.0 / 3.0)
            } else {
                (-3, -1.0)
            }
        } else if y > 0.0 {
            (1, 1.0)
        } else {
            (-1, -1.0)
        };
        let e = v - (self.centre + self.half * ideal);
        // The audio chain follows its levels from the start. While unlocked it follows only symbols near a level; the
        // others (a gap, noise, the tail of a burst) bring the levels back towards their start, as the tones of both
        // standards lie about ±450 Hz from the centre.
        let near = (e / self.half).abs() < 0.5;
        if self.locked || (audio && near) {
            self.half = (self.half + 0.03 * e * ideal).clamp(lo, hi);
            self.centre += 0.01 * e;
            if audio {
                self.centre = self.centre.clamp(-250.0, 250.0);
            }
        } else if audio {
            let (start, _, _) = self.kind.half_swing();
            self.centre -= 0.05 * self.centre;
            self.half += 0.05 * (start - self.half);
        }
        let scale = if four { self.half / 3.0 } else { self.half };
        let own = (e.abs() / scale).min(2.0);
        self.eye_sum += own - self.eye[self.eye_at];
        self.eye[self.eye_at] = own;
        self.eye_at = (self.eye_at + 1) % self.eye.len();
        self.symbols += 1;
        if self.symbols.is_multiple_of(4096) {
            self.eye_sum = self.eye.iter().sum();
        }
        let eye = self.eye_mean();
        // Lock after more than 32 symbols with a mean eye error under 0.3; the lock is lost when the mean goes over 0.5.
        if eye < 0.3 {
            self.good_run += 1;
        } else {
            self.good_run = 0;
        }
        let t = self.n as f64 / self.rate;
        let was = self.locked;
        if !self.locked && self.good_run > 32 {
            self.locked = true;
            self.stretch.t0 = t;
        } else if self.locked && eye > 0.5 {
            self.locked = false;
        }
        if four {
            if let Some(at) = self.pending_cross.take() {
                if level == -self.last_level {
                    let k = if self.locked { 0.85 } else { 0.6 };
                    self.phase -= at * (1.0 - k);
                }
            }
        }
        self.last_level = level;
        self.clean = self.locked && eye < 0.3 && own < 0.25;
        if self.clean {
            let m = &mut self.measured;
            let a = if m.have { 0.02 } else { 1.0 };
            m.centre += a * (self.centre - m.centre);
            m.half += a * (self.half - m.half);
            if audio {
                if level > 0 {
                    m.space += if m.have {
                        0.02 * (v - m.space)
                    } else {
                        v - m.space
                    };
                } else {
                    m.mark += if m.have {
                        0.02 * (v - m.mark)
                    } else {
                        v - m.mark
                    };
                }
            }
            m.have = true;
        }
        // The bits: higher frequency is 1 on direct FSK; mark (the lower tone) is 1 on audio FSK.
        let (bits, nbits): ([u8; 2], usize) = if four {
            let b = match level {
                3 => [0, 1],
                1 => [0, 0],
                -1 => [1, 0],
                _ => [1, 1],
            };
            (b, 2)
        } else if audio {
            ([u8::from(level < 0), 0], 1)
        } else {
            ([u8::from(level > 0), 0], 1)
        };
        let locked = self.locked;
        let index = self.index;
        for &b in &bits[..nbits] {
            for a in &mut self.asyncs {
                a.bit(b, t, locked, own, index, cands);
            }
            for h in &mut self.hdlcs {
                h.bit(b, t, locked, own, index, cands);
            }
        }
        if locked {
            let s = &mut self.stretch;
            s.symbols += 1;
            s.eye_sum += own;
            for &b in &bits[..nbits] {
                // The bits as sent, higher frequency 1.
                let raw = if audio { b ^ 1 } else { b };
                if s.nbits > 0 && raw != s.last {
                    s.changes += 1;
                }
                s.last = raw;
                for (lag, a) in s.agree.iter_mut().enumerate() {
                    *a += u32::from((s.history >> lag) & 1 == raw);
                }
                s.history = s.history << 1 | raw;
                if s.nbits < STRETCH_BITS {
                    if s.nbits.is_multiple_of(8) {
                        s.bits.push(0);
                    }
                    if raw != 0 {
                        let last = s.bits.len() - 1;
                        s.bits[last] |= 0x80 >> (s.nbits % 8);
                    }
                }
                s.nbits += 1;
            }
        }
        if was && !self.locked {
            for a in &mut self.asyncs {
                a.lock_lost(t, index, cands);
            }
            self.end_stretch(t, unknowns);
        }
    }

    /// Ends the locked stretch: it is kept when it is long, changes often, does not repeat with a period of 8 bits or less
    /// (an audio tone read as direct FSK) and has a good eye.
    fn end_stretch(&mut self, t: f64, unknowns: &mut Vec<Unknown>) {
        let s = &mut self.stretch;
        let need = (s.nbits / 16).max(16);
        let periodic = s.agree.iter().any(|&a| a as f32 >= 0.9 * s.nbits as f32);
        if s.symbols >= 64 && s.changes >= need && !periodic && s.eye_sum / s.symbols as f32 <= 0.35
        {
            unknowns.push(Unknown {
                t0: s.t0,
                t1: t,
                chain: self.index,
                eye: s.eye_sum / s.symbols as f32,
                bits: s.bits.clone(),
                nbits: s.nbits.min(STRETCH_BITS),
            });
        }
        s.bits.clear();
        s.nbits = 0;
        s.changes = 0;
        s.symbols = 0;
        s.eye_sum = 0.0;
        s.history = 0;
        s.agree = [0; 8];
    }
}

/// The link of a frame and what was measured on it.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkText {
    /// The chain and framing ("Bell 202 AFSK 1200 Bd, 8E1").
    pub short: String,
    /// With the tones, the deviation and the carrier offset.
    pub long: String,
    /// The carrier offset, Hz.
    pub offset_hz: f32,
    /// The deviation, Hz (the FM deviation on audio FSK, the outer deviation on 4-FSK).
    pub deviation_hz: f32,
}

/// A frame passed on.
#[derive(Clone, Debug, PartialEq)]
pub struct RxFrame {
    /// The link.
    pub link: LinkText,
    /// The protocol; `None` for an HDLC frame with neither protocol.
    pub protocol: Option<Protocol>,
    /// The frame as the reader takes it.
    pub bytes: Vec<u8>,
    /// Every check holds.
    pub crc_ok: bool,
    /// A note.
    pub note: Option<String>,
    /// -20 log10 of the mean eye error, dB.
    pub quality_db: f32,
}

/// A locked burst that gave no frame.
#[derive(Clone, Debug, PartialEq)]
pub struct RxUnknown {
    /// The link.
    pub link: LinkText,
    /// The bits as sent, higher frequency 1, packed most significant first.
    pub bits: Vec<u8>,
    /// The number of bits.
    pub nbits: usize,
    /// -20 log10 of the mean eye error, dB.
    pub quality_db: f32,
}

/// What the modem measures.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScadaStatus {
    /// A chain holds lock.
    pub locked: bool,
    /// The level of the channel, dBFS.
    pub level_dbfs: f32,
}

/// What the modem passes on.
#[derive(Clone, Debug, PartialEq)]
pub enum ScadaHeard {
    /// A frame.
    Frame(RxFrame),
    /// A burst not recognised.
    Unknown(RxUnknown),
    /// The measurements.
    Status(ScadaStatus),
}

impl Item for ScadaHeard {
    const KIND: &'static str = "scada.heard";
}

/// A frame waiting for the readings of other chains and framers.
struct Pending {
    t: f64,
    due: f64,
    cand: Cand,
}

/// The `scada.modem` decoder.
pub struct ScadaModem {
    wide: NfmFront,
    disc_wide: Vec<f32>,
    narrow_i: Butterworth4,
    narrow_q: Butterworth4,
    last_narrow: Complex32,
    quad: [f32; 4],
    quad_at: usize,
    audio_phase: f64,
    audio_i: Butterworth4,
    audio_q: Butterworth4,
    last_audio: Complex32,
    /// The audio path: the mean and the mean square of the narrow discriminator at 24 kHz over clean audio symbols.
    audio_dc: f32,
    audio_ms: f32,
    audio_have: bool,
    /// The sum, the sum of squares and the count of the narrow discriminator at 24 kHz since the last audio symbol.
    audio_acc: (f32, f32, u32),
    chains: Vec<Chain>,
    /// The power of the wide channel over the last 0.25 ms and the two steps before it.
    power: [f32; 3],
    power_acc: f32,
    power_n: u32,
    n: u64,
    cands: Vec<Cand>,
    unknowns: Vec<Unknown>,
    held_unknowns: Vec<Unknown>,
    pending: Vec<Pending>,
    /// The times of the frames given (good or damaged), newest last.
    framed: Vec<f64>,
    next_status: f64,
}

impl Default for ScadaModem {
    fn default() -> Self {
        let mut chains = Vec::new();
        for (i, k) in ChainKind::ALL.into_iter().enumerate() {
            let rate = if k.input() == Input::Audio {
                AUDIO_RATE
            } else {
                WORK
            };
            chains.push(Chain::new(k, i, rate));
        }
        ScadaModem {
            wide: NfmFront::new(WORK, 16_000.0),
            disc_wide: Vec::new(),
            narrow_i: Butterworth4::lowpass(WORK, 8000.0),
            narrow_q: Butterworth4::lowpass(WORK, 8000.0),
            last_narrow: Complex32::new(0.0, 0.0),
            quad: [0.0; 4],
            quad_at: 0,
            audio_phase: 0.0,
            audio_i: Butterworth4::lowpass(AUDIO_RATE, 1300.0),
            audio_q: Butterworth4::lowpass(AUDIO_RATE, 1300.0),
            last_audio: Complex32::new(0.0, 0.0),
            audio_dc: 0.0,
            audio_ms: 0.0,
            audio_have: false,
            audio_acc: (0.0, 0.0, 0),
            chains,
            power: [0.0; 3],
            power_acc: 0.0,
            power_n: 0,
            n: 0,
            cands: Vec::with_capacity(32),
            unknowns: Vec::with_capacity(8),
            held_unknowns: Vec::with_capacity(8),
            pending: Vec::with_capacity(32),
            framed: Vec::with_capacity(64),
            next_status: 0.0,
        }
    }
}

impl ScadaModem {
    fn now(&self) -> f64 {
        self.n as f64 / WORK
    }

    /// The name of a chain ("Bell 202 AFSK 1200 Bd", "4-FSK 9600 bit/s (4800 symbols/s)"), what it measured as text, the
    /// carrier offset and the deviation.
    fn chain_text(&self, chain: usize) -> (String, String, f32, f32) {
        let c = &self.chains[chain];
        let m = c.measured;
        if c.kind == ChainKind::Afsk1200 {
            let mark = AUDIO_CENTRE_HZ as f32 + m.mark;
            let space = AUDIO_CENTRE_HZ as f32 + m.space;
            let name = if (mark - 1200.0).abs() + (space - 2200.0).abs()
                <= (mark - 1300.0).abs() + (space - 2100.0).abs()
            {
                "Bell 202"
            } else {
                "V.23"
            };
            let dev = (2.0 * (self.audio_ms - self.audio_dc * self.audio_dc))
                .max(0.0)
                .sqrt();
            (
                format!("{name} AFSK 1200 Bd"),
                format!(
                    "tones {mark:.0} / {space:.0} Hz, FM deviation about {:.1} kHz, carrier offset {:+.0} Hz",
                    dev / 1000.0,
                    self.audio_dc
                ),
                self.audio_dc,
                dev,
            )
        } else {
            let outer = if c.kind.bits() == 2 { " (outer)" } else { "" };
            (
                c.kind.direct_name().to_string(),
                format!(
                    "deviation{outer} {:.1} kHz, carrier offset {:+.0} Hz",
                    m.half / 1000.0,
                    m.centre
                ),
                m.centre,
                m.half,
            )
        }
    }

    /// The link text of a chain, a framing (`None` for a burst with no frame) and a sense.
    fn link(&self, chain: usize, framing: Option<Framing>, inverted: bool) -> LinkText {
        let (name, details, offset_hz, deviation_hz) = self.chain_text(chain);
        let sense = if inverted { ", inverted" } else { "" };
        let short = match framing {
            Some(f) => format!("{name}, {}{sense}", f.label()),
            None => name,
        };
        let long = format!("{short}, {details}");
        LinkText {
            short,
            long,
            offset_hz,
            deviation_hz,
        }
    }

    /// Takes the frames that the framers read: the same frame from several is one, named by the strictest framing.
    fn take_cands(&mut self) {
        for c in self.cands.drain(..) {
            let same = self.pending.iter_mut().find(|p| {
                (p.t - c.t).abs() < SAME_FRAME_S
                    && p.cand.found.protocol == c.found.protocol
                    && p.cand.found.bytes == c.found.bytes
            });
            match same {
                Some(p) => {
                    let (a, b) = (c.framing.strictness(), p.cand.framing.strictness());
                    let better_crc = c.found.crc_ok && !p.cand.found.crc_ok;
                    if better_crc || a > b || (a == b && c.eye < p.cand.eye) {
                        p.cand = c;
                    }
                }
                None => {
                    let t = c.t;
                    self.pending.push(Pending {
                        t,
                        due: t + HOLD_S,
                        cand: c,
                    });
                }
            }
        }
    }

    /// Gives the frames whose wait is over, and the stretches that no frame explains.
    fn give(&mut self, now: f64, out: &mut Vec<Timed<ScadaHeard>>) {
        let mut k = 0;
        while k < self.pending.len() {
            if self.pending[k].due > now {
                k += 1;
                continue;
            }
            let p = self.pending.swap_remove(k);
            let c = p.cand;
            if self.framed.len() == self.framed.capacity() {
                self.framed.remove(0);
            }
            self.framed.push(p.t);
            let link = self.link(c.chain, Some(c.framing), c.inverted);
            out.push(Timed {
                time: p.t,
                value: ScadaHeard::Frame(RxFrame {
                    link,
                    protocol: (!c.raw_hdlc).then_some(c.found.protocol),
                    bytes: c.found.bytes,
                    crc_ok: c.found.crc_ok,
                    note: c.found.note,
                    quality_db: quality(c.eye),
                }),
            });
        }
        self.held_unknowns.append(&mut self.unknowns);
        let mut k = 0;
        while k < self.held_unknowns.len() {
            let u = &self.held_unknowns[k];
            if u.t1 + UNKNOWN_AFTER_S > now {
                k += 1;
                continue;
            }
            let (t0, t1) = (u.t0 - 0.3, u.t1 + UNKNOWN_AFTER_S);
            let explained = self.framed.iter().any(|&t| t >= t0 && t <= t1)
                || self.pending.iter().any(|p| p.t >= t0 && p.t <= t1);
            let u = self.held_unknowns.swap_remove(k);
            if explained {
                continue;
            }
            // The best-locked chain of the stretches of the same time.
            let mut best = u;
            let mut j = 0;
            while j < self.held_unknowns.len() {
                let o = &self.held_unknowns[j];
                if o.t0 < best.t1 && o.t1 > best.t0 {
                    let o = self.held_unknowns.swap_remove(j);
                    if o.eye < best.eye {
                        best = o;
                    }
                } else {
                    j += 1;
                }
            }
            let link = self.link(best.chain, None, false);
            out.push(Timed {
                time: best.t1,
                value: ScadaHeard::Unknown(RxUnknown {
                    link,
                    bits: best.bits,
                    nbits: best.nbits,
                    quality_db: quality(best.eye),
                }),
            });
        }
    }

    /// Watches the power of the wide channel in steps of 0.25 ms: a rise of more than 6 dB over 0.5 ms is a carrier coming
    /// on.
    fn carrier_watch(&mut self, p: f32) {
        self.power_acc += p;
        self.power_n += 1;
        if self.power_n < 24 {
            return;
        }
        let now = self.power_acc / 24.0;
        self.power_acc = 0.0;
        self.power_n = 0;
        let before = self.power[0];
        self.power = [self.power[1], self.power[2], now];
        if now > 4.0 * before && now > 0.0 {
            for c in &mut self.chains {
                c.carrier_start();
            }
        }
    }

    /// One 96 kHz sample of the two discriminators.
    fn sample(&mut self, wide: f32, narrow: f32) {
        let ScadaModem {
            chains,
            cands,
            unknowns,
            ..
        } = self;
        for c in chains.iter_mut().skip(1) {
            let x = if c.kind.input() == Input::Narrow {
                narrow
            } else {
                wide
            };
            c.sample(x, cands, unknowns);
        }
        self.quad[self.quad_at] = narrow;
        self.quad_at += 1;
        if self.quad_at == 4 {
            self.quad_at = 0;
            let a = 0.25 * self.quad.iter().sum::<f32>();
            self.audio(a);
        }
        self.n += 1;
    }

    /// One 24 kHz sample of the narrow discriminator: mixed down by 1700 Hz, low-passed at 1300 Hz, a second discriminator.
    fn audio(&mut self, a: f32) {
        let (s, c) = self.audio_phase.sin_cos();
        self.audio_phase = (self.audio_phase - TAU * AUDIO_CENTRE_HZ / AUDIO_RATE) % TAU;
        let z = Complex32::new(
            self.audio_i.process_sample(a * c as f32),
            self.audio_q.process_sample(a * s as f32),
        );
        let d = z * self.last_audio.conj();
        self.last_audio = z;
        let hz = atan2(d.im, d.re) * (AUDIO_RATE / TAU) as f32;
        let chain = &mut self.chains[0];
        let before = chain.symbols;
        chain.sample(hz, &mut self.cands, &mut self.unknowns);
        self.audio_acc.0 += a;
        self.audio_acc.1 += a * a;
        self.audio_acc.2 += 1;
        if chain.symbols != before {
            // The mean and mean square over the symbol just read, kept when the symbol is clean.
            let (sum, sq, n) = std::mem::take(&mut self.audio_acc);
            if chain.clean && n > 0 {
                let (dc, ms) = (sum / n as f32, sq / n as f32);
                let k = if self.audio_have { 0.02 } else { 1.0 };
                self.audio_dc += k * (dc - self.audio_dc);
                self.audio_ms += k * (ms - self.audio_ms);
                self.audio_have = true;
            }
        }
    }
}

/// The quality of an eye error, dB.
fn quality(eye: f32) -> f32 {
    -20.0 * eye.max(1e-3).log10()
}

impl Decoder for ScadaModem {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "scada.modem"
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<ScadaHeard>("out")]
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        *self = ScadaModem::default();
        self.wide.keep_channel(true);
        self.wide.setup(f.rate, f.signal_offset_hz())?;
        Ok(vec![crate::typed_format::<ScadaHeard>(f)])
    }
    fn retune(&mut self, f: &Format) {
        // A new channel: everything half read is dropped.
        let rate = f.rate;
        *self = ScadaModem::default();
        self.wide.keep_channel(true);
        let _ = self.wide.setup(rate, f.signal_offset_hz());
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        let mut disc = std::mem::take(&mut self.disc_wide);
        disc.clear();
        self.wide.run(x, &mut disc);
        let k = (WORK / TAU) as f32;
        for (i, &w) in disc.iter().enumerate() {
            let y = self.wide.channel()[i];
            let z = Complex32::new(
                self.narrow_i.process_sample(y.re),
                self.narrow_q.process_sample(y.im),
            );
            let d = z * self.last_narrow.conj();
            self.last_narrow = z;
            let narrow = atan2(d.im, d.re) * k;
            self.carrier_watch(y.norm_sqr());
            self.sample(w, narrow);
        }
        self.disc_wide = disc;
        self.take_cands();
        let now = self.now();
        let items = out.items::<ScadaHeard>(0);
        self.give(now, items);
        if now >= self.next_status {
            self.next_status = now + STATUS_EVERY_S;
            items.push(Timed {
                time: now,
                value: ScadaHeard::Status(ScadaStatus {
                    locked: self.chains.iter().any(|c| c.locked),
                    level_dbfs: self.wide.level_dbfs(),
                }),
            });
        }
    }
}
