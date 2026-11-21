//! The front end: input conditioning at the radio rate, the band front end (a phase-continuous mixer and a 256-phase
//! windowed-sinc resampler to the band's working rate), the band buffer that channels read from, and narrowband interference
//! excision on the L1 and B1I bands.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32;
use std::f64::consts::PI;

/// Input conditioning: DC removal, level, gain to unit noise, clipping, bad samples.
#[derive(Clone, Debug, Default)]
pub struct Conditioner {
    pub dc: (f64, f64),
    rms: f64,
    gain: f64,
    clip: f64,
    started: bool,
    /// Bad samples (not finite) replaced by zero, counted.
    pub bad: u64,
}

impl Conditioner {
    /// Conditions one piece of `n` samples at `rate` in place.
    pub fn process(&mut self, x: &mut [Complex32], rate: f64) {
        if x.is_empty() {
            return;
        }
        let n = x.len() as f64;
        let (mut si, mut sq) = (0f64, 0f64);
        let mut clipped = 0usize;
        for s in x.iter_mut() {
            if !s.re.is_finite() || !s.im.is_finite() {
                *s = Complex32::new(0.0, 0.0);
                self.bad += 1;
            }
            si += s.re as f64;
            sq += s.im as f64;
            if s.re.abs() > 0.98 || s.im.abs() > 0.98 {
                clipped += 1;
            }
        }
        let (mi, mq) = (si / n, sq / n);
        let mut var = 0f64;
        for s in x.iter() {
            var += (s.re as f64 - mi).powi(2) + (s.im as f64 - mq).powi(2);
        }
        let rms = (var / (2.0 * n)).sqrt().max(1e-9);
        let dt = n / rate;
        let clip = clipped as f64 / n;
        if !self.started {
            self.started = true;
            self.dc = (mi, mq);
            self.rms = rms;
            self.gain = 1.0 / rms;
            self.clip = clip;
        }
        let (di, dq) = self.dc;
        let g = self.gain as f32;
        for s in x.iter_mut() {
            s.re = (s.re - di as f32) * g;
            s.im = (s.im - dq as f32) * g;
        }
        let a_dc = 1.0 - (-dt / 0.3).exp();
        let a_lv = 1.0 - (-dt / 0.5).exp();
        let a_cl = 1.0 - (-dt / 1.0).exp();
        self.dc.0 += a_dc * (mi - self.dc.0);
        self.dc.1 += a_dc * (mq - self.dc.1);
        self.rms += a_lv * (rms - self.rms);
        self.gain += a_lv * (1.0 / rms - self.gain);
        self.clip += a_cl * (clip - self.clip);
    }
    /// The level, 20·log10(RMS) dBFS.
    pub fn level_dbfs(&self) -> f64 {
        20.0 * self.rms.max(1e-12).log10()
    }
    /// Clipped samples, percent.
    pub fn clip_percent(&self) -> f64 {
        self.clip * 100.0
    }
    /// The RMS per component.
    pub fn rms(&self) -> f64 {
        self.rms
    }
}

/// A sine and cosine table for the mixers and the correlators: 4096 entries, indexed by the top 12 bits of a 32-bit phase.
pub struct SinCos {
    pub cos: Vec<f32>,
    pub sin: Vec<f32>,
}

/// Bits of the table index.
pub const LUT_BITS: u32 = 12;

impl SinCos {
    pub fn new() -> SinCos {
        let n = 1usize << LUT_BITS;
        SinCos {
            cos: (0..n)
                .map(|i| (2.0 * PI * (i as f64 + 0.5) / n as f64).cos() as f32)
                .collect(),
            sin: (0..n)
                .map(|i| (2.0 * PI * (i as f64 + 0.5) / n as f64).sin() as f32)
                .collect(),
        }
    }
}

impl Default for SinCos {
    fn default() -> Self {
        Self::new()
    }
}

/// The 256-phase windowed-sinc resampler, with its mixer in front.
pub struct BandResampler {
    /// Input rate, output rate.
    pub fs_in: f64,
    pub fs_out: f64,
    taps: usize,
    /// 256 + 1 phases of `taps` coefficients each.
    bank: Vec<f32>,
    /// Mixer: phase in cycles (f64 kept exact across pieces), and its step.
    phase: f64,
    step: f64,
    /// Input history (mixed), with the index of its first sample.
    hist: Vec<Complex32>,
    hist_start: u64,
    /// Next output index.
    next_out: u64,
    ratio: f64,
}

/// Phases of the resampler.
pub const PHASES: usize = 256;

impl BandResampler {
    /// A resampler from `fs_in` to `fs_out`, mixing `offset_hz` (the band's carrier offset from the input centre) to 0 Hz first.
    pub fn new(fs_in: f64, fs_out: f64, offset_hz: f64) -> BandResampler {
        let ratio = fs_in / fs_out;
        let mut taps = (10.3 * ratio.max(1.0)).ceil() as usize;
        taps = taps.div_ceil(4) * 4;
        taps = taps.clamp(16, 128);
        let fc = (0.5 * fs_out).min(0.47 * fs_in) / fs_in; // cycles per input sample
        let beta = 6.5;
        let i0b = sdrkit_algo_dsp::window::bessel_i0(beta);
        let mut bank = vec![0f32; (PHASES + 1) * taps];
        let half = taps as f64 / 2.0;
        for p in 0..=PHASES {
            let frac = p as f64 / PHASES as f64;
            let mut sum = 0.0;
            let row = &mut bank[p * taps..(p + 1) * taps];
            let mut tmp = vec![0f64; taps];
            for (k, t) in tmp.iter_mut().enumerate() {
                // Tap k multiplies input sample n0 - half + 1 + k; its distance from the output instant n0 + frac:
                let d = (k as f64 - half + 1.0) - frac;
                let x = 2.0 * fc * d;
                let sinc = if x.abs() < 1e-12 {
                    1.0
                } else {
                    (PI * x).sin() / (PI * x)
                };
                let u = d / half;
                let w = if u.abs() <= 1.0 {
                    sdrkit_algo_dsp::window::bessel_i0(beta * (1.0 - u * u).sqrt()) / i0b
                } else {
                    0.0
                };
                *t = sinc * w;
                sum += *t;
            }
            for (r, t) in row.iter_mut().zip(&tmp) {
                *r = (t / sum) as f32;
            }
        }
        BandResampler {
            fs_in,
            fs_out,
            taps,
            bank,
            phase: 0.0,
            step: -offset_hz / fs_in,
            hist: Vec::new(),
            hist_start: 0,
            next_out: 0,
            ratio,
        }
    }

    /// The tap count.
    pub fn taps(&self) -> usize {
        self.taps
    }

    /// Mixes and resamples a piece, appending to `out`. Output sample m is the band at input instant m·fs_in/fs_out.
    pub fn process(&mut self, x: &[Complex32], lut: &SinCos, out: &mut Vec<Complex32>) {
        // Mix into the history.
        let base = self.hist.len();
        self.hist.reserve(x.len());
        let mut ph = self.phase.rem_euclid(1.0);
        let step = self.step;
        // A phase as a 32-bit integer for the table; re-anchored from f64 every piece.
        let mut acc = (ph * 4_294_967_296.0) as u32;
        let inc = (step.rem_euclid(1.0) * 4_294_967_296.0).round() as u32;
        let shift = 32 - LUT_BITS;
        for s in x {
            let i = (acc >> shift) as usize;
            let (c, sn) = (lut.cos[i], lut.sin[i]);
            self.hist
                .push(Complex32::new(s.re * c - s.im * sn, s.re * sn + s.im * c));
            acc = acc.wrapping_add(inc);
        }
        ph += step * x.len() as f64;
        self.phase = ph.rem_euclid(1.0);
        let _ = base;
        // Outputs whose window is complete.
        let half = self.taps as i64 / 2;
        let end = self.hist_start + self.hist.len() as u64;
        loop {
            let t = self.next_out as f64 * self.ratio;
            let n0 = t.floor() as i64;
            let last = n0 + half; // highest input index used
            if last >= end as i64 {
                break;
            }
            let first = n0 - half + 1;
            let frac = t - n0 as f64;
            let p = ((frac * PHASES as f64).round() as usize).min(PHASES);
            let h = &self.bank[p * self.taps..(p + 1) * self.taps];
            let mut re = 0f32;
            let mut im = 0f32;
            if first < self.hist_start as i64 {
                // The very start: samples before 0 are zero.
                for (k, &c) in h.iter().enumerate() {
                    let idx = first + k as i64;
                    if idx >= self.hist_start as i64 {
                        let v = self.hist[(idx - self.hist_start as i64) as usize];
                        re += v.re * c;
                        im += v.im * c;
                    }
                }
            } else {
                let off = (first - self.hist_start as i64) as usize;
                let w = &self.hist[off..off + self.taps];
                for (v, &c) in w.iter().zip(h) {
                    re += v.re * c;
                    im += v.im * c;
                }
            }
            out.push(Complex32::new(re, im));
            self.next_out += 1;
        }
        // Keep what the next outputs need.
        let t = self.next_out as f64 * self.ratio;
        let need = (t.floor() as i64 - half + 1).max(0) as u64;
        if need > self.hist_start {
            let drop = ((need - self.hist_start) as usize).min(self.hist.len());
            self.hist.drain(..drop);
            self.hist_start += drop as u64;
        }
    }
}

/// A band's samples at its working rate, addressed by a running index since the start.
#[derive(Clone, Debug, Default)]
pub struct BandBuffer {
    pub start: u64,
    pub data: Vec<Complex32>,
}

impl BandBuffer {
    /// The index one past the newest sample.
    pub fn end(&self) -> u64 {
        self.start + self.data.len() as u64
    }
    /// Samples `[from, from + n)`, if held.
    pub fn slice(&self, from: u64, n: usize) -> Option<&[Complex32]> {
        if from < self.start || from + n as u64 > self.end() {
            return None;
        }
        let o = (from - self.start) as usize;
        Some(&self.data[o..o + n])
    }
    /// Drops samples before `keep_from`, as long as at least `min_keep` remain.
    pub fn trim(&mut self, keep_from: u64, min_keep: usize) {
        let end = self.end();
        let keep_from = keep_from.min(end.saturating_sub(min_keep as u64));
        if keep_from > self.start {
            let n = (keep_from - self.start) as usize;
            self.data.drain(..n);
            self.start = keep_from;
        }
    }
}

/// Narrowband interference excision: 2048-sample frames, hop 1024, square-root Hann analysis and synthesis; bins
/// over 14 times the frame's mean bin power are zeroed with two bins either side.
pub struct Excision {
    fwd: Fft,
    inv: Fft,
    win: Vec<f32>,
    /// Input not yet framed.
    pending: Vec<Complex32>,
    /// Overlap-add accumulator of the frame's second half.
    tail: Vec<Complex32>,
    buf: Vec<Complex32>,
    mask: Vec<bool>,
    /// Bins cut so far.
    pub bins_cut: u64,
    /// Frames processed and frames with a cut.
    pub frames: u64,
    pub frames_cut: u64,
}

/// Frame length.
pub const EXC_N: usize = 2048;

impl Default for Excision {
    fn default() -> Self {
        Excision {
            fwd: Fft::forward(EXC_N),
            inv: Fft::inverse(EXC_N),
            win: (0..EXC_N)
                .map(|n| (PI * n as f64 / EXC_N as f64).sin() as f32)
                .collect(),
            pending: Vec::with_capacity(4 * EXC_N),
            tail: vec![Complex32::new(0.0, 0.0); EXC_N / 2],
            buf: vec![Complex32::new(0.0, 0.0); EXC_N],
            mask: vec![false; EXC_N],
            bins_cut: 0,
            frames: 0,
            frames_cut: 0,
        }
    }
}

impl Excision {
    /// Takes input samples and appends the finished output (1024 samples behind the input) to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        self.pending.extend_from_slice(x);
        let hop = EXC_N / 2;
        let mut pos = 0;
        while self.pending.len() - pos >= EXC_N {
            let frame = &self.pending[pos..pos + EXC_N];
            for i in 0..EXC_N {
                self.buf[i] = frame[i] * self.win[i];
            }
            self.fwd.process(&mut self.buf);
            let mean = self.buf.iter().map(|c| c.norm_sqr()).sum::<f32>() / EXC_N as f32;
            let lim = 14.0 * mean;
            let mut any = false;
            for m in self.mask.iter_mut() {
                *m = false;
            }
            for k in 0..EXC_N {
                if self.buf[k].norm_sqr() > lim {
                    any = true;
                    for d in -2i64..=2 {
                        self.mask[(k as i64 + d).rem_euclid(EXC_N as i64) as usize] = true;
                    }
                }
            }
            self.frames += 1;
            if any {
                self.frames_cut += 1;
                let mut n = 0;
                for (b, &m) in self.buf.iter_mut().zip(&self.mask) {
                    if m {
                        *b = Complex32::new(0.0, 0.0);
                        n += 1;
                    }
                }
                self.bins_cut += n;
                self.inv.process(&mut self.buf);
                let s = 1.0 / EXC_N as f32;
                for i in 0..EXC_N {
                    self.buf[i] = self.buf[i] * (self.win[i] * s);
                }
            } else {
                // Nothing cut: the frame passes unchanged (window applied twice).
                for i in 0..EXC_N {
                    self.buf[i] = frame[i] * (self.win[i] * self.win[i]);
                }
            }
            for i in 0..hop {
                out.push(self.tail[i] + self.buf[i]);
                self.tail[i] = self.buf[hop + i];
            }
            pos += hop;
        }
        self.pending.drain(..pos);
    }
    /// Samples of delay between input and output.
    pub const DELAY: usize = EXC_N / 2;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampler_keeps_a_tone_and_its_timing() {
        let lut = SinCos::new();
        for (fs_in, fs_out) in [
            (4e6, 4.096e6),
            (10e6, 4.096e6),
            (2.046e6, 4.096e6),
            (20e6, 1.024e6),
        ] {
            let f = 300e3_f64.min(fs_out * 0.15);
            let n = (fs_in * 0.01) as usize;
            let x: Vec<Complex32> = (0..n)
                .map(|i| {
                    let p = 2.0 * PI * f * i as f64 / fs_in;
                    Complex32::new(p.cos() as f32, p.sin() as f32)
                })
                .collect();
            let mut r = BandResampler::new(fs_in, fs_out, 0.0);
            let mut out = Vec::new();
            for c in x.chunks(777) {
                r.process(c, &lut, &mut out);
            }
            // Output m is the tone at input instant m * fs_in / fs_out.
            for m in [200usize, 1000, out.len() - 10] {
                let p = 2.0 * PI * f * m as f64 / fs_out;
                let want = Complex32::new(p.cos() as f32, p.sin() as f32);
                assert!(
                    (out[m] - want).norm() < 0.01,
                    "{fs_in} {m} {:?} {:?}",
                    out[m],
                    want
                );
            }
        }
    }

    #[test]
    fn excision_passes_noise_and_cuts_a_line() {
        let mut e = Excision::default();
        let mut s = 12345u64;
        let mut noise = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 40) as f32 / (1u64 << 24) as f32 - 0.5
        };
        let x: Vec<Complex32> = (0..20_000)
            .map(|_| Complex32::new(noise(), noise()))
            .collect();
        let mut out = Vec::new();
        e.process(&x, &mut out);
        // Away from the very start, the output is the input delayed by 1024.
        for i in 3000..out.len() {
            assert!((out[i] - x[i]).norm() < 1e-4);
        }
        assert_eq!(e.bins_cut, 0);
        let y: Vec<Complex32> = x
            .iter()
            .enumerate()
            .map(|(i, v)| v + Complex32::from_polar(3.0, 2.0 * PI as f32 * 0.1 * i as f32))
            .collect();
        let mut e = Excision::default();
        let mut out = Vec::new();
        e.process(&y, &mut out);
        assert!(e.bins_cut > 0);
        let p: f32 = out[4000..8000].iter().map(|c| c.norm_sqr()).sum::<f32>() / 4000.0;
        assert!(p < 0.3, "{p}");
    }

    #[test]
    fn conditioner_scales_to_unit_noise() {
        let mut c = Conditioner::default();
        let mut x: Vec<Complex32> = (0..16384)
            .map(|i| {
                Complex32::new(
                    0.1 * ((i % 7) as f32 - 3.0) + 0.05,
                    0.1 * ((i % 5) as f32 - 2.0),
                )
            })
            .collect();
        c.process(&mut x, 4e6);
        assert!((c.dc.0 - 0.05).abs() < 1e-3);
        let p: f32 = x.iter().map(|s| s.norm_sqr()).sum::<f32>() / x.len() as f32;
        assert!((p - 2.0).abs() < 0.05, "{p}");
    }
}
