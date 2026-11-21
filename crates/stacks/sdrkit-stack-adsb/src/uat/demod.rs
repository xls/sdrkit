//! The UAT demodulator: continuous-phase FSK sliced on the phase difference across each bit, the sync words of the downlink and
//! the uplink at both sample phases, and the Reed-Solomon codes. [`UatDemod`] is the component.
//!
//! Standard: RTCA DO-282B 2.2.2 and 2.2.3

use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_fec::rs::{codes, Rs};
use sdrkit_core::{Complex32, ConfigError, Values};
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};
use std::collections::VecDeque;

/// The bit rate.
pub const BIT_RATE: f64 = 1.0e6 * 25.0 / 24.0;
/// The working rate: two samples per bit.
pub const WORK_RATE: f64 = 2.0 * BIT_RATE;
/// The downlink (ADS-B) sync word.
pub const SYNC_DOWN: u64 = 0xEACDDA4E2;
/// The uplink sync word.
pub const SYNC_UP: u64 = 0x153225B1D;
const SYNC_MASK: u64 = (1 << 36) - 1;

/// What kind of frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UatKind {
    /// A short downlink frame (18 payload bytes).
    Short,
    /// A long downlink frame (34 payload bytes).
    Long,
    /// An uplink frame (432 bytes).
    Uplink,
}

/// A frame after Reed-Solomon decoding.
#[derive(Clone, Debug, PartialEq)]
pub struct UatFrame {
    /// Signal time of the first data bit, s.
    pub time: f64,
    /// The kind.
    pub kind: UatKind,
    /// The payload (18, 34 or 432 bytes).
    pub bytes: Vec<u8>,
    /// Byte errors corrected.
    pub corrected: usize,
    /// Power over the frame, dBFS.
    pub level_dbfs: f32,
    /// The stream's noise figure at the time, dBFS.
    pub noise_dbfs: f32,
}

/// The demodulator.
pub struct UatDemodulator {
    /// The anti-alias filter and rate change to the working rate. `None` for a rate that is not positive and finite.
    rs: Option<Resampler<Complex32>>,
    /// The resampler delay in seconds, taken off the frame times.
    delay_s: f64,
    work: Vec<Complex32>,
    epoch: f64,
    base: u64,
    ph: Vec<u16>,
    pw: Vec<f32>,
    total: u64,
    next: u64,
    regs: [u64; 2],
    nbits: [u32; 2],
    pending: VecDeque<(u64, bool)>,
    skip_until: u64,
    blk_sum: f64,
    blk_n: usize,
    noise_db: Option<f32>,
    codec: UatCodec,
}

const MAX_FRAME_SAMPLES: u64 = (36 + 4416) * 2 + 8;

impl UatDemodulator {
    /// For an input rate.
    pub fn new(in_rate: f64) -> UatDemodulator {
        let mut d = UatDemodulator {
            rs: None,
            delay_s: 0.0,
            work: Vec::new(),
            epoch: 0.0,
            base: 0,
            ph: Vec::new(),
            pw: Vec::new(),
            total: 0,
            next: 0,
            regs: [0; 2],
            nbits: [0; 2],
            pending: VecDeque::new(),
            skip_until: 0,
            blk_sum: 0.0,
            blk_n: 0,
            noise_db: None,
            codec: UatCodec::default(),
        };
        d.reset(in_rate);
        d
    }
    /// Starts a new stream at this rate (the time base continues). The input is low-passed to the UAT channel (flat to 0.4 of the
    /// working rate, 60 dB down from 0.5 of it) before the rate changes, so a signal outside the channel does not fold into it.
    pub fn reset(&mut self, in_rate: f64) {
        self.epoch += self.total as f64 / WORK_RATE;
        self.rs = Resampler::with_quality(in_rate, WORK_RATE, 0.4, 60.0).ok();
        self.delay_s = self.rs.as_ref().map_or(0.0, |r| r.delay() / in_rate);
        self.base = 0;
        self.ph.clear();
        self.pw.clear();
        self.total = 0;
        self.next = 0;
        self.regs = [0; 2];
        self.nbits = [0; 2];
        self.pending.clear();
        self.skip_until = 0;
    }
    /// The noise figure, dBFS.
    pub fn noise_dbfs(&self) -> f32 {
        self.noise_db.unwrap_or(-120.0)
    }
    /// The signal time at the end of what was fed.
    pub fn time(&self) -> f64 {
        self.epoch + self.total as f64 / WORK_RATE
    }

    fn push(&mut self, z: Complex32) {
        // A 16-bit phase. (The samples are not rounded to 8-bit steps first: a weak burst of a float stream keeps its phase.)
        let ang = z.im.atan2(z.re) as f64;
        let p = ((ang / std::f64::consts::TAU) * 65536.0).round() as i64;
        self.ph.push(p as u16);
        let pwr = z.norm_sqr();
        self.pw.push(pwr);
        self.total += 1;
        self.blk_sum += pwr as f64;
        self.blk_n += 1;
        if self.blk_n == 16384 {
            let db = 10.0 * ((self.blk_sum / 16384.0).max(1e-12)).log10() as f32;
            self.noise_db = Some(match self.noise_db {
                None => db,
                Some(o) => 0.95 * o + 0.05 * db,
            });
            self.blk_sum = 0.0;
            self.blk_n = 0;
        }
    }

    #[inline]
    fn d(&self, abs: u64) -> i16 {
        let i = (abs - self.base) as usize;
        self.ph[i + 1].wrapping_sub(self.ph[i]) as i16
    }

    /// Feeds samples; decoded frames are appended to `out`.
    pub fn feed(&mut self, iq: &[Complex32], out: &mut Vec<UatFrame>) {
        let Some(rs) = &mut self.rs else {
            return;
        };
        let mut work = std::mem::take(&mut self.work);
        work.clear();
        rs.process(iq, &mut work);
        for &z in &work {
            self.push(z);
        }
        self.work = work;
        self.search(out);
        let keep = self.next.saturating_sub(MAX_FRAME_SAMPLES + 200);
        let keep = self
            .pending
            .front()
            .map_or(keep, |p| keep.min(p.0.saturating_sub(100)));
        if keep > self.base + 65536 {
            let d = (keep - self.base) as usize;
            self.ph.drain(..d);
            self.pw.drain(..d);
            self.base = keep;
        }
    }

    fn search(&mut self, out: &mut Vec<UatFrame>) {
        while self.next + 1 < self.total {
            let i = self.next;
            self.next += 1;
            let ph = (i % 2) as usize;
            let bit = (self.d(i) > 0) as u64;
            self.regs[ph] = ((self.regs[ph] << 1) | bit) & SYNC_MASK;
            self.nbits[ph] = (self.nbits[ph] + 1).min(64);
            if self.nbits[ph] < 36 || i < self.skip_until {
                continue;
            }
            let r = self.regs[ph];
            let down = (r ^ SYNC_DOWN).count_ones() <= 4;
            let up = (r ^ SYNC_UP).count_ones() <= 4;
            if down || up {
                self.pending.push_back((i + 2, up));
            }
            while let Some(&(start, up)) = self.pending.front() {
                let need = start + if up { 4416 * 2 } else { 384 * 2 } + 4;
                if need >= self.total {
                    break;
                }
                self.pending.pop_front();
                if start < self.skip_until {
                    continue;
                }
                if let Some(f) = self.frame(start, up) {
                    let bits = match f.kind {
                        UatKind::Short => 240,
                        UatKind::Long => 384,
                        UatKind::Uplink => 4416,
                    };
                    self.skip_until = start + bits * 2;
                    out.push(f);
                }
            }
        }
    }

    fn frame(&mut self, start: u64, up: bool) -> Option<UatFrame> {
        let sync = if up { SYNC_UP } else { SYNC_DOWN };
        if start < self.base + 74 {
            return None;
        }
        // Threshold from the sync's own bits, at the exact phase.
        let (mut s1, mut n1, mut s0, mut n0) = (0f64, 0, 0f64, 0);
        for k in 0..36 {
            let v = self.d(start - 72 + 2 * k as u64) as f64;
            if sync >> (35 - k) & 1 == 1 {
                s1 += v;
                n1 += 1;
            } else {
                s0 += v;
                n0 += 1;
            }
        }
        let th = 0.5 * (s1 / n1 as f64 + s0 / n0 as f64);
        let mut bad = 0;
        for k in 0..36 {
            let v = self.d(start - 72 + 2 * k as u64) as f64;
            if (v > th) != (sync >> (35 - k) & 1 == 1) {
                bad += 1;
            }
        }
        if bad > 4 {
            return None;
        }
        let nbytes = if up { 552 } else { 48 };
        let slice = |off: u64| -> Vec<u8> {
            let mut v = vec![0u8; nbytes];
            for (b, byte) in v.iter_mut().enumerate() {
                let mut x = 0u8;
                for k in 0..8 {
                    let s = start + off + 2 * (b * 8 + k) as u64;
                    x = x << 1 | ((self.d(s) as f64) > th) as u8;
                }
                *byte = x;
            }
            v
        };
        let raws = [slice(0), slice(1)];
        let mut best: Option<(UatKind, Vec<u8>, usize)> = None;
        for raw in raws {
            let got = if up {
                self.codec.decode_uplink(&raw)
            } else {
                self.codec.decode_downlink(&raw)
            };
            if let Some((kind, bytes, n)) = got {
                if best.as_ref().is_none_or(|b| n < b.2) {
                    best = Some((kind, bytes, n));
                }
            }
        }
        let (kind, bytes, corrected) = best?;
        let nbits = match kind {
            UatKind::Short => 240,
            UatKind::Long => 384,
            UatKind::Uplink => 4416,
        };
        let i0 = (start - self.base) as usize;
        let i1 = (i0 + nbits * 2).min(self.pw.len());
        let p = self.pw[i0..i1].iter().map(|&v| v as f64).sum::<f64>() / (i1 - i0).max(1) as f64;
        Some(UatFrame {
            time: self.epoch + start as f64 / WORK_RATE - self.delay_s,
            kind,
            bytes,
            corrected,
            level_dbfs: 10.0 * p.max(1e-12).log10() as f32,
            noise_dbfs: self.noise_dbfs(),
        })
    }
}

/// The UAT Reed-Solomon codes of `sdrkit-algo-fec`: RS(30,18) for the short downlink frame, RS(48,34) for the long one, and six
/// interleaved RS(92,72) code words for the uplink frame. The decoders hold their scratch, so decoding allocates nothing but the
/// payload it returns.
///
/// Standard: RTCA DO-282B 2.2.3.1 and 2.2.3.2
#[derive(Clone, Debug)]
pub struct UatCodec {
    short: Rs,
    long: Rs,
    uplink: Rs,
    word: Vec<u8>,
}

impl Default for UatCodec {
    fn default() -> Self {
        let rs = |c| Rs::new(c).expect("the UAT codes are valid");
        UatCodec {
            short: rs(codes::UAT_30_18),
            long: rs(codes::UAT_48_34),
            uplink: rs(codes::UAT_92_72),
            word: Vec::with_capacity(552),
        }
    }
}

impl UatCodec {
    /// Decodes a downlink code word (48 bytes as sliced): a long frame when its payload type is not 0, otherwise a short frame
    /// (the first 30 bytes). Returns the kind, the payload and the bytes corrected, or `None` when neither code decodes or `raw`
    /// is shorter than 48 bytes.
    pub fn decode_downlink(&mut self, raw: &[u8]) -> Option<(UatKind, Vec<u8>, usize)> {
        if raw.len() < 48 {
            return None;
        }
        self.word.clear();
        self.word.extend_from_slice(&raw[..48]);
        if let Ok(d) = self.long.decode(&mut self.word, &[]) {
            if self.word[0] >> 3 != 0 {
                return Some((
                    UatKind::Long,
                    self.word[..34].to_vec(),
                    d.corrected as usize,
                ));
            }
        }
        self.word.clear();
        self.word.extend_from_slice(&raw[..30]);
        if let Ok(d) = self.short.decode(&mut self.word, &[]) {
            if self.word[0] >> 3 == 0 {
                return Some((
                    UatKind::Short,
                    self.word[..18].to_vec(),
                    d.corrected as usize,
                ));
            }
        }
        None
    }

    /// Decodes an uplink code word (552 bytes as sliced: six RS(92,72) code words, byte `i` in code word `i mod 6`). Returns the
    /// 432 data bytes and the bytes corrected, or `None` when a code word does not decode or `raw` is shorter than 552 bytes.
    pub fn decode_uplink(&mut self, raw: &[u8]) -> Option<(UatKind, Vec<u8>, usize)> {
        if raw.len() < 552 {
            return None;
        }
        self.word.clear();
        self.word.extend_from_slice(&raw[..552]);
        let d = self
            .uplink
            .decode_interleaved(&mut self.word, 6, &[])
            .ok()?;
        if d.failed != 0 {
            return None;
        }
        let mut out = Vec::with_capacity(432);
        for b in 0..6 {
            out.extend((0..72).map(|i| self.word[i * 6 + b]));
        }
        Some((UatKind::Uplink, out, d.corrected as usize))
    }

    /// The code word of a downlink payload: 18 bytes give a short frame (30 bytes), 34 bytes a long frame (48 bytes). `None` for
    /// any other length.
    pub fn encode_downlink(&self, payload: &[u8]) -> Option<Vec<u8>> {
        let (rs, n) = match payload.len() {
            18 => (&self.short, 30),
            34 => (&self.long, 48),
            _ => return None,
        };
        let mut v = payload.to_vec();
        v.resize(n, 0);
        rs.encode_word(&mut v).ok()?;
        Some(v)
    }

    /// The interleaved code word (552 bytes) of an uplink frame of 432 bytes. `None` for any other length.
    pub fn encode_uplink(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() != 432 {
            return None;
        }
        let mut out = vec![0u8; 552];
        let mut cw = [0u8; 92];
        for b in 0..6 {
            cw[..72].copy_from_slice(&data[b * 72..b * 72 + 72]);
            self.uplink.encode_word(&mut cw).ok()?;
            for (i, &v) in cw.iter().enumerate() {
                out[i * 6 + b] = v;
            }
        }
        Some(out)
    }
}

/// The demodulator as a component.
pub struct UatDemod {
    d: UatDemodulator,
    out: Vec<UatFrame>,
}

impl Default for UatDemod {
    fn default() -> Self {
        UatDemod {
            d: UatDemodulator::new(2e6),
            out: Vec::new(),
        }
    }
}

/// Measurements of the demodulator at a report.
#[derive(Clone, Debug, PartialEq)]
pub struct UatTick {
    /// Signal time.
    pub time: f64,
    /// Noise, dBFS.
    pub noise_dbfs: f32,
}

/// What the demodulator outputs.
#[derive(Clone, Debug)]
pub enum UatOut {
    /// A frame.
    Frame(UatFrame),
    /// A report.
    Tick(UatTick),
}

impl Item for UatOut {
    const KIND: &'static str = "uat.frame";
}

impl Component for UatDemod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "adsb.uat_demod",
            vec![Port::iq("in")],
            vec![Port::typed::<UatOut>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate < 1.9e6 {
            return Err(ConfigError::RateTooLow {
                need: 1.9e6,
                got: f.rate,
            });
        }
        self.d.reset(f.rate);
        Ok(vec![Format::new(Kind::typed::<UatOut>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let iq = io.iq(0);
        let mut out = std::mem::take(&mut self.out);
        out.clear();
        self.d.feed(iq, &mut out);
        let tick = io.tick();
        let o = io.items_out::<UatOut>(0);
        for f in out.drain(..) {
            o.push(Timed {
                time: f.time,
                value: UatOut::Frame(f),
            });
        }
        if tick {
            let t = self.d.time();
            o.push(Timed {
                time: t,
                value: UatOut::Tick(UatTick {
                    time: t,
                    noise_dbfs: self.d.noise_dbfs(),
                }),
            });
        }
        self.out = out;
    }
}
