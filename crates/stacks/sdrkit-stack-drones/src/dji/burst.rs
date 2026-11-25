//! Wideband burst detection and measurement: the power envelope in 10 us bins, the noise floor, the burst
//! detector, the sample history, the idle spectrum, and the analysis of each burst into the signals it holds (Welch spectrum,
//! clusters, resampling to 15.36 or 30.72 Msps, envelope, OFDM-likeness).

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::meter::power_to_db as db;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::SpanResampler;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;
use std::sync::Arc;

const NSPEC: usize = 1024;

/// The share of the power of a burst inside its own band, so that the level of a burst leaves out a signal next to it.
pub struct BandShare {
    fft: Fft,
    hann: Vec<f32>,
    buf: Vec<Complex32>,
}

impl Default for BandShare {
    fn default() -> Self {
        BandShare {
            fft: Fft::forward(NSPEC),
            hann: (0..NSPEC)
                .map(|i| Window::Hann.value(i, NSPEC) as f32)
                .collect(),
            buf: Vec::with_capacity(NSPEC),
        }
    }
}

impl BandShare {
    /// The share of the power of `x` (at `rate`) within `half_hz` of 0 Hz: Hann frames of 1024 points with half
    /// overlap, summed with equal weight. 1 when `x` is shorter than one frame or holds no power.
    pub fn of(&mut self, x: &[Complex32], rate: f64, half_hz: f64) -> f64 {
        let df = rate / NSPEC as f64;
        let (mut inside, mut all) = (0.0f64, 0.0f64);
        let mut p = 0;
        while p + NSPEC <= x.len() {
            self.buf.clear();
            self.buf
                .extend(x[p..p + NSPEC].iter().zip(&self.hann).map(|(v, w)| v * w));
            self.fft.process(&mut self.buf);
            for (k, v) in self.buf.iter().enumerate() {
                let bin = if k < NSPEC / 2 {
                    k as f64
                } else {
                    k as f64 - NSPEC as f64
                };
                let e = f64::from(v.norm_sqr());
                all += e;
                if (bin * df).abs() <= half_hz {
                    inside += e;
                }
            }
            p += NSPEC / 2;
        }
        if all > 0.0 {
            inside / all
        } else {
            1.0
        }
    }
}

/// One signal inside a burst.
#[derive(Clone, Debug)]
pub struct BurstSignal {
    /// Offset from the capture centre, Hz.
    pub offset_hz: f64,
    /// Occupied width, Hz.
    pub width_hz: f64,
    /// The cluster touches the edge of the capture, so the signal can be wider.
    pub clipped: bool,
    /// Start, seconds of signal since configure.
    pub start_s: f64,
    /// Duration, seconds.
    pub duration_s: f64,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// SNR, dB.
    pub snr_db: f64,
    /// OFDM-likeness (0..1).
    pub ofdm: f64,
    /// Another signal was on during this burst.
    pub crowded: bool,
    /// The signal alone at `rate`, centred on 0 Hz.
    pub samples: Arc<Vec<Complex32>>,
    /// The rate of `samples` (15.36 or 30.72 Msps).
    pub rate: f64,
}

/// Counters and levels of the detector.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DetectorStats {
    /// The noise floor, dBFS per sample.
    pub floor_dbfs: Option<f64>,
    /// Bursts measured (signals).
    pub measured: u64,
    /// Of them OFDM-like.
    pub ofdm_like: u64,
    /// Bursts detected in time.
    pub spans: u64,
    /// Idle spectrum blocks averaged.
    pub idle_blocks: u64,
    /// The strongest burst level in the last report period (dBFS), else the floor.
    pub level_dbfs: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
struct Open {
    start_bin: u64,
    quiet: u32,
    sum: f64,
    bins: u64,
}

/// The detector and analyser.
pub struct BurstAnalyser {
    rate: f64,
    bin_len: usize,
    hist: Vec<Complex32>,
    n: u64,
    acc: f64,
    acc_n: usize,
    bin: u64,
    seed: Vec<f64>,
    floor: Option<f64>,
    open: Option<Open>,
    pending: std::collections::VecDeque<(u64, u64)>,
    idle: Vec<f64>,
    idle_n: u64,
    idle_next: u64,
    fft: Fft,
    hann: Vec<f32>,
    hann_pow: f64,
    stats: DetectorStats,
    period_max: Option<f64>,
    scratch: Vec<Complex32>,
    mixed: Vec<Complex32>,
    /// From `rate` to 15.36 and to 30.72 Msps; `None` where the resampler refuses the rates.
    resamplers: [Option<SpanResampler>; 2],
    share: BandShare,
    out: Vec<BurstSignal>,
}

impl BurstAnalyser {
    /// For input at `rate`.
    pub fn new(rate: f64) -> Self {
        let bin_len = ((rate * 10e-6).round() as usize).max(32);
        let hlen = (rate * 12e-3) as usize + 4096 + NSPEC;
        let hann: Vec<f32> = (0..NSPEC)
            .map(|i| Window::Hann.value(i, NSPEC) as f32)
            .collect();
        let hann_pow = hann.iter().map(|w| (w * w) as f64).sum();
        BurstAnalyser {
            rate,
            bin_len,
            hist: vec![Complex32::new(0.0, 0.0); hlen],
            n: 0,
            acc: 0.0,
            acc_n: 0,
            bin: 0,
            seed: Vec::with_capacity(64),
            floor: None,
            open: None,
            pending: std::collections::VecDeque::new(),
            idle: vec![0.0; NSPEC],
            idle_n: 0,
            idle_next: (rate * 0.5e-3) as u64,
            fft: Fft::forward(NSPEC),
            hann,
            hann_pow,
            stats: DetectorStats::default(),
            period_max: None,
            scratch: Vec::new(),
            mixed: Vec::new(),
            resamplers: [15.36e6, 30.72e6].map(|rs| SpanResampler::new(rate, rs).ok()),
            share: BandShare::default(),
            out: Vec::new(),
        }
    }

    /// The counters; the level of the period restarts.
    pub fn stats(&mut self) -> DetectorStats {
        let mut s = self.stats.clone();
        s.floor_dbfs = self.floor.map(db);
        s.idle_blocks = self.idle_n;
        s.level_dbfs = self.period_max.or(s.floor_dbfs);
        self.period_max = None;
        s
    }

    /// Restarts after a retune: floor, idle spectrum and open bursts start again (counters stay).
    pub fn restart(&mut self) {
        let stats = std::mem::take(&mut self.stats);
        *self = BurstAnalyser::new(self.rate);
        self.stats = stats;
    }

    #[inline]
    fn h(&self, n: u64) -> Complex32 {
        self.hist[(n % self.hist.len() as u64) as usize]
    }

    /// Feeds samples; the signals measured are appended to `out`.
    pub fn feed(&mut self, x: &[Complex32], out: &mut Vec<BurstSignal>) {
        let hl = self.hist.len() as u64;
        for &v in x {
            let v = if v.re.is_finite() && v.im.is_finite() {
                v
            } else {
                Complex32::new(0.0, 0.0)
            };
            self.hist[(self.n % hl) as usize] = v;
            self.n += 1;
            self.acc += v.norm_sqr() as f64;
            self.acc_n += 1;
            if self.acc_n == self.bin_len {
                let p = self.acc / self.bin_len as f64;
                self.acc = 0.0;
                self.acc_n = 0;
                self.take_bin(p);
            }
            if self.n >= self.idle_next {
                self.idle_next = self.n + (self.rate * 0.5e-3) as u64;
                self.idle_block();
            }
            if let Some(&(_, e)) = self.pending.front() {
                if self.n >= e + (self.rate * 120e-6) as u64 {
                    if let Some((s, e)) = self.pending.pop_front() {
                        self.analyse(s, e);
                    }
                }
            }
        }
        out.append(&mut self.out);
    }

    fn take_bin(&mut self, p: f64) {
        let b = self.bin;
        self.bin += 1;
        let Some(floor) = self.floor else {
            self.seed.push(p);
            if self.seed.len() == 64 {
                let mut s = self.seed.clone();
                s.sort_by(|a, b| a.total_cmp(b));
                let m = s[..32].iter().sum::<f64>() / 32.0;
                self.floor = Some((1.1 * m).max(1e-20));
            }
            return;
        };
        match self.open.as_mut() {
            None => {
                if p > 2.5 * floor {
                    self.open = Some(Open {
                        start_bin: b.saturating_sub(1),
                        quiet: 0,
                        sum: p,
                        bins: 1,
                    });
                } else {
                    let a = if p < 0.6 * floor {
                        0.2
                    } else if p < 1.6 * floor {
                        0.01
                    } else {
                        1e-4
                    };
                    self.floor = Some(floor + a * (p - floor));
                }
            }
            Some(o) => {
                o.sum += p;
                o.bins += 1;
                if p < 1.8 * floor {
                    o.quiet += 1;
                } else {
                    o.quiet = 0;
                }
                let o = *o;
                if o.quiet >= 3 {
                    // Ends 3 bins before here: at the first quiet bin.
                    let end_bin = b + 1 - 3;
                    self.open = None;
                    if end_bin > o.start_bin + 2 {
                        self.close(o.start_bin, end_bin);
                    }
                } else if (b + 1 - o.start_bin) as f64 * self.bin_len as f64 / self.rate > 5e-3 {
                    // A continuous signal: closed, and the floor raised under it.
                    self.open = None;
                    let mean = o.sum / o.bins as f64;
                    self.floor = Some(floor.max(0.9 * mean));
                    self.close(o.start_bin, b + 1);
                }
            }
        }
    }

    fn close(&mut self, start_bin: u64, end_bin: u64) {
        self.stats.spans += 1;
        let s = start_bin * self.bin_len as u64;
        let e = end_bin * self.bin_len as u64;
        if self.pending.len() < 32 {
            self.pending.push_back((s, e));
        }
    }

    fn idle_block(&mut self) {
        let Some(floor) = self.floor else {
            return;
        };
        if self.open.is_some() || !self.pending.is_empty() || self.n < NSPEC as u64 {
            return;
        }
        let start = self.n - NSPEC as u64;
        let mut p = 0.0;
        for k in 0..NSPEC as u64 {
            p += self.h(start + k).norm_sqr() as f64;
        }
        if p / NSPEC as f64 > 1.5 * floor {
            return;
        }
        let mut buf = std::mem::take(&mut self.scratch);
        buf.clear();
        for k in 0..NSPEC {
            buf.push(self.h(start + k as u64) * self.hann[k]);
        }
        self.fft.process(&mut buf);
        self.idle_n += 1;
        let a = if self.idle_n <= 16 {
            1.0 / self.idle_n as f64
        } else {
            0.05
        };
        for (i, v) in buf.iter().enumerate() {
            let q = v.norm_sqr() as f64 / self.hann_pow;
            self.idle[i] += a * (q - self.idle[i]);
        }
        self.scratch = buf;
    }

    fn analyse(&mut self, s: u64, e: u64) {
        if self.idle_n < 8 {
            return;
        }
        let margin = (self.rate * 120e-6) as u64;
        let hl = self.hist.len() as u64;
        let a = s.saturating_sub(margin).max(self.n.saturating_sub(hl - 1));
        let b = (e + margin).min(self.n);
        if b <= a + 64 {
            return;
        }
        let span: Vec<Complex32> = (a..b).map(|k| self.h(k)).collect();
        let l = span.len();
        // Welch spectrum.
        let mut positions: Vec<usize> = if l <= NSPEC {
            vec![0]
        } else {
            (0..=(l - NSPEC)).step_by(NSPEC / 2).collect()
        };
        if positions.len() > 48 {
            let m = positions.len();
            positions = (0..48).map(|i| positions[i * (m - 1) / 47]).collect();
        }
        let centre = ((s - a + (e - s) / 2) as usize)
            .saturating_sub(NSPEC / 2)
            .min(l.saturating_sub(NSPEC));
        if !positions.contains(&centre) && l > NSPEC {
            positions.push(centre);
        }
        let mut psd = vec![0f64; NSPEC];
        let mut buf = std::mem::take(&mut self.scratch);
        for &p in &positions {
            buf.clear();
            for k in 0..NSPEC {
                let v = if p + k < l {
                    span[p + k]
                } else {
                    Complex32::new(0.0, 0.0)
                };
                buf.push(v * self.hann[k]);
            }
            self.fft.process(&mut buf);
            for (i, v) in buf.iter().enumerate() {
                psd[i] += v.norm_sqr() as f64 / self.hann_pow;
            }
        }
        self.scratch = buf;
        let np = positions.len() as f64;
        // Subtract the idle spectrum, put DC in the middle, smooth over 17 bins.
        let mut sh = vec![0f64; NSPEC];
        for i in 0..NSPEC {
            let v = (psd[i] / np - self.idle[i]).max(0.0);
            sh[(i + NSPEC / 2) % NSPEC] = v;
        }
        let mut sm = vec![0f64; NSPEC];
        for (i, v) in sm.iter_mut().enumerate() {
            let lo = i.saturating_sub(8);
            let hi = (i + 8).min(NSPEC - 1);
            *v = sh[lo..=hi].iter().sum::<f64>() / (hi - lo + 1) as f64;
        }
        let mut sorted = sm.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let thr = 0.1 * sorted[(0.97 * (NSPEC - 1) as f64) as usize];
        if thr <= 0.0 {
            return;
        }
        // Clusters, bridging gaps of up to 24 bins.
        let mut clusters: Vec<(usize, usize, f64)> = Vec::new();
        let mut i = 0;
        while i < NSPEC {
            if sm[i] > thr {
                let st = i;
                let mut en = i;
                let mut gap = 0;
                let mut j = i + 1;
                while j < NSPEC {
                    if sm[j] > thr {
                        en = j;
                        gap = 0;
                    } else {
                        gap += 1;
                        if gap > 24 {
                            break;
                        }
                    }
                    j += 1;
                }
                let sum = sm[st..=en].iter().sum();
                clusters.push((st, en, sum));
                i = en + 1;
            } else {
                i += 1;
            }
        }
        clusters.sort_by(|a, b| b.2.total_cmp(&a.2));
        clusters.truncate(3);
        let df = self.rate / NSPEC as f64;
        let wide: Vec<(usize, usize, f64)> = clusters
            .into_iter()
            .filter(|c| (c.1 - c.0 + 1) as f64 * df >= 3e6)
            .collect();
        let crowded = wide.len() > 1;
        let t0 = a as f64 / self.rate;
        for (st, en, _) in &wide {
            let width = (en - st + 1) as f64 * df;
            let offset = ((st + en) as f64 / 2.0 - (NSPEC / 2) as f64) * df;
            let clipped = *st < 6 || *en >= NSPEC - 6;
            if let Some(sig) = self.measure(&span, t0, offset, width, clipped, crowded) {
                self.stats.measured += 1;
                if sig.ofdm >= if width > 12e6 { 0.6 } else { 0.35 } {
                    self.stats.ofdm_like += 1;
                }
                self.period_max = Some(
                    self.period_max
                        .map_or(sig.level_dbfs, |m: f64| m.max(sig.level_dbfs)),
                );
                self.out.push(sig);
            }
        }
    }

    fn measure(
        &mut self,
        span: &[Complex32],
        t0: f64,
        offset: f64,
        width: f64,
        clipped: bool,
        crowded: bool,
    ) -> Option<BurstSignal> {
        let (rs, pass): (f64, f64) = if width <= 12e6 {
            (15.36e6, 7e6)
        } else {
            (30.72e6, (0.46 * self.rate).min(14e6))
        };
        // Mix to 0 Hz.
        let _ = pass;
        self.mixed.clear();
        self.mixed.resize(span.len(), Complex32::default());
        Nco::new(self.rate, -offset).mix(span, &mut self.mixed);
        let mut y = Vec::new();
        if let Some(r) = &mut self.resamplers[usize::from(width > 12e6)] {
            r.run(&self.mixed, &mut y);
        }
        // Envelope in 10 us steps.
        let step = (rs * 10e-6).round() as usize;
        let env: Vec<f64> = y
            .chunks(step)
            .filter(|c| c.len() == step)
            .map(|c| c.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / step as f64)
            .collect();
        if env.len() < 3 {
            return None;
        }
        let mut sorted = env.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let quiet = sorted[(sorted.len() - 1) / 6].max(1e-30);
        let top = &sorted[sorted.len() - sorted.len() / 3..];
        let loud = top.iter().sum::<f64>() / top.len().max(1) as f64;
        if loud < 2.0 * quiet {
            return None;
        }
        let thr = quiet + 0.35 * (loud - quiet);
        let mut best = (0usize, 0usize);
        let mut i = 0;
        while i < env.len() {
            if env[i] > thr {
                let st = i;
                let mut en = i;
                let mut gap = 0;
                let mut j = i + 1;
                while j < env.len() {
                    if env[j] > thr {
                        en = j;
                        gap = 0;
                    } else {
                        gap += 1;
                        if gap > 2 {
                            break;
                        }
                    }
                    j += 1;
                }
                if en + 1 - st > best.1 - best.0 {
                    best = (st, en + 1);
                }
                i = en + 1;
            } else {
                i += 1;
            }
        }
        let steps = best.1 - best.0;
        if (steps as f64) * 10e-6 < 20e-6 {
            return None;
        }
        let run = &env[best.0..best.1];
        // The resampled band is wider than the signal: a signal next to it would add to the level.
        let inside = self
            .share
            .of(&y[best.0 * step..best.1 * step], rs, 0.5 * width);
        let level = db(inside * run.iter().sum::<f64>() / run.len() as f64);
        let snr = db((loud - quiet).max(quiet * 1e-3) / quiet);
        // Refine the edges to the sample: where the 10 us envelope crosses into the run.
        let start_sample = best.0 * step;
        let ofdm = ofdm_likeness(&y, rs, start_sample);
        Some(BurstSignal {
            offset_hz: offset,
            width_hz: width,
            clipped,
            start_s: t0 + start_sample as f64 / rs,
            duration_s: steps as f64 * 10e-6,
            level_dbfs: level,
            snr_db: snr,
            ofdm,
            crowded,
            samples: Arc::new(y),
            rate: rs,
        })
    }
}

/// OFDM-likeness: the cyclic-prefix correlation for 15 kHz spacing (FFT `rate / 15 kHz`, prefix 72/1024 of it) and for 30 kHz
/// (half of both), the best over timing offsets, over up to 8 consecutive symbols from 400 samples before `start`.
pub fn ofdm_likeness(y: &[Complex32], rate: f64, start: usize) -> f64 {
    let n0 = (rate / 15e3).round() as usize;
    let l0 = ((n0 as f64) * 72.0 / 1024.0).round() as usize;
    let mut best = 0.0f64;
    for (n, l) in [(n0, l0), (n0 / 2, (l0 / 2).max(2))] {
        let from = start.saturating_sub(400);
        let per = n + l;
        let end = (from + 16 * per + n + l).min(y.len());
        if end <= from + n + l {
            continue;
        }
        let len = end - from;
        // Prefix sums of the lag-n products and energies.
        let mut pc = vec![Complex32::new(0.0, 0.0); len + 1];
        let mut pe = vec![0f64; len + 1];
        for i in 0..len {
            let a = y[from + i];
            let (p, e) = if from + i + n < y.len() {
                let b = y[from + i + n];
                (a * b.conj(), 0.5 * (a.norm_sqr() + b.norm_sqr()) as f64)
            } else {
                (Complex32::new(0.0, 0.0), 0.0)
            };
            pc[i + 1] = pc[i] + p;
            pe[i + 1] = pe[i] + e;
        }
        for tau in 0..2 * per {
            let mut num = 0.0f64;
            let mut den = 0.0f64;
            let mut syms = 0;
            for j in 0..8 {
                let a = tau + j * per;
                let b = a + l;
                if b + n > len {
                    break;
                }
                num += (pc[b] - pc[a]).norm() as f64;
                den += pe[b] - pe[a];
                syms += 1;
            }
            if syms >= 3 && den > 0.0 {
                best = best.max(num / den);
            }
        }
    }
    best.min(1.0)
}
