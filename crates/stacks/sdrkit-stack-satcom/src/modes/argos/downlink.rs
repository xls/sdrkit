//! The Argos A-DCS downlink: 400 bit/s Manchester BPSK, the carrier from the squared signal (+-14 kHz), a
//! second-order squaring loop of 60 Hz, half-bit sums with timing by the largest differences, and HDLC deframers over three codings
//! (NRZ, inverted NRZ, NRZI).

use crate::modes::sarsat::baseband;
use sdrkit_algo_dsp::channel::Channelizer;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::loopfilter::loop_gains;
use sdrkit_algo_dsp::meter::power_to_db as db;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::{Complex32, ConfigError};
use sdrkit_util_bits::hdlc::HdlcDeframer;
use sdrkit_util_bits::line::Nrzi;
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

pub const RATE: f64 = 64_000.0;
pub const BIT_RATE: f64 = 400.0;
const SQ_N: usize = 16_384;
const SQ_BLOCKS: usize = 4;
const HALF: usize = (RATE / BIT_RATE / 2.0) as usize;

sdrkit_core::choice_enum! {
    /// The line coding of the downlink bits.
    #[derive(Serialize, Deserialize)]
    pub enum Coding {
        /// NRZ.
        Nrz = ("nrz", "NRZ"),
        /// Inverted NRZ.
        InvertedNrz = ("inverted_nrz", "Inverted NRZ"),
        /// NRZI.
        Nrzi = ("nrzi", "NRZI"),
    }
}

/// The codings the deframers try, in order.
pub const CODINGS: [Coding; 3] = [Coding::Nrz, Coding::InvertedNrz, Coding::Nrzi];

/// A frame found.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DlFrame {
    pub time: f64,
    pub coding: Coding,
    pub fcs_ok: bool,
    pub data: std::sync::Arc<[u8]>,
    pub bytes: usize,
}

/// The downlink receiver.
pub struct Downlink {
    mix: Channelizer,
    tmp: Vec<Complex32>,
    pub locked: bool,
    acq: Vec<Complex32>,
    acc: Vec<f32>,
    blocks: usize,
    fft: Fft,
    win: Vec<f32>,
    /// Loop: phase (rad) and frequency (rad/sample) of the carrier.
    phase: f64,
    freq: f64,
    alpha: f64,
    beta: f64,
    pub line_db: f64,
    /// The data axis samples of the last bit (2 half bits per phase).
    ring: Vec<f32>,
    rpos: usize,
    /// Timing: the energy of the half-bit difference per sample phase.
    timing: Vec<f32>,
    n: u64,
    deframers: [HdlcDeframer; 3],
    scratch: Vec<f32>,
    nrzi: Nrzi,
    pub flags: [u64; 3],
    pub frames: Vec<DlFrame>,
    pub fcs_fail: u64,
    pub bits: u64,
    lost: f64,
    pub track: Vec<(f64, f64)>,
    last_track: f64,
    flag_sr: [u8; 3],
    best: usize,
    /// Samples left of the wide pull-in after an acquisition.
    pull: u32,
    wide: (f64, f64),
}

/// The deframer of one coding: ISO/IEC 13239 frames of 1 to 398 bytes with the X.25 FCS, bad ones reported for the count.
fn deframer() -> HdlcDeframer {
    HdlcDeframer::x25(1, 398).deliver_bad(true)
}

impl Downlink {
    /// A receiver of the downlink at `offset` Hz in an input at `rate_in`. Errors when the rate is not positive.
    pub fn new(rate_in: f64, offset: f64) -> Result<Self, ConfigError> {
        let gains = |bn: f64| loop_gains(bn / RATE, std::f64::consts::FRAC_1_SQRT_2, 1.0);
        let (alpha, beta) = gains(60.0)?;
        Ok(Downlink {
            mix: baseband(rate_in, offset, RATE)?,
            tmp: Vec::new(),
            locked: false,
            acq: Vec::with_capacity(SQ_N),
            acc: vec![0.0; SQ_N],
            blocks: 0,
            fft: Fft::forward(SQ_N),
            win: Window::Hann.coefficients(SQ_N),
            phase: 0.0,
            freq: 0.0,
            alpha,
            beta,
            line_db: 0.0,
            ring: vec![0.0; 2 * HALF],
            rpos: 0,
            timing: vec![0.0; 2 * HALF],
            n: 0,
            deframers: [deframer(), deframer(), deframer()],
            scratch: Vec::with_capacity(SQ_N),
            nrzi: Nrzi::new(0),
            flags: [0; 3],
            frames: Vec::new(),
            fcs_fail: 0,
            bits: 0,
            lost: 0.0,
            track: Vec::new(),
            last_track: -1.0,
            flag_sr: [0; 3],
            best: 0,
            pull: 0,
            wide: gains(400.0)?,
        })
    }

    /// The carrier, Hz from the mixer's centre.
    pub fn carrier_hz(&self) -> f64 {
        self.freq * RATE / TAU
    }

    /// The coding that shows flags (the most).
    pub fn coding(&self) -> Option<Coding> {
        let (i, &m) = self.flags.iter().enumerate().max_by_key(|x| *x.1)?;
        (m >= 4).then_some(CODINGS[i])
    }

    fn acquire(&mut self) {
        let n = SQ_N;
        let bin = RATE / n as f64;
        let lim = (28e3 / bin) as i64;
        let med = percentile(&self.acc, 0.5, &mut self.scratch).max(1e-30);
        let mut best = (0i64, 0.0f32);
        for k in -lim..=lim {
            let i = k.rem_euclid(n as i64) as usize;
            let v = self.acc[i];
            if v > best.1 {
                best = (k, v);
            }
        }
        self.line_db = db(best.1 as f64 / med as f64);
        if self.line_db > 15.0 {
            let two = best.0 as f64 * bin;
            self.freq = TAU * (two / 2.0) / RATE;
            self.phase = 0.0;
            self.locked = true;
            self.lost = 0.0;
            self.pull = (0.5 * RATE) as u32;
        }
    }

    /// Input samples; `t0` is the stream time of the first.
    pub fn process(&mut self, x: &[Complex32], t0: f64) {
        self.tmp.clear();
        self.mix.process(x, &mut self.tmp);
        let tmp = std::mem::take(&mut self.tmp);
        for &v in &tmp {
            self.n += 1;
            let t = t0 + self.n as f64 / RATE;
            if !self.locked {
                self.acq.push(v * v);
                if self.acq.len() == SQ_N {
                    let mut b = std::mem::take(&mut self.acq);
                    for (s, w) in b.iter_mut().zip(self.win.iter()) {
                        *s *= *w;
                    }
                    self.fft.process(&mut b);
                    for (a, s) in self.acc.iter_mut().zip(b.iter()) {
                        *a += s.norm_sqr();
                    }
                    b.clear();
                    self.acq = b;
                    self.blocks += 1;
                    if self.blocks == SQ_BLOCKS {
                        self.acquire();
                        self.blocks = 0;
                        self.acc.iter_mut().for_each(|a| *a = 0.0);
                    }
                }
                continue;
            }
            self.phase = (self.phase + self.freq) % TAU;
            let y = v * Complex32::new(self.phase.cos() as f32, -self.phase.sin() as f32);
            // Squaring loop: the error of the doubled phase, halved.
            let y2 = y * y;
            let e = (y2.im as f64).atan2(y2.re as f64) / 2.0;
            let m = (y.norm() as f64).min(1e3);
            let w = if m > 0.0 { 1.0 } else { 0.0 };
            // A wide loop pulls the carrier in after an acquisition, then the 60 Hz loop follows it.
            let (a, b) = if self.pull > 0 {
                self.pull -= 1;
                self.wide
            } else {
                (self.alpha, self.beta)
            };
            self.freq += b * e * w;
            self.phase += a * e * w;
            // The loop holds: |e| small on average.
            self.lost = 0.999 * self.lost + 0.001 * (e.abs() / (PI / 4.0));
            if t - self.last_track >= 0.1 {
                self.last_track = t;
                self.track.push((t, self.carrier_hz()));
            }
            // Data on the real axis; half-bit sums by the sample phase.
            let d = y.re;
            let l = self.ring.len();
            self.ring[self.rpos] = d;
            self.rpos = (self.rpos + 1) % l;
            // At every sample phase p: the difference of the last two half bits.
            let p = (self.n % l as u64) as usize;
            let mut h1 = 0.0f32;
            let mut h2 = 0.0f32;
            for k in 0..HALF {
                h1 += self.ring[(self.rpos + k) % l];
                h2 += self.ring[(self.rpos + HALF + k) % l];
            }
            let diff = h1 - h2;
            self.timing[p] = 0.995 * self.timing[p] + 0.005 * diff.abs();
            if p == 0 {
                self.best = (0..l)
                    .max_by(|&a, &b| self.timing[a].total_cmp(&self.timing[b]))
                    .unwrap_or(0);
            }
            if p == self.best {
                self.bits += 1;
                let bit = diff > 0.0;
                self.deliver(bit, t);
            }
        }
        self.tmp = tmp;
        if self.locked && self.lost > 0.7 {
            self.locked = false;
        }
    }

    fn deliver(&mut self, bit: bool, t: f64) {
        let mut z = [bit as u8];
        self.nrzi.decode(&mut z);
        let coded = [bit as u8, !bit as u8, z[0]];
        for (i, &b) in coded.iter().enumerate() {
            self.flag_sr[i] = (self.flag_sr[i] << 1) | b;
            if self.flag_sr[i] == 0x7E {
                self.flags[i] += 1;
            }
            if let Some(f) = self.deframers[i].push(b) {
                if f.fcs_ok {
                    self.frames.push(DlFrame {
                        time: t,
                        coding: CODINGS[i],
                        fcs_ok: true,
                        data: std::sync::Arc::from(self.deframers[i].frame()),
                        bytes: f.len,
                    });
                } else if f.len >= 2 && self.coding() == Some(CODINGS[i]) {
                    self.fcs_fail += 1;
                }
            }
        }
    }
}
