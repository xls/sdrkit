//! The FleetSync burst: FFSK audio tones on an analogue FM voice channel, plain NRZ, bit 1 (mark) at 1200 Hz. No public
//! document gives the frame. This module holds what the specification states: the preamble and the sync word A23E, the
//! plain frame of two 32-bit words with its block check, the FleetSync II frame of four blocks of (15,7) code words, the
//! fleet and unit fields, the test signal and the decoder. Every other bit of the two words is not interpreted.
//!
//! At 1200 bit/s the space tone is 1800 Hz. The maker's manuals name the signalling an MSK data protocol at 1200 or 2400
//! bit/s; at 2400 bit/s this module takes the MSK tone spacing of half the bit rate, a space tone of 2400 Hz. No public
//! document confirms that pair.

use sdrkit_algo_fec::small::SmallCode;
use sdrkit_algo_modem::fsk::ToneCorrelator;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;
use std::sync::OnceLock;

/// The mark tone (bit 1), Hz.
pub const MARK_HZ: f64 = 1200.0;
/// The sync word, sent most significant bit first.
pub const SYNC: u16 = 0xA23E;
/// The preamble bits of the test signal.
pub const PREAMBLE_BITS: usize = 48;
/// The bits after the sync word that are not interpreted.
pub const LEAD_IN_BITS: usize = 4;
/// The bits of a plain frame after the sync word.
pub const PLAIN_BITS: usize = LEAD_IN_BITS + 64;
/// The bits of a FleetSync II frame after the sync word.
pub const FS2_BITS: usize = LEAD_IN_BITS + 4 * 64;
/// The lowest fleet and unit numbers on the wire, and the highest.
pub const FLEET_WIRE: (u16, u16) = (99, 354);
/// See [`FLEET_WIRE`].
pub const UNIT_WIRE: (u16, u16) = (999, 5094);
/// The fleet and unit numbers that the maker's manuals allow a radio to be given.
pub const FLEET_MAKER: (u16, u16) = (100, 349);
/// See [`FLEET_MAKER`].
pub const UNIT_MAKER: (u16, u16) = (1000, 4999);
/// The lowest audio rate of the decoder, Hz.
pub const MIN_RATE_HZ: f64 = 8000.0;
/// The bit clocks of the decoder, one eighth of a bit apart.
const CLOCKS: usize = 8;
/// A second report of the same words within this time is the same burst, s.
const SAME_BURST_S: f64 = 0.030;
/// A decoder is active this long after a burst it reported, s: longer than a FleetSync II frame at 1200 bit/s (0.22 s).
const ACTIVE_HOLD_S: f64 = 0.3;

/// The FleetSync II code word of each nibble: the valid (15,7) word with the smallest value in bits 6 to 0 (the
/// specification's table). Bit 14 is sent first.
pub const FS2_WORDS: [u16; 16] = [
    0x0000, 0x5103, 0x2207, 0x7304, 0x440E, 0x150D, 0x6609, 0x370A, 0x6881, 0x3982, 0x4A86, 0x1B85,
    0x2C8F, 0x7D8C, 0x0E88, 0x5F8B,
];

/// The signalling rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BitRate {
    /// 1200 bit/s, tones 1200 and 1800 Hz.
    B1200,
    /// 2400 bit/s, tones 1200 and 2400 Hz (the tone pair is not confirmed).
    B2400,
}

impl BitRate {
    /// Bits a second.
    pub fn bits_per_s(self) -> f64 {
        match self {
            BitRate::B1200 => 1200.0,
            BitRate::B2400 => 2400.0,
        }
    }
    /// The space tone (bit 0), Hz: the mark tone plus half the bit rate.
    pub fn space_hz(self) -> f64 {
        MARK_HZ + self.bits_per_s() / 2.0
    }
}

/// The frame type of a burst.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// One plain block of 64 bits.
    Plain,
    /// Four blocks of (15,7) code words.
    Fs2,
}

impl Kind {
    /// The name shown.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Plain => "FleetSync",
            Kind::Fs2 => "FleetSync II",
        }
    }
}

/// The plain block check: the (63,48) cyclic code with generator x^15 + x^14 + x^13 + x^11 + x^4 + x^2 + 1, its last check
/// bit inverted, and an even parity bit (64 bits).
fn block_code() -> &'static SmallCode {
    static CODE: OnceLock<SmallCode> = OnceLock::new();
    CODE.get_or_init(SmallCode::mpt1327_64_48)
}

/// The FleetSync II code: the cyclic (15,7) code with x^8 + x^7 + x^6 + x^4 + 1 when the first bit sent is the highest
/// power (x^8 + x^4 + x^2 + x + 1 in the other order).
fn nibble_code() -> &'static SmallCode {
    static CODE: OnceLock<SmallCode> = OnceLock::new();
    CODE.get_or_init(|| SmallCode::cyclic(15, 0x1D1).expect("a valid (15,7) generator"))
}

/// The two 32-bit words of a message, each most significant bit first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Message {
    /// Word 1: bits 31 to 16 not interpreted, the fleet byte, the high byte of the unit.
    pub word1: u32,
    /// Word 2: the low nibble of the unit, bits 27 to 16 not interpreted, the block check.
    pub word2: u32,
}

impl Message {
    /// The message of `fleet` and `unit` with the uninterpreted fields `raw1` (word 1 bits 31 to 16) and `raw2` (word 2
    /// bits 27 to 16), with its block check. `None` when a number is outside the wire range or `raw2` has more than 12 bits.
    pub fn new(fleet: u16, unit: u16, raw1: u16, raw2: u16) -> Option<Message> {
        if !(FLEET_WIRE.0..=FLEET_WIRE.1).contains(&fleet)
            || !(UNIT_WIRE.0..=UNIT_WIRE.1).contains(&unit)
            || raw2 > 0xFFF
        {
            return None;
        }
        let f = u32::from(fleet - FLEET_WIRE.0);
        let u = u32::from(unit - UNIT_WIRE.0);
        let word1 = u32::from(raw1) << 16 | f << 8 | u >> 4;
        let high = (u & 0xF) << 12 | u32::from(raw2);
        Some(Message {
            word1,
            word2: high << 16 | u32::from(check_value(word1, high as u16)),
        })
    }

    /// The fleet number, 99 to 354.
    pub fn fleet(&self) -> u16 {
        ((self.word1 >> 8) & 0xFF) as u16 + FLEET_WIRE.0
    }
    /// The unit number, 999 to 5094.
    pub fn unit(&self) -> u16 {
        (16 * (self.word1 & 0xFF) + (self.word2 >> 28)) as u16 + UNIT_WIRE.0
    }
    /// Word 1 bits 31 to 16, not interpreted.
    pub fn raw1(&self) -> u16 {
        (self.word1 >> 16) as u16
    }
    /// Word 2 bits 27 to 16, not interpreted.
    pub fn raw2(&self) -> u16 {
        ((self.word2 >> 16) & 0xFFF) as u16
    }
    /// The block check carried, word 2 bits 15 to 0.
    pub fn check(&self) -> u16 {
        self.word2 as u16
    }
    /// Whether the block check holds.
    pub fn check_holds(&self) -> bool {
        block_code().syndrome(self.block()) == 0
    }
    /// Whether the fleet and unit are in the ranges that the maker's manuals allow.
    pub fn in_maker_range(&self) -> bool {
        (FLEET_MAKER.0..=FLEET_MAKER.1).contains(&self.fleet())
            && (UNIT_MAKER.0..=UNIT_MAKER.1).contains(&self.unit())
    }
    /// The 64 bits of the plain block, the first sent most significant.
    pub fn block(&self) -> u128 {
        u128::from(self.word1) << 32 | u128::from(self.word2)
    }
    /// The message of the 64 bits `block`, first sent most significant.
    pub fn of_block(block: u64) -> Message {
        Message {
            word1: (block >> 32) as u32,
            word2: block as u32,
        }
    }
}

/// The block check of the 48 data bits: word 1 and `word2_high` (word 2 bits 31 to 16).
pub fn check_value(word1: u32, word2_high: u16) -> u16 {
    let info = u128::from(word1) << 16 | u128::from(word2_high);
    block_code().encode(info) as u16
}

/// The four 64-bit FleetSync II blocks of `m`, first sent most significant. Block `b` carries 16 data bits as four nibbles
/// n0 (bits 15 to 12) to n3; its 16-bit slots are sent in the order n1, n0, n3, n2, each a 0 bit and the code word.
pub fn fs2_blocks(m: &Message) -> [u64; 4] {
    let data = [
        m.word1 >> 16,
        m.word1 & 0xFFFF,
        m.word2 >> 16,
        m.word2 & 0xFFFF,
    ];
    data.map(|d| {
        let n = |i: u32| u64::from(FS2_WORDS[((d >> (12 - 4 * i)) & 0xF) as usize]);
        n(1) << 48 | n(0) << 32 | n(3) << 16 | n(2)
    })
}

/// The message of four received FleetSync II blocks and the number of bits repaired. `None` when a code word has more
/// than one wrong bit, or the plain block check fails on the repaired data.
pub fn fs2_decode(blocks: &[u64; 4]) -> Option<(Message, u32)> {
    let code = nibble_code();
    let mut fixed = 0u32;
    let mut data = [0u32; 4];
    for (d, &b) in data.iter_mut().zip(blocks) {
        let mut nib = [0u32; 4];
        for (slot, i) in [1usize, 0, 3, 2].into_iter().enumerate() {
            let w = (b >> (48 - 16 * slot)) & 0x7FFF;
            let (cw, n) = code.decode(u128::from(w))?;
            if n > 1 {
                return None;
            }
            fixed += u32::from(n);
            nib[i] = ((cw >> 8) & 0xF) as u32;
        }
        *d = nib[0] << 12 | nib[1] << 8 | nib[2] << 4 | nib[3];
    }
    let m = Message {
        word1: data[0] << 16 | data[1],
        word2: data[2] << 16 | data[3],
    };
    m.check_holds().then_some((m, fixed))
}

/// How a test burst is sent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BurstOptions {
    /// The frame type.
    pub kind: Kind,
    /// The signalling rate.
    pub bit_rate: BitRate,
    /// Alternating bits before the sync word, ending with a 0.
    pub preamble_bits: usize,
    /// The tones swapped, as an inverted discriminator gives them.
    pub inverted: bool,
    /// The peak level of the audio, full scale 1.
    pub amplitude: f32,
}

impl Default for BurstOptions {
    fn default() -> Self {
        BurstOptions {
            kind: Kind::Plain,
            bit_rate: BitRate::B1200,
            preamble_bits: PREAMBLE_BITS,
            inverted: false,
            amplitude: 0.5,
        }
    }
}

/// The bits of a burst of `m` in the order sent: the preamble, the sync word, the lead-in 0000, the frame and four mark bits.
pub fn burst_bits(m: &Message, o: &BurstOptions) -> Vec<u8> {
    let p = o.preamble_bits;
    let mut bits = Vec::with_capacity(p + 16 + FS2_BITS + 4);
    bits.extend((0..p).map(|j| ((p - 1 - j) % 2) as u8));
    let mut word = |v: u64, n: u32| bits.extend((0..n).rev().map(|k| ((v >> k) & 1) as u8));
    word(u64::from(SYNC), 16);
    word(0, LEAD_IN_BITS as u32);
    match o.kind {
        Kind::Plain => word(m.block() as u64, 64),
        Kind::Fs2 => fs2_blocks(m).into_iter().for_each(|b| word(b, 64)),
    }
    word(0xF, 4);
    bits
}

/// The audio of a burst of `m` at `rate`: each bit a sine of the mark or the space tone, the phase continuous. Bit `i` ends
/// at sample `round((i + 1) * rate / bit rate)`.
pub fn burst_audio(m: &Message, rate: f64, o: &BurstOptions) -> Vec<f32> {
    bits_audio(&burst_bits(m, o), rate, o)
}

/// The audio of `bits` (one bit per byte) at `rate`, sent at the rate, tone sense and amplitude of `o`.
pub fn bits_audio(bits: &[u8], rate: f64, o: &BurstOptions) -> Vec<f32> {
    let per_bit = rate / o.bit_rate.bits_per_s();
    let mut out = Vec::with_capacity((bits.len() as f64 * per_bit) as usize + 1);
    let mut phase = 0.0f64;
    for (i, &b) in bits.iter().enumerate() {
        let hz = if (b == 1) != o.inverted {
            MARK_HZ
        } else {
            o.bit_rate.space_hz()
        };
        let end = ((i + 1) as f64 * per_bit).round() as usize;
        while out.len() < end {
            phase = (phase + hz / rate).fract();
            out.push(o.amplitude * (TAU * phase).sin() as f32);
        }
    }
    out
}

/// A decoded burst.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decoded {
    /// The two words.
    pub message: Message,
    /// The frame type.
    pub kind: Kind,
    /// The signalling rate.
    pub bit_rate: BitRate,
    /// Bits repaired by the FleetSync II code.
    pub repaired: u32,
    /// The tones came swapped.
    pub inverted: bool,
    /// The signal time of its last bit, s.
    pub end_s: f64,
}

/// A frame being read by one clock.
#[derive(Clone, Copy)]
struct Read {
    inverted: bool,
    /// The signal time of the end of its sync word, s.
    sync_s: f64,
    /// Bits taken after the sync word.
    n: usize,
    /// The bits after the lead-in, the first most significant.
    data: [u64; 4],
}

/// One bit clock: its next decision and what it reads.
#[derive(Clone, Copy)]
struct Clock {
    next: f64,
    /// The last 40 decisions, newest lowest.
    recent: u64,
    read: Option<Read>,
}

/// The burst decoder of one signalling rate: real audio in, bursts out. Eight bit clocks one eighth of a bit apart hunt
/// for the preamble and sync word and read on their own, and a burst is reported once.
pub struct BurstDecoder {
    rate: f64,
    bit_rate: BitRate,
    per_bit: f64,
    corr: ToneCorrelator,
    count: u64,
    clocks: [Clock; CLOCKS],
    last: Option<Decoded>,
    check_failures: u64,
    /// The sync time of the last burst reported, s.
    reported_sync: Option<f64>,
    /// The sync time of a frame that failed, s, not yet counted: a clock of another phase may still read it.
    failed: Option<f64>,
    /// A decoder of another rate is reading or has just read a burst: a frame that fails now is not counted.
    foreign: bool,
}

impl BurstDecoder {
    /// A decoder of `bit_rate` for audio at `rate` Hz. `None` below 8 kHz.
    pub fn new(rate: f64, bit_rate: BitRate) -> Option<BurstDecoder> {
        if !(rate >= MIN_RATE_HZ && rate.is_finite()) {
            return None;
        }
        let per_bit = rate / bit_rate.bits_per_s();
        let window = (per_bit.round() as usize).max(4);
        let corr =
            ToneCorrelator::new(&[MARK_HZ / rate, bit_rate.space_hz() / rate], window).ok()?;
        let mut d = BurstDecoder {
            rate,
            bit_rate,
            per_bit,
            corr,
            count: 0,
            clocks: [Clock {
                next: 0.0,
                recent: 0,
                read: None,
            }; CLOCKS],
            last: None,
            check_failures: 0,
            reported_sync: None,
            failed: None,
            foreign: false,
        };
        d.place_clocks();
        Some(d)
    }

    fn place_clocks(&mut self) {
        let base = self.count as f64;
        for (k, c) in self.clocks.iter_mut().enumerate() {
            *c = Clock {
                next: base + self.per_bit * (1.0 + k as f64 / CLOCKS as f64),
                recent: 0,
                read: None,
            };
        }
    }

    /// The signalling rate.
    pub fn bit_rate(&self) -> BitRate {
        self.bit_rate
    }

    /// Frames found by their preamble and sync word that no clock could read.
    pub fn check_failures(&self) -> u64 {
        self.check_failures
    }

    /// Whether a clock is reading a frame, or a burst was reported less than 0.3 s ago.
    pub fn active(&self) -> bool {
        self.clocks.iter().any(|c| c.read.is_some())
            || self
                .last
                .is_some_and(|l| self.now_s() - l.end_s < ACTIVE_HOLD_S)
    }

    /// Says whether a decoder of another rate on the same audio is [`active`](Self::active). Its bursts can hold a
    /// preamble and sync word at this rate; a frame that fails while it is active is not counted.
    pub fn set_foreign(&mut self, active: bool) {
        self.foreign = active;
    }

    /// Forgets the burst in progress, as at a gap in the audio. A frame that failed is counted, as in [`process`](Self::process).
    pub fn reset(&mut self) {
        if self.failed.take().is_some() && !self.foreign {
            self.check_failures += 1;
        }
        self.corr.reset();
        self.place_clocks();
    }

    fn now_s(&self) -> f64 {
        self.count as f64 / self.rate
    }

    /// Whether the sync times `a` and `b` belong to one frame.
    fn same_frame(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < SAME_BURST_S)
    }

    /// Reports `message` of the frame read `r`, unless it is the burst reported last.
    fn report(
        &mut self,
        message: Message,
        kind: Kind,
        repaired: u32,
        r: &Read,
        on: &mut impl FnMut(Decoded),
    ) {
        let end_s = self.now_s();
        // A failed read of the same frame by a clock of another phase is not a failure.
        if Self::same_frame(self.failed, r.sync_s) {
            self.failed = None;
        }
        self.reported_sync = Some(r.sync_s);
        let inverted = r.inverted;
        if self
            .last
            .is_some_and(|l| l.message == message && end_s - l.end_s < SAME_BURST_S)
        {
            return;
        }
        let d = Decoded {
            message,
            kind,
            bit_rate: self.bit_rate,
            repaired,
            inverted,
            end_s,
        };
        self.last = Some(d);
        on(d);
    }

    /// Takes the decision `bit` (1 for the mark tone) of clock `k`.
    fn decision(&mut self, k: usize, bit: u64, on: &mut impl FnMut(Decoded)) {
        let c = &mut self.clocks[k];
        let Some(r) = c.read.as_mut() else {
            c.recent = ((c.recent << 1) | bit) & 0xFF_FFFF_FFFF;
            let pre = c.recent >> 16;
            let sync = (c.recent & 0xFFFF) as u16;
            let alternating =
                (pre ^ 0xAA_AAAA).count_ones() <= 3 || (pre ^ 0x55_5555).count_ones() <= 3;
            if alternating {
                let inverted = if (sync ^ SYNC).count_ones() <= 1 {
                    Some(false)
                } else if (sync ^ !SYNC).count_ones() <= 1 {
                    Some(true)
                } else {
                    None
                };
                if let Some(inverted) = inverted {
                    c.read = Some(Read {
                        inverted,
                        sync_s: self.count as f64 / self.rate,
                        n: 0,
                        data: [0; 4],
                    });
                    c.recent = 0;
                }
            }
            return;
        };
        let bit = bit ^ u64::from(r.inverted);
        if r.n >= LEAD_IN_BITS {
            let d = r.n - LEAD_IN_BITS;
            r.data[d / 64] |= bit << (63 - d % 64);
        }
        r.n += 1;
        let r = *r;
        if r.n == PLAIN_BITS {
            let m = Message::of_block(r.data[0]);
            if m.check_holds() {
                self.clocks[k].read = None;
                self.report(m, Kind::Plain, 0, &r, on);
            }
        } else if r.n == FS2_BITS {
            self.clocks[k].read = None;
            match fs2_decode(&r.data) {
                Some((m, fixed)) => self.report(m, Kind::Fs2, fixed, &r, on),
                None => {
                    let same = Self::same_frame(self.failed, r.sync_s)
                        || Self::same_frame(self.reported_sync, r.sync_s);
                    if !same {
                        if self.failed.is_some() {
                            self.check_failures += 1;
                        }
                        self.failed = Some(r.sync_s);
                    }
                }
            }
        }
    }

    /// Decodes `audio` and calls `on` for each burst.
    pub fn process(&mut self, audio: &[f32], mut on: impl FnMut(Decoded)) {
        // Every clock has read a frame this long after its sync.
        let settled_s = FS2_BITS as f64 / self.bit_rate.bits_per_s() + SAME_BURST_S;
        for &v in audio {
            self.corr.push(Complex32::new(v, 0.0));
            self.count += 1;
            let at = self.count as f64;
            if self.failed.is_some_and(|f| at / self.rate - f > settled_s) {
                self.failed = None;
                if !self.foreign {
                    self.check_failures += 1;
                }
            }
            for k in 0..CLOCKS {
                if at < self.clocks[k].next {
                    continue;
                }
                self.clocks[k].next += self.per_bit;
                let s = self.corr.sums();
                let bit = u64::from(s[0].norm_sqr() > s[1].norm_sqr());
                self.decision(k, bit, &mut on);
            }
        }
    }
}
