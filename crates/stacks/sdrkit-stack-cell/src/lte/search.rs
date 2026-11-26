//! The LTE search of one capture: carrier finder, candidates, PSS detection and tracking, SSS, frame
//! timing, PBCH and the MIB, channel estimation, and the system information on SI-RNTI (PCFICH, PDCCH common search space, PDSCH
//! with the turbo code, SIB1 combining). Receive only: a downlink control message is accepted only when its CRC matches SI-RNTI.

use super::msg::Mib;
use super::phy::*;
use crate::decoder::Capture;
use crate::dsp::{resample_to, welch, ResamplerCache};
use sdrkit_algo_dsp::fft::{Fft, FftPlans};
use sdrkit_algo_dsp::meter::power_to_db as db;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_core::Complex32;
use sdrkit_util_bits::crc::catalog::CRC16_XMODEM;
use std::f64::consts::TAU;
use std::sync::atomic::{AtomicU64, Ordering};

/// SI-RNTI.
pub const SI_RNTI: u32 = 0xFFFF;

/// PDSCH decodes started for an RNTI other than SI-RNTI (always 0: such a DCI is never accepted). For the receive-only test.
pub static NON_SI_PDSCH_DECODES: AtomicU64 = AtomicU64::new(0);

/// A hump of the spectrum.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Hump {
    /// Centre offset from 0 Hz.
    pub offset_hz: f64,
    /// Width, Hz.
    pub width_hz: f64,
    /// Peak over the floor, dB.
    pub level_db: f64,
}

/// The LTE carrier finder.
pub fn find_humps(x: &[Complex32], fs: f64) -> Vec<Hump> {
    let nfft = ((fs / 25e3).ceil() as usize).next_power_of_two();
    if nfft < 64 || x.len() < 8 * nfft {
        return Vec::new();
    }
    let s = welch(x, nfft, 240);
    let n = s.len();
    let sm: Vec<f32> = (0..n)
        .map(|i| {
            let lo = i.saturating_sub(2);
            let hi = (i + 2).min(n - 1);
            db(s[lo..=hi].iter().sum::<f32>() as f64) as f32
        })
        .collect();
    let floor = percentile(&sm, 1.0 / 6.0, &mut Vec::new());
    let thr = floor + 6.0;
    let bin = fs / nfft as f64;
    let edge = (0.03 * n as f64).ceil() as usize;
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        if sm[i] < thr {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i;
        let mut peak = sm[i];
        let mut j = i + 1;
        while j < n {
            if sm[j] >= thr {
                end = j;
                peak = peak.max(sm[j]);
                j += 1;
            } else if j + 2 < n && (sm[j + 1] >= thr || sm[j + 2] >= thr) {
                j += 1;
            } else {
                break;
            }
        }
        i = end + 1;
        let width = (end - start + 1) as f64 * bin;
        if (0.9e6..=19.5e6).contains(&width) && start >= edge && end + edge < n {
            out.push(Hump {
                offset_hz: ((start + end) as f64 / 2.0 - (n / 2) as f64) * bin,
                width_hz: width,
                level_db: (peak - floor) as f64,
            });
        }
    }
    out
}

/// A candidate frequency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cand {
    /// Offset from 0 Hz.
    pub offset_hz: f64,
    /// Carrier offset searched, +- Hz.
    pub range_hz: f64,
    /// Snapped to the 100 kHz raster.
    pub snapped: bool,
    /// The offset before snapping.
    pub raw_hz: f64,
}

/// What the search found of one cell.
#[derive(Clone, Debug, PartialEq)]
pub struct LteFound {
    /// The candidate.
    pub cand: Cand,
    /// Carrier offset from the candidate, Hz.
    pub cfo_hz: f64,
    /// Clock error, ppm.
    pub clock_ppm: f64,
    /// PCI.
    pub pci: u16,
    /// TDD.
    pub tdd: bool,
    /// Extended cyclic prefix.
    pub ext: bool,
    /// MIB, ports, SFN of the first whole frame.
    pub mib: Option<(Mib, usize, u32)>,
    /// PSS strength, dB.
    pub pss_db: f64,
    /// SSS significance.
    pub sss_sig: f64,
    /// RSRP, dBFS.
    pub rsrp_dbfs: f64,
    /// RSRQ, dB.
    pub rsrq_db: f64,
    /// SINR, dB.
    pub sinr_db: f64,
    /// Decoded transport blocks: (SFN, subframe, format 1C, bits).
    pub tbs: Vec<(u32, usize, bool, Vec<u8>)>,
    /// Transport blocks that failed their CRC.
    pub crc_failures: u32,
    /// DCIs that failed to parse.
    pub dci_failed: u32,
    /// What is needed to cancel this cell from the grids of a weaker one at the same timing.
    pub decoded: Option<Box<super::cancel::Decoded>>,
    /// Where its synchronisation signals sit (for the SSS cancellation and the later stages).
    pub sync: Option<SyncAt>,
}

/// The position of a cell's PSS in its candidate's sync-rate stream, and the start of its first whole frame in the input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyncAt {
    /// PSS position of half frame 0 (samples at 1.92 Msps).
    pub t0: f64,
    /// Half-frame length (samples at 1.92 Msps; carries the clock error).
    pub slope: f64,
    /// Which half frames (even 0, odd 1) hold subframe 0.
    pub parity: usize,
    /// The (cyclic prefix, duplex) hypothesis of the SSS.
    pub hyp: usize,
    /// Input sample of the first whole frame.
    pub start_in: f64,
}

/// Whether decoded cell `d` is at the same timing as a cell whose first whole frame starts at input sample `start_in` on carrier
/// `centre` (3.12: frame starts a whole number of frames apart within 6 samples at 1.92 Msps, carriers within 1.5 kHz): the
/// number of frames between them.
fn same_timing(rate: f64, start_in: f64, centre: f64, d: &super::cancel::Decoded) -> Option<i64> {
    if (d.centre_hz - centre).abs() >= 1500.0 {
        return None;
    }
    let frame_in = 0.01 * rate;
    let apart = ((start_in - d.start_in) / frame_in).round();
    let err = start_in - d.start_in - apart * frame_in;
    (err.abs() <= 6.0 * rate / SYNC_RATE).then_some(apart as i64)
}

/// The mean of +-6 neighbours along the 62 subcarriers of the synchronisation signals.
fn smooth62(v: &[Complex32]) -> Vec<Complex32> {
    (0..v.len())
        .map(|k| {
            let lo = k.saturating_sub(6);
            let hi = (k + 6).min(v.len() - 1);
            v[lo..=hi].iter().sum::<Complex32>() / (hi - lo + 1) as f32
        })
        .collect()
}

/// Whether a cell found without a MIB is listed (its SSS stands clear of noise).
pub fn listed(f: &LteFound) -> bool {
    f.mib.is_some() || f.sss_sig >= 5.5
}

/// The results of a capture.
#[derive(Clone, Debug, Default)]
pub struct LteSearchOut {
    /// Cells.
    pub cells: Vec<LteFound>,
    /// The humps of the capture.
    pub humps: Vec<Hump>,
    /// The candidate frequencies searched.
    pub candidates: usize,
    /// The strongest PSS correlation of the capture over its mean, dB, whether over the threshold or not.
    pub best_pss_db: f64,
}

/// The LTE searcher.
pub struct LteSearcher {
    cache: ResamplerCache,
    plans: FftPlans,
    /// Read the system information.
    pub system_information: bool,
    /// Cells per candidate.
    pub max_cells: usize,
    /// PSS threshold, dB.
    pub min_pss_db: f64,
    /// Carrier offset searched for snapped candidates, Hz.
    pub max_cfo_hz: f64,
    templ: [Vec<Complex32>; 3],
    borrow_tries: usize,
    si_wide: usize,
    tbcc: Tbcc,
    turbo: TurboRx,
    chan: ChanEstimator,
    /// The strongest PSS correlation over its mean since the search of the capture started, dB.
    best_db: f64,
}

const SYNC_RATE: f64 = 1.92e6;
const HALF: usize = 9600;

fn template(nid2: u16) -> Vec<Complex32> {
    let p = pss(nid2);
    let mut f = vec![Complex32::new(0.0, 0.0); 128];
    for (n, v) in p.iter().enumerate() {
        f[bin(6, 128, 5 + n)] = *v;
    }
    let mut ifft = Fft::inverse(128);
    ifft.process(&mut f);
    let e: f32 = f.iter().map(|v| v.norm_sqr()).sum::<f32>().sqrt();
    f.iter().map(|v| v / e).collect()
}

impl Default for LteSearcher {
    fn default() -> Self {
        LteSearcher {
            cache: ResamplerCache::default(),
            plans: FftPlans::new(),
            system_information: true,
            max_cells: 12,
            min_pss_db: 9.0,
            max_cfo_hz: 25e3,
            templ: [template(0), template(1), template(2)],
            borrow_tries: 0,
            si_wide: 0,
            tbcc: Tbcc::new(),
            turbo: TurboRx::default(),
            chan: ChanEstimator::default(),
            best_db: f64::NEG_INFINITY,
        }
    }
}

/// A PSS peak.
#[derive(Clone, Copy, Debug)]
struct Peak {
    nid2: u16,
    pos: usize,
    cfo: f64,
    db: f64,
}

/// One OFDM grid of a subframe: 14 (or 12) symbols x 12 N_RB subcarriers.
pub type Grid = Vec<Vec<Complex32>>;

/// The channel of a subframe: per port, symbol and subcarrier. It also holds the noise variance, the RSRP (port 0 CRS power minus
/// noise) and the RSSI (mean power of the symbols that carry CRS).
pub struct Chan {
    /// The channel `h[port][l][k]`.
    pub h: Vec<Grid>,
    /// Noise variance.
    pub noise: f64,
    /// RSRP (linear).
    pub rsrp: f64,
    /// RSSI (linear, mean total power of the symbols that carry CRS, over the grid).
    pub rssi: f64,
}

/// Demodulates subframe `sf_start` (sample of the subframe's first symbol) of `x` at FFT size `n`; the window starts a quarter of
/// the cyclic prefix early.
pub fn ofdm_subframe(
    fft: &mut Fft,
    x: &[Complex32],
    sf_start: f64,
    n: usize,
    n_rb: usize,
    ext: bool,
    out: &mut Grid,
) {
    let nsym = 2 * symbols_per_slot(ext);
    out.resize(nsym, Vec::new());
    let mut buf = vec![Complex32::new(0.0, 0.0); n];
    let base = sf_start.round() as i64;
    for l in 0..nsym {
        let (s, cp) = symbol_start(n, ext, l);
        let early = cp / 4;
        let at = base + (s + cp - early) as i64;
        for (i, b) in buf.iter_mut().enumerate() {
            let j = at + i as i64;
            *b = if j >= 0 && (j as usize) < x.len() {
                x[j as usize]
            } else {
                Complex32::new(0.0, 0.0)
            };
        }
        fft.process(&mut buf);
        let row = &mut out[l];
        row.clear();
        let norm = 1.0 / (n as f32).sqrt();
        for k in 0..12 * n_rb {
            let b = bin(n_rb, n, k);
            let signed = if b >= n / 2 {
                b as f64 - n as f64
            } else {
                b as f64
            };
            let a = TAU * signed * early as f64 / n as f64;
            row.push(buf[b] * Complex32::new(a.cos() as f32, a.sin() as f32) * norm);
        }
    }
}

/// Channel estimation of one subframe for `ports` ports. `wide`: average +-`wide` pilots along frequency and over the
/// subframe's pilot symbols. Each call allocates; a receiver keeps a [`ChanEstimator`] instead.
pub fn estimate(
    g: &Grid,
    pci: u16,
    n_rb: usize,
    sf: usize,
    ports: usize,
    ext: bool,
    wide: usize,
) -> Chan {
    let mut out = Chan::default();
    ChanEstimator::default().estimate(g, pci, n_rb, sf, ports, ext, wide, &mut out);
    out
}

impl Default for Chan {
    fn default() -> Self {
        Chan {
            h: Vec::new(),
            noise: 0.0,
            rsrp: 0.0,
            rssi: 0.0,
        }
    }
}

/// The channel estimator with its work buffers: after the first subframe of a size it allocates nothing.
#[derive(Default)]
pub struct ChanEstimator {
    res: Vec<(usize, Complex32)>,
    ls: Vec<Complex32>,
    sm: Vec<Complex32>,
    flat: Vec<Complex32>,
    mean: Vec<Complex32>,
    /// Pilot symbol rows: the symbol index and the row, `used` of them valid.
    rows: Vec<(usize, Vec<Complex32>)>,
}

impl ChanEstimator {
    /// Estimates the channel of grid `g` (one subframe `sf` of cell `pci`, `n_rb` RBs, `ports` ports) into `out`. `wide`: average
    /// +-`wide` pilots along frequency and over the subframe's pilot symbols.
    #[allow(clippy::too_many_arguments)]
    pub fn estimate(
        &mut self,
        g: &Grid,
        pci: u16,
        n_rb: usize,
        sf: usize,
        ports: usize,
        ext: bool,
        wide: usize,
        out: &mut Chan,
    ) {
        let nsym = 2 * symbols_per_slot(ext);
        let nsc = 12 * n_rb;
        out.h.resize_with(ports, Vec::new);
        for hp in out.h.iter_mut() {
            hp.resize_with(nsym, Vec::new);
            for row in hp.iter_mut() {
                row.clear();
                row.resize(nsc, Complex32::new(0.0, 0.0));
            }
        }
        let mut noise_acc = 0f64;
        let mut noise_n = 0usize;
        let mut p0_pow = 0f64;
        let mut p0_n = 0usize;
        let mut rssi = 0f64;
        let mut rssi_n = 0usize;
        for p in 0..ports {
            let mut used = 0;
            for l in 0..nsym {
                let ns = 2 * sf + l / symbols_per_slot(ext);
                crs_res_into(
                    pci,
                    n_rb,
                    p,
                    ns,
                    l % symbols_per_slot(ext),
                    ext,
                    &mut self.res,
                );
                let m = self.res.len();
                if m < 2 {
                    continue;
                }
                self.ls.clear();
                self.ls
                    .extend(self.res.iter().map(|&(k, v)| g[l][k] * v.conj()));
                let ls = &self.ls;
                if p == 0 {
                    rssi += g[l].iter().map(|v| v.norm_sqr() as f64).sum::<f64>();
                    rssi_n += 1;
                }
                self.sm.clear();
                self.sm.extend((0..m).map(|i| {
                    if i == 0 {
                        (ls[0] + ls[1]) * 0.5
                    } else if i == m - 1 {
                        (ls[m - 1] + ls[m - 2]) * 0.5
                    } else {
                        ls[i - 1] * 0.25 + ls[i] * 0.5 + ls[i + 1] * 0.25
                    }
                }));
                if p == 0 {
                    for (a, b) in ls.iter().zip(&self.sm) {
                        noise_acc += (a - b).norm_sqr() as f64;
                        noise_n += 1;
                        p0_pow += a.norm_sqr() as f64;
                        p0_n += 1;
                    }
                }
                if wide > 0 {
                    // Take out the phase slope of the timing offset, average +-wide pilots, put it back.
                    let slope: Complex32 = ls.windows(2).map(|w| w[1] * w[0].conj()).sum();
                    let rot = if slope.norm() > 0.0 {
                        slope / slope.norm()
                    } else {
                        Complex32::new(1.0, 0.0)
                    };
                    self.flat.clear();
                    let mut r = Complex32::new(1.0, 0.0);
                    for v in ls {
                        self.flat.push(v * r.conj());
                        r *= rot;
                    }
                    let mut r = Complex32::new(1.0, 0.0);
                    for (i, s) in self.sm.iter_mut().enumerate() {
                        let lo = i.saturating_sub(wide);
                        let hi = (i + wide).min(m - 1);
                        let avg: Complex32 =
                            self.flat[lo..=hi].iter().sum::<Complex32>() / (hi - lo + 1) as f32;
                        *s = avg * r;
                        r *= rot;
                    }
                }
                // Interpolate along frequency to every subcarrier.
                if self.rows.len() <= used {
                    self.rows.push((0, Vec::new()));
                }
                let (rl, row) = &mut self.rows[used];
                *rl = l;
                row.clear();
                row.resize(nsc, Complex32::new(0.0, 0.0));
                let (res, sm) = (&self.res, &self.sm);
                let k_at = |i: usize| res[i].0;
                let mut j = 0;
                for (k, rv) in row.iter_mut().enumerate() {
                    while j + 1 < m && k_at(j + 1) <= k {
                        j += 1;
                    }
                    *rv = if k <= k_at(0) {
                        let t = (k as f32 - k_at(0) as f32) / (k_at(1) as f32 - k_at(0) as f32);
                        sm[0] + (sm[1] - sm[0]) * t
                    } else if j + 1 >= m {
                        let t = (k as f32 - k_at(m - 2) as f32)
                            / (k_at(m - 1) as f32 - k_at(m - 2) as f32);
                        sm[m - 2] + (sm[m - 1] - sm[m - 2]) * t
                    } else {
                        let t = (k - k_at(j)) as f32 / (k_at(j + 1) - k_at(j)) as f32;
                        sm[j] * (1.0 - t) + sm[j + 1] * t
                    };
                }
                used += 1;
            }
            let pilots = &mut self.rows[..used];
            if used == 0 {
                continue;
            }
            if wide > 0 && used > 1 {
                // Mean over the subframe's pilot symbols, the phase drift taken out and put back.
                let drift: Complex32 = pilots
                    .windows(2)
                    .map(|w| {
                        w[1].1
                            .iter()
                            .zip(&w[0].1)
                            .map(|(a, b)| a * b.conj())
                            .sum::<Complex32>()
                    })
                    .sum();
                let per_sym = drift.arg()
                    / (pilots
                        .windows(2)
                        .map(|w| (w[1].0 - w[0].0) as f32)
                        .sum::<f32>()
                        / (used - 1) as f32);
                self.mean.clear();
                self.mean.resize(nsc, Complex32::new(0.0, 0.0));
                for (l, row) in pilots.iter() {
                    let r = Complex32::from_polar(1.0, -per_sym * *l as f32);
                    for (m, v) in self.mean.iter_mut().zip(row) {
                        *m += v * r;
                    }
                }
                let cnt = used as f32;
                for (l, row) in pilots.iter_mut() {
                    let r = Complex32::from_polar(1.0, per_sym * *l as f32);
                    for (v, m) in row.iter_mut().zip(&self.mean) {
                        *v = m / cnt * r;
                    }
                }
            }
            // Along time to every symbol (extrapolated at the ends).
            for l in 0..nsym {
                let row = &mut out.h[p][l];
                if used == 1 {
                    row.copy_from_slice(&pilots[0].1);
                    continue;
                }
                let mut i = 0;
                while i + 2 < used && pilots[i + 1].0 < l {
                    i += 1;
                }
                let (a, b) = (&pilots[i], &pilots[i + 1]);
                let t = (l as f32 - a.0 as f32) / (b.0 as f32 - a.0 as f32);
                for k in 0..nsc {
                    row[k] = a.1[k] + (b.1[k] - a.1[k]) * t;
                }
            }
        }
        out.noise = noise_acc / noise_n.max(1) as f64 / 0.375;
        let raw = p0_pow / p0_n.max(1) as f64;
        out.rsrp = (raw - out.noise).max(0.05 * raw);
        out.rssi = rssi / rssi_n.max(1) as f64;
    }
}

impl LteSearcher {
    fn pss_peaks(&mut self, x: &[Complex32], range: f64, thr: f64) -> Vec<Peak> {
        let hmax = ((range / 7500.0) - 0.34).ceil().max(0.0) as i64;
        let nfft = 4096usize;
        let tl = 128usize;
        let step = nfft - tl;
        let limit = (8 * HALF + tl).min(x.len());
        let x = &x[..limit];
        let mut fwd = self.plans.forward(nfft);
        let mut inv = self.plans.inverse(nfft);
        let nh = (2 * hmax + 1) as usize;
        let mut folds = vec![vec![vec![0f32; HALF]; nh]; 3];
        let mut tf = Vec::new();
        for t in &self.templ {
            let mut v = vec![Complex32::new(0.0, 0.0); nfft];
            v[..tl].copy_from_slice(t);
            fwd.process(&mut v);
            tf.push(v);
        }
        let mut buf = vec![Complex32::new(0.0, 0.0); nfft];
        let mut prod = vec![Complex32::new(0.0, 0.0); nfft];
        let mut s = 0usize;
        while s + tl < x.len() {
            for (i, b) in buf.iter_mut().enumerate() {
                *b = x.get(s + i).copied().unwrap_or_default();
            }
            fwd.process(&mut buf);
            for (nid2, t) in tf.iter().enumerate() {
                for (hi, hv) in (-hmax..=hmax).enumerate() {
                    let sh = 16 * hv;
                    for (k, p) in prod.iter_mut().enumerate() {
                        let kt = (k as i64 - sh).rem_euclid(nfft as i64) as usize;
                        *p = buf[k] * t[kt].conj();
                    }
                    inv.process(&mut prod);
                    let valid = step.min(x.len().saturating_sub(s + tl) + 1);
                    for i in 0..valid {
                        folds[nid2][hi][(s + i) % HALF] += prod[i].norm_sqr();
                    }
                }
            }
            s += step;
        }
        let mut peaks = Vec::new();
        for nid2 in 0..3 {
            for k in 0..3 {
                let mut best = (0f32, 0usize, 0usize, 0f32);
                for (hi, f) in folds[nid2].iter().enumerate() {
                    let mean = f.iter().sum::<f32>() / HALF as f32;
                    for (p, &v) in f.iter().enumerate() {
                        let r = v / mean.max(1e-30);
                        if r > best.3 {
                            best = (v, hi, p, r);
                        }
                    }
                }
                let d = 10.0 * (best.3.max(1e-30) as f64).log10();
                if k == 0 {
                    self.best_db = self.best_db.max(d);
                }
                let need = if k == 0 { thr } else { thr.max(12.0) };
                if d < need {
                    break;
                }
                peaks.push(Peak {
                    nid2: nid2 as u16,
                    pos: best.2,
                    cfo: (best.1 as i64 - hmax) as f64 * 7500.0,
                    db: d,
                });
                for f in folds[nid2].iter_mut() {
                    for dd in -64i64..=64 {
                        f[(best.2 as i64 + dd).rem_euclid(HALF as i64) as usize] = 0.0;
                    }
                }
            }
        }
        peaks.sort_by(|a, b| b.db.total_cmp(&a.db));
        peaks
    }

    /// A PSS probe at `offset`: the strongest PSS strength in 20 ms, if over 10 dB.
    pub fn probe(&mut self, cap: &Capture, offset: f64) -> Option<f64> {
        let n = (0.02 * SYNC_RATE) as usize + 256;
        let start = (cap.samples.len() as f64 / 2.0 - 0.01 * cap.rate).max(0.0);
        let mut y = Vec::new();
        resample_to(
            &mut self.cache,
            &cap.samples,
            cap.rate,
            offset,
            SYNC_RATE,
            1.3e6,
            start,
            0.0,
            n,
            &mut y,
        );
        self.pss_peaks(&y, 25e3, 10.0).first().map(|p| p.db)
    }

    fn corr(&self, y: &[Complex32], pos: f64, nid2: u16, cfo: f64) -> (Complex32, Complex32) {
        let t = &self.templ[nid2 as usize];
        let i0 = pos.round() as i64;
        let (mut a, mut b) = (Complex32::new(0.0, 0.0), Complex32::new(0.0, 0.0));
        for (i, tv) in t.iter().enumerate() {
            let j = i0 + i as i64;
            if j < 0 || j as usize >= y.len() {
                continue;
            }
            let ph = TAU * cfo * j as f64 / SYNC_RATE;
            let v = y[j as usize] * Complex32::new(ph.cos() as f32, -ph.sin() as f32) * tv.conj();
            if i < 64 {
                a += v;
            } else {
                b += v;
            }
        }
        (a, b)
    }

    /// Searches one candidate.
    pub fn search_candidate(&mut self, cap: &Capture, c: Cand) -> Vec<LteFound> {
        let fs = cap.rate;
        if fs < SYNC_RATE {
            return Vec::new();
        }
        let n = (cap.samples.len() as f64 * SYNC_RATE / fs) as usize;
        if n < 3 * HALF {
            return Vec::new();
        }
        let mut y = Vec::new();
        resample_to(
            &mut self.cache,
            &cap.samples,
            fs,
            c.offset_hz,
            SYNC_RATE,
            1.3e6,
            0.0,
            0.0,
            n,
            &mut y,
        );
        let peaks = self.pss_peaks(&y, c.range_hz, self.min_pss_db);
        // Every cell seen behind this candidate, listed or pending (all are known to the SSS cancellation).
        let mut all: Vec<LteFound> = Vec::new();
        let max_cells = self.max_cells;
        let limit = |all: &[LteFound], extra: usize| {
            all.iter().filter(|f| listed(f)).count() >= max_cells + extra
        };
        // Passes over the PSS peaks while new cells are found; after the first, only peaks at the timing of a known cell.
        for pass in 0..4 {
            let before = all.len();
            for pk in &peaks {
                if limit(&all, 0) {
                    break;
                }
                if pass > 0
                    && !all.iter().filter_map(|f| f.sync).any(|s| {
                        let d = (s.t0 - pk.pos as f64).rem_euclid(HALF as f64);
                        d <= 3.0 || d >= HALF as f64 - 3.0
                    })
                {
                    continue;
                }
                if let Some(f) = self.examine(cap, &c, &y, pk, &all) {
                    all.extend(f);
                }
            }
            if all.len() == before {
                break;
            }
        }
        // Leftovers: cells without a MIB under decoded cells (hard try, then a borrowed configuration), while anything decodes.
        for _ in 0..3 {
            if limit(&all, 4) {
                break;
            }
            let mut progress = false;
            for i in 0..all.len() {
                if all[i].mib.is_some() || all[i].tdd || all[i].sss_sig < 3.0 {
                    continue;
                }
                let decoded: Vec<super::cancel::Decoded> = all
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .filter_map(|(_, k)| k.decoded.as_deref().cloned())
                    .collect();
                if decoded.is_empty() {
                    continue;
                }
                let mut f = all[i].clone();
                self.complete(cap, &mut f, &decoded);
                if f.mib.is_some() {
                    all[i] = f;
                    progress = true;
                }
            }
            if !progress {
                break;
            }
        }
        // Decoded cells still without SIB1: the system information again with every other decoded cell at their timing taken out.
        if self.system_information {
            for _ in 0..3 {
                let mut progress = false;
                for i in 0..all.len() {
                    if all[i].mib.is_none() || has_sib1(&all[i]) {
                        continue;
                    }
                    let decoded: Vec<super::cancel::Decoded> = all
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| *j != i)
                        .filter_map(|(_, k)| k.decoded.as_deref().cloned())
                        .collect();
                    if decoded.is_empty() {
                        continue;
                    }
                    let mut f = all[i].clone();
                    f.tbs.clear();
                    f.crc_failures = 0;
                    f.dci_failed = 0;
                    self.complete(cap, &mut f, &decoded);
                    if has_sib1(&f) {
                        all[i] = f;
                        progress = true;
                    }
                }
                if !progress {
                    break;
                }
            }
        }
        all.retain(listed);
        all
    }

    /// Reads what a cell's MIB allows: for a cell without one under decoded cells at its timing, the hard PBCH try with them
    /// cancelled and, failing that, their configuration borrowed (confirmed only by a CRC-checked system information decode);
    /// for a cell with a MIB, the system information.
    fn complete(&mut self, cap: &Capture, f: &mut LteFound, decoded: &[super::cancel::Decoded]) {
        let Some(s) = f.sync else { return };
        let centre = f.cand.offset_hz + f.cfo_hz;
        // Cancellation rebuilds cells with the normal cyclic prefix only (the prefix must be the same).
        let same: Vec<(&super::cancel::Decoded, i64)> = decoded
            .iter()
            .filter(|d| d.pci != f.pci && !f.ext)
            .filter_map(|d| same_timing(cap.rate, s.start_in, centre, d).map(|a| (d, a)))
            .collect();
        // A site's sectors share one oscillator: under decoded cells, their carrier offset is adopted.
        let shared_centre = same.first().map(|(d, _)| d.centre_hz);
        if f.mib.is_none() && !f.tdd && f.sss_sig >= 3.0 {
            if let Some(sc) = shared_centre {
                f.mib = self.read_mib_hard(cap, sc, s.start_in, s.slope, f.pci, f.ext, &same);
                if f.mib.is_some() {
                    f.cfo_hz = sc - f.cand.offset_hz;
                }
            }
        }
        let fits =
            |n_rb: usize| f.cand.offset_hz.abs() + n_rb as f64 * 90e3 + 30e3 < cap.rate / 2.0;
        if let Some((m, ports, sfn0)) = f.mib {
            let n_rb = N_RB[m.bw_index as usize];
            if self.system_information && !f.tdd && fits(n_rb) {
                let (clock, pci, ext) = (f.clock_ppm, f.pci, f.ext);
                let mut g = f.clone();
                self.read_si(
                    cap, centre, clock, s.start_in, s.slope, pci, n_rb, ports, m.ng_index, ext,
                    sfn0, m, decoded, &mut g,
                );
                // Nothing decoded under decoded cells: again with their carrier offset and wider channel averaging.
                if g.tbs.is_empty() {
                    if let Some(sc) = shared_centre {
                        let mut h = f.clone();
                        h.cfo_hz = sc - f.cand.offset_hz;
                        self.si_wide = 2;
                        self.read_si(
                            cap, sc, clock, s.start_in, s.slope, pci, n_rb, ports, m.ng_index, ext,
                            sfn0, m, decoded, &mut h,
                        );
                        self.si_wide = 0;
                        if !h.tbs.is_empty() {
                            g = h;
                        }
                    }
                }
                *f = g;
            }
            return;
        }
        // A borrowed configuration: the same carrier has the same bandwidth and PHICH configuration, the SFN follows from the
        // timing; the decoded cell's port count first, then the others.
        if f.tdd || f.sss_sig < 3.0 || !self.system_information || self.borrow_tries >= 6 {
            return;
        }
        let Some(&(d, apart)) = same.first() else {
            return;
        };
        if !fits(d.n_rb) {
            return;
        }
        let centre = d.centre_hz;
        self.borrow_tries += 1;
        let sfn0 = (d.sfn0 as i64 + apart).rem_euclid(1024) as u32;
        let mut order = vec![d.ports];
        for p in [2usize, 4, 1] {
            if !order.contains(&p) {
                order.push(p);
            }
        }
        for ports in order {
            let mut g = f.clone();
            g.tbs.clear();
            g.crc_failures = 0;
            g.dci_failed = 0;
            self.read_si(
                cap,
                centre,
                f.clock_ppm,
                s.start_in,
                s.slope,
                f.pci,
                d.n_rb,
                ports,
                d.mib.ng_index,
                f.ext,
                sfn0,
                d.mib,
                decoded,
                &mut g,
            );
            if !g.tbs.is_empty() {
                g.mib = Some((d.mib, ports, sfn0));
                g.cfo_hz = centre - g.cand.offset_hz;
                if let Some(dd) = g.decoded.as_mut() {
                    dd.ports = ports;
                }
                *f = g;
                return;
            }
        }
    }

    /// The hard PBCH try: at the decoded cells' bandwidth, every subframe 0 with them cancelled, the channel averaged
    /// wider, then the plain decode.
    #[allow(clippy::too_many_arguments)]
    fn read_mib_hard(
        &mut self,
        cap: &Capture,
        centre: f64,
        start_in: f64,
        slope: f64,
        pci: u16,
        ext: bool,
        same: &[(&super::cancel::Decoded, i64)],
    ) -> Option<(Mib, usize, u32)> {
        let n_rb = same[0].0.n_rb;
        let n = fft_size(n_rb);
        let rate = 15e3 * n as f64;
        let ppm = (slope / HALF as f64 - 1.0) * 1e6;
        // Only subframe 0 of each frame is needed: it alone is resampled.
        let step_in = cap.rate / rate * (1.0 + ppm * 1e-6);
        let frame = 150 * n;
        let sf_len = 15 * n;
        let nframes =
            ((cap.samples.len() as f64 - start_in) / (frame as f64 * step_in)).floor() as usize;
        let mut x = Vec::new();
        let mut fft = self.plans.forward(n);
        let res = pbch_res(pci, n_rb, ext);
        let seg = 2 * res.len();
        let mut g: Grid = Vec::new();
        let mut est = Vec::new();
        let mut soft: Vec<[Vec<f32>; 3]> = Vec::with_capacity(nframes);
        for fr in 0..nframes {
            resample_to(
                &mut self.cache,
                &cap.samples,
                cap.rate,
                centre,
                rate,
                2.0 * (n_rb as f64 * 90e3 + 30e3),
                start_in + (fr * frame) as f64 * step_in,
                ppm,
                sf_len,
                &mut x,
            );
            ofdm_subframe(&mut fft, &x, 0.0, n, n_rb, ext, &mut g);
            for (d, apart) in same {
                if d.n_rb == n_rb {
                    let their = ((d.sfn0 as i64 + fr as i64 + apart).rem_euclid(1024)) as u32;
                    super::cancel::cancel(&mut g, d, their, 0);
                }
            }
            let ch = estimate(&g, pci, n_rb, 0, 4, ext, 3);
            let r: Vec<Complex32> = res.iter().map(|&(k, l)| g[l][k]).collect();
            let mut per = [Vec::new(), Vec::new(), Vec::new()];
            for (pi, ports) in [1usize, 2, 4].iter().enumerate() {
                let h: Vec<Vec<Complex32>> = (0..*ports)
                    .map(|p| res.iter().map(|&(k, l)| ch.h[p][l][k]).collect())
                    .collect();
                txd_decode(&r, &h, *ports, &mut est);
                soft_qpsk(&est, ch.noise.max(1e-12) as f32, &mut per[pi]);
                per[pi].resize(seg, 0.0);
            }
            soft.push(per);
        }
        decode_mib(&mut self.tbcc, &soft, seg, pci)
            .or_else(|| decode_mib_combined(&mut self.tbcc, &soft, seg, pci))
    }

    fn examine(
        &mut self,
        cap: &Capture,
        c: &Cand,
        y: &[Complex32],
        pk: &Peak,
        known: &[LteFound],
    ) -> Option<Vec<LteFound>> {
        let halves = (y.len() - 256) / HALF;
        // Tracking.
        let mut pts: Vec<(f64, f64)> = Vec::new();
        let mut slope = HALF as f64;
        let mut last = pk.pos as f64;
        for m in 0..halves {
            let pred = if pts.len() >= 2 {
                last + slope
            } else {
                pk.pos as f64 + m as f64 * HALF as f64
            };
            let mut best = (0f32, 0i64);
            let mut vals = [0f32; 25];
            for (di, d) in (-12i64..=12).enumerate() {
                let (a, b) = self.corr(y, pred + d as f64, pk.nid2, pk.cfo);
                let e = (a + b).norm_sqr();
                vals[di] = e;
                if e > best.0 {
                    best = (e, d);
                }
            }
            let di = (best.1 + 12) as usize;
            let mut frac = 0.0;
            if di > 0 && di < 24 {
                let (a, b, cc) = (vals[di - 1] as f64, vals[di] as f64, vals[di + 1] as f64);
                let den = a - 2.0 * b + cc;
                if den.abs() > 1e-20 {
                    frac = (0.5 * (a - cc) / den).clamp(-0.5, 0.5);
                }
            }
            let p = pred.round() + best.1 as f64 + frac;
            if pred + best.1 as f64 + 128.0 > y.len() as f64 {
                break;
            }
            pts.push((m as f64, p));
            last = p;
            if pts.len() >= 2 {
                let (_, s) = line(&pts);
                slope = s.clamp(HALF as f64 * (1.0 - 1e-4), HALF as f64 * (1.0 + 1e-4));
            }
        }
        if pts.len() < 3 {
            return None;
        }
        let (t0, mut slope) = line(&pts);
        if (slope / HALF as f64 - 1.0).abs() > 100e-6 {
            slope = HALF as f64;
        }
        let clock_ppm = (slope / HALF as f64 - 1.0) * 1e6;
        let pos = |m: usize| t0 + slope * m as f64;
        // Fine carrier offset, two steps.
        let mut acc = Complex32::new(0.0, 0.0);
        for m in 0..pts.len() {
            let (a, b) = self.corr(y, pos(m), pk.nid2, pk.cfo);
            acc += b * a.conj();
        }
        let mut cfo = pk.cfo + acc.arg() as f64 / (TAU * 64.0 / SYNC_RATE);
        let mut acc2 = Complex32::new(0.0, 0.0);
        let mut prev: Option<Complex32> = None;
        for m in 0..pts.len() {
            let (a, b) = self.corr(y, pos(m), pk.nid2, cfo);
            let v = a + b;
            if let Some(p) = prev {
                acc2 += v * p.conj();
            }
            prev = Some(v);
        }
        cfo += acc2.arg() as f64 / (TAU * slope / SYNC_RATE);
        // The sync-rate stream with the offset taken out.
        let fs = cap.rate;
        let n = y.len();
        let mut z = Vec::new();
        resample_to(
            &mut self.cache,
            &cap.samples,
            fs,
            c.offset_hz + cfo,
            SYNC_RATE,
            1.3e6,
            0.0,
            clock_ppm * 0.0,
            n,
            &mut z,
        );
        let mut fft = self.plans.forward(128);
        // SSS: four (prefix, duplex) hypotheses.
        let hyps: [(usize, bool, bool); 4] = [
            (137, false, false),
            (160, false, true),
            (412, true, false),
            (480, true, true),
        ];
        let p_seq = pss(pk.nid2);
        // Known cells at this timing (PSS within 2.5 samples in the same half frame, carrier within 1.5 kHz): their offset in half
        // frames from this cell's tracking.
        let kn: Vec<(u16, SyncAt, i64)> = known
            .iter()
            .filter(|k| (k.cand.offset_hz + k.cfo_hz - c.offset_hz - cfo).abs() < 1500.0)
            .filter_map(|k| {
                let s = k.sync?;
                let dm = ((pos(0) - s.t0) / s.slope).round();
                ((s.t0 + s.slope * dm - pos(0)).abs() <= 2.5).then_some((k.pci, s, dm as i64))
            })
            .collect();
        let mut best: Option<(f64, f64, usize, u16, usize)> = None; // (sig, score, hyp, nid1, parity)
        let mut all: Vec<(f64, f64, usize, u16, usize)> = Vec::new();
        let mut buf = vec![Complex32::new(0.0, 0.0); 128];
        for (hi, &(off, _tdd, _ext)) in hyps.iter().enumerate() {
            let mut zs: Vec<(Vec<f32>, f64)> = Vec::new();
            for m in 0..pts.len() {
                let pp = pos(m);
                let take = |at: f64, fft: &mut Fft, buf: &mut Vec<Complex32>| -> Vec<Complex32> {
                    let i0 = at.round() as i64;
                    for (i, b) in buf.iter_mut().enumerate() {
                        let j = i0 + i as i64;
                        *b = if j >= 0 && (j as usize) < z.len() {
                            z[j as usize]
                        } else {
                            Complex32::new(0.0, 0.0)
                        };
                    }
                    fft.process(buf);
                    (0..62).map(|nn| buf[bin(6, 128, 5 + nn)]).collect()
                };
                let mut yp = take(pp, &mut fft, &mut buf);
                let mut ys = take(pp - off as f64, &mut fft, &mut buf);
                // Cancellation of the known cells: their PSS (another N_ID2) and SSS times their channel.
                let mut known_h: Vec<Complex32> = vec![Complex32::new(0.0, 0.0); 62];
                for (kpci, ks, dm) in kn.iter().filter(|k| k.1.hyp == hi) {
                    let mk = m as i64 + dm;
                    let sf5 = (mk.rem_euclid(2) as usize) != ks.parity;
                    let kseq = sss(kpci / 3, kpci % 3, sf5);
                    let hk = smooth62(
                        &ys.iter()
                            .zip(&kseq)
                            .map(|(v, s)| v * *s)
                            .collect::<Vec<_>>(),
                    );
                    for n in 0..62 {
                        ys[n] -= hk[n] * kseq[n];
                    }
                    if kpci % 3 == pk.nid2 {
                        for n in 0..62 {
                            known_h[n] += hk[n];
                        }
                    } else {
                        let kp = pss(kpci % 3);
                        let hp = smooth62(
                            &yp.iter()
                                .zip(&kp)
                                .map(|(v, s)| v * s.conj())
                                .collect::<Vec<_>>(),
                        );
                        for n in 0..62 {
                            yp[n] -= hp[n] * kp[n];
                        }
                    }
                }
                let raw: Vec<Complex32> =
                    yp.iter().zip(&p_seq).map(|(a, b)| a * b.conj()).collect();
                let hch: Vec<Complex32> = smooth62(&raw)
                    .iter()
                    .zip(&known_h)
                    .map(|(a, b)| a - b)
                    .collect();
                let zz: Vec<f32> = ys
                    .iter()
                    .zip(&hch)
                    .map(|(s, h)| (s * h.conj()).re)
                    .collect();
                let en: f64 = zz.iter().map(|v| (*v as f64).powi(2)).sum();
                zs.push((zz, en));
            }
            let mut vals: Vec<(f64, f64, u16, usize)> = Vec::with_capacity(336);
            let norm: f64 = zs.iter().map(|(_, e)| (62.0 * e).sqrt()).sum();
            for nid1 in 0..168u16 {
                let s0 = sss(nid1, pk.nid2, false);
                let s5 = sss(nid1, pk.nid2, true);
                for parity in 0..2 {
                    let mut cacc = 0f64;
                    for (m, (zz, _)) in zs.iter().enumerate() {
                        let seq = if m % 2 == parity { &s0 } else { &s5 };
                        cacc += zz
                            .iter()
                            .zip(seq.iter())
                            .map(|(a, b)| (*a * *b) as f64)
                            .sum::<f64>();
                    }
                    vals.push((cacc, cacc / norm.max(1e-30), nid1, parity));
                }
            }
            let rms = (vals.iter().map(|v| v.0 * v.0).sum::<f64>() / vals.len() as f64).sqrt();
            for v in &vals {
                let sig = v.0 / rms.max(1e-30);
                all.push((sig, v.1, hi, v.2, v.3));
                if best.is_none_or(|b| sig > b.0) {
                    best = Some((sig, v.1, hi, v.2, v.3));
                }
            }
        }
        let (_, _, bhyp, _, _) = best?;
        // Known cells are not reported again.
        let is_known = |nid1: u16| {
            let pci = 3 * nid1 + pk.nid2;
            known.iter().any(|k| {
                k.pci == pci && (k.cand.offset_hz + k.cfo_hz - c.offset_hz - cfo).abs() < 1500.0
            })
        };
        let mut cands: Vec<(f64, f64, usize, u16, usize)> = all
            .iter()
            .copied()
            .filter(|v| v.2 == bhyp && (v.1 > 0.25 || v.0 >= 5.0) && !is_known(v.3))
            .collect();
        cands.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut picked: Vec<(f64, f64, usize, u16, usize)> = Vec::new();
        for cd in cands {
            if picked.len() >= 3 || picked.iter().any(|p| p.3 == cd.3) {
                continue;
            }
            if !picked.is_empty() && cd.0 < 5.0 {
                continue;
            }
            picked.push(cd);
        }
        let not_ok = picked.is_empty();
        if not_ok {
            match all
                .iter()
                .filter(|v| v.2 == bhyp && !is_known(v.3))
                .max_by(|a, b| a.0.total_cmp(&b.0))
            {
                Some(b) => picked.push(*b),
                None => return Some(Vec::new()),
            }
        }
        let (off, tdd, ext) = hyps[bhyp];
        let _ = off;
        let mut out = Vec::new();
        for (sig, _, _, nid1, parity) in picked {
            let pci = 3 * nid1 + pk.nid2;
            // Frame timing: the PSS of a subframe-0 half frame minus the frame offset.
            let m0 = parity;
            let frame_off = if !tdd {
                832.0
            } else if ext {
                2272.0
            } else {
                2204.0
            };
            let mut fstart = pos(m0) - frame_off;
            while fstart < 0.0 {
                fstart += 2.0 * slope;
            }
            while fstart - 2.0 * slope >= 0.0 {
                fstart -= 2.0 * slope;
            }
            let mib = self.read_mib(&z, fstart, slope, pci, ext);
            let mut f = LteFound {
                cand: *c,
                cfo_hz: cfo,
                clock_ppm,
                pci,
                tdd,
                ext,
                mib: mib.0,
                pss_db: pk.db,
                sss_sig: sig,
                rsrp_dbfs: db(mib.1),
                rsrq_db: db(6.0 * mib.1 / mib.2.max(1e-30)),
                sinr_db: db(mib.1 / mib.3.max(1e-30)),
                tbs: Vec::new(),
                crc_failures: 0,
                dci_failed: 0,
                decoded: None,
                sync: Some(SyncAt {
                    t0,
                    slope,
                    parity,
                    hyp: bhyp,
                    start_in: fstart * cap.rate / SYNC_RATE,
                }),
            };
            if not_ok && f.mib.is_none() {
                continue;
            }
            let decoded: Vec<super::cancel::Decoded> = known
                .iter()
                .chain(out.iter())
                .filter_map(|k| k.decoded.as_deref().cloned())
                .collect();
            self.complete(cap, &mut f, &decoded);
            // Kept: listed (a MIB, or an SSS clear of noise), or pending (FDD, significance 3.0 or more) for the later rounds.
            if listed(&f) || (!f.tdd && f.sss_sig >= 3.0) {
                out.push(f);
            }
        }
        Some(out)
    }

    // PBCH over every whole frame: (MIB with ports and SFN of the first whole frame, RSRP, RSSI, noise).
    #[allow(clippy::type_complexity)]
    fn read_mib(
        &mut self,
        z: &[Complex32],
        fstart: f64,
        slope: f64,
        pci: u16,
        ext: bool,
    ) -> (Option<(Mib, usize, u32)>, f64, f64, f64) {
        let frame = 2.0 * slope;
        let nframes = ((z.len() as f64 - fstart) / frame).floor() as usize;
        let mut fft = self.plans.forward(128);
        let res = pbch_res(pci, 6, ext);
        let seg = 2 * res.len();
        let mut soft: Vec<[Vec<f32>; 3]> = Vec::new();
        let (mut rsrp, mut rssi, mut noise) = (0f64, 0f64, 0f64);
        let mut g: Grid = Vec::new();
        let mut est = Vec::new();
        let mut ch = Chan::default();
        for f in 0..nframes {
            ofdm_subframe(&mut fft, z, fstart + f as f64 * frame, 128, 6, ext, &mut g);
            self.chan.estimate(&g, pci, 6, 0, 4, ext, 0, &mut ch);
            rsrp += ch.rsrp;
            rssi += ch.rssi;
            noise += ch.noise;
            let r: Vec<Complex32> = res.iter().map(|&(k, l)| g[l][k]).collect();
            let mut per = [Vec::new(), Vec::new(), Vec::new()];
            for (pi, ports) in [1usize, 2, 4].iter().enumerate() {
                let h: Vec<Vec<Complex32>> = (0..*ports)
                    .map(|p| res.iter().map(|&(k, l)| ch.h[p][l][k]).collect())
                    .collect();
                txd_decode(&r, &h, *ports, &mut est);
                soft_qpsk(&est, ch.noise.max(1e-12) as f32, &mut per[pi]);
                per[pi].resize(seg, 0.0);
            }
            soft.push(per);
        }
        let nf = nframes.max(1) as f64;
        let meas = (rsrp / nf, rssi / nf, noise / nf);
        (
            decode_mib(&mut self.tbcc, &soft, seg, pci),
            meas.0,
            meas.1,
            meas.2,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn read_si(
        &mut self,
        cap: &Capture,
        centre: f64,
        clock_ppm: f64,
        start_in: f64,
        slope: f64,
        pci: u16,
        n_rb: usize,
        ports: usize,
        ng: u8,
        ext: bool,
        sfn0: u32,
        mib: Mib,
        others: &[super::cancel::Decoded],
        f: &mut LteFound,
    ) {
        let n = fft_size(n_rb);
        let rate = 15e3 * n as f64;
        let ppm = (slope / HALF as f64 - 1.0) * 1e6;
        let _ = clock_ppm;
        let total = ((cap.samples.len() as f64 - start_in) * rate / cap.rate) as usize;
        let mut x = Vec::new();
        resample_to(
            &mut self.cache,
            &cap.samples,
            cap.rate,
            centre,
            rate,
            2.0 * (n_rb as f64 * 90e3 + 30e3),
            start_in,
            ppm,
            total,
            &mut x,
        );
        let sf_len = 15 * n;
        let frames = x.len() / (10 * sf_len);
        // Decoded cells at the same timing (frame starts a whole number of frames apart within 6 samples at 1.92 Msps, carriers
        // within 1.5 kHz) are taken out of every grid first.
        let frame_in = 10.0 * sf_len as f64 * cap.rate / rate;
        let tol = 6.0 * cap.rate / SYNC_RATE;
        let same: Vec<(&super::cancel::Decoded, i64)> = others
            .iter()
            .filter(|d| {
                !ext && d.n_rb == n_rb && d.pci != pci && (d.centre_hz - centre).abs() < 1500.0
            })
            .filter_map(|d| {
                let frames_apart = ((start_in - d.start_in) / frame_in).round();
                let err = start_in - d.start_in - frames_apart * frame_in;
                (err.abs() <= tol).then_some((d, frames_apart as i64))
            })
            .collect();
        let mut subframes: Vec<super::cancel::SubframeInfo> = Vec::new();
        let mut fft = self.plans.forward(n);
        let mut g: Grid = Vec::new();
        let mut rsrp = 0f64;
        let mut rssi = 0f64;
        let mut noise = 0f64;
        let mut cnt = 0usize;
        // SIB1 repetitions kept for combining: (allocation, rv, soft values).
        let mut sib1_reps: Vec<(Dci, usize, Vec<f32>, usize)> = Vec::new();
        let mut est = Vec::new();
        let mut ch = Chan::default();
        for fr in 0..frames {
            let sfn = (sfn0 + fr as u32) % 1024;
            for sf in 0..10 {
                let at = (fr * 10 + sf) * sf_len;
                ofdm_subframe(&mut fft, &x, at as f64, n, n_rb, ext, &mut g);
                for (d, apart) in &same {
                    let their = ((d.sfn0 as i64 + fr as i64 + apart).rem_euclid(1024)) as u32;
                    super::cancel::cancel(&mut g, d, their, sf);
                }
                self.chan
                    .estimate(&g, pci, n_rb, sf, ports, ext, self.si_wide, &mut ch);
                rsrp += ch.rsrp;
                rssi += ch.rssi;
                noise += ch.noise;
                cnt += 1;
                // PCFICH.
                let c1 = control(pci, n_rb, ports, 1, ng, ext);
                let scr = scrambling(
                    ((sf as u32 + 1) * (2 * pci as u32 + 1)) << 9 | pci as u32,
                    32,
                );
                let mut r = Vec::with_capacity(16);
                let mut hh: Vec<Vec<Complex32>> = vec![Vec::new(); ports];
                for reg in &c1.pcfich {
                    for &k in &reg.ks {
                        r.push(g[reg.l][k]);
                        for (p, hp) in hh.iter_mut().enumerate() {
                            hp.push(ch.h[p][reg.l][k]);
                        }
                    }
                }
                txd_decode(&r, &hh, ports, &mut est);
                let mut sb = Vec::new();
                soft_qpsk(&est, ch.noise as f32, &mut sb);
                let mut bestc = (f32::MIN, 1u8);
                for cfi in 1..=3u8 {
                    let w = cfi_codeword(cfi);
                    let m: f32 = sb
                        .iter()
                        .enumerate()
                        .map(|(i, &v)| if (w[i] ^ scr[i]) == 0 { v } else { -v })
                        .sum();
                    if m > bestc.0 {
                        bestc = (m, cfi);
                    }
                }
                let cfi = bestc.1;
                // The PCFICH always carries signal: a PDCCH candidate much weaker than it holds nothing (unused CCEs are empty).
                let pcfich_level = sb.iter().map(|v| v.abs()).sum::<f32>() / sb.len().max(1) as f32;
                let ctl = control(pci, n_rb, ports, cfi, ng, ext);
                let n_reg = ctl.pdcch.len();
                let n_cce = n_reg / 9;
                if n_cce == 0 {
                    continue;
                }
                // PDCCH: REGs back into quadruplet order.
                let perm = pdcch_permutation(n_reg, pci);
                let mut rq = vec![Complex32::new(0.0, 0.0); 4 * n_reg];
                let mut hq: Vec<Vec<Complex32>> =
                    vec![vec![Complex32::new(0.0, 0.0); 4 * n_reg]; ports];
                for (i, reg) in ctl.pdcch.iter().enumerate() {
                    let q = perm[i];
                    for (j, &k) in reg.ks.iter().enumerate() {
                        rq[4 * q + j] = g[reg.l][k];
                        for p in 0..ports {
                            hq[p][4 * q + j] = ch.h[p][reg.l][k];
                        }
                    }
                }
                txd_decode(&rq, &hq, ports, &mut est);
                let mut soft = Vec::new();
                soft_qpsk(&est, ch.noise as f32, &mut soft);
                let scr = scrambling((sf as u32) << 9 | pci as u32, soft.len());
                for (v, &s) in soft.iter_mut().zip(&scr) {
                    if s == 1 {
                        *v = -*v;
                    }
                }
                // Common search space, SI-RNTI only.
                let mut seen: Vec<Vec<u8>> = Vec::new();
                let mut info = super::cancel::SubframeInfo {
                    sfn,
                    sf,
                    cfi,
                    dcis: Vec::new(),
                };
                for (lvl, ncand) in [(4usize, 4usize), (8, 2)] {
                    let slots = n_cce / lvl;
                    if slots == 0 {
                        continue;
                    }
                    for m in 0..ncand {
                        let cce = lvl * (m % slots);
                        let e = 72 * lvl;
                        if 72 * cce + e > soft.len() {
                            continue;
                        }
                        let s = &soft[72 * cce..72 * cce + e];
                        let level = s.iter().map(|v| v.abs()).sum::<f32>() / e as f32;
                        if level < 0.5 * pcfich_level {
                            continue;
                        }
                        for format in [DciFormat::F1a, DciFormat::F1c] {
                            let k = dci_size(format, n_rb) + 16;
                            let mut word = [0u8; 64];
                            if k > word.len() || !self.tbcc.decode(s, k, &mut word[..k]) {
                                continue;
                            }
                            let bits = &word[..k];
                            if !CRC16_XMODEM.verify_bits_masked(bits, SI_RNTI as u16) {
                                continue;
                            }
                            let mut re = [0u8; 72 * 8];
                            if !self.tbcc.encode(bits, &mut re[..e]) || agreement(s, &re[..e]) < 0.3
                            {
                                continue;
                            }
                            let payload = bits[..k - 16].to_vec();
                            if seen.contains(&payload) {
                                continue;
                            }
                            seen.push(payload.clone());
                            match dci_parse(&payload, format, n_rb) {
                                None => f.dci_failed += 1,
                                Some(d) => {
                                    match self.pdsch(
                                        &g,
                                        &ch,
                                        pci,
                                        n_rb,
                                        ports,
                                        ext,
                                        ctl.symbols,
                                        sf,
                                        &d,
                                        SI_RNTI,
                                    ) {
                                        Some((tb, ok, softv, k)) => {
                                            info.dcis.push((cce, d, ok.then(|| tb.clone())));
                                            if ok {
                                                if !f.tbs.iter().any(|t| t.3 == tb) {
                                                    f.tbs.push((
                                                        sfn,
                                                        sf,
                                                        d.format == DciFormat::F1c,
                                                        tb,
                                                    ));
                                                }
                                            } else {
                                                f.crc_failures += 1;
                                            }
                                            if sf == 5
                                                && sfn.is_multiple_of(2)
                                                && d.format == DciFormat::F1a
                                            {
                                                sib1_reps.push((d, sib1_rv(sfn), softv, k));
                                            }
                                        }
                                        None => f.dci_failed += 1,
                                    }
                                }
                            }
                        }
                    }
                }
                subframes.push(info);
            }
        }
        f.decoded = Some(Box::new(super::cancel::Decoded {
            pci,
            n_rb,
            ports,
            mib,
            sfn0,
            start_in,
            centre_hz: centre,
            subframes,
        }));
        // SIB1 combining when no single SIB1 decoded.
        let have_sib1 = f.tbs.iter().any(|t| t.1 == 5 && !t.2);
        if !have_sib1 && sib1_reps.len() >= 2 {
            let mut allocs: Vec<Dci> = Vec::new();
            for r in &sib1_reps {
                let key = Dci { rv: 0, ..r.0 };
                if !allocs.contains(&key) && allocs.len() < 4 {
                    allocs.push(key);
                }
            }
            for a in allocs {
                let Some(tbs) = tbs(&a) else { continue };
                let Some((k, fil)) = turbo_block(tbs) else {
                    continue;
                };
                if !self.turbo.begin(k, fil) {
                    continue;
                }
                for r in sib1_reps.iter().filter(|r| Dci { rv: 0, ..r.0 } == a) {
                    self.turbo.add(&r.2, r.1 as u8);
                }
                if let Some(dec) = self.turbo.decode(10) {
                    let tb = dec[fil..fil + tbs].to_vec();
                    if f.crc_failures > 0 {
                        f.crc_failures = f.crc_failures.saturating_sub(sib1_reps.len() as u32);
                    }
                    f.tbs.push((0, 5, false, tb));
                    break;
                }
            }
        }
        if cnt > 0 {
            let c = cnt as f64;
            f.rsrp_dbfs = db(rsrp / c);
            f.rsrq_db = db(n_rb as f64 * (rsrp / c) / (rssi / c).max(1e-30));
            f.sinr_db = db((rsrp / c) / (noise / c).max(1e-30));
        }
    }

    /// Demodulates and decodes the PDSCH of an assignment: (transport block, CRC ok, the soft values received, block size K).
    #[allow(clippy::too_many_arguments)]
    fn pdsch(
        &mut self,
        g: &Grid,
        ch: &Chan,
        pci: u16,
        n_rb: usize,
        ports: usize,
        ext: bool,
        ctrl: usize,
        sf: usize,
        d: &Dci,
        rnti: u32,
    ) -> Option<(Vec<u8>, bool, Vec<f32>, usize)> {
        if rnti != SI_RNTI {
            NON_SI_PDSCH_DECODES.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        let prbs = assignment_prbs(d, n_rb);
        if prbs.iter().flatten().any(|&p| p >= n_rb) {
            return None;
        }
        let res = pdsch_res(pci, n_rb, ports, ext, ctrl, sf, &prbs);
        let r: Vec<Complex32> = res.iter().map(|&(k, l)| g[l][k]).collect();
        let h: Vec<Vec<Complex32>> = (0..ports)
            .map(|p| res.iter().map(|&(k, l)| ch.h[p][l][k]).collect())
            .collect();
        let mut est = Vec::new();
        txd_decode(&r, &h, ports, &mut est);
        let mut soft = Vec::new();
        soft_qpsk(&est, ch.noise as f32, &mut soft);
        let gbits = 2 * res.len();
        let layers = if ports > 1 { 2 } else { 1 };
        let e = (2 * layers) * (gbits / (2 * layers));
        soft.truncate(e);
        let c_init = rnti * (1 << 14) + ((sf as u32) << 9) + pci as u32;
        let scr = scrambling(c_init, e);
        for (v, &s) in soft.iter_mut().zip(&scr) {
            if s == 1 {
                *v = -*v;
            }
        }
        let tbs = tbs(d)?;
        let (k, fil) = turbo_block(tbs)?;
        let rvs: &[u8] = match d.format {
            DciFormat::F1a => &[d.rv],
            DciFormat::F1c => &[0, 2, 3, 1],
        };
        for &rv in rvs {
            if !self.turbo.begin(k, fil) || !self.turbo.add(&soft, rv) {
                continue;
            }
            if let Some(dec) = self.turbo.decode(8) {
                return Some((dec[fil..fil + tbs].to_vec(), true, soft, k));
            }
        }
        Some((Vec::new(), false, soft, k))
    }
}

/// Whether a cell's SIB1 was read.
fn has_sib1(f: &LteFound) -> bool {
    f.tbs.iter().any(|t| t.1 == 5 && !t.2)
}

/// The plain MIB decode over per-frame soft values (per port hypothesis 1, 2, 4; `seg` values per frame): every span of 1
/// to 4 frames, every first frame and first segment; a MIB counts only when two decodes from disjoint frames agree. Returns the
/// MIB, the port count and the SFN of frame 0.
fn decode_mib(
    tbcc: &mut Tbcc,
    soft: &[[Vec<f32>; 3]],
    seg: usize,
    pci: u16,
) -> Option<(Mib, usize, u32)> {
    let nframes = soft.len();
    let scr = scrambling(pci as u32, 4 * seg);
    let mut decodes: Vec<(usize, usize, Mib, usize, u32)> = Vec::new();
    let mut s = vec![0f32; 4 * seg];
    let mut re = vec![0u8; 4 * seg];
    for (pi, ports) in [1usize, 2, 4].iter().enumerate() {
        let mask = match ports {
            1 => 0,
            2 => 0xFFFF,
            _ => 0x5555,
        };
        for span in 1..=4usize {
            for f0 in 0..nframes.saturating_sub(span - 1) {
                for j0 in 0..=(4 - span) {
                    s.iter_mut().for_each(|v| *v = 0.0);
                    for k in 0..span {
                        let j = j0 + k;
                        for i in 0..seg {
                            let v = soft[f0 + k][pi][i];
                            s[j * seg + i] = if scr[j * seg + i] == 1 { -v } else { v };
                        }
                    }
                    let mut bits = [0u8; 40];
                    if !tbcc.decode(&s, 40, &mut bits)
                        || !CRC16_XMODEM.verify_bits_masked(&bits, mask)
                        || !tbcc.encode(&bits, &mut re)
                    {
                        continue;
                    }
                    let used = &s[j0 * seg..(j0 + span) * seg];
                    if agreement(used, &re[j0 * seg..(j0 + span) * seg]) < 0.2 {
                        continue;
                    }
                    let Some(m) = Mib::from_bits(&bits[..24]) else {
                        continue;
                    };
                    let sfn_f0 = 4 * m.sfn_msb as u32 + j0 as u32;
                    let sfn0 = (sfn_f0 + 1024 - (f0 as u32 % 1024)) % 1024;
                    decodes.push((f0, span, m, *ports, sfn0));
                }
            }
        }
    }
    for (i, a) in decodes.iter().enumerate() {
        for b in decodes.iter().skip(i + 1) {
            let disjoint = a.0 + a.1 <= b.0 || b.0 + b.1 <= a.0;
            if disjoint
                && a.2.bw_index == b.2.bw_index
                && a.3 == b.3
                && a.2.phich_extended == b.2.phich_extended
                && a.2.ng_index == b.2.ng_index
                && a.4 == b.4
            {
                return Some((a.2, a.3, a.4));
            }
        }
    }
    None
}

/// The combined MIB decode (hard decisions only): every frame of the capture in one decode. Period P's soft values are turned
/// into period 0's by flipping them where the code word of the SFN difference ((s0 + P) mod 256) XOR s0 has a one. For each place
/// `a` of frame 0 in its period, each port hypothesis and each class of start values s0 giving the same differences: the CRC with
/// the port mask, zero spare bits, a valid bandwidth, the decoded SFN one of the class, and an agreement of at least 0.05; the
/// best agreement wins.
fn decode_mib_combined(
    tbcc: &mut Tbcc,
    soft: &[[Vec<f32>; 3]],
    seg: usize,
    pci: u16,
) -> Option<(Mib, usize, u32)> {
    let nframes = soft.len();
    if nframes < 2 {
        return None;
    }
    let len = 4 * seg;
    let scr = scrambling(pci as u32, len);
    // Code word of a MIB whose only non-zero bits are an 8-bit SFN value (with its CRC, no mask: the masks cancel).
    let words: Vec<Vec<u8>> = (0..=255u8)
        .map(|d| {
            let m = Mib {
                bw_index: 0,
                phich_extended: false,
                ng_index: 0,
                sfn_msb: d,
                spare: 0,
            };
            let mut b = m.to_bits().to_vec();
            append_crc16(&mut b, 0);
            let mut w = vec![0u8; len];
            tbcc.encode(&b, &mut w);
            w
        })
        .collect();
    let mut best: Option<(f32, Mib, usize, u32)> = None;
    let mut comb = vec![0f32; len];
    for a in 0..4usize {
        let periods = (a + nframes - 1) / 4 + 1;
        // Classes of start values with the same differences.
        let mut classes: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for s0 in 0..=255u8 {
            let diffs: Vec<u8> = (0..periods)
                .map(|p| s0.wrapping_add(p as u8) ^ s0)
                .collect();
            match classes.iter_mut().find(|c| c.0 == diffs) {
                Some(c) => c.1.push(s0),
                None => classes.push((diffs, vec![s0])),
            }
        }
        for (pi, ports) in [1usize, 2, 4].iter().enumerate() {
            let mask = match ports {
                1 => 0,
                2 => 0xFFFF,
                _ => 0x5555,
            };
            for (diffs, members) in &classes {
                comb.iter_mut().for_each(|v| *v = 0.0);
                for (f, fs) in soft.iter().enumerate() {
                    let j = (a + f) % 4;
                    let p = (a + f) / 4;
                    let w = (diffs[p] != 0).then(|| &words[diffs[p] as usize]);
                    for i in 0..seg {
                        let at = j * seg + i;
                        let mut v = fs[pi][i];
                        if scr[at] == 1 {
                            v = -v;
                        }
                        if w.is_some_and(|w| w[at] == 1) {
                            v = -v;
                        }
                        comb[at] += v;
                    }
                }
                let mut bits = [0u8; 40];
                if !tbcc.decode(&comb, 40, &mut bits)
                    || !CRC16_XMODEM.verify_bits_masked(&bits, mask)
                {
                    continue;
                }
                let Some(m) = Mib::from_bits(&bits[..24]) else {
                    continue;
                };
                if m.spare != 0 || !members.contains(&m.sfn_msb) {
                    continue;
                }
                let mut re = vec![0u8; len];
                tbcc.encode(&bits, &mut re);
                let ag = agreement(&comb, &re);
                if ag < 0.05 || best.as_ref().is_some_and(|b| b.0 >= ag) {
                    continue;
                }
                let sfn0 = (4 * m.sfn_msb as u32 + a as u32) % 1024;
                best = Some((ag, m, *ports, sfn0));
            }
        }
    }
    best.map(|b| (b.1, b.2, b.3))
}

fn line(p: &[(f64, f64)]) -> (f64, f64) {
    let m = p.len() as f64;
    let sx: f64 = p.iter().map(|v| v.0).sum();
    let sy: f64 = p.iter().map(|v| v.1).sum();
    let sxx: f64 = p.iter().map(|v| v.0 * v.0).sum();
    let sxy: f64 = p.iter().map(|v| v.0 * v.1).sum();
    let den = m * sxx - sx * sx;
    if den.abs() < 1e-12 {
        return (p[0].1, HALF as f64);
    }
    let b = (m * sxy - sx * sy) / den;
    ((sy - b * sx) / m, b)
}

/// The candidates of a capture.
pub fn candidates(s: &mut LteSearcher, cap: &Capture, humps: &[Hump]) -> Vec<Cand> {
    let mut out: Vec<Cand> = Vec::new();
    let push = |out: &mut Vec<Cand>, c: Cand| {
        if !out.iter().any(|x| (x.offset_hz - c.offset_hz).abs() < 50e3) {
            out.push(c);
        }
    };
    let snap = |f: f64| (f / 100e3).round() * 100e3;
    if cap.centre_known() {
        let dial = cap.centre_hz + cap.dial_offset_hz;
        push(
            &mut out,
            Cand {
                offset_hz: snap(dial) - cap.centre_hz,
                range_hz: s.max_cfo_hz,
                snapped: true,
                raw_hz: cap.dial_offset_hz,
            },
        );
        let lim = 0.46 * cap.rate - 0.7e6;
        let lo = ((cap.centre_hz - lim) / 100e3).ceil() as i64;
        let hi = ((cap.centre_hz + lim) / 100e3).floor() as i64;
        let mut probes: Vec<(f64, f64)> = Vec::new();
        for r in lo..=hi {
            let off = r as f64 * 100e3 - cap.centre_hz;
            if let Some(d) = s.probe(cap, off) {
                probes.push((d, off));
            }
        }
        probes.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut kept: Vec<f64> = Vec::new();
        for (_, off) in probes {
            if kept.iter().any(|&k| (k - off).abs() < 1.4e6) {
                continue;
            }
            kept.push(off);
            push(
                &mut out,
                Cand {
                    offset_hz: off,
                    range_hz: s.max_cfo_hz,
                    snapped: true,
                    raw_hz: off,
                },
            );
        }
        for h in humps {
            if kept.iter().any(|&k| (k - h.offset_hz).abs() < 1.4e6) {
                continue;
            }
            push(
                &mut out,
                Cand {
                    offset_hz: snap(cap.centre_hz + h.offset_hz) - cap.centre_hz,
                    range_hz: s.max_cfo_hz,
                    snapped: true,
                    raw_hz: h.offset_hz,
                },
            );
        }
    } else {
        push(
            &mut out,
            Cand {
                offset_hz: cap.dial_offset_hz,
                range_hz: 60e3,
                snapped: false,
                raw_hz: cap.dial_offset_hz,
            },
        );
        for h in humps {
            push(
                &mut out,
                Cand {
                    offset_hz: h.offset_hz,
                    range_hz: 60e3,
                    snapped: false,
                    raw_hz: h.offset_hz,
                },
            );
        }
    }
    out
}

impl LteSearcher {
    /// Searches the whole capture for every cell it holds.
    pub fn search(&mut self, cap: &Capture) -> LteSearchOut {
        self.borrow_tries = 0;
        self.best_db = f64::NEG_INFINITY;
        let humps = find_humps(&cap.samples, cap.rate);
        let mut out = LteSearchOut {
            humps: humps.clone(),
            ..Default::default()
        };
        let cands = candidates(self, cap, &humps);
        out.candidates = cands.len();
        for c in cands {
            let mut found = self.search_candidate(cap, c);
            if found.is_empty() && c.snapped && (c.raw_hz - c.offset_hz).abs() > 20e3 {
                let raw = Cand {
                    offset_hz: c.raw_hz,
                    range_hz: 60e3,
                    snapped: false,
                    raw_hz: c.raw_hz,
                };
                found = self.search_candidate(cap, raw);
            }
            for f in found {
                let act = f.cand.offset_hz + f.cfo_hz;
                if !out
                    .cells
                    .iter()
                    .any(|x| x.pci == f.pci && (x.cand.offset_hz + x.cfo_hz - act).abs() < 60e3)
                {
                    out.cells.push(f);
                }
            }
        }
        out.best_pss_db = self.best_db;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng as Noise;

    /// Per-frame PBCH soft values of a MIB (2 ports) from SFN `sfn0`, with Gaussian noise of standard deviation `sigma`.
    fn frames(m: Mib, pci: u16, sfn0: u32, n: usize, sigma: f32) -> Vec<[Vec<f32>; 3]> {
        let seg = 480;
        let scr = scrambling(pci as u32, 4 * seg);
        let mut noise = Noise::new(7);
        (0..n)
            .map(|f| {
                let sfn = sfn0 + f as u32;
                let mut b = Mib {
                    sfn_msb: (sfn >> 2) as u8,
                    ..m
                }
                .to_bits()
                .to_vec();
                append_crc16(&mut b, 0xFFFF);
                let c = tbcc_encode(&b, 4 * seg);
                let j = (sfn % 4) as usize;
                let v: Vec<f32> = (0..seg)
                    .map(|i| {
                        let bit = c[j * seg + i] ^ scr[j * seg + i];
                        (1.0 - 2.0 * bit as f32) + sigma * noise.complex_normal(2.0 * 1.0 * 1.0).re
                    })
                    .collect();
                let other: Vec<f32> = (0..seg)
                    .map(|_| noise.complex_normal(2.0 * 1.0 * 1.0).re)
                    .collect();
                [other.clone(), v, other]
            })
            .collect()
    }

    #[test]
    fn combined_mib_decode_across_periods() {
        let m = Mib {
            bw_index: 2,
            phich_extended: false,
            ng_index: 1,
            sfn_msb: 0,
            spare: 0,
        };
        // Clean: the plain decode and the combined one agree, across a period boundary of the SFN's 8 bits (frames 1021 to 1040).
        let soft = frames(m, 304, 1021, 20, 0.0);
        let plain = decode_mib(&mut Tbcc::new(), &soft, 480, 304).expect("plain");
        let comb = decode_mib_combined(&mut Tbcc::new(), &soft, 480, 304).expect("combined");
        assert_eq!((plain.1, plain.2), (2, 1021));
        assert_eq!((comb.0.bw_index, comb.1, comb.2), (2, 2, 1021));
        // Weak: one frame alone cannot carry the MIB, all twenty together do.
        let soft = frames(m, 304, 60, 20, 6.0);
        assert!(decode_mib(&mut Tbcc::new(), &soft, 480, 304).is_none());
        let comb = decode_mib_combined(&mut Tbcc::new(), &soft, 480, 304).expect("combined, weak");
        assert_eq!(
            (comb.0.bw_index, comb.0.ng_index, comb.1, comb.2),
            (2, 1, 2, 60)
        );
        // Noise alone: nothing.
        let mut noise = Noise::new(3);
        let soft: Vec<[Vec<f32>; 3]> = (0..20)
            .map(|_| {
                let v: Vec<f32> = (0..480)
                    .map(|_| noise.complex_normal(2.0 * 1.0 * 1.0).re)
                    .collect();
                [v.clone(), v.clone(), v]
            })
            .collect();
        assert!(decode_mib(&mut Tbcc::new(), &soft, 480, 304).is_none());
    }
}
