//! The burst receiver of `arq_packet`, streaming at 320 ksps (4 samples a symbol): the input converted to that rate, a matched
//! RRC filter, a detector of the preamble that sums the lag-one products of each half against its tone (a carrier offset turns
//! them all by one angle, so the sum is blind to it and costs a few operations a sample), a check by the power of 31 coherent
//! blocks of 8 preamble symbols, then timing and carrier from the whole preamble, the header (Viterbi over the 4 copies, at
//! carrier steps of 10 Hz), a fine fit on the 942 known symbols, the timing drift of a long payload (a sample-clock error)
//! from a straight line through the Gardner timing error of windows of 512 symbols, pilot gains and the LDPC blocks.

use super::{
    header_symbols, parse_header, preamble, scrambler, Header, Profile, HEADER_BITS, HEADER_CODED,
    HEADER_SYMBOLS, KNOWN, N, PREAMBLE, RAMP, SYMBOL_RATE, ZC_LEN,
};
use crate::family::fit::{parabola, phase_slope};
use crate::family::{self, ldpc, qam, read_bits};
use sdrkit_algo_dsp::interp::Cubic;
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_dsp::sinc_table::SincConverter;
use sdrkit_algo_fec::conv::codes::K7_171_133;
use sdrkit_algo_fec::conv::{Ends, Viterbi};
use sdrkit_algo_fec::ldpc::LdpcDecoder;
use sdrkit_algo_modem::Demap;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;
use std::sync::OnceLock;

/// The internal rate of the receiver.
pub const FS: f64 = 320_000.0;
/// Samples a symbol at [`FS`].
pub const SPS: usize = 4;
const HALF: usize = 24;
/// The detector threshold of the lag-one sums (noise gives about 0.008, a clean preamble 1).
pub const DETECT: f32 = 0.022;
/// The threshold of the block check (noise gives about 1, a clean preamble 8).
pub const BLOCK_CHECK: f64 = 1.6;
/// Block LLRs are limited to ±30 and then mapped to ±127.
const LLR_CLIP: f32 = 30.0;
/// Samples on each side of a detector peak that the block check searches.
const CHECK_SPAN: u64 = 320;
/// Symbols of a window of the timing fit.
const TIMING_WIN: usize = 512;
const LAG_SPAN: usize = SPS * (ZC_LEN - 1);
const DET_LEN: usize = SPS * (PREAMBLE - 1);

/// A burst received.
#[derive(Clone, Debug)]
pub struct RxBurst {
    /// The header.
    pub header: Header,
    /// The payload bytes of each block; `None` when its CRC failed.
    pub blocks: Vec<Option<Vec<u8>>>,
    /// The Es/N0 measured on the known symbols, dB.
    pub esn0_db: f64,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// The input time of the first preamble symbol, seconds from the start of the stream.
    pub time: f64,
}

/// The matched filter: the RRC of the transmitter at 4 samples a symbol, unit energy.
fn mf_taps() -> &'static [f32] {
    static H: OnceLock<Vec<f32>> = OnceLock::new();
    H.get_or_init(|| taps(Shape::Rrc { beta: 0.25 }, SPS as f64, 12.0, Norm::Energy).expect("RRC taps"))
}

/// The detector: per sample, the lag-one products of the matched-filter output turned by the tone of each preamble half and
/// summed over the 126 products of the half, in each of the 4 sample phases.
struct Detector {
    tone: Vec<Complex32>,
    w0: Vec<(f64, f64)>,
    w1: Vec<(f64, f64)>,
    q: Vec<f64>,
    r0: [(f64, f64); SPS],
    r1: [(f64, f64); SPS],
    rq: [f64; SPS],
    hist: Vec<((f64, f64), f64)>,
    /// The next sample to take.
    n: u64,
    /// Candidates start at or after this sample.
    from: u64,
    peak: Option<(u64, f32)>,
    above: u64,
}

impl Detector {
    fn new() -> Detector {
        let period = SPS * ZC_LEN;
        let tone = (0..period)
            .map(|n| {
                let a = TAU * (super::ROOTS[0] as f64) * n as f64 / period as f64;
                Complex32::new(a.cos() as f32, a.sin() as f32)
            })
            .collect();
        Detector {
            tone,
            w0: vec![(0.0, 0.0); LAG_SPAN],
            w1: vec![(0.0, 0.0); LAG_SPAN],
            q: vec![0.0; LAG_SPAN],
            r0: [(0.0, 0.0); SPS],
            r1: [(0.0, 0.0); SPS],
            rq: [0.0; SPS],
            hist: vec![((0.0, 0.0), 0.0); period],
            n: 0,
            from: 0,
            peak: None,
            above: 0,
        }
    }

    /// Starts again at sample `n`; candidates start at `from` or later.
    fn restart(&mut self, n: u64, from: u64) {
        self.w0.fill((0.0, 0.0));
        self.w1.fill((0.0, 0.0));
        self.q.fill(0.0);
        self.r0 = [(0.0, 0.0); SPS];
        self.r1 = [(0.0, 0.0); SPS];
        self.rq = [0.0; SPS];
        self.hist.fill(((0.0, 0.0), 0.0));
        self.n = n;
        self.from = from;
        self.peak = None;
    }

    /// Takes sample `self.n` (with `z` the filter output from absolute sample `base`); returns a candidate start when a peak
    /// of the metric is final.
    fn step(&mut self, z: &[Complex32], base: u64) -> Option<u64> {
        let n = self.n;
        self.n += 1;
        let i = (n - base) as usize;
        let e = z[i] * z[i - SPS].conj();
        let t = self.tone[(n % self.tone.len() as u64) as usize];
        let a = e * t;
        let b = e * t.conj();
        let a = (f64::from(a.re), f64::from(a.im));
        let b = (f64::from(b.re), f64::from(b.im));
        let p = (n % SPS as u64) as usize;
        let s = (n % LAG_SPAN as u64) as usize;
        let qa = f64::from(e.norm_sqr());
        self.r0[p].0 += a.0 - self.w0[s].0;
        self.r0[p].1 += a.1 - self.w0[s].1;
        self.r1[p].0 += b.0 - self.w1[s].0;
        self.r1[p].1 += b.1 - self.w1[s].1;
        self.rq[p] += qa - self.q[s];
        self.w0[s] = a;
        self.w1[s] = b;
        self.q[s] = qa;
        let h = (n % self.hist.len() as u64) as usize;
        let (s0, q0) = self.hist[h];
        self.hist[h] = (self.r0[p], self.rq[p]);
        let s1 = self.r1[p];
        let q1 = self.rq[p];
        let n0 = n.checked_sub(DET_LEN as u64)?;
        if n0 < self.from {
            return None;
        }
        let m = ((s0.0 * s0.0 + s0.1 * s0.1 + s1.0 * s1.0 + s1.1 * s1.1) / (126.0 * (q0 + q1)).max(1e-30)) as f32;
        // A peak is the largest value of a run above the threshold; it is final CHECK_SPAN samples after the run ends (the metric
        // falls slowly away from the start, by one product in 126 a symbol).
        if m > DETECT && m <= 1.5 {
            self.above = n0;
            if self.peak.is_none_or(|(_, pm)| m > pm) {
                self.peak = Some((n0, m));
            }
        }
        let mut out = None;
        if let Some((p0, _)) = self.peak {
            if n0 > self.above + CHECK_SPAN || n0 > p0 + 2 * DET_LEN as u64 {
                out = Some(p0);
                self.peak = None;
            }
        }
        out
    }
}

enum State {
    Search,
    /// Starts that passed the block check, best first, and where the search goes on when none gives a header.
    Header(Vec<u64>, u64),
    /// A header was read; the burst is decoded when its last sample has arrived.
    Payload(Box<Found>, u64),
}

/// What the known symbols gave.
struct Found {
    header: Header,
    /// The time of preamble symbol 0, samples.
    tau: f64,
    /// Carrier, radians a symbol.
    w: f64,
    /// The complex gain at known symbol 471.
    a: Complex32,
    /// Noise over |a|^2.
    n0: f64,
    esn0_db: f64,
}

/// The streaming receiver.
pub struct Receiver {
    conv: Option<SincConverter>,
    scratch: Vec<Complex32>,
    x: Vec<Complex32>,
    z: Vec<Complex32>,
    /// The absolute sample of `x[0]` and `z[0]`.
    base: u64,
    det: Detector,
    state: State,
    vit: Viterbi,
    decs: Vec<(usize, LdpcDecoder)>,
    /// Bursts whose header was read.
    pub headers: u64,
    /// Peaks of the detector.
    pub candidates: u64,
    /// Peaks that passed the block check.
    pub checked: u64,
}

impl Receiver {
    /// A receiver of input at `rate` samples a second.
    pub fn new(rate: f64) -> Receiver {
        let conv = if (rate - FS).abs() > 1e-6 { SincConverter::new(rate, FS).ok() } else { None };
        let mut r = Receiver {
            conv,
            scratch: Vec::new(),
            x: Vec::new(),
            z: Vec::new(),
            base: 0,
            det: Detector::new(),
            state: State::Search,
            vit: Viterbi::new(K7_171_133, HEADER_BITS + 6).expect("header Viterbi"),
            decs: Vec::new(),
            headers: 0,
            candidates: 0,
            checked: 0,
        };
        r.reset();
        r
    }

    /// Back to the start of a stream.
    pub fn reset(&mut self) {
        if let Some(c) = self.conv.as_mut() {
            c.reset();
        }
        self.x = vec![Complex32::new(0.0, 0.0); HALF];
        self.z = vec![Complex32::new(0.0, 0.0); HALF];
        self.base = 0;
        self.det.restart(SPS as u64, 0);
        self.state = State::Search;
    }

    fn zend(&self) -> u64 {
        self.base + self.z.len() as u64
    }

    /// Takes input samples and appends the bursts they complete.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<RxBurst>) {
        match self.conv.as_mut() {
            Some(c) => {
                self.scratch.clear();
                c.process(input, &mut self.scratch);
                self.x.extend_from_slice(&self.scratch);
            }
            None => self.x.extend_from_slice(input),
        }
        self.filter();
        self.run(out);
    }

    /// Ends the stream: the converter and filter are flushed with zeros and a burst in progress is decoded with what arrived.
    pub fn finish(&mut self, out: &mut Vec<RxBurst>) {
        let pad = vec![Complex32::new(0.0, 0.0); 4096];
        if let Some(c) = self.conv.as_mut() {
            self.scratch.clear();
            c.flush(&mut self.scratch);
            self.x.extend_from_slice(&self.scratch);
        }
        self.x.extend_from_slice(&pad);
        self.filter();
        self.run(out);
        if let State::Payload(f, _) = std::mem::replace(&mut self.state, State::Search) {
            out.push(self.payload(&f));
        }
        if let State::Header(c, _) = std::mem::replace(&mut self.state, State::Search) {
            if let Some(f) = c.iter().find_map(|&n0| self.header(n0)) {
                out.push(self.payload(&f));
            }
        }
    }

    fn filter(&mut self) {
        let h = mf_taps();
        let from = self.z.len();
        let to = self.x.len().saturating_sub(HALF);
        for i in from..to {
            let w = &self.x[i - HALF..=i + HALF];
            let mut acc = Complex32::new(0.0, 0.0);
            for (v, &t) in w.iter().zip(h) {
                acc += v * t;
            }
            self.z.push(acc);
        }
    }

    fn run(&mut self, out: &mut Vec<RxBurst>) {
        loop {
            let end = self.zend();
            match self.state {
                State::Search => {
                    let mut found = None;
                    while self.det.n < end {
                        if let Some(n0) = self.det.step(&self.z, self.base) {
                            self.candidates += 1;
                            let c = self.block_check(n0);
                            if !c.is_empty() {
                                self.checked += 1;
                                found = Some((c, n0 + CHECK_SPAN));
                                break;
                            }
                        }
                    }
                    match found {
                        Some((c, r)) => self.state = State::Header(c, r),
                        None => break,
                    }
                }
                State::Header(ref c, resume) => {
                    let last = c.iter().copied().max().unwrap_or(0);
                    let need = last + (SPS * (KNOWN + 8)) as u64 + 2 * HALF as u64;
                    if end < need {
                        break;
                    }
                    let c = c.clone();
                    match c.iter().find_map(|&n0| self.header(n0)) {
                        Some(f) => {
                            self.headers += 1;
                            let p = f.header.profile();
                            let np = super::payload_symbols(usize::from(f.header.blocks) * p.symbols());
                            let last = f.tau + (SPS * (KNOWN + np + RAMP)) as f64;
                            // A sample-clock error of up to 50 ppm moves the end of the burst.
                            let drift = (last - f.tau) * 50e-6;
                            self.state = State::Payload(Box::new(f), (last + drift).ceil() as u64 + 8);
                        }
                        None => self.resume(resume),
                    }
                }
                State::Payload(_, need) => {
                    if end < need {
                        break;
                    }
                    if let State::Payload(f, need) = std::mem::replace(&mut self.state, State::Search) {
                        out.push(self.payload(&f));
                        self.resume(need);
                    }
                }
            }
        }
        self.trim();
    }

    fn resume(&mut self, at: u64) {
        let start = at.saturating_sub(DET_LEN as u64 + 2 * SPS as u64).max(self.base + SPS as u64);
        self.det.restart(start, at);
        self.state = State::Search;
    }

    fn trim(&mut self) {
        let keep = match &self.state {
            State::Search => self.det.n.saturating_sub(2 * (DET_LEN as u64 + CHECK_SPAN) + 64),
            State::Header(c, _) => c.iter().copied().min().unwrap_or(0).saturating_sub(2 * DET_LEN as u64),
            State::Payload(f, _) => (f.tau as u64).saturating_sub((DET_LEN + 64) as u64),
        };
        let drop = keep.saturating_sub(self.base) as usize;
        if drop > 1 << 16 {
            let drop = drop.min(self.z.len());
            self.x.drain(..drop);
            self.z.drain(..drop);
            self.base += drop as u64;
        }
    }

    /// The filter output at absolute sample position `pos` by cubic interpolation (0 outside the buffer).
    fn at(&self, pos: f64) -> Complex32 {
        let fl = pos.floor();
        let i = fl as i64 - self.base as i64;
        if i < 1 || i as usize + 2 >= self.z.len() {
            return Complex32::new(0.0, 0.0);
        }
        let w = Cubic::weights((pos - fl) as f32);
        let b = (i - 1) as usize;
        self.z[b] * w[0] + self.z[b + 1] * w[1] + self.z[b + 2] * w[2] + self.z[b + 3] * w[3]
    }

    /// Symbols `from .. from + n` of a burst whose preamble starts at `tau`, with the carrier `w` (radians a symbol) removed.
    fn symbols(&self, tau: f64, w: f64, from: usize, n: usize) -> Vec<Complex32> {
        (from..from + n)
            .map(|k| {
                let a = -w * k as f64;
                self.at(tau + (SPS * k) as f64) * Complex32::new(a.cos() as f32, a.sin() as f32)
            })
            .collect()
    }

    /// Symbols `from .. from + n` of a burst whose preamble starts at `tau`, read at `tau + 4 k + a + b (k - KNOWN)` samples
    /// (a timing that drifts with a sample-clock error), with the carrier `w` removed.
    fn symbols_timed(&self, tau: f64, w: f64, from: usize, n: usize, a: f64, b: f64) -> Vec<Complex32> {
        (from..from + n)
            .map(|k| {
                let ph = -w * k as f64;
                let d = a + b * (k as f64 - KNOWN as f64);
                self.at(tau + (SPS * k) as f64 + d) * Complex32::new(ph.cos() as f32, ph.sin() as f32)
            })
            .collect()
    }

    /// The timing drift over the `np` payload symbols of a burst: the zero of the Gardner timing error of each window of
    /// [`TIMING_WIN`] symbols, found near the timing the earlier windows predict, and a straight line through them and the
    /// timing of the known symbols. Returns `(a, b)`: the timing of payload symbol `k` is `a + b (k - KNOWN)` samples from its
    /// nominal place. A payload shorter than 4 windows gives `(0, 0)`.
    fn timing_fit(&self, tau: f64, w: f64, np: usize) -> (f64, f64) {
        let wins = np / TIMING_WIN;
        if wins < 4 {
            return (0.0, 0.0);
        }
        // The Gardner error of window `j` with the timing moved by `d` samples.
        let gardner = |j: usize, d: f64| -> f64 {
            let k0 = KNOWN + j * TIMING_WIN;
            let at = |p: f64| {
                let ph = -w * p;
                self.at(tau + SPS as f64 * p + d) * Complex32::new(ph.cos() as f32, ph.sin() as f32)
            };
            let mut prev = at(k0 as f64);
            let mut e = 0.0f64;
            for k in k0 + 1..k0 + TIMING_WIN {
                let cur = at(k as f64);
                let mid = at(k as f64 - 0.5);
                e += f64::from(((prev - cur) * mid.conj()).re);
                prev = cur;
            }
            e
        };
        // Points (payload symbol, timing); the known symbols hold the timing at 0.
        let mut pts: Vec<(f64, f64)> = vec![(-((KNOWN / 2) as f64), 0.0)];
        let line = |pts: &[(f64, f64)]| -> (f64, f64) {
            let n = pts.len() as f64;
            let (sx, sy) = pts.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
            let (mx, my) = (sx / n, sy / n);
            let sxx: f64 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
            let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
            let b = if sxx > 0.0 { sxy / sxx } else { 0.0 };
            (my - b * mx, b)
        };
        for j in 0..wins {
            let centre = (j * TIMING_WIN + TIMING_WIN / 2) as f64;
            let (a, b) = line(&pts);
            let p = if pts.len() >= 3 { a + b * centre } else { pts.last().map_or(0.0, |q| q.1) };
            let (em, e0, ep) = (gardner(j, p - 1.0), gardner(j, p), gardner(j, p + 1.0));
            let slope = (ep - em) / 2.0;
            if slope.abs() < 1e-12 {
                continue;
            }
            // The zero of the parabola through the three errors nearest to the prediction.
            let curv = (ep + em) / 2.0 - e0;
            let mut d = -e0 / slope;
            if curv.abs() > 1e-12 {
                let disc = slope * slope - 4.0 * curv * e0;
                if disc >= 0.0 {
                    let r = disc.sqrt();
                    let (r1, r2) = ((-slope + r) / (2.0 * curv), (-slope - r) / (2.0 * curv));
                    d = if r1.abs() < r2.abs() { r1 } else { r2 };
                }
            }
            if d.abs() <= 1.0 {
                pts.push((centre, p + d));
            }
        }
        if pts.len() < 4 {
            return (0.0, 0.0);
        }
        line(&pts)
    }

    /// The block check within [`CHECK_SPAN`] samples of `n0`: the starts of the 4 best peaks of the metric that pass, best
    /// first.
    fn block_check(&self, n0: u64) -> Vec<u64> {
        let lo = n0.saturating_sub(CHECK_SPAN).max(self.det.from).max(self.base);
        let metric = |n: u64| -> Option<f64> {
            let pre = preamble();
            let i0 = (n - self.base) as usize;
            if i0 + SPS * 248 > self.z.len() {
                return None;
            }
            let mut sum = 0.0;
            let mut e = 0.0;
            for j in 0..31 {
                let mut b = Complex32::new(0.0, 0.0);
                for k in 0..8 {
                    let v = self.z[i0 + SPS * (8 * j + k)];
                    b += v * pre[8 * j + k].conj();
                    e += f64::from(v.norm_sqr());
                }
                sum += f64::from(b.norm_sqr());
            }
            Some(sum / e.max(1e-30))
        };
        // Every other sample, then the neighbours of the best peaks.
        let m: Vec<f64> = (lo..=n0 + CHECK_SPAN).step_by(2).map_while(metric).collect();
        let mut peaks: Vec<(f64, u64)> = (0..m.len())
            .filter(|&i| (i == 0 || m[i] >= m[i - 1]) && (i + 1 == m.len() || m[i] > m[i + 1]))
            .map(|i| (m[i], lo + 2 * i as u64))
            .collect();
        peaks.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut out: Vec<(f64, u64)> = peaks
            .into_iter()
            .take(4)
            .filter_map(|(v, n)| {
                [n.saturating_sub(1).max(lo), n + 1]
                    .into_iter()
                    .filter_map(|k| metric(k).map(|w| (w, k)))
                    .chain([(v, n)])
                    .max_by(|a, b| a.0.total_cmp(&b.0))
            })
            .filter(|p| p.0 > BLOCK_CHECK)
            .collect();
        out.sort_by(|a, b| b.0.total_cmp(&a.0));
        out.into_iter().map(|p| p.1).collect()
    }

    /// Timing, carrier and the header of a candidate.
    fn header(&mut self, n0: u64) -> Option<Found> {
        let pre = preamble();
        let corr = |s: &Self, n: f64, w: f64, from: usize, to: usize| -> Complex32 {
            let y = s.symbols(n, w, from, to - from);
            y.iter().zip(&pre[from..to]).map(|(a, b)| a * b.conj()).sum()
        };
        // The carrier from the products of neighbouring block sums.
        let block_w = |s: &Self, n: f64| -> f64 {
            let y = s.symbols(n, 0.0, 0, 248);
            let b: Vec<Complex32> =
                (0..31).map(|j| (0..8).map(|k| y[8 * j + k] * pre[8 * j + k].conj()).sum()).collect();
            let d: Complex32 = b.windows(2).map(|p| p[1] * p[0].conj()).sum();
            f64::from(d.arg()) / 8.0
        };
        let search = |s: &Self, w: f64| -> (f64, f64) {
            let mut best = (f64::MIN, n0 as f64);
            for d in -(SPS as i64)..=SPS as i64 {
                let n = n0 as f64 + d as f64;
                let v = f64::from(corr(s, n, w, 0, PREAMBLE).norm_sqr());
                if v > best.0 {
                    best = (v, n);
                }
            }
            best
        };
        let wb = block_w(self, n0 as f64);
        let (m0, p0) = search(self, 0.0);
        let (m1, p1) = search(self, wb);
        let (mut n, mut w) = if m1 > 1.5 * m0 { (p1, block_w(self, p1)) } else { (p0, 0.0) };
        if m1 > 1.5 * m0 {
            n = search(self, w).1;
        }
        let f = |d: f64| f64::from(corr(self, n + d, w, 0, PREAMBLE).norm_sqr());
        n += parabola(f(-1.0), f(0.0), f(1.0));
        // The carrier from the phase slope over the whole preamble.
        let y = self.symbols(n, w, 0, PREAMBLE);
        let v: Vec<(f64, Complex32)> =
            y.iter().zip(pre).enumerate().map(|(k, (a, b))| (k as f64 - 126.5, a * b.conj())).collect();
        w += phase_slope(&v, 0.0, TAU * 150.0 / SYMBOL_RATE, TAU * 2.0 / SYMBOL_RATE).0;
        let step = TAU * 10.0 / SYMBOL_RATE;
        let mut header = None;
        for t in [0.0, 1.0, -1.0, 2.0, -2.0, 3.0, -3.0] {
            let wt = w + t * step;
            if let Some(h) = self.try_header(n, wt) {
                header = Some((h, wt));
                break;
            }
        }
        let (header, w) = header?;
        Some(self.fine(header, n, w))
    }

    fn try_header(&mut self, n: f64, w: f64) -> Option<Header> {
        let pre = preamble();
        let y = self.symbols(n, w, 0, KNOWN);
        let a: Complex32 = y[..PREAMBLE].iter().zip(pre).map(|(v, p)| v * p.conj()).sum::<Complex32>() / PREAMBLE as f32;
        let amp2 = f64::from(a.norm_sqr()).max(1e-30);
        let resid = y[..PREAMBLE].iter().zip(pre).map(|(v, p)| f64::from((v - a * p).norm_sqr())).sum::<f64>()
            / PREAMBLE as f64;
        let n0 = (resid / amp2).max(1e-3);
        let mut llr = [0f32; HEADER_CODED];
        for i in 0..HEADER_SYMBOLS {
            let v = y[PREAMBLE + i] / a;
            llr[i % HEADER_CODED] += (4.0 * f64::from(v.re) / n0) as f32;
        }
        let mean = llr.iter().map(|v| v.abs()).sum::<f32>() / HEADER_CODED as f32;
        let q: Vec<i8> = llr.iter().map(|v| (v * 20.0 / mean.max(1e-12)).round().clamp(-127.0, 127.0) as i8).collect();
        let mut bits = [0u8; HEADER_BITS];
        self.vit.decode(&q, Ends::Zero, &mut bits).ok()?;
        let h = parse_header(&bits)?;
        h.plausible().then_some(h)
    }

    /// Timing, carrier, gain and noise from the 942 known symbols.
    fn fine(&self, header: Header, n: f64, w: f64) -> Found {
        let mut known = preamble().to_vec();
        known.extend(header_symbols(&header));
        let metric = |s: &Self, tau: f64| -> f64 {
            let y = s.symbols(tau, w, 0, KNOWN);
            f64::from(y.iter().zip(&known).map(|(a, b)| a * b.conj()).sum::<Complex32>().norm_sqr())
        };
        // Timing within ±0.5 sample.
        let mut best = (f64::MIN, 0.0);
        let mut vals = [0.0; 9];
        for (i, v) in vals.iter_mut().enumerate() {
            let d = (i as f64 - 4.0) * 0.125;
            *v = metric(self, n + d);
            if *v > best.0 {
                best = (*v, d);
            }
        }
        let bi = ((best.1 / 0.125) + 4.0).round() as usize;
        let mut tau = n + best.1;
        if bi > 0 && bi < 8 {
            tau += 0.125 * parabola(vals[bi - 1], vals[bi], vals[bi + 1]);
        }
        let y = self.symbols(tau, w, 0, KNOWN);
        let centre = (KNOWN / 2) as f64;
        let v: Vec<(f64, Complex32)> =
            y.iter().zip(&known).enumerate().map(|(k, (a, b))| (k as f64 - centre, a * b.conj())).collect();
        let st = TAU / SYMBOL_RATE;
        let (dw, _) = phase_slope(&v, 0.0, 24.0 * st, st);
        let w = w + dw;
        let y = self.symbols(tau, w, 0, KNOWN);
        // With the carrier removed, the gain is the mean over the known symbols.
        let a: Complex32 = y.iter().zip(&known).map(|(v, p)| v * p.conj()).sum::<Complex32>() / KNOWN as f32;
        let noise = y.iter().zip(&known).map(|(v, p)| f64::from((v - a * p).norm_sqr())).sum::<f64>() / KNOWN as f64;
        let amp2 = f64::from(a.norm_sqr()).max(1e-30);
        Found {
            header,
            tau,
            w,
            a,
            n0: (noise / amp2).max(2e-4),
            esn0_db: 10.0 * (amp2 / noise.max(1e-30)).log10(),
        }
    }

    fn decoder(&mut self, rate: usize) -> &mut LdpcDecoder {
        if let Some(i) = self.decs.iter().position(|d| d.0 == rate) {
            return &mut self.decs[i].1;
        }
        let c = ldpc::code(N, rate).expect("a code of the family");
        self.decs.push((rate, c.decoder()));
        &mut self.decs.last_mut().expect("just pushed").1
    }

    /// The blocks of a burst whose header was read.
    fn payload(&mut self, f: &Found) -> RxBurst {
        let p: Profile = f.header.profile();
        let nb = usize::from(f.header.blocks);
        let d = nb * p.symbols();
        let np = super::payload_symbols(d);
        let (a, b) = self.timing_fit(f.tau, f.w, np);
        let y: Vec<Complex32> = self.symbols_timed(f.tau, f.w, KNOWN, np, a, b).into_iter().map(|v| v / f.a).collect();
        let pilots: Vec<(f64, Complex32)> = (0..np / 32).map(|j| (j as f64, y[32 * j + 31])).collect();
        let theta = if pilots.len() >= 8 {
            let step = 0.001f64.min(0.25 * TAU / pilots.len() as f64);
            phase_slope(&pilots, 0.0, 0.02, step).0
        } else {
            0.0
        };
        let wwin = match p.bits {
            1 | 2 => 48,
            4 => 16,
            6 => 8,
            _ => 5,
        };
        let q: Vec<Complex32> = pilots
            .iter()
            .map(|&(j, v)| {
                let a = -theta * j;
                v * Complex32::new(a.cos() as f32, a.sin() as f32)
            })
            .collect();
        let gains: Vec<Complex32> = (0..q.len())
            .map(|j| {
                let lo = j.saturating_sub(wwin);
                let hi = (j + wwin + 1).min(q.len());
                let g = q[lo..hi].iter().sum::<Complex32>() / (hi - lo) as f32;
                if g.norm() < 1e-3 {
                    Complex32::new(1.0, 0.0)
                } else {
                    g
                }
            })
            .collect();
        let mut data = Vec::with_capacity(d);
        for (i, &v) in y.iter().enumerate() {
            if i % 32 == 31 {
                continue;
            }
            let pos = (i as f64 - 31.0) / 32.0;
            let g = if gains.is_empty() {
                Complex32::new(1.0, 0.0)
            } else if pos <= 0.0 {
                gains[0]
            } else if pos >= (gains.len() - 1) as f64 {
                gains[gains.len() - 1]
            } else {
                let j = pos.floor() as usize;
                let t = (pos - j as f64) as f32;
                gains[j] * (1.0 - t) + gains[j + 1] * t
            };
            let a = -theta * pos;
            data.push(v * Complex32::new(a.cos() as f32, a.sin() as f32) / g);
        }
        let c = qam::constellation(p.bits).expect("a family set");
        let k = p.k();
        let per = p.symbols();
        let mut blocks = Vec::with_capacity(nb);
        let mut llr = vec![0f32; per * p.bits];
        for b in 0..nb {
            let _ = c.demap_block(&data[b * per..(b + 1) * per], f.n0 as f32, Demap::MaxLog, &mut llr);
            let scr = scrambler(b);
            let l: Vec<i8> = (0..N)
                .map(|i| {
                    let v = if scr[i] == 1 { -llr[i] } else { llr[i] };
                    (v.clamp(-LLR_CLIP, LLR_CLIP) * (127.0 / LLR_CLIP)).round() as i8
                })
                .collect();
            let mut out = vec![0u8; N];
            let ok = self.decoder(p.rate).decode(&l, 50, &mut out).is_ok();
            let bytes = family::pack(&out[..k - 16]);
            let crc = read_bits(&out, k - 16, 16) as u16;
            blocks.push((ok && family::crc16(&bytes) == crc).then_some(bytes));
        }
        RxBurst {
            header: f.header.clone(),
            blocks,
            esn0_db: f.esn0_db,
            offset_hz: f.w * SYMBOL_RATE / TAU,
            time: (f.tau - HALF as f64) / FS,
        }
    }
}
