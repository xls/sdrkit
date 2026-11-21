//! The NB-IoT search of one capture: candidates, NPSS detection at 240 ksps, alignment at 1.92 Msps,
//! NSSS, NPBCH and MIB-NB, SIB1-NB.

use super::msg::{MibNb, OperationMode, Sib1Nb};
use super::phy::*;
use crate::decoder::Capture;
use crate::dsp::{resample_to, ResamplerCache};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// The NPSS detection rate.
const R240: f64 = 240e3;

/// A cell found.
#[derive(Clone, Debug, PartialEq)]
pub struct NbFound {
    /// The candidate's offset from 0 Hz.
    pub candidate_hz: f64,
    /// The measured carrier offset from the candidate, Hz.
    pub cfo_hz: f64,
    /// PCI.
    pub pci: u16,
    /// NPSS score.
    pub npss_score: f64,
    /// Clock error, ppm.
    pub clock_ppm: f64,
    /// MIB-NB, its port count and the SFN of frame 0.
    pub mib: Option<(MibNb, usize, u32)>,
    /// SIB1-NB.
    pub sib1: Option<Sib1Nb>,
    /// SIB1-NB tries that failed.
    pub sib1_failed: u32,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// SNR, dB.
    pub snr_db: f64,
}

/// The results of a capture.
#[derive(Clone, Debug, Default)]
pub struct NbSearchOut {
    /// Cells.
    pub cells: Vec<NbFound>,
}

/// The NB-IoT searcher.
pub struct NbSearcher {
    cache: ResamplerCache,
    tbcc: crate::lte::phy::Tbcc,
    ofdm: Ofdm,
    /// Detection threshold.
    pub min_npss: f64,
    /// Read SIB1-NB.
    pub read_sib1: bool,
    template: [Complex32; 16],
    offsets: [usize; 11],
}

impl Default for NbSearcher {
    fn default() -> Self {
        let zc = npss_zc();
        let mut t = [Complex32::new(0.0, 0.0); 16];
        for (m, v) in t.iter_mut().enumerate() {
            for (k, z) in zc.iter().enumerate() {
                let a = TAU * (k as f64 - 5.5) * m as f64 / 16.0;
                *v += z * Complex32::new(a.cos() as f32, a.sin() as f32);
            }
        }
        let e: f32 = t.iter().map(|v| v.norm_sqr()).sum::<f32>().sqrt();
        for v in t.iter_mut() {
            *v /= e;
        }
        let mut offsets = [0usize; 11];
        for (i, o) in offsets.iter_mut().enumerate() {
            *o = (useful_start(i + 3) as f64 / 8.0).round() as usize;
        }
        NbSearcher {
            cache: ResamplerCache::default(),
            tbcc: crate::lte::phy::Tbcc::new(),
            ofdm: Ofdm::default(),
            min_npss: 3.6,
            read_sib1: true,
            template: t,
            offsets,
        }
    }
}

/// An NPSS detection: subframe 5 start (mod 2400 at 240 ksps), carrier offset, score.
#[derive(Clone, Copy, Debug)]
pub struct Npss {
    /// The position in the fold.
    pub pos: usize,
    /// Carrier offset, Hz.
    pub cfo: f64,
    /// Score.
    pub score: f64,
}

impl NbSearcher {
    /// NPSS detection on `x` at 240 ksps over +-`range` Hz.
    pub fn npss(&self, x: &[Complex32], range: f64) -> Option<Npss> {
        let span = 2400usize;
        let last = self.offsets[self.offsets.len() - 1] + 16;
        if x.len() < span + last {
            return None;
        }
        let h = (range / 2500.0).ceil() as i64;
        let n = x.len() - last;
        // Power of the 16-sample windows (prefix sums).
        let mut pw = vec![0f64; x.len() + 1];
        for (i, v) in x.iter().enumerate() {
            pw[i + 1] = pw[i] + v.norm_sqr() as f64;
        }
        let mut z = vec![Complex32::new(0.0, 0.0); x.len() - 15];
        let mut results: Vec<(f64, usize, f64)> = Vec::new();
        for hi in -h..=h {
            let f = hi as f64 * 2500.0;
            let mut t = self.template;
            for (m, v) in t.iter_mut().enumerate() {
                let a = TAU * f * m as f64 / R240;
                *v *= Complex32::new(a.cos() as f32, a.sin() as f32);
            }
            for (i, zi) in z.iter_mut().enumerate() {
                let mut acc = Complex32::new(0.0, 0.0);
                for m in 0..16 {
                    acc += x[i + m] * t[m].conj();
                }
                *zi = acc;
            }
            let mut fold = vec![0f64; span];
            for i in 0..n {
                let mut s = Complex32::new(0.0, 0.0);
                for l in 0..10 {
                    let c = NPSS_COVER[l] * NPSS_COVER[l + 1];
                    s += z[i + self.offsets[l]] * z[i + self.offsets[l + 1]].conj() * c;
                }
                let mut p = 0f64;
                for &o in &self.offsets {
                    p += pw[i + o + 16] - pw[i + o];
                }
                fold[i % span] += s.norm() as f64 / (p + 1e-12);
            }
            let (pk, &pv) = fold
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .expect("the fold has one bin per sample of the span");
            let mut sum = 0f64;
            let mut cnt = 0;
            for (i, &v) in fold.iter().enumerate() {
                let d = (i as i64 - pk as i64).rem_euclid(span as i64);
                let d = d.min(span as i64 - d);
                if d > 10 {
                    sum += v;
                    cnt += 1;
                }
            }
            results.push((pv / (sum / cnt.max(1) as f64).max(1e-30), pk, f));
        }
        let bi = (0..results.len()).max_by(|&a, &b| results[a].0.total_cmp(&results[b].0))?;
        let (score, pos, f) = results[bi];
        let mut cfo = f;
        if bi > 0 && bi + 1 < results.len() {
            let (a, b, c) = (results[bi - 1].0, score, results[bi + 1].0);
            let den = a - 2.0 * b + c;
            if den.abs() > 1e-12 {
                cfo += (0.5 * (a - c) / den).clamp(-0.5, 0.5) * 2500.0;
            }
        }
        (score >= self.min_npss).then_some(Npss { pos, cfo, score })
    }

    /// A quick probe: NPSS score at `offset` on the first 80 ms.
    pub fn probe(&mut self, cap: &Capture, offset: f64) -> Option<Npss> {
        let n = ((0.08 * R240) as usize).max((0.05 * R240) as usize);
        let mut y = Vec::new();
        resample_to(
            &mut self.cache,
            &cap.samples,
            cap.rate,
            offset,
            R240,
            210e3,
            0.0,
            0.0,
            n,
            &mut y,
        );
        self.npss(&y, 22.5e3)
    }

    fn npss_metric(&mut self, y: &[Complex32], sf_start: i64) -> f64 {
        let zc = npss_zc();
        let mut h = [Complex32::new(0.0, 0.0); 11];
        for (i, hv) in h.iter_mut().enumerate() {
            let l = i + 3;
            let v = self.ofdm.demod(y, sf_start + useful_start(l) as i64);
            for k in 0..11 {
                *hv += v[k] * (zc[k] * NPSS_COVER[i]).conj();
            }
        }
        let mut s = Complex32::new(0.0, 0.0);
        for i in 0..10 {
            s += h[i + 1] * h[i].conj();
        }
        s.norm() as f64
    }

    /// Searches one candidate.
    pub fn search_candidate(&mut self, cap: &Capture, cand: f64) -> Option<NbFound> {
        let fs = cap.rate;
        let n240 = (cap.samples.len() as f64 * R240 / fs) as usize;
        let mut x = Vec::new();
        resample_to(
            &mut self.cache,
            &cap.samples,
            fs,
            cand,
            R240,
            210e3,
            0.0,
            0.0,
            n240,
            &mut x,
        );
        let det = self.npss(&x, 22.5e3)?;
        // 1.92 Msps, coarse offset out.
        let f0 = cand + det.cfo;
        let step = fs / RATE;
        let n = (cap.samples.len() as f64 / step) as usize;
        let mut y = Vec::new();
        resample_to(
            &mut self.cache,
            &cap.samples,
            fs,
            f0,
            RATE,
            200e3,
            0.0,
            0.0,
            n,
            &mut y,
        );
        let s0 = ((det.pos as i64 * 8 - 9600).rem_euclid(FRAME as i64)) as f64;
        // Timing of the NPSS per window of 8 frames.
        let frames = ((y.len() as f64 - s0) / FRAME as f64).floor() as usize;
        if frames < 2 {
            return None;
        }
        let mut pts: Vec<(f64, f64)> = Vec::new();
        let mut w0 = 0;
        while w0 < frames {
            let w1 = (w0 + 8).min(frames);
            let mut best = (0f64, 0i64);
            let mut vals = [0f64; 33];
            for (di, d) in (-16i64..=16).enumerate() {
                let mut m = 0.0;
                for k in w0..w1 {
                    let sf = s0 as i64 + (k * FRAME) as i64 + 5 * SF as i64 + d;
                    m += self.npss_metric(&y, sf);
                }
                vals[di] = m;
                if m > best.0 {
                    best = (m, d);
                }
            }
            let di = (best.1 + 16) as usize;
            let mut frac = 0.0;
            if di > 0 && di < 32 {
                let (a, b, c) = (vals[di - 1], vals[di], vals[di + 1]);
                let den = a - 2.0 * b + c;
                if den.abs() > 1e-12 {
                    frac = (0.5 * (a - c) / den).clamp(-0.5, 0.5);
                }
            }
            let kc = (w0 + w1 - 1) as f64 / 2.0;
            pts.push((kc, s0 + kc * FRAME as f64 + best.1 as f64 + frac));
            w0 = w1;
        }
        let (a, b) = if pts.len() >= 2 {
            let m = pts.len() as f64;
            let sx: f64 = pts.iter().map(|p| p.0).sum();
            let sy: f64 = pts.iter().map(|p| p.1).sum();
            let sxx: f64 = pts.iter().map(|p| p.0 * p.0).sum();
            let sxy: f64 = pts.iter().map(|p| p.0 * p.1).sum();
            let b = (m * sxy - sx * sy) / (m * sxx - sx * sx);
            let b = b.clamp(FRAME as f64 * (1.0 - 1e-4), FRAME as f64 * (1.0 + 1e-4));
            ((sy - b * sx) / m, b)
        } else {
            (pts[0].1 - pts[0].0 * FRAME as f64, FRAME as f64)
        };
        let clock_ppm = (b / FRAME as f64 - 1.0) * 1e6;
        // Aligned frames: frame k at 19200 k + 16.
        let step2 = step * b / FRAME as f64;
        let start = (a - 16.0) * step;
        let nfr = ((cap.samples.len() as f64 - start) / step2 / FRAME as f64)
            .floor()
            .max(0.0) as usize;
        if nfr < 2 {
            return None;
        }
        let mut z = Vec::new();
        let cutoff_w = 200e3;
        resample_to(
            &mut self.cache,
            &cap.samples,
            fs,
            f0,
            RATE,
            cutoff_w,
            start,
            (b / FRAME as f64 - 1.0) * 1e6,
            nfr * FRAME + 32,
            &mut z,
        );
        // Residual carrier offset from the NPSS symbols (lags 1, 5, 10).
        let zc = npss_zc();
        let mut lag = [Complex32::new(0.0, 0.0); 3];
        for k in 0..nfr {
            let base = (16 + k * FRAME + 5 * SF) as i64;
            let mut h = [Complex32::new(0.0, 0.0); 11];
            for (i, hv) in h.iter_mut().enumerate() {
                let v = self.ofdm.demod(&z, base + useful_start(i + 3) as i64);
                for kk in 0..11 {
                    *hv += v[kk] * (zc[kk] * NPSS_COVER[i]).conj();
                }
            }
            for (j, &l) in [1usize, 5, 10].iter().enumerate() {
                for i in 0..11 - l {
                    lag[j] += h[i + l] * h[i].conj();
                }
            }
        }
        let dt = |l: usize| (useful_start(3 + l) - useful_start(3)) as f64 / RATE;
        let f1 = lag[0].arg() as f64 / (TAU * dt(1));
        let refine = |f: f64, l: usize, j: usize| -> f64 {
            let t = dt(l);
            let base = lag[j].arg() as f64 / (TAU * t);
            let period = 1.0 / t;
            let k = ((f - base) / period).round();
            base + k * period
        };
        let f5 = refine(f1, 5, 1);
        let fres = refine(f5, 10, 2);
        let mut osc = Nco::new(RATE, -fres);
        osc.mix_in_place(&mut z);
        let cfo = det.cfo + fres;
        // NSSS: PCI and n_f mod 8 of frame 0.
        let mut ys: Vec<(usize, [Complex32; 132])> = Vec::new();
        for k in 0..nfr {
            let base = (16 + k * FRAME + 9 * SF) as i64;
            let mut v = [Complex32::new(0.0, 0.0); 132];
            for l in 0..11 {
                let s = self.ofdm.demod(&z, base + useful_start(l + 3) as i64);
                v[l * 12..l * 12 + 12].copy_from_slice(&s);
            }
            ys.push((k, v));
        }
        let mut best: (f64, u16, u32) = (0.0, 0, 0);
        let mut all_scores: Vec<f64> = Vec::with_capacity(504);
        let mut best_by_h: Vec<(f64, f64, u16)> = Vec::new();
        for h in 0..8u32 {
            let mut scores = vec![0f64; 504];
            for (pci, sc) in scores.iter_mut().enumerate() {
                let mut tot = 0f64;
                for (k, v) in &ys {
                    let nf = h + *k as u32;
                    if !nf.is_multiple_of(2) {
                        continue;
                    }
                    let d = nsss(pci as u16, nf);
                    let mut c = Complex32::new(0.0, 0.0);
                    for i in 0..132 {
                        c += v[i] * d[i].conj();
                    }
                    tot += c.norm() as f64;
                }
                *sc = tot;
            }
            let (bp, &bv) = scores
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .expect("the score table has one entry per identity");
            let mean_others = (scores.iter().sum::<f64>() - bv) / 503.0;
            best_by_h.push((bv, bv / mean_others.max(1e-30), bp as u16));
            if bv > best.0 {
                best = (bv, bp as u16, h);
                all_scores = scores;
            }
        }
        let _ = all_scores;
        let hbest = best.2 as usize;
        let nsss_score = best_by_h[hbest].1;
        if nsss_score < 2.6 {
            return None;
        }
        let pci = best.1;
        let nf_mod8 = best.2;
        // NPBCH per frame, both port hypotheses.
        let res = npbch_res(pci);
        let mut level = 0f64;
        let mut noise_sum = 0f64;
        let mut meas = 0usize;
        let mut per_frame: Vec<[Vec<f32>; 2]> = Vec::new();
        for k in 0..nfr {
            let base = (16 + k * FRAME) as i64;
            let (soft, p, nv) = self.subframe_soft(&z, base, pci, &res);
            level += p;
            noise_sum += nv;
            meas += 1;
            per_frame.push(soft);
        }
        let p_nrs = level / meas.max(1) as f64;
        let noise = noise_sum / meas.max(1) as f64;
        let level_dbfs = 10.0 * (12.0 * p_nrs).max(1e-30).log10();
        let snr_db = 10.0 * (p_nrs / noise.max(1e-30)).log10();
        let scr = npbch_scrambling(pci);
        // Groups of 8 frames starting where n_f mod 8 = 0.
        let first = (8 - nf_mod8 as usize) % 8;
        // Without the per-frame rotation: every first segment.
        let mut mib = decode_npbch(&mut self.tbcc, &per_frame, &scr, first, nf_mod8, None);
        // With it (later releases): the rotation needs the SFN, so each SFN of frame 0 that agrees with the NSSS is tried, and the
        // decode must give that SFN back.
        if mib.is_none() {
            let mut rotated = per_frame.clone();
            'sfn: for q in 0..128u32 {
                let sfn0 = 8 * q + nf_mod8;
                for (k, (r, f)) in rotated.iter_mut().zip(&per_frame).enumerate() {
                    let rot = npbch_rotation(pci, (sfn0 + k as u32) % 1024);
                    for p in 0..2 {
                        r[p].copy_from_slice(&f[p]);
                        derotate_soft(&mut r[p], &rot);
                    }
                }
                if let Some(m) =
                    decode_npbch(&mut self.tbcc, &rotated, &scr, first, nf_mod8, Some(sfn0))
                {
                    mib = Some(m);
                    break 'sfn;
                }
            }
        }
        let mut sib1 = None;
        let mut sib1_failed = 0;
        if let (Some((m, ports, sfn0)), true) = (&mib, self.read_sib1) {
            match self.read_sib1_nb(&z, nfr, pci, m, *ports, *sfn0) {
                Some(Ok(s)) => sib1 = Some(s),
                Some(Err(())) => sib1_failed += 1,
                None => {}
            }
        }
        Some(NbFound {
            candidate_hz: cand,
            cfo_hz: cfo,
            pci,
            npss_score: det.score,
            clock_ppm,
            mib,
            sib1,
            sib1_failed,
            level_dbfs,
            snr_db,
        })
    }

    // Channel from the NRS of a subframe: per port, per slot (linear between the slots); noise from the residual.
    fn channel(
        &mut self,
        z: &[Complex32],
        base: i64,
        pci: u16,
        sf: usize,
    ) -> ([[Complex32; 14]; 2], f64, f64, [[Complex32; 12]; 14]) {
        let mut grid = [[Complex32::new(0.0, 0.0); 12]; 14];
        for (l, g) in grid.iter_mut().enumerate() {
            *g = self.ofdm.demod(z, base + useful_start(l) as i64);
        }
        let mut hs = [[Complex32::new(0.0, 0.0); 2]; 2];
        let mut est: Vec<(usize, usize, f64, Complex32)> = Vec::new();
        for port in 0..2 {
            for (slot, ls) in [[5usize, 6], [12, 13]].iter().enumerate() {
                let mut acc = Complex32::new(0.0, 0.0);
                for &l in ls {
                    let ns = 2 * sf + l / 7;
                    for (m, &k) in nrs_subcarriers(pci, port, l).iter().enumerate() {
                        let e = grid[l][k] * nrs_value(pci, ns, l, m).conj();
                        acc += e;
                        est.push((port, slot, l as f64, e));
                    }
                }
                hs[port][slot] = acc / 4.0;
            }
        }
        let mut h = [[Complex32::new(0.0, 0.0); 14]; 2];
        for port in 0..2 {
            for (l, hl) in h[port].iter_mut().enumerate() {
                let t = (l as f32 - 5.5) / 7.0;
                *hl = hs[port][0] + (hs[port][1] - hs[port][0]) * t;
            }
        }
        let mut nv = 0f64;
        for (port, _, l, e) in &est {
            nv += (e - h[*port][*l as usize]).norm_sqr() as f64;
        }
        let noise = nv / est.len() as f64 * 4.0 / 3.0;
        let p0 = (hs[0][0].norm_sqr() + hs[0][1].norm_sqr()) as f64 / 2.0;
        (h, p0, noise, grid)
    }

    fn subframe_soft(
        &mut self,
        z: &[Complex32],
        base: i64,
        pci: u16,
        res: &[(usize, usize)],
    ) -> ([Vec<f32>; 2], f64, f64) {
        let (h, p0, noise, grid) = self.channel(z, base, pci, 0);
        let y: Vec<Complex32> = res.iter().map(|&(k, l)| grid[l][k]).collect();
        let hh: Vec<Vec<Complex32>> = (0..2)
            .map(|p| res.iter().map(|&(_, l)| h[p][l]).collect())
            .collect();
        let mut s1 = Vec::new();
        let mut s2 = Vec::new();
        soft_bits(&y, &hh, 1, noise as f32, &mut s1);
        soft_bits(&y, &hh, 2, noise as f32, &mut s2);
        ([s1, s2], p0, noise)
    }

    #[allow(clippy::too_many_arguments)]
    fn read_sib1_nb(
        &mut self,
        z: &[Complex32],
        nfr: usize,
        pci: u16,
        m: &MibNb,
        ports: usize,
        sfn0: u32,
    ) -> Option<Result<Sib1Nb, ()>> {
        let (reps, tbs) = sib1_schedule(m.scheduling_info_sib1)?;
        let start = sib1_start(reps, pci);
        let period = 256 / reps;
        // CRS hypotheses.
        let hyps: Vec<(usize, Option<(usize, usize)>)> = match m.mode {
            OperationMode::InbandSamePci(_) => vec![(3, Some((pci as usize % 6, ports)))],
            OperationMode::InbandDifferentPci { four_ports, .. } => {
                let p = if four_ports { 4 } else { ports };
                (0..6).map(|s| (3, Some((s, p)))).collect()
            }
            _ => vec![(0, None)],
        };
        // Repetitions inside the capture.
        let mut reps_at: Vec<(usize, u32)> = Vec::new();
        for k in 0..nfr {
            let nf = (sfn0 + k as u32) % 1024;
            let f = nf % 256;
            if f >= start && (f - start).is_multiple_of(period) && k + 15 < nfr {
                reps_at.push((k, nf));
            }
        }
        if reps_at.is_empty() {
            return None;
        }
        for (l0, crs) in hyps {
            let res = npdsch_res(pci, ports, l0, crs);
            let g = 2 * res.len();
            let e = 8 * g;
            let mut acc = vec![0f32; e];
            for &(k, nf) in &reps_at {
                let c = sib1_scrambling(pci, nf, e);
                for j in 0..8 {
                    let base = (16 + (k + 2 * j) * FRAME + 4 * SF) as i64;
                    let (h, _, noise, grid) = self.channel(z, base, pci, 4);
                    let y: Vec<Complex32> = res.iter().map(|&(kk, l)| grid[l][kk]).collect();
                    let hh: Vec<Vec<Complex32>> = (0..2)
                        .map(|p| res.iter().map(|&(_, l)| h[p][l]).collect())
                        .collect();
                    let mut s = Vec::new();
                    soft_bits(&y, &hh, ports, noise as f32, &mut s);
                    for (i, v) in s.iter().enumerate().take(g) {
                        let idx = j * g + i;
                        acc[idx] += if c[idx] == 1 { -v } else { *v };
                    }
                }
            }
            if let Some(tb) = decode_sib1(&mut self.tbcc, &acc, tbs) {
                if let Some(s) = Sib1Nb::parse(&tb) {
                    return Some(Ok(s));
                }
            }
        }
        Some(Err(()))
    }
}

/// The MIB-NB from per-frame NPBCH soft values (per port hypothesis 1, 2): frames summed per 80 ms group from frame `first`
/// (where n_f mod 8 = 0), each run of groups placed from every first segment, descrambled, decoded; accepted when the SFN of
/// frame 0 agrees with the NSSS (and equals `sfn0` when one is given).
fn decode_npbch(
    tbcc: &mut crate::lte::phy::Tbcc,
    per_frame: &[[Vec<f32>; 2]],
    scr: &[u8],
    first: usize,
    nf_mod8: u32,
    sfn0_given: Option<u32>,
) -> Option<(MibNb, usize, u32)> {
    let nfr = per_frame.len();
    for ports in [1usize, 2] {
        let mut groups: Vec<Vec<f32>> = Vec::new();
        let mut g = first;
        while g + 8 <= nfr {
            let mut s = vec![0f32; 200];
            for f in &per_frame[g..g + 8] {
                for (a, b) in s.iter_mut().zip(&f[ports - 1]) {
                    *a += b;
                }
            }
            groups.push(s);
            g += 8;
        }
        let firsts: Vec<usize> = match sfn0_given {
            Some(s0) => vec![(((s0 as usize + first) % 64) / 8) % 8],
            None => (0..8).collect(),
        };
        for i0 in firsts {
            let mut soft = vec![0f32; 1600];
            let mut any = false;
            for (gi, s) in groups.iter().enumerate() {
                let seg = i0 + gi;
                if seg >= 8 {
                    break;
                }
                any = true;
                for j in 0..200 {
                    let c = scr[seg * 200 + j];
                    soft[seg * 200 + j] = if c == 1 { -s[j] } else { s[j] };
                }
            }
            if !any {
                continue;
            }
            if let Some(bits) = decode_mib_nb(tbcc, &soft, ports) {
                let m = MibNb::from_bits(&bits);
                let sfn_run = 64 * m.sfn_msb as u32 + 8 * i0 as u32;
                let sfn0 = (sfn_run + 1024 - first as u32) % 1024;
                if sfn0 % 8 == nf_mod8 && sfn0_given.is_none_or(|g| g == sfn0) {
                    return Some((m, ports, sfn0));
                }
            }
        }
    }
    None
}

/// The candidates of a capture.
pub fn candidates(s: &mut NbSearcher, cap: &Capture) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    let push = |out: &mut Vec<f64>, f: f64| {
        if !out.iter().any(|&x| (x - f).abs() < 50e3) {
            out.push(f);
        }
    };
    if cap.centre_known() {
        let dial = cap.centre_hz + cap.dial_offset_hz;
        push(&mut out, (dial / 100e3).round() * 100e3 - cap.centre_hz);
        let lim = 0.45 * cap.rate - 150e3;
        let lo = ((cap.centre_hz - lim) / 100e3).ceil() as i64;
        let hi = ((cap.centre_hz + lim) / 100e3).floor() as i64;
        let mut probes: Vec<(f64, f64)> = Vec::new();
        for r in lo..=hi {
            let off = r as f64 * 100e3 - cap.centre_hz;
            let saved = s.min_npss;
            s.min_npss = 4.6;
            if let Some(d) = s.probe(cap, off) {
                probes.push((d.score, off));
            }
            s.min_npss = saved;
        }
        probes.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, off) in probes.into_iter().take(6) {
            push(&mut out, off);
        }
    } else {
        push(&mut out, cap.dial_offset_hz);
    }
    out
}

impl NbSearcher {
    /// Searches the whole capture for every cell it holds.
    pub fn search(&mut self, cap: &Capture) -> NbSearchOut {
        let mut out = NbSearchOut::default();
        if cap.rate < 1e6 {
            return out;
        }
        for c in candidates(self, cap) {
            if let Some(f) = self.search_candidate(cap, c) {
                let actual = c + f.cfo_hz;
                if !out
                    .cells
                    .iter()
                    .any(|x| x.pci == f.pci && (x.candidate_hz + x.cfo_hz - actual).abs() < 60e3)
                {
                    out.cells.push(f);
                }
            }
        }
        // A neighbouring raster point sees a strong carrier through its filter's skirt: a result without a MIB next to a stronger
        // one is that carrier's shadow, not a cell.
        let all = out.cells.clone();
        out.cells.retain(|c| {
            c.mib.is_some()
                || !all.iter().any(|o| {
                    o.npss_score > c.npss_score
                        && (o.candidate_hz + o.cfo_hz - c.candidate_hz - c.cfo_hz).abs() < 150e3
                })
        });
        out
    }
}
