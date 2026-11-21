//! The voice codec 4 decoder-side enhancer: restoration noise, the enhancer post-filter, loudness, comfort-noise shaping, the high-band
//! harmonic extension and smoothing, de-click, the burst limit and the optional noise reduction. It changes only the output.

use super::core::{CoreDecoder, PitchRule, PostFilter};
use super::decoder::{DecoderSettings, Upsampler};
use super::encoder::NoiseSuppressor;
use super::highband::{HbDecoder, EDGES, WIDTH};
use super::layout::Layout;
use super::lpc::{synth_step, ORDER};
use super::math::{db, lin, pow10, Lcg, Mdct};

/// When the enhancer runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EnhancerMode {
    /// On when the speech bits per 20 ms (without the type bit) are below 140: the 3.2, 4.8 and 6.0 profiles.
    Auto,
    /// Always on.
    On,
    /// Always off.
    Off,
}

/// One row of the enhancer strength table.
#[derive(Clone, Copy)]
struct Strength {
    gn: f64,
    gd: f64,
    pitch: f64,
    tilt: f64,
    fill: f64,
    harmonic: f64,
    restore_db: f64,
}

const fn st(
    gn: f64,
    gd: f64,
    pitch: f64,
    tilt: f64,
    fill: f64,
    harmonic: f64,
    restore_db: f64,
) -> Strength {
    Strength {
        gn,
        gd,
        pitch,
        tilt,
        fill,
        harmonic,
        restore_db,
    }
}

const STRENGTH: [(u32, Strength); 6] = [
    (0, st(0.55, 0.72, 0.50, 0.45, 0.30, 0.30, 2.0)),
    (80, st(0.55, 0.71, 0.45, 0.45, 0.0, 0.30, 1.0)),
    (110, st(0.55, 0.70, 0.40, 0.45, 0.0, 0.30, 0.8)),
    (140, st(0.60, 0.75, 0.40, 0.80, 0.0, 0.30, 0.5)),
    (200, st(0.65, 0.75, 0.25, 0.80, 0.0, 0.30, 0.4)),
    (280, st(0.70, 0.75, 0.15, 0.80, 0.0, 0.30, 0.3)),
];

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// The enhancer state.
pub(crate) struct Enhancer {
    s: Strength,
    postfilter: bool,
    rate16: bool,
    g1: Lcg,
    ghb: Lcg,
    rmem: [f32; ORDER],
    pf: PostFilter,
    lost_run: u32,
    was_lost: u32,
    comfort_e: f64,
    comfort_g: f64,
    up: Upsampler,
    ola: [f32; 80],
    smooth: [f64; 6],
    have: bool,
    last_sample: f32,
    last_step: f32,
    recent_rms: f64,
    noise_db: f64,
    nr: Option<Box<NoiseSuppressor>>,
    mdct: Mdct,
    r16: [f32; 720],
}

impl Enhancer {
    /// True when the settings and the layout turn the enhancer on.
    pub(crate) fn wanted(lay: &Layout, s: DecoderSettings) -> bool {
        match s.enhancer {
            EnhancerMode::On => true,
            EnhancerMode::Off => false,
            EnhancerMode::Auto => lay.enh_b20 < 140,
        }
    }

    pub(crate) fn new(lay: &Layout, s: DecoderSettings) -> Self {
        let strength = STRENGTH
            .iter()
            .rev()
            .find(|(m, _)| *m <= lay.enh_b20)
            .map(|r| r.1)
            .unwrap_or(STRENGTH[0].1);
        Self {
            s: strength,
            postfilter: s.postfilter,
            rate16: lay.hb_on,
            g1: Lcg(0x454E_4831),
            ghb: Lcg(0x454E_4832),
            rmem: [0.0; ORDER],
            pf: PostFilter::new(),
            lost_run: 0,
            was_lost: 0,
            comfort_e: 0.0,
            comfort_g: 1.0,
            up: Upsampler::new(),
            ola: [0.0; 80],
            smooth: [-100.0; 6],
            have: false,
            last_sample: 0.0,
            last_step: 0.0,
            recent_rms: 0.0,
            noise_db: -100.0,
            nr: s
                .noise_reduction
                .then(|| Box::new(NoiseSuppressor::new(lay.hb_on, true))),
            mdct: Mdct::new(),
            r16: [0.0; 720],
        }
    }

    /// The comfort-noise stage is on (it weights the comfort-noise LPC by 0.92).
    pub(crate) fn comfort_on(&self) -> bool {
        true
    }

    /// The restart after comfort noise.
    pub(crate) fn restart(&mut self) {
        self.pf.clear_history();
        self.rmem = [0.0; ORDER];
        self.lost_run = 0;
    }

    /// The low band of a speech or concealed frame, in place on the raw core synthesis.
    pub(crate) fn low_band(
        &mut self,
        lay: &Layout,
        core: &CoreDecoder,
        concealed: bool,
        lb: &mut [f32],
    ) {
        self.was_lost = self.lost_run;
        self.lost_run = if concealed { self.lost_run + 1 } else { 0 };
        let l = lay.l;
        let gain = pow10(self.s.restore_db / 20.0) as f32;
        let mut e = 0f64;
        for s in 0..lay.s {
            let info = core.sub[s];
            let x = &mut lb[s * l..(s + 1) * l];
            let mut pn = 0f64;
            if !info.random && info.gc > 0.0 && info.ep > 0.0 {
                let u = 1.0 - clamp01(info.gp);
                pn = self.s.fill * u * u * info.gc * info.gc * info.ep;
            }
            let sd = pn.sqrt() as f32;
            for v in x.iter_mut() {
                let ex = sd * self.g1.gauss();
                *v += synth_step(&info.aq, &mut self.rmem, ex);
            }
            let mut y = [0f32; 80];
            if self.postfilter {
                let w = {
                    let w = 0.55 + 0.45 * clamp01((info.gp - 0.2) / 0.5);
                    if concealed {
                        0.6 * w
                    } else {
                        w
                    }
                };
                let gd = self.s.gn + w * (self.s.gd - self.s.gn);
                let allowed = info.gp > 0.3 && !(concealed && self.lost_run > 2);
                let rule = PitchRule {
                    allowed,
                    strength: self.s.pitch,
                    enhancer: true,
                    lost: concealed,
                };
                self.pf.run(
                    &info.aq,
                    info.ti,
                    info.gp,
                    self.s.gn,
                    gd,
                    x,
                    &mut y[..l],
                    Some(rule),
                    Some(self.s.tilt),
                );
                x.copy_from_slice(&y[..l]);
            }
            for v in x.iter_mut() {
                *v *= gain;
                e += *v as f64 * *v as f64;
            }
        }
        let d = db(e / lay.nc as f64 + 1e-3);
        if self.noise_db < -90.0 {
            self.noise_db = d;
        } else if d < self.noise_db {
            self.noise_db = 0.6 * self.noise_db + 0.4 * d;
        } else {
            self.noise_db += 0.05 * lay.frame_ms as f64 / 20.0;
        }
    }

    /// The low band of a comfort-noise frame, in place.
    pub(crate) fn comfort_low(&mut self, lay: &Layout, lb: &mut [f32]) {
        let nc = lay.nc;
        let mut e = 0f64;
        for &v in lb.iter() {
            e += v as f64 * v as f64;
        }
        e = e / nc as f64 + 1e-3;
        self.noise_db = db(e);
        self.comfort_e = if self.comfort_e <= 0.0 {
            e
        } else {
            0.7 * self.comfort_e + 0.3 * e
        };
        let g = (self.comfort_e / e).sqrt().clamp(0.7, 1.4);
        let old = self.comfort_g;
        for (i, v) in lb.iter_mut().enumerate() {
            *v = (*v as f64 * (old + (g - old) * (i + 1) as f64 / nc as f64)) as f32;
        }
        self.comfort_g = g;
    }

    /// The high band of a speech, concealed or comfort frame. `levels` holds the band levels per block. For comfort frames,
    /// `core` is `None`.
    pub(crate) fn highband(
        &mut self,
        lay: &Layout,
        core: Option<&CoreDecoder>,
        levels: &[[f64; 6]],
        out: &mut [f32],
    ) {
        let nc = lay.nc;
        let harmonic = match core {
            Some(c) => {
                let mut u = [0f32; 360];
                u[..nc].copy_from_slice(&c.frame_exc[..nc]);
                let t = c.sub[lay.s - 1].ti.max(20);
                for k in 0..40 {
                    u[nc + k] = u[nc + k - t];
                }
                self.up.run(&u[..nc], &mut self.r16[..2 * nc]);
                let mut copy = self.up.clone();
                copy.run(&u[nc..nc + 40], &mut self.r16[2 * nc..2 * nc + 80]);
                for v in self.r16[..2 * nc + 80].iter_mut() {
                    *v = v.abs();
                }
                true
            }
            None => {
                let zeros = [0f32; 320];
                let mut sink = [0f32; 640];
                self.up.run(&zeros[..nc], &mut sink[..2 * nc]);
                false
            }
        };
        for (b, lv) in levels.iter().enumerate() {
            let mut lvl = [0f64; 6];
            for j in 0..6 {
                if !self.have {
                    self.smooth[j] = lv[j];
                } else {
                    let k = if lv[j] > self.smooth[j] { 0.9 } else { 0.7 };
                    self.smooth[j] += k * (lv[j] - self.smooth[j]);
                }
                lvl[j] = self.smooth[j];
            }
            self.have = true;
            let mut v = 0f64;
            let mut xh = [0f32; 80];
            if harmonic {
                let c = core.map(|c| c.sub).unwrap_or_default();
                let s = ((40 * b + 16) / lay.l).min(lay.s - 1);
                v = self.s.harmonic * clamp01((c[s].gp - 0.35) / 0.5);
                if v > 0.0 {
                    self.mdct.forward(&self.r16[80 * b..80 * b + 160], &mut xh);
                }
            }
            let mut x = [0f32; 80];
            for j in 0..6 {
                if lvl[j] < -60.0 {
                    continue;
                }
                let a = lin(lvl[j]).sqrt();
                let mut vj = v * (1.0 - 0.1 * j as f64);
                let mut ah = 0f64;
                if vj > 0.0 {
                    let mut eh = 0f64;
                    for &h in &xh[EDGES[j]..EDGES[j + 1]] {
                        eh += h as f64 * h as f64;
                    }
                    eh /= WIDTH[j];
                    if eh > 1e-6 {
                        ah = (vj / eh).sqrt();
                    } else {
                        vj = 0.0;
                    }
                }
                let an = (1.0 - vj).sqrt();
                for (k, xk) in x.iter_mut().enumerate().take(EDGES[j + 1]).skip(EDGES[j]) {
                    let nz = if self.ghb.next() & 0x10000 != 0 {
                        1.0
                    } else {
                        -1.0
                    };
                    let val = if ah > 0.0 {
                        a * (an * nz + ah * xh[k] as f64)
                    } else {
                        a * (an * nz)
                    };
                    *xk = val as f32;
                }
            }
            self.mdct
                .inverse_ola(&x, &mut self.ola, &mut out[b * 80..(b + 1) * 80]);
        }
    }

    /// The comfort high band: the plain levels go to the high-band decoder as "last".
    pub(crate) fn comfort_high(
        &mut self,
        lay: &Layout,
        levels: &[[f64; 6]],
        hb: &mut HbDecoder,
        out: &mut [f32],
    ) {
        hb.last = levels[0];
        hb.have = true;
        self.highband(lay, None, levels, out);
    }

    /// The output stage on the merged frame (section 3.9, output stage).
    pub(crate) fn output(&mut self, lay: &Layout, speech: bool, comfort: bool, x: &mut [f32]) {
        let n = x.len();
        if speech && self.was_lost > 0 {
            let d = self.last_sample - x[0];
            if d.abs() > 4.0 * self.last_step + 64.0 {
                let r = if self.rate16 { 0.94f32 } else { 0.88 };
                let mut f = d;
                let mut i = 0;
                while f.abs() > 0.5 && i < n {
                    x[i] += f;
                    f *= r;
                    i += 1;
                }
            }
            if self.was_lost * lay.frame_ms <= 120 && self.recent_rms > 100.0 {
                let bl = if self.rate16 { 40 } else { 20 };
                let nbk = 4;
                let mut g0 = 1f64;
                for b in 0..=nbk {
                    let mut g = 1f64;
                    let blk = &mut x[b * bl..((b + 1) * bl).min(n)];
                    if b < nbk {
                        let mut s = 0f64;
                        for &v in blk.iter() {
                            s += v as f64 * v as f64;
                        }
                        let rms = (s / bl as f64).sqrt();
                        let lim = 4.0 * self.recent_rms;
                        if rms > lim {
                            g = (lim / rms).max(0.25);
                        }
                    }
                    for (i, v) in blk.iter_mut().enumerate() {
                        *v = (*v as f64 * (g0 + (g - g0) * (i + 1) as f64 / bl as f64)) as f32;
                    }
                    g0 = g;
                }
            }
            self.was_lost = 0;
        }
        if let Some(nr) = self.nr.as_mut() {
            nr.run(x, comfort);
        }
        let rn = if self.rate16 { 160 } else { 80 };
        let mut s = 0f64;
        for &v in &x[n - rn..] {
            s += v as f64 * v as f64;
        }
        self.recent_rms = (s / rn as f64).sqrt();
        let sn = if self.rate16 { 16 } else { 8 };
        let mut st = 0f32;
        for i in n - sn..n {
            st += (x[i] - x[i - 1]).abs();
        }
        self.last_step = st / sn as f32;
        self.last_sample = x[n - 1];
    }
}
