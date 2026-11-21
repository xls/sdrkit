//! One Orbcomm downlink decoder: the channel at 38.4 ksps, the carrier from the square of the signal, the fine mixer, matched
//! filter and Gardner timing, the differential detector, and frame and packet sync.

use sdrkit_algo_dsp::meter::power_to_db as db;
use super::packet::{check, FRAME_PACKETS, PACKET, SYNC};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::floor::BinMedian;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::timing::{Gardner, MatchedInterp, SymbolSync, SymbolSyncConfig};
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// The matched filter and timing loop.
type Sync = SymbolSync<Complex32, Gardner, MatchedInterp<64, 32>>;

pub const BAUD: f64 = 4800.0;
pub const SPS: f64 = 8.0;
pub const RATE: f64 = BAUD * SPS;
/// The square's spectrum: blocks of this many samples, this many averaged.
pub const SQ_N: usize = 4096;
pub const SQ_BLOCKS: usize = 6;

/// A packet with its place.
#[derive(Clone, Debug)]
pub struct RawPacket {
    pub bytes: [u8; PACKET],
    pub ok: bool,
    /// Seconds since the decoder's start of the samples it came from.
    pub time: f64,
    /// The frame's satellite (from its sync packet).
    pub satellite: Option<u8>,
}

/// One downlink decoder.
pub struct Decoder {
    /// Stream seconds at the decoder's start.
    pub t0: f64,
    /// Offset of the carrier from the input's 0 Hz (coarse, the channel's centre).
    pub centre: f64,
    pub rate_in: f64,
    chan: Channelizer,
    /// 1 acquiring, 2 tracking, 3 frame lock.
    pub state: u8,
    sq_acc: Vec<f32>,
    sq_blocks: usize,
    sq_buf: Vec<Complex32>,
    sq_p3: Vec<f32>,
    sq_floor: BinMedian,
    fft: Fft,
    win: Vec<f32>,
    /// Fine frequency (Hz).
    pub fine: f64,
    nco: Nco,
    sync: Sync,
    syms: Vec<Complex32>,
    prev: Complex32,
    /// Bit history for the sync hunt (newest in bit 0).
    sr: u32,
    /// Frame alignment: bits into the current frame, polarity.
    aligned: Option<(usize, bool)>,
    cur: Vec<u8>,
    byte: u8,
    nbit: usize,
    missed: u32,
    pub frames: u64,
    pub good: u64,
    pub bad: u64,
    pub sync_losses: u64,
    pub satellite: Option<u8>,
    pub tried_pairs: usize,
    pub since_frame: f64,
    pub snr_db: f64,
    pub level_db: f64,
    err: f64,
    /// Recent symbols after the differential detector, for the constellation.
    pub constellation: Vec<[f32; 2]>,
    pub samples: u64,
    tmp: Vec<Complex32>,
}

/// The sync packet's first 24 bits, least significant bit of each byte first.
fn sync_word() -> u32 {
    let mut w = 0u32;
    let mut i = 0;
    for b in SYNC {
        for k in 0..8 {
            if (b >> k) & 1 == 1 {
                w |= 1 << (23 - i);
            }
            i += 1;
        }
    }
    w
}

fn new_sync() -> Sync {
    let mut cfg = SymbolSyncConfig::new(SPS);
    cfg.bn_t = 0.005;
    cfg.pulse = Shape::Rc { beta: 0.4 };
    let interp = MatchedInterp::new(Shape::Rrc { beta: 0.4 }, SPS).expect("a valid RRC pulse");
    SymbolSync::new(Gardner, interp, cfg).expect("a valid timing loop")
}

impl Decoder {
    /// A decoder of the downlink whose carrier is near `centre` Hz from the input's 0 Hz, input at `rate_in`.
    ///
    /// # Errors
    ///
    /// A [`DspError`](sdrkit_algo_dsp::DspError) when the rate is too low for the channel.
    pub fn new(rate_in: f64, centre: f64) -> Result<Self, sdrkit_algo_dsp::DspError> {
        let mut cc = ChannelConfig::new(rate_in, centre, 30e3, RATE);
        cc.exact_rate = true;
        let chan = Channelizer::new(cc)?;
        Ok(Decoder {
            t0: 0.0,
            centre,
            rate_in,
            chan,
            state: 1,
            sq_acc: vec![0.0; SQ_N],
            sq_blocks: 0,
            sq_buf: Vec::with_capacity(SQ_N),
            sq_p3: vec![0.0; SQ_N],
            sq_floor: BinMedian::new((3 * SQ_BLOCKS) as u32)?,
            fft: Fft::forward(SQ_N),
            win: Window::Hann.coefficients(SQ_N),
            fine: 0.0,
            nco: Nco::new(RATE, 0.0),
            sync: new_sync(),
            syms: Vec::with_capacity(4),
            prev: Complex32::new(0.0, 0.0),
            sr: 0,
            aligned: None,
            cur: Vec::with_capacity(PACKET),
            byte: 0,
            nbit: 0,
            missed: 0,
            frames: 0,
            good: 0,
            bad: 0,
            sync_losses: 0,
            satellite: None,
            tried_pairs: 0,
            since_frame: 0.0,
            snr_db: 0.0,
            level_db: -200.0,
            err: 0.0,
            constellation: Vec::with_capacity(256),
            samples: 0,
            tmp: Vec::new(),
        })
    }

    /// The carrier, Hz from the input's 0 Hz.
    pub fn carrier(&self) -> f64 {
        self.centre + self.fine
    }

    /// The symbol clock error, ppm: how much longer a symbol lasts in the receiver's sample clock than nominal (a sample clock
    /// that runs fast against the transmitter's reads positive).
    pub fn clock_ppm(&self) -> f64 {
        self.sync.clock_ppm()
    }

    /// Finds the pair of lines 4800 Hz apart with the most power in the averaged square's spectrum. `tried_pairs` pairs are passed over
    /// (after a carrier that produced no frames, the next is tried).
    fn acquire(&mut self) -> bool {
        let n = SQ_N;
        let bin = RATE / n as f64;
        let k = (4800.0 / bin).round() as usize;
        let p = &self.sq_acc;
        for (i, v) in self.sq_p3.iter_mut().enumerate() {
            *v = p[(i + n - 1) % n] + p[i] + p[(i + 1) % n];
        }
        let p3 = &self.sq_p3;
        let med = self
            .sq_floor
            .estimate(p3.iter().copied())
            .unwrap_or(0.0)
            .max(1e-30);
        // Pairs: i and i + k (both lines), the centre of the pair at i + k/2. Search 2 offset within +-8 kHz.
        let lim = (16e3 / bin) as i64;
        let mut cands: Vec<(f32, i64)> = Vec::new();
        for c in -lim..=lim {
            let a = (c - k as i64 / 2).rem_euclid(n as i64) as usize;
            let b = (c + k as i64 / 2).rem_euclid(n as i64) as usize;
            let s = p3[a].min(p3[b]);
            cands.push((s, c));
        }
        cands.sort_by(|x, y| y.0.total_cmp(&x.0));
        // Local best, skipping near duplicates.
        let mut picked: Vec<(f32, i64)> = Vec::new();
        for c in cands {
            if picked.iter().all(|p| (p.1 - c.1).abs() > 4) {
                picked.push(c);
            }
            if picked.len() > 4 {
                break;
            }
        }
        let Some(&(s, c)) = picked.get(self.tried_pairs % picked.len().max(1)) else {
            return false;
        };
        if db(s as f64 / med as f64) < 8.0 {
            return false;
        }
        // Refine: parabolic on each line.
        let fr = |i: usize| {
            let (a, b, cc) = (
                p3[(i + n - 1) % n].max(1e-30).ln(),
                p3[i].max(1e-30).ln(),
                p3[(i + 1) % n].max(1e-30).ln(),
            );
            let d = a - 2.0 * b + cc;
            if d.abs() < 1e-12 {
                0.0
            } else {
                (0.5 * (a - cc) / d).clamp(-0.5, 0.5) as f64
            }
        };
        let a = (c - k as i64 / 2).rem_euclid(n as i64) as usize;
        let b = (c + k as i64 / 2).rem_euclid(n as i64) as usize;
        let fa = (c - k as i64 / 2) as f64 + fr(a);
        let fb = (c + k as i64 / 2) as f64 + fr(b);
        let two_df = (fa + fb) / 2.0 * bin;
        self.fine = two_df / 2.0;
        self.nco = Nco::new(RATE, -self.fine);
        self.sync = new_sync();
        self.state = 2;
        self.aligned = None;
        self.missed = 0;
        self.since_frame = 0.0;
        true
    }

    fn push_bit(&mut self, bit: bool, t: f64, out: &mut Vec<RawPacket>) {
        self.sr = (self.sr << 1) | bit as u32;
        let w = sync_word();
        match self.aligned {
            None => {
                let e = ((self.sr & 0xFF_FFFF) ^ w).count_ones();
                let ei = ((!self.sr & 0xFF_FFFF) ^ w).count_ones();
                if e <= 2 || ei <= 2 {
                    let inv = ei < e;
                    self.aligned = Some((24, inv));
                    self.cur.clear();
                    self.cur.extend_from_slice(&SYNC);
                    self.nbit = 0;
                    self.byte = 0;
                }
            }
            Some((pos, inv)) => {
                let b = bit ^ inv;
                self.byte |= (b as u8) << self.nbit;
                self.nbit += 1;
                if self.nbit == 8 {
                    self.cur.push(self.byte);
                    self.byte = 0;
                    self.nbit = 0;
                }
                let pos = pos + 1;
                if self.cur.len() == PACKET {
                    let mut bytes = [0u8; PACKET];
                    bytes.copy_from_slice(&self.cur);
                    self.cur.clear();
                    let ok = check(&bytes);
                    let first = pos == 96;
                    if first {
                        if ok {
                            self.satellite = Some(bytes[3]);
                            self.frames += 1;
                            self.since_frame = 0.0;
                            self.state = 3;
                            self.missed = 0;
                        } else if bytes[..3] == SYNC {
                            self.frames += 1;
                            self.since_frame = 0.0;
                        }
                    }
                    if ok {
                        self.good += 1;
                    } else {
                        self.bad += 1;
                    }
                    out.push(RawPacket {
                        bytes,
                        ok,
                        time: t,
                        satellite: self.satellite,
                    });
                }
                if pos == FRAME_PACKETS * PACKET * 8 {
                    // The next frame's sync: check it as it arrives.
                    self.aligned = Some((0, inv));
                } else {
                    self.aligned = Some((pos, inv));
                }
                if pos == 24 {
                    // Just after a frame start: is the sync word there?
                    let got = if inv {
                        !self.sr & 0xFF_FFFF
                    } else {
                        self.sr & 0xFF_FFFF
                    };
                    if (got ^ w).count_ones() > 4 {
                        self.missed += 1;
                        if self.missed >= 3 {
                            self.aligned = None;
                            self.sync_losses += 1;
                            self.state = 2;
                            self.cur.clear();
                        }
                    }
                }
            }
        }
    }

    /// Takes input samples (at the input rate) and appends the packets found to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<RawPacket>) {
        self.tmp.clear();
        self.chan.process(x, &mut self.tmp);
        let tmp = std::mem::take(&mut self.tmp);
        for &v in &tmp {
            self.samples += 1;
            let t = self.samples as f64 / RATE;
            if self.state == 1 {
                self.acquire_sample(v);
                continue;
            }
            let y = v * self.nco.next_sample();
            self.level_db = 0.999 * self.level_db.max(-200.0) + 0.001 * db(y.norm_sqr() as f64);
            self.syms.clear();
            self.sync.process(&[y], &mut self.syms);
            for i in 0..self.syms.len() {
                let s = self.syms[i];
                let d = s * self.prev.conj();
                self.prev = s;
                let mag = d.norm() as f64 + 1e-20;
                // Frequency error: a clean product is at +-90 degrees, so its square at 180.
                let d2 = d * d;
                let e = (-(d2.im as f64)).atan2(-(d2.re as f64)) / 2.0;
                self.fine += 0.02 * e * BAUD / TAU;
                self.nco.set_freq(-self.fine);
                let u = d / mag as f32;
                self.err += 0.01 * (u.re as f64 * u.re as f64 - self.err);
                self.snr_db = db(1.0 / self.err.max(1e-6));
                if self.constellation.len() < 256 {
                    self.constellation.push([u.re, u.im]);
                }
                let bit = d.im > 0.0;
                self.push_bit(bit, t, out);
            }
        }
        self.tmp = tmp;
        self.since_frame += x.len() as f64 / self.rate_in;
        // No frame for 4 s: acquire again on the next pair.
        if self.state >= 2 && self.since_frame > 4.0 {
            self.state = 1;
            self.tried_pairs += 1;
            self.sq_blocks = 0;
            self.sq_buf.clear();
            self.since_frame = 0.0;
        }
    }

    /// One sample while acquiring: the averaged spectrum of the square, then the search for the pair of lines.
    fn acquire_sample(&mut self, v: Complex32) {
        self.sq_buf.push(v * v);
        if self.sq_buf.len() < SQ_N {
            return;
        }
        let mut b = std::mem::take(&mut self.sq_buf);
        for (s, wv) in b.iter_mut().zip(self.win.iter()) {
            *s *= *wv;
        }
        self.fft.process(&mut b);
        // Indexed by FFT bin: 0 Hz at 0.
        for (a, s) in self.sq_acc.iter_mut().zip(b.iter()) {
            *a += s.norm_sqr();
        }
        b.clear();
        self.sq_buf = b;
        self.sq_blocks += 1;
        if self.sq_blocks >= SQ_BLOCKS {
            let ok = self.acquire();
            self.sq_acc.iter_mut().for_each(|a| *a = 0.0);
            self.sq_blocks = 0;
            if !ok {
                self.tried_pairs += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_word_bits() {
        // 0x65 least significant bit first: 1 0 1 0 0 1 1 0.
        assert_eq!(sync_word() >> 16, 0b1010_0110);
    }
}
