//! The cleanup part of the listening chain: automatic notch, manual notch, noise reduction, voice emphasis, de-esser, silence
//! gate, leveller with soft limiter, and the neural voice denoiser. Every stage fades in and out. The latency is 256 samples at
//! 48 kHz while the cleanup is on, and 0 while it is off.

use super::settings::CleanupSettings;
use crate::audio::VoiceDenoiser;
use crate::smoothing::one_pole;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use sdrkit_algo_dsp::ring::DelayLine;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;

/// A fade between 0 and 1 over a fixed time.
#[derive(Clone, Copy, Debug)]
pub struct Fade {
    g: f32,
    step: f32,
}

impl Fade {
    /// Over `seconds` at `rate`.
    pub fn new(rate: f64, seconds: f64) -> Self {
        Fade {
            g: 0.0,
            step: (1.0 / (seconds * rate).max(1.0)) as f32,
        }
    }
    /// Moves one sample towards on or off. Returns the gain.
    #[inline]
    pub fn next(&mut self, on: bool) -> f32 {
        if on {
            self.g = (self.g + self.step).min(1.0);
        } else {
            self.g = (self.g - self.step).max(0.0);
        }
        self.g
    }
    /// The gain now.
    pub fn value(&self) -> f32 {
        self.g
    }
    /// Fully off and wanting off.
    pub fn idle(&self, on: bool) -> bool {
        !on && self.g == 0.0
    }
}

/// A state-variable band-pass (topology-preserving), unity gain at its centre.
#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
}

impl Svf {
    /// Retunes (state kept, so it follows a drifting tone).
    pub fn set(&mut self, rate: f64, f: f64, q: f64) {
        let g = (std::f64::consts::PI * (f / rate).clamp(1e-5, 0.49)).tan() as f32;
        let k = (1.0 / q) as f32;
        self.g = g;
        self.k = k;
        self.a1 = 1.0 / (1.0 + g * (g + k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }
    /// Clears the state.
    pub fn reset(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }
    /// The band-pass output (peak gain 1).
    #[inline]
    pub fn bandpass(&mut self, x: f32) -> f32 {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        v1 * self.k
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Track {
    bin: f32,
    level_db: f32,
    steady: u32,
    missing: u32,
    seen: bool,
    slot: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default)]
struct NotchSlot {
    used: bool,
    freq: f64,
    target: f64,
    depth: f32,
    want: bool,
    svf: [Svf; 2],
}

/// The automatic notch of the cleanup: up to 4 steady narrow tones found in a 1024-point spectrum every 10 ms, 18 dB above their
/// surroundings and steady for about 0.25 s. Each is cut by a band-pass subtracted from the sound, following it as it drifts, with
/// 20 ms attack and 350 ms release.
pub struct AutoNotchTracker {
    rate: f64,
    fft: Fft,
    win: Vec<f32>,
    ring: Vec<f32>,
    pos: usize,
    hop: usize,
    since: usize,
    frame: Vec<Complex32>,
    power: Vec<f32>,
    tracks: Vec<Track>,
    slots: [NotchSlot; 4],
    attack: f32,
    release: f32,
    steady_frames: u32,
    release_frames: u32,
}

impl AutoNotchTracker {
    /// At `rate`.
    pub fn new(rate: f64) -> Self {
        let hop = ((rate / 100.0).round() as usize).max(1);
        AutoNotchTracker {
            rate,
            fft: Fft::forward(1024),
            win: Window::Hann.coefficients(1024),
            ring: vec![0.0; 1024],
            pos: 0,
            hop,
            since: 0,
            frame: vec![Complex32::default(); 1024],
            power: vec![0.0; 512],
            tracks: Vec::with_capacity(16),
            slots: [NotchSlot::default(); 4],
            attack: (1.0 / (0.02 * rate)) as f32,
            release: (1.0 / (0.35 * rate)) as f32,
            steady_frames: 25,
            release_frames: 35,
        }
    }
    /// The frequencies cut now (Hz).
    pub fn notched(&self) -> impl Iterator<Item = f64> + '_ {
        self.slots
            .iter()
            .filter(|s| s.used && s.depth > 0.5)
            .map(|s| s.freq)
    }
    fn analyse(&mut self) {
        for i in 0..1024 {
            self.frame[i] = Complex32::new(self.ring[(self.pos + i) % 1024] * self.win[i], 0.0);
        }
        self.fft.process(&mut self.frame);
        for k in 0..512 {
            self.power[k] = self.frame[k].norm_sqr() + 1e-20;
        }
        for t in &mut self.tracks {
            t.seen = false;
        }
        let bin_hz = self.rate / 1024.0;
        let kmin = ((60.0 / bin_hz) as usize).max(14);
        for k in kmin..(512 - 14) {
            let p = self.power[k];
            if !(p > self.power[k - 1] && p >= self.power[k + 1]) {
                continue;
            }
            let mut around = 0.0;
            for d in 4..=12 {
                around += self.power[k - d] + self.power[k + d];
            }
            around /= 18.0;
            if p < 63.1 * around {
                continue;
            }
            let (a, b, c) = (self.power[k - 1].ln(), p.ln(), self.power[k + 1].ln());
            let den = a - 2.0 * b + c;
            let off = if den.abs() > 1e-9 {
                (0.5 * (a - c) / den).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            let bin = k as f32 + off;
            let level = 10.0 * p.log10();
            if let Some(t) = self.tracks.iter_mut().find(|t| (t.bin - bin).abs() <= 2.0) {
                let steady = (t.level_db - level).abs() < 3.0 && (t.bin - bin).abs() < 1.5;
                t.steady = if steady { t.steady + 1 } else { t.steady / 2 };
                t.bin = bin;
                t.level_db = level;
                t.seen = true;
                t.missing = 0;
            } else if self.tracks.len() < 16 {
                self.tracks.push(Track {
                    bin,
                    level_db: level,
                    steady: 0,
                    missing: 0,
                    seen: true,
                    slot: None,
                });
            }
        }
        for t in &mut self.tracks {
            if !t.seen {
                t.missing += 1;
                t.steady = t.steady.saturating_sub(1);
            }
        }
        // Slots follow their tracks. A lost track lets go of its slot.
        for s in &mut self.slots {
            s.want = false;
        }
        let (sf, rf) = (self.steady_frames, self.release_frames);
        for t in &mut self.tracks {
            if t.slot.is_none() && t.steady >= sf && t.seen {
                if let Some(i) = self.slots.iter().position(|s| !s.used) {
                    self.slots[i].used = true;
                    self.slots[i].freq = t.bin as f64 * bin_hz;
                    self.slots[i].depth = 0.0;
                    for v in &mut self.slots[i].svf {
                        v.reset();
                    }
                    t.slot = Some(i);
                }
            }
            if let Some(i) = t.slot {
                if t.missing == 0 {
                    self.slots[i].target = t.bin as f64 * bin_hz;
                    self.slots[i].want = true;
                } else if t.missing < rf {
                    self.slots[i].want = true;
                }
            }
        }
        self.tracks
            .retain(|t| t.missing < rf + 5 || t.slot.is_some() && t.missing < rf + 5);
        for (i, s) in self.slots.iter_mut().enumerate() {
            if s.used && !s.want && s.depth <= 0.0 {
                s.used = false;
                for t in &mut self.tracks {
                    if t.slot == Some(i) {
                        t.slot = None;
                    }
                }
            }
        }
        for t in &mut self.tracks {
            if let Some(i) = t.slot {
                if !self.slots[i].used {
                    t.slot = None;
                }
            }
        }
    }
    /// Processes one frame (left, right). `mono` processes the left only and copies it.
    #[inline]
    pub fn process(&mut self, l: f32, r: f32, mono: bool, analyse_input: f32) -> (f32, f32) {
        self.ring[self.pos] = analyse_input;
        self.pos = (self.pos + 1) % 1024;
        self.since += 1;
        if self.since >= self.hop {
            self.since = 0;
            self.analyse();
        }
        let (mut yl, mut yr) = (l, r);
        let rate = self.rate;
        for s in &mut self.slots {
            if !s.used {
                continue;
            }
            s.depth = if s.want {
                (s.depth + self.attack).min(1.0)
            } else {
                (s.depth - self.release).max(0.0)
            };
            s.freq += 0.002 * (s.target - s.freq);
            let q = (s.freq / 40.0).clamp(4.0, 40.0);
            s.svf[0].set(rate, s.freq, q);
            yl -= s.depth * s.svf[0].bandpass(l);
            if !mono {
                s.svf[1].set(rate, s.freq, q);
                yr -= s.depth * s.svf[1].bandpass(r);
            }
        }
        if mono {
            yr = yl;
        }
        (yl, yr)
    }
}

/// The spectral noise reduction of the cleanup: frames of 256 samples (at 48 kHz), 50 % overlap, sine window, minimum statistics and
/// a Wiener gain. Its delay is the frame length.
pub struct SpectralNr {
    n: usize,
    hop: usize,
    bins: usize,
    fft: Fft,
    ifft: Fft,
    win: Vec<f32>,
    inbuf: Vec<f32>,
    fill: usize,
    outacc: Vec<f32>,
    outq: Vec<f32>,
    outpos: usize,
    spec: Vec<Complex32>,
    smooth: Vec<f32>,
    cur_min: Vec<f32>,
    sub_min: Vec<f32>,
    sub_i: usize,
    sub_n: u32,
    sub_len: u32,
    noise: Vec<f32>,
    prev: Vec<f32>,
    gain: Vec<f32>,
    tmp: Vec<f32>,
    frames: u64,
    /// Strength, 0 to 1.
    pub strength: f32,
}

const NR_SUBS: usize = 6;

impl SpectralNr {
    /// With frames of `n` samples at `rate`.
    pub fn new(rate: f64, n: usize) -> Self {
        let hop = n / 2;
        let bins = n / 2 + 1;
        let win: Vec<f32> = (0..n)
            .map(|i| (std::f64::consts::PI * (i as f64 + 0.5) / n as f64).sin() as f32)
            .collect();
        SpectralNr {
            n,
            hop,
            bins,
            fft: Fft::forward(n),
            ifft: Fft::inverse(n),
            win,
            inbuf: vec![0.0; n],
            fill: n - hop,
            outacc: vec![0.0; n],
            outq: vec![0.0; hop],
            outpos: 0,
            spec: vec![Complex32::default(); n],
            smooth: vec![0.0; bins],
            cur_min: vec![f32::MAX; bins],
            sub_min: vec![f32::MAX; NR_SUBS * bins],
            sub_i: 0,
            sub_n: 0,
            sub_len: ((0.25 * rate / hop as f64).round() as u32).max(1),
            noise: vec![0.0; bins],
            prev: vec![0.0; bins],
            gain: vec![1.0; bins],
            tmp: vec![1.0; bins],
            frames: 0,
            strength: 0.5,
        }
    }
    /// The delay (samples).
    pub fn delay(&self) -> usize {
        self.n
    }
    fn frame(&mut self) {
        let (n, bins) = (self.n, self.bins);
        for i in 0..n {
            self.spec[i] = Complex32::new(self.inbuf[i] * self.win[i], 0.0);
        }
        self.fft.process(&mut self.spec);
        self.frames += 1;
        let first = self.frames == 1;
        for k in 0..bins {
            let p = self.spec[k].norm_sqr();
            self.smooth[k] = if first {
                p
            } else {
                0.88 * self.smooth[k] + 0.12 * p
            };
            self.cur_min[k] = self.cur_min[k].min(self.smooth[k]);
        }
        self.sub_n += 1;
        if first || self.sub_n >= self.sub_len {
            let base = self.sub_i * bins;
            self.sub_min[base..base + bins].copy_from_slice(&self.cur_min);
            if !first {
                self.sub_i = (self.sub_i + 1) % NR_SUBS;
                self.cur_min.iter_mut().for_each(|v| *v = f32::MAX);
                self.sub_n = 0;
            }
        }
        let s = self.strength.clamp(0.0, 1.0);
        let over = 1.9 * (1.0 + 1.6 * s);
        for k in 0..bins {
            let mut m = self.cur_min[k];
            for j in 0..NR_SUBS {
                m = m.min(self.sub_min[j * bins + k]);
            }
            self.tmp[k] = m;
        }
        for k in 0..bins {
            let a = self.tmp[k.saturating_sub(1)];
            let c = self.tmp[(k + 1).min(bins - 1)];
            self.noise[k] = (0.25 * a + 0.5 * self.tmp[k] + 0.25 * c) * over;
        }
        let floor = 10f32.powf(-(6.0 + 22.0 * s) / 20.0);
        for k in 0..bins {
            let lambda = self.noise[k].max(1e-20);
            let p = self.spec[k].norm_sqr();
            let gamma = p / lambda;
            let xi = if first {
                (gamma - 1.0).max(0.0)
            } else {
                0.94 * self.prev[k] / lambda + 0.06 * (gamma - 1.0).max(0.0)
            };
            let mut g = (xi / (1.0 + xi)).max(floor);
            if !first {
                g = g.max(0.8 * self.gain[k]);
            }
            self.gain[k] = g.min(1.0);
            self.prev[k] = self.gain[k] * self.gain[k] * p;
        }
        for k in 0..bins {
            let a = self.gain[k.saturating_sub(1)];
            let c = self.gain[(k + 1).min(bins - 1)];
            self.tmp[k] = 0.25 * a + 0.5 * self.gain[k] + 0.25 * c;
        }
        for k in 0..bins {
            let g = self.tmp[k];
            self.spec[k] *= g;
            if k > 0 && k < n / 2 {
                self.spec[n - k] *= g;
            }
        }
        self.ifft.process(&mut self.spec);
        let scale = 1.0 / n as f32;
        for i in 0..n {
            self.outacc[i] += self.spec[i].re * scale * self.win[i];
        }
        let hop = self.hop;
        self.outq.copy_from_slice(&self.outacc[..hop]);
        self.outacc.copy_within(hop.., 0);
        self.outacc[n - hop..].iter_mut().for_each(|v| *v = 0.0);
        self.inbuf.copy_within(hop.., 0);
        self.outpos = 0;
    }
    /// One sample in, one (delayed) out.
    #[inline]
    pub fn push(&mut self, x: f32) -> f32 {
        self.inbuf[self.fill] = x;
        self.fill += 1;
        let y = self.outq[self.outpos];
        self.outpos += 1;
        if self.fill == self.n {
            self.frame();
            self.fill = self.n - self.hop;
        }
        y
    }
}

/// A soft limiter: linear below 0.8, then a tanh knee that never reaches 1.0.
#[inline]
pub fn soft_limit(x: f32) -> f32 {
    let a = x.abs();
    if a <= 0.8 {
        x
    } else {
        (0.8 + 0.2 * ((a - 0.8) / 0.2).tanh()).copysign(x)
    }
}

/// The minimum of a level over the last few seconds, in sub-windows (no allocation).
#[derive(Clone, Debug)]
pub struct RunningMin {
    subs: Vec<f32>,
    cur: f32,
    n: u32,
    len: u32,
    i: usize,
}

impl RunningMin {
    /// Over `subs` sub-windows of `len` updates.
    pub fn new(subs: usize, len: u32) -> Self {
        RunningMin {
            subs: vec![f32::MAX; subs],
            cur: f32::MAX,
            n: 0,
            len: len.max(1),
            i: 0,
        }
    }
    /// A new value. Returns the minimum.
    pub fn push(&mut self, v: f32) -> f32 {
        self.cur = self.cur.min(v);
        self.n += 1;
        if self.n >= self.len {
            self.subs[self.i] = self.cur;
            self.i = (self.i + 1) % self.subs.len();
            self.cur = f32::MAX;
            self.n = 0;
        }
        self.subs.iter().copied().fold(self.cur, f32::min)
    }
}

struct ChannelState {
    notch: Biquad,
    nr: SpectralNr,
    dry: DelayLine<f32>,
    voice: [Biquad; 3],
    deess_bp: Biquad,
    denoiser: Option<Box<VoiceDenoiser>>,
}

/// The cleanup chain.
pub struct Cleanup {
    rate: f64,
    s: CleanupSettings,
    master: Fade,
    f_auto: Fade,
    f_notch: Fade,
    f_nr: Fade,
    f_voice: Fade,
    f_deess: Fade,
    f_den: Fade,
    tracker: AutoNotchTracker,
    ch: [ChannelState; 2],
    bypass_delay: [DelayLine<f32>; 2],
    deess_eb: f32,
    deess_ef: f32,
    deess_a: f32,
    block: usize,
    block_fill: usize,
    block_acc: f64,
    gate_floor: RunningMin,
    gate_open: bool,
    gate_hang: u32,
    gate_gain: f32,
    gate_att: f32,
    gate_rel: f32,
    f_gate: Fade,
    lev_ms: f32,
    lev_a: f32,
    lev_floor: RunningMin,
    lev_floor_db: f32,
    lev_gain_db: f32,
    lev_up: f32,
    lev_down: f32,
    f_lev: Fade,
    latency: usize,
}

fn voice_biquads(rate: f64) -> [Biquad; 3] {
    [
        Biquad::new(BiquadCoeffs::highpass(rate, 100.0, 0.707)),
        Biquad::new(BiquadCoeffs::peaking(rate, 280.0, 1.4, -2.0)),
        Biquad::new(BiquadCoeffs::peaking(
            rate,
            2800.0f64.min(0.45 * rate),
            1.0,
            5.0,
        )),
    ]
}

/// The frame of the cleanup's noise reduction (and its latency) at `rate`: 256 at 48 kHz, 128 at 8 to 24 kHz, scaled up above.
pub fn cleanup_frame(rate: f64) -> usize {
    if rate <= 24_000.0 {
        128
    } else {
        let n = 256.0 * rate / 48_000.0;
        (n.round() as usize).next_power_of_two().max(256)
    }
}

fn notch_q(width: f32) -> f64 {
    // 0 narrowest (Q 30) to 1 widest (Q 1), on a log scale.
    (30f64.ln() * (1.0 - width.clamp(0.0, 1.0) as f64)).exp()
}

impl Cleanup {
    /// At `rate` (at least 8 kHz).
    pub fn new(rate: f64, s: CleanupSettings) -> Self {
        let n = cleanup_frame(rate);
        let fade = |t| Fade::new(rate, t);
        let notch_c = BiquadCoeffs::notch(
            rate,
            (s.notch_hz as f64).min(0.45 * rate),
            notch_q(s.notch_width),
        );
        let mk = || ChannelState {
            notch: Biquad::new(notch_c),
            nr: SpectralNr::new(rate, n),
            dry: DelayLine::new(n),
            voice: voice_biquads(rate),
            deess_bp: Biquad::new(BiquadCoeffs::bandpass(
                rate,
                6500.0f64.min(0.45 * rate),
                1.0,
            )),
            denoiser: None,
        };
        let block = ((rate / 100.0).round() as usize).max(1);
        Cleanup {
            rate,
            s,
            master: fade(0.015),
            f_auto: fade(0.015),
            f_notch: fade(0.015),
            f_nr: fade(0.015),
            f_voice: fade(0.015),
            f_deess: fade(0.015),
            f_den: fade(0.015),
            tracker: AutoNotchTracker::new(rate),
            ch: [mk(), mk()],
            bypass_delay: [DelayLine::new(n), DelayLine::new(n)],
            deess_eb: 0.0,
            deess_ef: 0.0,
            deess_a: one_pole(0.005, rate),
            block,
            block_fill: 0,
            block_acc: 0.0,
            gate_floor: RunningMin::new(8, 50),
            gate_open: true,
            gate_hang: 0,
            gate_gain: 1.0,
            gate_att: one_pole(0.002, rate),
            gate_rel: one_pole(0.03, rate),
            f_gate: fade(0.015),
            lev_ms: 0.0,
            lev_a: one_pole(0.05, rate),
            lev_floor: RunningMin::new(8, 50),
            lev_floor_db: -200.0,
            lev_gain_db: 0.0,
            lev_up: one_pole(0.6, rate),
            lev_down: one_pole(0.008, rate),
            f_lev: fade(0.015),
            latency: n,
        }
    }
    /// The settings.
    pub fn settings(&self) -> &CleanupSettings {
        &self.s
    }
    /// New settings, applied from the next sample with fades.
    pub fn set(&mut self, s: CleanupSettings) {
        if s.notch_hz != self.s.notch_hz || s.notch_width != self.s.notch_width {
            let c = BiquadCoeffs::notch(
                self.rate,
                (s.notch_hz as f64).min(0.45 * self.rate),
                notch_q(s.notch_width),
            );
            for ch in &mut self.ch {
                ch.notch.set_coeffs(c);
            }
        }
        self.s = s;
    }
    /// The latency while on (samples).
    pub fn latency(&self) -> usize {
        self.latency
    }
    /// Whether the silence gate is shut now.
    pub fn gate_shut(&self) -> bool {
        self.s.gate && !self.gate_open
    }
    /// The automatic notch's frequencies.
    pub fn notched(&self) -> Vec<f64> {
        self.tracker.notched().collect()
    }
    /// The leveller gain (dB).
    pub fn leveller_db(&self) -> f32 {
        self.lev_gain_db
    }
    /// Whether the cleanup is doing anything (on, or fading out).
    pub fn active(&self) -> bool {
        !self.master.idle(self.s.on)
    }
    /// Processes `buf` in place. Returns false (and leaves `buf` untouched) while off.
    #[allow(clippy::needless_range_loop)] // the two channels index several parallel arrays
    pub fn process(&mut self, buf: &mut [[f32; 2]]) -> bool {
        if !self.active() {
            return false;
        }
        let mono = buf.iter().all(|f| f[0] == f[1]);
        let s = self.s.clone();
        let deess_ok = self.rate >= 16_000.0;
        let den_ok = (self.rate - 48_000.0).abs() < 1e-6;
        for fr in buf.iter_mut() {
            let master = self.master.next(s.on);
            let [inl, inr] = *fr;
            let dl = self.bypass_delay[0].push(inl);
            let dr = if mono {
                dl
            } else {
                self.bypass_delay[1].push(inr)
            };
            let nchan = if mono { 1 } else { 2 };
            // 1. Automatic notch (analysed on the mid signal).
            let ga = self.f_auto.next(s.on && s.auto_notch);
            let mid = 0.5 * (inl + inr);
            let (al, ar) = self.tracker.process(inl, inr, mono, mid);
            let mut x = [inl + ga * (al - inl), inr + ga * (ar - inr)];
            // 2. Manual notch.
            let gn = self.f_notch.next(s.on && s.notch);
            for c in 0..nchan {
                let y = self.ch[c].notch.process_sample(x[c]);
                x[c] += gn * (y - x[c]);
            }
            // 3. Noise reduction (always transformed while on, so the delay stays).
            let gr = self.f_nr.next(s.on && s.nr);
            for c in 0..nchan {
                let ch = &mut self.ch[c];
                ch.nr.strength = s.nr_strength;
                let y = ch.nr.push(x[c]);
                let d = ch.dry.push(x[c]);
                x[c] = d + gr * (y - d);
            }
            // Voice denoiser (48 kHz only).
            let gd = self.f_den.next(s.on && s.denoiser && den_ok);
            if gd > 0.0 {
                for c in 0..nchan {
                    let ch = &mut self.ch[c];
                    let d = ch
                        .denoiser
                        .get_or_insert_with(|| Box::new(VoiceDenoiser::new()));
                    d.strength = s.denoiser_strength * gd;
                    let mut one = [x[c]];
                    d.process(&mut one, |_, _| {});
                    x[c] = one[0];
                }
            }
            // 4. Voice emphasis.
            let gv = self.f_voice.next(s.on && s.voice) * s.voice_amount;
            for c in 0..nchan {
                let ch = &mut self.ch[c];
                let mut y = x[c];
                for b in &mut ch.voice {
                    y = b.process_sample(y);
                }
                x[c] += gv * (y - x[c]);
            }
            // 5. De-esser.
            let ge = self.f_deess.next(s.on && s.deesser && deess_ok);
            if deess_ok {
                let band0 = self.ch[0].deess_bp.process_sample(x[0]);
                self.deess_eb += self.deess_a * (band0 * band0 - self.deess_eb);
                self.deess_ef += self.deess_a * (x[0] * x[0] - self.deess_ef);
                let ratio = if self.deess_ef > 1e-12 {
                    self.deess_eb / self.deess_ef
                } else {
                    0.0
                };
                let k = ge * (2.0 * s.deesser_amount * (ratio - 0.3) / 0.4).clamp(0.0, 1.0);
                x[0] -= k * band0;
                if nchan == 2 {
                    let band1 = self.ch[1].deess_bp.process_sample(x[1]);
                    x[1] -= k * band1;
                }
            }
            // Level of the mid signal per 10 ms block: the gate and the leveller's floor.
            let m = if mono { x[0] } else { 0.5 * (x[0] + x[1]) };
            self.block_acc += (m * m) as f64;
            self.block_fill += 1;
            if self.block_fill >= self.block {
                let ms = self.block_acc / self.block as f64;
                let lvl = (10.0 * ms.max(1e-20).log10()) as f32;
                self.block_acc = 0.0;
                self.block_fill = 0;
                let floor = self.gate_floor.push(lvl);
                let want = lvl >= floor + s.gate_offset_db && lvl >= -75.0;
                if want {
                    self.gate_open = true;
                    self.gate_hang = 15;
                } else if self.gate_hang > 0 {
                    self.gate_hang -= 1;
                } else {
                    self.gate_open = false;
                }
                self.lev_floor_db = self.lev_floor.push(lvl);
            }
            // 6. Silence gate.
            let gg = self.f_gate.next(s.on && s.gate);
            let tgt = if self.gate_open { 1.0 } else { 0.0 };
            let a = if tgt > self.gate_gain {
                self.gate_att
            } else {
                self.gate_rel
            };
            self.gate_gain += a * (tgt - self.gate_gain);
            let gate = 1.0 - gg * (1.0 - self.gate_gain);
            // 7. Leveller with soft limiter.
            let gl = self.f_lev.next(s.on && s.leveller);
            self.lev_ms += self.lev_a * (m * m - self.lev_ms);
            let level_db = 10.0 * self.lev_ms.max(1e-20).log10();
            let mut want_db =
                (s.leveller_amount * (s.leveller_target_db - level_db)).clamp(-18.0, 18.0);
            if s.leveller_hold
                && want_db > 0.0
                && (level_db < self.lev_floor_db + 8.0 || level_db < -62.0)
            {
                // Do not boost the noise: no more gain than there is now.
                want_db = want_db.min(self.lev_gain_db.max(0.0));
            }
            let la = if want_db < self.lev_gain_db {
                self.lev_down
            } else {
                self.lev_up
            };
            self.lev_gain_db += la * (want_db - self.lev_gain_db);
            let lg = 10f32.powf(gl * self.lev_gain_db / 20.0);
            for c in 0..nchan {
                let mut y = x[c] * gate * lg;
                if gl > 0.0 {
                    y += gl * (soft_limit(y) - y);
                }
                x[c] = y;
            }
            if mono {
                x[1] = x[0];
            }
            *fr = [dl + master * (x[0] - dl), dr + master * (x[1] - dr)];
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_stages_off_is_a_pure_delay() {
        let rate = 48e3;
        let mut c = Cleanup::new(
            rate,
            CleanupSettings {
                on: true,
                ..Default::default()
            },
        );
        let x: Vec<[f32; 2]> = (0..48_000)
            .map(|i| {
                let v = (i as f32 * 0.07).sin() * 0.3;
                [v, v]
            })
            .collect();
        let mut y = x.clone();
        assert!(c.process(&mut y));
        let d = c.latency();
        let mut err = 0.0f64;
        let mut sig = 0.0f64;
        for i in 2000 + d..x.len() {
            err += ((y[i][0] - x[i - d][0]) as f64).powi(2);
            sig += (x[i - d][0] as f64).powi(2);
        }
        assert!(10.0 * (sig / err.max(1e-30)).log10() > 60.0);
    }

    #[test]
    fn off_leaves_the_input_untouched() {
        let mut c = Cleanup::new(48e3, CleanupSettings::default());
        let x: Vec<[f32; 2]> = (0..1000)
            .map(|i| [i as f32 * 1e-3, -(i as f32) * 1e-3])
            .collect();
        let mut y = x.clone();
        assert!(!c.process(&mut y));
        assert_eq!(x, y);
    }
}
