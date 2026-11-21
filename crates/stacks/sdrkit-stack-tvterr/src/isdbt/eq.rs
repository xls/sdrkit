//! The ISDB-T equaliser, one symbol at a time: the channel of the synchronous segments from their scattered pilots, the
//! differential detection of the DQPSK segments, and the noise variance of every cell.
//!
//! Standard: ARIB STD-B31 clause 3.13.

use super::layout::{Band, Layout};
use super::sync::Symbol;
use num_complex::Complex32;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_modem::ofdm::{
    continual_mean, delay_window, equalise, DelayWindowRule, PilotTimeConfig, PilotTimeInterp,
    WindowedGridInterp,
};
use sdrkit_graph::SlicePool;
use std::sync::Arc;

/// A plot of the equaliser: points of x and y.
pub type Plot = Arc<[[f32; 2]]>;

/// Equalised cells of one symbol, in segment-number order and data-position order (before frequency de-interleaving).
#[derive(Clone, Debug, Default)]
pub struct Cells {
    /// Signal time, s.
    pub time: f64,
    /// Index in the frame.
    pub frame_pos: usize,
    /// The symbol's epoch (a change restarts the stages after).
    pub epoch: u32,
    /// The TMCC in force.
    pub tmcc: super::tmcc::Tmcc,
    /// Mode.
    pub mode: u8,
    /// The band.
    pub band: Band,
    /// Cells and noise variances.
    pub cells: Arc<[(Complex32, f32)]>,
}

/// What the equaliser measures.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EqState {
    /// SNR from the pilots, dB.
    pub snr_db: f64,
    /// Carriers erased as hit by an interferer.
    pub erased: usize,
    /// Equalised data cells of the last symbol (subsampled, at most 1500).
    pub constellation: Plot,
    /// Equalised scattered pilots of the last symbol.
    pub pilots: Plot,
    /// Channel magnitude in dB per carrier (at most 2048 points, [carrier, dB]).
    pub channel_db: Plot,
    /// Channel phase per carrier ([carrier, rad]).
    pub channel_phase: Plot,
    /// Impulse response from the channel ([lag in samples, dB]).
    pub impulse: Plot,
}

/// The working buffers of the plots, and the pools they are shared from.
#[derive(Default)]
struct Views {
    points: Vec<[f32; 2]>,
    lags: Vec<i32>,
    carriers: Vec<usize>,
    power: Vec<f32>,
    pool: SlicePool<[f32; 2]>,
}

impl Views {
    /// The points written so far as a shared plot.
    fn share(&mut self) -> Plot {
        let p = &self.points;
        self.pool.exact(p.len(), |v| v.copy_from_slice(p))
    }
}

/// Symbols the equaliser works behind the newest: the channel of a grid point is interpolated in time between the pilots
/// before and after the symbol.
pub const DELAY: usize = 8;

/// The soft-bit noise is the time interpolator's noise times this.
const NOISE_CALIBRATION: f64 = 1.0;

/// Without scattered pilots (every segment differential) the noise from the continual pilots is taken times this.
const DIFFERENTIAL_NOISE: f32 = 1.2;

/// The delay window is measured at this output symbol and every [`WINDOW_EVERY`] after.
const WINDOW_FIRST: usize = 7;
const WINDOW_EVERY: usize = 16;

/// Taps either side of the frequency interpolator over the grid of every third carrier.
const FREQ_HALF: usize = 4;

/// A symbol waiting for the pilots after it.
struct Waiting {
    symbol: Symbol,
    index: i64,
    /// The common phase taken off its pilots (radians).
    phase: f32,
    /// The continual pilot above each synchronous run, with the phase taken off.
    edges: [Complex32; 7],
}

/// The equaliser. Each symbol waits [`DELAY`] symbols so that the channel of every scattered-pilot grid point is interpolated
/// in time between the pilots before and after it (the shared [`PilotTimeInterp`]); the carrier above each synchronous run is a
/// continual pilot, its channel the mean of the symbols before, at and after. The common phase of each symbol (phase noise) is
/// followed from symbol to symbol on the carriers present in every symbol and taken off before the pilots enter the time
/// interpolation, then put back on the channel of the symbol when it leaves; a slow phase slope (timing drift) is left to the
/// time interpolation. Between grid carriers the shared [`WindowedGridInterp`] fills in; at the 8th symbol let out and every
/// 16 after, the impulse response of the full-window estimate gives the delay window ([`delay_window`]) that the
/// interpolator then passes. The noise of a synchronous cell comes from the interpolator's pilot statistics; a differential
/// cell is compared with the previous symbol that went through the equaliser, its noise from the continual pilots against their
/// previous values (times 1.2 when no segment has scattered pilots).
pub struct Equaliser {
    epoch: u32,
    layout: Option<Layout>,
    runs: ([(usize, usize); 7], usize),
    /// The first grid point of each run.
    run_point: [usize; 7],
    time: Option<PilotTimeInterp>,
    freq: WindowedGridInterp,
    /// The inverse FFT of the layout's size and the impulse response it makes, for the delay window.
    ifft: Option<Fft>,
    ir: Vec<Complex32>,
    window_rule: DelayWindowRule,
    window_scratch: Vec<f64>,
    /// Symbols let out since the layout began.
    released: usize,
    /// The carriers of the previous symbol as received.
    last_in: Vec<Complex32>,
    /// The common phase taken off the pilots (radians).
    phase: f32,
    /// Per grid point: the pilot change over four symbols, smoothed, and the pilots seen.
    noise_g: Vec<f32>,
    last_raw: Vec<Complex32>,
    seen: Vec<u8>,
    waiting: std::collections::VecDeque<Waiting>,
    index: i64,
    pil: Vec<(usize, Complex32)>,
    grid: Vec<Complex32>,
    run_grid: Vec<Complex32>,
    prev_edges: Option<[Complex32; 7]>,
    est: Vec<Complex32>,
    have_est: bool,
    prev: Vec<Complex32>,
    have_prev: bool,
    syms: usize,
    sigma2: f32,
    sigma2_d: f32,
    band: Vec<(Complex32, f32)>,
    zf: Vec<Complex32>,
    csi: Vec<f32>,
    cells: SlicePool<(Complex32, f32)>,
    views: Views,
    /// What was measured last.
    pub state: EqState,
}

impl Default for Equaliser {
    fn default() -> Self {
        Equaliser {
            epoch: u32::MAX,
            layout: None,
            runs: ([(0, 0); 7], 0),
            run_point: [0; 7],
            time: None,
            freq: WindowedGridInterp::new(3, FREQ_HALF).unwrap_or_else(|_| {
                unreachable!("a grid of every third carrier with 4 taps a side is valid")
            }),
            ifft: None,
            ir: Vec::new(),
            window_rule: DelayWindowRule::default(),
            window_scratch: Vec::new(),
            released: 0,
            last_in: Vec::new(),
            phase: 0.0,
            noise_g: Vec::new(),
            last_raw: Vec::new(),
            seen: Vec::new(),
            waiting: std::collections::VecDeque::with_capacity(DELAY + 1),
            index: 0,
            pil: Vec::new(),
            grid: Vec::new(),
            run_grid: Vec::new(),
            prev_edges: None,
            est: Vec::new(),
            have_est: false,
            prev: Vec::new(),
            have_prev: false,
            syms: 0,
            sigma2: 1e-3,
            sigma2_d: 1e-3,
            band: Vec::new(),
            zf: Vec::new(),
            csi: Vec::new(),
            cells: SlicePool::default(),
            views: Views::default(),
            state: EqState::default(),
        }
    }
}

/// `e^(j phase)`.
fn turn(phase: f32) -> Complex32 {
    Complex32::from_polar(1.0, phase)
}

impl Equaliser {
    /// Starts again for the layout of `s`.
    fn restart(&mut self, s: &Symbol) {
        let nc = s.carriers.len();
        let lay = Layout::new(s.mode, &s.tmcc.current, s.band);
        self.runs = sync_runs(&lay);
        let runs = &self.runs.0[..self.runs.1];
        let mut points = 0;
        for (k, &(a, b)) in runs.iter().enumerate() {
            self.run_point[k] = points;
            points += (b - a) / 3;
        }
        self.time = (points > 0)
            .then(|| PilotTimeInterp::new(PilotTimeConfig::new(points)).ok())
            .flatten();
        // No window until the first impulse response of this layout.
        let _ = self
            .freq
            .set_window(lay.n_fft, None, self.window_rule.margin);
        self.ifft = (points > 0 && nc <= lay.n_fft).then(|| Fft::inverse(lay.n_fft));
        self.ir = vec![Complex32::new(0.0, 0.0); lay.n_fft];
        self.released = 0;
        self.layout = Some(lay);
        self.epoch = s.epoch;
        self.last_in = vec![Complex32::new(0.0, 0.0); nc];
        self.phase = 0.0;
        self.noise_g = vec![0.0; points];
        self.last_raw = vec![Complex32::new(0.0, 0.0); points];
        self.seen = vec![0; points];
        self.grid = vec![Complex32::new(0.0, 0.0); points];
        self.prev_edges = None;
        self.est = vec![Complex32::new(0.0, 0.0); nc];
        self.have_est = false;
        self.prev = vec![Complex32::new(0.0, 0.0); nc];
        self.have_prev = false;
        self.syms = 0;
        self.band = vec![(Complex32::new(0.0, 0.0), f32::INFINITY); nc];
        self.zf = vec![Complex32::new(0.0, 0.0); nc];
        self.csi = vec![0.0; nc];
        self.index = 0;
    }

    /// Takes one symbol. The symbols [`DELAY`] behind it leave through `out`, each with the measurements and scope series when
    /// they were filled for it (every 51st symbol of a frame). A change of epoch first lets out the symbols still waiting.
    pub fn process(&mut self, s: &Symbol, out: &mut impl FnMut(Cells, Option<&EqState>)) {
        if s.epoch != self.epoch || self.layout.is_none() {
            self.flush(out);
            self.restart(s);
        }
        let lay = self
            .layout
            .as_ref()
            .expect("the layout is built above when it is missing");
        let x = &s.carriers;
        let fp = s.frame_pos;
        let runs = &self.runs.0[..self.runs.1];
        let ph = 3 * (fp % 4);
        // The common phase step from the previous symbol, from the carriers present in every symbol (TMCC and AC, DBPSK, and the
        // continual pilots): squared products take off the modulation. Summed, it is the common phase of this symbol.
        if self.index > 0 {
            let mut z2 = Complex32::new(0.0, 0.0);
            let mut add = |c: usize| {
                let d = x[c] * self.last_in[c].conj();
                z2 += d * d / (d.norm() + 1e-20);
            };
            for seg in 0..lay.ns() {
                let t = &lay.tables[seg];
                if lay.diff[seg] {
                    add(lay.band(seg, 0));
                }
                for &k in t.tmcc.iter().chain(&t.ac1).chain(&t.ac2) {
                    add(lay.band(seg, k as usize));
                }
            }
            add(x.len() - 1);
            self.phase = (self.phase + z2.arg() / 2.0 + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
        }
        self.last_in.copy_from_slice(x);
        let phase = self.phase;
        self.pil.clear();
        for (k, &(a, b)) in runs.iter().enumerate() {
            for c in (a + ph..b).step_by(12) {
                let p = x[c] / lay.pilot(c) * turn(-phase);
                let g = self.run_point[k] + (c - a) / 3;
                if self.seen[g] > 0 {
                    let e = (p - self.last_raw[g]).norm_sqr();
                    self.noise_g[g] = 0.8 * self.noise_g[g] + 0.2 * e;
                }
                self.seen[g] = self.seen[g].saturating_add(1);
                self.last_raw[g] = p;
                self.pil.push((g, p));
            }
        }
        if let Some(t) = &mut self.time {
            // The symbol index only grows.
            let _ = t.push_symbol(self.index, &self.pil);
        }
        let mut edges = [Complex32::new(0.0, 0.0); 7];
        for (e, &(_, b)) in edges.iter_mut().zip(runs) {
            *e = x[b] / lay.pilot(b) * turn(-phase);
        }
        self.waiting.push_back(Waiting {
            symbol: s.clone(),
            index: self.index,
            phase,
            edges,
        });
        self.index += 1;
        while self.waiting.len() > DELAY {
            self.release(out);
        }
    }

    /// Lets out every symbol still waiting (the end of the input or a new epoch).
    pub fn flush(&mut self, out: &mut impl FnMut(Cells, Option<&EqState>)) {
        while !self.waiting.is_empty() {
            self.release(out);
        }
    }

    /// Equalises the oldest waiting symbol.
    fn release(&mut self, out: &mut impl FnMut(Cells, Option<&EqState>)) {
        let Some(w) = self.waiting.pop_front() else {
            return;
        };
        let next_edges = self.waiting.front().map_or(w.edges, |n| n.edges);
        let prev_edges = self.prev_edges.unwrap_or(w.edges);
        self.prev_edges = Some(w.edges);
        let lay = self
            .layout
            .as_ref()
            .expect("a symbol waits only once the layout is built");
        let s = &w.symbol;
        let x = &s.carriers;
        let nc = x.len();
        let fp = s.frame_pos;
        let ls = lay.ls;
        let runs = &self.runs.0[..self.runs.1];
        // The channel of the synchronous runs: the grid in time, the edge pilot from its neighbours, then across frequency.
        let mut ready = !runs.is_empty()
            && self
                .time
                .as_ref()
                .is_some_and(|t| t.channel_at(w.index, &mut self.grid));
        // The delay window, from the impulse response of the full-window estimate.
        let measure = self.released >= WINDOW_FIRST
            && (self.released - WINDOW_FIRST).is_multiple_of(WINDOW_EVERY);
        self.released += 1;
        if ready && measure {
            if let Some(ifft) = &mut self.ifft {
                self.ir.fill(Complex32::new(0.0, 0.0));
                let mut ok = true;
                for (k, &(a, b)) in runs.iter().enumerate() {
                    let g0 = self.run_point[k];
                    let m = (b - a) / 3;
                    self.run_grid.clear();
                    self.run_grid.extend_from_slice(&self.grid[g0..g0 + m]);
                    self.run_grid
                        .push(continual_mean(prev_edges[k], w.edges[k], next_edges[k]));
                    ok &= self
                        .freq
                        .full()
                        .fill(&self.run_grid, &mut self.ir[a..=b])
                        .is_ok();
                }
                if ok {
                    ifft.process(&mut self.ir);
                    let window =
                        delay_window(&self.ir, &self.window_rule, &mut self.window_scratch);
                    let _ = self
                        .freq
                        .set_window(lay.n_fft, window, self.window_rule.margin);
                }
            }
        }
        if ready {
            for (k, &(a, b)) in runs.iter().enumerate() {
                let g0 = self.run_point[k];
                let m = (b - a) / 3;
                self.run_grid.clear();
                self.run_grid.extend_from_slice(&self.grid[g0..g0 + m]);
                self.run_grid
                    .push(continual_mean(prev_edges[k], w.edges[k], next_edges[k]));
                if self
                    .freq
                    .fill(&self.run_grid, &mut self.est[a..=b])
                    .is_err()
                {
                    ready = false;
                }
                let back = turn(w.phase);
                for v in &mut self.est[a..=b] {
                    *v *= back;
                }
            }
        }
        self.have_est = ready;
        if let Some(n) = self.time.as_ref().and_then(|t| t.soft_noise()) {
            self.sigma2 = (n * NOISE_CALIBRATION) as f32;
        }
        // Narrowband interferers: a pilot whose change stands far above the others erases the carriers around it.
        let (mut gsum, mut gn) = (0f64, 0usize);
        for (g, &v) in self.noise_g.iter().enumerate() {
            if self.seen[g] > 1 {
                gsum += v as f64;
                gn += 1;
            }
        }
        let gmean = if gn > 0 {
            (gsum / gn as f64) as f32
        } else {
            0.0
        };
        // Differential segments: the common phase step from the continual, TMCC and AC carriers.
        let mut z2 = Complex32::new(0.0, 0.0);
        let (mut cpn, mut cpk) = (0f32, 0usize);
        if self.have_prev {
            let mut add = |c: usize, cp: bool, z2: &mut Complex32| {
                let d = x[c] * self.prev[c].conj();
                *z2 += d * d / (d.norm() + 1e-20);
                if cp {
                    cpn += (x[c] - self.prev[c]).norm_sqr();
                    cpk += 1;
                }
            };
            for seg in 0..lay.ns() {
                if !lay.diff[seg] {
                    continue;
                }
                add(lay.band(seg, 0), true, &mut z2);
                let t = &lay.tables[seg];
                for &k in t.tmcc.iter().chain(&t.ac1).chain(&t.ac2) {
                    add(lay.band(seg, k as usize), false, &mut z2);
                }
            }
            add(nc - 1, true, &mut z2);
        }
        let phi_d = z2.arg() / 2.0;
        let rot_d = Complex32::new(phi_d.cos(), -phi_d.sin());
        if cpk > 0 {
            let only = if runs.is_empty() {
                DIFFERENTIAL_NOISE
            } else {
                1.0
            };
            let s2 = only * cpn / cpk as f32 / 2.0;
            self.sigma2_d = if self.syms < 8 {
                s2
            } else {
                0.9 * self.sigma2_d + 0.1 * s2
            };
        }
        // Equalise every data carrier into the band: zero forcing on the synchronous carriers, with the inverse of each cell's
        // noise variance as its weight.
        let zf_ok = self.have_est
            && equalise(
                x,
                &self.est,
                self.sigma2.max(1e-20),
                &mut self.zf,
                &mut self.csi,
            )
            .is_ok();
        let mut erased = 0usize;
        for seg in 0..lay.ns() {
            let base = lay.pos(seg) * ls;
            let list = &lay.data[seg * 4 + if lay.diff[seg] { 0 } else { fp % 4 }];
            if lay.diff[seg] {
                for &k in list {
                    let c = base + k as usize;
                    let pv = self.prev[c];
                    let e = pv.norm_sqr();
                    self.band[c] = if self.have_prev && e > 1e-20 {
                        (x[c] * pv.conj() * rot_d / e, 2.0 * self.sigma2_d / e)
                    } else {
                        (Complex32::new(0.0, 0.0), f32::INFINITY)
                    };
                }
            } else {
                for &k in list {
                    let c = base + k as usize;
                    let hit = runs
                        .iter()
                        .position(|&(a, b)| c >= a && c < b)
                        .map(|r| self.run_point[r] + (c - runs[r].0) / 3)
                        .is_some_and(|g| gmean > 0.0 && self.noise_g[g] > 30.0 * gmean);
                    if hit {
                        erased += 1;
                    }
                    self.band[c] = if zf_ok && self.csi[c] > 0.0 && !hit {
                        (self.zf[c], 1.0 / self.csi[c])
                    } else {
                        (Complex32::new(0.0, 0.0), f32::INFINITY)
                    };
                }
            }
        }
        let band = &self.band;
        let cells = self.cells.exact(lay.ns() * lay.d, |v| {
            v.fill((Complex32::new(0.0, 0.0), f32::INFINITY));
            lay.take(band, fp, v);
        });
        // SNR from the pilots (or, without scattered pilots, all carriers against the continual pilots' noise).
        let snr = if !runs.is_empty() && self.have_est {
            let mut ph2 = 0f64;
            let mut n = 0usize;
            for &(a, b) in runs {
                for c in (a..b).step_by(3) {
                    ph2 += self.est[c].norm_sqr() as f64;
                    n += 1;
                }
            }
            10.0 * ((ph2 / n.max(1) as f64) / self.sigma2.max(1e-12) as f64).log10()
        } else {
            let p: f64 = x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / nc as f64;
            10.0 * (p / self.sigma2_d.max(1e-12) as f64).log10()
        };
        self.state.snr_db = snr;
        self.state.erased = erased;
        let view = fp % 51 == 0;
        if view {
            self.view(x, fp);
        }
        self.prev.copy_from_slice(x);
        self.have_prev = true;
        self.syms += 1;
        out(
            Cells {
                time: s.time,
                frame_pos: fp,
                epoch: s.epoch,
                tmcc: s.tmcc,
                mode: s.mode,
                band: s.band,
                cells,
            },
            view.then_some(&self.state),
        );
    }

    fn view(&mut self, x: &[Complex32], fp: usize) {
        let runs = &self.runs.0[..self.runs.1];
        let lay = self
            .layout
            .as_ref()
            .expect("process builds the layout before it calls view");
        let st = &mut self.state;
        let v = &mut self.views;
        v.points.clear();
        let step = (lay.ns() * lay.d / 1500).max(1);
        let mut k = 0usize;
        for seg in 0..lay.ns() {
            let base = lay.pos(seg) * lay.ls;
            for &c in &lay.data[seg * 4 + if lay.diff[seg] { 0 } else { fp % 4 }] {
                if k.is_multiple_of(step) {
                    let (z, n) = self.band[base + c as usize];
                    if n.is_finite() {
                        v.points.push([z.re, z.im]);
                    }
                }
                k += 1;
            }
        }
        st.constellation = v.share();
        v.points.clear();
        let ph = 3 * (fp % 4);
        for &(a, b) in runs {
            for c in (a + ph..b).step_by(12) {
                if self.est[c].norm_sqr() > 0.0 {
                    let z = x[c] / self.est[c] / lay.pilot(c);
                    v.points.push([z.re, z.im]);
                }
            }
        }
        st.pilots = v.share();
        let step = (x.len() / 2048).max(1);
        v.points.clear();
        for c in (0..x.len()).step_by(step) {
            let h = self.est[c];
            if h.norm_sqr() > 0.0 {
                v.points
                    .push([c as f32, 10.0 * h.norm_sqr().max(1e-12).log10()]);
            }
        }
        st.channel_db = v.share();
        v.points.clear();
        for c in (0..x.len()).step_by(step) {
            let h = self.est[c];
            if h.norm_sqr() > 0.0 {
                v.points.push([c as f32, h.arg()]);
            }
        }
        st.channel_phase = v.share();
        // Impulse response: an inverse DFT of the estimate over the synchronous carriers, lags −N/16 .. guard-ish.
        v.points.clear();
        let n = lay.n_fft;
        if runs.is_empty() {
            st.impulse = v.share();
            return;
        }
        v.lags.clear();
        v.lags
            .extend((-(n as i32) / 16..(n as i32) / 8).step_by((n / 512).max(1)));
        v.carriers.clear();
        v.carriers
            .extend(runs.iter().flat_map(|&(a, b)| (a..b).step_by(3)));
        v.power.clear();
        let mut peak = 0f32;
        for &lag in &v.lags {
            let mut acc = Complex32::new(0.0, 0.0);
            for &c in &v.carriers {
                let ang = std::f32::consts::TAU * c as f32 * lag as f32 / n as f32;
                acc += self.est[c] * Complex32::from_polar(1.0, ang);
            }
            let p = acc.norm_sqr();
            peak = peak.max(p);
            v.power.push(p);
        }
        for (lag, p) in v.lags.iter().zip(&v.power) {
            v.points
                .push([*lag as f32, 10.0 * (p / peak.max(1e-20)).max(1e-9).log10()]);
        }
        st.impulse = v.share();
    }
}

/// The runs of adjacent synchronous segments, by frequency position, as band carrier ranges (at most 7 among 13 positions).
fn sync_runs(lay: &Layout) -> ([(usize, usize); 7], usize) {
    let ns = lay.ns();
    let mut sync_pos = [false; 13];
    for (seg, &d) in lay.diff.iter().enumerate().take(ns) {
        sync_pos[lay.pos(seg)] = !d;
    }
    let mut runs = [(0usize, 0usize); 7];
    let mut nruns = 0;
    let mut p = 0;
    while p < ns {
        if sync_pos[p] {
            let st = p;
            while p < ns && sync_pos[p] {
                p += 1;
            }
            runs[nruns] = (st * lay.ls, p * lay.ls);
            nruns += 1;
        } else {
            p += 1;
        }
    }
    (runs, nruns)
}
