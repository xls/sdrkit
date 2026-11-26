//! The IEEE 802.11b DSSS PHY at 1 and 2 Mbit/s (IEEE Std 802.11-2020 clause 16, with the HR/DSSS header rules of clause 17 of
//! 802.11-2007, now clause 16.3 of 802.11-2020).
//!
//! - Spreading: each symbol (1 µs) is the 11-chip Barker sequence (`sdrkit_algo_modem::dsss::BARKER_11`, clause 16.4.6.3).
//! - Modulation: DBPSK at 1 Mbit/s and DQPSK at 2 Mbit/s (clause 16.4.6.4, table 16-2 and 16-3). Phase changes are
//!   counterclockwise.
//! - Scrambler: the self-synchronising scrambler 1 + z^-4 + z^-7 (`sdrkit_util_bits::seq::ieee80211b`, clause 16.2.4).
//! - PLCP: the long preamble (128 scrambled ones, SFD 0xF3A0) and the short preamble (56 scrambled zeros, SFD 0x05CF), and the
//!   48-bit header (SIGNAL, SERVICE, LENGTH, CRC-16) sent least significant bit first (clause 16.2.2, 16.2.3).
//! - CRC-16: CRC-16/GENIBUS over the 32 header bits in the order sent, the result sent most significant bit first (clause
//!   16.2.3.7). The engine is `sdrkit_util_bits::crc::catalog::CRC16_GENIBUS`.
//!
//! The header rules cover all four rates, CCK included. The receiver decodes DBPSK and DQPSK PSDUs. It counts CCK headers
//! (5.5 and 11 Mbit/s) in [`DsssStats::other_rate`] but does not decode their PSDU.
//!
//! ```
//! use sdrkit_proto_wlan_phy::dsss::{self, PlcpHeader, Preamble, Rate};
//! let h = PlcpHeader::for_psdu(Rate::Cck11, 1000).unwrap();
//! assert_eq!(h.length_us, 728); // ceil(8000 / 11)
//! assert_eq!(h.psdu_octets(), Some(1000));
//! let mut bits = Vec::new();
//! dsss::ppdu_bits(Preamble::Long, &PlcpHeader::for_psdu(Rate::Dbpsk1, 3).unwrap(), &[1, 2, 3], &mut bits).unwrap();
//! assert_eq!(bits.len(), 128 + 16 + 48 + 24);
//! ```

use crate::{finite, CheckKind, Complex32, Error, Malformation, Param, Result};
use sdrkit_algo_dsp::fir::Fir;
pub use sdrkit_algo_modem::dsss::BARKER_11;
use sdrkit_util_bits::crc::catalog::CRC16_GENIBUS;
use sdrkit_util_bits::lfsr::Multiplicative;
use sdrkit_util_bits::{pack, seq, BitOrder, Pad};

/// The SFD of the long preamble (clause 16.2.3.3), sent least significant bit first.
pub const SFD_LONG: u16 = 0xF3A0;
/// The SFD of the short preamble (clause 16.2.3.3): the long SFD sent in time-reversed order.
pub const SFD_SHORT: u16 = 0x05CF;
/// The chip rate, Hz.
pub const CHIP_RATE: f64 = 11e6;
/// The longest PSDU the LENGTH field can describe at 1 Mbit/s, octets (65535 µs).
pub const MAX_PSDU: usize = 4095;
/// The scrambler start state of a long-preamble transmitter (clause 16.2.4): 1101100, the register's stage 1 in bit 0.
pub const SCRAMBLER_SEED_LONG: u8 = 0b110_1100;
/// The scrambler start state of a short-preamble transmitter (clause 16.2.4): 0011011.
pub const SCRAMBLER_SEED_SHORT: u8 = 0b001_1011;

/// The data rate of a PSDU, from the SIGNAL field (clause 16.2.3.4, rate in units of 100 kbit/s).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rate {
    /// 1 Mbit/s DBPSK (SIGNAL 0x0A).
    Dbpsk1,
    /// 2 Mbit/s DQPSK (SIGNAL 0x14).
    Dqpsk2,
    /// 5.5 Mbit/s CCK (SIGNAL 0x37).
    Cck5_5,
    /// 11 Mbit/s CCK (SIGNAL 0x6E).
    Cck11,
}

impl Rate {
    /// The SIGNAL field value.
    pub fn signal(self) -> u8 {
        match self {
            Rate::Dbpsk1 => 0x0A,
            Rate::Dqpsk2 => 0x14,
            Rate::Cck5_5 => 0x37,
            Rate::Cck11 => 0x6E,
        }
    }

    /// The rate of a SIGNAL field value.
    pub fn from_signal(v: u8) -> Option<Rate> {
        Some(match v {
            0x0A => Rate::Dbpsk1,
            0x14 => Rate::Dqpsk2,
            0x37 => Rate::Cck5_5,
            0x6E => Rate::Cck11,
            _ => return None,
        })
    }

    /// The rate in units of 100 kbit/s.
    pub fn units_100k(self) -> u32 {
        self.signal() as u32
    }

    /// Bits per symbol of the Barker rates (1 or 2). `None` for CCK.
    pub fn barker_bits(self) -> Option<usize> {
        match self {
            Rate::Dbpsk1 => Some(1),
            Rate::Dqpsk2 => Some(2),
            _ => None,
        }
    }
}

/// The preamble format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Preamble {
    /// The long preamble: SYNC of 128 scrambled ones, the long SFD, and the header at 1 Mbit/s.
    Long,
    /// The short preamble: SYNC of 56 scrambled zeros, the short SFD, and the header at 2 Mbit/s.
    Short,
}

/// The SERVICE bit that extends LENGTH at 11 Mbit/s (bit 7, clause 16.3.3.6).
pub const SERVICE_LENGTH_EXTENSION: u8 = 0x80;
/// The SERVICE bit that says the transmit clocks are locked (bit 2).
pub const SERVICE_LOCKED_CLOCKS: u8 = 0x04;

/// The PLCP header (clause 16.2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlcpHeader {
    /// The PSDU rate.
    pub rate: Rate,
    /// The SERVICE field.
    pub service: u8,
    /// The LENGTH field: the time to send the PSDU, µs.
    pub length_us: u16,
}

impl PlcpHeader {
    /// The header for a PSDU of `octets` at `rate` (clause 16.3.3.6). At 11 Mbit/s the length extension bit is set when the
    /// rounding of LENGTH added 8/11 µs or more. At CCK rates the locked-clocks bit is set. Errors when LENGTH does not fit in
    /// 16 bits.
    pub fn for_psdu(rate: Rate, octets: usize) -> Result<PlcpHeader> {
        let r = rate.units_100k() as u64;
        // LENGTH = ceil(octets * 8 / (r / 10)) = ceil(octets * 80 / r).
        let num = octets as u64 * 80;
        let length = num.div_ceil(r);
        if length > u16::MAX as u64 {
            return Err(Error::TooLong);
        }
        let mut service = 0;
        if matches!(rate, Rate::Cck5_5 | Rate::Cck11) {
            service |= SERVICE_LOCKED_CLOCKS;
        }
        // Extension when LENGTH - octets * 8 / 11 >= 8 / 11, that is 11 * LENGTH - 8 * octets >= 8.
        if rate == Rate::Cck11 && 11 * length - 8 * octets as u64 >= 8 {
            service |= SERVICE_LENGTH_EXTENSION;
        }
        Ok(PlcpHeader {
            rate,
            service,
            length_us: length as u16,
        })
    }

    /// The PSDU length in octets, computed from LENGTH and at 11 Mbit/s the extension bit (clause 16.3.4.3). `None` when LENGTH is
    /// not a whole number of octets at 1 or 2 Mbit/s.
    pub fn psdu_octets(&self) -> Option<usize> {
        let l = self.length_us as u64;
        match self.rate {
            Rate::Dbpsk1 => l.is_multiple_of(8).then_some(l as usize / 8),
            Rate::Dqpsk2 => l.is_multiple_of(4).then_some(l as usize / 4),
            Rate::Cck5_5 => Some((l * 11 / 16) as usize),
            Rate::Cck11 => {
                let ext = (self.service & SERVICE_LENGTH_EXTENSION != 0) as u64;
                (l * 11 / 8).checked_sub(ext).map(|v| v as usize)
            }
        }
    }

    /// Appends the 48 header bits in the order sent: SIGNAL, SERVICE, LENGTH (each least significant bit first), then the CRC-16
    /// most significant bit first.
    pub fn push_bits(&self, out: &mut Vec<u8>) {
        let start = out.len();
        pack::push_u64_lsb(out, self.rate.signal() as u64, 8);
        pack::push_u64_lsb(out, self.service as u64, 8);
        pack::push_u64_lsb(out, self.length_us as u64, 16);
        let crc = CRC16_GENIBUS.bits(&out[start..]);
        pack::push_u64(out, crc as u64, 16);
    }

    /// Reads 48 header bits (as sent). Errors for a bad CRC or an unknown SIGNAL value.
    pub fn from_bits(bits: &[u8]) -> Result<PlcpHeader> {
        if bits.len() < 48 {
            return Err(Error::InputTooShort);
        }
        if !CRC16_GENIBUS.verify_bits(&bits[..48]) {
            return Err(Error::Check(CheckKind::PlcpCrc));
        }
        let signal = pack::to_u64_lsb(&bits[..8]) as u8;
        let rate =
            Rate::from_signal(signal).ok_or(Error::Malformed(Malformation::UnknownSignal))?;
        Ok(PlcpHeader {
            rate,
            service: pack::to_u64_lsb(&bits[8..16]) as u8,
            length_us: pack::to_u64_lsb(&bits[16..32]) as u16,
        })
    }
}

/// Appends the PPDU bits before scrambling: SYNC, SFD, the header, then the PSDU least significant bit first. Errors when the
/// PSDU length does not match the header.
pub fn ppdu_bits(
    preamble: Preamble,
    header: &PlcpHeader,
    psdu: &[u8],
    out: &mut Vec<u8>,
) -> Result<()> {
    if header.psdu_octets() != Some(psdu.len()) {
        return Err(Error::BadParameter(Param::PsduLengthMismatch));
    }
    match preamble {
        Preamble::Long => {
            out.extend(std::iter::repeat_n(1u8, 128));
            pack::push_u64_lsb(out, SFD_LONG as u64, 16);
        }
        Preamble::Short => {
            out.extend(std::iter::repeat_n(0u8, 56));
            pack::push_u64_lsb(out, SFD_SHORT as u64, 16);
        }
    }
    header.push_bits(out);
    pack::unpack(psdu, BitOrder::Lsb, out);
    Ok(())
}

/// Scrambles PPDU bits in place with the transmitter start state of the preamble format.
pub fn scramble(bits: &mut [u8], preamble: Preamble) {
    let seed = match preamble {
        Preamble::Long => SCRAMBLER_SEED_LONG,
        Preamble::Short => SCRAMBLER_SEED_SHORT,
    };
    seq::ieee80211b(seed).scramble(bits);
}

/// The phase change, in quarter turns, of a DQPSK dibit `(d0, d1)` (table 16-3): 00 → 0, 01 → 1, 11 → 2, 10 → 3.
pub fn dqpsk_quarters(d0: u8, d1: u8) -> u8 {
    match (d0 & 1, d1 & 1) {
        (0, 0) => 0,
        (0, 1) => 1,
        (1, 1) => 2,
        _ => 3,
    }
}

/// Appends the symbols (unit phasors) of scrambled PPDU bits: the preamble and header at their rates, then the PSDU at `rate`.
/// The first symbol has phase 0. Errors for a CCK rate.
pub fn symbols(
    scrambled: &[u8],
    preamble: Preamble,
    rate: Rate,
    out: &mut Vec<Complex32>,
) -> Result<()> {
    let psdu_bits = rate
        .barker_bits()
        .ok_or(Error::BadParameter(Param::CckNotBuilt))?;
    let (dbpsk_len, header_bits) = match preamble {
        Preamble::Long => (128 + 16 + 48, 1),
        Preamble::Short => (56 + 16, 2),
    };
    const Q: [Complex32; 4] = [
        Complex32::new(1.0, 0.0),
        Complex32::new(0.0, 1.0),
        Complex32::new(-1.0, 0.0),
        Complex32::new(0.0, -1.0),
    ];
    let mut phase = 0u8;
    let mut i = 0;
    let mut first = true;
    while i < scrambled.len() {
        let per = if i < dbpsk_len {
            1
        } else if i < dbpsk_len + if header_bits == 2 { 48 } else { 0 } {
            2
        } else {
            psdu_bits
        };
        let q = if per == 1 {
            2 * (scrambled[i] & 1)
        } else {
            dqpsk_quarters(scrambled[i], *scrambled.get(i + 1).unwrap_or(&0))
        };
        if !first {
            phase = (phase + q) & 3;
        } else {
            first = false;
        }
        out.push(Q[phase as usize]);
        i += per;
    }
    Ok(())
}

/// Appends the 11 Barker chips of each symbol (at 11 Mchip/s).
pub fn spread(symbols: &[Complex32], out: &mut Vec<Complex32>) {
    for &s in symbols {
        out.extend(BARKER_11.iter().map(|&c| s * c as f32));
    }
}

/// The DSSS receiver's counters since it was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DsssStats {
    /// SFDs found (long and short).
    pub sfds: u64,
    /// Headers with a good CRC.
    pub headers_ok: u64,
    /// Headers with a bad CRC or a length that does not fit.
    pub headers_bad: u64,
    /// Good headers at a CCK rate (not decoded).
    pub other_rate: u64,
    /// PSDUs with a good FCS.
    pub frames: u64,
    /// PSDUs with a bad FCS.
    pub bad_fcs: u64,
}

/// A PSDU whose FCS is good. It borrows the receiver's buffer for the time of the callback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DsssFrame<'a> {
    /// The MPDU without its FCS.
    pub mpdu: &'a [u8],
    /// The header.
    pub header: PlcpHeader,
    /// The preamble format.
    pub preamble: Preamble,
    /// The input sample index of the last SFD symbol.
    pub sample: u64,
    /// The carrier offset from the mean phase step, Hz.
    pub offset_hz: f64,
    /// The signal-to-noise ratio from the spread of the phase steps, dB.
    pub snr_db: f64,
    /// The mean power of the despread symbols (full scale 1).
    pub power: f64,
    /// The power of the signal over the 32 symbols that end with the SFD, dBFS (dB relative to full scale): the input power
    /// less the noise that the spread of the phase steps shows.
    pub level_dbfs: f64,
}

#[derive(Clone, Copy, Debug)]
struct Phase {
    /// The received bits before the descrambler, the newest in bit 0.
    raw: u64,
    /// The descrambled bits, the newest in bit 0.
    hist: u64,
    energy: f32,
}

#[derive(Clone, Copy, Debug)]
struct Hit {
    n: u64,
    energy: f32,
    descr: Multiplicative,
    preamble: Preamble,
}

#[derive(Clone, Debug)]
struct Dec {
    active: bool,
    t: u64,
    start: u64,
    prev: Complex32,
    descr: Multiplicative,
    acc: f32,
    preamble: Preamble,
    header: Option<PlcpHeader>,
    per: usize,
    need: usize,
    bits: Vec<u8>,
    perr_sum: f64,
    perr_sq: f64,
    pow: f64,
    nsym: u64,
    /// The mean input power `|x|^2` over [`LEVEL_SYMBOLS`] symbols up to the SFD.
    level: f64,
}

const RING: usize = 1 << 12;
/// Symbols up to the end of the SFD that give the level of a frame. At 64 samples per symbol they fit in the ring.
const LEVEL_SYMBOLS: u64 = 32;
const DECODERS: usize = 4;
/// Samples that the matched filter takes in one call.
const BLOCK: usize = 256;

/// Writes `src` (at most [`RING`] values) into `ring` from the place of sample `at` on.
fn ring_write(ring: &mut [Complex32; RING], at: usize, src: &[Complex32]) {
    let at = at & (RING - 1);
    let head = src.len().min(RING - at);
    ring[at..at + head].copy_from_slice(&src[..head]);
    ring[..src.len() - head].copy_from_slice(&src[head..]);
}

/// Reads `dst.len()` values (at most [`RING`]) of `ring` from the place of sample `at` on.
fn ring_read(ring: &[Complex32; RING], at: usize, dst: &mut [Complex32]) {
    let at = at & (RING - 1);
    let head = dst.len().min(RING - at);
    let rest = dst.len() - head;
    dst[..head].copy_from_slice(&ring[at..at + head]);
    dst[head..].copy_from_slice(&ring[..rest]);
}

/// The response of the descrambler to one received 1 bit, the first output in bit 0. The descrambler has no feedback, so an
/// output bit is the parity of the received bits at these delays.
fn descrambler_delays() -> u64 {
    let mut bits = [0u8; 64];
    bits[0] = 1;
    seq::ieee80211b(0).descramble(&mut bits);
    bits.iter()
        .enumerate()
        .fold(0, |m, (k, &b)| m | (b as u64) << k)
}

/// The 802.11b receiver on input at a whole number of samples per symbol (11 to 64: 20 for a 20 Msps capture). It despreads
/// with a Barker matched filter at every sample, runs one differential detector and descrambler per sample phase to find the
/// SFD, then follows each frame with an early-late timing tracker.
#[derive(Clone, Debug)]
pub struct DsssReceiver {
    sps: usize,
    /// The Barker matched filter.
    matched: Fir<Complex32>,
    x: Box<[Complex32; RING]>,
    c: Box<[Complex32; RING]>,
    n: u64,
    phases: Vec<Phase>,
    /// The sample phase of the next sample: `n` modulo `sps`.
    phase: usize,
    /// The delays of the descrambler: bit `k` is set when a received bit changes the output `k` bits later.
    delays: u64,
    group: Option<Hit>,
    decs: Vec<Dec>,
    /// The count of samples at which a frame in progress has its next symbol; `u64::MAX` with no frame in progress.
    wake: u64,
    stats: DsssStats,
    sfd_long: u64,
    sfd_short: u64,
    bytes: Vec<u8>,
}

impl DsssReceiver {
    /// A receiver for `sps` samples per symbol (11 to 64). Errors for another value.
    pub fn new(sps: usize) -> Result<DsssReceiver> {
        if !(11..=64).contains(&sps) {
            return Err(Error::BadParameter(Param::DsssSamplesPerSymbol));
        }
        // The matched filter: chip m of the Barker sequence covers the samples whose centre lies in it.
        let taps: Vec<f32> = (0..sps)
            .rev()
            .map(|i| BARKER_11[((2 * i + 1) * 11) / (2 * sps)] as f32 / sps as f32)
            .collect();
        let phase = Phase {
            raw: 0,
            hist: 0,
            energy: 0.0,
        };
        let dec = Dec {
            active: false,
            t: 0,
            start: 0,
            prev: Complex32::default(),
            descr: seq::ieee80211b(0),
            acc: 0.0,
            preamble: Preamble::Long,
            header: None,
            per: 1,
            need: 48,
            bits: Vec::with_capacity(48 + 8 * MAX_PSDU),
            perr_sum: 0.0,
            perr_sq: 0.0,
            pow: 0.0,
            nsym: 0,
            level: 0.0,
        };
        Ok(DsssReceiver {
            sps,
            matched: Fir::new(&taps),
            x: Box::new([Complex32::default(); RING]),
            c: Box::new([Complex32::default(); RING]),
            n: 0,
            phases: vec![phase; sps],
            phase: 0,
            delays: descrambler_delays(),
            group: None,
            decs: vec![dec; DECODERS],
            wake: u64::MAX,
            stats: DsssStats::default(),
            sfd_long: (SFD_LONG.reverse_bits()) as u64,
            sfd_short: (SFD_SHORT.reverse_bits()) as u64,
            bytes: Vec::with_capacity(MAX_PSDU),
        })
    }

    /// The counters.
    pub fn stats(&self) -> DsssStats {
        self.stats
    }

    /// Forgets the signal and every frame in progress. The counters stay.
    pub fn reset(&mut self) {
        self.matched.reset();
        self.x.fill(Complex32::default());
        self.c.fill(Complex32::default());
        for p in &mut self.phases {
            p.hist = 0;
            p.energy = 0.0;
        }
        for d in &mut self.decs {
            d.active = false;
        }
        self.wake = u64::MAX;
        self.group = None;
    }

    #[inline]
    fn at(&self, n: u64) -> Complex32 {
        self.c[(n as usize) & (RING - 1)]
    }

    /// Runs the samples `x`. The receiver calls `on_frame` for each PSDU with a good FCS.
    pub fn process(&mut self, x: &[Complex32], mut on_frame: impl FnMut(&DsssFrame<'_>)) {
        let mut input = [Complex32::default(); BLOCK];
        let mut despread = [Complex32::default(); BLOCK];
        let mut earlier = [Complex32::default(); BLOCK];
        let mut flips = [0u8; BLOCK];
        let mut powers = [0f32; BLOCK];
        for block in x.chunks(BLOCK) {
            let k = block.len();
            for (v, &s) in input.iter_mut().zip(block) {
                *v = finite(s);
            }
            self.matched.process(&input[..k], &mut despread[..k]);
            // The whole block goes into the rings first; a frame in progress reads only up to its own sample.
            let first = self.n as usize;
            ring_write(&mut self.x, first, &input[..k]);
            ring_write(&mut self.c, first, &despread[..k]);
            // The differential decision against the output one symbol earlier, and the power.
            ring_read(&self.c, first + RING - self.sps, &mut earlier[..k]);
            for (((f, p), c), prev) in flips
                .iter_mut()
                .zip(&mut powers)
                .zip(&despread[..k])
                .zip(&earlier)
            {
                *f = ((c * prev.conj()).re < 0.0) as u8;
                *p = c.norm_sqr();
            }
            self.search(&flips[..k], &powers[..k], &mut on_frame);
        }
    }

    /// Takes the differential decisions `flips` and the powers `powers` of the next outputs of the matched filter, which the
    /// rings already hold: searches each sample phase for an SFD and runs the frames in progress.
    fn search(&mut self, flips: &[u8], powers: &[f32], on_frame: &mut impl FnMut(&DsssFrame<'_>)) {
        let sps = self.sps as u64;
        let (delays, sfd_long, sfd_short) = (self.delays, self.sfd_long, self.sfd_short);
        // The counters stay in registers over the block; a frame in progress reads `self.n`.
        let mut next = self.n;
        let mut phase = self.phase;
        for (&raw, &power) in flips.iter().zip(powers) {
            let n = next;
            next += 1;
            let ph = &mut self.phases[phase];
            phase = if phase + 1 == self.sps { 0 } else { phase + 1 };
            if n >= sps {
                ph.raw = (ph.raw << 1) | raw as u64;
                let bit = (ph.raw & delays).count_ones() as u64 & 1;
                ph.hist = (ph.hist << 1) | bit;
                ph.energy = 0.9 * ph.energy + 0.1 * power;
                let h = ph.hist;
                let sfd = h & 0xFFFF;
                if sfd == sfd_long || sfd == sfd_short {
                    let sync = (h >> 16) & 0xFFFF_FFFF;
                    let pre = if sfd == sfd_long && sync.count_ones() >= 31 {
                        Some(Preamble::Long)
                    } else if sfd == sfd_short && sync.count_ones() <= 1 {
                        Some(Preamble::Short)
                    } else {
                        None
                    };
                    if let Some(preamble) = pre {
                        let hit = Hit {
                            n,
                            energy: ph.energy,
                            descr: seq::ieee80211b((ph.raw & 0x7F) as u8),
                            preamble,
                        };
                        self.group = Some(match self.group {
                            Some(g) if n <= g.n + sps + 4 => {
                                if hit.energy > g.energy {
                                    hit
                                } else {
                                    g
                                }
                            }
                            Some(g) => {
                                self.start(g);
                                hit
                            }
                            None => hit,
                        });
                    }
                }
            }
            if let Some(g) = self.group {
                if n > g.n + sps + 4 {
                    self.group = None;
                    self.start(g);
                }
            }
            if next >= self.wake {
                self.n = next;
                self.advance(on_frame);
            }
        }
        self.n = next;
        self.phase = phase;
    }

    fn start(&mut self, h: Hit) {
        self.stats.sfds += 1;
        let sps = self.sps as u64;
        if self
            .decs
            .iter()
            .any(|d| d.active && d.start.abs_diff(h.n) < 100 * sps)
        {
            return;
        }
        let prev = self.at(h.n);
        // A DSSS signal fills the band of the chain (its main lobe is 22 MHz wide): the input power holds the signal and the
        // noise of the band. The frame's end takes the noise out.
        let span = (LEVEL_SYMBOLS * sps).min(h.n + 1);
        let level = (h.n + 1 - span..=h.n)
            .map(|i| self.x[(i as usize) & (RING - 1)].norm_sqr() as f64)
            .sum::<f64>()
            / span as f64;
        let Some(d) = self.decs.iter_mut().find(|d| !d.active) else {
            return;
        };
        self.wake = 0;
        d.active = true;
        d.t = h.n;
        d.start = h.n;
        d.prev = prev;
        d.descr = h.descr;
        d.acc = 0.0;
        d.preamble = h.preamble;
        d.header = None;
        d.per = if h.preamble == Preamble::Short { 2 } else { 1 };
        d.need = 48;
        d.bits.clear();
        d.perr_sum = 0.0;
        d.perr_sq = 0.0;
        d.pow = 0.0;
        d.nsym = 0;
        d.level = level;
    }

    fn advance(&mut self, on_frame: &mut impl FnMut(&DsssFrame<'_>)) {
        let sps = self.sps as u64;
        let avail = self.n;
        for i in 0..self.decs.len() {
            if !self.decs[i].active {
                continue;
            }
            loop {
                let next = self.decs[i].t + sps;
                if next + 1 >= avail {
                    break;
                }
                let (cm, c0, cp) = (self.at(next - 1), self.at(next), self.at(next + 1));
                let d = &mut self.decs[i];
                d.t = next;
                let z = c0 * d.prev.conj();
                d.prev = c0;
                let quarter = std::f64::consts::FRAC_PI_2;
                let ang = (z.im as f64).atan2(z.re as f64);
                let (raw, e) = if d.per == 1 {
                    let r = (z.re < 0.0) as u8;
                    let mut e = ang - if r == 1 { std::f64::consts::PI } else { 0.0 };
                    if e < -std::f64::consts::PI {
                        e += std::f64::consts::TAU;
                    }
                    ([r, 0], e)
                } else {
                    let q = ((ang / quarter).round() as i64).rem_euclid(4) as u8;
                    let e = ang - (q as f64) * quarter;
                    let e = (e + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                        - std::f64::consts::PI;
                    let dibit = match q {
                        0 => [0, 0],
                        1 => [0, 1],
                        2 => [1, 1],
                        _ => [1, 0],
                    };
                    (dibit, e)
                };
                let mut b = raw;
                d.descr.descramble(&mut b[..d.per]);
                d.bits.extend_from_slice(&b[..d.per]);
                d.perr_sum += e;
                d.perr_sq += e * e;
                d.pow += c0.norm_sqr() as f64;
                d.nsym += 1;
                let m = c0.norm().max(1e-12);
                d.acc = 0.9 * d.acc + (cp.norm() - cm.norm()) / m;
                if d.acc > 2.5 {
                    d.t += 1;
                    d.acc = 0.0;
                } else if d.acc < -2.5 {
                    d.t -= 1;
                    d.acc = 0.0;
                }
                if d.bits.len() < d.need {
                    continue;
                }
                if d.header.is_none() {
                    match PlcpHeader::from_bits(&d.bits[..48]) {
                        Ok(h) => {
                            self.stats.headers_ok += 1;
                            match (h.rate.barker_bits(), h.psdu_octets()) {
                                (Some(per), Some(oct)) if (1..=MAX_PSDU).contains(&oct) => {
                                    let d = &mut self.decs[i];
                                    d.header = Some(h);
                                    d.per = per;
                                    d.need = 48 + 8 * oct;
                                    continue;
                                }
                                (None, _) => self.stats.other_rate += 1,
                                _ => self.stats.headers_bad += 1,
                            }
                        }
                        Err(_) => self.stats.headers_bad += 1,
                    }
                    self.decs[i].active = false;
                    break;
                }
                self.finish(i, on_frame);
                self.decs[i].active = false;
                break;
            }
        }
        self.wake = self
            .decs
            .iter()
            .filter(|d| d.active)
            .map(|d| d.t + sps + 2)
            .min()
            .unwrap_or(u64::MAX);
    }

    fn finish(&mut self, i: usize, on_frame: &mut impl FnMut(&DsssFrame<'_>)) {
        let d = &self.decs[i];
        let Some(header) = d.header else { return };
        self.bytes.clear();
        pack::pack(
            &d.bits[48..d.need],
            BitOrder::Lsb,
            Pad::Zero,
            &mut self.bytes,
        );
        if !crate::mac::fcs_ok(&self.bytes) {
            self.stats.bad_fcs += 1;
            return;
        }
        self.stats.frames += 1;
        let k = d.nsym.max(1) as f64;
        let mean = d.perr_sum / k;
        let var = (d.perr_sq / k - mean * mean).max(1e-6);
        let n = self.bytes.len();
        on_frame(&DsssFrame {
            mpdu: &self.bytes[..n - 4],
            header,
            preamble: d.preamble,
            sample: d.start,
            offset_hz: mean * 1e6 / std::f64::consts::TAU,
            snr_db: 10.0 * (1.0 / var).log10(),
            power: d.pow / k,
            // The phase steps of the despread symbols spread by var = noise / (sps * signal) (the despread noise is 1 / sps of
            // the input noise), so input = signal * (1 + sps * var). The correlation loss only makes var larger: the level
            // never reads high.
            level_dbfs: sdrkit_algo_dsp::meter::power_to_db(d.level / (1.0 + var * self.sps as f64)),
        });
    }
}
