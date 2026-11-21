//! `dvbt.equaliser`: the channel estimate from the pilots, the zero-forcing equaliser with the noise of each cell, and the
//! measurements and plots of the equalised signal.
//!
//! Standard: ETSI EN 300 744 clause 4.5.

use crate::items::{Carriers, DataCells};
use crate::params::{Mode, TpsParams};
use crate::settings::{INTERFERER_ERASE, TIME_INTERP};
use crate::tables::{CarrierKind, Layout};
use crate::typed::EqFields;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::sample::Sample;
use sdrkit_algo_modem::Constellation;
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::{field, Published, SeriesDef};
use sdrkit_core::results::SeriesId;
use sdrkit_core::{
    Complex32, ConfigError, Results, ResultsDecl, Setting, SettingError, SettingId, SettingSet,
    Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kept, Kind, Port, RateFrom,
    SlicePool, Timed,
};
use std::collections::VecDeque;
use std::f64::consts::PI;
use std::sync::Arc;

/// Flat noise weighting.
pub const FLAT_CSI: Setting<field::Bool> =
    Setting::new("flat_csi", "Flat noise weighting", "", false);
/// The full delay window always.
pub const FULL_WINDOW: Setting<field::Bool> =
    Setting::new("full_window", "Full delay window", "", false);

sdrkit_core::settings! {
    /// The settings of `dvbt.equaliser`.
    pub enum EqualiserSetting {
        /// Time interpolation of the pilots.
        TimeInterp = TIME_INTERP,
        /// Narrowband interferer weighting.
        InterfererErase = INTERFERER_ERASE,
        /// Flat noise weighting.
        FlatCsi = FLAT_CSI,
        /// The full delay window always.
        FullWindow = FULL_WINDOW,
    }
}

/// A plot of the equaliser.
const fn plot(
    key: &'static str,
    label: &'static str,
    x_unit: &'static str,
    y_unit: &'static str,
) -> SeriesDef {
    SeriesDef {
        key,
        label,
        x_unit,
        y_unit,
    }
}

/// The plots of the equaliser, in order.
pub const SERIES: [SeriesDef; 6] = [
    plot("constellation", "Constellation", "", ""),
    plot("pilots", "Pilots", "", ""),
    plot("tps", "TPS", "", ""),
    plot("channel_db", "Channel magnitude", "carrier", "dB"),
    plot("channel_phase", "Channel phase", "carrier", "rad"),
    plot("impulse_db", "Impulse response", "sample", "dB"),
];

fn eq_results() -> Published {
    let decl = SERIES.iter().fold(
        ResultsDecl::new().with_fields(EqFields::SHAPE.fields),
        |d, s| d.with_series(s),
    );
    Published::from_decl(std::sync::Arc::new(decl))
}

/// Symbols the equaliser works behind the newest (time interpolation).
pub const DELAY: usize = 8;

#[derive(Clone, Debug, Default)]
struct GridPoint {
    value: Complex32,
    count: u32,
    last: i64,
    hist: VecDeque<(i64, Complex32)>,
}

struct Delayed {
    cells: Arc<[Complex32]>,
    t: i64,
    l: usize,
    frame: u8,
    tps_word: Option<u128>,
    params: Option<TpsParams>,
    erased: bool,
    fresh: bool,
    time: f64,
    rate: f64,
}

/// The weight of a new symbol in the average of each continual pilot.
const REF_WEIGHT: f32 = 0.15;
/// The deviation of a pilot from its average, over the noise of the pilot: the average has noise of its own.
const REF_NOISE: f64 = 1.0 + REF_WEIGHT as f64 / (2.0 - REF_WEIGHT as f64);
/// The margin on the noise that the de-mapper works with.
const NOISE_MARGIN: f64 = 1.2;
/// The least noise against the power of the signal (60 dB under it).
const NOISE_FLOOR: f64 = 1e-6;

/// Measurements the equaliser publishes.
#[derive(Clone, Debug, Default)]
pub struct EqStats {
    /// The power of a data cell over the noise of a carrier, from the continual pilots, dB.
    pub snr_db: f32,
    /// Decision-directed MER of the data cells, dB.
    pub mer_db: f32,
    /// The grid is filled: equalised data flows.
    pub data_valid: bool,
    /// Noise variance of the carriers.
    pub noise: f32,
    /// Time-averaging gain.
    pub alpha: f32,
    /// Delay window centre and half width, samples.
    pub window: (f32, f32),
    /// Measured delay spread, samples (0 when not measured).
    pub spread: f32,
    /// Cells erased as narrowband interference in the last symbol.
    pub erased_cells: u32,
}

/// `dvbt.equaliser`.
pub struct DvbtEqualiser {
    link: Option<Arc<crate::items::Link>>,
    lay: Option<Layout>,
    mode: Option<Mode>,
    params: Option<TpsParams>,
    guard: usize,
    t: i64,
    since: i64,
    ref_p: Vec<Complex32>,
    have_ref: bool,
    phase_total: f64,
    slope_total: f64,
    noise: f64,
    d1: f64,
    d2: f64,
    dstats: u32,
    prev1: Vec<Complex32>,
    prev2: Vec<Complex32>,
    alpha: f64,
    grid: Vec<GridPoint>,
    delay: VecDeque<Delayed>,
    weights: [Vec<f32>; 3],
    saved_weights: [Vec<f32>; 3],
    spread_pw: Vec<(i64, f64)>,
    spread_vals: Vec<f64>,
    taps: usize,
    centre: f64,
    half: f64,
    spread_due: i64,
    measured_spread: f64,
    p_avg: Vec<f32>,
    n_avg: Vec<f32>,
    p_count: Vec<u32>,
    h: Vec<Complex32>,
    hg: Vec<Complex32>,
    lg: Vec<Complex32>,
    cur: Vec<Complex32>,
    ir: Vec<Complex32>,
    phasor_key: Option<(u64, usize, usize)>,
    phasor_in: Vec<Complex32>,
    phasor_out: Vec<Complex32>,
    z_all: Vec<Complex32>,
    var_all: Vec<f32>,
    pool_c: SlicePool<Complex32>,
    pool_n: SlicePool<f32>,
    pool_r: SlicePool<Complex32>,
    ifft: Option<Fft>,
    fresh_out: bool,
    mapper: Option<Constellation>,
    stats: EqStats,
    results: Published,
    plot_buf: Vec<Complex32>,
    plot_t: f64,
    plot_syms: u32,
    mer_acc: (f64, f64),
    tps_pts: VecDeque<[f32; 2]>,
    restart_pending_move: bool,
    /// Time interpolation of the pilots (off: the newest pilot per grid point).
    pub time_interp: bool,
    /// Narrowband interferer weighting.
    pub interferer: bool,
    /// Flat noise weighting (no channel-state information), a test setting.
    pub flat_csi: bool,
    /// The full delay window always, a test setting.
    pub full_window: bool,
}

impl Default for DvbtEqualiser {
    fn default() -> Self {
        Self::new()
    }
}

impl DvbtEqualiser {
    /// A new equaliser.
    pub fn new() -> Self {
        DvbtEqualiser {
            link: None,
            lay: None,
            mode: None,
            params: None,
            guard: 0,
            t: 0,
            since: 0,
            ref_p: Vec::new(),
            have_ref: false,
            phase_total: 0.0,
            slope_total: 0.0,
            noise: 0.0,
            d1: 0.0,
            d2: 0.0,
            dstats: 0,
            prev1: Vec::new(),
            prev2: Vec::new(),
            alpha: 1.0,
            grid: Vec::new(),
            delay: VecDeque::new(),
            weights: Default::default(),
            saved_weights: Default::default(),
            spread_pw: Vec::new(),
            spread_vals: Vec::new(),
            taps: 0,
            centre: 0.0,
            half: 0.0,
            spread_due: 8,
            measured_spread: 0.0,
            p_avg: Vec::new(),
            n_avg: Vec::new(),
            p_count: Vec::new(),
            h: Vec::new(),
            hg: Vec::new(),
            lg: Vec::new(),
            cur: Vec::new(),
            ir: Vec::new(),
            phasor_key: None,
            phasor_in: Vec::new(),
            phasor_out: Vec::new(),
            z_all: Vec::new(),
            var_all: Vec::new(),
            pool_c: SlicePool::default(),
            pool_n: SlicePool::default(),
            pool_r: SlicePool::default(),
            ifft: None,
            fresh_out: true,
            mapper: None,
            stats: EqStats::default(),
            results: eq_results(),
            plot_buf: Vec::new(),
            plot_t: f64::NEG_INFINITY,
            plot_syms: 0,
            mer_acc: (0.0, 0.0),
            tps_pts: VecDeque::new(),
            restart_pending_move: false,
            time_interp: true,
            interferer: true,
            flat_csi: false,
            full_window: false,
        }
    }

    /// An equaliser that publishes into `link`'s telemetry.
    pub fn with_link(link: Arc<crate::items::Link>) -> Self {
        DvbtEqualiser {
            link: Some(link),
            ..Self::new()
        }
    }

    /// The measurements.
    pub fn stats(&self) -> &EqStats {
        &self.stats
    }

    /// Restarts the estimate (a new acquisition, a resynchronisation, new parameters).
    fn restart(&mut self, mode: Mode, guard: usize) {
        if self.mode != Some(mode) {
            self.lay = Some(Layout::new(mode));
            self.mode = Some(mode);
            self.ifft = Some(Fft::inverse(mode.fft()));
        }
        let (lk, lc) = {
            let lay = self
                .lay
                .as_ref()
                .expect("a restart builds the layout before a symbol is handled");
            (lay.k, lay.continual.len())
        };
        let m = (lk - 1) / 3 + 1;
        self.grid = vec![GridPoint::default(); m];
        self.ref_p = vec![Complex32::default(); lc];
        self.have_ref = false;
        self.phase_total = 0.0;
        self.slope_total = 0.0;
        self.noise = 0.0;
        self.d1 = 0.0;
        self.d2 = 0.0;
        self.dstats = 0;
        self.prev1.clear();
        self.prev2.clear();
        self.alpha = 1.0;
        self.since = 0;
        self.guard = guard;
        let n = mode.fft() as f64;
        let b = (guard / 4).min(24) as f64;
        self.set_window(b + guard as f64 / 2.0, n / 6.0);
        self.spread_due = 8;
        self.measured_spread = 0.0;
        self.p_avg = vec![0.0; lk];
        self.n_avg = vec![0.0; lk];
        self.p_count = vec![0; lk];
        self.fresh_out = true;
    }

    /// The delay window: centre and half width in samples. The interpolation filter follows.
    fn set_window(&mut self, centre: f64, half: f64) {
        let n = self.mode.map_or(2048, |m| m.fft()) as f64;
        let half = half.min(n / 6.0);
        self.centre = centre;
        self.half = half;
        let fc = half / n; // cycles per carrier (two-sided width 2 half / N)
        let lobe = 1.0 / (2.0 * fc); // carriers to the first zero
        let j = ((2.5 * lobe / 3.0).round() as usize).clamp(4, 24);
        self.taps = j;
        for r in 0..3 {
            let w = &mut self.weights[r];
            w.clear();
            for jj in -(j as i64)..=(j as i64) {
                let x = r as f64 - 3.0 * jj as f64;
                let arg = 2.0 * fc * x;
                let sinc = if arg.abs() < 1e-12 {
                    1.0
                } else {
                    (PI * arg).sin() / (PI * arg)
                };
                let span = 3.0 * (j as f64 + 1.0);
                let win = 0.5 + 0.5 * (PI * x / span).cos();
                w.push((3.0 * 2.0 * fc * sinc * win) as f32);
            }
        }
        self.stats.window = (centre as f32, half as f32);
    }

    fn time_avg_gain(&mut self) {
        if self.dstats < 8 {
            self.alpha = 1.0;
            return;
        }
        let r = (4.0 * self.d1 - self.d2) / 6.0;
        let q = (self.d2 - self.d1) / 3.0;
        if r <= 0.0 {
            self.alpha = 1.0;
            return;
        }
        let q = q.max(0.0);
        let mut best = (1.0f64, f64::MAX);
        for i in 1..=20 {
            let a = i as f64 * 0.05;
            let cost = 16.0 * q * ((1.0 - a) / a).powi(2) + r * a / (2.0 - a);
            if cost < best.1 {
                best = (a, cost);
            }
        }
        self.alpha = best.0.max(0.1);
    }

    /// Common phase and slope, noise, rate of change, and the grid update for a symbol. Returns the rotated cells.
    fn track(&mut self, c: &Carriers, restarted: bool) -> Arc<[Complex32]> {
        let lay = self
            .lay
            .as_ref()
            .expect("a restart builds the layout before a symbol is handled");
        let k = lay.k;
        let half = (k as f64 - 1.0) / 2.0;
        let n = c.fft as f64;
        // A known window move first (not across a restart: the new estimate starts in the new window's frame).
        let d = if restarted { 0.0 } else { c.moved as f64 };
        self.phase_total += 2.0 * PI * c.int_offset as f64 * d / n;
        self.slope_total += 2.0 * PI * d / n;
        // Fit the residual against the known pilots: first the common turn (a circular mean, so a turn near half a revolution does not
        // wrap), then phase and slope of what is left by weighted least squares.
        let (pt, st) = (self.phase_total, self.slope_total);
        if self.have_ref {
            let mut sum = Complex32::default();
            for (i, &kk) in lay.continual.iter().enumerate() {
                let x = kk as f64 - half;
                let a = -(pt + st * x);
                let z =
                    c.cells[kk] / lay.pilot[kk] * Complex32::new(a.cos() as f32, a.sin() as f32);
                sum += z * self.ref_p[i].conj();
            }
            let a0 = (sum.im as f64).atan2(sum.re as f64);
            let (mut sw, mut sx, mut sy, mut sxx, mut sxy) = (0f64, 0f64, 0f64, 0f64, 0f64);
            for (i, &kk) in lay.continual.iter().enumerate() {
                let x = kk as f64 - half;
                let a = -(pt + a0 + st * x);
                let z =
                    c.cells[kk] / lay.pilot[kk] * Complex32::new(a.cos() as f32, a.sin() as f32);
                let r = z * self.ref_p[i].conj();
                let w = z.norm_sqr() as f64;
                let y = (r.im as f64).atan2(r.re as f64);
                sw += w;
                sx += w * x;
                sy += w * y;
                sxx += w * x * x;
                sxy += w * x * y;
            }
            let den = sw * sxx - sx * sx;
            self.phase_total += a0;
            if den.abs() > 1e-12 && sw > 0.0 {
                let b = (sw * sxy - sx * sy) / den;
                let a = (sy - b * sx) / sw;
                self.phase_total += a;
                self.slope_total += b;
            }
        }
        let (pt, st) = (self.phase_total, self.slope_total);
        // Rotate the whole symbol.
        let src = c.cells.clone();
        let rot = self.pool_r.exact(src.len(), |v| {
            let step = Complex32::new((-st).cos() as f32, (-st).sin() as f32);
            let a0 = -(pt - st * half);
            let mut r = Complex32::new(a0.cos() as f32, a0.sin() as f32);
            for (i, &x) in src.iter().enumerate() {
                if i % 64 == 0 {
                    let a = -(pt + st * (i as f64 - half));
                    r = Complex32::new(a.cos() as f32, a.sin() as f32);
                }
                v[i] = x * r;
                r *= step;
            }
        });
        let lay = self
            .lay
            .as_ref()
            .expect("a restart builds the layout before a symbol is handled");
        // Noise and the known pilot values.
        let np = lay.continual.len();
        let mut e = 0f64;
        let mut cur = std::mem::take(&mut self.cur);
        cur.clear();
        for (i, &kk) in lay.continual.iter().enumerate() {
            let z = rot[kk] / lay.pilot[kk];
            cur.push(z);
            if self.have_ref {
                e += (z - self.ref_p[i]).norm_sqr() as f64;
            }
        }
        if self.have_ref {
            let inst = e / np as f64 * 16.0 / 9.0 * NOISE_MARGIN;
            self.noise = if self.noise == 0.0 {
                inst
            } else {
                self.noise + 0.1 * (inst - self.noise)
            };
            // The floor follows the level of the signal: 60 dB under the pilots.
            let power = cur.iter().map(|z| z.norm_sqr() as f64).sum::<f64>() / np as f64;
            self.noise = self.noise.max(NOISE_FLOOR * power);
            for (rf, z) in self.ref_p.iter_mut().zip(&cur) {
                *rf = *rf * (1.0 - REF_WEIGHT) + z * REF_WEIGHT;
            }
        } else {
            self.ref_p.copy_from_slice(&cur);
            self.have_ref = true;
        }
        // How fast the channel changes.
        if self.prev1.len() == np {
            let m1 = cur
                .iter()
                .zip(&self.prev1)
                .map(|(a, b)| (a - b).norm_sqr() as f64)
                .sum::<f64>()
                / np as f64;
            self.d1 = if self.dstats == 0 {
                m1
            } else {
                self.d1 + 0.08 * (m1 - self.d1)
            };
            if self.prev2.len() == np {
                let m2 = cur
                    .iter()
                    .zip(&self.prev2)
                    .map(|(a, b)| (a - b).norm_sqr() as f64)
                    .sum::<f64>()
                    / np as f64;
                self.d2 = if self.dstats <= 1 {
                    m2
                } else {
                    self.d2 + 0.08 * (m2 - self.d2)
                };
                self.dstats += 1;
            }
        }
        let old2 = std::mem::replace(&mut self.prev2, std::mem::replace(&mut self.prev1, cur));
        self.cur = old2;
        self.time_avg_gain();
        let lay = self
            .lay
            .as_ref()
            .expect("a restart builds the layout before a symbol is handled");
        // The pilot grid.
        let p = c.l % 4;
        let t = self.t;
        let a = self.alpha;
        for &kk in &lay.scattered[p] {
            let kk = kk as usize;
            let m = kk / 3;
            let obs = rot[kk] / lay.pilot[kk];
            let gp = &mut self.grid[m];
            if gp.count == 0 || t - gp.last > 8 {
                gp.value = obs;
                gp.count = 1;
                gp.hist.clear();
            } else {
                gp.count += 1;
                let g = (a as f32).max(1.0 / gp.count as f32);
                gp.value += (obs - gp.value) * g;
            }
            gp.last = t;
            gp.hist.push_back((t, gp.value));
            if gp.hist.len() > 5 {
                gp.hist.pop_front();
            }
        }
        rot
    }

    /// The channel of grid point values at time `t` (time interpolation), into `self.hg`.
    fn grid_at(&mut self, t: i64) -> bool {
        self.hg.clear();
        for gp in &self.grid {
            let h = &gp.hist;
            if h.is_empty() {
                return false;
            }
            let v = if !self.time_interp {
                h[h.len() - 1].1
            } else {
                // The points before (time <= t) and after.
                let nb = h.iter().take_while(|x| x.0 <= t).count();
                let na = h.len() - nb;
                let mut pts = [(0f64, Complex32::default()); 4];
                let n = match (nb, na) {
                    (_, 0) => {
                        pts[0] = (0.0, h[h.len() - 1].1);
                        1
                    }
                    (0, _) => {
                        pts[0] = (0.0, h[0].1);
                        1
                    }
                    (1, _) | (_, 1) => {
                        pts[0] = (h[nb - 1].0 as f64, h[nb - 1].1);
                        pts[1] = (h[nb].0 as f64, h[nb].1);
                        2
                    }
                    _ => {
                        for (k, i) in (nb - 2..nb + 2).enumerate() {
                            pts[k] = (h[i].0 as f64, h[i].1);
                        }
                        4
                    }
                };
                if n == 1 {
                    pts[0].1
                } else {
                    // Lagrange through the points.
                    let x = t as f64;
                    let mut acc = Complex32::default();
                    for i in 0..n {
                        let mut w = 1.0;
                        for j in 0..n {
                            if i != j {
                                w *= (x - pts[j].0) / (pts[i].0 - pts[j].0);
                            }
                        }
                        acc += pts[i].1 * w as f32;
                    }
                    acc
                }
            };
            self.hg.push(v);
        }
        true
    }

    /// Frequency interpolation of the grid to all K carriers with the current window, into `self.h`.
    fn interpolate_current(&mut self) {
        let w = std::mem::take(&mut self.weights);
        let (c, t) = (self.centre, self.taps);
        self.interpolate(c, &w, t);
        self.weights = w;
    }

    /// Frequency interpolation of the grid to all K carriers, into `self.h`.
    fn interpolate(&mut self, centre: f64, weights: &[Vec<f32>; 3], taps: usize) {
        let lay = self
            .lay
            .as_ref()
            .expect("a restart builds the layout before a symbol is handled");
        let k = lay.k;
        let n = lay.mode.fft() as f64;
        let m = self.hg.len();
        // Remove the centre delay so the grid is slowly varying.
        let w0 = 2.0 * PI * centre / n;
        if self.phasor_key != Some((w0.to_bits(), m, k)) {
            self.phasor_key = Some((w0.to_bits(), m, k));
            phasors(3.0 * w0, |i| w0 * (3 * i) as f64, m, &mut self.phasor_in);
            phasors(-w0, |i| -w0 * i as f64, k, &mut self.phasor_out);
        }
        self.lg.clear();
        self.lg
            .extend(self.hg.iter().zip(&self.phasor_in).map(|(&v, &r)| v * r));
        self.h.clear();
        let j = taps as i64;
        let (lg, back) = (&self.lg, &self.phasor_out);
        // The weight sums of a full kernel, per phase (interior carriers need no renormalisation).
        let full: [f32; 3] = [
            weights[0].iter().sum(),
            weights[1].iter().sum(),
            weights[2].iter().sum(),
        ];
        let len = (2 * j + 1) as usize;
        for (kk, &rr) in back.iter().enumerate() {
            let m0 = (kk / 3) as i64;
            let r = kk % 3;
            let w = &weights[r];
            let v = if m0 - j >= 0 && m0 + j < m as i64 {
                // Interior: a fixed-length dot product.
                let base = (m0 - j) as usize;
                Complex32::dot(&w[..len], &lg[base..base + len]) / full[r]
            } else {
                let lo = (m0 - j).max(0);
                let hi = (m0 + j).min(m as i64 - 1);
                let mut acc = Complex32::default();
                let mut ws = 0f32;
                for mi in lo..=hi {
                    let wt = w[(mi - m0 + j) as usize];
                    acc += lg[mi as usize] * wt;
                    ws += wt;
                }
                if ws.abs() > 1e-6 {
                    acc / ws
                } else {
                    acc
                }
            };
            self.h.push(v * rr);
        }
    }

    /// Measures the delay spread on the full-window estimate and narrows the window.
    fn measure_spread(&mut self) {
        let Some(lay) = self.lay.as_ref() else { return };
        let n = lay.mode.fft();
        let g = self.guard as f64;
        let k = lay.k;
        let half = (k as i64 - 1) / 2;
        // The full-window estimate.
        let b = (self.guard / 4).min(24) as f64;
        let full_centre = b + g / 2.0;
        let saved = (self.centre, self.half, self.taps);
        for (s, w) in self.saved_weights.iter_mut().zip(&self.weights) {
            s.clone_from(w);
        }
        self.set_window(full_centre, n as f64 / 6.0);
        self.interpolate_current();
        let mut buf = std::mem::take(&mut self.ir);
        buf.clear();
        buf.resize(n, Complex32::default());
        for (kk, &v) in self.h.iter().enumerate() {
            buf[((kk as i64 - half).rem_euclid(n as i64)) as usize] = v;
        }
        if let Some(f) = self.ifft.as_mut() {
            f.process(&mut buf);
        }
        (self.centre, self.half, self.taps) = saved;
        for (w, s) in self.weights.iter_mut().zip(&self.saved_weights) {
            w.clone_from(s);
        }
        self.stats.window = (self.centre as f32, self.half as f32);
        let nf = n as f64;
        let lo = (-(g / 2.0).max(64.0)).max(g / 2.0 - nf / 6.0) as i64;
        let hi = (1.5 * g).min(g / 2.0 + nf / 6.0) as i64;
        let mut pw = std::mem::take(&mut self.spread_pw);
        pw.clear();
        pw.extend((lo..=hi).map(|d| (d, buf[d.rem_euclid(n as i64) as usize].norm_sqr() as f64)));
        self.ir = buf;
        let mut vals = std::mem::take(&mut self.spread_vals);
        vals.clear();
        vals.extend(pw.iter().map(|p| p.1));
        let mid = vals.len() / 2;
        let median = *vals.select_nth_unstable_by(mid, f64::total_cmp).1;
        self.spread_vals = vals;
        let floor = 1.5 * median;
        let peak = pw.iter().map(|p| p.1).fold(0.0, f64::max);
        let thr = (12.0 * floor).max(3e-4 * peak);
        let first = pw.iter().find(|p| p.1 > thr).map(|p| p.0);
        let last = pw.iter().rev().find(|p| p.1 > thr).map(|p| p.0);
        self.spread_pw = pw;
        if peak < 30.0 * floor {
            return;
        }
        let (Some(first), Some(last)) = (first, last) else {
            return;
        };
        let span = (last - first) as f64;
        self.measured_spread = span;
        self.stats.spread = span as f32;
        if self.full_window {
            return;
        }
        let half_w = span * 1.4 / 2.0 + 48.0;
        if half_w < 0.9 * nf / 6.0 {
            self.set_window((first + last) as f64 / 2.0, half_w);
        } else {
            self.set_window(full_centre, nf / 6.0);
        }
    }

    fn erased_cells(&mut self, nd: usize) -> (Arc<[Complex32]>, Arc<[f32]>) {
        let c = self.pool_c.exact(nd, |v| v.fill(Complex32::default()));
        let n = self.pool_n.exact(nd, |v| {
            v.fill(1e6);
        });
        (c, n)
    }

    fn output(&mut self, d: Delayed, out: &mut Vec<Timed<DataCells>>) {
        let Some(params) = d.params else { return };
        let nd = params.mode.data_carriers();
        let fresh = std::mem::take(&mut self.fresh_out) || d.fresh;
        let valid = !d.erased && self.grid_at(d.t);
        if !valid {
            let (cells, noise) = self.erased_cells(nd);
            out.push(Timed {
                time: d.time,
                value: DataCells {
                    cells,
                    noise,
                    l: d.l,
                    frame: d.frame,
                    tps_word: d.tps_word,
                    params,
                    erased: true,
                    fresh,
                    time: d.time,
                    rate: d.rate,
                },
            });
            return;
        }
        self.interpolate_current();
        self.stats.data_valid = true;
        let lay = self
            .lay
            .as_ref()
            .expect("a restart builds the layout before a symbol is handled");
        let p = d.l % 4;
        // The power of a data cell: the channel estimate at every seventh carrier.
        let mut hp = 0f64;
        let mut cnt = 0;
        for kk in (0..lay.k).step_by(7) {
            hp += self.h[kk].norm_sqr() as f64;
            cnt += 1;
        }
        let mean_h2 = (hp / cnt as f64) as f32;
        let noise = self.noise.max(NOISE_FLOOR * mean_h2 as f64).max(1e-30) as f32;
        // SNR: the data cell over the noise of a carrier, which the continual pilots gave. The noise of the de-mapper
        // has a margin and the noise of the pilot average in it; the SNR has neither.
        let carrier_noise = noise as f64 / (NOISE_MARGIN * REF_NOISE);
        self.stats.snr_db = (10.0 * (mean_h2 as f64 / carrier_noise).log10()) as f32;
        self.stats.noise = noise;
        self.stats.alpha = self.alpha as f32;
        let h = &self.h;
        let warm = self.since > 24;
        let (flat, inter) = (self.flat_csi, self.interferer);
        let (p_avg, n_avg, p_count) = (&mut self.p_avg, &mut self.n_avg, &mut self.p_count);
        let mut erased = 0u32;
        let mapper = &self.mapper;
        let mut mer = (0f64, 0f64);
        let data = &lay.data[p];
        let n = data.len();
        // Each carrier divided by its channel, and its noise; the data cells are picked from them.
        let (z_all, var_all) = (&mut self.z_all, &mut self.var_all);
        z_all.clear();
        z_all.extend(d.cells.iter().zip(h.iter()).map(|(&s, &hh)| {
            let h2 = hh.norm_sqr().max(1e-9);
            s * hh.conj() / h2
        }));
        var_all.clear();
        if flat {
            var_all.resize(z_all.len(), noise / 2.0 / mean_h2.max(1e-9));
        } else {
            var_all.extend(h.iter().map(|hh| noise / 2.0 / hh.norm_sqr().max(1e-9)));
        }
        let pool_n = &mut self.pool_n;
        let mut noise_a = None;
        let cells = self.pool_c.exact(n, |cv| {
            let filled = pool_n.exact(n, |nv| {
                for ((z, var), &kk) in cv.iter_mut().zip(nv.iter_mut()).zip(data.iter()) {
                    *z = z_all[kk as usize];
                    *var = var_all[kk as usize];
                }
                if inter {
                    for ((z, var), &kk) in cv.iter_mut().zip(nv.iter_mut()).zip(data.iter()) {
                        let kk = kk as usize;
                        let expected = 1.0 + 2.0 * *var;
                        let pw = z.norm_sqr().min(40.0 * expected);
                        // 1 / (count + 1), and 1 / 32 from 32 values on.
                        let g = if p_count[kk] >= 31 {
                            1.0 / 32.0
                        } else {
                            1.0 / (p_count[kk] + 1) as f32
                        };
                        p_avg[kk] += g * (pw - p_avg[kk]);
                        n_avg[kk] += g * (expected - n_avg[kk]);
                        p_count[kk] = p_count[kk].saturating_add(1);
                        if warm {
                            let ratio = p_avg[kk] / n_avg[kk].max(1e-9);
                            if ratio > 10.0 {
                                *z = Complex32::default();
                                *var = 1e6;
                                erased += 1;
                            } else if ratio > 1.6 {
                                *var += 0.5 * (p_avg[kk] - n_avg[kk]);
                            }
                        }
                    }
                }
                if let Some(m) = mapper {
                    for (&z, &var) in cv.iter().zip(nv.iter()) {
                        if var < 1e5 {
                            let q = m.nearest(z);
                            mer.0 += q.norm_sqr() as f64;
                            mer.1 += (z - q).norm_sqr() as f64;
                        }
                    }
                }
            });
            noise_a = Some(filled);
        });
        let Some(noise_a) = noise_a else { return };
        self.stats.erased_cells = erased;
        self.mer_acc.0 += mer.0;
        self.mer_acc.1 += mer.1;
        // Plots, at most every max(6 symbols, 25 ms).
        self.plot_syms += 1;
        if self.plot_syms >= 6 && d.time - self.plot_t >= 0.025 {
            self.plot_syms = 0;
            self.plot_t = d.time;
            self.plots(&cells, &d);
        }
        out.push(Timed {
            time: d.time,
            value: DataCells {
                cells,
                noise: noise_a,
                l: d.l,
                frame: d.frame,
                tps_word: d.tps_word,
                params,
                erased: false,
                fresh,
                time: d.time,
                rate: d.rate,
            },
        });
    }

    fn plots(&mut self, cells: &Arc<[Complex32]>, d: &Delayed) {
        if self.mer_acc.1 > 0.0 {
            self.stats.mer_db = (10.0 * (self.mer_acc.0 / self.mer_acc.1).log10()) as f32;
        }
        self.mer_acc = (0.0, 0.0);
        let Some(lay) = self.lay.as_ref() else { return };
        let r = self.results.results_mut();
        let step = (cells.len() / 1500).max(1);
        r.update_series(SeriesId(0), |s| {
            s.fill(false, cells.iter().step_by(step).map(|c| [c.re, c.im]))
        });
        let p = d.l % 4;
        let h = &self.h;
        r.update_series(SeriesId(1), |s| {
            s.fill(
                false,
                lay.kind[p]
                    .iter()
                    .enumerate()
                    .filter(|(_, kind)| {
                        matches!(kind, CarrierKind::Scattered | CarrierKind::Continual)
                    })
                    .map(|(kk, _)| {
                        let hh = h[kk];
                        let z = d.cells[kk] * hh.conj() / hh.norm_sqr().max(1e-9) / lay.pilot[kk];
                        [z.re, z.im]
                    }),
            )
        });
        let tps = &self.tps_pts;
        r.update_series(SeriesId(2), |s| s.fill(false, tps.iter().copied()));
        let hstep = (h.len() / 2048).max(1);
        r.update_series(SeriesId(3), |s| {
            s.fill(
                true,
                h.iter()
                    .enumerate()
                    .step_by(hstep)
                    .map(|(i, h)| [i as f32, 10.0 * h.norm_sqr().max(1e-12).log10()]),
            )
        });
        r.update_series(SeriesId(4), |s| {
            s.fill(
                true,
                h.iter()
                    .enumerate()
                    .step_by(hstep)
                    .map(|(i, h)| [i as f32, h.arg()]),
            )
        });
        // Impulse response.
        let n = lay.mode.fft();
        let half = (lay.k as i64 - 1) / 2;
        let buf = &mut self.plot_buf;
        buf.clear();
        buf.resize(n, Complex32::default());
        for (kk, &v) in h.iter().enumerate() {
            buf[((kk as i64 - half).rem_euclid(n as i64)) as usize] = v;
        }
        if let Some(f) = self.ifft.as_mut() {
            f.process(buf);
        }
        let g = self.guard as i64;
        let lo = -(n as i64 / 16);
        let hi = g + (8i64).max(g / 4);
        let peak = buf
            .iter()
            .map(|x| x.norm_sqr())
            .fold(0f32, f32::max)
            .max(1e-20);
        let buf = &self.plot_buf;
        r.update_series(SeriesId(5), |s| {
            s.fill(
                true,
                (lo..=hi).map(|dd| {
                    [
                        dd as f32,
                        10.0 * (buf[dd.rem_euclid(n as i64) as usize].norm_sqr() / peak)
                            .max(1e-12)
                            .log10(),
                    ]
                }),
            )
        });
        let st = &self.stats;
        self.results.put(&EqFields {
            snr_db: st.data_valid.then_some(f64::from(st.snr_db)),
            mer_db: st.data_valid.then_some(f64::from(st.mer_db)),
            alpha: f64::from(st.alpha),
            delay_window_centre: f64::from(st.window.0),
            delay_window_half: f64::from(st.window.1),
            delay_spread: f64::from(st.spread),
        });
    }
}

impl Component for DvbtEqualiser {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbt.equaliser",
            vec![Port::typed::<Carriers>("in")],
            vec![Port::typed::<DataCells>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(EqualiserSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.params = None;
        self.delay.clear();
        Ok(vec![Format::new(Kind::typed::<DataCells>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut out = std::mem::take(io.items_out::<DataCells>(0));
        let _timer = io.section("equaliser");
        for item in io.items::<Carriers>(0) {
            let c = &item.value;
            let Some(params) = c.params else { continue };
            let mode = params.mode;
            let mut restarted = false;
            let param_change = self
                .params
                .map(|p| (p.mode, p.guard, p.constellation, p.hierarchy))
                != Some((
                    params.mode,
                    params.guard,
                    params.constellation,
                    params.hierarchy,
                ));
            if c.fresh || c.rebuild || param_change || self.lay.is_none() {
                if c.fresh || param_change || self.lay.is_none() {
                    // A new acquisition or new parameters: what is delayed belongs to the old signal, and the error correction starts again.
                    self.delay.clear();
                    self.restart(mode, c.guard);
                } else {
                    // A resynchronisation: the delayed symbols are equalised with the estimate they belong to, then the estimate is
                    // rebuilt. The error correction keeps running (the symbols that fill the new grid reach it as erased ones).
                    while let Some(d) = self.delay.pop_front() {
                        self.output(d, &mut out);
                    }
                    let keep_fresh = self.fresh_out;
                    self.restart(mode, c.guard);
                    self.fresh_out = keep_fresh;
                }
                self.mapper = Some(crate::coding::constellation(
                    params.constellation,
                    params.hierarchy,
                ));
                self.params = Some(params);
                restarted = true;
            }
            self.params = Some(params);
            self.t += 1;
            self.since += 1;
            self.tps_pts.push_back([c.tps_point.re, c.tps_point.im]);
            if self.tps_pts.len() > 68 {
                self.tps_pts.pop_front();
            }
            let erased = c.erased || c.collapsed;
            if erased && restarted {
                self.restart_pending_move = true;
            }
            let cells = if erased {
                c.cells.clone()
            } else {
                let r = restarted || std::mem::take(&mut self.restart_pending_move);
                self.track(c, r)
            };
            self.delay.push_back(Delayed {
                cells,
                t: self.t,
                l: c.l,
                frame: c.frame,
                tps_word: c.tps_word,
                params: Some(params),
                erased,
                fresh: c.fresh,
                time: c.time,
                rate: c.rate,
            });
            // The delay spread, at the 8th grid symbol and every 16 after.
            if !erased
                && self.since >= self.spread_due
                && self.grid.iter().all(|g| !g.hist.is_empty())
            {
                self.spread_due = self.since + 16;
                self.measure_spread();
            }
            let delay = if self.time_interp { DELAY } else { 0 };
            while self.delay.len() > delay {
                let Some(d) = self.delay.pop_front() else {
                    break;
                };
                self.output(d, &mut out);
            }
        }
        *io.items_out::<DataCells>(0) = out;
        if let Some(l) = &self.link {
            if let Ok(mut t) = l.telemetry.lock() {
                t.eq = self.stats.clone();
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match EqualiserSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            EqualiserSetting::TimeInterp => self.time_interp = TIME_INTERP.read(id, v)?,
            EqualiserSetting::InterfererErase => self.interferer = INTERFERER_ERASE.read(id, v)?,
            EqualiserSetting::FlatCsi => self.flat_csi = FLAT_CSI.read(id, v)?,
            EqualiserSetting::FullWindow => {
                self.full_window = FULL_WINDOW.read(id, v)?;
                if self.full_window {
                    let n = self.mode.map_or(2048, |m| m.fft()) as f64;
                    let g = self.guard as f64;
                    self.set_window((self.guard / 4).min(24) as f64 + g / 2.0, n / 6.0);
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (EqualiserSetting::TimeInterp, Value::Bool(self.time_interp)),
            (
                EqualiserSetting::InterfererErase,
                Value::Bool(self.interferer),
            ),
            (EqualiserSetting::FlatCsi, Value::Bool(self.flat_csi)),
            (EqualiserSetting::FullWindow, Value::Bool(self.full_window)),
        ])
    }
    /// The channel estimate measures the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        let r = self.results.results_mut();
        r.fields_mut().clear();
        for i in 0..SERIES.len() {
            r.update_series(SeriesId(i as u16), |s| s.fill(false, std::iter::empty()));
        }
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

/// `n` phasors, each the one before times the phasor of the angle `step`; every 64th is the phasor of `angle(index)`.
fn phasors(step: f64, angle: impl Fn(usize) -> f64, n: usize, out: &mut Vec<Complex32>) {
    let by = Complex32::new(step.cos() as f32, step.sin() as f32);
    let mut r = Complex32::new(1.0, 0.0);
    out.clear();
    for i in 0..n {
        if i % 64 == 0 {
            let a = angle(i);
            r = Complex32::new(a.cos() as f32, a.sin() as f32);
        }
        out.push(r);
        r *= by;
    }
}
