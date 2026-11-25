//! The DroneID burst: numerology, Zadoff-Chu symbols, the frame layouts (the readable packet, the encrypted format and the
//! empty frame), the coding chain, the burst modulator for the generator, and the burst receiver.
//! Standard: 3GPP TS 36.212 clauses 5.1.1, 5.1.3.2 and 5.1.4.1 (the coding chain); TS 36.211 clause 7.2 (the scrambling sequence).

use crate::family::ascii_text;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_algo_fec::ratematch::{LteCode, LteRateMatch};
use sdrkit_algo_fec::turbo::Turbo;
use sdrkit_core::Complex32;
use sdrkit_util_bits::crc::catalog::CRC24_LTE_A;
use sdrkit_util_bits::crc::{Crc, CrcSpec};
use sdrkit_util_bits::pack::{self, BitOrder, Pad};
use sdrkit_util_bits::seq::ZadoffChu;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use std::ops::Range;

/// The burst's sample rate.
pub const FS: f64 = 15.36e6;
/// FFT size.
pub const NFFT: usize = 1024;
/// Used carriers.
pub const NC: usize = 600;
/// Coded bits.
pub const E: usize = 7200;
/// Information bits.
pub const K: usize = 1408;
/// Frame bytes.
pub const FRAME: usize = 176;
/// Packet bytes.
pub const PACKET: usize = 91;
/// Samples of a 9-symbol burst.
pub const BURST_LEN: usize = 9880;
/// The ZC roots of symbols 4 and 6.
pub const ROOTS: [u32; 2] = [600, 147];
/// The scrambler's initial value.
pub const SCRAMBLER_INIT: u32 = 0x1234_5678;

/// The lowest normalised Zadoff-Chu score of each of the two symbols of a burst that the pair trigger found.
pub const MIN_SCORE: f32 = 0.04;
/// The share of the hard decisions that must equal the coded bits of an empty frame. Bits that do not belong to the frame
/// agree by chance: one half, within 0.01 for the 2500 bits of the smallest part of a burst that is tried. A frame that the
/// turbo code can decode agrees in more than three quarters of its bits.
pub const EMPTY_AGREEMENT: f32 = 0.70;
/// The whole-carrier offsets that the receiver tries first on a burst of the pair trigger: 2 carriers and the fraction are
/// 37 kHz, 15 parts in a million at 2.4 GHz.
const NEAR_CARRIERS: i32 = 2;
/// Carriers in a block whose soft bits get one weight for the noise and the other signals on it.
const WEIGHT_BLOCK: usize = 50;
/// Turbo iterations at most.
const ITERATIONS: u32 = 8;
/// The first byte after the zero part of an empty frame: the 4-byte tail starts here.
pub const EMPTY_ZEROS: usize = 169;

/// The inner CRC of a DroneID packet: CRC-16/MCRF4XX with the start value 0x3692 in the reflected register.
pub static INNER_CRC: Crc<u16> = Crc::<u16>::new(CrcSpec::new(
    "CRC-16/DJI-DRONEID",
    16,
    0x1021,
    0x496C,
    true,
    true,
    0x0,
    0x7109,
));

/// The cyclic prefix of symbol `s` (1-based, of 9).
pub fn cp_len(s: usize) -> usize {
    if s == 1 || s == 9 {
        80
    } else {
        72
    }
}

/// The start of symbol `s`'s cyclic prefix in a 9-symbol burst.
pub fn symbol_start(s: usize) -> usize {
    (1..s).map(|k| cp_len(k) + NFFT).sum()
}

/// The FFT bin of carrier `k` (0..600).
pub fn carrier_bin(k: usize) -> usize {
    if k < 300 {
        724 + k
    } else {
        k - 299
    }
}

/// The signed FFT bin of carrier `k` (0..600): -300..=-1 below the centre, 1..=300 above.
pub fn carrier_index(k: usize) -> i32 {
    if k < 300 {
        k as i32 - 300
    } else {
        k as i32 - 299
    }
}

/// The carriers of a burst centred `offset_hz` from the centre of a capture that lie within `edge_hz` of that centre. The
/// receiver gives the other carriers zero weight.
pub fn visible_carriers(offset_hz: f64, edge_hz: f64) -> Range<usize> {
    let inside = |k: usize| (offset_hz + f64::from(carrier_index(k)) * 15e3).abs() <= edge_hz;
    let lo = (0..NC).find(|&k| inside(k)).unwrap_or(NC);
    let hi = (0..NC).rfind(|&k| inside(k)).map_or(lo, |k| k + 1);
    lo..hi
}

/// The Zadoff-Chu sequence of `root`, 600 values in carrier order (n = 300 left out).
pub fn zc(root: u32) -> Vec<Complex32> {
    let Ok(z) = ZadoffChu::new(root, 601) else {
        return Vec::new();
    };
    (0..601u64)
        .filter(|&n| n != 300)
        .map(|n| {
            let [re, im] = z.value(n);
            Complex32::new(re as f32, im as f32)
        })
        .collect()
}

/// The scrambling sequence of a variant: 0 takes bit i of the constant as x2(i), 1 its 31-bit reversal.
pub fn scrambler(variant: u8) -> Vec<u8> {
    let c = if variant == 0 {
        SCRAMBLER_INIT & 0x7FFF_FFFF
    } else {
        (SCRAMBLER_INIT & 0x7FFF_FFFF).reverse_bits() >> 1
    };
    let mut v = vec![0u8; E];
    sdrkit_util_bits::seq::gold_3gpp(c, &mut v);
    v
}

/// The DroneID packet fields.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DroneIdPacket {
    /// Packet length byte.
    pub length: u8,
    /// Version.
    pub version: u8,
    /// Sequence number.
    pub seq: u16,
    /// State information.
    pub state: u16,
    /// Serial number.
    pub serial: String,
    /// Longitude, degrees.
    pub lon: f64,
    /// Latitude, degrees.
    pub lat: f64,
    /// Altitude raw, read as 0.01 m (the unit is not confirmed).
    pub alt_raw: i16,
    /// Height above home raw, read as 0.01 m (the unit is not confirmed).
    pub height_raw: i16,
    /// Velocity north raw.
    pub v_north: i16,
    /// Velocity east raw.
    pub v_east: i16,
    /// Velocity up raw.
    pub v_up: i16,
    /// Yaw raw.
    pub yaw: i16,
    /// GPS time, ms since 1970.
    pub gps_ms: u64,
    /// Pilot latitude, degrees.
    pub pilot_lat: f64,
    /// Pilot longitude, degrees.
    pub pilot_lon: f64,
    /// Home longitude, degrees.
    pub home_lon: f64,
    /// Home latitude, degrees.
    pub home_lat: f64,
    /// Device type code.
    pub device_type: u8,
    /// UUID length.
    pub uuid_len: u8,
    /// UUID text.
    pub uuid: String,
    /// The inner CRC-16 matched.
    pub inner_crc_ok: bool,
}

fn rad7(deg: f64) -> i32 {
    // Radians x 1e7, truncated.
    (deg.to_radians() * 1e7) as i32
}

fn deg7(raw: i32) -> f64 {
    raw as f64 / 1e7 * 180.0 / PI
}

impl DroneIdPacket {
    /// Altitude, metres.
    pub fn alt_m(&self) -> f64 {
        self.alt_raw as f64 * 0.01
    }
    /// Height above home, metres.
    pub fn height_m(&self) -> f64 {
        self.height_raw as f64 * 0.01
    }
    /// Whether the position is a fix (|lat|, |lon| > 1e-4 degrees and in range).
    pub fn has_fix(&self) -> bool {
        self.lat.abs() > 1e-4
            && self.lon.abs() > 1e-4
            && self.lat.abs() <= 90.0
            && self.lon.abs() <= 180.0
    }

    /// The 176-byte frame of this packet (inner CRC-16 and CRC-24A filled in).
    pub fn frame(&self) -> [u8; FRAME] {
        let mut f = [0u8; FRAME];
        f[0] = self.length;
        f[2] = self.version;
        f[3..5].copy_from_slice(&self.seq.to_le_bytes());
        f[5..7].copy_from_slice(&self.state.to_le_bytes());
        for (o, c) in f[7..23].iter_mut().zip(self.serial.bytes()) {
            *o = c;
        }
        f[0x17..0x1B].copy_from_slice(&rad7(self.lon).to_le_bytes());
        f[0x1B..0x1F].copy_from_slice(&rad7(self.lat).to_le_bytes());
        f[0x1F..0x21].copy_from_slice(&self.alt_raw.to_le_bytes());
        f[0x21..0x23].copy_from_slice(&self.height_raw.to_le_bytes());
        f[0x23..0x25].copy_from_slice(&self.v_north.to_le_bytes());
        f[0x25..0x27].copy_from_slice(&self.v_east.to_le_bytes());
        f[0x27..0x29].copy_from_slice(&self.v_up.to_le_bytes());
        f[0x29..0x2B].copy_from_slice(&self.yaw.to_le_bytes());
        f[0x2B..0x33].copy_from_slice(&self.gps_ms.to_le_bytes());
        f[0x33..0x37].copy_from_slice(&rad7(self.pilot_lat).to_le_bytes());
        f[0x37..0x3B].copy_from_slice(&rad7(self.pilot_lon).to_le_bytes());
        f[0x3B..0x3F].copy_from_slice(&rad7(self.home_lon).to_le_bytes());
        f[0x3F..0x43].copy_from_slice(&rad7(self.home_lat).to_le_bytes());
        f[0x43] = self.device_type;
        f[0x44] = self.uuid_len;
        for (o, c) in f[0x45..0x59].iter_mut().zip(self.uuid.bytes()) {
            *o = c;
        }
        let c16 = INNER_CRC.checksum(&f[..0x59]);
        f[0x59..0x5B].copy_from_slice(&c16.to_le_bytes());
        close_frame(&mut f);
        f
    }

    /// Parses a frame whose CRC-24A passed. `None` when the length byte is outside 60..=120.
    pub fn parse(f: &[u8; FRAME]) -> Option<DroneIdPacket> {
        if !(60..=120).contains(&f[0]) {
            return None;
        }
        let i16le = |i: usize| i16::from_le_bytes([f[i], f[i + 1]]);
        let i32le = |i: usize| i32::from_le_bytes([f[i], f[i + 1], f[i + 2], f[i + 3]]);
        let mut g = [0u8; 8];
        g.copy_from_slice(&f[0x2B..0x33]);
        let c16 = INNER_CRC.checksum(&f[..0x59]);
        Some(DroneIdPacket {
            length: f[0],
            version: f[2],
            seq: u16::from_le_bytes([f[3], f[4]]),
            state: u16::from_le_bytes([f[5], f[6]]),
            serial: ascii_text(&f[7..23]),
            lon: deg7(i32le(0x17)),
            lat: deg7(i32le(0x1B)),
            alt_raw: i16le(0x1F),
            height_raw: i16le(0x21),
            v_north: i16le(0x23),
            v_east: i16le(0x25),
            v_up: i16le(0x27),
            yaw: i16le(0x29),
            gps_ms: u64::from_le_bytes(g),
            pilot_lat: deg7(i32le(0x33)),
            pilot_lon: deg7(i32le(0x37)),
            home_lon: deg7(i32le(0x3B)),
            home_lat: deg7(i32le(0x3F)),
            device_type: f[0x43],
            uuid_len: f[0x44],
            uuid: ascii_text(&f[0x45..0x59]),
            inner_crc_ok: c16 == u16::from_le_bytes([f[0x59], f[0x5A]]),
        })
    }
}

/// The magic of the encrypted format with the type 0x10 or 0x11.
pub const MAGIC_INFP: [u8; 4] = *b"INFP";
/// The magic of the encrypted format with the type 0x13.
pub const MAGIC_CRYP: [u8; 4] = *b"CRYP";

/// The readable head of a frame of the encrypted format. The body after it is not readable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncryptedHead {
    /// The length byte: the inner CRC-16 covers the bytes 0 to this offset.
    pub length: u8,
    /// The frame type (0x10, 0x11 and 0x13 were seen).
    pub frame_type: u8,
    /// The magic is "CRYP" (else "INFP").
    pub cryp: bool,
    /// The 4 bytes after the magic. They are constant for one aircraft and differ between aircraft. They are not a serial
    /// number.
    pub key: [u8; 4],
}

/// What a frame holds whose CRC-24A passed.
#[derive(Clone, Debug, PartialEq)]
pub enum FrameKind {
    /// The bytes before the 4-byte tail are all zero.
    Empty,
    /// The encrypted format: a magic and a good inner CRC-16.
    Encrypted(EncryptedHead),
    /// The readable packet.
    Readable(DroneIdPacket),
    /// None of the known layouts.
    Other,
}

/// The kind of a frame whose CRC-24A passed.
pub fn classify(f: &[u8; FRAME]) -> FrameKind {
    if f[..EMPTY_ZEROS].iter().all(|&b| b == 0) {
        return FrameKind::Empty;
    }
    let magic = [f[2], f[3], f[4], f[5]];
    if magic == MAGIC_INFP || magic == MAGIC_CRYP {
        let crc_at = usize::from(f[0]) + 1;
        if crc_at >= 20
            && crc_at + 2 <= FRAME - 3
            && INNER_CRC.checksum(&f[..crc_at]) == u16::from_le_bytes([f[crc_at], f[crc_at + 1]])
        {
            return FrameKind::Encrypted(EncryptedHead {
                length: f[0],
                frame_type: f[1],
                cryp: magic == MAGIC_CRYP,
                key: [f[6], f[7], f[8], f[9]],
            });
        }
    }
    DroneIdPacket::parse(f).map_or(FrameKind::Other, FrameKind::Readable)
}

fn close_frame(f: &mut [u8; FRAME]) {
    let c = CRC24_LTE_A.checksum(&f[..173]);
    f[173] = (c >> 16) as u8;
    f[174] = (c >> 8) as u8;
    f[175] = c as u8;
}

/// A frame of the encrypted format with the magic "INFP": the length 128, `frame_type`, the 4 bytes `key` of the aircraft,
/// the 8 bytes `changing`, the inner length, the 109 bytes `body`, the inner CRC-16, the 4-byte `tail` and the CRC-24A.
pub fn encrypted_frame(
    frame_type: u8,
    key: [u8; 4],
    changing: [u8; 8],
    body: &[u8; 109],
    tail: [u8; 4],
) -> [u8; FRAME] {
    let mut f = [0u8; FRAME];
    f[0] = 128;
    f[1] = frame_type;
    f[2..6].copy_from_slice(&MAGIC_INFP);
    f[6..10].copy_from_slice(&key);
    f[10..18].copy_from_slice(&changing);
    f[18..20].copy_from_slice(&109u16.to_le_bytes());
    f[20..129].copy_from_slice(body);
    let c16 = INNER_CRC.checksum(&f[..129]);
    f[129..131].copy_from_slice(&c16.to_le_bytes());
    f[EMPTY_ZEROS..EMPTY_ZEROS + 4].copy_from_slice(&tail);
    close_frame(&mut f);
    f
}

/// An empty frame: zeros, the 4-byte `tail` and the CRC-24A.
pub fn empty_frame(tail: [u8; 4]) -> [u8; FRAME] {
    let mut f = [0u8; FRAME];
    f[EMPTY_ZEROS..EMPTY_ZEROS + 4].copy_from_slice(&tail);
    close_frame(&mut f);
    f
}

/// The coding chain of a DroneID frame: CRC-24A over the frame, the turbo code of K = 1408, rate matching to 7200 bits with
/// redundancy version 0, and the scrambling sequence. It holds its buffers, so a frame codes and decodes without allocation.
pub struct Coder {
    turbo: Turbo,
    rm: LteRateMatch,
    scr: [Vec<u8>; 2],
    bits: Vec<u8>,
    streams: Vec<u8>,
    soft: Vec<f32>,
    llr: Vec<i8>,
    dematched: Vec<i8>,
    out: Vec<u8>,
    coded: Vec<u8>,
    q: Quantizer,
}

impl Default for Coder {
    fn default() -> Self {
        Coder::new()
    }
}

impl Coder {
    /// The coder.
    pub fn new() -> Coder {
        let rm = LteRateMatch::new(LteCode::Turbo, K + 4, 0).expect("the DroneID stream length");
        Coder {
            turbo: Turbo::new(K).expect("K = 1408 is a block size of TS 36.212 table 5.1.3-3"),
            scr: [scrambler(0), scrambler(1)],
            bits: Vec::with_capacity(K),
            streams: vec![0; 3 * (K + 4)],
            soft: vec![0.0; E],
            llr: vec![0; E],
            dematched: vec![0; 3 * (K + 4)],
            out: vec![0; K],
            coded: vec![0; E],
            q: Quantizer::default(),
            rm,
        }
    }

    /// The 7200 coded and scrambled bits of `frame` with the scrambler `variant` (0 or 1).
    pub fn encode(&mut self, frame: &[u8; FRAME], variant: u8) -> Vec<u8> {
        self.coded(frame, variant).to_vec()
    }

    /// As [`Coder::encode`], into a buffer of the coder.
    pub fn coded(&mut self, frame: &[u8; FRAME], variant: u8) -> &[u8] {
        self.bits.clear();
        pack::unpack(frame, BitOrder::Msb, &mut self.bits);
        self.turbo
            .encode(&self.bits, &mut self.streams)
            .expect("K bits in");
        let n_cb = self.rm.buffer_len();
        self.rm
            .rate_match(&self.streams, 0, n_cb, &mut self.coded)
            .expect("the DroneID rate matching");
        for (b, &s) in self.coded.iter_mut().zip(&self.scr[variant as usize & 1]) {
            *b ^= s;
        }
        &self.coded
    }

    /// Decodes 7200 soft values (positive means 0) scrambled with `variant`, with at most `iterations` turbo iterations, stopping
    /// when the CRC-24A holds. Returns the frame and the iterations used, or `None` when the CRC never held.
    pub fn decode(
        &mut self,
        soft: &[f32],
        variant: u8,
        iterations: u32,
    ) -> Option<([u8; FRAME], u32)> {
        if soft.len() != E {
            return None;
        }
        for ((o, &v), &s) in self
            .soft
            .iter_mut()
            .zip(soft)
            .zip(&self.scr[variant as usize & 1])
        {
            *o = if s == 1 { -v } else { v };
        }
        let scale = self.q.scale_for(&self.soft);
        self.q.quantize(&self.soft, scale, &mut self.llr).ok()?;
        self.dematched.iter_mut().for_each(|v| *v = 0);
        let n_cb = self.rm.buffer_len();
        self.rm
            .dematch(&self.llr, 0, n_cb, &mut self.dematched)
            .ok()?;
        let r = self
            .turbo
            .decode(
                &self.dematched,
                iterations,
                |b| CRC24_LTE_A.verify_bits(b),
                &mut self.out,
            )
            .ok()?;
        let mut f = [0u8; FRAME];
        pack::pack_into(&self.out, BitOrder::Msb, Pad::Zero, &mut f).ok()?;
        Some((f, r.iterations))
    }
}

/// The 7200 coded, scrambled bits of a frame.
pub fn encode_frame(frame: &[u8; FRAME], variant: u8) -> Vec<u8> {
    Coder::new().encode(frame, variant)
}

fn ifft_symbol(x: &[Complex32; NFFT], fft: &mut Fft) -> Vec<Complex32> {
    let mut b = x.to_vec();
    fft.process(&mut b);
    let s = 1.0 / (NC as f32).sqrt();
    for v in &mut b {
        *v *= s;
    }
    b
}

/// A DroneID burst at 15.36 Msps: 9 symbols (or 8 without the first), carrying `frame`, scrambled with `variant`. Symbol 1 carries
/// a pseudo-random QPSK pattern (its real content is not published).
pub fn modulate(frame: &[u8; FRAME], variant: u8, eight: bool) -> Vec<Complex32> {
    modulate_carriers(frame, variant, eight, 0..NC)
}

/// As [`modulate`], with only the carriers of `carriers` sent: a burst that the edge of a capture cuts.
pub fn modulate_carriers(
    frame: &[u8; FRAME],
    variant: u8,
    eight: bool,
    carriers: Range<usize>,
) -> Vec<Complex32> {
    let bits = encode_frame(frame, variant);
    let mut fft = Fft::inverse(NFFT);
    let mut out = Vec::with_capacity(BURST_LEN);
    let mut rng = sdrkit_algo_sim::rng::Rng::new(0xD1D);
    let mut data = 0;
    for s in 1..=9usize {
        let mut x = [Complex32::new(0.0, 0.0); NFFT];
        match s {
            4 | 6 => {
                let z = zc(if s == 4 { ROOTS[0] } else { ROOTS[1] });
                for (k, v) in z.iter().enumerate() {
                    x[carrier_bin(k)] = *v;
                }
            }
            1 => {
                for k in 0..NC {
                    let b = rng.next_u64();
                    x[carrier_bin(k)] = qpsk((b & 1) as u8, ((b >> 1) & 1) as u8);
                }
            }
            _ => {
                for k in 0..NC {
                    x[carrier_bin(k)] = qpsk(bits[data + 2 * k], bits[data + 2 * k + 1]);
                }
                data += 2 * NC;
            }
        }
        if s == 1 && eight {
            continue;
        }
        for k in (0..NC).filter(|k| !carriers.contains(k)) {
            x[carrier_bin(k)] = Complex32::new(0.0, 0.0);
        }
        let t = ifft_symbol(&x, &mut fft);
        let cp = cp_len(s);
        out.extend_from_slice(&t[NFFT - cp..]);
        out.extend_from_slice(&t);
    }
    out
}

/// The Zadoff-Chu symbol of root index `r` (0 for root 600, 1 for root 147) at 15.36 Msps: the 72-sample prefix and the body.
pub fn zc_symbol(r: usize) -> Vec<Complex32> {
    let mut x = [Complex32::new(0.0, 0.0); NFFT];
    for (k, v) in zc(ROOTS[r & 1]).iter().enumerate() {
        x[carrier_bin(k)] = *v;
    }
    let t = ifft_symbol(&x, &mut Fft::inverse(NFFT));
    let mut out = Vec::with_capacity(NFFT + 72);
    out.extend_from_slice(&t[NFFT - 72..]);
    out.extend_from_slice(&t);
    out
}

fn qpsk(b0: u8, b1: u8) -> Complex32 {
    let s = std::f32::consts::FRAC_1_SQRT_2;
    Complex32::new(if b0 == 0 { s } else { -s }, if b1 == 0 { s } else { -s })
}

/// What the burst receiver found.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BurstDecode {
    /// ZC coherence of symbols 4 and 6.
    pub zc: [f32; 2],
    /// The normalised correlation of symbols 4 and 6 with their sequences: the share of the power of the symbol (signal and
    /// noise over the whole 15.36 MHz) that is the sequence on the carriers used. 1 for a clean whole burst.
    pub score: [f32; 2],
    /// The two symbols were found: both coherences above 0.3 for a burst of the burst detector, both scores at least
    /// [`MIN_SCORE`] for a burst of the pair trigger.
    pub zc_found: bool,
    /// Carrier offset of the burst, Hz (relative to its 0 Hz).
    pub cfo_hz: f64,
    /// SNR from the two ZC symbols, dB.
    pub snr_db: f64,
    /// The scrambler variant that decoded.
    pub variant: Option<u8>,
    /// The QPSK rotation (quarter turns) that decoded.
    pub rotation: Option<u8>,
    /// Turbo iterations used.
    pub iterations: u32,
    /// The share of the hard decisions on the carriers used that equal the coded bits of the decoded frame.
    pub agreement: f32,
    /// The decoded packet, when the CRC-24A passed and the frame is the readable packet.
    pub packet: Option<DroneIdPacket>,
    /// The frame, when decoded.
    #[serde(skip)]
    pub frame: Option<[u8; FRAME]>,
    /// Sample (in the input) where symbol 2's prefix starts.
    pub t2: usize,
}

impl BurstDecode {
    /// Whether the decoded frame is accepted. A frame of zeros passes the CRC-24A whatever was received, so an empty frame
    /// also needs both scores of at least [`MIN_SCORE`] and an agreement of at least [`EMPTY_AGREEMENT`].
    pub fn accepted(&self) -> bool {
        match &self.frame {
            None => false,
            Some(f) if f[..EMPTY_ZEROS].iter().all(|&b| b == 0) => {
                self.score[0] >= MIN_SCORE
                    && self.score[1] >= MIN_SCORE
                    && self.agreement >= EMPTY_AGREEMENT
            }
            Some(_) => true,
        }
    }
}

/// The burst receiver: reusable state (FFT plans, sequences, buffers).
pub struct BurstReceiver {
    fft: Fft,
    z: [Vec<Complex32>; 2],
    turn: Vec<Complex32>,
    coder: Coder,
    pbuf: Vec<Complex32>,
    ebuf: Vec<f32>,
    x: Vec<Complex32>,
    /// The symbols 2 to 9 in the frequency domain, and two more buffers of one symbol.
    ys: Vec<Vec<Complex32>>,
    h: [Vec<Complex32>; 3],
    flat: Vec<Complex32>,
    soft: Vec<Complex32>,
    llr: Vec<f32>,
}

impl Default for BurstReceiver {
    fn default() -> Self {
        BurstReceiver {
            fft: Fft::forward(NFFT),
            z: [zc(ROOTS[0]), zc(ROOTS[1])],
            turn: (0..NFFT)
                .map(|i| {
                    let a = 2.0 * PI * i as f64 / NFFT as f64;
                    Complex32::new(a.cos() as f32, a.sin() as f32)
                })
                .collect(),
            coder: Coder::new(),
            pbuf: Vec::new(),
            ebuf: Vec::new(),
            x: Vec::with_capacity(2 * BURST_LEN),
            ys: vec![vec![Complex32::new(0.0, 0.0); NFFT]; 10],
            h: std::array::from_fn(|_| vec![Complex32::new(0.0, 0.0); NC]),
            flat: Vec::with_capacity(2 * NC + 1),
            soft: vec![Complex32::new(0.0, 0.0); E / 2],
            llr: vec![0.0; E],
        }
    }
}

/// The FFT of the 1024 samples of `x` from `start` into `out`. Samples outside `x` count as 0.
fn fft_at(fft: &mut Fft, x: &[Complex32], start: isize, out: &mut [Complex32]) {
    for (i, o) in out.iter_mut().enumerate() {
        let j = start + i as isize;
        *o = if j >= 0 && (j as usize) < x.len() {
            x[j as usize]
        } else {
            Complex32::new(0.0, 0.0)
        };
    }
    fft.process(out);
}

fn wrap(a: f64) -> f64 {
    let mut a = a % (2.0 * PI);
    if a > PI {
        a -= 2.0 * PI;
    } else if a < -PI {
        a += 2.0 * PI;
    }
    a
}

/// The prefix correlation and energy of the symbols 2 to 9 with symbol 2's prefix at `t2` of `x`. Parts outside `x` count as
/// silence.
fn prefix_metric(x: &[Complex32], t2: isize) -> (Complex32, f32) {
    let mut c = Complex32::new(0.0, 0.0);
    let mut e = 0.0;
    for s in 0..8isize {
        let st = t2 + s * (NFFT as isize + 72) + if s == 7 { 8 } else { 0 };
        if st < 0 || st as usize + 72 + NFFT > x.len() {
            continue;
        }
        let st = st as usize;
        for i in st..st + 72 {
            c += x[i] * x[i + NFFT].conj();
            e += 0.5 * (x[i].norm_sqr() + x[i + NFFT].norm_sqr());
        }
    }
    (c, e)
}

impl BurstReceiver {
    /// The CP metric at symbol 2's prefix start `t2`: (correlation, energy).
    fn cp_metric(&self, t2: usize) -> (Complex32, f32) {
        let mut c = Complex32::new(0.0, 0.0);
        let mut e = 0.0;
        for s in 2..=9usize {
            let mut st = t2 + (s - 2) * (NFFT + 72);
            if s == 9 {
                st += 8;
            }
            if st + 72 > self.pbuf.len() {
                break;
            }
            for i in st..st + 72 {
                c += self.pbuf[i];
                e += self.ebuf[i];
            }
        }
        (c, e)
    }

    /// Coherence of symbol `y` with the ZC sequence of root index `r` over the carriers `on`, its carriers shifted by `s` bins:
    /// (coherence, phase slope).
    fn coherence(&self, y: &[Complex32], r: usize, s: i32, on: &Range<usize>) -> (f32, f64) {
        let mut num = Complex32::new(0.0, 0.0);
        let mut den = 0.0f32;
        let mut prev: Option<Complex32> = None;
        for k in on.clone() {
            let b = (carrier_bin(k) as i32 + s).rem_euclid(NFFT as i32) as usize;
            let h = y[b] * self.z[r][k].conj();
            if k == 300 {
                prev = None;
            }
            if let Some(p) = prev {
                num += h * p.conj();
                den += h.norm() * p.norm();
            }
            prev = Some(h);
        }
        let c = if den > 0.0 { num.norm() / den } else { 0.0 };
        (c, num.im.atan2(num.re) as f64)
    }

    /// The normalised correlation of symbol `y` (taken up to 72 samples early inside its prefix) with the ZC sequence of root
    /// index `r` on the carriers `on`: the best over the timing of |sum Y Z*|^2 / (carriers x the power of all bins).
    fn score(&self, y: &[Complex32], r: usize, on: &Range<usize>) -> f32 {
        let total: f32 = y.iter().map(|v| v.norm_sqr()).sum();
        if total <= 0.0 || on.is_empty() {
            return 0.0;
        }
        let mut best = 0.0f32;
        for lag in 0..=72i32 {
            let mut acc = Complex32::new(0.0, 0.0);
            for k in on.clone() {
                let m = carrier_index(k);
                let turn = self.turn[(m * lag).rem_euclid(NFFT as i32) as usize];
                acc += y[carrier_bin(k)] * self.z[r][k].conj() * turn;
            }
            best = best.max(acc.norm_sqr());
        }
        best / (NC as f32 * total)
    }

    /// Decodes one burst: `x` at 15.36 Msps centred on 0 Hz with some margin around it.
    pub fn decode(&mut self, x: &[Complex32]) -> BurstDecode {
        if x.len() < BURST_LEN - (NFFT + 80) {
            return BurstDecode::default();
        }
        // CP products and energies.
        self.pbuf.clear();
        self.ebuf.clear();
        for i in 0..x.len() - NFFT {
            self.pbuf.push(x[i] * x[i + NFFT].conj());
            self.ebuf
                .push(0.5 * (x[i].norm_sqr() + x[i + NFFT].norm_sqr()));
        }
        // Coarse timing: the best normalised CP metric over symbols 2..9.
        let span = 7 * (NFFT + 72) + 8 + 72;
        if self.pbuf.len() < span + 1 {
            return BurstDecode::default();
        }
        // Sliding window sums of 72, then the 8-symbol sum at each t2.
        let n = self.pbuf.len();
        let mut wc = vec![Complex32::new(0.0, 0.0); n];
        let mut we = vec![0f32; n];
        let (mut sc, mut se) = (Complex32::new(0.0, 0.0), 0.0f32);
        for i in 0..n {
            sc += self.pbuf[i];
            se += self.ebuf[i];
            if i >= 72 {
                sc -= self.pbuf[i - 72];
                se -= self.ebuf[i - 72];
            }
            if i + 1 >= 72 {
                wc[i + 1 - 72] = sc;
                we[i + 1 - 72] = se;
            }
        }
        let mut best = (0.0f32, 0usize, Complex32::new(0.0, 0.0));
        for t2 in 0..n.saturating_sub(span) {
            let mut c = Complex32::new(0.0, 0.0);
            let mut e = 0.0;
            for s in 0..8 {
                let st = t2 + s * (NFFT + 72) + if s == 7 { 8 } else { 0 };
                c += wc[st];
                e += we[st];
            }
            let m = if e > 0.0 { c.norm() / e } else { 0.0 };
            if m > best.0 {
                best = (m, t2, c);
            }
        }
        let peak = best.1 as isize;
        let all = 0..NC;
        // Candidates around the peak, each with its own fractional offset; the one whose symbol 4 best matches root 600.
        let mut cand = (-1.0f32, peak, 0.0f64);
        for d in [0isize, 8, -8, 1096, -1096, 1104, -1104, 2192, -2192] {
            let t2 = peak + d;
            if t2 < 0 || t2 as usize + span >= n {
                continue;
            }
            let (c, _) = self.cp_metric(t2 as usize);
            let f = -(c.im.atan2(c.re) as f64) / (2.0 * PI * NFFT as f64) * FS;
            self.derotate(x, f);
            let mut y = std::mem::take(&mut self.ys[8]);
            fft_at(
                &mut self.fft,
                &self.x,
                t2 + 2 * (NFFT as isize + 72) + 72 - 36,
                &mut y,
            );
            let (coh, _) = self.coherence(&y, 0, 0, &all);
            // A few whole-carrier shifts too, so a large offset is not missed here.
            let mut cbest = coh;
            for s in [-2i32, -1, 1, 2] {
                cbest = cbest.max(self.coherence(&y, 0, s, &all).0);
            }
            self.ys[8] = y;
            if cbest > cand.0 {
                cand = (cbest, t2, f);
            }
        }
        self.finish(x, cand.1, cand.2, all, false)
    }

    /// Decodes a burst that the pair trigger found: `x` at 15.36 Msps centred on 0 Hz, the prefix of symbol 4 (root 600) within a
    /// few samples of `zc_at`, and only the carriers `on` inside the capture. The other carriers get zero weight. The channel
    /// estimate is smoothed over neighbouring carriers, so a weak burst decodes.
    pub fn decode_at(&mut self, x: &[Complex32], zc_at: usize, on: Range<usize>) -> BurstDecode {
        let hint = zc_at as isize - 2 * (NFFT as isize + 72);
        let mut best = (-1.0f32, hint, Complex32::new(0.0, 0.0));
        for d in (-16..=16isize).step_by(2) {
            let (c, e) = prefix_metric(x, hint + d);
            let m = if e > 0.0 { c.norm() / e } else { 0.0 };
            if m > best.0 {
                best = (m, hint + d, c);
            }
        }
        let c = best.2;
        let f = -(c.im.atan2(c.re) as f64) / (2.0 * PI * NFFT as f64) * FS;
        self.finish(x, best.1, f, on, true)
    }

    /// The steps after the timing `t2` (symbol 2's prefix) and the fractional offset `frac`: the whole-carrier offset, then
    /// [`BurstReceiver::finish_at`]: the symbols, the scores, the channel, the soft bits and the turbo decoding.
    fn finish(
        &mut self,
        x: &[Complex32],
        t2: isize,
        frac: f64,
        on: Range<usize>,
        triggered: bool,
    ) -> BurstDecode {
        // Whole-carrier offset: the shift that makes the delays measured on symbols 4 and 6 agree.
        self.derotate(x, frac);
        let mut y4 = std::mem::take(&mut self.ys[8]);
        let mut y6 = std::mem::take(&mut self.ys[9]);
        let sym = NFFT as isize + 72;
        fft_at(&mut self.fft, &self.x, t2 + 2 * sym + 72 - 36, &mut y4);
        fft_at(&mut self.fft, &self.x, t2 + 4 * sym + 72 - 36, &mut y6);
        let expected = -2.0 * PI * 36.0 / NFFT as f64;
        let best = |rx: &Self, within: i32| {
            let mut sbest = (f64::MAX, 0i32);
            for s in -within..=within {
                let (c4, a4) = rx.coherence(&y4, 0, s, &on);
                let (c6, a6) = rx.coherence(&y6, 1, s, &on);
                if c4 < 0.1 || c6 < 0.1 {
                    continue;
                }
                let score =
                    wrap(a4 - a6).abs() + 0.01 * wrap(a4 - expected).abs() - 0.1 * (c4 + c6) as f64;
                if score < sbest.0 {
                    sbest = (score, s);
                }
            }
            sbest.1
        };
        let wide = best(self, 12);
        let near = best(self, NEAR_CARRIERS);
        self.ys[8] = y4;
        self.ys[9] = y6;
        if !triggered {
            return self.finish_at(x, t2, frac, wide, on, false);
        }
        // A shift of 4 carriers moves the delay of root 147 by nearly a whole symbol, so a weak burst cannot tell 0 from 4.
        // The offsets of real transmitters are small: the best shift of up to 2 carriers is tried first.
        let out = self.finish_at(x, t2, frac, near, on.clone(), true);
        if out.frame.is_some() || wide == near {
            return out;
        }
        let other = self.finish_at(x, t2, frac, wide, on, true);
        if other.frame.is_some() {
            other
        } else {
            out
        }
    }

    /// The steps after the whole-carrier offset `s`.
    fn finish_at(
        &mut self,
        x: &[Complex32],
        t2: isize,
        frac: f64,
        s: i32,
        on: Range<usize>,
        triggered: bool,
    ) -> BurstDecode {
        let mut out = BurstDecode::default();
        let cfo = frac + s as f64 * 15e3;
        out.cfo_hz = cfo;
        out.t2 = t2.max(0) as usize;
        self.derotate(x, cfo);
        let mut ys = std::mem::take(&mut self.ys);
        // Symbols 2..9 (index 0 = symbol 2), each 36 samples early inside its prefix.
        for (k, y) in ys.iter_mut().take(8).enumerate() {
            let body = t2 + k as isize * (NFFT as isize + 72) + if k == 7 { 80 } else { 72 };
            fft_at(&mut self.fft, &self.x, body - 36, y);
        }
        self.decide(&ys, &on, triggered, &mut out);
        self.ys = ys;
        out
    }

    /// The scores of the symbols `ys`, and with the two ZC symbols found, the channel, the soft bits and the turbo decoding.
    fn decide(
        &mut self,
        ys: &[Vec<Complex32>],
        on: &Range<usize>,
        triggered: bool,
        out: &mut BurstDecode,
    ) {
        let (z4, slope) = self.coherence(&ys[2], 0, 0, on);
        let z6 = self.coherence(&ys[4], 1, 0, on).0;
        out.zc = [z4, z6];
        out.score = [self.score(&ys[2], 0, on), self.score(&ys[4], 1, on)];
        out.zc_found = if triggered {
            out.score[0] >= MIN_SCORE && out.score[1] >= MIN_SCORE
        } else {
            z4 > 0.3 && z6 > 0.3
        };
        if !out.zc_found {
            return;
        }
        // Channel from the two ZC symbols.
        let used = on.len().max(1) as f64;
        let [mut h1, mut h2, mut h] = std::mem::take(&mut self.h);
        let mut acc = Complex32::new(0.0, 0.0);
        for k in on.clone() {
            let b = carrier_bin(k);
            h1[k] = ys[2][b] * self.z[0][k].conj();
            h2[k] = ys[4][b] * self.z[1][k].conj();
            acc += h2[k] * h1[k].conj();
        }
        let phi = acc.im.atan2(acc.re);
        let rot = Complex32::new((-phi).cos(), (-phi).sin());
        let mut pn = 0.0f64;
        for k in on.clone() {
            h[k] = (h1[k] + h2[k] * rot) * 0.5;
            pn += (h1[k] - h2[k] * rot).norm_sqr() as f64;
        }
        if triggered {
            smooth_channel(&mut h, on, slope, &mut self.flat);
        }
        let mh = on.clone().map(|k| h[k].norm_sqr() as f64).sum::<f64>() / used;
        let sigma2 = (pn / (2.0 * used)).max(mh * 1e-4);
        out.snr_db = 10.0 * (mh / sigma2).log10();
        // Soft symbols of the six data symbols (2, 3, 5, 7, 8, 9).
        let mut soft = std::mem::take(&mut self.soft);
        let mut j = 0;
        for sym in [2usize, 3, 5, 7, 8, 9] {
            let ang = phi * (sym as f32 - 4.0) / 2.0;
            let r = Complex32::new(ang.cos(), ang.sin());
            let y = &ys[sym - 2];
            for c in 0..NC {
                soft[j] = if on.contains(&c) {
                    let hh = h[c] * r;
                    y[carrier_bin(c)] * hh.conj() * (2.0 * std::f32::consts::SQRT_2 / sigma2 as f32)
                } else {
                    Complex32::new(0.0, 0.0)
                };
                j += 1;
            }
            if triggered {
                // Another signal on a part of the band during this symbol: the received power of a block of carriers
                // is above the power of the burst and the noise. The soft bits of the block get less weight.
                let soft = &mut soft[j - NC..j];
                let mut from = on.start;
                while from < on.end {
                    let to = (from + WEIGHT_BLOCK).min(on.end);
                    let n = (to - from) as f64;
                    let got = (from..to)
                        .map(|c| y[carrier_bin(c)].norm_sqr() as f64)
                        .sum::<f64>()
                        / n;
                    let burst = (from..to).map(|c| h[c].norm_sqr() as f64).sum::<f64>() / n;
                    let excess = (got - burst) / sigma2;
                    if excess > 1.5 {
                        let w = (1.0 / excess) as f32;
                        soft[from..to].iter_mut().for_each(|v| *v *= w);
                    }
                    from = to;
                }
            }
        }
        self.h = [h1, h2, h];
        // Turbo decoding over the variants and rotations.
        let mut llr = std::mem::take(&mut self.llr);
        'trials: for variant in 0..2u8 {
            for rotation in 0..4u8 {
                let q = match rotation {
                    0 => Complex32::new(1.0, 0.0),
                    1 => Complex32::new(0.0, 1.0),
                    2 => Complex32::new(-1.0, 0.0),
                    _ => Complex32::new(0.0, -1.0),
                };
                for (i, v) in soft.iter().enumerate() {
                    let v = v * q;
                    // LLR = log P(0) / P(1): a positive part means 0.
                    llr[2 * i] = v.re;
                    llr[2 * i + 1] = v.im;
                }
                if let Some((frame, it)) = self.coder.decode(
                    &llr,
                    variant,
                    if triggered {
                        2 * ITERATIONS
                    } else {
                        ITERATIONS
                    },
                ) {
                    out.variant = Some(variant);
                    out.rotation = Some(rotation);
                    out.iterations = it;
                    let coded = self.coder.coded(&frame, variant);
                    let (mut same, mut all) = (0u32, 0u32);
                    for (i, (&l, &b)) in llr.iter().zip(coded).enumerate() {
                        if on.contains(&((i / 2) % NC)) {
                            all += 1;
                            same += u32::from((l < 0.0) == (b == 1));
                        }
                    }
                    out.agreement = same as f32 / all.max(1) as f32;
                    out.packet = DroneIdPacket::parse(&frame);
                    out.frame = Some(frame);
                    break 'trials;
                }
            }
        }
        self.soft = soft;
        self.llr = llr;
    }

    fn derotate(&mut self, x: &[Complex32], f: f64) {
        self.x.clear();
        let w = -2.0 * PI * f / FS;
        let step = Complex32::new(w.cos() as f32, w.sin() as f32);
        let mut p = Complex32::new(1.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            if i % 1024 == 0 {
                let a = w * i as f64;
                p = Complex32::new(a.cos() as f32, a.sin() as f32);
            }
            self.x.push(v * p);
            p *= step;
        }
    }
}

/// Replaces the channel of each carrier of `on` by the mean over up to 8 carriers on each side, after the phase step per
/// carrier (the timing inside the prefix) is taken out. `slope` is a first value of that step. `scratch` is a buffer.
fn smooth_channel(
    h: &mut [Complex32],
    on: &Range<usize>,
    slope: f64,
    scratch: &mut Vec<Complex32>,
) {
    const SIDE: usize = 8;
    const LAG: usize = 8;
    // The step again from carriers 8 apart: eight times the phase for the same noise. `slope` tells which turn it is.
    let mut acc = Complex32::new(0.0, 0.0);
    for k in on.start + LAG..on.end {
        if (k < 300) == (k - LAG < 300) {
            acc += h[k] * h[k - LAG].conj();
        }
    }
    let coarse = slope * LAG as f64;
    let fine = f64::from(acc.im.atan2(acc.re));
    let slope = if acc.norm_sqr() > 0.0 {
        (coarse + wrap(fine - coarse)) / LAG as f64
    } else {
        slope
    };
    let turn = |k: usize, sign: f64| {
        let a = sign * slope * f64::from(carrier_index(k));
        Complex32::new(a.cos() as f32, a.sin() as f32)
    };
    // The running sums of the flattened channel.
    scratch.clear();
    let mut acc = Complex32::new(0.0, 0.0);
    scratch.push(acc);
    for k in on.clone() {
        acc += h[k] * turn(k, -1.0);
        scratch.push(acc);
    }
    let n = on.len();
    for (i, k) in on.clone().enumerate() {
        let lo = i.saturating_sub(SIDE);
        let hi = (i + SIDE + 1).min(n);
        h[k] = (scratch[hi] - scratch[lo]) / (hi - lo) as f32 * turn(k, 1.0);
    }
}
