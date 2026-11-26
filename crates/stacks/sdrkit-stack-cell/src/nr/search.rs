//! The NR search of one capture: sync raster candidates, PSS search folded over the 20 ms period, SSB
//! timing and carrier offset, N_ID1, the SSB index from the PBCH DMRS, PBCH and MIB, beams.

use super::dlsch::Numerology;
use super::phy::*;
use super::sib1::{Sib1Receiver, Sib1Result, Sib1Step, Sib1Target};
use crate::bands::{gscn_to_hz, nr_band_of, nr_bands};
use crate::decoder::Capture;
use crate::dsp::{resample_to, ResamplerCache};
use sdrkit_algo_dsp::fft::{Fft, FftPlans};
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// A search candidate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cand {
    /// Offset from 0 Hz.
    pub offset_hz: f64,
    /// SSB spacing, kHz.
    pub scs: u32,
    /// Carrier offset searched, +- Hz.
    pub range_hz: f64,
    /// Its GSCN, when the centre is known.
    pub gscn: Option<u32>,
}

/// A beam measured.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beam {
    /// SSB index.
    pub ssb: u8,
    /// RSRP, dBFS.
    pub rsrp_dbfs: f64,
}

/// One NR cell found.
#[derive(Clone, Debug, PartialEq)]
pub struct NrFound {
    /// The candidate.
    pub cand: Cand,
    /// Carrier offset from the candidate, Hz.
    pub cfo_hz: f64,
    /// PCI.
    pub pci: u16,
    /// N_ID1 significance.
    pub significance: f64,
    /// MIB, the DMRS index it decoded with, and whether the Lmax-8 descrambling was the one.
    pub mib: Option<(Mib, u8, bool)>,
    /// The strongest SSB's DMRS index and whether it was sure.
    pub dmrs_index: (u8, bool),
    /// Beams (SSB index, RSRP).
    pub beams: Vec<Beam>,
    /// Lmax of a decoded cell (from the decode, else the band's usual Lmax).
    pub lmax: Option<u8>,
    /// RSRP, dBFS.
    pub rsrp_dbfs: f64,
    /// RSRQ, dB.
    pub rsrq_db: f64,
    /// SINR, dB.
    pub sinr_db: f64,
    /// PSS strength, dB.
    pub pss_db: f64,
    /// PBCH decodes and tries.
    pub pbch_ok: u32,
    /// PBCH tries.
    pub pbch_tried: u32,
    /// The input sample of this SSB's PSS useful part in the first period.
    pub anchor: f64,
    /// The index of the SSB of `anchor`.
    pub anchor_ssb: u8,
    /// The try to receive SIB1 in this capture, when one was made.
    pub sib1: Option<Sib1Result>,
}

/// The results of a capture.
#[derive(Clone, Debug, Default)]
pub struct NrSearchOut {
    /// Cells.
    pub cells: Vec<NrFound>,
    /// Lowest and highest GSCN searched, candidates.
    pub gscn_range: Option<(u32, u32)>,
    /// Candidates searched.
    pub candidates: usize,
    /// The strongest PSS correlation of the capture over its mean, dB, whether over the threshold or not.
    pub best_pss_db: f64,
}

/// The NR searcher.
pub struct NrSearcher {
    cache: ResamplerCache,
    plans: FftPlans,
    /// Cells per candidate.
    pub max_cells: usize,
    /// PSS threshold, dB.
    pub min_pss_db: f64,
    /// The bands taken first where the sync rasters of bands overlap.
    pub band_order: Vec<u16>,
    pbch: PbchDecoder,
    sib1: Sib1Receiver,
    /// Whether the symbols are taken to carry the phase of up-conversion (3GPP TS 38.211 clause 5.4).
    pub symbol_phase: bool,
    /// The cells whose SIB1 was tried: PCI and SS_REF in steps of 30 kHz, the signal time of the try, whether it decoded.
    sib1_tried: Vec<((u16, i64), f64, bool)>,
    /// The strongest PSS correlation over its mean since the search of the capture started, dB.
    best_db: f64,
}

/// Seconds until the SIB1 of a cell is read again after a decode.
pub const SIB1_AGAIN_S: f64 = 30.0;
/// Seconds until the SIB1 of a cell is tried again after a try without a decode.
pub const SIB1_RETRY_S: f64 = 1.0;

impl Default for NrSearcher {
    fn default() -> Self {
        NrSearcher {
            cache: ResamplerCache::default(),
            plans: FftPlans::new(),
            max_cells: 6,
            min_pss_db: 10.0,
            band_order: Vec::new(),
            pbch: PbchDecoder::new(),
            sib1: Sib1Receiver::default(),
            symbol_phase: true,
            sib1_tried: Vec::new(),
            best_db: f64::NEG_INFINITY,
        }
    }
}

/// The candidates of a capture.
pub fn candidates(cap: &Capture, order: &[u16]) -> (Vec<Cand>, Option<(u32, u32)>) {
    let fs = cap.rate;
    let mut v: Vec<Cand> = Vec::new();
    let push = |v: &mut Vec<Cand>, c: Cand| -> bool {
        if fs < 0.97 * 256.0 * c.scs as f64 * 1e3 {
            return false;
        }
        if !v
            .iter()
            .any(|x| x.scs == c.scs && (x.offset_hz - c.offset_hz).abs() < 5e3)
        {
            v.push(c);
            return true;
        }
        false
    };
    let range = |f: f64, scs: u32| (scs as f64 * 500.0).max(45e3f64.min(10e-6 * f));
    let mut lo_hi: Option<(u32, u32)> = None;
    if cap.centre_known() {
        let dial = cap.centre_hz + cap.dial_offset_hz;
        for scs in [30u32, 15] {
            if let Some(g) = crate::bands::hz_to_gscn(dial) {
                if let Some(f) = gscn_to_hz(g) {
                    if (f - dial).abs() <= 30e3 && nr_band_of(g, Some(scs), order).is_some() {
                        push(
                            &mut v,
                            Cand {
                                offset_hz: f - cap.centre_hz,
                                scs,
                                range_hz: range(f, scs),
                                gscn: Some(g),
                            },
                        );
                    }
                }
            }
        }
        for b in nr_bands(order).into_iter().flat_map(|b| b.info.sync.iter()) {
            let scs = b.scs_khz;
            let mut g = b.first;
            while g <= b.last {
                if let Some(f) = gscn_to_hz(g) {
                    let off = f - cap.centre_hz;
                    let half = 120.0 * scs as f64 * 1e3 + 0.2e6;
                    if off.abs() + half <= 0.46 * fs {
                        let added = push(
                            &mut v,
                            Cand {
                                offset_hz: off,
                                scs,
                                range_hz: range(f, scs),
                                gscn: Some(g),
                            },
                        );
                        if added {
                            lo_hi = Some(lo_hi.map_or((g, g), |(a, z)| (a.min(g), z.max(g))));
                        }
                    }
                }
                g += b.step;
            }
        }
        if v.is_empty() {
            for scs in [30u32, 15] {
                push(
                    &mut v,
                    Cand {
                        offset_hz: cap.dial_offset_hz,
                        scs,
                        range_hz: range(dial, scs),
                        gscn: None,
                    },
                );
            }
        }
    } else {
        for scs in [30u32, 15] {
            push(
                &mut v,
                Cand {
                    offset_hz: cap.dial_offset_hz,
                    scs,
                    range_hz: 60e3,
                    gscn: None,
                },
            );
        }
    }
    (v, lo_hi)
}

/// A PSS peak.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Peak {
    /// N_ID2 of the PSS.
    pub nid2: u16,
    /// The position in the folded period, samples.
    pub pos: usize,
    /// The carrier offset hypothesis, Hz.
    pub cfo: f64,
    /// The peak over the mean of the folded energies, dB.
    pub db: f64,
}

/// A peak after the first of one N_ID2 must stand this far over the mean, dB, as in the LTE PSS search.
pub const SECOND_PEAK_DB: f64 = 12.0;

/// The peaks of the folded PSS energies of one N_ID2: `fold[hyp][pos]` for the carrier hypotheses -h to h of `scs` kHz. The
/// first peak must reach `min_db` over the mean; each later peak must reach the larger of `min_db` and [`SECOND_PEAK_DB`]. At
/// most ten peaks; the energies around each peak are cleared.
pub fn pick_peaks(
    fold: &mut [Vec<f32>],
    nid2: u16,
    h: i64,
    scs: u32,
    min_db: f64,
    out: &mut Vec<Peak>,
) {
    let (Some(first), n) = (fold.first(), fold.len()) else {
        return;
    };
    let period = first.len();
    if period == 0 {
        return;
    }
    let mean: f32 = fold.iter().map(|f| f.iter().sum::<f32>()).sum::<f32>() / (n * period) as f32;
    for k in 0..10 {
        let mut best = (0f32, 0usize, 0usize);
        for (hi, f) in fold.iter().enumerate() {
            for (p, &v) in f.iter().enumerate() {
                if v > best.0 {
                    best = (v, hi, p);
                }
            }
        }
        let db = 10.0 * (best.0 / mean.max(1e-30)).max(1e-30).log10() as f64;
        let need = if k == 0 {
            min_db
        } else {
            min_db.max(SECOND_PEAK_DB)
        };
        if db < need {
            break;
        }
        out.push(Peak {
            nid2,
            pos: best.2,
            cfo: (best.1 as i64 - h) as f64 * scs as f64 * 500.0,
            db,
        });
        for f in fold.iter_mut() {
            for d in -128i64..=128 {
                let p = (best.2 as i64 + d).rem_euclid(period as i64) as usize;
                f[p] = 0.0;
            }
        }
    }
}

impl NrSearcher {
    fn pss_template(nid2: u16, n: usize, bins_per_sc: usize) -> Vec<Complex32> {
        // PSS on subcarriers 56..182 (bin k - 120), an n-point IFFT (n = 128: 1 bin per subcarrier).
        let p = pss(nid2);
        let mut f = vec![Complex32::new(0.0, 0.0); n];
        for (i, v) in p.iter().enumerate() {
            let k = 56 + i as i64 - 120;
            f[(k * bins_per_sc as i64).rem_euclid(n as i64) as usize] = Complex32::new(*v, 0.0);
        }
        let mut ifft = Fft::inverse(n);
        ifft.process(&mut f);
        let e: f32 = f.iter().map(|v| v.norm_sqr()).sum::<f32>().sqrt();
        f.iter().map(|v| v / e).collect()
    }

    fn pss_peaks(&mut self, x: &[Complex32], scs: u32, range: f64) -> Vec<Peak> {
        let det_rate = 128.0 * scs as f64 * 1e3;
        let period = (0.02 * det_rate) as usize;
        let h = ((range / (scs as f64 * 500.0)) - 0.3).ceil().max(0.0) as i64;
        let nfft = 4096usize;
        let tl = 128usize;
        let step = nfft - tl;
        let mut fwd = self.plans.forward(nfft);
        let mut inv = self.plans.inverse(nfft);
        let mut peaks = Vec::new();
        // Folded energies: [nid2][hyp][pos].
        let nh = (2 * h + 1) as usize;
        let mut folds = vec![vec![vec![0f32; period]; nh]; 3];
        let mut templ = Vec::new();
        for nid2 in 0..3u16 {
            let t = Self::pss_template(nid2, 128, 1);
            let mut tf = vec![Complex32::new(0.0, 0.0); nfft];
            tf[..tl].copy_from_slice(&t);
            fwd.process(&mut tf);
            templ.push(tf);
        }
        let mut buf = vec![Complex32::new(0.0, 0.0); nfft];
        let mut prod = vec![Complex32::new(0.0, 0.0); nfft];
        let mut s = 0usize;
        while s + tl < x.len() {
            for (i, b) in buf.iter_mut().enumerate() {
                *b = x.get(s + i).copied().unwrap_or_default();
            }
            fwd.process(&mut buf);
            for nid2 in 0..3 {
                for (hi, hv) in (-h..=h).enumerate() {
                    // A carrier offset of hv x SCS/2 moves the template's spectrum by 16 hv bins.
                    let sh = 16 * hv;
                    for (k, p) in prod.iter_mut().enumerate() {
                        let kt = (k as i64 - sh).rem_euclid(nfft as i64) as usize;
                        *p = buf[k] * templ[nid2][kt].conj();
                    }
                    inv.process(&mut prod);
                    for i in 0..step.min(x.len().saturating_sub(s + tl) + 1) {
                        let pos = s + i;
                        folds[nid2][hi][pos % period] += prod[i].norm_sqr();
                    }
                }
            }
            s += step;
        }
        for (nid2, fold) in folds.iter_mut().enumerate() {
            let n = fold.iter().map(Vec::len).sum::<usize>().max(1);
            let mean = fold.iter().map(|f| f.iter().sum::<f32>()).sum::<f32>() / n as f32;
            let top = fold.iter().flat_map(|f| f.iter()).fold(0f32, |a, &b| a.max(b));
            self.best_db = self.best_db.max(10.0 * f64::from((top / mean.max(1e-30)).max(1e-30)).log10());
            pick_peaks(fold, nid2 as u16, h, scs, self.min_pss_db, &mut peaks);
        }
        peaks.sort_by(|a, b| b.db.total_cmp(&a.db));
        peaks.truncate(4 * self.max_cells.max(1));
        peaks
    }

    /// Searches one candidate.
    pub fn search_candidate(&mut self, cap: &Capture, c: &Cand) -> Vec<NrFound> {
        let fs = cap.rate;
        let scs = c.scs;
        let det_rate = 128.0 * scs as f64 * 1e3;
        let n = (cap.samples.len() as f64 * det_rate / fs) as usize;
        let mut x = Vec::new();
        resample_to(
            &mut self.cache,
            &cap.samples,
            fs,
            c.offset_hz,
            det_rate,
            127.0 * scs as f64 * 1e3,
            0.0,
            0.0,
            n,
            &mut x,
        );
        let peaks = self.pss_peaks(&x, scs, c.range_hz);
        let mut out: Vec<NrFound> = Vec::new();
        for pk in peaks {
            if let Some(f) = self.examine(cap, c, &pk) {
                match out.iter_mut().find(|o| o.pci == f.pci) {
                    Some(o) => {
                        for b in &f.beams {
                            if !o.beams.iter().any(|x| x.ssb == b.ssb) {
                                o.beams.push(*b);
                            }
                        }
                        if f.rsrp_dbfs > o.rsrp_dbfs && (f.mib.is_some() || o.mib.is_none()) {
                            let beams = std::mem::take(&mut o.beams);
                            let mib = o.mib.clone();
                            *o = NrFound { beams, ..f.clone() };
                            if o.mib.is_none() {
                                o.mib = mib;
                            }
                        }
                    }
                    None => {
                        if f.mib.is_some() || f.significance >= 6.0 {
                            out.push(f);
                        }
                    }
                }
            }
            if out.len() >= self.max_cells {
                break;
            }
        }
        // The other SSBs of every decoded cell.
        for o in out.iter_mut() {
            self.measure_beams(cap, c, o);
        }
        out
    }

    fn ssb_grid(
        &mut self,
        cap: &Capture,
        centre: f64,
        scs: u32,
        start: f64,
    ) -> [[Complex32; 240]; 4] {
        let rate = 256.0 * scs as f64 * 1e3;
        let step = cap.rate / rate;
        let mut y = Vec::new();
        let len = 4 * 278 + 16;
        resample_to(
            &mut self.cache,
            &cap.samples,
            cap.rate,
            centre,
            rate,
            250.0 * scs as f64 * 1e3,
            start * step,
            0.0,
            len,
            &mut y,
        );
        let mut fft = self.plans.forward(256);
        let mut g = [[Complex32::new(0.0, 0.0); 240]; 4];
        let mut buf = vec![Complex32::new(0.0, 0.0); 256];
        for (l, row) in g.iter_mut().enumerate() {
            // Symbol l's useful part starts (l x 274 + 18) after the SSB start (the SSB never starts a half millisecond, so all
            // four prefixes are 18); the window starts 4 samples early.
            let u = l * 274 + 18;
            for (i, b) in buf.iter_mut().enumerate() {
                *b = y.get(u + i - 4).copied().unwrap_or_default();
            }
            fft.process(&mut buf);
            for (k, r) in row.iter_mut().enumerate() {
                let bin = k as i64 - 120;
                let a = TAU * bin as f64 * 4.0 / 256.0;
                *r = buf[bin.rem_euclid(256) as usize]
                    * Complex32::new(a.cos() as f32, a.sin() as f32)
                    / 256.0;
            }
        }
        g
    }

    fn examine(&mut self, cap: &Capture, c: &Cand, pk: &Peak) -> Option<NrFound> {
        let scs = c.scs;
        let rate = 256.0 * scs as f64 * 1e3;
        let fs = cap.rate;
        let det_rate = 128.0 * scs as f64 * 1e3;
        let period_in = 0.02 * fs;
        // The PSS useful start in input samples, in every period.
        let first = pk.pos as f64 * fs / det_rate;
        let mut places: Vec<f64> = Vec::new();
        let mut p = first;
        while p + 0.002 * fs < cap.samples.len() as f64 {
            places.push(p);
            p += period_in;
        }
        if places.is_empty() {
            return None;
        }
        let mut cfo = pk.cfo;
        let t = Self::pss_template(pk.nid2, 256, 1);
        let step = fs / rate;
        // Fine timing and carrier offset, two passes.
        let mut shift = 0f64;
        for _ in 0..2 {
            let mut best = (0f32, 0i64);
            let mut halves = Complex32::new(0.0, 0.0);
            for d in -6i64..=6 {
                let mut e = 0f32;
                for &pl in &places {
                    let mut y = Vec::new();
                    resample_to(
                        &mut self.cache,
                        &cap.samples,
                        fs,
                        c.offset_hz + cfo,
                        rate,
                        250.0 * scs as f64 * 1e3,
                        pl + (shift + d as f64) * step,
                        0.0,
                        256,
                        &mut y,
                    );
                    let s: Complex32 = y.iter().zip(&t).map(|(a, b)| a * b.conj()).sum();
                    e += s.norm();
                }
                if e > best.0 {
                    best = (e, d);
                }
            }
            shift += best.1 as f64;
            for &pl in &places {
                let mut y = Vec::new();
                resample_to(
                    &mut self.cache,
                    &cap.samples,
                    fs,
                    c.offset_hz + cfo,
                    rate,
                    250.0 * scs as f64 * 1e3,
                    pl + shift * step,
                    0.0,
                    256,
                    &mut y,
                );
                let a: Complex32 = y[..128]
                    .iter()
                    .zip(&t[..128])
                    .map(|(a, b)| a * b.conj())
                    .sum();
                let b: Complex32 = y[128..]
                    .iter()
                    .zip(&t[128..])
                    .map(|(a, b)| a * b.conj())
                    .sum();
                halves += b * a.conj();
            }
            cfo += halves.arg() as f64 / (TAU * 128.0 / rate);
        }
        // SSB grids (the SSB starts one symbol prefix before the PSS useful part).
        let ssb_start = |pl: f64| pl / step + shift - 18.0;
        let mut grids: Vec<[[Complex32; 240]; 4]> = Vec::new();
        for &pl in &places {
            grids.push(self.ssb_grid(cap, c.offset_hz + cfo, scs, ssb_start(pl)));
        }
        let pss_seq = pss(pk.nid2);
        // N_ID1.
        let mut scores = vec![0f64; 336];
        let es: Vec<Vec<Complex32>> = grids
            .iter()
            .map(|g| {
                (0..127)
                    .map(|i| g[2][56 + i] * (g[0][56 + i] * pss_seq[i]).conj())
                    .collect()
            })
            .collect();
        for (nid1, sc) in scores.iter_mut().enumerate() {
            let s = sss(nid1 as u16, pk.nid2);
            for e in &es {
                let v: Complex32 = e.iter().zip(&s).map(|(a, b)| a * *b).sum();
                *sc += v.norm() as f64;
            }
        }
        let (bi, &bv) = scores
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .expect("the score table has one entry per identity");
        let others: Vec<f64> = scores
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != bi)
            .map(|(_, v)| *v)
            .collect();
        let mean = others.iter().sum::<f64>() / others.len() as f64;
        let sd =
            (others.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / others.len() as f64).sqrt();
        let significance = (bv - mean) / sd.max(1e-30);
        if significance < 4.5 {
            return None;
        }
        let nid1 = bi as u16;
        let pci = 3 * nid1 + pk.nid2;
        // The SSB index from the DMRS.
        let dres = dmrs_res(pci);
        let mut dm = [0f64; 8];
        for (ib, d) in dm.iter_mut().enumerate() {
            let r = dmrs(pci, ib as u8);
            for g in &grids {
                for l in 1..4 {
                    let mut s = Complex32::new(0.0, 0.0);
                    for (m, &(k, ll)) in dres.iter().enumerate() {
                        if ll == l {
                            s += g[l][k] * r[m].conj();
                        }
                    }
                    *d += s.norm() as f64;
                }
            }
        }
        let mut order: Vec<usize> = (0..8).collect();
        order.sort_by(|&a, &b| dm[b].total_cmp(&dm[a]));
        let ibar = order[0] as u8;
        let sure = dm[order[0]] > 1.6 * dm[order[1]];
        // The phase that up-conversion gives every symbol (38.211 5.4), known with the radio frequency: taken out, so that
        // the phase between symbols is the carrier offset alone. The place of the block in its subframe follows from its index.
        let nominal = (self.symbol_phase && cap.centre_known())
            .then(|| c.gscn.and_then(gscn_to_hz).unwrap_or(cap.centre_hz + c.offset_hz));
        let turns: [Complex32; 4] = std::array::from_fn(|j| match nominal {
            Some(f) => Numerology { scs_khz: scs, fft: 256 }
                .carrier_phase(f, self.pattern_of(c).first_symbol(ibar as usize & 7) + j)
                .conj(),
            None => Complex32::new(1.0, 0.0),
        });
        let turn = |g: &mut [[Complex32; 240]; 4]| {
            for (row, t) in g.iter_mut().zip(&turns) {
                for v in row.iter_mut() {
                    *v *= *t;
                }
            }
        };
        grids.iter_mut().for_each(turn);
        let es: Vec<Vec<Complex32>> = es
            .iter()
            .map(|e| e.iter().map(|v| v * turns[2] * turns[0].conj()).collect())
            .collect();
        // Residual carrier offset: SSS against PSS, DMRS symbol 3 against symbol 1.
        let sss_seq = sss(nid1, pk.nid2);
        let r = dmrs(pci, ibar);
        let mut acc = Complex32::new(0.0, 0.0);
        for (gi, g) in grids.iter().enumerate() {
            let s: Complex32 = es[gi].iter().zip(&sss_seq).map(|(a, b)| a * *b).sum();
            acc += s;
            let mut z1 = Vec::new();
            let mut z3 = Vec::new();
            for (m, &(k, l)) in dres.iter().enumerate() {
                if l == 1 {
                    z1.push(g[1][k] * r[m].conj());
                } else if l == 3 {
                    z3.push(g[3][k] * r[m].conj());
                }
            }
            for (a, b) in z3.iter().zip(&z1) {
                acc += a * b.conj();
            }
        }
        let fres = acc.arg() as f64 / (TAU * 2.0 * 274.0 / rate);
        cfo += fres;
        for (gi, &pl) in places.iter().enumerate() {
            grids[gi] = self.ssb_grid(cap, c.offset_hz + cfo, scs, ssb_start(pl));
            turn(&mut grids[gi]);
        }
        // Channel and measurements, PBCH.
        let pres = pbch_res(pci);
        let mut rsrp = 0f64;
        let mut rssi = 0f64;
        let mut noise_t = 0f64;
        let mut mib: Option<(Mib, u8, bool)> = None;
        let mut tried = 0u32;
        let mut ok = 0u32;
        let mut after_ok = 0;
        for g in &grids {
            let (h, noise) = channel(g, &dres, &r);
            let sss_p: f64 = (0..127)
                .map(|i| g[2][56 + i].norm_sqr() as f64)
                .sum::<f64>()
                / 127.0;
            rsrp += (sss_p - noise).max(0.05 * sss_p);
            rssi += (0..4)
                .map(|l| g[l].iter().map(|v| v.norm_sqr() as f64).sum::<f64>())
                .sum::<f64>()
                / 4.0;
            noise_t += noise;
            if mib.is_some() {
                if after_ok >= 1 {
                    continue;
                }
                after_ok += 1;
            }
            tried += 1;
            let mut soft = Vec::with_capacity(864);
            let k = 2.0 * std::f32::consts::SQRT_2 / noise.max(1e-12) as f32;
            for &(kk, l) in &pres {
                let hh = h[l][kk];
                let z = g[l][kk] * hh.conj();
                soft.push(k * z.re);
                soft.push(k * z.im);
            }
            let mut got = None;
            for (v, l8) in [(ibar as usize & 7, true), (ibar as usize & 3, false)] {
                if let Some(m) = self.pbch.decode(&soft, pci, v) {
                    got = Some((m, ibar, l8 && ibar >= 4));
                    break;
                }
            }
            if let Some(gm) = got {
                ok += 1;
                if mib.is_none() {
                    mib = Some(gm);
                }
            }
        }
        let np = grids.len() as f64;
        let (rsrp, rssi, noise) = (rsrp / np, rssi / np, noise_t / np);
        // Lmax: 8 when the DMRS index was 4 or more with the Lmax-8 descrambling, 4 when it was 4 or more with the Lmax-4 one,
        // else the band's usual Lmax (8 without the centre).
        let lmax = mib.as_ref().map(|m| {
            if m.2 {
                8
            } else if m.1 >= 4 {
                4
            } else {
                match c.gscn {
                    Some(g) => {
                        let b = nr_band_of(g, Some(c.scs), &self.band_order);
                        crate::bands::usual_lmax(b, c.scs, gscn_to_hz(g).unwrap_or(0.0)) as u8
                    }
                    None => 8,
                }
            }
        });
        let ssb_index = match &mib {
            Some((_, ib, l8)) if !*l8 && *ib >= 4 => ib & 3,
            _ => ibar,
        };
        let beams = if mib.is_some() || sure {
            vec![Beam {
                ssb: ssb_index,
                rsrp_dbfs: 10.0 * rsrp.max(1e-30).log10(),
            }]
        } else {
            Vec::new()
        };
        Some(NrFound {
            cand: *c,
            cfo_hz: cfo,
            pci,
            significance,
            mib,
            dmrs_index: (ibar, sure),
            beams,
            lmax,
            anchor: places[0] + shift * step,
            anchor_ssb: ssb_index,
            sib1: None,
            rsrp_dbfs: 10.0 * rsrp.max(1e-30).log10(),
            rsrq_db: 10.0 * (20.0 * rsrp / rssi.max(1e-30)).max(1e-30).log10(),
            sinr_db: 10.0 * (rsrp / noise.max(1e-30)).max(1e-30).log10(),
            pss_db: pk.db,
            pbch_ok: ok,
            pbch_tried: tried,
        })
    }

    fn measure_beams(&mut self, cap: &Capture, c: &Cand, o: &mut NrFound) {
        let Some((_, ibar, l8)) = o.mib.clone() else {
            // A cell without a MIB drops the beams known only from its DMRS.
            o.beams.clear();
            return;
        };
        let scs = c.scs;
        let rate = 256.0 * scs as f64 * 1e3;
        let fs = cap.rate;
        let lmax: usize = o.lmax.map(|l| l as usize).unwrap_or(8);
        let anchor_idx = if lmax == 4 {
            ibar as usize & 3
        } else {
            ibar as usize
        };
        let (nid1, nid2) = (o.pci / 3, o.pci % 3);
        let p = pss(nid2);
        let s = sss(nid1, nid2);
        let period = 0.02 * fs;
        let step = fs / rate;
        let pattern = self.pattern_of(c);
        let mut found: Vec<Beam> = Vec::new();
        for i in 0..lmax.min(8) {
            let dsamp = (useful_start(scs, pattern.first_symbol(i)) as f64
                - useful_start(scs, pattern.first_symbol(anchor_idx)) as f64)
                * step;
            let mut pl = o.anchor + dsamp;
            while pl - period >= 0.0 {
                pl -= period;
            }
            while pl < 0.0 {
                pl += period;
            }
            let (mut sig, mut noise, mut count) = (0f64, 0f64, 0usize);
            while pl + 0.002 * fs < cap.samples.len() as f64 {
                let g = self.ssb_grid(cap, c.offset_hz + o.cfo_hz, scs, pl / step - 18.0);
                // PSS and SSS power, coherent over groups of 16 subcarriers, noise from the spread.
                for (l, seq) in [(0usize, &p), (2, &s)] {
                    let mut lo = 0;
                    while lo < 127 {
                        let hi = (lo + 16).min(127);
                        let nn = (hi - lo) as f64;
                        let z: Complex32 = (lo..hi).map(|i| g[l][56 + i] * seq[i]).sum();
                        let e: f64 = (lo..hi).map(|i| g[l][56 + i].norm_sqr() as f64).sum();
                        let coh = z.norm_sqr() as f64 / nn;
                        sig += coh;
                        noise += (e - coh).max(0.0);
                        lo = hi;
                    }
                }
                count += 1;
                pl += period;
            }
            if count == 0 {
                continue;
            }
            // Per RE: 254 REs of PSS and SSS per period; the noise over the remaining degrees of freedom.
            let n_re = 254.0 * count as f64;
            let groups = 16.0 * count as f64;
            let nv = noise / (n_re - groups).max(1.0);
            let ps = ((sig - groups * nv) / n_re).max(0.0);
            if ps > nv / 8.0 {
                found.push(Beam {
                    ssb: i as u8,
                    rsrp_dbfs: 10.0 * ps.max(1e-30).log10(),
                });
            }
        }
        for b in found {
            match o.beams.iter_mut().find(|x| x.ssb == b.ssb) {
                Some(x) => *x = b,
                None => o.beams.push(b),
            }
        }
        if !l8 && ibar >= 4 {
            for b in o.beams.iter_mut() {
                b.ssb &= 3;
            }
        }
        o.beams
            .sort_by(|a, b| a.ssb.cmp(&b.ssb).then(b.rsrp_dbfs.total_cmp(&a.rsrp_dbfs)));
        o.beams.dedup_by_key(|b| b.ssb);
        o.beams.retain(|b| (b.ssb as usize) < lmax);
    }
}

// Channel estimate from the DMRS: smoothed over neighbours, interpolated along frequency per symbol (not across the gap of
// symbol 2); noise from the residual.
fn channel(
    g: &[[Complex32; 240]; 4],
    dres: &[(usize, usize)],
    r: &[Complex32; 144],
) -> ([[Complex32; 240]; 4], f64) {
    let mut h = [[Complex32::new(0.0, 0.0); 240]; 4];
    let mut res = 0f64;
    let mut cnt = 0usize;
    for l in 1..4 {
        let pts: Vec<(usize, Complex32)> = dres
            .iter()
            .enumerate()
            .filter(|(_, (_, ll))| *ll == l)
            .map(|(m, &(k, _))| (k, g[l][k] * r[m].conj()))
            .collect();
        // Groups split at the gap of symbol 2.
        let groups: Vec<Vec<(usize, Complex32)>> = if l == 2 {
            vec![
                pts.iter().filter(|p| p.0 < 48).cloned().collect(),
                pts.iter().filter(|p| p.0 >= 192).cloned().collect(),
            ]
        } else {
            vec![pts]
        };
        for gr in groups {
            let n = gr.len();
            let sm: Vec<Complex32> = (0..n)
                .map(|i| {
                    let a = gr[i.saturating_sub(1)].1;
                    let b = gr[i].1;
                    let c = gr[(i + 1).min(n - 1)].1;
                    a * 0.25 + b * 0.5 + c * 0.25
                })
                .collect();
            for i in 0..n {
                res += (gr[i].1 - sm[i]).norm_sqr() as f64;
                cnt += 1;
            }
            let (k0, k1) = (gr[0].0, gr[n - 1].0);
            let (lo, hi) = if l == 2 {
                if k0 < 48 {
                    (0, 48)
                } else {
                    (192, 240)
                }
            } else {
                (0, 240)
            };
            for k in lo..hi {
                let v = if k <= k0 {
                    sm[0]
                } else if k >= k1 {
                    sm[n - 1]
                } else {
                    let j = gr
                        .iter()
                        .position(|p| p.0 > k)
                        .expect("k is below the last grid point");
                    let (ka, kb) = (gr[j - 1].0, gr[j].0);
                    let t = (k - ka) as f32 / (kb - ka) as f32;
                    sm[j - 1] * (1.0 - t) + sm[j] * t
                };
                h[l][k] = v;
            }
        }
    }
    (h, res / cnt.max(1) as f64 / 0.375)
}

impl NrSearcher {
    /// Searches the whole capture for every cell it holds.
    pub fn search(&mut self, cap: &Capture) -> NrSearchOut {
        self.best_db = f64::NEG_INFINITY;
        let (cands, range) = candidates(cap, &self.band_order);
        let mut out = NrSearchOut {
            gscn_range: range,
            candidates: cands.len(),
            ..Default::default()
        };
        for c in &cands {
            for f in self.search_candidate(cap, c) {
                let actual = f.cand.offset_hz + f.cfo_hz;
                if !out.cells.iter().any(|x| {
                    x.pci == f.pci
                        && x.cand.scs == f.cand.scs
                        && (x.cand.offset_hz + x.cfo_hz - actual).abs() < 30e3
                }) {
                    out.cells.push(f);
                }
            }
        }
        for f in out.cells.iter_mut() {
            self.read_sib1(cap, f);
        }
        out.best_pss_db = self.best_db;
        out
    }

    /// The time pattern of the blocks of candidate `c`: by its band when its GSCN is known.
    fn pattern_of(&self, c: &Cand) -> SsbPattern {
        let band = c.gscn.and_then(|g| nr_band_of(g, Some(c.scs), &self.band_order)).map(|b| b.number());
        SsbPattern::of(c.scs, band)
    }

    /// Forgets that the SIB1 of the cell (PCI, SS_REF in steps of 30 kHz) was tried, so that the next capture tries it.
    pub fn forget_sib1(&mut self, pci: u16, raster: i64) {
        self.sib1_tried.retain(|x| x.0 != (pci, raster));
    }

    /// Tries the SIB1 of a cell with a MIB, unless it was read or tried a short time ago.
    fn read_sib1(&mut self, cap: &Capture, f: &mut NrFound) {
        let Some((mib, _, _)) = &f.mib else {
            return;
        };
        let ssb_hz = f.cand.offset_hz + f.cfo_hz;
        // The nominal frequency of the block: its place on the sync raster, else its place by the radio centre.
        let nominal = cap.centre_known().then(|| {
            f.cand
                .gscn
                .and_then(gscn_to_hz)
                .unwrap_or(cap.centre_hz + f.cand.offset_hz)
        });
        let key = (f.pci, ((cap.centre_hz + f.cand.offset_hz) / 30e3).round() as i64);
        let now = cap.end_time;
        if let Some(&(_, at, ok)) = self.sib1_tried.iter().find(|x| x.0 == key) {
            let wait = if ok { SIB1_AGAIN_S } else { SIB1_RETRY_S };
            if now >= at && now - at < wait {
                return;
            }
        }
        let r = self.sib1.receive(
            cap,
            &Sib1Target {
                pci: f.pci,
                ssb_scs: f.cand.scs,
                mib,
                ssb: f.anchor_ssb,
                anchor: f.anchor,
                ssb_hz,
                ssb_nominal_hz: nominal,
                symbol_phase: self.symbol_phase,
                pattern: self.pattern_of(&f.cand),
            },
        );
        let ok = r.step == Sib1Step::Decoded;
        self.sib1_tried.retain(|x| x.0 != key);
        if self.sib1_tried.len() >= 4 * crate::nr::CELL_CAP {
            self.sib1_tried.remove(0);
        }
        self.sib1_tried.push((key, now, ok));
        f.sib1 = Some(r);
    }
}
