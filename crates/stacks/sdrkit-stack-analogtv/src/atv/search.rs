//! The carrier search and the monitor spectrum: averaged spectra are searched for vision carriers with their sound carriers, in the
//! normal and the mirrored spectrum. Once a carrier is followed, the monitor spectrum serves the carrier follow and the display.

use super::{last, Candidate, Candidates, Monitor, Phase, Tune};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::meter::power_to_db as db10;
use sdrkit_core::{Complex32, ConfigError};
use sdrkit_graph::{Component, ComponentInfo, Format, Io, Kind, Port};

/// FFT size.
pub const N: usize = 8192;
/// Spectra averaged before the candidates are judged.
pub const SPECTRA: u32 = 24;
/// The standard sound spacings, MHz.
pub const SPACINGS: [f64; 6] = [5.5, 6.0, 6.5, 4.5, 3.5, 11.15];

/// The search component.
pub struct CarrierSearch {
    fs: f64,
    fft: Fft,
    window: Vec<f32>,
    buf_in: Vec<Complex32>,
    buf_raw: Vec<Complex32>,
    work: Vec<Complex32>,
    acc_in: Vec<f64>,
    acc_raw: Vec<f64>,
    mon: Vec<f64>,
    spectra: u32,
    mon_spectra: u32,
    pos: u64,
    collecting: bool,
    phase: Phase,
    epoch: u32,
    tune: Tune,
    /// The whole input is shown (FM video, NBTV): the display spans ±0.9 of Nyquist.
    pub whole: bool,
    /// Only the monitor runs (FM video and NBTV have no carrier to search).
    pub monitor_only: bool,
    mon_every: u64,
    report: Monitor,
}

impl Default for CarrierSearch {
    fn default() -> Self {
        CarrierSearch {
            fs: 1.0,
            fft: Fft::forward(N),
            window: (0..N)
                .map(|i| (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / N as f64).cos()) as f32)
                .collect(),
            buf_in: Vec::with_capacity(N),
            buf_raw: Vec::with_capacity(N),
            work: vec![Complex32::default(); N],
            acc_in: vec![0.0; N],
            acc_raw: vec![0.0; N],
            mon: vec![0.0; N],
            spectra: 0,
            mon_spectra: 0,
            pos: 0,
            collecting: false,
            phase: Phase::Search,
            epoch: 0,
            tune: Tune::default(),
            whole: false,
            monitor_only: false,
            mon_every: 2 * N as u64,
            report: Monitor::default(),
        }
    }
}

/// The candidates of one power spectrum (fft-shifted: bin k is (k - N/2) fs / N).
pub fn find(
    p: &[f64],
    fs: f64,
    exclude_centre: bool,
    only_centre: bool,
    mirrored: bool,
) -> Vec<Candidate> {
    let n = p.len();
    let bin = fs / n as f64;
    let l: Vec<f64> = p.iter().map(|&v| db10(v)).collect();
    let k_of = |f: f64| ((f / bin).round() as i64 + n as i64 / 2).clamp(0, n as i64 - 1) as usize;
    let f_of = |k: usize| (k as f64 - n as f64 / 2.0) * bin;
    let mean_p = |a: f64, b: f64| -> f64 {
        let (ka, kb) = (k_of(a.min(b)), k_of(a.max(b)));
        if kb <= ka {
            return p[ka];
        }
        p[ka..=kb].iter().sum::<f64>() / (kb - ka + 1) as f64
    };
    let mut sorted: Vec<f64> = p.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let floor_db = db10(sorted[n / 2]);
    let edge = (150e3 / bin).ceil() as usize;
    let centre = (12e3 / bin).ceil() as i64;
    let w = 8usize;
    let mut cands: Vec<(usize, f64)> = Vec::new();
    for k in edge.max(w)..n - edge.max(w) {
        let dc = k as i64 - n as i64 / 2;
        if exclude_centre && dc.abs() <= centre {
            continue;
        }
        if only_centre && (dc as f64 * bin).abs() > 40e3 {
            continue;
        }
        let v = p[k];
        if (k - w..=k + w).any(|j| p[j] > v) {
            continue;
        }
        let f = f_of(k);
        let ring = 0.5 * (mean_p(f - 90e3, f - 24e3) + mean_p(f + 24e3, f + 90e3));
        let prom = l[k] - db10(ring);
        if prom > 9.0 {
            cands.push((k, prom));
        }
    }
    cands.sort_by(|a, b| l[b.0].total_cmp(&l[a.0]));
    cands.truncate(6);
    let line_at = |f: f64| -> (f64, f64) {
        // Strongest 7-bin average within ±30 kHz, and the peak bin there.
        let (a, b) = (k_of(f - 30e3), k_of(f + 30e3));
        let mut best = f64::MIN;
        let mut peak = f64::MIN;
        for k in a.max(3)..=b.min(n - 4) {
            let avg = p[k - 3..=k + 3].iter().sum::<f64>() / 7.0;
            best = best.max(db10(avg));
            peak = peak.max(l[k]);
        }
        (best, peak)
    };
    let pic = |f: f64| db10(0.5 * (mean_p(f - 0.9e6, f - 0.25e6) + mean_p(f + 0.25e6, f + 0.9e6)));
    let half = fs / 2.0 - 150e3;
    let mut out: Vec<Candidate> = cands
        .iter()
        .map(|&(k, prom)| {
            let (a, b, c) = (l[k - 1], l[k], l[k + 1]);
            let den = a - 2.0 * b + c;
            let d = if den.abs() > 1e-12 {
                (0.5 * (a - c) / den).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            let f = f_of(k) + d * bin;
            let lev = b;
            let mut spacing = 0.0;
            let mut best_margin = f64::MIN;
            for s in SPACINGS {
                let fs_ = f + s * 1e6;
                if fs_.abs() > half {
                    continue;
                }
                let (avg, peak) = line_at(fs_);
                let floor = db10(mean_p(fs_ + 150e3, fs_ + 320e3))
                    .min(db10(mean_p(fs_ - 320e3, fs_ - 150e3)));
                let margin = avg - floor;
                if margin >= 16.0 && peak >= lev - 30.0 && peak <= lev + 0.5 && margin > best_margin
                {
                    best_margin = margin;
                    spacing = s * 1e6;
                }
            }
            let here = pic(f);
            let mut turned = false;
            for s in SPACINGS {
                for sign in [-1.0, 1.0] {
                    let g = f + sign * s * 1e6;
                    if g.abs() > half {
                        continue;
                    }
                    let (_, peak) = line_at(g);
                    if peak >= lev - 20.0 && pic(g) > here + 8.0 {
                        turned = true;
                    }
                }
            }
            let dsb = db10(mean_p(f - 0.6e6, f - 0.3e6)) - db10(mean_p(f + 0.3e6, f + 0.6e6));
            let mut score = prom.min(40.0) + if spacing > 0.0 { 10.0 } else { 0.0 }
                - if turned { 15.0 } else { 0.0 };
            if here - floor_db < 6.0 && spacing == 0.0 {
                score -= 5.0;
            }
            Candidate {
                hz: f,
                mirrored,
                centre: only_centre,
                prominence: prom,
                spacing_hz: spacing,
                dsb_db: dsb,
                score,
                level_db: lev,
            }
        })
        .collect();
    // A bare carrier (no picture around it) beside a stronger carrier with a picture is a sound carrier of that channel.
    let pics: Vec<f64> = out.iter().map(|c| pic(c.hz)).collect();
    for i in 0..out.len() {
        for j in 0..out.len() {
            if i != j
                && (out[i].hz - out[j].hz).abs() < 7.2e6
                && out[j].level_db > out[i].level_db + 3.0
                && pics[j] > pics[i] + 8.0
            {
                out[i].score -= 15.0;
                break;
            }
        }
    }
    if only_centre {
        out.retain(|c| c.spacing_hz > 0.0 && c.prominence >= 12.0);
    }
    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    out
}

/// The mirrored spectrum (that of the conjugated input).
pub fn mirror(p: &[f64]) -> Vec<f64> {
    let n = p.len();
    (0..n).map(|k| p[(n - k) % n]).collect()
}

/// The search's decision from the four spectra: best first.
pub fn judge(p_in: &[f64], p_raw: &[f64], fs: f64) -> Vec<Candidate> {
    let normal = find(p_in, fs, true, false, false);
    let mirrored = find(&mirror(p_in), fs, true, false, true);
    let best_n = normal.first().map_or(f64::MIN, |c| c.score);
    let best_m = mirrored.first().map_or(f64::MIN, |c| c.score);
    let mut list = if best_m > best_n + 5.0 {
        mirrored
    } else {
        normal
    };
    let mut centre = find(p_raw, fs, false, true, false);
    centre.extend(find(&mirror(p_raw), fs, false, true, true));
    centre.sort_by(|a, b| b.score.total_cmp(&a.score));
    let best = list.first().map_or(f64::MIN, |c| c.score);
    if let Some(c) = centre.first() {
        if c.score >= best {
            list.insert(0, *c);
        }
    }
    list.retain(|c| c.prominence >= 12.0);
    list.truncate(6);
    list
}

impl CarrierSearch {
    fn transform(&mut self, which_in: bool) -> &[Complex32] {
        let src = if which_in {
            &self.buf_in
        } else {
            &self.buf_raw
        };
        for i in 0..N {
            self.work[i] = src[i] * self.window[i];
        }
        self.fft.process(&mut self.work);
        &self.work
    }

    fn add_power(acc: &mut [f64], spec: &[Complex32], weight: Option<f64>) {
        let h = N / 2;
        for k in 0..N {
            let v = spec[(k + h) % N].norm_sqr() as f64;
            match weight {
                None => acc[k] += v,
                Some(w) => acc[k] += w * (v - acc[k]),
            }
        }
    }

    fn monitor(&mut self) {
        let mut spec = std::mem::take(&mut self.work);
        let src = &self.buf_in;
        let mir = self.tune.mirrored;
        for i in 0..N {
            let x = src[i];
            spec[i] = if mir { x.conj() } else { x } * self.window[i];
        }
        self.fft.process(&mut spec);
        let w = if self.mon_spectra == 0 {
            None
        } else {
            Some(0.3)
        };
        if w.is_none() {
            self.mon.iter_mut().for_each(|v| *v = 0.0);
        }
        Self::add_power(&mut self.mon, &spec, w);
        self.work = spec;
        self.mon_spectra += 1;
        let bin = self.fs / N as f64;
        let k_of =
            |f: f64| ((f / bin).round() as i64 + N as i64 / 2).clamp(0, N as i64 - 1) as usize;
        // The strongest line near the carrier.
        let c = self.tune.carrier_hz;
        let (a, b) = (k_of(c - 150e3), k_of(c + 150e3));
        let mut kb = a;
        for k in a..=b {
            if self.mon[k] > self.mon[kb] {
                kb = k;
            }
        }
        let f = (kb as f64 - N as f64 / 2.0) * bin;
        let mean = |x: f64, y: f64| {
            let (ka, kb) = (k_of(x), k_of(y));
            self.mon[ka..=kb.max(ka)].iter().sum::<f64>() / (kb.max(ka) - ka + 1) as f64
        };
        let ring = 0.5 * (mean(f - 90e3, f - 24e3) + mean(f + 24e3, f + 90e3));
        let (l0, l1, l2) = (
            db10(self.mon[kb.saturating_sub(1)]),
            db10(self.mon[kb]),
            db10(self.mon[(kb + 1).min(N - 1)]),
        );
        let den = l0 - 2.0 * l1 + l2;
        let d = if den.abs() > 1e-12 {
            (0.5 * (l0 - l2) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        self.report.spectra = self.mon_spectra;
        self.report.line_hz = f + d * bin;
        self.report.prominence = l1 - db10(ring);
        self.report.level_db = l1;
        // The display spectrum.
        let (lo, hi) = if self.whole {
            (-0.9 * self.fs / 2.0, 0.9 * self.fs / 2.0)
        } else {
            (c - 1.5e6, c + 7e6)
        };
        let pts = 170;
        self.report.spectrum.resize(pts, [0.0; 2]);
        let mut top = f64::MIN;
        for i in 0..pts {
            let fa = lo + (hi - lo) * i as f64 / pts as f64;
            let fb = lo + (hi - lo) * (i + 1) as f64 / pts as f64;
            let (ka, kb) = (k_of(fa), k_of(fb));
            let v = self.mon[ka..=kb.max(ka)]
                .iter()
                .cloned()
                .fold(0.0, f64::max);
            let db = db10(v);
            top = top.max(db);
            let x = if self.whole {
                (fa + fb) / 2.0
            } else {
                (fa + fb) / 2.0 - c
            };
            self.report.spectrum[i] = [(x / 1e6) as f32, db as f32];
        }
        for p in &mut self.report.spectrum {
            p[1] -= top as f32;
        }
        self.report.fresh = true;
    }
}

impl Component for CarrierSearch {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.search",
            vec![
                Port::iq("in"),
                Port::iq("raw"),
                Port::typed::<Tune>("control"),
            ],
            vec![
                Port::typed::<Candidates>("found"),
                Port::typed::<Monitor>("monitor"),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        self.mon_every = (2 * N as u64).max((0.025 * self.fs).round() as u64);
        self.restart();
        self.phase = Phase::Search;
        self.epoch = u32::MAX;
        Ok(vec![
            Format::new(Kind::typed::<Candidates>(), 0.0),
            Format::new(Kind::typed::<Monitor>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(t) = last::<Tune>(io, 2) {
            if t.epoch != self.epoch || t.phase != self.phase {
                let back_to_search = t.phase == Phase::Search
                    && (self.phase != Phase::Search || t.epoch != self.epoch);
                self.phase = t.phase;
                self.epoch = t.epoch;
                if back_to_search {
                    self.restart();
                }
                if t.phase != Phase::Search {
                    self.mon_spectra = 0;
                    self.pos = 0;
                    self.buf_in.clear();
                }
            }
            self.tune = t;
        }
        let x = io.iq(0);
        let raw = io.iq(1);
        let searching = self.phase == Phase::Search && !self.monitor_only;
        let mut found = None;
        for i in 0..x.len() {
            if searching {
                let in_window = self.pos % (2 * N as u64) < N as u64;
                if in_window {
                    self.buf_in.push(x[i]);
                    self.buf_raw.push(raw[i]);
                    if self.buf_in.len() == N {
                        let s = self.transform(true).to_vec();
                        Self::add_power(&mut self.acc_in, &s, None);
                        let s = self.transform(false).to_vec();
                        Self::add_power(&mut self.acc_raw, &s, None);
                        self.buf_in.clear();
                        self.buf_raw.clear();
                        self.spectra += 1;
                        if self.spectra >= SPECTRA {
                            let list = judge(&self.acc_in, &self.acc_raw, self.fs);
                            found = Some(Candidates { list });
                            // Keep monitoring the average for the display.
                            self.mon.copy_from_slice(&self.acc_in);
                            self.acc_in.iter_mut().for_each(|v| *v = 0.0);
                            self.acc_raw.iter_mut().for_each(|v| *v = 0.0);
                            self.spectra = 0;
                        }
                    }
                }
            } else {
                let slot = self.pos % self.mon_every;
                if slot < N as u64 {
                    self.buf_in.push(x[i]);
                    if self.buf_in.len() == N {
                        self.monitor();
                        self.buf_in.clear();
                    }
                }
            }
            self.pos += 1;
        }
        let t = io.input_format(0).time_of(io.input(0).first());
        if let Some(f) = found {
            io.items_out::<Candidates>(0)
                .push(sdrkit_graph::Timed { time: t, value: f });
        }
        if io.tick() {
            io.items_out::<Monitor>(1).push(sdrkit_graph::Timed {
                time: t,
                value: self.report.clone(),
            });
            self.report.fresh = false;
        }
    }
}

impl CarrierSearch {
    fn restart(&mut self) {
        self.buf_in.clear();
        self.buf_raw.clear();
        self.acc_in.iter_mut().for_each(|v| *v = 0.0);
        self.acc_raw.iter_mut().for_each(|v| *v = 0.0);
        self.spectra = 0;
        self.mon_spectra = 0;
        self.pos = 0;
        self.collecting = false;
        self.report = Monitor::default();
    }
}
