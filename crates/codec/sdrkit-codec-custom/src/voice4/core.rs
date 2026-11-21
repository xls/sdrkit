//! The voice codec 4 core decoder: LSF dequantisation, the adaptive and fixed codebooks, the gains, synthesis, the core post-filter and
//! concealment. The encoder shares the state and the subframe steps.

use super::layout::Layout;
use super::lpc::{fir_step, interp, lsf_to_lpc, stabilise, synth_step, weight, Lpc, ORDER};
use super::math::{db, pow10, Lcg};
use super::tables::{HALF_TAPS, KLT_BASIS, KLT_SD, LSF_MEAN};

/// Past excitation samples kept between frames.
pub(crate) const HIST: usize = 168;
/// Residual history of the post-filter.
const PF_HIST: usize = 149;
/// The 40 ms mid-frame interpolation weights.
pub(crate) const INTERP_W: [f64; 4] = [0.25, 0.45, 0.62, 0.80];
/// Gain prediction taps on the last four corrections, newest first.
const GAIN_TAPS: [f64; 4] = [0.68, 0.58, 0.34, 0.19];
const GP3: [f64; 8] = [0.0, 0.22, 0.42, 0.58, 0.72, 0.84, 0.96, 1.10];

/// The Lloyd-Max levels for `bits` bits.
pub(crate) fn lloyd(bits: u32) -> &'static [f64] {
    use super::tables::*;
    match bits {
        1 => &LLOYD_1,
        2 => &LLOYD_2,
        3 => &LLOYD_3,
        4 => &LLOYD_4,
        5 => &LLOYD_5,
        _ => &LLOYD_6,
    }
}

/// The pitch-gain level of an index.
pub(crate) fn gp_level(bits: u32, idx: u32) -> f64 {
    if bits == 3 {
        GP3[(idx & 7) as usize]
    } else {
        1.2 * idx as f64 / 15.0
    }
}

/// The code-gain correction level of an index, in dB.
pub(crate) fn gc_level(bits: u32, idx: u32) -> f64 {
    let n = (1u32 << bits) as f64;
    let step = match bits {
        3 => 5.0,
        4 => 2.8,
        _ => 1.6,
    };
    (idx as f64 - (n - 1.0) / 2.0) * step
}

/// The excitation guard of section 3.5.8: values not inside (-1e5, 1e5) become the limit, and not-a-number becomes 1e5.
#[inline]
pub(crate) fn guard(v: f32) -> f32 {
    if !(v < 1e5) {
        1e5
    } else if !(v > -1e5) {
        -1e5
    } else {
        v
    }
}

/// Dequantises the LSF indices against `prev` (section 3.5.2).
pub(crate) fn lsf_dequant(
    lay: &Layout,
    idx: &[u32; 10],
    prev: &[f64; ORDER],
    stab: bool,
) -> [f64; ORDER] {
    let t = lay.table;
    let mut q = [0f64; ORDER];
    for i in 0..ORDER {
        q[i] = LSF_MEAN[i] + lay.rho * (prev[i] - LSF_MEAN[i]);
    }
    for k in 0..ORDER {
        let b = lay.lsf_bits[k];
        if b == 0 {
            continue;
        }
        let lv = lloyd(b);
        let i = (idx[k] as usize).min(lv.len() - 1);
        let c = lv[i] * KLT_SD[t][k];
        for i in 0..ORDER {
            q[i] += c * KLT_BASIS[t][k][i];
        }
    }
    if stab {
        stabilise(&mut q, 0.025);
    }
    q
}

/// The LSF vector of subframe `s` (section 3.5.3 step 2).
pub(crate) fn subframe_lsf(
    lay: &Layout,
    s: usize,
    prev: &[f64; ORDER],
    mid: &[f64; ORDER],
    cur: &[f64; ORDER],
) -> [f64; ORDER] {
    if lay.halves == 1 {
        interp(prev, cur, (s + 1) as f64 / lay.s as f64)
    } else {
        let sh = lay.s / 2;
        if s < sh {
            interp(prev, mid, (s + 1) as f64 / sh as f64)
        } else {
            interp(mid, cur, (s - sh + 1) as f64 / sh as f64)
        }
    }
}

/// Splits a lag in units into the integer lag and the half-sample flag.
#[inline]
pub(crate) fn split_lag(u: i32, res: u32) -> (usize, bool) {
    if res == 2 {
        ((u / 2) as usize, u & 1 == 1)
    } else {
        (u as usize, false)
    }
}

/// Writes the adaptive vector for lag `u` into `exc[pos .. pos + l]` (section 3.5.5).
pub(crate) fn adaptive(exc: &mut [f32], pos: usize, l: usize, u: i32, res: u32) {
    let (ti, frac) = split_lag(u, res);
    if !frac {
        for n in 0..l {
            exc[pos + n] = exc[pos + n - ti];
        }
    } else {
        for n in 0..l {
            let base = pos + n - ti - 8;
            let mut acc = 0f32;
            for j in 0..16 {
                acc += HALF_TAPS[j] * exc[base + j];
            }
            exc[pos + n] = acc;
        }
    }
}

/// Pitch sharpening of the pulse vector (section 3.5.6 step 4).
pub(crate) fn sharpen(y: &[i32], prev_gp: f64, ti: usize, c: &mut [f32]) {
    let l = y.len();
    let beta = prev_gp.clamp(0.0, 0.8) as f32;
    for n in 0..l {
        c[n] = y[n] as f32;
    }
    if ti < l {
        for n in ti..l {
            c[n] += beta * c[n - ti];
        }
    }
}

/// Random pulses for a lost frame or a damaged pulse field (section 3.5.6 step 3).
pub(crate) fn random_pulses(rng: &mut Lcg, k: u32, y: &mut [i32]) {
    let l = y.len() as u32;
    y.fill(0);
    for _ in 0..k.max(1) {
        let p = ((rng.next() >> 8) % l) as usize;
        if rng.next() & 0x10000 != 0 {
            y[p] += 1;
        } else {
            y[p] -= 1;
        }
    }
}

/// The core post-filter (section 3.5.9).
#[derive(Clone)]
pub(crate) struct PostFilter {
    res: [f32; PF_HIST + 80],
    fir: [f32; ORDER],
    iir: [f32; ORDER],
    tilt: f32,
    agc: f64,
}

impl PostFilter {
    pub(crate) fn new() -> Self {
        Self {
            res: [0.0; PF_HIST + 80],
            fir: [0.0; ORDER],
            iir: [0.0; ORDER],
            tilt: 0.0,
            agc: 1.0,
        }
    }

    /// Filters one subframe `x` into `out`. `gn`, `gd` and `strength` set the filter. `tilt_factor` scales the tilt.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run(
        &mut self,
        aq: &Lpc,
        ti: usize,
        gp: f64,
        gn: f64,
        gd: f64,
        x: &[f32],
        out: &mut [f32],
        pitch: Option<PitchRule>,
        tilt_factor: Option<f64>,
    ) {
        let l = x.len();
        let an = weight(aq, gn);
        let ad = weight(aq, gd);
        for n in 0..l {
            self.res[PF_HIST + n] = fir_step(&an, &mut self.fir, x[n]);
        }
        let r = &self.res;
        let mut gl = 0f64;
        let mut bt = ti;
        if let Some(rule) = pitch {
            if rule.allowed {
                let mut bc = -1f64;
                let mut be = 1f64;
                let mut rr = 0f64;
                for n in 0..l {
                    let v = r[PF_HIST + n] as f64;
                    rr += v * v;
                }
                rr += 1e-3;
                let lo = ti.saturating_sub(1);
                for t in lo..=ti + 1 {
                    if !(20..=147).contains(&t) {
                        continue;
                    }
                    let mut c = 0f64;
                    let mut e = 0f64;
                    for n in 0..l {
                        let a = r[PF_HIST + n] as f64;
                        let b = r[PF_HIST + n - t] as f64;
                        c += a * b;
                        e += b * b;
                    }
                    let better = if rule.enhancer {
                        c > 0.0 && c * c * be > bc * bc * (e + 1e-3)
                    } else {
                        c * c * be > bc * bc * e && c > 0.0
                    };
                    if better {
                        bc = c;
                        be = e + 1e-3;
                        bt = t;
                    }
                }
                if bc > 0.0 {
                    let rho = bc / (be * rr).sqrt();
                    if rule.enhancer {
                        if rho > 0.5 {
                            gl = rule.strength
                                * (bc / be).min(1.0)
                                * ((rho - 0.5) / 0.3).clamp(0.0, 1.0);
                            if rule.lost {
                                gl *= 0.5;
                            }
                        }
                    } else if rho > 0.45 && gp > 0.25 {
                        gl = rule.strength * (bc / be).min(1.0);
                    }
                }
            }
        }
        for n in 0..l {
            let v = ((r[PF_HIST + n] as f64 + gl * r[PF_HIST + n - bt] as f64) / (1.0 + gl)) as f32;
            out[n] = synth_step(&ad, &mut self.iir, v);
        }
        if let Some(tf) = tilt_factor {
            let mut h = [0f32; 22];
            h[..=ORDER].copy_from_slice(&an);
            let mut m = [0f32; ORDER];
            let mut ho = [0f32; 22];
            for i in 0..22 {
                ho[i] = synth_step(&ad, &mut m, h[i]);
            }
            let mut h0 = 0f64;
            let mut h1 = 0f64;
            for i in 0..22 {
                h0 += ho[i] as f64 * ho[i] as f64;
            }
            for i in 0..21 {
                h1 += ho[i] as f64 * ho[i + 1] as f64;
            }
            let mu = if h1 > 0.0 { tf * h1 / (h0 + 1e-9) } else { 0.0 };
            let muf = mu as f32;
            for v in out.iter_mut().take(l) {
                let y = *v;
                *v = y - muf * self.tilt;
                self.tilt = y;
            }
        } else if l > 0 {
            self.tilt = out[l - 1];
        }
        let mut ein = 0f64;
        let mut eout = 0f64;
        for n in 0..l {
            ein += x[n] as f64 * x[n] as f64;
            eout += out[n] as f64 * out[n] as f64;
        }
        let g = if eout > 1e-3 {
            (ein / eout).sqrt()
        } else {
            0.0
        };
        for v in out.iter_mut().take(l) {
            self.agc = 0.9 * self.agc + 0.1 * g;
            *v = (*v as f64 * self.agc) as f32;
        }
        self.res.copy_within(l..l + PF_HIST, 0);
    }

    /// Clears the residual history (an enhancer restart).
    pub(crate) fn clear_history(&mut self) {
        self.res = [0.0; PF_HIST + 80];
    }
}

/// How the post-filter searches the pitch.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PitchRule {
    pub allowed: bool,
    pub strength: f64,
    pub enhancer: bool,
    pub lost: bool,
}

/// The decoded parameters of one subframe, for the enhancer.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SubInfo {
    pub aq: Lpc,
    pub ti: usize,
    pub gp: f64,
    pub gc: f64,
    pub ep: f64,
    pub random: bool,
}

impl Default for SubInfo {
    fn default() -> Self {
        Self {
            aq: [0.0; ORDER + 1],
            ti: 40,
            gp: 0.0,
            gc: 0.0,
            ep: 0.0,
            random: false,
        }
    }
}

/// The source of the gains of one subframe.
pub(crate) enum Gains {
    /// A good frame: the pitch-gain and code-gain indices.
    Good { gp: u32, gc: u32 },
    /// A lost frame with attenuation `att`.
    Lost { att: f64 },
}

/// The state of the core decoder (section 3.5.1).
#[derive(Clone)]
pub(crate) struct CoreDecoder {
    pub prev_lsf: [f64; ORDER],
    pub exc: [f32; HIST + 320],
    pub syn: [f32; ORDER],
    pub hist: [f64; 4],
    pub prev_gp: f64,
    pub prev_lag: i32,
    pub last_gp: f64,
    pub lost_run: u32,
    pub mute: f64,
    pub rng: Lcg,
    pub pf: PostFilter,
    /// The decoded parameters of each subframe of the last frame, for the enhancer.
    pub sub: [SubInfo; 8],
    /// The decoded excitation of the last frame (Nc samples), for the enhancer.
    pub frame_exc: [f32; 320],
}

impl CoreDecoder {
    pub(crate) fn new(lay: &Layout) -> Self {
        Self {
            prev_lsf: LSF_MEAN,
            exc: [0.0; HIST + 320],
            syn: [0.0; ORDER],
            hist: [0.0; 4],
            prev_gp: 0.0,
            prev_lag: 40 * lay.res as i32,
            last_gp: 0.0,
            lost_run: 0,
            mute: 1.0,
            rng: Lcg(0x4B56_3444),
            pf: PostFilter::new(),
            sub: [SubInfo::default(); 8],
            frame_exc: [0.0; 320],
        }
    }

    /// The core restart of section 3.7.2.
    pub(crate) fn restart(&mut self, lsf: &[f64; ORDER]) {
        self.exc = [0.0; HIST + 320];
        self.syn = [0.0; ORDER];
        self.hist = [0.0; 4];
        self.prev_gp = 0.0;
        self.prev_lsf = *lsf;
        self.lost_run = 0;
        self.mute = 1.0;
    }

    fn pred(&self) -> f64 {
        let mut p = 33.0;
        for i in 0..4 {
            p += GAIN_TAPS[i] * self.hist[i];
        }
        p
    }

    fn push_corr(&mut self, corr: f64) {
        self.hist = [corr, self.hist[0], self.hist[1], self.hist[2]];
    }

    /// Decodes one subframe into `out` (the synthesis, before any post-filter). Returns the subframe info.
    #[allow(clippy::too_many_arguments)]
    fn subframe(
        &mut self,
        lay: &Layout,
        s: usize,
        aq: &Lpc,
        u: i32,
        y: &[i32],
        random: bool,
        gains: Gains,
        out: &mut [f32],
    ) {
        let l = lay.l;
        let pos = HIST + s * l;
        adaptive(&mut self.exc, pos, l, u, lay.res);
        let (ti, _) = split_lag(u, lay.res);
        let mut c = [0f32; 80];
        sharpen(y, self.prev_gp, ti, &mut c[..l]);
        let e: i64 = y.iter().map(|&v| (v as i64) * (v as i64)).sum();
        let ec = if e > 0 { db(e as f64 / l as f64) } else { 0.0 };
        let pred = self.pred();
        let (gp, corr, gc) = match gains {
            Gains::Good { gp: gi, gc: ci } => {
                let mut gp = gp_level(lay.gp_bits, gi);
                let corr = gc_level(lay.gc_bits, ci);
                if gp > 1.0 && self.prev_gp > 1.0 {
                    gp = 1.0;
                }
                if self.lost_run > 0 && gp > 1.0 {
                    gp = 1.0;
                }
                let gc = if e > 0 {
                    pow10((pred + corr - ec) / 20.0)
                } else {
                    0.0
                };
                (gp, corr, gc)
            }
            Gains::Lost { att } => {
                let gp = (self.last_gp * att).min(0.95);
                let m = (self.hist[0] + self.hist[1] + self.hist[2] + self.hist[3]) / 4.0;
                let corr = m.min(0.0) - 2.0 * self.lost_run as f64;
                let gc = if e > 0 {
                    pow10((pred + corr - ec) / 20.0) * att
                } else {
                    0.0
                };
                (gp, corr, gc)
            }
        };
        self.push_corr(corr);
        self.prev_gp = gp;
        self.prev_lag = u;
        self.last_gp = gp;
        let gpf = gp as f32;
        let gcf = gc as f32;
        let mut ep = 0f64;
        for n in 0..l {
            let v = guard(gpf * self.exc[pos + n] + gcf * c[n]);
            self.exc[pos + n] = v;
            ep += c[n] as f64 * c[n] as f64;
        }
        for n in 0..l {
            out[n] = synth_step(aq, &mut self.syn, self.exc[pos + n]);
        }
        for m in self.syn.iter_mut() {
            *m = guard(*m);
        }
        self.sub[s] = SubInfo {
            aq: *aq,
            ti,
            gp,
            gc,
            ep: ep / l as f64,
            random,
        };
    }

    /// Decodes a good frame into `out` (Nc core samples). `postfilter` runs the core post-filter.
    pub(crate) fn decode(
        &mut self,
        lay: &Layout,
        p: &super::frame::Params,
        postfilter: bool,
        out: &mut [f32],
    ) {
        let cur = lsf_dequant(lay, &p.lsf, &self.prev_lsf, true);
        let mid = if lay.halves == 2 {
            interp(&self.prev_lsf, &cur, INTERP_W[(p.interp & 3) as usize])
        } else {
            cur
        };
        let low = 20 * lay.res as i32;
        let high = if lay.res == 2 { 295 } else { 147 };
        let per_half = lay.s / lay.halves;
        let half_range = 1i32 << (lay.lagd_bits - 1);
        for s in 0..lay.s {
            let lsf = subframe_lsf(lay, s, &self.prev_lsf, &mid, &cur);
            let aq = lsf_to_lpc(&lsf);
            let u = if s % per_half == 0 {
                p.lag[s] as i32 + low
            } else {
                self.prev_lag + p.lag[s] as i32 - half_range
            }
            .clamp(low, high);
            let l = lay.l;
            let mut y = [0i32; 80];
            let random = p.pulse_bad[s];
            if random {
                random_pulses(&mut self.rng, lay.pulses[s], &mut y[..l]);
            } else if lay.pulses[s] > 0 {
                super::pvq::decode(p.pulse[s], lay.pulses[s] as usize, &mut y[..l]);
            }
            let gains = Gains::Good {
                gp: p.gp[s],
                gc: p.gc[s],
            };
            self.finish_subframe(
                lay,
                s,
                &aq,
                u,
                &y[..l],
                random,
                gains,
                postfilter,
                false,
                out,
            );
        }
        self.end_frame(lay, &cur);
        self.lost_run = 0;
        self.mute = 1.0;
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_subframe(
        &mut self,
        lay: &Layout,
        s: usize,
        aq: &Lpc,
        u: i32,
        y: &[i32],
        random: bool,
        gains: Gains,
        postfilter: bool,
        _lost: bool,
        out: &mut [f32],
    ) {
        let l = lay.l;
        let mut syn = [0f32; 80];
        self.subframe(lay, s, aq, u, y, random, gains, &mut syn[..l]);
        let o = &mut out[s * l..(s + 1) * l];
        if postfilter {
            let t = &lay.tier;
            let info = self.sub[s];
            let rule = PitchRule {
                allowed: true,
                strength: t.pitch_strength,
                enhancer: false,
                lost: false,
            };
            self.pf.run(
                aq,
                info.ti,
                info.gp,
                t.gn,
                t.gd,
                &syn[..l],
                o,
                Some(rule),
                Some(0.8),
            );
        } else {
            o.copy_from_slice(&syn[..l]);
        }
    }

    fn end_frame(&mut self, lay: &Layout, cur: &[f64; ORDER]) {
        let nc = lay.nc;
        self.frame_exc[..nc].copy_from_slice(&self.exc[HIST..HIST + nc]);
        self.exc.copy_within(nc..nc + HIST, 0);
        self.prev_lsf = *cur;
    }

    /// Conceals a lost frame into `out` (section 3.5.10).
    pub(crate) fn conceal(&mut self, lay: &Layout, postfilter: bool, out: &mut [f32]) {
        self.lost_run += 1;
        let lost_ms = self.lost_run * lay.frame_ms;
        let mut q = [0f64; ORDER];
        for i in 0..ORDER {
            q[i] = 0.85 * self.prev_lsf[i] + 0.15 * LSF_MEAN[i];
        }
        stabilise(&mut q, 0.025);
        let mid = if lay.halves == 2 {
            interp(&self.prev_lsf, &q, 0.5)
        } else {
            q
        };
        let att = if lost_ms <= 20 {
            0.98
        } else if lost_ms <= 40 {
            0.9
        } else if lost_ms <= 60 {
            0.75
        } else if lost_ms <= 100 {
            0.5
        } else {
            0.25
        };
        for s in 0..lay.s {
            let lsf = subframe_lsf(lay, s, &self.prev_lsf, &mid, &q);
            let aq = lsf_to_lpc(&lsf);
            let u = self.prev_lag;
            let l = lay.l;
            let mut y = [0i32; 80];
            random_pulses(&mut self.rng, lay.pulses[s], &mut y[..l]);
            self.finish_subframe(
                lay,
                s,
                &aq,
                u,
                &y[..l],
                true,
                Gains::Lost { att },
                postfilter,
                true,
                out,
            );
        }
        self.end_frame(lay, &q);
        if lost_ms > 120 {
            for v in out[..lay.nc].iter_mut() {
                self.mute *= 0.995;
                *v = (*v as f64 * self.mute) as f32;
            }
        }
    }
}
