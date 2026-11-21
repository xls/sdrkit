//! The carrier finder: the carriers of a capture on a raster, from averaged power spectra.

use crate::items::{Carrier, Carriers};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;
use sdrkit_graph::{Format, Kind, Timed};

/// The lowest noise floor of a raster cell: -100 dBFS, below the noise of any real capture. A clean test signal has no noise, and
/// its window leakage must not count as carriers.
const FLOOR_MIN: f64 = 1e-10;
/// The spectra averaged for one estimate, at most.
const FRAMES_MAX: usize = 8;

/// The settings of a [`Finder`]. Ratios are linear power ratios where the name says `ratio`.
#[derive(Clone, Debug, PartialEq)]
pub struct FinderConfig {
    /// The raster step, Hz.
    pub step_hz: f64,
    /// An absolute frequency the raster passes through, Hz. A capture without a known centre takes its dial frequency.
    pub origin_hz: f64,
    /// The part of the rate searched on each side of the centre.
    pub span: f64,
    /// The widest spectrum bin, Hz (the transform length is the next power of two, 64 to 16384).
    pub resolution_hz: f64,
    /// Half the width a cell's power is summed over, Hz.
    pub cell_half_hz: f64,
    /// Half the width the occupied width is measured over, Hz.
    pub width_half_hz: f64,
    /// A cell this far above its floor becomes a carrier, dB.
    pub on_db: f64,
    /// A cell below this goes quiet, dB above its floor.
    pub off_db: f64,
    /// A known channel uses thresholds this much lower, dB.
    pub known_db: f64,
    /// Absolute frequencies of known channels, Hz.
    pub known: Vec<f64>,
    /// The time above the threshold before a carrier is reported, s.
    pub report_after_s: f64,
    /// The signal one estimate covers, s.
    pub estimate_s: f64,
    /// A quiet carrier stays tracked this long, s.
    pub keep_s: f64,
    /// The cells on each side that give a cell's noise floor.
    pub floor_cells: usize,
    /// A cell this far below a neighbour is its skirt (power ratio).
    pub skirt_ratio: f64,
    /// Of two adjacent active cells the weaker is dropped when it is this far below the other (power ratio).
    pub merge_ratio: f64,
    /// A cell whose strongest bin holds more than this share of its power is an unmodulated line and is rejected.
    pub line_share: Option<f64>,
    /// The least signal-to-noise ratio of a new carrier, dB.
    pub min_snr_db: Option<f64>,
    /// The most carriers tracked at once.
    pub max: usize,
    /// At the cap a new carrier must be this much stronger than the weakest tracked one (power ratio).
    pub replace_ratio: f64,
    /// A retune by less than 0.45 of the rate keeps the tracked carriers.
    pub keep_on_retune: bool,
}

impl FinderConfig {
    /// A finder on a raster of `step_hz` through `origin_hz` with the thresholds of a narrow-band FM service.
    pub fn new(step_hz: f64, origin_hz: f64) -> Self {
        FinderConfig {
            step_hz,
            origin_hz,
            span: 0.46,
            resolution_hz: 1000.0,
            cell_half_hz: 0.4 * step_hz,
            width_half_hz: 0.5 * step_hz,
            on_db: 9.0,
            off_db: 5.0,
            known_db: 3.0,
            known: Vec::new(),
            report_after_s: 0.12,
            estimate_s: 0.05,
            keep_s: 5.0,
            floor_cells: 12,
            skirt_ratio: 1000.0,
            merge_ratio: 10.0,
            line_share: None,
            min_snr_db: None,
            max: 10,
            replace_ratio: 4.0,
            keep_on_retune: false,
        }
    }

    /// The raster cells the capture of `f` holds: (absolute frequency, offset in the stream), within `span` Hz of the centre.
    pub fn cells(&self, f: &Format, span: f64) -> Vec<(f64, f64)> {
        let step = self.step_hz;
        let mut cells = Vec::new();
        if f.centre_hz > 0.0 {
            let k0 = ((f.centre_hz - span - self.origin_hz) / step).ceil() as i64;
            let k1 = ((f.centre_hz + span - self.origin_hz) / step).floor() as i64;
            for k in k0..=k1 {
                let abs = self.origin_hz + k as f64 * step;
                cells.push((abs, abs - f.centre_hz));
            }
        } else {
            let d = f.tuned_hz - f.centre_hz;
            let k0 = ((-span - d) / step).ceil() as i64;
            let k1 = ((span - d) / step).floor() as i64;
            for k in k0..=k1 {
                cells.push((f.tuned_hz + k as f64 * step, d + k as f64 * step));
            }
        }
        cells
    }

    /// The channels a capture holds (absolute frequencies of the first and last raster cell), from the format of the carrier
    /// stream: its centre and tuned frequency are the capture's, its bandwidth twice the span searched.
    pub fn held(&self, carriers: &Format) -> Option<(f64, f64)> {
        let cells = self.cells(carriers, carriers.bandwidth_hz / 2.0);
        Some((cells.first()?.0, cells.last()?.0))
    }
}

#[derive(Clone, Debug, Default)]
struct Cell {
    freq: f64,
    off: f64,
    known: bool,
    floor: f64,
    above_since: Option<f64>,
    active: bool,
    last_active: f64,
    id: Option<u64>,
    since: f64,
    power: f64,
    width: f64,
}

/// The carrier finder. It takes the samples of a capture and gives one carrier list for every estimate period.
pub struct Finder {
    cfg: FinderConfig,
    rate: f64,
    centre: f64,
    n: usize,
    fft: Option<Fft>,
    win: Vec<f32>,
    buf: Vec<Complex32>,
    fill: usize,
    acc: Vec<f64>,
    frames: usize,
    per_estimate: u64,
    est_start: u64,
    cells: Vec<Cell>,
    next_id: u64,
    playing: Option<f64>,
    work: Vec<Complex32>,
    count: u64,
}

impl Finder {
    /// A finder with the settings `cfg`. It finds nothing before [`Finder::configure`].
    pub fn new(cfg: FinderConfig) -> Self {
        Finder {
            cfg,
            rate: 0.0,
            centre: 0.0,
            n: 256,
            fft: None,
            win: Vec::new(),
            buf: Vec::new(),
            fill: 0,
            acc: Vec::new(),
            frames: 0,
            per_estimate: 0,
            est_start: 0,
            cells: Vec::new(),
            next_id: 1,
            playing: None,
            work: Vec::new(),
            count: 0,
        }
    }

    /// The settings.
    pub fn config(&self) -> &FinderConfig {
        &self.cfg
    }

    /// The settings, to change. A change of the raster or the timing takes effect at the next [`Finder::configure`].
    pub fn config_mut(&mut self) -> &mut FinderConfig {
        &mut self.cfg
    }

    /// The frequency that plays: its carrier is never pushed out at the cap.
    pub fn set_playing(&mut self, hz: Option<f64>) {
        self.playing = hz;
    }

    /// Forgets every carrier.
    pub fn drop_carriers(&mut self) {
        for c in &mut self.cells {
            c.id = None;
            c.active = false;
            c.above_since = None;
        }
    }

    /// Starts on the capture of format `f` and returns the format of the carrier stream. The signal time starts at 0.
    pub fn configure(&mut self, f: &Format) -> Format {
        let keep = self.cfg.keep_on_retune
            && self.rate == f.rate
            && f.centre_hz > 0.0
            && self.centre > 0.0
            && (f.centre_hz - self.centre).abs() < 0.45 * f.rate;
        let t_old = self.count as f64 / self.rate.max(1.0);
        let old: Vec<Cell> = if keep {
            self.cells.drain(..).filter(|c| c.id.is_some()).collect()
        } else {
            Vec::new()
        };
        if !keep {
            self.next_id = 1;
        }
        self.playing = None;
        self.rate = f.rate;
        self.centre = f.centre_hz;
        self.n = ((f.rate / self.cfg.resolution_hz) as usize)
            .next_power_of_two()
            .clamp(64, 16384);
        self.fft = Some(Fft::forward(self.n));
        self.win = Window::Hann.coefficients(self.n);
        self.buf = vec![Complex32::default(); self.n];
        self.work = vec![Complex32::default(); self.n];
        self.acc = vec![0.0; self.n];
        self.fill = 0;
        self.frames = 0;
        self.count = 0;
        self.est_start = 0;
        self.per_estimate = (self.cfg.estimate_s * f.rate).round() as u64;
        let span = self.cfg.span * f.rate;
        let half = self.cfg.step_hz / 2.0;
        self.cells = self
            .cfg
            .cells(f, span)
            .into_iter()
            .map(|(freq, off)| {
                let mut c = Cell {
                    freq,
                    off,
                    known: self.cfg.known.iter().any(|k| (k - freq).abs() <= half),
                    ..Default::default()
                };
                if let Some(o) = old.iter().find(|o| (o.freq - freq).abs() < 1.0) {
                    c.id = o.id;
                    c.active = o.active;
                    c.since = o.since - t_old;
                    c.width = o.width;
                }
                c
            })
            .collect();
        Format {
            centre_hz: f.centre_hz,
            tuned_hz: f.tuned_hz,
            bandwidth_hz: 2.0 * span,
            ..Format::new(Kind::typed::<Carriers>(), 0.0)
        }
    }

    /// Takes the samples `x` and appends a carrier list to `out` for every estimate period that ends in them.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Timed<Carriers>>) {
        if self.fft.is_none() {
            return;
        }
        for &s in x {
            self.count += 1;
            let into = self.count - self.est_start;
            if self.frames < FRAMES_MAX && into <= (FRAMES_MAX * self.n) as u64 {
                self.buf[self.fill] = s;
                self.fill += 1;
                if self.fill == self.n {
                    self.fill = 0;
                    for i in 0..self.n {
                        self.work[i] = self.buf[i] * self.win[i];
                    }
                    if let Some(f) = &mut self.fft {
                        f.process(&mut self.work);
                    }
                    let norm = (self.n * self.n) as f64;
                    for i in 0..self.n {
                        self.acc[i] += f64::from(self.work[i].norm_sqr()) / norm;
                    }
                    self.frames += 1;
                }
            }
            if into >= self.per_estimate {
                let c = self.estimate();
                out.push(Timed {
                    time: self.count as f64 / self.rate,
                    value: Carriers(c),
                });
                self.acc.iter_mut().for_each(|v| *v = 0.0);
                self.frames = 0;
                self.fill = 0;
                self.est_start = self.count;
            }
        }
    }

    /// The bins of the averaged spectrum from `lo` to `hi` Hz of the stream.
    fn bins(&self, lo: f64, hi: f64) -> impl Iterator<Item = (isize, f64)> + '_ {
        let bin = self.rate / self.n as f64;
        let nf = self.frames.max(1) as f64;
        let n = self.n as isize;
        let k0 = (lo / bin).round() as isize;
        let k1 = (hi / bin).round() as isize;
        (k0..=k1).map(move |k| (k, self.acc[k.rem_euclid(n) as usize] / nf))
    }

    fn estimate(&mut self) -> Vec<Carrier> {
        let cfg = &self.cfg;
        let t = self.count as f64 / self.rate;
        let half = cfg.cell_half_hz;
        // Cell power and peak.
        let mut pw = Vec::with_capacity(self.cells.len());
        let mut line = Vec::with_capacity(self.cells.len());
        for c in &self.cells {
            let (mut s, mut pk) = (0.0f64, 0.0f64);
            for (_, v) in self.bins(c.off - half, c.off + half) {
                s += v;
                pk = pk.max(v);
            }
            pw.push(s);
            line.push(if s > 0.0 { pk / s } else { 0.0 });
        }
        let mut sorted = pw.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let span_floor = sorted
            .get(sorted.len() / 10)
            .copied()
            .unwrap_or(0.0)
            .max(FLOOR_MIN);
        let m = self.cells.len();
        let side = cfg.floor_cells;
        for i in 0..m {
            let lo = i.saturating_sub(side);
            let hi = (i + side + 1).min(m);
            let mut nb: Vec<f64> = (lo..hi).filter(|&j| j != i).map(|j| pw[j]).collect();
            nb.sort_by(|a, b| a.total_cmp(b));
            let target = nb
                .get(nb.len() / 5)
                .copied()
                .unwrap_or(span_floor)
                .max(span_floor);
            let c = &mut self.cells[i];
            c.floor = if c.floor <= 0.0 {
                target
            } else {
                c.floor + 0.05 * (target - c.floor)
            };
            c.power = pw[i];
        }
        for i in 0..m {
            let db = 10.0 * (pw[i] / self.cells[i].floor.max(1e-30)).log10();
            let skirt = (i > 0 && pw[i] * cfg.skirt_ratio < pw[i - 1])
                || (i + 1 < m && pw[i] * cfg.skirt_ratio < pw[i + 1]);
            let is_line = cfg.line_share.is_some_and(|s| line[i] > s);
            let c = &mut self.cells[i];
            let lower = if c.known { cfg.known_db } else { 0.0 };
            if db > cfg.on_db - lower && !skirt && !is_line {
                if c.above_since.is_none() {
                    c.above_since = Some(t);
                }
                if t - c.above_since.unwrap_or(t) >= cfg.report_after_s {
                    c.active = true;
                }
            } else if db < cfg.off_db - lower || skirt {
                c.above_since = None;
                c.active = false;
            }
            if c.active {
                c.last_active = t;
            }
        }
        // One signal over two adjacent cells is one carrier: the weaker cell is dropped.
        for i in 0..m.saturating_sub(1) {
            if self.cells[i].active && self.cells[i + 1].active {
                let weaker = if pw[i] < pw[i + 1] { i } else { i + 1 };
                if pw[weaker] * cfg.merge_ratio < pw[i + i + 1 - weaker] {
                    self.cells[weaker].active = false;
                }
            }
        }
        // Identities stay for the keep time after a carrier went quiet. A carrier that drifts into the next cell keeps its
        // identity.
        for i in 0..m {
            if self.cells[i].active && self.cells[i].id.is_none() {
                let moved = [i.wrapping_sub(1), i + 1]
                    .into_iter()
                    .filter(|&j| j < m)
                    .find(|&j| self.cells[j].id.is_some() && !self.cells[j].active);
                if let Some(j) = moved {
                    self.cells[i].id = self.cells[j].id.take();
                    self.cells[i].since = self.cells[j].since;
                    self.cells[i].width = self.cells[j].width;
                } else {
                    let snr = 10.0 * (pw[i] / self.cells[i].floor.max(1e-30)).log10();
                    if cfg.min_snr_db.is_some_and(|min| snr < min) {
                        continue;
                    }
                    // The cap: a newcomer must beat the weakest tracked one. The playing one is never pushed out.
                    let tracked: Vec<usize> =
                        (0..m).filter(|&k| self.cells[k].id.is_some()).collect();
                    if tracked.len() >= cfg.max {
                        let weakest = tracked
                            .iter()
                            .copied()
                            .filter(|&k| {
                                Some(self.cells[k].freq) != self.playing && !self.cells[k].active
                            })
                            .chain(
                                tracked
                                    .iter()
                                    .copied()
                                    .filter(|&k| Some(self.cells[k].freq) != self.playing),
                            )
                            .min_by(|&a, &b| pw[a].total_cmp(&pw[b]));
                        match weakest {
                            Some(w) if pw[i] > cfg.replace_ratio * pw[w] => self.cells[w].id = None,
                            _ => continue,
                        }
                    }
                    self.cells[i].id = Some(self.next_id);
                    self.cells[i].since = t;
                    self.cells[i].width = 0.0;
                    self.next_id += 1;
                }
            }
            let c = &mut self.cells[i];
            if c.id.is_some() && !c.active && t - c.last_active > cfg.keep_s {
                c.id = None;
            }
        }
        let bin = self.rate / self.n as f64;
        let mut out = Vec::new();
        for i in 0..m {
            let c = &self.cells[i];
            let Some(id) = c.id else { continue };
            // The floor of one bin, the centroid of what lies above it in the cell, and the bins more than 6 dB above it.
            let cell_bins = self.bins(c.off - half, c.off + half).count().max(1);
            let fb = c.floor / cell_bins as f64;
            let (mut sw, mut sf) = (0.0f64, 0.0f64);
            for (k, v) in self.bins(c.off - half, c.off + half) {
                let e = (v - fb).max(0.0);
                sw += e;
                sf += e * k as f64 * bin;
            }
            let centroid = if sw > 0.0 { sf / sw } else { c.off };
            let wide = self
                .bins(c.off - cfg.width_half_hz, c.off + cfg.width_half_hz)
                .filter(|(_, v)| *v > 4.0 * fb)
                .count() as f64
                * bin;
            let width = if c.width > 0.0 {
                0.7 * c.width + 0.3 * wide
            } else {
                wide
            };
            out.push(Carrier {
                id,
                freq_hz: c.freq,
                offset_hz: c.off,
                level_dbfs: 10.0 * c.power.max(1e-30).log10(),
                snr_db: 10.0 * (c.power / c.floor.max(1e-30)).max(1e-30).log10(),
                active: c.active,
                centroid_hz: c.freq + (centroid - c.off),
                width_hz: width,
                age_s: t - c.since,
            });
            self.cells[i].width = width;
        }
        out
    }
}
