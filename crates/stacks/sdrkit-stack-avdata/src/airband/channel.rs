//! One airband channel: decimation to about 48 kS/s, the fine shift by the tuning error, the channel filter for its spacing,
//! the carrier search and squelch, the Costas-type loop blended with the envelope, 8 kHz audio and the delayed gate.
//! Standard: ICAO Annex 10 Volume III Part II clause 2.2 (25 kHz and 8.33 kHz channels).

use super::Activity;
use crate::filters::blackman_lowpass;
use sdrkit_algo_dsp::approx::sin_cos;
use sdrkit_algo_dsp::cic::Cic;
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs, Butterworth4};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::DspError;
use sdrkit_core::Complex32;
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// The numbers of a spacing.
#[derive(Clone, Copy, Debug)]
pub struct Spacing {
    /// Pass edge, Hz.
    pub pass: f64,
    /// Stop edge.
    pub stop: f64,
    /// Carrier window, +- Hz.
    pub window: f64,
    /// Tuning tolerance.
    pub tolerance: f64,
    /// Noise bandwidth.
    pub noise_bw: f64,
}

/// 8.33 kHz.
pub const S833: Spacing = Spacing {
    pass: 3400.0,
    stop: 4900.0,
    window: 1000.0,
    tolerance: 700.0,
    noise_bw: 6800.0,
};
/// 25 kHz.
pub const S25: Spacing = Spacing {
    pass: 5000.0,
    stop: 8000.0,
    window: 2500.0,
    tolerance: 2500.0,
    noise_bw: 10_000.0,
};

const NFFT: usize = 256;
const HOP: usize = 128;
/// The hold spectrum's size.
pub const HOLD_N: usize = 1024;

/// What a channel publishes.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ChannelStatus {
    /// Hz.
    pub freq_hz: f64,
    /// Name.
    pub name: String,
    /// Label.
    pub label: String,
    /// 8.33.
    pub is833: bool,
    /// In band.
    pub in_band: bool,
    /// Power in the channel filter, dBFS.
    pub level_db: f32,
    /// C/N, dB.
    pub cn_db: f32,
    /// Carrier offset (tuning error included), Hz.
    pub offset_hz: f32,
    /// Transmissions.
    pub transmissions: u64,
    /// End of the last transmission (or now while open), signal time.
    pub last_s: f64,
    /// Open.
    pub open: bool,
    /// Playing.
    pub playing: bool,
    /// Two stations on the channel.
    pub heterodyne: bool,
    /// Muted.
    pub mute: bool,
    /// Solo.
    pub solo: bool,
    /// Priority.
    pub priority: bool,
    /// Own squelch.
    pub squelch_db: Option<f64>,
}

/// A channel receiver.
pub struct ChannelRx {
    /// Index in the list.
    pub index: usize,
    /// Status.
    pub st: ChannelStatus,
    sp: Spacing,
    nco: Nco,
    cic: Cic<Complex32>,
    fine: Nco,
    fir: FirDecimator<Complex32>,
    fi: f64,
    fc: f64,
    fft: Fft,
    hold_fft: Fft,
    win: Vec<f32>,
    hold_win: Vec<f32>,
    buf: VecDeque<Complex32>,
    hold_buf: Vec<Complex32>,
    /// The averaged 1024-point spectrum at the CIC rate (for the tuning error).
    pub hold: Vec<f32>,
    hold_n: u64,
    since_hop: usize,
    floor: f64,
    detections: u32,
    open_bin: i64,
    last_detect_s: f64,
    start_s: f64,
    best_cn: f32,
    power_acc: f64,
    power_n: u64,
    hang: f64,
    amp: f32,
    // Demodulator.
    ph: f64,
    fr: f64,
    alpha: f64,
    beta: f64,
    quality: f32,
    carrier_sign: f32,
    hp: Biquad,
    lp: Butterworth4,
    interp_t: f64,
    last_a: f32,
    delay: VecDeque<f32>,
    delay_len: usize,
    gate: f32,
    /// The channel's sample count.
    pub n: u64,
    s0: Vec<Complex32>,
    s1: Vec<Complex32>,
    s2: Vec<Complex32>,
    /// 8 kHz audio of the last call.
    pub audio: Vec<f32>,
    /// Finished transmissions.
    pub finished: Vec<Activity>,
    tune_err: f64,
    prom_scratch: Vec<f64>,
    settle: u32,
    spec: Vec<Complex32>,
    /// The channel filter's group delay, output samples.
    fir_delay: u64,
    pending_df: f64,
    pending_at: u64,
}

/// The CIC decimator by `r` to about 48 kS/s: order 5, or a lower order when the register cannot hold order 5 at a full-scale
/// input (order 5 overflows above a factor of 891, an input of about 42.8 MS/s).
pub fn channel_cic(r: usize) -> Result<Cic<Complex32>, DspError> {
    Cic::new(5, r)
        .or_else(|_| Cic::new(4, r))
        .or_else(|_| Cic::new(3, r))
}

impl ChannelRx {
    /// A receiver for the channel at `offset_hz` from the centre, at input rate `rate`. An error when no CIC decimator to about
    /// 48 kS/s fits the integer register.
    pub fn new(
        index: usize,
        st: ChannelStatus,
        rate: f64,
        offset_hz: f64,
        tune_err: f64,
    ) -> Result<Self, DspError> {
        let sp = if st.is833 { S833 } else { S25 };
        let r = ((rate / 48e3).floor() as usize).max(1);
        let fi = rate / r as f64;
        let trans = sp.stop - sp.pass;
        let taps = ((5.5 * fi / trans) as usize).clamp(31, 1201) | 1;
        let d = ((fi / 16e3).floor() as usize).max(1);
        let fc = fi / d as f64;
        let wn = 2.0 * std::f64::consts::PI * 25.0 / fc;
        let zeta = 0.707;
        Ok(ChannelRx {
            index,
            sp,
            nco: Nco::new(rate, -offset_hz),
            cic: channel_cic(r)?,
            fine: Nco::new(fi, -tune_err),
            fir: FirDecimator::new(&blackman_lowpass(fi, (sp.pass + sp.stop) / 2.0, taps), d),
            fi,
            fc,
            fft: Fft::forward(NFFT),
            hold_fft: Fft::forward(HOLD_N),
            win: (0..NFFT)
                .map(|i| (0.5 - 0.5 * (TAU * i as f64 / NFFT as f64).cos()) as f32)
                .collect(),
            hold_win: (0..HOLD_N)
                .map(|i| (0.5 - 0.5 * (TAU * i as f64 / HOLD_N as f64).cos()) as f32)
                .collect(),
            buf: VecDeque::with_capacity(NFFT),
            hold_buf: Vec::with_capacity(HOLD_N),
            hold: vec![0.0; HOLD_N],
            hold_n: 0,
            since_hop: 0,
            floor: 0.0,
            detections: 0,
            open_bin: 0,
            last_detect_s: f64::NEG_INFINITY,
            start_s: 0.0,
            best_cn: -99.0,
            power_acc: 0.0,
            power_n: 0,
            hang: 0.5,
            amp: 0.0,
            ph: 0.0,
            fr: 0.0,
            alpha: 2.0 * zeta * wn,
            beta: wn * wn,
            quality: 0.0,
            carrier_sign: 1.0,
            hp: Biquad::new(BiquadCoeffs::highpass(
                fc,
                300.0,
                std::f64::consts::FRAC_1_SQRT_2,
            )),
            lp: Butterworth4::lowpass(fc, 3000.0),
            interp_t: 0.0,
            last_a: 0.0,
            delay: VecDeque::new(),
            delay_len: (0.060 * 8000.0) as usize,
            gate: 0.0,
            n: 0,
            s0: Vec::new(),
            s1: Vec::new(),
            s2: Vec::new(),
            audio: Vec::new(),
            finished: Vec::new(),
            tune_err,
            prom_scratch: Vec::new(),
            settle: 0,
            fir_delay: ((taps - 1) / 2).div_ceil(d) as u64,
            pending_df: 0.0,
            pending_at: 0,
            spec: Vec::with_capacity(NFFT),
            st,
        })
    }

    /// Whether the hold spectrum has averaged long enough (about 1 s) to show lines.
    pub fn hold_ready(&self) -> bool {
        self.hold_n as f64 * HOLD_N as f64 >= self.fi
    }

    /// The CIC output rate (the hold spectrum's rate).
    pub fn fi(&self) -> f64 {
        self.fi
    }

    /// The tuning tolerance of the channel's spacing.
    pub fn tolerance(&self) -> f64 {
        self.sp.tolerance
    }

    /// A new tuning error.
    pub fn set_tune_err(&mut self, e: f64) {
        // The carrier moves by the change: an open channel follows it.
        let d = e - self.tune_err;
        let binw = self.fc / NFFT as f64;
        self.open_bin -= (d / binw).round() as i64;
        // The loop follows when the shifted samples come out of the channel filter (its group delay later), not before: a 300 Hz step
        // taken 2 ms early turns the carrier by several radians and slips the loop.
        self.pending_df += TAU * d / self.fc;
        self.pending_at = self.n + self.fir_delay;
        self.tune_err = e;
        self.fine.set_freq(-e);
        // A spectrum spanning the step shows the carrier twice: no heterodyne verdict until it has passed.
        self.settle = (NFFT / HOP) as u32;
    }

    /// Takes input samples; `t0` is the signal time of the first; `squelch` the global threshold, `hang` the hang time.
    pub fn feed(&mut self, x: &[Complex32], squelch: f64, hang: f64) {
        self.hang = hang;
        self.audio.clear();
        self.s0.resize(x.len(), Complex32::default());
        self.nco.mix(x, &mut self.s0);
        self.s1.clear();
        for &v in &self.s0 {
            if let Some(y) = self.cic.push(v) {
                self.s1.push(y);
            }
        }
        // The hold spectrum at the CIC rate.
        for &v in &self.s1 {
            self.hold_buf.push(v);
            if self.hold_buf.len() == HOLD_N {
                let mut b = std::mem::take(&mut self.spec);
                b.clear();
                b.extend(
                    self.hold_buf
                        .iter()
                        .zip(&self.hold_win)
                        .map(|(a, w)| a * *w),
                );
                self.hold_fft.process(&mut b);
                // About 15 s of averaging.
                let a = (HOLD_N as f64 / (15.0 * self.fi)).min(1.0) as f32;
                let a = if self.hold_n < 4 {
                    1.0 / (self.hold_n + 1) as f32
                } else {
                    a
                };
                for (h, v) in self.hold.iter_mut().zip(&b) {
                    *h += a * (v.norm_sqr() - *h);
                }
                self.spec = b;
                self.hold_n += 1;
                self.hold_buf.clear();
            }
        }
        let mut s1 = std::mem::take(&mut self.s1);
        self.fine.mix_in_place(&mut s1);
        self.s2.clear();
        self.fir.process(&s1, &mut self.s2);
        self.s1 = s1;
        let s2 = std::mem::take(&mut self.s2);
        for &z in &s2 {
            self.sample(z, squelch);
        }
        self.s2 = s2;
    }

    fn time(&self) -> f64 {
        self.n as f64 / self.fc
    }

    fn sample(&mut self, z: Complex32, squelch: f64) {
        self.n += 1;
        self.power_acc += z.norm_sqr() as f64;
        self.power_n += 1;
        if self.buf.len() == NFFT {
            self.buf.pop_front();
        }
        self.buf.push_back(z);
        self.since_hop += 1;
        if self.since_hop == HOP && self.buf.len() == NFFT {
            self.since_hop = 0;
            self.search(squelch);
        } else if self.since_hop >= HOP {
            self.since_hop = 0;
        }
        if self.pending_df != 0.0 && self.n >= self.pending_at {
            self.fr -= self.pending_df;
            self.pending_df = 0.0;
        }
        // Demodulation: a Costas-type loop blended with the envelope.
        let (sn, cs) = sin_cos(self.ph);
        let rot = Complex32::new(cs, -sn);
        let y = z * rot;
        let err = (y.re * y.im).signum() as f64 * (y.im.abs() / (y.norm() + 1e-12)) as f64;
        self.fr =
            (self.fr + self.beta * err).clamp(-TAU * 3000.0 / self.fc, TAU * 3000.0 / self.fc);
        self.ph = (self.ph + self.fr + self.alpha * err) % TAU;
        let (i2, q2) = (y.re * y.re, y.im * y.im);
        let q = if i2 + q2 > 0.0 {
            (i2 - q2) / (i2 + q2)
        } else {
            0.0
        };
        let k = (1.0 / (0.020 * self.fc)) as f32;
        self.quality += k * (q - self.quality);
        self.carrier_sign += k * (y.re.signum() - self.carrier_sign);
        let w = ((self.quality - 0.3) / 0.4).clamp(0.0, 1.0);
        let env = z.norm();
        let mut a = w * y.re * self.carrier_sign.signum() + (1.0 - w) * env;
        a = self.hp.process_sample(a);
        a = self.lp.process_sample(a);
        // Linear interpolation to 8 kHz.
        let step = 8000.0 / self.fc;
        self.interp_t += step;
        while self.interp_t >= 1.0 {
            self.interp_t -= 1.0;
            let frac = (1.0 - self.interp_t / step).clamp(0.0, 1.0) as f32;
            let v = self.last_a + (a - self.last_a) * frac;
            self.out8k(v);
        }
        self.last_a = a;
    }

    fn out8k(&mut self, v: f32) {
        let g = if self.amp > 0.0 { 0.7 / self.amp } else { 0.0 };
        self.delay.push_back(v * g);
        let target = if self.st.open { 1.0 } else { 0.0 };
        self.gate += 0.1 * (target - self.gate);
        if self.delay.len() > self.delay_len {
            let d = self.delay.pop_front().unwrap_or(0.0);
            let o = (d * self.gate).clamp(-1.5, 1.5);
            self.audio.push(if o.is_finite() { o } else { 0.0 });
        }
    }

    fn search(&mut self, squelch: f64) {
        let mut b = std::mem::take(&mut self.spec);
        b.clear();
        b.extend(self.buf.iter().zip(&self.win).map(|(a, w)| a * *w));
        self.fft.process(&mut b);
        let n = NFFT as i64;
        let binw = self.fc / NFFT as f64;
        let pw = |k: i64| b[k.rem_euclid(n) as usize].norm_sqr() as f64;
        // Noise floor: the median of the pass-band bins / ln 2.
        let pb = (self.sp.pass / binw) as i64;
        self.prom_scratch.clear();
        for k in -pb..=pb {
            self.prom_scratch.push(pw(k));
        }
        let mid = self.prom_scratch.len() / 2;
        self.prom_scratch
            .select_nth_unstable_by(mid, |a, c| a.total_cmp(c));
        let median = self.prom_scratch[mid];
        let est = median / std::f64::consts::LN_2;
        if self.floor == 0.0 || est < self.floor {
            self.floor += if self.floor == 0.0 {
                est
            } else {
                0.3 * (est - self.floor)
            };
        } else if !self.st.open {
            self.floor += 0.004 * (est - self.floor);
        }
        let floor = self.floor.max(1e-30);
        let wb = (self.sp.window / binw).ceil() as i64;
        let (mut best, mut bk) = (0.0, 0i64);
        for k in -wb..=wb {
            let p = pw(k);
            if p > best {
                best = p;
                bk = k;
            }
        }
        let p3 = pw(bk - 1) + pw(bk) + pw(bk + 1);
        let cn = ((p3 - 3.0 * floor) / 256.0) / (floor * self.sp.noise_bw / self.fc);
        let cn_db = 10.0 * cn.max(1e-6).log10();
        let prominence = (p3 / 3.0) / median.max(1e-30);
        let thr = self.st.squelch_db.unwrap_or(squelch) - if self.st.open { 3.0 } else { 0.0 };
        let mut detect = cn_db >= thr && prominence >= 8.0;
        if self.st.open && (bk - self.open_bin).abs() > 2 {
            detect = false;
        }
        let now = self.time();
        self.st.cn_db = cn_db as f32;
        self.st.level_db = (10.0
            * (self.power_acc / self.power_n.max(1) as f64)
                .max(1e-20)
                .log10()) as f32;
        self.power_acc = 0.0;
        self.power_n = 0;
        // Heterodyne: another local maximum 4+ bins away, over the threshold, within 20 dB.
        let mut het = false;
        let settling = self.settle > 0;
        self.settle = self.settle.saturating_sub(1);
        if detect && !settling {
            for k in -wb..=wb {
                if (k - bk).abs() < 4 {
                    continue;
                }
                let p = pw(k);
                if p > pw(k - 1) && p > pw(k + 1) {
                    let q3 = pw(k - 1) + p + pw(k + 1);
                    let c2 = ((q3 - 3.0 * floor) / 256.0) / (floor * self.sp.noise_bw / self.fc);
                    // Modulation sidebands come in mirrored pairs around the carrier. A second station does not.
                    let m = 2 * bk - k;
                    let mirror = (m - 1..=m + 1).map(pw).fold(0.0f64, f64::max);
                    let sideband = mirror > p / 4.0 && mirror < p * 4.0;
                    if 10.0 * c2.max(1e-6).log10() >= thr && q3 >= p3 / 100.0 && !sideband {
                        het = true;
                    }
                }
            }
        }
        if detect {
            // Parabolic interpolation of the carrier.
            let (a, c, e) = (
                pw(bk - 1).max(1e-30).ln(),
                pw(bk).max(1e-30).ln(),
                pw(bk + 1).max(1e-30).ln(),
            );
            let d = 0.5 * (a - e) / (a - 2.0 * c + e);
            let fpk = (bk as f64
                + if d.is_finite() {
                    d.clamp(-0.5, 0.5)
                } else {
                    0.0
                })
                * binw;
            let amp = (p3.sqrt() / (NFFT as f64 / 2.0)) as f32;
            if !self.st.open {
                if self.detections > 0 && (bk - self.open_bin).abs() <= 2 {
                    self.detections += 1;
                } else {
                    self.detections = 1;
                    self.open_bin = bk;
                    self.start_s = now - NFFT as f64 / self.fc + 0.25 * NFFT as f64 / self.fc;
                }
                if self.detections >= 2 {
                    self.st.open = true;
                    self.st.heterodyne = false;
                    self.best_cn = -99.0;
                    self.amp = amp;
                    self.ph = 0.0;
                    self.fr = TAU * fpk / self.fc;
                }
            }
            if self.st.open {
                self.amp += 0.3 * (amp - self.amp);
                self.last_detect_s = now;
                self.best_cn = self.best_cn.max(cn_db as f32);
                self.st.heterodyne |= het;
                self.st.offset_hz = (fpk + self.tune_err) as f32;
                // Re-seed the loop at the carrier when more than half a bin away.
                let lf = self.fr * self.fc / TAU;
                if (lf - fpk).abs() > binw / 2.0 && !het && !settling {
                    self.fr = TAU * fpk / self.fc;
                }
                self.st.last_s = now;
            }
        } else if !self.st.open {
            self.detections = 0;
        }
        if self.st.open && now - self.last_detect_s > self.hang {
            self.st.open = false;
            self.detections = 0;
            self.st.transmissions += 1;
            self.st.last_s = self.last_detect_s;
            self.finished.push(Activity {
                channel: self.index,
                name: self.st.name.clone(),
                label: self.st.label.clone(),
                freq_hz: self.st.freq_hz,
                start_s: self.start_s,
                dur_s: self.last_detect_s - self.start_s,
                cn_db: self.best_cn,
                level_db: self.st.level_db,
                heterodyne: self.st.heterodyne,
            });
        }
        self.spec = b;
    }

    /// The signal time of the start of the transmission now open.
    pub fn open_since(&self) -> Option<f64> {
        self.st.open.then_some(self.start_s)
    }

    /// The best C/N of the transmission now open.
    pub fn best_cn(&self) -> f32 {
        self.best_cn
    }
}
