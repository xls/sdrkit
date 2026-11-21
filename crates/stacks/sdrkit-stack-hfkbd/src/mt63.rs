//! MT63: 64 differential BPSK carriers, one 7-bit Walsh-Hadamard coded character per symbol, spread over 32 (short) or 64
//! (long) symbols by the carrier delays. The receiver finds the flat band, the symbol timing and the carrier comb, detects the
//! phase steps, de-interleaves both ways, decodes the Walsh words and follows the carriers. The generator sends the clean signal.
//!
//! Standard: P. Jalocha SP9VRC, "MT63" (published description).

use crate::decoder::HeardCommand;
use crate::decoder::{
    self, last_call, put_text, Heard, HfGenerator, Modulator, GEN_CENTRE_HZ, GEN_MESSAGE,
};
use crate::typed::Mt63Fields;
use crate::util::{db, walsh_chip, walsh_decode};
use sdrkit_algo_dsp::decim::Decimator;
use sdrkit_algo_dsp::dynamics::SmoothLimiter;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::Record;
use sdrkit_core::{
    Choice, Complex32, ConfigError, GenConfig, Generator, LockState, Schema, SeriesId, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, Status, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kept, Outbox, Port};
use sdrkit_util_text::store::TextStore;
use std::f64::consts::{PI, TAU};
use std::sync::Arc;

/// Carriers.
pub const CARRIERS: usize = 64;
/// Internal samples between symbols.
pub const SYMBOL: usize = 200;
/// Internal samples of a pulse.
pub const PULSE: usize = 512;

/// The pulse: symmetric over 512 samples, 1.0 at sample 256, below 0.02 one symbol (200 samples) from the peak, zero outside.
pub fn pulse(n: f64) -> f64 {
    let x = n - 256.0;
    if x.abs() >= 256.0 {
        return 0.0;
    }
    (-0.5 * (x / 80.0).powi(2)).exp() * (PI * x / 512.0).cos()
}

/// The carrier delays (symbols) of an interleave: carrier `i` waits the sum of the first `i` steps, modulo the length.
pub fn delays(long: bool) -> [usize; CARRIERS] {
    let len = if long { 64 } else { 32 };
    let mut d = [0usize; CARRIERS];
    let mut acc = 0;
    for (i, x) in d.iter_mut().enumerate() {
        *x = acc % len;
        let step = if long { (i + 1) % 64 } else { 4 + i % 4 };
        acc += step;
    }
    d
}

/// The chips of a code (0 to 127) over the 64 carriers: +1 or -1.
pub fn chips(code: u8) -> [f32; CARRIERS] {
    let mut c = [0.0; CARRIERS];
    for (i, x) in c.iter_mut().enumerate() {
        *x = walsh_chip((code & 127) as u32, i as u32, 6);
    }
    c
}

/// Decodes 64 soft chips: (code, quality = peak over the rms of the other correlations).
pub fn decode(soft: &[f32; CARRIERS], work: &mut [f32; CARRIERS]) -> (u8, f32) {
    let (code, best, rest) = walsh_decode(soft, work);
    let rms = (rest / 63.0).max(1e-20).sqrt();
    (code as u8, best / rms)
}

sdrkit_core::choice_enum! {
    /// The band width to look for.
    pub enum WidthChoice {
        /// Any of the three.
        Auto = ("auto", "Auto"),
        /// 500 Hz.
        W500 = ("500", "500 Hz"),
        /// 1000 Hz.
        W1000 = ("1000", "1000 Hz"),
        /// 2000 Hz.
        W2000 = ("2000", "2000 Hz"),
    }
}

impl WidthChoice {
    /// The width in Hz, `None` for any.
    pub fn hz(self) -> Option<f64> {
        match self {
            WidthChoice::Auto => None,
            WidthChoice::W500 => Some(500.0),
            WidthChoice::W1000 => Some(1000.0),
            WidthChoice::W2000 => Some(2000.0),
        }
    }
}

sdrkit_core::choice_enum! {
    /// The interleave to decode.
    pub enum InterleaveChoice {
        /// The one the signal uses.
        Auto = ("auto", "Auto"),
        /// Short (32 symbols).
        Short = ("short", "Short"),
        /// Long (64 symbols).
        Long = ("long", "Long"),
    }
}

/// The `bandwidth` setting.
pub const BANDWIDTH: Setting<field::Choice<WidthChoice>> =
    Setting::choice("bandwidth", "Bandwidth", WidthChoice::Auto);
/// The `interleave` setting.
pub const INTERLEAVE_SETTING: Setting<field::Choice<InterleaveChoice>> =
    Setting::choice("interleave", "Interleave", InterleaveChoice::Auto);

sdrkit_core::settings! {
    /// The settings of [`Mt63Decoder`].
    pub enum Mt63Setting {
        /// The band width.
        Bandwidth = BANDWIDTH,
        /// The interleave.
        Interleave = INTERLEAVE_SETTING,
    }
}

sdrkit_core::settings! {
    /// The test-signal settings of the MT63 stack.
    pub enum Mt63GenSetting {
        /// The text sent.
        Message = GEN_MESSAGE,
        /// The audio centre.
        CentreHz = GEN_CENTRE_HZ,
    }
}

/// The carrier signs of a transmission. Each symbol enters one character; carrier i sends bit i of the character entered d(i)
/// symbols before ([`delays`]); before the first character, the chips of NUL or, after [`Mt63Signs::random_start`], random
/// chips. A bit 1 (chip -1) keeps the sign of the carrier, a bit 0 (chip +1) reverses it. Every sign starts at +1.
#[derive(Clone, Debug)]
pub struct Mt63Signs {
    codes: Vec<[f32; CARRIERS]>,
    signs: [f32; CARRIERS],
    sym: usize,
    dl: [usize; CARRIERS],
}

impl Mt63Signs {
    /// The signs of the long or the short interleave.
    pub fn new(long: bool) -> Self {
        let len = if long { 64 } else { 32 };
        Mt63Signs {
            codes: vec![chips(0); len + 1],
            signs: [1.0; CARRIERS],
            sym: 0,
            dl: delays(long),
        }
    }

    /// Random chips from the generator seeded with `seed` before the first character.
    pub fn random_start(mut self, seed: u64) -> Self {
        let mut r = Rng::new(seed);
        for c in &mut self.codes {
            for x in c.iter_mut() {
                *x = if r.bit() == 1 { 1.0 } else { -1.0 };
            }
        }
        self
    }

    /// Enters the character `c` (its 7 low bits) and returns the signs of the symbol.
    pub fn push(&mut self, c: u8) -> &[f32; CARRIERS] {
        let len = self.codes.len();
        self.codes[self.sym % len] = chips(c & 0x7F);
        for (i, p) in self.signs.iter_mut().enumerate() {
            let k = (self.sym + len - self.dl[i] % len) % len;
            if self.codes[k][i] > 0.0 {
                *p = -*p;
            }
        }
        self.sym += 1;
        &self.signs
    }
}

/// The MT63 transmitter.
pub struct Mt63Modulator {
    width: f64,
    rate: f64,
    text: Vec<u8>,
    ti: usize,
    signs: Mt63Signs,
    phase: Vec<[f32; CARRIERS]>,
    sym: i64,
    t: f64,
    carrier: [Complex32; CARRIERS],
    rot: [Complex32; CARRIERS],
}

impl Mt63Modulator {
    /// Sends `text` again and again in a band `width` Hz wide at `centre` Hz, with the long or the short interleave.
    pub fn new(width: f64, long: bool, rate: f64, centre: f64, text: &str) -> Self {
        let spacing = width / 64.0;
        let mut rot = [Complex32::default(); CARRIERS];
        for (c, r) in rot.iter_mut().enumerate() {
            let f = centre + (c as f64 - 31.5) * spacing;
            let w = TAU * f / rate;
            *r = Complex32::new(w.cos() as f32, w.sin() as f32);
        }
        Mt63Modulator {
            width,
            rate,
            text: text.as_bytes().to_vec(),
            ti: 0,
            signs: Mt63Signs::new(long),
            phase: vec![[1.0; CARRIERS]; 4],
            sym: -1,
            t: 0.0,
            carrier: [Complex32::new(1.0, 0.0); CARRIERS],
            rot,
        }
    }

    fn next_symbol(&mut self) {
        self.sym += 1;
        let c = self.text[self.ti % self.text.len()];
        self.ti += 1;
        self.phase[(self.sym.rem_euclid(4)) as usize] = *self.signs.push(c);
    }
}

impl Modulator for Mt63Modulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        let r = 2.0 * self.width;
        let step = r / self.rate;
        for o in out {
            // Symbol s's pulse spans internal samples s*200 .. s*200+512.
            let newest = (self.t / SYMBOL as f64).floor() as i64;
            while self.sym < newest {
                self.next_symbol();
            }
            let mut v = Complex32::default();
            for back in 0..3i64 {
                let s = newest - back;
                if s < 0 {
                    continue;
                }
                let u = self.t - (s as f64 * SYMBOL as f64);
                let p = pulse(u) as f32;
                if p == 0.0 {
                    continue;
                }
                let ph = &self.phase[(s.rem_euclid(4)) as usize];
                for c in 0..CARRIERS {
                    v += self.carrier[c] * (ph[c] * p);
                }
            }
            for c in 0..CARRIERS {
                self.carrier[c] *= self.rot[c];
            }
            *o = v * (1.0 / 8.0);
            self.t += step;
            if (self.t as u64).is_multiple_of(4096) {
                for c in &mut self.carrier {
                    *c /= c.norm();
                }
            }
        }
    }
}

const PHASES: usize = 8;
const QMIN: f32 = 5.0;

/// The MT63 receiver.
pub struct Mt63Decoder {
    force_width: Option<f64>,
    force_long: Option<bool>,
    dial: f64,
    // band finder
    fspec: WindowedFft,
    fbuf: Vec<Complex32>,
    favg: Vec<f32>,
    fframes: u64,
    scratch: Vec<f32>,
    floor: f32,
    // demodulator
    width: f64,
    centre: f64,
    afc: f64,
    nco: Nco,
    dec: Option<Decimator<Complex32>>,
    mixed: Vec<Complex32>,
    low: Vec<Complex32>,
    hist: SampleRing<Complex32>,
    spec: WindowedFft,
    n: u64,
    prev: Vec<Vec<Complex32>>,
    metric: Vec<f32>,
    best: (usize, i32),
    soft: Vec<[f32; CARRIERS]>,
    nsym: u64,
    dl: [[usize; CARRIERS]; 2],
    qual: [f32; 2],
    quality: f32,
    long: bool,
    state: u8,
    locks: u32,
    afc_lock: f64,
    snr: f32,
    levels: [f32; CARRIERS],
    work: [f32; CARRIERS],
    chips: [f32; CARRIERS],
    store: TextStore,
    unclear: u64,
    heard: Heard,
    status: Status,
    width_choice: WidthChoice,
    interleave_choice: InterleaveChoice,
    series: Option<[SeriesId; 2]>,
    t_in: u64,
    since_find: usize,
    shift: Vec<Complex32>,
    last_dec: u64,
    afc_ph: Complex32,
    afc_rot: Complex32,
    spare: Vec<Complex32>,
}

impl Mt63Decoder {
    /// A receiver at the default settings: the width and the interleave found by themselves.
    pub fn new() -> Self {
        let mut d = Mt63Decoder {
            force_width: None,
            force_long: None,
            dial: 0.0,
            fspec: WindowedFft::new(2048, 2048, Window::Hann)
                .expect("a window that fits the transform"),
            fbuf: Vec::with_capacity(2048),
            favg: vec![0.0; 2048],
            fframes: 0,
            scratch: Vec::with_capacity(2048),
            floor: 0.0,
            width: 0.0,
            centre: 0.0,
            afc: 0.0,
            nco: Nco::new(8000.0, 0.0),
            dec: None,
            mixed: Vec::new(),
            low: Vec::new(),
            hist: SampleRing::new(PULSE).expect("a ring of at least one sample"),
            spec: WindowedFft::new(PULSE, PULSE, Window::Rectangular)
                .expect("a window that fits the transform"),
            n: 0,
            prev: vec![vec![Complex32::default(); PULSE]; PHASES],
            metric: Vec::new(),
            best: (0, 0),
            soft: vec![[0.0; CARRIERS]; 66],
            nsym: 0,
            dl: [delays(false), delays(true)],
            qual: [0.0; 2],
            quality: 0.0,
            long: false,
            state: 0,
            locks: 0,
            afc_lock: 0.0,
            snr: 0.0,
            levels: [0.0; CARRIERS],
            work: [0.0; CARRIERS],
            chips: [0.0; CARRIERS],
            store: decoder::store(4000, 80),
            unclear: 0,
            heard: Heard::new("MT63"),
            status: Status::default(),
            width_choice: WidthChoice::Auto,
            interleave_choice: InterleaveChoice::Auto,
            series: None,
            t_in: 0,
            since_find: 0,
            shift: (-512..512)
                .map(|b: i32| {
                    Complex32::from_polar(
                        1.0,
                        (-TAU * b as f64 * SYMBOL as f64 / PULSE as f64) as f32,
                    )
                })
                .collect(),
            last_dec: 0,
            afc_ph: Complex32::new(1.0, 0.0),
            afc_rot: Complex32::new(1.0, 0.0),
            spare: Vec::with_capacity(PULSE),
        };
        let w: Vec<f32> = (0..PULSE).map(|n| pulse(n as f64) as f32).collect();
        d.spec.window_mut().copy_from_slice(&w);
        d
    }

    fn drop_sync(&mut self) {
        self.width = 0.0;
        self.state = 0;
        self.fframes = 0;
        self.favg.iter_mut().for_each(|v| *v = 0.0);
        self.fbuf.clear();
        self.dec = None;
    }

    /// Finds the flat band (500, 1000 or 2000 Hz wide) in the 2048-point spectrum.
    fn find_band(&mut self) -> Option<(f64, f64, f32)> {
        let bin = 8000.0 / 2048.0;
        let lo = (100.0 / bin) as usize;
        let hi = (3900.0 / bin) as usize;
        self.floor = crate::util::median(&self.favg[lo..hi], &mut self.scratch).max(1e-20);
        let mut best: Option<(f64, f64, f32)> = None;
        let widths: &[f64] = match self.force_width {
            Some(w) => &[w][..],
            None => &[500.0, 1000.0, 2000.0][..],
        };
        let widths = widths.to_vec();
        for w in widths {
            let nb = (w / bin).round() as usize;
            let side = nb / 8;
            if lo + side + nb + side >= hi {
                continue;
            }
            let mut pre = vec![0.0f32; hi + 1];
            for i in lo..hi {
                pre[i + 1] = pre[i] + self.favg[i];
            }
            let sum = |a: usize, b: usize| pre[b] - pre[a];
            // The position: the most power inside the box against beside it. Then the width by the ratio.
            let mut pos: Option<(usize, f32)> = None;
            for s in lo + side..hi - nb - side {
                let inside = sum(s, s + nb) / nb as f32;
                let beside = (sum(s - side, s) + sum(s + nb, s + nb + side)) / (2 * side) as f32;
                let d = inside - beside;
                if pos.is_none_or(|p| d > p.1) {
                    pos = Some((s, d));
                }
            }
            let Some((s, _)) = pos else { continue };
            let inside = sum(s, s + nb) / nb as f32;
            let beside = (sum(s - side, s) + sum(s + nb, s + nb + side)) / (2 * side) as f32;
            let q = nb / 4;
            let qs: Vec<f32> = (0..4)
                .map(|k| sum(s + k * q, s + (k + 1) * q) / q as f32)
                .collect();
            let flat = qs.iter().cloned().fold(f32::MAX, f32::min)
                / qs.iter().cloned().fold(0.0, f32::max).max(1e-20);
            if inside < 4.0 * self.floor || flat < 0.4 {
                continue;
            }
            let score = (inside / beside.max(self.floor)) * flat;
            if best.is_none_or(|b| score > b.2) {
                best = Some((w, (s as f64 + nb as f64 / 2.0) * bin, score));
            }
        }
        best.filter(|b| b.2 > 3.0)
    }

    fn start(&mut self, width: f64, centre: f64) {
        self.width = width;
        self.centre = centre;
        self.afc = 0.0;
        let r = 2.0 * width;
        let d = (8000.0 / r).round() as usize;
        self.nco = Nco::new(8000.0, -centre);
        self.dec = (d > 1).then(|| Decimator::new(8000.0, d, 0.52 * width, 70.0));
        self.hist.reset();
        self.n = 0;
        for p in &mut self.prev {
            p.iter_mut().for_each(|v| *v = Complex32::default());
        }
        self.metric = vec![0.0; PHASES * 9];
        self.last_dec = 0;
        self.afc_ph = Complex32::new(1.0, 0.0);
        self.afc_rot = Complex32::new(1.0, 0.0);
        self.nsym = 0;
        self.qual = [0.0; 2];
        self.state = 1;
        self.heard
            .logs
            .push(format!("MT63-{width:.0}: synchronising at {centre:.0} Hz"));
    }

    fn bin_hz(&self) -> f64 {
        2.0 * self.width / PULSE as f64
    }

    /// One FFT of the last pulse length at timing phase `ph`.
    fn frame(&mut self, ph: usize, sink: &mut Outbox) {
        // The new frame goes into a spare buffer (no allocation), which then becomes this phase's previous frame.
        let mut bins = std::mem::take(&mut self.spare);
        bins.clear();
        bins.extend_from_slice(self.spec.run(self.hist.last(PULSE)));
        let n = PULSE as i32;
        // The comb: carrier c at bin 4c - 126 + k0 from the centre.
        for k in 0..9i32 {
            let k0 = k - 4;
            let mut sum2 = Complex32::default();
            let mut zs = [Complex32::default(); CARRIERS];
            for (c, z) in zs.iter_mut().enumerate() {
                let bi = 4 * c as i32 - 126 + k0;
                let b = bi.rem_euclid(n) as usize;
                // The window moved by a symbol: a steady carrier at bin b turns by 2 pi b 200 / 512.
                *z = bins[b] * self.prev[ph][b].conj() * self.shift[(bi + 512) as usize];
                sum2 += *z * *z;
            }
            let rot = 0.5 * sum2.arg();
            let r = Complex32::from_polar(1.0, -rot);
            let mut m = 0.0;
            let mut a = 0.0;
            for z in &zs {
                let w = *z * r;
                m += w.re.abs() - w.im.abs();
                a += w.norm();
            }
            let i = ph * 9 + k as usize;
            let v = if a > 0.0 { m / a } else { 0.0 };
            // A comb shifted by a whole carrier leaves one end carrier outside the signal and finds energy beyond the other end.
            let e = |c: i32| bins[(4 * c - 126 + k0).rem_euclid(n) as usize].norm_sqr();
            let inner = (0..CARRIERS as i32).map(e).sum::<f32>() / CARRIERS as f32;
            let beyond = 0.5 * (e(-1) + e(64));
            let edge = if inner > 0.0 {
                (beyond / inner).min(2.0)
            } else {
                0.0
            };
            self.metric[i] += 0.15 * (v - 0.5 * edge - self.metric[i]);
        }
        let prev = std::mem::replace(&mut self.prev[ph], bins);
        self.spare = prev;
        let prev = &self.spare;
        // Decide at the best phase only.
        let (bp, bk) = {
            let mut best = (0usize, 0usize, f32::MIN);
            for p in 0..PHASES {
                for k in 0..9 {
                    let v = self.metric[p * 9 + k];
                    if v > best.2 {
                        best = (p, k, v);
                    }
                }
            }
            (best.0, best.1 as i32 - 4)
        };
        // The decision phase moves one step at a time, and only for a clearly better one. A symbol is taken once per period.
        let cur_m = self.metric[self.best.0 * 9 + (bk + 4) as usize];
        let new_m = self.metric[bp * 9 + (bk + 4) as usize];
        let mut dp = self.best.0;
        if bp != dp && new_m > cur_m + 0.03 {
            let d = (bp + PHASES - dp) % PHASES;
            dp = if d <= PHASES / 2 {
                (dp + 1) % PHASES
            } else {
                (dp + PHASES - 1) % PHASES
            };
        }
        self.best = (dp, bk);
        let bp = dp;
        if ph != bp || self.n < self.last_dec + (SYMBOL as u64 * 3 / 4) {
            return;
        }
        self.last_dec = self.n;
        let cur = &self.prev[ph];
        let mut zs = [Complex32::default(); CARRIERS];
        let mut sum2 = Complex32::default();
        let mut amp = 0.0;
        for (c, z) in zs.iter_mut().enumerate() {
            let bi = 4 * c as i32 - 126 + bk;
            let b = bi.rem_euclid(n) as usize;
            *z = cur[b] * prev[b].conj() * self.shift[(bi + 512) as usize];
            sum2 += *z * *z;
            amp += z.norm();
            self.levels[c] += 0.1 * (cur[b].norm_sqr() - self.levels[c]);
        }
        let rot = 0.5 * sum2.arg();
        // AFC from the residual rotation (a step per symbol).
        let t_sym = SYMBOL as f64 / (2.0 * self.width);
        let df = rot as f64 / (TAU * t_sym);
        self.afc += 0.1 * df;
        let w = -TAU * self.afc / (2.0 * self.width);
        self.afc_rot = Complex32::new(w.cos() as f32, w.sin() as f32);

        let r = Complex32::from_polar(1.0, -rot);
        let norm = (amp / CARRIERS as f32).max(1e-20);
        let slot = (self.nsym % self.soft.len() as u64) as usize;
        let mut err = 0.0;
        for (c, z) in zs.iter().enumerate() {
            let w = *z * r;
            // Positive when the phase reversed (chip +1).
            self.soft[slot][c] = -w.re / norm;
            err += (w.im / norm).powi(2);
        }
        let e = err / CARRIERS as f32;
        let snr = 10.0 * (1.0 / (2.0 * e.max(1e-4))).log10();
        self.snr += 0.1 * (snr - self.snr);
        self.nsym += 1;
        // Decode the character whose last chip just arrived, both ways.
        let mut out: [Option<(u8, f32)>; 2] = [None, None];
        for (w, il) in [false, true].iter().enumerate() {
            let len = if *il { 64 } else { 32 };
            if self.nsym < len as u64 + 1 {
                continue;
            }
            let k = self.nsym - 1 - (len as u64 - 1);
            for i in 0..CARRIERS {
                let s = (k + self.dl[w][i] as u64) % self.soft.len() as u64;
                self.chips[i] = self.soft[s as usize][i];
            }
            let (code, q) = decode(&self.chips, &mut self.work);
            self.qual[w] += 0.1 * (q - self.qual[w]);
            out[w] = Some((code, q));
        }
        let long = match self.force_long {
            Some(l) => l,
            None => self.qual[1] > self.qual[0],
        };
        if long != self.long && self.force_long.is_none() {
            self.long = long;
        }
        self.quality = self.qual[long as usize];
        let was = self.state;
        self.state = if self.quality >= QMIN { 2 } else { 1 };
        if self.state == 2 && was != 2 {
            self.locks += 1;
            self.afc_lock = self.afc;
            self.heard.logs.push(format!(
                "MT63-{:.0} {}: decoding",
                self.width,
                if long { "long" } else { "short" }
            ));
        }
        if let Some((code, q)) = out[long as usize] {
            if self.state == 2 {
                if q >= QMIN {
                    if code != 0 {
                        let time = self.t_in as f64 / 8000.0;
                        let f = self.dial + self.centre;
                        let snr = self.snr;
                        self.heard.char(
                            &mut self.store,
                            sink,
                            code,
                            time,
                            (q * 10.0).min(100.0) as u8,
                            0,
                            f,
                            snr,
                        );
                    }
                } else {
                    self.unclear += 1;
                }
            }
        }
    }
}

impl Default for Mt63Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl sdrkit_graph::Decoder for Mt63Decoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.mt63"
    }
    fn outputs(&self) -> Vec<Port> {
        decoder::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        Mt63Setting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match Mt63Setting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Mt63Setting::Bandwidth => {
                self.width_choice = BANDWIDTH.read(id, value)?;
                self.force_width = self.width_choice.hz();
                self.drop_sync();
            }
            Mt63Setting::Interleave => {
                self.interleave_choice = INTERLEAVE_SETTING.read(id, value)?;
                self.force_long = match self.interleave_choice {
                    InterleaveChoice::Short => Some(false),
                    InterleaveChoice::Long => Some(true),
                    InterleaveChoice::Auto => None,
                };
                if let Some(l) = self.force_long {
                    self.long = l;
                }
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (
                Mt63Setting::Bandwidth,
                Value::Choice(self.width_choice.index()),
            ),
            (
                Mt63Setting::Interleave,
                Value::Choice(self.interleave_choice.index()),
            ),
        ])
    }
    fn results(&self) -> Published {
        crate::typed::results("mt63")
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.dial = input.tuned_hz;
        self.drop_sync();
        Ok(decoder::formats())
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.drop_sync();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        // In pieces that end where the band finder decides, so the result does not depend on how the input is cut.
        let mut rest = x;
        while !rest.is_empty() {
            let n = rest.len().min(2048 - self.fbuf.len());
            let (piece, r) = rest.split_at(n);
            rest = r;
            self.demod_piece(piece, out);
            self.fbuf.extend_from_slice(piece);
            self.t_in += n as u64;
            if self.fbuf.len() == 2048 {
                self.band_frame();
            }
        }
    }
    fn report(&mut self, res: &mut Published) {
        let w = self.width as u32;
        let il = if self.long { "long" } else { "short" };
        let st = &mut self.status;
        st.lock_state =
            [LockState::Searching, LockState::Tracking, LockState::Locked][self.state as usize];
        st.offset_hz = Some(self.afc);
        st.snr_db = (self.state == 2).then_some(self.snr);
        match self.state {
            0 => put_text(
                &mut st.summary,
                format_args!("searching for MT63 (500 / 1000 / 2000 Hz)"),
            ),
            1 => put_text(
                &mut st.summary,
                format_args!("MT63-{w}: synchronising, centre {:.0} Hz", self.centre),
            ),
            _ => put_text(
                &mut st.summary,
                format_args!(
                    "MT63-{w} {il}: decoding, centre {:.0} Hz, {:.0} dB, {} chars",
                    self.centre, self.snr, self.store.total
                ),
            ),
        }
        res.results_mut().set_status(st);
        let maxl = self.levels.iter().cloned().fold(1e-20f32, f32::max);
        let centre = self.centre
            + self.afc
            + self.best.1 as f64 * if self.width > 0.0 { self.bin_hz() } else { 0.0 };
        self.heard.update(&self.store);
        res.put(&Mt63Fields {
            bandwidth_hz: Hertz(w as f64),
            interleave: if self.long {
                InterleaveChoice::Long
            } else {
                InterleaveChoice::Short
            },
            centre_hz: Hertz(centre),
            quality_pct: (f64::from(self.quality.clamp(0.0, 1.0)) * 100.0).round(),
            floor_db: db(self.floor as f64),
            locks: self.locks as i64,
            afc_hz: Hertz(self.afc - self.afc_lock),
            snr_db: f64::from(self.snr),
            characters: self.store.total as i64,
            errors: self.unclear as i64,
            text: self.heard.text(),
            callsign: last_call(&self.store),
        });
        if self.series.is_none() {
            let d = res.results().decl();
            self.series = d
                .series_id("spectrum")
                .zip(d.series_id("carriers"))
                .map(|(a, b)| [a, b]);
        }
        if let Some([spectrum, carriers]) = self.series {
            let favg = &self.favg;
            res.results_mut().update_series(spectrum, |sp| {
                sp.points.clear();
                let bin = 8000.0 / 2048.0;
                for i in (100.0 / bin) as usize..(3900.0 / bin) as usize {
                    sp.points
                        .push([(i as f64 * bin) as f32, db(favg[i] as f64) as f32]);
                }
                true
            });
            let levels = &self.levels;
            res.results_mut().update_series(carriers, |cl| {
                cl.points.clear();
                for (c, &l) in levels.iter().enumerate() {
                    cl.points.push([c as f32, db((l / maxl) as f64) as f32]);
                }
                true
            });
        }
        self.heard.report(res);
    }
    fn commands(&self) -> &'static [&'static Command] {
        HeardCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let p = self.results();
        self.heard.invoke(id, args, &p)
    }
    fn clear(&mut self) {
        self.store.clear();
        self.heard.clear();
    }
    fn forget(&mut self, kept: &Kept) {
        self.drop_sync();
        if !kept.field("text") {
            self.store.clear();
        }
        self.heard.forget(kept);
    }
}

impl Mt63Decoder {
    fn band_frame(&mut self) {
        let p = self.fspec.run_power(&self.fbuf);
        let a = if self.fframes == 0 { 1.0 } else { 0.3 };
        for (v, &q) in self.favg.iter_mut().zip(p) {
            *v += a * (q - *v);
        }
        self.fframes += 1;
        self.fbuf.clear();
        self.since_find += 1;
        if self.fframes >= 2 && (self.state == 0 || (self.state == 1 && self.since_find >= 8)) {
            self.since_find = 0;
            if let Some((w, c, _)) = self.find_band() {
                if self.state == 0 || (w - self.width).abs() > 1.0 || (c - self.centre).abs() > 20.0
                {
                    self.start(w, c);
                }
            } else if self.state == 1 {
                self.state = 0;
                self.width = 0.0;
            }
        }
    }

    fn demod_piece(&mut self, x: &[Complex32], out: &mut Outbox) {
        if self.width == 0.0 {
            return;
        }
        self.mixed.clear();
        self.mixed.resize(x.len(), Complex32::default());
        self.nco.mix(x, &mut self.mixed);
        self.low.clear();
        match &mut self.dec {
            Some(d) => d.process(&self.mixed, &mut self.low),
            None => self.low.extend_from_slice(&self.mixed),
        }
        let hop = SYMBOL / PHASES;
        let low = std::mem::take(&mut self.low);
        for &s in &low {
            // The AFC turns the samples after the decimation, one sample at a time.
            let s = s * self.afc_ph;
            self.afc_ph *= self.afc_rot;
            self.n += 1;
            if self.n.is_multiple_of(1024) {
                self.afc_ph /= self.afc_ph.norm();
            }
            self.hist.push(s);
            if self.n.is_multiple_of(hop as u64) && self.n >= PULSE as u64 {
                let ph = ((self.n / hop as u64) % PHASES as u64) as usize;
                self.frame(ph, out);
            }
        }
        self.low = low;
    }
}

/// The MT63 test text.
pub const TEST_TEXT: &str = "CQ CQ CQ DE K0TST K0TST MT63 TEST PSE K\n";

/// The generator. It reads the settings `bandwidth` and `interleave` (`auto` sends 1000 Hz and the short interleave),
/// `gen_message` and `gen_centre_hz`.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let v = &cfg.settings;
    let w = v.get(&BANDWIDTH).hz().unwrap_or(1000.0);
    let long = v.get(&INTERLEAVE_SETTING) == InterleaveChoice::Long;
    let centre = v.get(&GEN_CENTRE_HZ);
    let m = Mt63Modulator::new(
        w,
        long,
        cfg.rate_hz,
        centre,
        &decoder::message(cfg, TEST_TEXT),
    );
    let baud = 2.0 * w / SYMBOL as f64;
    Ok(HfGenerator::boxed(
        cfg.rate_hz,
        m,
        SignalInfo::new(w)
            .with_symbol_rate(baud)
            .with_bit_rate(baud * 7.0)
            .with_power(0.24),
    ))
}

/// The lowest carrier of the sender.
pub const TX_LOW_HZ: Setting<field::Hz> = Setting::new("tx_low_hz", "Lowest carrier", "Hz", 500.0)
    .range(200.0, 3000.0, 1.0)
    .section(crate::sender::TX_SECTION)
    .help("The audio frequency of carrier 0; the 64 carriers run up from it, one band width / 64 apart.");

sdrkit_core::settings! {
    /// The settings of the MT63 sender.
    pub enum Mt63TxSetting {
        /// The band width (`auto` sends 1000 Hz).
        Bandwidth = BANDWIDTH,
        /// The interleave (`auto` sends the short one).
        Interleave = INTERLEAVE_SETTING,
        /// The lowest carrier.
        LowHz = TX_LOW_HZ,
        /// The level.
        Level = crate::sender::TX_LEVEL,
        /// The tune time.
        TuneLimit = crate::sender::TX_TUNE_LIMIT,
    }
}

/// The NUL characters at the start of a burst last at least this long, seconds.
const TX_PREAMBLE_S: f64 = 5.5;
/// The seed of the chips sent before the first character.
const TX_START_SEED: u64 = 0x63_5EED;
/// The mean power of the signal under its peak, dB.
const TX_CREST_DB: f64 = 10.0;
/// The look-ahead of the limiter, seconds.
const TX_LIMIT_AHEAD_S: f64 = 0.002;
/// The release of the limiter, seconds.
const TX_LIMIT_RELEASE_S: f64 = 0.02;

/// The MT63 coder: samples at 8 kHz around the middle of the comb. Carrier i (0 to 63) is at `low` + i width / 64,
/// starts at phase 0 at the first sample of the burst and runs on; one character per symbol (T = 100 / width s) sets the signs
/// of [`Mt63Signs`]. Each symbol is the sum of the carriers with their signs, shaped by [`pulse`] over 2.56 symbols, so three
/// symbols overlap. A burst is NUL for max(5.5 s, L + 16 symbols), the text, L + 12 NUL characters and the end of the last
/// pulse, L the interleave length (32 or 64); before the first character the carriers send random chips (a fixed seed), so
/// their signs differ. The gain puts the mean power of random data 10 dB under 1; a look-ahead limiter (2 ms) keeps every
/// sample at or under 1.
pub struct Mt63Coder {
    width: f64,
    long: bool,
    /// The middle of the comb, Hz.
    middle: f64,
    signs: Mt63Signs,
    /// Samples per symbol.
    sps: usize,
    /// The pulse at 8 kHz.
    shape: Vec<f64>,
    /// The samples from the start of the newest symbol on.
    acc: Vec<Complex32>,
    /// The sample of `acc[0]` from the start of the burst.
    t: u64,
    gain: f64,
    limit: SmoothLimiter<Complex32>,
}

impl Mt63Coder {
    /// A coder of a band `width` Hz wide from the carrier at `low` Hz, with the long or the short interleave.
    pub fn new(width: f64, long: bool, low: f64) -> Self {
        let r = crate::sender::WAVE_RATE;
        let sps = (SYMBOL as f64 * r / (2.0 * width)).round() as usize;
        let len = (PULSE as f64 * r / (2.0 * width)).round() as usize;
        let shape: Vec<f64> = (0..len)
            .map(|m| pulse(m as f64 * 2.0 * width / r))
            .collect();
        // Random signs: the mean power of a unit gain is the carriers times the pulse energy per symbol.
        let power = CARRIERS as f64 * shape.iter().map(|p| p * p).sum::<f64>() / sps as f64;
        Mt63Coder {
            width,
            long,
            middle: low + 31.5 * width / CARRIERS as f64,
            signs: Mt63Signs::new(long).random_start(TX_START_SEED),
            sps,
            shape,
            acc: vec![Complex32::default(); len],
            t: 0,
            gain: (10f64.powf(-TX_CREST_DB / 10.0) / power).sqrt(),
            limit: SmoothLimiter::new(r, TX_LIMIT_AHEAD_S, 1.0, TX_LIMIT_RELEASE_S),
        }
    }

    /// The interleave length, symbols.
    fn interleave(&self) -> usize {
        if self.long {
            64
        } else {
            32
        }
    }

    /// The NUL symbols before the text.
    pub fn preamble(&self) -> usize {
        let t = SYMBOL as f64 / (2.0 * self.width);
        ((TX_PREAMBLE_S / t).ceil() as usize).max(self.interleave() + 16)
    }

    /// The NUL symbols after the text.
    pub fn tail(&self) -> usize {
        self.interleave() + 12
    }

    /// One symbol of the character `c`: a symbol of samples out.
    fn symbol(&mut self, c: u8, out: &mut Vec<Complex32>) {
        let r = crate::sender::WAVE_RATE;
        let signs = *self.signs.push(c);
        let spacing = self.width / CARRIERS as f64;
        let mut z = [(0.0f64, 0.0f64); CARRIERS];
        let mut rot = [(0.0f64, 0.0f64); CARRIERS];
        for i in 0..CARRIERS {
            let f = (i as f64 - 31.5) * spacing / r;
            let a = TAU * (f * self.t as f64).rem_euclid(1.0);
            z[i] = (a.cos() * f64::from(signs[i]), a.sin() * f64::from(signs[i]));
            rot[i] = ((TAU * f).cos(), (TAU * f).sin());
        }
        for (m, o) in self.acc.iter_mut().enumerate() {
            let (mut re, mut im) = (0.0, 0.0);
            for (zi, ri) in z.iter_mut().zip(&rot) {
                re += zi.0;
                im += zi.1;
                *zi = (zi.0 * ri.0 - zi.1 * ri.1, zi.0 * ri.1 + zi.1 * ri.0);
            }
            let g = self.shape[m] * self.gain;
            *o += Complex32::new((re * g) as f32, (im * g) as f32);
        }
        for &z in &self.acc[..self.sps] {
            out.push(self.limit.push(z, z.norm()));
        }
        self.acc.rotate_left(self.sps);
        let len = self.acc.len();
        self.acc[len - self.sps..].fill(Complex32::default());
        self.t += self.sps as u64;
    }
}

impl crate::sender::WaveCoder for Mt63Coder {
    /// The middle of the comb: half a spacing below the centre of the band.
    fn centre_hz(&self) -> f64 {
        self.middle
    }
    fn begin(&mut self, out: &mut Vec<Complex32>) {
        for _ in 0..self.preamble() {
            self.symbol(0, out);
        }
    }
    /// A character above 255 is sent as NUL.
    fn char(&mut self, c: char, out: &mut Vec<Complex32>) {
        self.symbol(u8::try_from(c).unwrap_or(0), out);
    }
    fn end(&mut self, out: &mut Vec<Complex32>) {
        for _ in 0..self.tail() {
            self.symbol(0, out);
        }
        let rest = self.acc.len() - self.sps;
        for &z in &self.acc[..rest] {
            out.push(self.limit.push(z, z.norm()));
        }
        for _ in 0..self.limit.delay() {
            out.push(self.limit.push(Complex32::default(), 0.0));
        }
    }
}

/// The MT63 side of the sender.
#[derive(Clone, Debug)]
pub struct Mt63Modem {
    width: WidthChoice,
    interleave: InterleaveChoice,
    low: f64,
    common: crate::sender::Common,
}

impl Default for Mt63Modem {
    fn default() -> Self {
        Mt63Modem {
            width: BANDWIDTH.default,
            interleave: INTERLEAVE_SETTING.default,
            low: TX_LOW_HZ.default,
            common: crate::sender::Common::default(),
        }
    }
}

impl Mt63Modem {
    /// The band width sent, Hz: `auto` sends 1000 Hz.
    pub fn width_hz(&self) -> f64 {
        self.width.hz().unwrap_or(1000.0)
    }
    /// The long interleave is sent: `auto` sends the short one.
    pub fn long(&self) -> bool {
        self.interleave == InterleaveChoice::Long
    }
}

impl crate::sender::Modem for Mt63Modem {
    const NAME: &'static str = "hfkbd.mt63tx";
    fn schema() -> Arc<Schema> {
        Mt63TxSetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match Mt63TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Mt63TxSetting::Bandwidth => self.width = BANDWIDTH.read(id, v)?,
            Mt63TxSetting::Interleave => self.interleave = INTERLEAVE_SETTING.read(id, v)?,
            Mt63TxSetting::LowHz => self.low = TX_LOW_HZ.read(id, v)?,
            Mt63TxSetting::Level => self.common.level = crate::sender::TX_LEVEL.read(id, v)?,
            Mt63TxSetting::TuneLimit => {
                self.common.tune_s = crate::sender::TX_TUNE_LIMIT.read(id, v)?;
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (Mt63TxSetting::Bandwidth, Value::Choice(self.width.index())),
            (
                Mt63TxSetting::Interleave,
                Value::Choice(self.interleave.index()),
            ),
            (Mt63TxSetting::LowHz, Value::Hz(self.low)),
            (Mt63TxSetting::Level, Value::F64(self.common.level)),
            (
                Mt63TxSetting::TuneLimit,
                Value::Duration(self.common.tune_s),
            ),
        ])
    }
    fn common(&self) -> crate::sender::Common {
        self.common
    }
    /// Bytes 0 to 255 as typed, sent as the byte AND 127; a character above 255 refuses the text.
    fn accept(&self, text: &str, out: &mut Vec<char>) -> Result<(), crate::sender::TxError> {
        if let Some(c) = text.chars().find(|&c| u8::try_from(c).is_err()) {
            return Err(crate::sender::TxError::Character(c));
        }
        out.extend(text.chars());
        Ok(())
    }
    fn coder(&self) -> Result<crate::sender::Coding, crate::sender::TxError> {
        Ok(crate::sender::Coding::Wave(Box::new(Mt63Coder::new(
            self.width_hz(),
            self.long(),
            self.low,
        ))))
    }
    /// The centre of the band: carrier 32.
    fn centre_hz(&self) -> f64 {
        self.low + self.width_hz() / 2.0
    }
    /// The carriers and half a spacing each side.
    fn band(&self) -> (f64, f64) {
        let spacing = self.width_hz() / CARRIERS as f64;
        (self.low - spacing / 2.0, self.low + 63.5 * spacing)
    }
}

/// The MT63 sender component.
pub fn sender() -> Box<dyn sdrkit_graph::Component> {
    Box::new(crate::sender::TextSender::<Mt63Modem>::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walsh_codes_round_trip() {
        let mut w = [0.0; CARRIERS];
        for code in 0..128u8 {
            let (c, q) = decode(&chips(code), &mut w);
            assert_eq!(c, code);
            assert!(q > 50.0);
        }
    }

    #[test]
    fn delays_and_pulse() {
        for long in [false, true] {
            let d = delays(long);
            let len = if long { 64 } else { 32 };
            assert_eq!(d[0], 0);
            assert!(d.iter().all(|&x| x < len));
            assert!(*d.iter().max().unwrap() >= len / 2);
        }
        assert!((pulse(256.0) - 1.0).abs() < 1e-12);
        assert!(pulse(56.0) < 0.02 && pulse(456.0) < 0.02);
        assert_eq!(pulse(-1.0), 0.0);
        assert_eq!(pulse(512.0), 0.0);
        for k in 0..256 {
            assert!((pulse(256.0 - k as f64) - pulse(256.0 + k as f64)).abs() < 1e-12);
        }
    }
}
