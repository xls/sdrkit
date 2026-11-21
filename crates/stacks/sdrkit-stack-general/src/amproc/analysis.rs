//! The analysis of the AM processor every 4096 samples: the spectra of the two sidebands, the noise floor, the audio edges,
//! interference, the sideband choice, the automatic bandwidth, the programme SNR, whistles and hum.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;

/// Analysis length.
pub const N: usize = 4096;
const HALF: usize = N / 2;

/// The side listened to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    /// Both sidebands.
    #[default]
    Both,
    /// The upper only.
    Upper,
    /// The lower only.
    Lower,
}

#[derive(Clone, Copy, Debug)]
struct Whistle {
    bin: f32,
    seen: u32,
    missed: u32,
    unseen: u32,
    seen_now: bool,
}

/// What an analysis found.
#[derive(Clone, Debug, Default)]
pub struct Findings {
    /// The noise floor (power per bin).
    pub floor: f32,
    /// The audio edge of each side (Hz): upper, lower.
    pub edges: (f64, f64),
    /// Interference share of each side: upper, lower.
    pub interference: (f32, f32),
    /// Where interference starts on each side (Hz), when it does.
    pub interference_start: (Option<f64>, Option<f64>),
    /// The automatic bandwidth (Hz).
    pub auto_bandwidth: f64,
    /// The programme SNR (dB).
    pub snr_db: f32,
    /// The whistles to notch (Hz).
    pub whistles: Vec<f64>,
    /// 50 or 60 when hum is to be notched.
    pub hum: Option<u32>,
    /// Valid (from the third frame on).
    pub valid: bool,
}

/// The analysis state.
pub struct Analysis {
    rate: f64,
    fft: Fft,
    win: Vec<f32>,
    frame: Vec<Complex32>,
    fill: usize,
    short_u: Vec<f32>,
    short_l: Vec<f32>,
    long_u: Vec<f32>,
    long_l: Vec<f32>,
    frames: u32,
    /// The side chosen by the automatic sideband choice.
    pub side: Side,
    pending_side: Side,
    pending_count: u32,
    tracks: Vec<Whistle>,
    scratch: Vec<f32>,
    both: Vec<f32>,
    /// The latest findings.
    pub findings: Findings,
}

impl Analysis {
    /// At `rate`.
    pub fn new(rate: f64) -> Self {
        Analysis {
            rate,
            fft: Fft::forward(N),
            win: Window::Hann.coefficients(N),
            frame: vec![Complex32::default(); N],
            fill: 0,
            short_u: vec![0.0; HALF],
            short_l: vec![0.0; HALF],
            long_u: vec![0.0; HALF],
            long_l: vec![0.0; HALF],
            frames: 0,
            side: Side::Both,
            pending_side: Side::Both,
            pending_count: 0,
            tracks: Vec::with_capacity(16),
            scratch: Vec::with_capacity(N),
            both: Vec::with_capacity(N),
            findings: Findings {
                auto_bandwidth: 4500.0,
                ..Default::default()
            },
        }
    }
    fn bin(&self, hz: f64) -> usize {
        ((hz * N as f64 / self.rate).round() as usize).min(HALF - 1)
    }
    /// One sample of `r`. Returns true when an analysis completed. `bandwidth` is the bandwidth in use (for the SNR and whistles).
    #[inline]
    pub fn push(&mut self, r: Complex32, bandwidth: f64) -> bool {
        self.frame[self.fill] = r * self.win[self.fill];
        self.fill += 1;
        if self.fill < N {
            return false;
        }
        self.fill = 0;
        self.analyse(bandwidth);
        true
    }
    fn analyse(&mut self, bandwidth: f64) {
        self.fft.process(&mut self.frame);
        self.frames += 1;
        let ws = if self.frames >= 4 {
            0.25
        } else {
            1.0 / self.frames as f32
        };
        let wl = if self.frames >= 48 {
            0.02
        } else {
            1.0 / self.frames as f32
        };
        for k in 1..HALF {
            let u = self.frame[k].norm_sqr();
            let l = self.frame[N - k].norm_sqr();
            self.short_u[k] += ws * (u - self.short_u[k]);
            self.short_l[k] += ws * (l - self.short_l[k]);
            self.long_u[k] += wl * (u - self.long_u[k]);
            self.long_l[k] += wl * (l - self.long_l[k]);
        }
        if self.frames < 3 {
            return;
        }
        let (k300, k11k) = (self.bin(300.0).max(1), self.bin(11_000.0));
        let kb = self.bin(bandwidth).max(k300);
        let (k150, kbw) = (self.bin(150.0).max(27), self.bin(bandwidth).min(HALF - 28));
        let hum_bins: [[usize; 4]; 2] =
            [50.0, 60.0].map(|b: f64| [1, 2, 3, 4].map(|h| self.bin(b * h as f64)));
        let f = &mut self.findings;
        f.valid = true;
        // Noise floor: the 20th percentile of both sides' short averages, 300 Hz to 11 kHz.
        self.both.clear();
        self.both.extend_from_slice(&self.short_u[k300..=k11k]);
        self.both.extend_from_slice(&self.short_l[k300..=k11k]);
        let floor = percentile(&self.both, 0.2, &mut self.scratch).max(1e-30);
        f.floor = floor;
        // Audio edges.
        let rate = self.rate;
        let edge_of = |s: &[f32]| -> f64 {
            let w = ((400.0 * N as f64 / rate).round() as usize).max(1);
            let start = (1500.0 * N as f64 / rate).round() as usize;
            let stop_gap = (700.0 * N as f64 / rate).round() as usize;
            let mut edge = 1500.0;
            let mut since = 0usize;
            let mut k = start;
            while k + w / 2 < HALF - 1 {
                let lo = k.saturating_sub(w / 2);
                let hi = (k + w / 2).min(HALF - 1);
                let mean = s[lo..=hi].iter().sum::<f32>() / (hi - lo + 1) as f32;
                if mean > 2.0 * floor {
                    edge = k as f64 * rate / N as f64;
                    since = 0;
                } else {
                    since += 1;
                    if since > stop_gap {
                        break;
                    }
                }
                k += 1;
            }
            edge
        };
        let eu = edge_of(&self.short_u);
        let el = edge_of(&self.short_l);
        f.edges = (eu, el);
        // Interference (long averages, 300 Hz to the wider edge).
        let kw = ((eu.max(el) * N as f64 / rate).round() as usize).min(HALF - 1);
        let interf = |a: &[f32], b: &[f32]| -> (f32, Option<f64>) {
            let (mut tot, mut bad) = (0.0f32, 0.0f32);
            let (mut run, mut start) = (0usize, None);
            for k in k300..=kw.max(k300) {
                tot += a[k];
                let is_bad = a[k] > 4.0 * b[k] && a[k] > 4.0 * floor;
                if is_bad {
                    bad += a[k];
                    run += 1;
                    if run == 20 && start.is_none() {
                        start = Some((k + 1 - 20) as f64 * rate / N as f64);
                    }
                } else {
                    run = 0;
                }
            }
            (if tot > 0.0 { bad / tot } else { 0.0 }, start)
        };
        let (iu, su) = interf(&self.long_u, &self.long_l);
        let (il, sl) = interf(&self.long_l, &self.long_u);
        f.interference = (iu, il);
        f.interference_start = (su, sl);
        // Sideband choice, 3 analyses in a row.
        let want = match self.side {
            Side::Lower if il <= 0.1 && iu > 0.1 => Side::Lower,
            Side::Upper if iu <= 0.1 && il > 0.1 => Side::Upper,
            _ => {
                if iu > 0.25 && iu > 3.0 * il {
                    Side::Lower
                } else if il > 0.25 && il > 3.0 * iu {
                    Side::Upper
                } else {
                    Side::Both
                }
            }
        };
        if want != self.side {
            if want == self.pending_side {
                self.pending_count += 1;
            } else {
                self.pending_side = want;
                self.pending_count = 1;
            }
            if self.pending_count >= 3 {
                self.side = want;
                self.pending_count = 0;
            }
        } else {
            self.pending_count = 0;
        }
        // Automatic bandwidth.
        let (edge, start) = match self.side {
            Side::Both => (eu.min(el), None),
            Side::Upper => (eu, su),
            Side::Lower => (el, sl),
        };
        let mut e = edge;
        if let Some(s) = start {
            e = e.min(s - 100.0);
        }
        f.auto_bandwidth = (((e + 300.0).clamp(2500.0, 10_000.0)) / 500.0).round() * 500.0;
        // Programme SNR between 300 Hz and the bandwidth.
        let (mut excess, mut n) = (0.0f64, 0usize);
        for k in k300..=kb {
            excess += ((self.short_u[k] + self.short_l[k]) as f64 - 2.0 * floor as f64).max(0.0);
            n += 1;
        }
        f.snr_db = if n > 0 {
            (10.0
                * (excess / (2.0 * floor as f64 * n as f64))
                    .max(1e-10)
                    .log10()) as f32
        } else {
            0.0
        };
        // Whistles.
        for t in &mut self.tracks {
            t.seen_now = false;
        }
        for (side, s, o) in [
            (Side::Upper, &self.short_u, &self.short_l),
            (Side::Lower, &self.short_l, &self.short_u),
        ] {
            if self.side != Side::Both && self.side != side {
                continue;
            }
            for k in k150..=kbw {
                let p = s[k];
                if !(p > s[k - 1] && p >= s[k + 1]) || p < 10.0 * o[k] || p < 30.0 * floor {
                    continue;
                }
                let mut around = 0.0;
                for d in 5..=26 {
                    around += s[k - d] + s[k + d];
                }
                around /= 44.0;
                if p < 30.0 * around {
                    continue;
                }
                let kf = k as f32;
                if let Some(t) = self.tracks.iter_mut().find(|t| (t.bin - kf).abs() <= 3.0) {
                    t.bin = 0.7 * t.bin + 0.3 * kf;
                    t.seen_now = true;
                } else if self.tracks.len() < 16 {
                    self.tracks.push(Whistle {
                        bin: kf,
                        seen: 0,
                        missed: 0,
                        unseen: 0,
                        seen_now: true,
                    });
                }
            }
        }
        for t in &mut self.tracks {
            if t.seen_now {
                t.seen += 1;
                t.unseen = 0;
            } else {
                t.missed += 1;
                t.unseen += 1;
            }
        }
        self.tracks.retain(|t| t.unseen < 6);
        f.whistles.clear();
        for t in &self.tracks {
            if t.seen >= 3 && t.missed <= 2 && f.whistles.len() < 6 {
                f.whistles.push(t.bin as f64 * rate / N as f64);
            }
        }
        // Hum: 3 of the first 4 harmonics 10 times over their surroundings.
        f.hum = None;
        for (bi, base) in [50u32, 60].into_iter().enumerate() {
            let mut stand = 0;
            for h in 1..=4 {
                let k = hum_bins[bi][h - 1];
                if k < 5 {
                    continue;
                }
                let p = self.short_u[k] + self.short_l[k];
                // The surroundings are the two quietest of the six bins 2 to 4 away: at 11.7 Hz a bin the harmonics of 50 Hz are
                // only 4.3 bins apart, so the main lobes of this line and of its neighbours cover up to four of those bins.
                let mut s6 = [0.0f32; 6];
                for (j, d) in (2..=4).enumerate() {
                    s6[2 * j] = self.short_u[k - d] + self.short_l[k - d];
                    s6[2 * j + 1] = self.short_u[k + d] + self.short_l[k + d];
                }
                s6.sort_unstable_by(|a, b| a.total_cmp(b));
                let around = 0.5 * (s6[0] + s6[1]);
                if p > 10.0 * around {
                    stand += 1;
                }
            }
            if stand >= 3 {
                f.hum = Some(base);
                break;
            }
        }
    }
}
