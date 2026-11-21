//! The UMTS cell search of one capture: slot timing from the primary synchronisation code, code group and frame timing from the
//! secondary codes, the primary scrambling code from the CPICH, the broadcast channel (MIB and SIB 3) from the P-CCPCH, and the
//! sample clock of the capture from the synchronisation code over all frames (the capture is resampled and searched again when
//! the clock is off).

use super::bch::{decode, parse_mib, parse_sib3, Mib, SibAssembly, FRAME_BITS, SIB_3, SIB_MIB};
use super::gen::NATIVE_RATE;
use super::CHIP_RATE;
use crate::decoder::Capture;
use crate::dsp::{resample_to, ResamplerCache};
use sdrkit_algo_dsp::fir::Convolver;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::{self, Norm, Shape};
use sdrkit_algo_modem::spreading::ovsf;
use sdrkit_algo_modem::spreading::wcdma::{
    psc, psc_base, psc_block_signs, ssc, DlScrambling, FRAME_CHIPS, SLOT_CHIPS, SSC_GROUPS, SYNC_CHIPS,
};
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// The width of the channel low-pass of the front end (two-sided), Hz.
const CHANNEL_HZ: f64 = 4.7e6;
/// The carrier bins of the slot search, Hz.
const BIN_HZ: f64 = 2500.0;
/// The most slot peaks kept.
const MAX_PEAKS: usize = 40;
/// A slot peak must exceed the floor this many times.
const PEAK_OVER_FLOOR: f64 = 2.5;
/// The best scrambling code must exceed the other seven of its group this many times.
const CODE_OVER_OTHERS: f64 = 2.5;
/// The carrier steps of the CPICH search, Hz.
const CPICH_STEP_HZ: f64 = 400.0;
/// The most slots of the SSC search.
const SSC_SLOTS: usize = 90;
/// Working samples of a frame.
const FRAME: usize = 2 * FRAME_CHIPS;
/// The clock search window, samples either side.
const CLOCK_SPAN: i64 = 48;

/// One cell found in a capture.
#[derive(Clone, Debug, PartialEq)]
pub struct UmtsFound {
    /// Primary scrambling code (0 to 511).
    pub code: usize,
    /// Code group (0 to 63).
    pub group: usize,
    /// CPICH Ec/Io, dB.
    pub ec_io_db: f64,
    /// CPICH level, dBFS.
    pub rscp_dbfs: f64,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// Frame start in the capture, chips modulo a frame.
    pub frame_offset: usize,
    /// The SCH symbol a is +1 (STTD encoded P-CCPCH).
    pub sttd: Option<bool>,
    /// The PLMN of the MIB.
    pub mib: Option<Mib>,
    /// The cell identity of SIB 3.
    pub cell_id: Option<u32>,
    /// Transport blocks whose CRC is right.
    pub good_blocks: u32,
}

/// What the slot search found at one slot position.
#[derive(Clone, Copy, Debug)]
struct SlotPeak {
    /// Chip of the slot start (0 to 2559).
    pos: usize,
    /// Carrier offset, Hz.
    offset_hz: f64,
}

/// The searcher: the front end and the buffers, kept from capture to capture.
pub struct UmtsSearcher {
    cache: ResamplerCache,
    roll_off: f64,
    mf: Option<Convolver<Complex32>>,
    raw: Vec<Complex32>,
    y: Vec<Complex32>,
    chips: Vec<Complex32>,
    psc: Vec<f32>,
    sscs: Vec<Vec<f32>>,
    c1: Vec<f32>,
}

impl Default for UmtsSearcher {
    fn default() -> Self {
        Self::new(0.22)
    }
}

impl UmtsSearcher {
    /// A searcher whose matched filter is a root raised cosine of `roll_off`.
    pub fn new(roll_off: f64) -> Self {
        let taps = pulse::taps(Shape::Rrc { beta: roll_off }, 2.0, 12.0, Norm::Energy).unwrap_or_else(|_| vec![1.0]);
        UmtsSearcher {
            cache: ResamplerCache::default(),
            roll_off,
            mf: Convolver::new(&taps).ok(),
            raw: Vec::new(),
            y: Vec::new(),
            chips: Vec::new(),
            psc: psc().iter().map(|&v| f32::from(v)).collect(),
            sscs: (1..=16).map(|k| ssc(k).iter().map(|&v| f32::from(v)).collect()).collect(),
            c1: ovsf(256, 1).iter().map(|&v| f32::from(v)).collect(),
        }
    }

    /// The roll-off of the matched filter.
    pub fn roll_off(&self) -> f64 {
        self.roll_off
    }

    /// The cells of `cap`: carriers within `max_offset_hz`, at most `max_cells`, the broadcast channel read when `read_bch`.
    pub fn search(&mut self, cap: &Capture, max_offset_hz: f64, max_cells: usize, read_bch: bool) -> Vec<UmtsFound> {
        if cap.rate <= 0.0 || cap.samples.is_empty() {
            return Vec::new();
        }
        self.front(cap, 0.0, 0.0);
        let (first, best_peak) = self.stages(max_offset_hz, max_cells, read_bch);
        // Stage 7: the sample clock from the strongest slot peak; resample and search again when it is off.
        let Some((pk, phase)) = best_peak else {
            return first;
        };
        let Some((ratio, start)) = self.clock(pk, phase) else {
            return first;
        };
        let frac = start - 2.0 * (start / 2.0).round();
        if (ratio - 1.0).abs() <= 0.5e-6 && frac.abs() <= 0.2 {
            return first;
        }
        self.front(cap, frac, (ratio - 1.0) * 1e6);
        let (second, _) = self.stages(max_offset_hz, max_cells, read_bch);
        let score = |v: &[UmtsFound]| v.iter().map(|c| 100 + c.good_blocks).sum::<u32>();
        if score(&second) > score(&first) {
            second
        } else {
            first
        }
    }

    /// Stage 1: the capture at the working rate from working sample `shift` with the clock corrected by `ppm`, matched
    /// filtered.
    fn front(&mut self, cap: &Capture, shift: f64, ppm: f64) {
        let n_out = (cap.samples.len() as f64 * NATIVE_RATE / cap.rate / (1.0 + ppm * 1e-6)) as usize;
        resample_to(
            &mut self.cache,
            &cap.samples,
            cap.rate,
            cap.dial_offset_hz,
            NATIVE_RATE,
            CHANNEL_HZ.min(0.98 * cap.rate),
            shift * cap.rate / NATIVE_RATE,
            ppm,
            n_out,
            &mut self.raw,
        );
        self.y.clear();
        self.y.resize(self.raw.len(), Complex32::default());
        match self.mf.as_ref() {
            Some(mf) if mf.centred(&self.raw, &mut self.y).is_ok() => {}
            _ => self.y.copy_from_slice(&self.raw),
        }
    }

    /// Stages 2 to 6 on the working samples. Returns the cells and the strongest slot peak with its sample phase.
    fn stages(&mut self, max_offset_hz: f64, max_cells: usize, read_bch: bool) -> (Vec<UmtsFound>, Option<(SlotPeak, usize)>) {
        let mut found = Vec::new();
        let mut best = None;
        for phase in 0..2 {
            self.chips.clear();
            self.chips.extend(self.y.iter().skip(phase).step_by(2).copied());
            let peaks = self.slot_peaks(max_offset_hz);
            if peaks.is_empty() {
                continue;
            }
            best = Some((peaks[0], phase));
            let p_tot = self.chips.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / self.chips.len().max(1) as f64;
            for pk in peaks {
                if found.len() >= max_cells {
                    break;
                }
                let Some(cell) = self.cell(pk, p_tot, read_bch) else { continue };
                if found.iter().any(|f: &UmtsFound| f.code == cell.code) {
                    continue;
                }
                found.push(cell);
            }
            break;
        }
        (found, best)
    }

    /// Stage 2: slot positions by the PSC, coherent over 16 partial sums of 16 chips in carrier bins of 2.5 kHz, folded over
    /// the slot; the strongest positions over 2.5 times the mean, at least 4 chips apart.
    fn slot_peaks(&self, max_offset_hz: f64) -> Vec<SlotPeak> {
        let x = &self.chips;
        if x.len() < SLOT_CHIPS + SYNC_CHIPS {
            return Vec::new();
        }
        let a: Vec<f32> = psc_base().iter().map(|&v| f32::from(v)).collect();
        let signs: Vec<f32> = psc_block_signs().iter().map(|&v| f32::from(v)).collect();
        // The correlation with the sequence a at every chip.
        let n16 = x.len() - 15;
        let mut ya = vec![Complex32::default(); n16];
        for (n, o) in ya.iter_mut().enumerate() {
            let mut acc = Complex32::default();
            for i in 0..16 {
                acc += x[n + i] * a[i];
            }
            *o = acc;
        }
        let half = (max_offset_hz / BIN_HZ).ceil() as i64;
        let bins: Vec<i64> = (-half..=half).collect();
        let rot: Vec<Vec<Complex32>> = bins
            .iter()
            .map(|&d| {
                (0..16)
                    .map(|b| {
                        let ph = -TAU * d as f64 * BIN_HZ * 16.0 * b as f64 / CHIP_RATE;
                        Complex32::from_polar(signs[b], ph as f32)
                    })
                    .collect()
            })
            .collect();
        let positions = x.len() - SYNC_CHIPS + 1;
        let mut acc = vec![0f64; SLOT_CHIPS * bins.len()];
        let mut count = vec![0u32; SLOT_CHIPS];
        for s in 0..positions {
            let slot = s % SLOT_CHIPS;
            count[slot] += 1;
            for (k, r) in rot.iter().enumerate() {
                let mut c = Complex32::default();
                for (b, w) in r.iter().enumerate() {
                    c += ya[s + 16 * b] * w;
                }
                acc[slot * bins.len() + k] += f64::from(c.norm_sqr());
            }
        }
        let mut best: Vec<(f64, usize)> = (0..SLOT_CHIPS)
            .map(|p| {
                let row = &acc[p * bins.len()..(p + 1) * bins.len()];
                let (k, v) = row
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .map_or((0, 0.0), |(k, &v)| (k, v));
                (v / f64::from(count[p].max(1)), k)
            })
            .collect();
        let floor = best.iter().map(|b| b.0).sum::<f64>() / SLOT_CHIPS as f64;
        let mut order: Vec<usize> = (0..SLOT_CHIPS).collect();
        order.sort_by(|&p, &q| best[q].0.total_cmp(&best[p].0));
        let mut peaks: Vec<SlotPeak> = Vec::new();
        for p in order {
            if peaks.len() >= MAX_PEAKS || best[p].0 < PEAK_OVER_FLOOR * floor {
                break;
            }
            let near = peaks.iter().any(|k| {
                let d = k.pos.abs_diff(p);
                d.min(SLOT_CHIPS - d) <= 4
            });
            if !near {
                peaks.push(SlotPeak {
                    pos: p,
                    offset_hz: bins[best[p].1] as f64 * BIN_HZ,
                });
            }
        }
        best.clear();
        peaks
    }

    /// The chips with the carrier `f` Hz taken off (phase 0 at chip 0).
    fn derotated(&self, f: f64) -> Vec<Complex32> {
        self.derotated_range(f, 0, self.chips.len())
    }

    /// Chips `start .. start + len` (cut at the end of the capture) with the carrier `f` Hz taken off, the phase that of chip 0.
    fn derotated_range(&self, f: f64, start: usize, len: usize) -> Vec<Complex32> {
        let end = (start + len).min(self.chips.len());
        let mut out = vec![Complex32::default(); end.saturating_sub(start)];
        let mut nco = Nco::new(CHIP_RATE, -f);
        nco.set_phase_cycles((-f * start as f64 / CHIP_RATE).rem_euclid(1.0));
        nco.mix(&self.chips[start.min(end)..end], &mut out);
        out
    }

    /// Stages 3 to 6 for one slot peak.
    fn cell(&self, pk: SlotPeak, p_tot: f64, read_bch: bool) -> Option<UmtsFound> {
        let n = self.chips.len();
        // Stage 3: the SSC of every slot, then the group and the slot number by the comma-free code.
        let slots = ((n.saturating_sub(pk.pos + SYNC_CHIPS)) / SLOT_CHIPS + 1).min(SSC_SLOTS);
        if slots < 15 {
            return None;
        }
        let pw: Vec<[f64; 16]> = (0..slots)
            .map(|k| {
                let w = self.derotated_range(pk.offset_hz, pk.pos + k * SLOT_CHIPS, SYNC_CHIPS);
                let mut p = [0f64; 16];
                for (j, s) in self.sscs.iter().enumerate() {
                    p[j] = f64::from(corr_real(&w, 0, s).norm_sqr());
                }
                p
            })
            .collect();
        let mut best = (0usize, 0usize, f64::NEG_INFINITY);
        for (g, seq) in SSC_GROUPS.iter().enumerate() {
            for o in 0..15 {
                let v: f64 = pw
                    .iter()
                    .enumerate()
                    .map(|(k, p)| p[usize::from(seq[(k + o) % 15]) - 1])
                    .sum();
                if v > best.2 {
                    best = (g, o, v);
                }
            }
        }
        let (group, o, _) = best;
        let frame_start = pk.pos + SLOT_CHIPS * ((15 - o) % 15);
        if frame_start + FRAME_CHIPS > n {
            return None;
        }
        // Stage 4: the primary scrambling code of the group by the CPICH over one frame, at seven carrier steps of 400 Hz. The
        // sums over sub-blocks of 256 chips are turned by each step and added per slot.
        let mut metrics: Vec<(usize, f64, f64)> = Vec::with_capacity(8);
        let turns: Vec<Vec<Complex32>> = (-3..=3)
            .map(|step| {
                let df = CPICH_STEP_HZ * f64::from(step);
                (0..FRAME_CHIPS / 256)
                    .map(|b| {
                        let t = (frame_start + 256 * b + 128) as f64 / CHIP_RATE;
                        Complex32::from_polar(1.0, (-TAU * df * t) as f32)
                    })
                    .collect()
            })
            .collect();
        let x = self.derotated_range(pk.offset_hz, frame_start, FRAME_CHIPS);
        let mut scr = Vec::with_capacity(FRAME_CHIPS);
        for i in 8 * group..8 * group + 8 {
            DlScrambling::code_into(16 * i, &mut scr);
            let subs: Vec<Complex32> = (0..FRAME_CHIPS / 256)
                .map(|b| despread(&x, 256 * b, &scr, 256 * b, 256, None))
                .collect();
            let mut best_f = (0.0, f64::NEG_INFINITY);
            for (k, turn) in turns.iter().enumerate() {
                let mut p = 0.0;
                for slot in 0..15 {
                    let c: Complex32 = (0..10).map(|b| subs[10 * slot + b] * turn[10 * slot + b]).sum();
                    p += f64::from(c.norm_sqr());
                }
                if p > best_f.1 {
                    best_f = (CPICH_STEP_HZ * (k as f64 - 3.0), p);
                }
            }
            // The CPICH chip is (1 + j) S of power 4 for a unit symbol, and the correlation with conj(S) gives (1 + j) 2 per
            // chip: the power over 15 slots of 2560 chips, over 2560^2 x 15 x 2, is the CPICH power.
            let power = best_f.1 / (SLOT_CHIPS as f64 * SLOT_CHIPS as f64 * 15.0) / 2.0;
            metrics.push((i, pk.offset_hz + best_f.0, power));
        }
        metrics.sort_by(|a, b| b.2.total_cmp(&a.2));
        let (code, offset_hz, cpich) = metrics[0];
        if cpich < CODE_OVER_OTHERS * metrics[1].2 {
            return None;
        }
        // Stage 5: measurements.
        let mut out = UmtsFound {
            code,
            group,
            ec_io_db: 10.0 * (cpich / p_tot.max(1e-30)).max(1e-30).log10(),
            rscp_dbfs: 10.0 * cpich.max(1e-30).log10(),
            offset_hz,
            frame_offset: frame_start % FRAME_CHIPS,
            sttd: None,
            mib: None,
            cell_id: None,
            good_blocks: 0,
        };
        // Stage 6: the SCH symbol a and the broadcast channel, on the chips with the measured carrier taken off.
        let x = self.derotated(offset_hz);
        let scr = DlScrambling::primary(code);
        out.sttd = Some(self.sttd(&x, frame_start, &scr));
        if read_bch {
            self.read_bch(&x, frame_start, &scr, &mut out);
        }
        Some(out)
    }

    /// The sign of the SCH symbol a: the SCH against the CPICH channel estimate of the same slots of the frame from `start`.
    fn sttd(&self, x: &[Complex32], start: usize, scr: &[(i8, i8)]) -> bool {
        let mut acc = 0.0;
        for s in 0..15 {
            let at = start + s * SLOT_CHIPS;
            let h = despread(x, at, scr, s * SLOT_CHIPS, SLOT_CHIPS, None);
            let sch = corr_real(x, at, &self.psc);
            acc += f64::from((sch * h.conj()).re);
        }
        acc > 0.0
    }

    /// Reads the BCH of the cell whose frame starts at chip `start`: every whole TTI of one parity, the other parity only when the
    /// first gave no good block.
    fn read_bch(&self, x: &[Complex32], start: usize, scr: &[(i8, i8)], out: &mut UmtsFound) {
        let n = x.len();
        let tti = 2 * FRAME_CHIPS;
        for parity in 0..2 {
            let mut sibs = SibAssembly::default();
            let mut t = (start + FRAME_CHIPS * parity) % tti;
            while t + tti <= n {
                let mut soft = [vec![0f32; FRAME_BITS], vec![0f32; FRAME_BITS]];
                for (fr, bits) in soft.iter_mut().enumerate() {
                    let f0 = t + fr * FRAME_CHIPS;
                    for slot in 0..15 {
                        let at = f0 + slot * SLOT_CHIPS;
                        let ci = slot * SLOT_CHIPS;
                        // The CPICH estimate h carries (1 - j) against the symbol: z = r conj(h) (1 - j) / 2 is the symbol.
                        let h = despread(x, at, scr, ci, SLOT_CHIPS, None) * Complex32::new(1.0, -1.0);
                        for s in 0..9 {
                            let c0 = SYNC_CHIPS + 256 * s;
                            let r = despread(x, at + c0, scr, ci + c0, 256, Some(&self.c1));
                            let z = r * h.conj();
                            bits[18 * slot + 2 * s] = z.re;
                            bits[18 * slot + 2 * s + 1] = z.im;
                        }
                    }
                }
                if let Some(block) = decode(&soft[0], &soft[1]) {
                    out.good_blocks += 1;
                    if let Some((_, done)) = sibs.push(&block) {
                        for (ty, data) in done {
                            match ty {
                                SIB_MIB => out.mib = parse_mib(&data).or(out.mib.take()),
                                SIB_3 => out.cell_id = parse_sib3(&data).or(out.cell_id),
                                _ => {}
                            }
                        }
                    }
                }
                t += tti;
            }
            if out.good_blocks > 0 {
                break;
            }
        }
    }

    /// Stage 7: the slot timing of the strongest peak in every frame of the working samples, at sample steps within 48 samples
    /// of the expected place. Returns the clock ratio (samples per frame over 76 800) and the fitted start of the first frame,
    /// with at least 3 frames and a ratio within 300 ppm.
    fn clock(&self, pk: SlotPeak, phase: usize) -> Option<(f64, f64)> {
        let mut y = vec![Complex32::default(); self.y.len()];
        Nco::new(NATIVE_RATE, -pk.offset_hz).mix(&self.y, &mut y);
        let n = y.len() as i64;
        let base = (2 * pk.pos + phase) as i64;
        let corr = |t: i64| -> f64 {
            if t < 0 || t + 2 * SYNC_CHIPS as i64 > n {
                return 0.0;
            }
            let mut acc = Complex32::default();
            for (i, &c) in self.psc.iter().enumerate() {
                acc += y[(t + 2 * i as i64) as usize] * c;
            }
            f64::from(acc.norm_sqr())
        };
        let mut pts = Vec::new();
        let mut f = 0i64;
        loop {
            let frame0 = base + FRAME as i64 * f;
            if frame0 - CLOCK_SPAN >= n {
                break;
            }
            let mut window = vec![0f64; (2 * CLOCK_SPAN + 1) as usize];
            let mut slots = 0;
            for j in 0..15 {
                let expect = frame0 + 2 * SLOT_CHIPS as i64 * j;
                if expect - CLOCK_SPAN < 0 || expect + CLOCK_SPAN + 2 * SYNC_CHIPS as i64 > n {
                    continue;
                }
                slots += 1;
                for (k, w) in window.iter_mut().enumerate() {
                    *w += corr(expect + k as i64 - CLOCK_SPAN);
                }
            }
            if slots >= 8 {
                let (k, &peak) = window.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
                let mean = window.iter().sum::<f64>() / window.len() as f64;
                if peak >= 3.0 * mean && k > 0 && k + 1 < window.len() {
                    let (l, r) = (window[k - 1], window[k + 1]);
                    let den = l - 2.0 * peak + r;
                    let frac = if den < 0.0 { 0.5 * (l - r) / den } else { 0.0 };
                    pts.push((f as f64, (frame0 + k as i64 - CLOCK_SPAN) as f64 + frac));
                }
            }
            f += 1;
        }
        if pts.len() < 3 {
            return None;
        }
        let m = pts.len() as f64;
        let mx = pts.iter().map(|p| p.0).sum::<f64>() / m;
        let my = pts.iter().map(|p| p.1).sum::<f64>() / m;
        let sxx: f64 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
        let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
        let slope = sxy / sxx;
        let ratio = slope / FRAME as f64;
        ((ratio - 1.0).abs() <= 300e-6).then_some((ratio, my - slope * mx))
    }
}

/// The correlation of `x` from `at` with a real code.
fn corr_real(x: &[Complex32], at: usize, code: &[f32]) -> Complex32 {
    let mut acc = Complex32::default();
    for (v, &c) in x.iter().skip(at).zip(code) {
        acc += v * c;
    }
    acc
}

/// The sum over `len` chips of `x` from `at` times `conj(S)` of the scrambling code from code chip `ci`, times the real
/// channelisation code `ch` (chip `k` of it for the `k`-th chip) when given.
fn despread(x: &[Complex32], at: usize, scr: &[(i8, i8)], ci: usize, len: usize, ch: Option<&[f32]>) -> Complex32 {
    let mut acc = Complex32::default();
    for k in 0..len {
        let Some(&v) = x.get(at + k) else { break };
        let (si, sq) = scr[(ci + k) % FRAME_CHIPS];
        let (si, sq) = (f32::from(si), f32::from(sq));
        let mut d = Complex32::new(v.re * si + v.im * sq, v.im * si - v.re * sq);
        if let Some(c) = ch {
            d *= c[k % c.len()];
        }
        acc += d;
    }
    acc
}
