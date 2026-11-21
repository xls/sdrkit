//! `dvbt.finder`: blind detection of the FFT size and the guard interval from the cyclic prefix. The samples pass through
//! unchanged, and the result leaves as a typed [`Timing`] tag.
//!
//! Standard: ETSI EN 300 744 clause 4.4.

use crate::items::{Link, Restart, Timing};
use crate::params::{Guard, Mode};
use sdrkit_core::publish::Published;
use sdrkit_core::{Complex32, ConfigError, Results, Values};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Port, TagKind, TagSource};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// One hypothesis' result.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Candidate {
    /// FFT size.
    pub fft: usize,
    /// Guard samples.
    pub guard: usize,
    /// Peak of the folded correlation.
    pub peak: f64,
    /// Peak over median.
    pub ratio: f64,
    /// The score.
    pub score: f64,
    /// Phase of the peak within the period, from the window start.
    pub phase: usize,
    /// Fractional carrier offset at the peak, subcarrier spacings.
    pub frac: f64,
}

/// The search itself, independent of the component (also used by the resynchronisation and the scanner).
#[derive(Clone, Debug, Default)]
pub struct Search {
    cp: Vec<[f64; 2]>,
    ep: Vec<f64>,
    acc: Vec<f64>,
    accc: Vec<[f64; 2]>,
    cnt: Vec<u32>,
    scratch: Vec<f64>,
}

impl Search {
    /// Evaluates every guard for FFT size `n` over `y`. Returns the candidates (all, scored, `score` 0 when not a candidate).
    pub fn evaluate(&mut self, y: &[Complex32], n: usize) -> Vec<Candidate> {
        let mut out = Vec::new();
        if y.len() < n + 2 * (n + n / 4) {
            return out;
        }
        let l = y.len() - n;
        self.cp.clear();
        self.ep.clear();
        self.cp.push([0.0, 0.0]);
        self.ep.push(0.0);
        let (mut cr, mut ci, mut e) = (0f64, 0f64, 0f64);
        for i in 0..l {
            let a = y[i];
            let b = y[i + n];
            let p = a * b.conj();
            cr += p.re as f64;
            ci += p.im as f64;
            e += 0.5 * (a.norm_sqr() + b.norm_sqr()) as f64;
            self.cp.push([cr, ci]);
            self.ep.push(e);
        }
        for g in Guard::ALL {
            let gs = g.samples(n);
            let period = n + gs;
            if l < gs + period {
                continue;
            }
            self.acc.clear();
            self.acc.resize(period, 0.0);
            self.accc.clear();
            self.accc.resize(period, [0.0, 0.0]);
            self.cnt.clear();
            self.cnt.resize(period, 0);
            let mut ph = 0usize;
            for i in 0..l - gs {
                let c = [
                    self.cp[i + gs][0] - self.cp[i][0],
                    self.cp[i + gs][1] - self.cp[i][1],
                ];
                let en = self.ep[i + gs] - self.ep[i];
                let rho = if en > 0.0 {
                    (c[0] * c[0] + c[1] * c[1]).sqrt() / en
                } else {
                    0.0
                };
                self.acc[ph] += rho;
                self.accc[ph][0] += c[0];
                self.accc[ph][1] += c[1];
                self.cnt[ph] += 1;
                ph += 1;
                if ph == period {
                    ph = 0;
                }
            }
            self.scratch.clear();
            let mut best = (0usize, 0f64);
            for k in 0..period {
                let v = if self.cnt[k] > 0 {
                    self.acc[k] / self.cnt[k] as f64
                } else {
                    0.0
                };
                self.scratch.push(v);
                if v > best.1 {
                    best = (k, v);
                }
            }
            let mid = period / 2;
            let median = *self.scratch.select_nth_unstable_by(mid, f64::total_cmp).1;
            let ratio = if median > 0.0 { best.1 / median } else { 0.0 };
            let c = self.accc[best.0];
            let frac = -c[1].atan2(c[0]) / (2.0 * std::f64::consts::PI);
            let is = best.1 > 0.12 && ratio > 3.0;
            out.push(Candidate {
                fft: n,
                guard: gs,
                peak: best.1,
                ratio,
                score: if is {
                    best.1 * ratio.min(12.0) / 12.0
                } else {
                    0.0
                },
                phase: best.0,
                frac,
            });
        }
        out
    }
}

/// `dvbt.finder`.
pub struct DvbtFinder {
    link: Arc<Link>,
    ring: Vec<Complex32>,
    base: u64,
    end: u64,
    next_search: u64,
    searching: bool,
    last_win: Option<(usize, usize)>,
    searching_since: u64,
    restart_seen: u32,
    rate: f64,
    search: Search,
    p: Published,
    /// The best candidate of the last search.
    pub last: Option<Candidate>,
    pending: Option<Timing>,
    timings: TagSource<Timing>,
}

impl Default for DvbtFinder {
    fn default() -> Self {
        DvbtFinder::new(Link::new())
    }
}

/// The longest symbol period (8K, 1/4).
const LONGEST: usize = 10240;
/// The analysis window of the 8K hypotheses: 8 periods plus N.
const WINDOW_8K: usize = 8 * LONGEST + 8192;
/// Samples needed before the first search.
const FIRST: usize = 7 * LONGEST;

impl DvbtFinder {
    /// A finder sharing `link`.
    pub fn new(link: Arc<Link>) -> Self {
        DvbtFinder {
            link,
            ring: Vec::new(),
            base: 0,
            end: 0,
            next_search: FIRST as u64,
            searching: true,
            last_win: None,
            searching_since: 0,
            restart_seen: 0,
            rate: 1.0,
            search: Search::default(),
            p: Published::new(&[]),
            last: None,
            pending: None,
            timings: TagSource::new(),
        }
    }

    fn restart(&mut self, at: u64) {
        self.searching = true;
        self.last_win = None;
        self.searching_since = at;
        self.next_search = at.max(self.base) + FIRST as u64;
        self.link.state.store(0, Ordering::Relaxed);
        self.link.detect.store(0, Ordering::Relaxed);
    }

    fn run_search(&mut self) {
        let have = (self.end - self.base) as usize;
        let mut best: Option<(Candidate, u64)> = None;
        let modes: &[Mode] = if self.link.sh.load(Ordering::Relaxed) {
            &Mode::SH
        } else {
            &Mode::ALL
        };
        for &m in modes {
            let n = m.fft();
            let win = (8 * (n + n / 4) + n).min(have);
            let start = self.ring.len() - win;
            let w0 = self.end - win as u64;
            let cands = self.search.evaluate(&self.ring[start..], n);
            for c in cands {
                if c.score > 0.0 && best.as_ref().is_none_or(|b| c.score > b.0.score) {
                    best = Some((c, w0));
                }
            }
        }
        self.last = best.map(|b| b.0);
        match best {
            Some((c, w0)) => {
                self.link.candidate.store(true, Ordering::Relaxed);
                self.link.detect.store(1, Ordering::Relaxed);
                if self.last_win == Some((c.fft, c.guard)) {
                    // Commit: a symbol start between 16 samples after the oldest buffered sample and 6 periods before the newest.
                    let p = (c.fft + c.guard) as u64;
                    let mut s = w0 + c.phase as u64;
                    while s >= self.base + 16 + p {
                        s -= p;
                    }
                    while s < self.base + 16 {
                        s += p;
                    }
                    if s + 6 * p > self.end {
                        s = s.saturating_sub(p * ((s + 6 * p - self.end).div_ceil(p)));
                    }
                    self.pending = Some(Timing {
                        fft: c.fft,
                        guard: c.guard,
                        start: s,
                        frac: c.frac,
                    });
                    self.searching = false;
                    self.link.state.store(1, Ordering::Relaxed);
                    self.p.results_mut().log(format!(
                        "mode found: {} {} (peak {:.2}, {:.1} times the floor)",
                        Mode::from_fft(c.fft).map_or("?", |m| m.text()),
                        Guard::ALL
                            .iter()
                            .find(|g| g.samples(c.fft) == c.guard)
                            .map_or("?", |g| g.text()),
                        c.peak,
                        c.ratio
                    ));
                } else {
                    self.last_win = Some((c.fft, c.guard));
                }
            }
            None => self.last_win = None,
        }
        // Cadence: half the window while young, seven eighths after 2 s without a find.
        let long = (self.end - self.searching_since) as f64 > 2.0 * self.rate;
        let hop = if long {
            WINDOW_8K * 7 / 8
        } else {
            WINDOW_8K / 2
        };
        self.next_search = self.end + hop as u64;
    }
}

impl Component for DvbtFinder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbt.finder",
            vec![Port::iq("in")],
            vec![Port::iq("out").same_format_as(0)],
        )
        .manual_tags()
        .in_place(0, 0)
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate.max(1.0);
        self.ring.clear();
        self.base = 0;
        self.end = 0;
        self.restart(0);
        self.restart_seen = self.link.restart.load(Ordering::Relaxed);
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let x = io.pass_through::<Complex32>(0, 0);
        let n_in = x.len();
        // Buffer the newest samples.
        self.ring.extend_from_slice(x);
        if self.end == 0 && self.ring.len() == x.len() {
            self.base = first;
        }
        self.end = first + n_in as u64;
        let keep = WINDOW_8K + 4096;
        if self.ring.len() > 2 * keep {
            let drop = self.ring.len() - keep;
            self.ring.drain(..drop);
            self.base += drop as u64;
        }
        let out_first = io.output(0).first();
        for i in 0..io.input(0).tags().len() {
            let t = io.input(0).tags()[i].clone();
            let restart = match &t.kind {
                TagKind::Typed(v) => v.get::<Restart>().is_some(),
                TagKind::Retuned { .. } => true,
                _ => false,
            };
            if restart {
                // The samples before a restart (a new mixer offset, a retune) say nothing about the signal from now on.
                if t.at > self.base {
                    let d = ((t.at - self.base) as usize).min(self.ring.len());
                    self.ring.drain(..d);
                    self.base += d as u64;
                }
                self.restart(t.at);
            }
            io.output(0).push_tag(t.at, t.kind);
        }
        let r = self.link.restart.load(Ordering::Relaxed);
        if r != self.restart_seen {
            self.restart_seen = r;
            self.restart(self.end);
        }
        if self.searching
            && self.end >= self.next_search
            && (self.end - self.base) as usize >= FIRST.min(self.end as usize)
        {
            self.run_search();
        }
        if let Some(t) = self.pending.take() {
            let tag = TagKind::Typed(self.timings.tag(t));
            io.output(0).push_tag(out_first, tag);
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{DvbtGenerator, GenParams};
    use crate::params::{Bandwidth, TpsParams};

    #[test]
    fn finds_every_mode_and_guard() {
        for mode in Mode::ALL {
            for guard in Guard::ALL {
                let tps = TpsParams {
                    mode,
                    guard,
                    ..Default::default()
                };
                let mut g = DvbtGenerator::new(GenParams::new(Bandwidth::B8, tps));
                let mut y = Vec::new();
                while y.len() < WINDOW_8K {
                    y.extend_from_slice(g.next_symbol());
                }
                let mut s = Search::default();
                let mut best: Option<Candidate> = None;
                for m in Mode::ALL {
                    for c in s.evaluate(&y[y.len() - WINDOW_8K..], m.fft()) {
                        if c.score > best.map_or(0.0, |b| b.score) {
                            best = Some(c);
                        }
                    }
                }
                let b = best.expect("a candidate");
                assert_eq!(
                    (b.fft, b.guard),
                    (mode.fft(), guard.samples(mode.fft())),
                    "{mode:?} {guard:?}"
                );
            }
        }
    }
}
