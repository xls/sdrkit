//! The audio control part of the listening chain: stereo mode, filters, notch, automatic notch, clarifier and CW pitch, EQ,
//! de-emphasis, AGC, the squelch gate with tail delay, and the meters.

use super::cleanup::Fade;
use super::settings::{AgcSpeed, ChainDeemph, ChainSquelch, ControlSettings, StereoMode};
use crate::smoothing::one_pole;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir::hilbert;
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs, BUTTERWORTH4_Q};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::ring::DelayLine;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::squelch::Gate;
use sdrkit_core::Complex32;
use sdrkit_proto_tones::ctcss::{CtcssConfig, CtcssDecoder, CtcssEventKind};
use sdrkit_proto_tones::dcs::{DcsConfig, DcsDecoder, DcsEventKind};
use sdrkit_util_ids::tones::dcs::Dcs;

/// The length of one squelch decision block, s (10 ms).
pub const SQUELCH_BLOCK_S: f64 = 0.01;

/// The squelch decision gate of the chain: it opens on a block at or above `open_db`, stays open
/// while a block reaches `open_db - hysteresis_db`, and closes when `hang_ms` passes without such a block. A block is 10 ms.
pub fn squelch_gate(open_db: f32, hysteresis_db: f32, hang_ms: f32) -> Gate {
    let hang = (hang_ms as f64 * 1e-3 / SQUELCH_BLOCK_S).round().max(0.0) as u32;
    // A value that is not finite (the settings have public fields) produces the default gate, so the audio path never stops.
    Gate::new(open_db, open_db - hysteresis_db.max(0.0), 1, hang)
        .or_else(|_| Gate::new(-50.0, -54.0, 1, 25))
        .expect("the default thresholds are ordered")
}

/// The tone squelch of the chain: the shared CTCSS or DCS decoder, and the tone or code it waits for.
enum ToneSquelch {
    Ctcss(Box<CtcssDecoder>, f32),
    Dcs(Box<DcsDecoder>, Dcs, bool),
}

impl ToneSquelch {
    /// The CTCSS decoder of the tone squelch: a 0.3 s window every 50 ms, a lock after 3 hops and a release after 4, so the squelch
    /// opens about 0.4 s after the tone starts.
    fn ctcss(rate: f64, hz: f32) -> Option<ToneSquelch> {
        let cfg = CtcssConfig {
            window_s: 0.3,
            hop_s: 0.05,
            lock_hops: 3,
            release_hops: 4,
            ..CtcssConfig::default()
        };
        CtcssDecoder::new(rate, cfg)
            .ok()
            .map(|d| ToneSquelch::Ctcss(Box::new(d), hz))
    }
    fn dcs(rate: f64, code: Dcs, inverted: bool) -> Option<ToneSquelch> {
        DcsDecoder::new(rate, DcsConfig::default())
            .ok()
            .map(|d| ToneSquelch::Dcs(Box::new(d), code, inverted))
    }
    /// Takes one sample. Returns whether the configured tone or code is present, and whether a turn-off came (close at once).
    #[inline]
    fn push(&mut self, x: f32) -> (bool, bool) {
        let mut turn_off = false;
        match self {
            ToneSquelch::Ctcss(d, want) => {
                d.process(&[x], |e| turn_off |= e.kind == CtcssEventKind::TurnOff);
                let present = d
                    .locked()
                    .is_some_and(|(t, _, _)| (t.hz - *want).abs() < 0.05);
                (present, turn_off)
            }
            ToneSquelch::Dcs(d, code, inv) => {
                d.process(&[x], |e| turn_off |= e.kind == DcsEventKind::TurnOff);
                let want = (*code, *inv);
                let present = d
                    .locked()
                    .is_some_and(|(r, _)| (r.code, r.inverted) == want || r.alias == Some(want));
                (present, turn_off)
            }
        }
    }
}

/// An adaptive line enhancer (Widrow 1975): an NLMS predictor of 32 taps behind a decorrelation delay of 24 samples, step 0.004.
pub struct LineEnhancer {
    w: [f32; 32],
    hist: Vec<f32>,
    pos: usize,
    mu: f32,
    pp: f32,
    px: f32,
    a: f32,
    last: f32,
    crossings: u32,
    count: u32,
    window: u32,
    /// The notched frequency (Hz), when the prediction holds more than a quarter of the power.
    pub notched_hz: Option<f64>,
    rate: f64,
}

impl LineEnhancer {
    const TAPS: usize = 32;
    const DELAY: usize = 24;
    /// At `rate`.
    pub fn new(rate: f64) -> Self {
        LineEnhancer {
            w: [0.0; 32],
            hist: vec![0.0; Self::TAPS + Self::DELAY + 1],
            pos: 0,
            mu: 0.004,
            pp: 0.0,
            px: 0.0,
            a: one_pole(0.2, rate),
            last: 0.0,
            crossings: 0,
            count: 0,
            window: (0.25 * rate) as u32,
            notched_hz: None,
            rate,
        }
    }
    /// Predicts the steady part of `x` (the tone) and returns it.
    #[inline]
    pub fn push(&mut self, x: f32) -> f32 {
        let n = self.hist.len();
        let mut p = 0.0f32;
        let mut norm = 1e-6f32;
        for k in 0..Self::TAPS {
            let u = self.hist[(self.pos + n - Self::DELAY - k) % n];
            p += self.w[k] * u;
            norm += u * u;
        }
        let e = x - p;
        let g = self.mu * e / norm;
        for k in 0..Self::TAPS {
            let u = self.hist[(self.pos + n - Self::DELAY - k) % n];
            self.w[k] += g * u;
        }
        self.hist[self.pos] = x;
        self.pos = (self.pos + 1) % n;
        // The notched frequency from the zero crossings of the prediction.
        self.pp += self.a * (p * p - self.pp);
        self.px += self.a * (x * x - self.px);
        if (p >= 0.0) != (self.last >= 0.0) {
            self.crossings += 1;
        }
        self.last = p;
        self.count += 1;
        if self.count >= self.window {
            self.notched_hz = (self.pp > 0.25 * self.px)
                .then(|| self.crossings as f64 * self.rate / (2.0 * self.count as f64));
            self.crossings = 0;
            self.count = 0;
        }
        p
    }
}

/// A single-sideband frequency shifter: a 63-tap Hilbert transformer, then the delayed signal times cos minus the Hilbert output
/// times sin of an oscillator at the shift.
pub struct Clarifier {
    hilbert: sdrkit_algo_dsp::fir::Fir<f32>,
    delay: DelayLine<f32>,
    osc: Nco,
}

/// The Hilbert transformer's Kaiser beta: 5.4 has about the side lobes of a Hamming window.
const CLARIFIER_BETA: f64 = 5.4;

impl Clarifier {
    /// A shifter with no shift.
    pub fn new() -> Self {
        Clarifier {
            hilbert: sdrkit_algo_dsp::fir::Fir::new(
                &hilbert(63, CLARIFIER_BETA).expect("the filter settings are valid"),
            ),
            delay: DelayLine::new(31),
            osc: Nco::new(1.0, 0.0),
        }
    }
    /// Sets the shift (Hz) at `rate`. The oscillator's phase runs on.
    pub fn set_shift(&mut self, hz: f64, rate: f64) {
        if self.osc.rate() != rate {
            self.osc = Nco::new(rate, hz);
        } else {
            self.osc.set_freq(hz);
        }
    }
    /// One sample.
    #[inline]
    pub fn push(&mut self, x: f32) -> f32 {
        let h = self.hilbert.process_sample(x);
        let d = self.delay.push(x);
        let o = self.osc.next_sample();
        d * o.re - h * o.im
    }
}

impl Default for Clarifier {
    fn default() -> Self {
        Self::new()
    }
}

/// The meters of what is played.
#[derive(Clone, Debug, Default)]
pub struct Meters {
    peak: f32,
    sum2: f64,
    n: u64,
    /// Samples at 0.999 or more, counted since the start.
    pub clips: u64,
    clip_hold: u64,
}

impl Meters {
    /// Peak (dBFS) and RMS (dBFS) since the last call. And whether CLIP shows (a clip in the last 2 s).
    pub fn take(&mut self) -> (f32, f32, bool) {
        let peak = 20.0 * self.peak.max(1e-10).log10();
        let rms = if self.n > 0 {
            (10.0 * (self.sum2 / self.n as f64).max(1e-20).log10()) as f32
        } else {
            -200.0
        };
        self.peak = 0.0;
        self.sum2 = 0.0;
        self.n = 0;
        (peak, rms, self.clip_hold > 0)
    }
}

/// The audio control part of the listening chain.
pub struct AudioControl {
    rate: f64,
    s: ControlSettings,
    master: Fade,
    hp: [[Biquad; 2]; 2],
    lp: [[Biquad; 2]; 2],
    tone_hp: [Biquad; 2],
    notch: [Biquad; 2],
    eq: [[Biquad; 5]; 2],
    eq_on: [bool; 5],
    eq_warm: u32,
    prev: Option<Box<PrevFilters>>,
    xfade: u32,
    xfade_len: u32,
    deemph_a: f32,
    deemph_y: [f32; 2],
    ale: LineEnhancer,
    clar: [Clarifier; 2],
    clar_on: bool,
    agc_level: f32,
    agc_gain: f32,
    lvl_att: f32,
    lvl_rel: f32,
    g_att: f32,
    g_rel: f32,
    gate_db: Gate,
    tone: Option<ToneSquelch>,
    tone_gate: Gate,
    tone_present: bool,
    block: usize,
    fill: usize,
    acc: f64,
    speech_bp: [Biquad; 2],
    noise_hp: [Biquad; 2],
    acc_speech: f64,
    acc_noise: f64,
    gate: f32,
    gate_step: f32,
    tail: [DelayLine<f32>; 2],
    /// The meters.
    pub meters: Meters,
    spec_ring: Vec<f32>,
    spec_pos: usize,
    fft: Fft,
    spec_win: Vec<f32>,
    spec_buf: Vec<Complex32>,
}

/// The filters as they were before a change, run beside the new ones during a 10 ms cross-fade.
#[derive(Clone)]
struct PrevFilters {
    hp: [[Biquad; 2]; 2],
    lp: [[Biquad; 2]; 2],
    notch: [Biquad; 2],
    eq: [[Biquad; 5]; 2],
    eq_active: bool,
}

#[inline]
fn run_section(hp: &mut [Biquad; 2], lp: &mut [Biquad; 2], notch: &mut Biquad, v: f32) -> f32 {
    let mut v = hp[0].process_sample(v);
    v = hp[1].process_sample(v);
    v = lp[0].process_sample(v);
    v = lp[1].process_sample(v);
    notch.process_sample(v)
}

#[inline]
fn run_eq(eq: &mut [Biquad; 5], mut v: f32) -> f32 {
    for b in eq {
        v = b.process_sample(v);
    }
    v
}

fn q_pair(steep: bool) -> [f64; 2] {
    if steep {
        BUTTERWORTH4_Q
    } else {
        [std::f64::consts::FRAC_1_SQRT_2; 2]
    }
}

impl AudioControl {
    /// At `rate`.
    pub fn new(rate: f64, s: ControlSettings) -> Self {
        let id = || {
            Biquad::new(BiquadCoeffs {
                b0: 1.0,
                b1: 0.0,
                b2: 0.0,
                a1: 0.0,
                a2: 0.0,
            })
        };
        let mut c = AudioControl {
            rate,
            master: Fade::new(rate, 0.01),
            hp: [[id(), id()], [id(), id()]],
            lp: [[id(), id()], [id(), id()]],
            tone_hp: [
                Biquad::new(BiquadCoeffs::highpass(
                    rate,
                    260.0,
                    std::f64::consts::FRAC_1_SQRT_2,
                )),
                Biquad::new(BiquadCoeffs::highpass(
                    rate,
                    260.0,
                    std::f64::consts::FRAC_1_SQRT_2,
                )),
            ],
            notch: [id(), id()],
            eq: [
                [id(), id(), id(), id(), id()],
                [id(), id(), id(), id(), id()],
            ],
            eq_on: [false; 5],
            eq_warm: 0,
            prev: None,
            xfade: 0,
            xfade_len: (0.01 * rate) as u32,
            deemph_a: 1.0,
            deemph_y: [0.0; 2],
            ale: LineEnhancer::new(rate),
            clar: [Clarifier::new(), Clarifier::new()],
            clar_on: false,
            agc_level: 0.0,
            agc_gain: 1.0,
            lvl_att: one_pole(0.0005, rate),
            lvl_rel: one_pole(0.03, rate),
            g_att: one_pole(0.002, rate),
            g_rel: one_pole(0.12, rate),
            gate_db: squelch_gate(-50.0, 4.0, 250.0),
            tone: None,
            tone_gate: squelch_gate(0.0, 0.0, 250.0),
            tone_present: false,
            block: ((rate / 100.0).round() as usize).max(1),
            fill: 0,
            acc: 0.0,
            speech_bp: [
                Biquad::new(BiquadCoeffs::highpass(
                    rate,
                    300.0,
                    std::f64::consts::FRAC_1_SQRT_2,
                )),
                Biquad::new(BiquadCoeffs::lowpass(
                    rate,
                    3000.0f64.min(0.45 * rate),
                    std::f64::consts::FRAC_1_SQRT_2,
                )),
            ],
            noise_hp: [
                Biquad::new(BiquadCoeffs::highpass(
                    rate,
                    5500.0f64.min(0.45 * rate),
                    BUTTERWORTH4_Q[0],
                )),
                Biquad::new(BiquadCoeffs::highpass(
                    rate,
                    5500.0f64.min(0.45 * rate),
                    BUTTERWORTH4_Q[1],
                )),
            ],
            acc_speech: 0.0,
            acc_noise: 0.0,
            gate: 0.0,
            gate_step: (1.0 / (0.005 * rate)) as f32,
            tail: [DelayLine::new(0), DelayLine::new(0)],
            meters: Meters::default(),
            spec_ring: vec![0.0; 1024],
            spec_pos: 0,
            fft: Fft::forward(1024),
            spec_win: Window::Hann.coefficients(1024),
            spec_buf: vec![Complex32::default(); 1024],
            s: ControlSettings::default(),
        };
        c.set(s);
        c
    }
    /// The settings.
    pub fn settings(&self) -> &ControlSettings {
        &self.s
    }
    /// New settings: filters are redesigned where they changed.
    pub fn set(&mut self, s: ControlSettings) {
        let rate = self.rate;
        let nyq = 0.47 * rate;
        let qs = q_pair(s.steep);
        let first = self.s == ControlSettings::default() && s != self.s;
        let before = PrevFilters {
            hp: self.hp.clone(),
            lp: self.lp.clone(),
            notch: self.notch.clone(),
            eq: self.eq.clone(),
            eq_active: self.eq_on.iter().any(|&g| g) || self.eq_warm > 0,
        };
        let old = std::mem::replace(&mut self.s, s);
        let s = &self.s;
        let id = BiquadCoeffs {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        };
        if first
            || old.highpass != s.highpass
            || old.highpass_hz != s.highpass_hz
            || old.steep != s.steep
        {
            for ch in &mut self.hp {
                for (k, b) in ch.iter_mut().enumerate() {
                    let on = s.highpass && (k == 0 || s.steep);
                    let c = if on {
                        BiquadCoeffs::highpass(rate, (s.highpass_hz as f64).clamp(5.0, nyq), qs[k])
                    } else {
                        id
                    };
                    b.set_coeffs(c);
                }
            }
        }
        if first
            || old.lowpass != s.lowpass
            || old.lowpass_hz != s.lowpass_hz
            || old.steep != s.steep
        {
            for ch in &mut self.lp {
                for (k, b) in ch.iter_mut().enumerate() {
                    let on = s.lowpass && (k == 0 || s.steep);
                    let c = if on {
                        BiquadCoeffs::lowpass(rate, (s.lowpass_hz as f64).clamp(10.0, nyq), qs[k])
                    } else {
                        id
                    };
                    b.set_coeffs(c);
                }
            }
        }
        if first || old.notch != s.notch || old.notch_hz != s.notch_hz || old.notch_q != s.notch_q {
            let c = if s.notch {
                BiquadCoeffs::notch(
                    rate,
                    (s.notch_hz as f64).clamp(5.0, nyq),
                    (s.notch_q as f64).max(0.3),
                )
            } else {
                id
            };
            for b in &mut self.notch {
                b.set_coeffs(c);
            }
        }
        let gains = s.eq.gains().unwrap_or(s.eq_gains);
        let was_on = self.eq_on.iter().any(|&g| g);
        let freqs = [120.0, 400.0, 1000.0, 2500.0, 6000.0f64];
        for (k, &g) in gains.iter().enumerate() {
            let g = g.clamp(-18.0, 18.0) as f64;
            let f0 = freqs[k].min(0.45 * rate);
            self.eq_on[k] = g.abs() > 1e-6;
            let c = match k {
                0 => BiquadCoeffs::low_shelf(rate, f0, std::f64::consts::FRAC_1_SQRT_2, g),
                4 => BiquadCoeffs::high_shelf(rate, f0, std::f64::consts::FRAC_1_SQRT_2, g),
                _ => BiquadCoeffs::peaking(rate, f0, 1.0, g),
            };
            for ch in &mut self.eq {
                if ch[k].coeffs() != &c {
                    ch[k].set_coeffs(c);
                }
            }
        }
        // Keep the bands running a little after the EQ goes flat, so they decay to rest before being skipped.
        if was_on && !self.eq_on.iter().any(|&g| g) {
            self.eq_warm = (0.05 * rate) as u32;
        }
        // A changed design is cross-faded from the old filters over 10 ms, so a change never clicks.
        let changed =
            |a: &[Biquad], b: &[Biquad]| a.iter().zip(b).any(|(x, y)| x.coeffs() != y.coeffs());
        let any = (0..2).any(|c| {
            changed(&before.hp[c], &self.hp[c])
                || changed(&before.lp[c], &self.lp[c])
                || changed(&before.eq[c], &self.eq[c])
        }) || changed(&before.notch, &self.notch);
        if any && !first {
            self.prev = Some(Box::new(before));
            self.xfade = self.xfade_len.max(1);
        }
        self.deemph_a = if s.deemph != ChainDeemph::Off {
            one_pole(f64::from(s.deemph.micros()) * 1e-6, rate)
        } else {
            1.0
        };
        let shift = s.clarifier_hz as f64
            + if s.cw_pitch_hz > 0.0 {
                s.cw_pitch_hz as f64 - 700.0
            } else {
                0.0
            };
        self.clar_on = shift.abs() > 0.5;
        for c in &mut self.clar {
            c.set_shift(shift, rate);
        }
        let (ga, gr) = if s.agc == AgcSpeed::Slow {
            (0.02, 1.5)
        } else {
            (0.002, 0.12)
        };
        self.g_att = one_pole(ga, rate);
        self.g_rel = one_pole(gr, rate);
        if first || old.squelch_hang_ms != s.squelch_hang_ms {
            self.gate_db = squelch_gate(s.squelch_open, s.squelch_hysteresis, s.squelch_hang_ms);
            self.tone_gate = squelch_gate(0.0, 0.0, s.squelch_hang_ms);
        } else {
            let close = s.squelch_open - s.squelch_hysteresis.max(0.0);
            let _ = self.gate_db.set_thresholds(s.squelch_open, close);
        }
        let tone_changed = first
            || old.squelch != s.squelch
            || old.ctcss_hz != s.ctcss_hz
            || old.dcs_code != s.dcs_code
            || old.dcs_inverted != s.dcs_inverted
            || self.tone.is_none();
        if tone_changed {
            self.tone = match s.squelch {
                ChainSquelch::Ctcss => ToneSquelch::ctcss(rate, s.ctcss_hz),
                ChainSquelch::Dcs => ToneSquelch::dcs(rate, s.dcs_code, s.dcs_inverted),
                _ => None,
            };
            self.tone_present = false;
        }
        let tail = (s.tail_ms as f64 * 1e-3 * rate).round() as usize;
        if tail != self.tail[0].delay() {
            self.tail = [DelayLine::new(tail), DelayLine::new(tail)];
        }
    }
    /// Whether the chain is doing anything (on, or fading out).
    pub fn active(&self) -> bool {
        !self.master.idle(self.s.on)
    }
    /// Whether the squelch is open now (always with no squelch).
    pub fn squelch_open(&self) -> bool {
        match self.s.squelch {
            ChainSquelch::Off => true,
            ChainSquelch::Ctcss | ChainSquelch::Dcs => self.tone_gate.is_open(),
            _ => self.gate_db.is_open(),
        }
    }
    /// Whether the recorder takes the sound now ("only while the squelch is open").
    pub fn recorder_open(&self) -> bool {
        !self.s.record_only_open || self.squelch_open()
    }
    /// The frequency the automatic notch removes.
    pub fn auto_notch_hz(&self) -> Option<f64> {
        if self.s.auto_notch {
            self.ale.notched_hz
        } else {
            None
        }
    }
    /// The AGC gain (dB).
    pub fn agc_gain_db(&self) -> f32 {
        20.0 * self.agc_gain.max(1e-9).log10()
    }
    fn measure_block(&mut self) {
        let n = self.fill.max(1) as u64;
        let measure = match self.s.squelch {
            ChainSquelch::Level => (10.0 * (self.acc / n as f64).max(1e-20).log10()) as f32,
            ChainSquelch::Snr if self.rate >= 16_000.0 => {
                let sp = self.acc_speech / n as f64 / 2700.0;
                let nb = (500.0f64).max(0.47 * self.rate - 5500.0);
                let no = self.acc_noise / n as f64 / nb;
                (10.0 * (sp.max(1e-30) / no.max(1e-30)).log10()) as f32
            }
            ChainSquelch::Snr => (10.0 * (self.acc / n as f64).max(1e-20).log10()) as f32,
            _ => 0.0,
        };
        match self.s.squelch {
            ChainSquelch::Level | ChainSquelch::Snr => {
                self.gate_db.update(Some(measure));
            }
            ChainSquelch::Ctcss | ChainSquelch::Dcs => {
                self.tone_gate.update(self.tone_present.then_some(0.0));
            }
            _ => {}
        }
        self.acc = 0.0;
        self.acc_speech = 0.0;
        self.acc_noise = 0.0;
        self.fill = 0;
    }
    /// Processes `buf` in place. Returns false (and leaves it untouched) while off.
    #[allow(clippy::needless_range_loop)] // the two channels index several parallel arrays
    pub fn process(&mut self, buf: &mut [[f32; 2]]) -> bool {
        if !self.active() {
            return false;
        }
        let s = self.s.clone();
        let gains_on = self.eq_on;
        let tone_hp = s.squelch.is_tone();
        let target = 10f32.powf(s.agc_target_db / 20.0);
        for fr in buf.iter_mut() {
            let master = self.master.next(s.on);
            let [inl, inr] = *fr;
            // The mode of the two channels.
            let (mut l, mut r) = match s.stereo {
                StereoMode::Mono => {
                    let m = 0.5 * (inl + inr);
                    (m, m)
                }
                StereoMode::Swap => (inr, inl),
                StereoMode::Left => (inl, inl),
                StereoMode::Right => (inr, inr),
                _ => (inl, inr),
            };
            // The squelch is measured here, before the filters.
            let mid = 0.5 * (l + r);
            self.acc += (mid * mid) as f64;
            if s.squelch == ChainSquelch::Snr {
                let sp = {
                    let t = self.speech_bp[0].process_sample(mid);
                    self.speech_bp[1].process_sample(t)
                };
                let no = {
                    let t = self.noise_hp[0].process_sample(mid);
                    self.noise_hp[1].process_sample(t)
                };
                self.acc_speech += (sp * sp) as f64;
                self.acc_noise += (no * no) as f64;
            }
            if let Some(t) = &mut self.tone {
                let (present, turn_off) = t.push(mid);
                self.tone_present = present;
                if turn_off {
                    // The CTCSS reverse burst or the DCS turn-off code closes the squelch at once.
                    self.tone_gate.reset();
                }
            }
            self.fill += 1;
            if self.fill >= self.block {
                self.measure_block();
            }
            let mut x = [l, r];
            let fade_t = if self.xfade > 0 {
                self.xfade as f32 / self.xfade_len.max(1) as f32
            } else {
                0.0
            };
            for c in 0..2 {
                let mut v = run_section(&mut self.hp[c], &mut self.lp[c], &mut self.notch[c], x[c]);
                if let Some(p) = self.prev.as_mut() {
                    let o = run_section(&mut p.hp[c], &mut p.lp[c], &mut p.notch[c], x[c]);
                    v = o * fade_t + v * (1.0 - fade_t);
                }
                if tone_hp {
                    v = self.tone_hp[c].process_sample(v);
                }
                x[c] = v;
            }
            if s.auto_notch {
                let p = self.ale.push(0.5 * (x[0] + x[1]));
                x[0] -= p;
                x[1] -= p;
            }
            if self.clar_on {
                for c in 0..2 {
                    x[c] = self.clar[c].push(x[c]);
                }
            }
            self.eq_warm = self.eq_warm.saturating_sub(1);
            if self.xfade > 0 {
                self.xfade -= 1;
                if self.xfade == 0 {
                    self.prev = None;
                }
            }
            for c in 0..2 {
                // Every band runs (a flat band is the identity), so a band switched on mid-stream starts from live state and
                // never clicks. With all bands flat the EQ is skipped.
                let input = x[c];
                if gains_on.iter().any(|&g| g) || self.eq_warm > 0 {
                    x[c] = run_eq(&mut self.eq[c], input);
                }
                if let Some(p) = self.prev.as_mut() {
                    let o = if p.eq_active {
                        run_eq(&mut p.eq[c], input)
                    } else {
                        input
                    };
                    x[c] = o * fade_t + x[c] * (1.0 - fade_t);
                }
                if self.deemph_a < 1.0 {
                    self.deemph_y[c] += self.deemph_a * (x[c] - self.deemph_y[c]);
                    x[c] = self.deemph_y[c];
                }
            }
            if s.agc != AgcSpeed::Off {
                let lvl = x[0].abs().max(x[1].abs());
                let a = if lvl > self.agc_level {
                    self.lvl_att
                } else {
                    self.lvl_rel
                };
                self.agc_level += a * (lvl - self.agc_level);
                if self.agc_level >= 0.002 {
                    let want = (target / self.agc_level).clamp(0.03, 31.6);
                    let a = if want < self.agc_gain {
                        self.g_att
                    } else {
                        self.g_rel
                    };
                    self.agc_gain += a * (want - self.agc_gain);
                }
                x[0] *= self.agc_gain;
                x[1] *= self.agc_gain;
            }
            // The squelch gate, with the sound running behind the decision by the tail.
            let open = self.squelch_open();
            if open {
                self.gate = (self.gate + self.gate_step).min(1.0);
            } else {
                self.gate = (self.gate - self.gate_step).max(0.0);
            }
            let tl = self.tail[0].push(x[0]);
            let tr = self.tail[1].push(x[1]);
            x[0] = tl * self.gate;
            x[1] = tr * self.gate;
            // Meters.
            let pk = x[0].abs().max(x[1].abs());
            self.meters.peak = self.meters.peak.max(pk);
            self.meters.sum2 += (0.5 * (x[0] * x[0] + x[1] * x[1])) as f64;
            self.meters.n += 1;
            if pk >= 0.999 {
                self.meters.clips += 1;
                self.meters.clip_hold = (2.0 * self.rate) as u64;
            } else if self.meters.clip_hold > 0 {
                self.meters.clip_hold -= 1;
            }
            if s.spectrum {
                self.spec_ring[self.spec_pos] = 0.5 * (x[0] + x[1]);
                self.spec_pos = (self.spec_pos + 1) % 1024;
            }
            l = inl + master * (x[0] - inl);
            r = inr + master * (x[1] - inr);
            *fr = [l, r];
        }
        true
    }
    /// The AF spectrum strip: 128 bins (a 1024-point Hann FFT, the maximum of each 4 bins), scaled so a full-scale sine reads about
    /// 0 dBFS. Computed from the last 1024 samples played, only when requested (and only while `spectrum` is on).
    pub fn af_spectrum(&mut self, out: &mut [f32; 128]) {
        let wsum: f32 = self.spec_win.iter().sum();
        for i in 0..1024 {
            self.spec_buf[i] = Complex32::new(
                self.spec_ring[(self.spec_pos + i) % 1024] * self.spec_win[i],
                0.0,
            );
        }
        self.fft.process(&mut self.spec_buf);
        for (b, o) in out.iter_mut().enumerate() {
            let mut m = 0.0f32;
            for k in 4 * b..4 * b + 4 {
                m = m.max(self.spec_buf[k].norm());
            }
            *o = 20.0 * (2.0 * m / wsum).max(1e-10).log10();
        }
    }
    /// The frequency at the centre of AF spectrum bin `b`.
    pub fn af_bin_hz(&self, b: usize) -> f64 {
        (4 * b) as f64 * self.rate / 1024.0 + 1.5 * self.rate / 1024.0
    }
}
