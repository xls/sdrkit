//! The parts the test-signal generators share: the PSK modulator for the clean signal and the CCSDS transmit chain.
//! Standard: CCSDS 131.0-B-5 clauses 3, 4, 9 and 10

use crate::psk::Modulation;
use sdrkit_algo_fec::conv::{codes, Encoder};
use sdrkit_algo_fec::rs::Basis;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::SimError;
use sdrkit_core::{Complex32, SignalInfo};
use sdrkit_proto_ccsds::channel::{ChannelCoder, ChannelConfig};
use sdrkit_proto_ccsds::marker;
use std::f64::consts::PI;

/// Why a generator of a test signal could not be made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GenError {
    /// The pulse shaper refused the rate, the symbol rate or the roll-off.
    Sim(SimError),
    /// The output rate is below the least rate of the test signal.
    RateTooLow {
        /// The least rate, samples a second.
        need: f64,
        /// The requested rate.
        got: f64,
    },
}

impl From<SimError> for GenError {
    fn from(e: SimError) -> Self {
        GenError::Sim(e)
    }
}

impl std::fmt::Display for GenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GenError::Sim(e) => e.fmt(f),
            GenError::RateTooLow { need, got } => write!(
                f,
                "the test signal needs at least {need} samples a second, not {got}"
            ),
        }
    }
}

impl std::error::Error for GenError {}

/// The span of the modulator's root raised cosine pulse, symbols.
const SPAN: f64 = 12.0;

/// The modulator's options.
#[derive(Clone, Debug, PartialEq)]
pub struct ModParams {
    /// Output samples per second.
    pub rate: f64,
    /// Symbols per second.
    pub symbol_rate: f64,
    /// Root raised cosine roll-off.
    pub roll_off: f64,
    /// The modulation.
    pub modulation: Modulation,
    /// A code bit 0 is sent as +1 (the geostationary BPSK links), not as -1.
    pub zero_is_plus: bool,
}

impl ModParams {
    /// Options at `rate` for a link.
    pub fn new(rate: f64, symbol_rate: f64, roll_off: f64, modulation: Modulation) -> Self {
        ModParams {
            rate,
            symbol_rate,
            roll_off,
            modulation,
            zero_is_plus: false,
        }
    }
    /// What the clean signal is: its bandwidth, symbol rate and code bit rate.
    pub fn signal_info(&self) -> SignalInfo {
        SignalInfo::new(self.symbol_rate * (1.0 + self.roll_off))
            .with_symbol_rate(self.symbol_rate)
            .with_bit_rate(self.symbol_rate * self.modulation.bits_per_symbol() as f64)
    }
}

/// A source of channel bits (code bits in sending order: for QPSK, I then Q of each symbol).
pub trait BitSource: Send {
    /// Appends the next bits (at least one).
    fn next_bits(&mut self, out: &mut Vec<u8>);
    /// The test pass has ended: the [`Modulator`] sends the bits given and then ends the signal. False for a signal
    /// without end.
    fn ended(&self) -> bool {
        false
    }
}

/// The pass of the test-signal setting `gen_lines`: `Some(lines)`, or `None` for 0, a signal without end.
pub fn pass_lines(gen_lines: i64) -> Option<u64> {
    u64::try_from(gen_lines).ok().filter(|&n| n > 0)
}

/// The PSK modulator: channel bits from a [`BitSource`] to the clean signal at unit mean power.
pub struct Modulator<S: BitSource> {
    p: ModParams,
    source: S,
    shaper: Linear,
    bits: Vec<u8>,
    next_bit: usize,
    symbols: Vec<Complex32>,
    samples: Vec<Complex32>,
    next_sample: usize,
}

impl<S: BitSource> Modulator<S> {
    /// A modulator for `source`.
    ///
    /// # Errors
    ///
    /// [`GenError::Sim`] if the rate, the symbol rate or the roll-off is out of range.
    pub fn new(p: ModParams, source: S) -> Result<Self, GenError> {
        let sps = p.rate / p.symbol_rate;
        let mut shaper = Linear::rrc(p.roll_off, SPAN, sps)?;
        if p.modulation == Modulation::Oqpsk {
            shaper = shaper.with_offset_q();
        }
        Ok(Modulator {
            p,
            source,
            shaper,
            bits: Vec::new(),
            next_bit: 0,
            symbols: Vec::with_capacity(64),
            samples: Vec::new(),
            next_sample: 0,
        })
    }

    /// The bit source.
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }

    /// The options.
    pub fn params(&self) -> &ModParams {
        &self.p
    }

    fn bit(&mut self) -> u8 {
        while self.next_bit >= self.bits.len() {
            self.bits.clear();
            self.next_bit = 0;
            self.source.next_bits(&mut self.bits);
        }
        self.next_bit += 1;
        self.bits[self.next_bit - 1]
    }

    fn level(&self, b: u8) -> f32 {
        if (b == 1) != self.p.zero_is_plus {
            1.0
        } else {
            -1.0
        }
    }

    fn more(&mut self) {
        self.symbols.clear();
        let r = std::f32::consts::FRAC_1_SQRT_2;
        for _ in 0..64 {
            let s = match self.p.modulation {
                Modulation::Bpsk => {
                    let b = self.bit();
                    Complex32::new(self.level(b), 0.0)
                }
                _ => {
                    let a = self.bit();
                    let b = self.bit();
                    Complex32::new(self.level(a) * r, self.level(b) * r)
                }
            };
            self.symbols.push(s);
        }
        self.samples.clear();
        self.next_sample = 0;
        self.shaper.push(&self.symbols, &mut self.samples);
    }

    /// Fills `out` with samples and returns the samples written: fewer than `out` holds once the pass of the source has
    /// ended ([`BitSource::ended`]).
    pub fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut filled = 0;
        while filled < out.len() {
            if self.next_sample >= self.samples.len() {
                if self.next_bit >= self.bits.len() && self.source.ended() {
                    return filled;
                }
                self.more();
                continue;
            }
            let n = (self.samples.len() - self.next_sample).min(out.len() - filled);
            out[filled..filled + n]
                .copy_from_slice(&self.samples[self.next_sample..self.next_sample + n]);
            self.next_sample += n;
            filled += n;
        }
        filled
    }
}

/// The CCSDS transmit chain of one frame: Reed-Solomon with interleave depth 4 in `basis` (or none), the randomiser and the
/// marker. Returns the 1024 bytes of the CADU.
pub fn cadu_bytes(frame: &[u8], basis: Basis, rs: bool) -> Vec<u8> {
    let cfg = if rs {
        ChannelConfig::rs_223(4).with_basis(sdrkit_proto_ccsds::channel::BasisPolicy::Fixed(basis))
    } else {
        ChannelConfig::uncoded(1020)
    };
    let mut out = Vec::with_capacity(1024);
    let len = cfg.frame_len();
    match ChannelCoder::new(cfg) {
        Ok(mut c) if frame.len() >= len => {
            if c.encode(&frame[..len], &mut out).is_err() {
                out.clear();
            }
        }
        _ => {}
    }
    if out.is_empty() {
        out.extend_from_slice(marker::TM.bytes());
        out.resize(1024, 0);
    }
    out
}

/// The coding options of a CCSDS link.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CodeOptions {
    /// NRZ-M on the frame bits before the code.
    pub nrzm: bool,
    /// The second output inverted (CCSDS 131.0-B-5 clause 3.3).
    pub invert_g2: bool,
    /// The convolutional code is on (off: the frame bits are the channel bits).
    pub conv: bool,
}

/// Turns CADU bytes into channel bits: NRZ-M (when on), then the rate 1/2, K = 7 code (when on).
#[derive(Clone, Debug)]
pub struct CcsdsCoder {
    opts: CodeOptions,
    enc: Encoder,
    level: u8,
    frame_bits: Vec<u8>,
}

impl CcsdsCoder {
    /// A coder.
    pub fn new(opts: CodeOptions) -> Self {
        let code = if opts.invert_g2 {
            codes::CCSDS_K7
        } else {
            codes::K7_171_133
        };
        CcsdsCoder {
            opts,
            enc: Encoder::new(code),
            level: 0,
            frame_bits: Vec::new(),
        }
    }
    /// Appends the channel bits of `bytes`.
    pub fn push(&mut self, bytes: &[u8], out: &mut Vec<u8>) {
        self.frame_bits.clear();
        for &b in bytes {
            for k in (0..8).rev() {
                let mut bit = (b >> k) & 1;
                if self.opts.nrzm {
                    self.level ^= bit;
                    bit = self.level;
                }
                self.frame_bits.push(bit);
            }
        }
        if self.opts.conv {
            self.enc.encode(&self.frame_bits, out);
        } else {
            out.extend_from_slice(&self.frame_bits);
        }
    }
}

/// Space packets laid end to end and cut into M_PDU zones, with the first header pointer of each zone.
#[derive(Clone, Debug, Default)]
pub struct ZoneStream {
    buf: std::collections::VecDeque<u8>,
    starts: std::collections::VecDeque<usize>,
    base: usize,
}

impl ZoneStream {
    /// Adds a packet.
    pub fn push(&mut self, packet: &[u8]) {
        self.starts.push_back(self.base + self.buf.len());
        self.buf.extend(packet.iter().copied());
    }
    /// Bytes waiting.
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    /// Nothing waiting.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
    /// The next zone of `zl` bytes and its first header pointer (no packet starts in it: 0x7FF). At least `zl` bytes must wait,
    /// else the zone is padded with zeros.
    pub fn take(&mut self, zl: usize) -> (sdrkit_proto_ccsds::tm::FirstHeader, Vec<u8>) {
        let fhp = self
            .starts
            .iter()
            .find(|&&s| s >= self.base && s < self.base + zl)
            .map_or(sdrkit_proto_ccsds::tm::FirstHeader::None, |&s| {
                sdrkit_proto_ccsds::tm::FirstHeader::At((s - self.base) as u16)
            });
        let n = zl.min(self.buf.len());
        let mut zone: Vec<u8> = self.buf.drain(..n).collect();
        zone.resize(zl, 0);
        self.base += zl;
        while self.starts.front().is_some_and(|&s| s < self.base) {
            self.starts.pop_front();
        }
        (fhp, zone)
    }
}

/// A space packet for the generators: the 6-byte header (version 0, telemetry, no secondary header) and `user` data, with the
/// CRC-16 of the xRIT transport layer appended when `crc`.
pub fn space_packet(apid: u16, flags: u8, seq: u16, user: &[u8], crc: bool) -> Vec<u8> {
    use sdrkit_proto_ccsds::packet::{build, build_with_crc, PrimaryHeader, SeqFlags};
    let h = PrimaryHeader::telemetry(apid & 0x7FF, SeqFlags::from_bits(flags), seq & 0x3FFF);
    let mut out = Vec::with_capacity(user.len() + 8);
    let r = if crc {
        build_with_crc(&h, user, &mut out)
    } else {
        build(&h, user, &mut out)
    };
    if r.is_err() {
        out.clear();
    }
    out
}

/// An AOS frame for the generators with `layout`: the header, a zero insert zone, the first header pointer and the zone.
pub fn aos_frame(
    layout: &sdrkit_proto_ccsds::aos::Layout,
    spacecraft: u8,
    vcid: u8,
    counter: u32,
    first_header: sdrkit_proto_ccsds::tm::FirstHeader,
    zone: &[u8],
) -> Vec<u8> {
    let h = sdrkit_proto_ccsds::aos::PrimaryHeader {
        spacecraft,
        vcid,
        counter: counter & 0xFF_FFFF,
        ..Default::default()
    };
    let mut out = vec![0u8; layout.frame_len];
    let zl = layout.data_zone_len().min(zone.len());
    if layout
        .build(&h, &[], first_header, &zone[..zl], &mut out)
        .is_err()
    {
        out.fill(0);
    }
    out
}

/// The 10-bit value of the test picture of the 137 MHz and L-band stacks at channel `c`, column `x`, line `y`:
/// `(5 x + 61 y + 97 c) mod 1024`. A receiver shows its top 8 bits. Lines differ by 61 counts, so a row at a wrong place fails
/// a comparison.
pub fn test_value(c: u16, x: u32, y: u32) -> u16 {
    ((5 * x + 61 * y + 97 * u32::from(c)) % 1024) as u16
}

/// A test picture value at (x, y) for channel `ch`: smooth gradients, a faint grid and wide rings, different in each channel.
/// The values are smooth, so the JPEG-coded Meteor rows stay about as short as real ones.
pub fn test_pixel(ch: u16, x: u32, y: u32, w: u32) -> u8 {
    let (xf, yf) = (x as f64, y as f64);
    let base = match ch % 3 {
        0 => 30.0 + xf / w as f64 * 180.0,
        1 => 40.0 + (yf * 0.7) % 170.0,
        _ => 128.0 + 70.0 * ((xf + yf) / 300.0 * PI).sin(),
    };
    let grid = if x % 128 < 2 || y % 64 < 2 { 12.0 } else { 0.0 };
    let (cx, cy) = (w as f64 / 2.0, 60.0);
    let r = ((xf - cx).powi(2) + (yf - cy).powi(2)).sqrt();
    let ring = 15.0 * (r / 90.0 * PI).cos();
    (base + grid + ring + ch as f64 * 5.0).clamp(1.0, 255.0) as u8
}
