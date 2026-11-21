//! Acquisition: the satellite jobs of the shared budgeted code search, with the replica table of one signal kind, the hit and the
//! acquisition plot.

use crate::codes::SignalKind;
use sdrkit_algo_modem::dsss::{
    CodeReplica, CodeSearch, SearchConfig, SearchGrid, SearchJob, SearchResult,
};
use sdrkit_algo_modem::level::cn0_dbhz;
use sdrkit_core::Complex32;
use std::collections::BTreeMap;

pub use sdrkit_algo_modem::dsss::Integration;

/// Points in an acquisition plot.
const PLOT_POINTS: usize = 256;

/// Samples per independent code cell of every signal kind (about four samples per chip).
const SAMPLES_PER_CELL: f64 = 4.0;

/// A search hit (the `gnss.acq_hit` stream).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AcqHit {
    pub prn: u16,
    pub doppler_hz: f64,
    /// Code phase in band samples: the first code period of the segment starts this many samples after `seg_start`.
    pub code_phase: f64,
    pub seg_start: u64,
    pub ratio: f64,
    pub cn0: f64,
    pub integration_ms: f64,
    /// Whether the peak passed the threshold.
    pub found: bool,
}

/// The acquisition plot of one satellite: its best Doppler's correlation over one code period, 256 points, peak 1.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AcqPlot {
    pub prn: u16,
    pub points: Vec<f32>,
    pub peak_index: usize,
    pub doppler_hz: f64,
    pub ratio: f64,
}

/// Replicas of one signal kind: the code of each PRN sampled at the band rate, with its conjugate spectrum, made once per PRN.
pub struct Replicas {
    pub kind: SignalKind,
    pub n: usize,
    codes: BTreeMap<u16, CodeReplica>,
}

impl Replicas {
    pub fn new(kind: SignalKind) -> Replicas {
        Replicas {
            kind,
            n: kind.period_samples(),
            codes: BTreeMap::new(),
        }
    }
    pub fn has(&self, prn: u16) -> bool {
        self.codes.contains_key(&prn)
    }
    pub fn time(&self, prn: u16) -> Option<&[f32]> {
        self.codes.get(&prn).map(|r| r.samples())
    }
}

/// One satellite's search.
pub struct Search {
    pub prn: u16,
    pub seg_start: u64,
    job: SearchJob,
    /// Whether the job could not start (a short segment): the next step reports a miss.
    failed: bool,
}

impl Search {
    /// Units a job of this grid costs (for progress).
    pub fn cost(&self) -> f64 {
        self.job.cost()
    }
    /// Units spent so far.
    pub fn spent(&self) -> f64 {
        self.job.spent()
    }
    /// Progress 0..1.
    pub fn progress(&self) -> f64 {
        if self.failed {
            1.0
        } else {
            self.job.progress()
        }
    }
    /// The integration of the job.
    pub fn integration(&self) -> Integration {
        self.job.config().integration
    }
}

/// The search engine of one band and signal kind: the shared engine and the replicas, reused from job to job.
pub struct Searcher {
    pub replicas: Replicas,
    engine: CodeSearch,
}

impl Searcher {
    pub fn new(kind: SignalKind) -> Searcher {
        let n = kind.period_samples();
        Searcher {
            replicas: Replicas::new(kind),
            engine: CodeSearch::new(n)
                .expect("every signal kind has a code period of thousands of samples"),
        }
    }
    /// Units spent in all.
    pub fn work(&self) -> f64 {
        self.engine.work()
    }
    /// Makes sure the replica of `prn` exists (chips 0/1).
    pub fn add_code(&mut self, prn: u16, chips: &[u8]) {
        if self.replicas.has(prn) {
            return;
        }
        let kind = self.replicas.kind;
        let r = crate::codes::sample_code(
            chips,
            kind.chip_rate(),
            kind.band_rate(),
            self.replicas.n,
            kind == SignalKind::E1b,
        );
        if let Ok(rep) = self.engine.replica(&r) {
            self.replicas.codes.insert(prn, rep);
        }
    }
    /// Starts a job over the window `centre_hz ± half_hz`, on the segment `seg` (K whole periods) that begins at band index
    /// `seg_start`. `subs`: sub-bin offsets per bin (4 for 1 ms codes, 2 for Galileo). `p_fa`: false alarm per satellite search.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &self,
        prn: u16,
        seg: &[Complex32],
        seg_start: u64,
        centre_hz: f64,
        half_hz: f64,
        subs: usize,
        integ: Integration,
        carrier: f64,
        code_rate_err: f64,
        p_fa: f64,
        reuse: Option<Search>,
    ) -> Search {
        let fs = self.replicas.kind.band_rate();
        let grid = SearchGrid::window(self.replicas.n, centre_hz / fs, half_hz / fs, subs);
        self.make(
            prn,
            seg,
            seg_start,
            grid.ok(),
            integ,
            carrier,
            code_rate_err,
            p_fa,
            reuse,
        )
    }

    /// A small search around one frequency (the confirmation): three frequencies 0.1 bin apart.
    #[allow(clippy::too_many_arguments)]
    pub fn start_confirm(
        &self,
        prn: u16,
        seg: &[Complex32],
        seg_start: u64,
        doppler: f64,
        integ: Integration,
        carrier: f64,
        code_rate_err: f64,
        reuse: Option<Search>,
    ) -> Search {
        let n = self.replicas.n;
        let fs = self.replicas.kind.band_rate();
        let grid = SearchGrid::around(n, doppler / fs, 0.1 / n as f64, 3);
        self.make(
            prn,
            seg,
            seg_start,
            grid.ok(),
            integ,
            carrier,
            code_rate_err,
            1e-3,
            reuse,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn make(
        &self,
        prn: u16,
        seg: &[Complex32],
        seg_start: u64,
        grid: Option<SearchGrid>,
        integ: Integration,
        carrier: f64,
        code_rate_err: f64,
        p_fa: f64,
        reuse: Option<Search>,
    ) -> Search {
        let mut s = reuse.unwrap_or_else(|| Search {
            prn,
            seg_start,
            job: SearchJob::new(),
            failed: false,
        });
        s.prn = prn;
        s.seg_start = seg_start;
        let cfg = SearchConfig {
            integration: integ,
            code_doppler: self.replicas.kind.band_rate() / carrier,
            code_rate_offset: code_rate_err,
            p_fa,
            samples_per_cell: SAMPLES_PER_CELL,
        };
        s.failed = match grid {
            Some(g) => self.engine.start(&mut s.job, seg, &g, &cfg).is_err(),
            None => true,
        };
        s
    }

    /// Spends up to `budget` units on the job (at least one step). Returns the result and its plot when the job is done.
    pub fn step(&mut self, s: &mut Search, budget: &mut f64) -> Option<(AcqHit, AcqPlot)> {
        let mut plot = AcqPlot::default();
        self.step_into(s, budget, &mut plot).map(|h| (h, plot))
    }

    /// The same as [`Searcher::step`], but the plot goes into `plot`, which keeps its buffer from job to job. Returns the result
    /// when the job is done. `plot` changes only then.
    pub fn step_into(
        &mut self,
        s: &mut Search,
        budget: &mut f64,
        plot: &mut AcqPlot,
    ) -> Option<AcqHit> {
        if s.failed || !self.replicas.has(s.prn) {
            s.failed = true;
            return Some(self.miss(s, plot));
        }
        let rep = &self.replicas.codes[&s.prn];
        match self.engine.step(&mut s.job, rep, budget) {
            Ok(Some(r)) => Some(self.hit(s, &r, plot)),
            Ok(None) => None,
            Err(_) => {
                s.failed = true;
                Some(self.miss(s, plot))
            }
        }
    }

    /// The result of a job that could not run: not found, with an empty plot.
    fn miss(&self, s: &Search, plot: &mut AcqPlot) -> AcqHit {
        plot.prn = s.prn;
        plot.points.clear();
        plot.points.resize(PLOT_POINTS, 0.0);
        plot.peak_index = 0;
        plot.doppler_hz = 0.0;
        plot.ratio = 0.0;
        AcqHit {
            prn: s.prn,
            doppler_hz: 0.0,
            code_phase: 0.0,
            seg_start: s.seg_start,
            ratio: 0.0,
            cn0: 0.0,
            integration_ms: 0.0,
            found: false,
        }
    }

    /// The result of a finished job, with its plot: the best Doppler's correlation, 256 points, peak 1.
    fn hit(&self, s: &Search, r: &SearchResult, plot: &mut AcqPlot) -> AcqHit {
        let n = self.replicas.n;
        let fs = self.replicas.kind.band_rate();
        let integ = s.integration();
        let t_coh = n as f64 / fs * integ.coherent as f64;
        let cn0 = cn0_dbhz(r.snr(), t_coh).unwrap_or(0.0).max(0.0);
        let row = s.job.best_powers();
        plot.points.clear();
        plot.points.resize(PLOT_POINTS, 0.0);
        for (k, p) in plot.points.iter_mut().enumerate() {
            let lo = k * n / PLOT_POINTS;
            let hi = ((k + 1) * n / PLOT_POINTS).max(lo + 1).min(n);
            *p = row[lo..hi].iter().cloned().fold(0.0, f32::max);
        }
        let mx = plot.points.iter().cloned().fold(1e-30, f32::max);
        for p in plot.points.iter_mut() {
            *p /= mx;
        }
        let doppler = r.freq * fs;
        plot.prn = s.prn;
        plot.peak_index = r.peak_index * PLOT_POINTS / n;
        plot.doppler_hz = doppler;
        plot.ratio = r.ratio;
        AcqHit {
            prn: s.prn,
            doppler_hz: doppler,
            code_phase: r.code_phase,
            seg_start: s.seg_start,
            ratio: r.ratio,
            cn0,
            integration_ms: integ.periods as f64 * n as f64 / fs * 1e3,
            found: r.detected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    #[test]
    fn finds_a_ca_code_in_noise() {
        let kind = SignalKind::L1Ca;
        let n = kind.period_samples();
        let mut s = Searcher::new(kind);
        let chips = crate::codes::l1_ca(7).unwrap();
        s.add_code(7, &chips);
        let rep = s.replicas.time(7).unwrap().to_vec();
        let (dop, tau) = (2345.0, 1234usize);
        let k = 16;
        let mut seed = 99u64;
        let mut g = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let u1 = (seed >> 11) as f64 / (1u64 << 53) as f64 + 1e-12;
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let u2 = (seed >> 11) as f64 / (1u64 << 53) as f64;
            ((-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()) as f32
        };
        // C/N0 40 dB-Hz with unit noise per component: A = sqrt(1e4 * 2 / fs).
        let amp = (1e4 * 2.0 / kind.band_rate()).sqrt() as f32;
        let seg: Vec<Complex32> = (0..k * n)
            .map(|i| {
                let c = rep[(i + n - tau) % n];
                let ph = TAU * dop * i as f64 / kind.band_rate();
                Complex32::new(g(), g()) + Complex32::from_polar(amp * c, ph as f32)
            })
            .collect();
        let integ = Integration {
            periods: k,
            coherent: 2,
        };
        let mut job = s.start(
            7, &seg, 0, 0.0, 10_000.0, 4, integ, 1575.42e6, 0.0, 1e-3, None,
        );
        let mut res = None;
        while res.is_none() {
            let mut b = 50.0;
            res = s.step(&mut job, &mut b);
        }
        let (hit, plot) = res.unwrap();
        assert!(hit.found, "{hit:?}");
        assert!((hit.doppler_hz - dop).abs() < 100.0, "{hit:?}");
        assert!((hit.code_phase - tau as f64).abs() < 1.0, "{hit:?}");
        assert!((hit.cn0 - 40.0).abs() < 4.0, "{hit:?}");
        assert_eq!(plot.points.len(), 256);
        // An absent code is not found.
        s.add_code(8, &crate::codes::l1_ca(8).unwrap());
        let mut job = s.start(
            8, &seg, 0, 0.0, 10_000.0, 4, integ, 1575.42e6, 0.0, 1e-3, None,
        );
        let mut res = None;
        while res.is_none() {
            let mut b = 1000.0;
            res = s.step(&mut job, &mut b);
        }
        assert!(!res.unwrap().0.found);
        // A short segment is a miss, not a panic.
        let mut job = s.start(
            7,
            &seg[..100],
            0,
            0.0,
            10_000.0,
            4,
            integ,
            1575.42e6,
            0.0,
            1e-3,
            None,
        );
        let mut b = 1000.0;
        assert!(!s.step(&mut job, &mut b).unwrap().0.found);
    }
}
