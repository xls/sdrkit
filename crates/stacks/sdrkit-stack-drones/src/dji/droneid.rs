//! The DroneID burst: numerology, Zadoff-Chu symbols, the packet layout, the coding chain, the burst
//! modulator for the generator, and the burst receiver.
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
        let c = CRC24_LTE_A.checksum(&f[..173]);
        f[173] = (c >> 16) as u8;
        f[174] = (c >> 8) as u8;
        f[175] = c as u8;
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
            q: Quantizer::default(),
            rm,
        }
    }

    /// The 7200 coded and scrambled bits of `frame` with the scrambler `variant` (0 or 1).
    pub fn encode(&mut self, frame: &[u8; FRAME], variant: u8) -> Vec<u8> {
        self.bits.clear();
        pack::unpack(frame, BitOrder::Msb, &mut self.bits);
        self.turbo
            .encode(&self.bits, &mut self.streams)
            .expect("K bits in");
        let mut e = vec![0u8; E];
        let n_cb = self.rm.buffer_len();
        self.rm
            .rate_match(&self.streams, 0, n_cb, &mut e)
            .expect("the DroneID rate matching");
        for (b, &s) in e.iter_mut().zip(&self.scr[variant as usize & 1]) {
            *b ^= s;
        }
        e
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
        let t = ifft_symbol(&x, &mut fft);
        let cp = cp_len(s);
        out.extend_from_slice(&t[NFFT - cp..]);
        out.extend_from_slice(&t);
    }
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
    /// Both above 0.3.
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
    /// The decoded packet, when the CRC-24A passed and the length is valid.
    pub packet: Option<DroneIdPacket>,
    /// The frame, when decoded.
    #[serde(skip)]
    pub frame: Option<[u8; FRAME]>,
    /// Sample (in the input) where symbol 2's prefix starts.
    pub t2: usize,
}

/// The burst receiver: reusable state (FFT plans, sequences, buffers).
pub struct BurstReceiver {
    fft: Fft,
    z: [Vec<Complex32>; 2],
    coder: Coder,
    pbuf: Vec<Complex32>,
    ebuf: Vec<f32>,
    x: Vec<Complex32>,
}

impl Default for BurstReceiver {
    fn default() -> Self {
        BurstReceiver {
            fft: Fft::forward(NFFT),
            z: [zc(ROOTS[0]), zc(ROOTS[1])],
            coder: Coder::new(),
            pbuf: Vec::new(),
            ebuf: Vec::new(),
            x: Vec::new(),
        }
    }
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

impl BurstReceiver {
    fn fft_at(&mut self, x: &[Complex32], start: isize) -> Vec<Complex32> {
        let mut b: Vec<Complex32> = (0..NFFT as isize)
            .map(|i| {
                let j = start + i;
                if j >= 0 && (j as usize) < x.len() {
                    x[j as usize]
                } else {
                    Complex32::new(0.0, 0.0)
                }
            })
            .collect();
        self.fft.process(&mut b);
        b
    }

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

    /// Coherence of symbol `y` with the ZC sequence of root index `r`, its carriers shifted by `s` bins: (coherence, phase slope).
    fn coherence(&self, y: &[Complex32], r: usize, s: i32) -> (f32, f64) {
        let mut num = Complex32::new(0.0, 0.0);
        let mut den = 0.0f32;
        let mut prev: Option<Complex32> = None;
        for k in 0..NC {
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

    /// Decodes one burst: `x` at 15.36 Msps centred on 0 Hz with some margin around it.
    pub fn decode(&mut self, x: &[Complex32]) -> BurstDecode {
        let mut out = BurstDecode::default();
        if x.len() < BURST_LEN - (NFFT + 80) {
            return out;
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
            return out;
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
            let xs = std::mem::take(&mut self.x);
            let y = self.fft_at(&xs, t2 + 2 * (NFFT as isize + 72) + 72 - 36);
            self.x = xs;
            let (coh, _) = self.coherence(&y, 0, 0);
            // A few whole-carrier shifts too, so a large offset is not missed here.
            let mut cbest = coh;
            for s in [-2i32, -1, 1, 2] {
                cbest = cbest.max(self.coherence(&y, 0, s).0);
            }
            if cbest > cand.0 {
                cand = (cbest, t2, f);
            }
        }
        let (t2, frac) = (cand.1, cand.2);
        // Whole-carrier offset: the shift that makes the delays measured on symbols 4 and 6 agree.
        self.derotate(x, frac);
        let xs = std::mem::take(&mut self.x);
        let y4 = self.fft_at(&xs, t2 + 2 * (NFFT as isize + 72) + 72 - 36);
        let y6 = self.fft_at(&xs, t2 + 4 * (NFFT as isize + 72) + 72 - 36);
        self.x = xs;
        let expected = -2.0 * PI * 36.0 / NFFT as f64;
        let mut sbest = (f64::MAX, 0i32);
        for s in -12i32..=12 {
            let (c4, a4) = self.coherence(&y4, 0, s);
            let (c6, a6) = self.coherence(&y6, 1, s);
            if c4 < 0.1 || c6 < 0.1 {
                continue;
            }
            let score =
                wrap(a4 - a6).abs() + 0.01 * wrap(a4 - expected).abs() - 0.1 * (c4 + c6) as f64;
            if score < sbest.0 {
                sbest = (score, s);
            }
        }
        let s = sbest.1;
        let cfo = frac + s as f64 * 15e3;
        out.cfo_hz = cfo;
        out.t2 = t2.max(0) as usize;
        self.derotate(x, cfo);
        let xs = std::mem::take(&mut self.x);
        let mut ys: Vec<Vec<Complex32>> = Vec::with_capacity(8);
        // Symbols 2..9 (index 0 = symbol 2), each 36 samples early inside its prefix.
        for k in 0..8isize {
            let body = t2 + k * (NFFT as isize + 72) + if k == 7 { 80 } else { 72 };
            ys.push(self.fft_at(&xs, body - 36));
        }
        self.x = xs;
        let z4 = self.coherence(&ys[2], 0, 0).0;
        let z6 = self.coherence(&ys[4], 1, 0).0;
        out.zc = [z4, z6];
        out.zc_found = z4 > 0.3 && z6 > 0.3;
        if !out.zc_found {
            return out;
        }
        // Channel from the two ZC symbols.
        let mut h1 = vec![Complex32::new(0.0, 0.0); NC];
        let mut h2 = vec![Complex32::new(0.0, 0.0); NC];
        let mut acc = Complex32::new(0.0, 0.0);
        for k in 0..NC {
            let b = carrier_bin(k);
            h1[k] = ys[2][b] * self.z[0][k].conj();
            h2[k] = ys[4][b] * self.z[1][k].conj();
            acc += h2[k] * h1[k].conj();
        }
        let phi = acc.im.atan2(acc.re);
        let rot = Complex32::new((-phi).cos(), (-phi).sin());
        let mut h = vec![Complex32::new(0.0, 0.0); NC];
        let (mut ph, mut pn) = (0.0f64, 0.0f64);
        for k in 0..NC {
            h[k] = (h1[k] + h2[k] * rot) * 0.5;
            ph += h[k].norm_sqr() as f64;
            pn += (h1[k] - h2[k] * rot).norm_sqr() as f64;
        }
        let mh = ph / NC as f64;
        let sigma2 = (pn / (2.0 * NC as f64)).max(mh * 1e-4);
        out.snr_db = 10.0 * (mh / sigma2).log10();
        // Soft symbols of the six data symbols (2, 3, 5, 7, 8, 9).
        let mut soft = vec![Complex32::new(0.0, 0.0); E / 2];
        let mut j = 0;
        for (k, sym) in [2usize, 3, 5, 7, 8, 9].iter().enumerate() {
            let _ = k;
            let ang = phi * (*sym as f32 - 4.0) / 2.0;
            let r = Complex32::new(ang.cos(), ang.sin());
            let y = &ys[sym - 2];
            for c in 0..NC {
                let hh = h[c] * r;
                soft[j] = y[carrier_bin(c)]
                    * hh.conj()
                    * (2.0 * std::f32::consts::SQRT_2 / sigma2 as f32);
                j += 1;
            }
        }
        // Turbo decoding over the variants and rotations.
        let mut llr = vec![0f32; E];
        for variant in 0..2u8 {
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
                if let Some((frame, it)) = self.coder.decode(&llr, variant, 8) {
                    out.variant = Some(variant);
                    out.rotation = Some(rotation);
                    out.iterations = it;
                    out.packet = DroneIdPacket::parse(&frame);
                    out.frame = Some(frame);
                    return out;
                }
            }
        }
        out
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
