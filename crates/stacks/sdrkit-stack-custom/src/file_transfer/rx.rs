//! The frame receiver of `file_transfer`. For each width it searches, the input is converted to 4 samples a symbol by the
//! rational resampler of the family. A detector runs the lag-251-symbol covariance of the preamble over a window of 261
//! symbols on a 4-sample running sum of the input (the mean is taken out, so a constant input gives nothing); each run above
//! 0.18 gives a candidate, unless the input there is narrower than the width (a signal of a smaller width). The matched RRC filter is evaluated only where symbols are read, at any fractional position, from
//! a table of 64 shifted copies. At a candidate: the timing by the power of 32 blocks of 16 preamble symbols and the carrier
//! from the phase step between the blocks, the timing again on the whole preamble at whole and tenth samples, the carrier
//! from the two copies of the sequence, the gain and noise by least squares, the header (Viterbi), then the payload with
//! pilot gains smoothed by a local straight-line fit, and the LDPC blocks with their CRC-32. After a frame without the last
//! flag, the next frame of the burst is looked for right behind it.

use super::{
    header_symbols, parse_header, pilots, pilots_of, preamble, scramble, FrameHeader, FrameKind, Width, FIXED, HEADER_INFO,
    HEADER_SYMBOLS, N, PREAMBLE, SPS, WIDTHS, ZC_LEN,
};
use crate::family::{self, ldpc, qam};
use sdrkit_algo_dsp::blackman_rational::BlackmanRational;
use sdrkit_algo_fec::conv::codes::K7_171_133;
use sdrkit_algo_fec::conv::{Ends, Viterbi};
use sdrkit_algo_fec::ldpc::LdpcDecoder;
use sdrkit_algo_modem::Demap;
use sdrkit_core::Complex32;
use std::f64::consts::{PI, TAU};
use std::sync::OnceLock;

/// A frame received.
#[derive(Clone, Debug)]
pub struct RxFrame {
    /// The width it was found at.
    pub width: Width,
    /// The header.
    pub header: FrameHeader,
    /// The payload bytes of each block; `None` when its CRC failed.
    pub blocks: Vec<Option<Vec<u8>>>,
    /// The Es/N0 from the pilots, dB.
    pub esn0_db: f64,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// The input time of the first preamble symbol, seconds from the start of the stream.
    pub time: f64,
}

/// Fractional positions of the matched-filter table.
const PHASES: usize = 64;
/// Taps of the matched filter.
const MF_LEN: usize = 81;
const MF_HALF: usize = 40;
/// The detector window, samples (261 symbols).
const WIN: usize = 4 * 261;
/// The detector lag, samples (251 symbols).
const LAG: usize = 4 * ZC_LEN;
/// Samples of the 4-sample running sum beyond its start.
const BOX: usize = 4;
/// The detector threshold.
const DETECT: f64 = 0.18;
/// The least normalised correlation of an accepted preamble.
const ACCEPT: f64 = 0.10;
/// Samples searched on each side of a detector peak.
const SPAN: i64 = 64;
/// Samples searched on each side of the expected start of the next frame of a burst.
const NEXT_SPAN: i64 = 8;
/// LLRs are limited to ±30 and then mapped to ±127.
const LLR_CLIP: f32 = 30.0;
/// LDPC iterations.
const ITERATIONS: u32 = 50;
/// The largest covariance at a lag of one symbol, over the variance, of the input of a candidate.
const NARROW: f32 = 0.4;
/// Samples of a detector run after which a candidate is taken anyway.
const RUN_CAP: u64 = 2 * WIN as u64;
/// Samples for which a failed candidate keeps the same run from giving another.
const QUIET: u64 = 32 * WIN as u64;

/// The root-raised-cosine pulse, roll-off 0.25, at `t` symbols from its centre.
fn rrc(t: f64) -> f64 {
    let b = 0.25;
    if t.abs() < 1e-9 {
        return 1.0 - b + 4.0 * b / PI;
    }
    if ((4.0 * b * t).abs() - 1.0).abs() < 1e-9 {
        return b / 2f64.sqrt() * ((1.0 + 2.0 / PI) * (PI / (4.0 * b)).sin() + (1.0 - 2.0 / PI) * (PI / (4.0 * b)).cos());
    }
    ((PI * t * (1.0 - b)).sin() + 4.0 * b * t * (PI * t * (1.0 + b)).cos()) / (PI * t * (1.0 - (4.0 * b * t).powi(2)))
}

/// The matched filter at the 64 fractional positions: row `p` holds the taps that give the output at `i + p / 64` from the
/// inputs `i - 40 ..= i + 40`; unit energy.
fn mf_table() -> &'static [f32] {
    static T: OnceLock<Vec<f32>> = OnceLock::new();
    T.get_or_init(|| {
        let e: f64 = (0..MF_LEN).map(|i| rrc((i as f64 - MF_HALF as f64) / SPS as f64).powi(2)).sum();
        let g = 1.0 / e.sqrt();
        let mut t = Vec::with_capacity(PHASES * MF_LEN);
        for p in 0..PHASES {
            let f = p as f64 / PHASES as f64;
            t.extend((0..MF_LEN).map(|i| (g * rrc((i as f64 - MF_HALF as f64 - f) / SPS as f64)) as f32));
        }
        t
    })
}

/// The matched-filter output of `x` (sample `j` is absolute sample `base + j`) at absolute position `pos`; 0 where the
/// taps leave the buffer.
fn mf_at(x: &[Complex32], base: u64, pos: f64) -> Complex32 {
    let fl = pos.floor();
    let mut i = fl as i64;
    let mut p = ((pos - fl) * PHASES as f64).round() as usize;
    if p == PHASES {
        p = 0;
        i += 1;
    }
    let start = i - MF_HALF as i64 - base as i64;
    if start < 0 || start as usize + MF_LEN > x.len() {
        return Complex32::new(0.0, 0.0);
    }
    let xs = &x[start as usize..start as usize + MF_LEN];
    let ts = &mf_table()[p * MF_LEN..(p + 1) * MF_LEN];
    let (mut re, mut im) = (0f32, 0f32);
    for (v, &t) in xs.iter().zip(ts) {
        re += v.re * t;
        im += v.im * t;
    }
    Complex32::new(re, im)
}

fn rot(a: f64) -> Complex32 {
    Complex32::new(a.cos() as f32, a.sin() as f32)
}

/// The running sums of the detector over the window that starts at `n`.
#[derive(Clone, Copy, Debug, Default)]
struct Sums {
    c: (f64, f64),
    m1: (f64, f64),
    m2: (f64, f64),
    e1: f64,
    e2: f64,
}

enum State {
    /// Looking for a preamble.
    Search,
    /// Synchronising on a candidate start `n0` within `span` samples; `next` when it is the expected next frame of a burst.
    Sync { n0: i64, span: i64, next: bool },
    /// A header was read; the frame is decoded once sample `need` has arrived.
    Payload { found: Box<Found>, need: u64 },
}

/// What the preamble and header gave.
struct Found {
    header: FrameHeader,
    /// The position of preamble symbol 0, samples.
    tau: f64,
    /// Carrier, radians a symbol.
    w: f64,
}

/// The receiver of one width.
pub struct Lane {
    width: Width,
    conv: Option<BlackmanRational>,
    scratch: Vec<Complex32>,
    x: Vec<Complex32>,
    /// The 4-sample running sums of `x`: `bx[i]` is `x[i] + .. + x[i + 3]`.
    bx: Vec<Complex32>,
    /// The absolute sample of `x[0]` and `bx[0]`.
    base: u64,
    /// The start of the next detector window.
    det: u64,
    sums: Sums,
    fresh: bool,
    steps: u32,
    /// The current run: its start, its peak position and value.
    run: Option<(u64, u64, f64)>,
    quiet_until: u64,
    avg: f64,
    state: State,
    vit: Viterbi,
    decs: Vec<(usize, LdpcDecoder)>,
    llr: Vec<f32>,
    q: Vec<i8>,
    hard: Vec<u8>,
    /// Detector runs that gave a candidate.
    pub candidates: u64,
    /// Headers read.
    pub headers: u64,
}

impl Lane {
    /// A receiver of `width` for input converted by `up / down` (identity when equal).
    pub fn new(width: Width, up: u64, down: u64) -> Lane {
        let conv = if up == down { None } else { BlackmanRational::new(up, down).ok() };
        Lane {
            width,
            conv,
            scratch: Vec::new(),
            x: Vec::new(),
            bx: Vec::new(),
            base: 0,
            det: 0,
            sums: Sums::default(),
            fresh: true,
            steps: 0,
            run: None,
            quiet_until: 0,
            avg: 0.0,
            state: State::Search,
            vit: Viterbi::new(K7_171_133, HEADER_INFO + 6).expect("header Viterbi"),
            decs: Vec::new(),
            llr: Vec::new(),
            q: Vec::new(),
            hard: vec![0u8; N],
            candidates: 0,
            headers: 0,
        }
    }

    /// The width.
    pub fn width(&self) -> Width {
        self.width
    }

    /// Back to the start of a stream.
    pub fn reset(&mut self) {
        if let Some(c) = self.conv.as_mut() {
            c.reset();
        }
        self.x.clear();
        self.bx.clear();
        self.base = 0;
        self.det = 0;
        self.fresh = true;
        self.run = None;
        self.quiet_until = 0;
        self.avg = 0.0;
        self.state = State::Search;
    }

    fn end(&self) -> u64 {
        self.base + self.x.len() as u64
    }

    fn rate(&self) -> f64 {
        self.width.rate()
    }

    /// Takes input samples and appends the frames they complete.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<RxFrame>) {
        match self.conv.as_mut() {
            Some(c) => {
                self.scratch.clear();
                c.process(input, &mut self.scratch);
                self.x.extend_from_slice(&self.scratch);
            }
            None => self.x.extend_from_slice(input),
        }
        self.extend_boxed();
        self.run_states(out);
    }

    /// Ends the stream: the converter is flushed, zeros follow, and a frame in progress is decoded with what arrived.
    pub fn finish(&mut self, out: &mut Vec<RxFrame>) {
        if let Some(c) = self.conv.as_mut() {
            self.scratch.clear();
            c.flush(&mut self.scratch);
            self.x.extend_from_slice(&self.scratch);
        }
        self.x.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), WIN + LAG + 4 * MF_LEN));
        self.extend_boxed();
        self.run_states(out);
        if let State::Payload { found, .. } = std::mem::replace(&mut self.state, State::Search) {
            out.push(self.payload(&found));
        }
    }

    /// The 4-sample running sum at absolute sample `n`.
    #[inline]
    fn boxed(&self, n: u64) -> Complex32 {
        self.bx[(n - self.base) as usize]
    }

    /// Extends the 4-sample running sums to the input that has arrived.
    fn extend_boxed(&mut self) {
        let from = self.bx.len();
        let to = self.x.len().saturating_sub(BOX - 1);
        for i in from..to {
            let v = self.x[i] + self.x[i + 1] + self.x[i + 2] + self.x[i + 3];
            self.bx.push(v);
        }
    }

    /// The exact sums of the window that starts at `self.det`.
    fn recompute(&mut self) {
        let mut s = Sums::default();
        for i in self.det..self.det + WIN as u64 {
            let a = self.boxed(i);
            let b = self.boxed(i + LAG as u64);
            let p = b * a.conj();
            s.c.0 += f64::from(p.re);
            s.c.1 += f64::from(p.im);
            s.m1.0 += f64::from(a.re);
            s.m1.1 += f64::from(a.im);
            s.m2.0 += f64::from(b.re);
            s.m2.1 += f64::from(b.im);
            s.e1 += f64::from(a.norm_sqr());
            s.e2 += f64::from(b.norm_sqr());
        }
        self.sums = s;
        self.steps = 0;
    }

    /// The metric of the window at `self.det`: the magnitude of the covariance over the root of the two variances.
    fn metric(&self) -> f64 {
        let w = WIN as f64;
        let s = &self.sums;
        let cr = s.c.0 - (s.m2.0 * s.m1.0 + s.m2.1 * s.m1.1) / w;
        let ci = s.c.1 - (s.m2.1 * s.m1.0 - s.m2.0 * s.m1.1) / w;
        let v1 = s.e1 - (s.m1.0 * s.m1.0 + s.m1.1 * s.m1.1) / w;
        let v2 = s.e2 - (s.m2.0 * s.m2.0 + s.m2.1 * s.m2.1) / w;
        // Digital silence and a window far below the average power give nothing.
        let floor = 1e-6 * self.avg * w * 16.0;
        if v1 <= floor.max(1e-20) || v2 <= floor.max(1e-20) {
            return 0.0;
        }
        (cr * cr + ci * ci).sqrt() / (v1 * v2).sqrt()
    }

    /// Moves the window one sample on.
    fn slide(&mut self) {
        let n = self.det;
        let (a0, b0) = (self.boxed(n), self.boxed(n + LAG as u64));
        let (a1, b1) = (self.boxed(n + WIN as u64), self.boxed(n + (WIN + LAG) as u64));
        let p0 = b0 * a0.conj();
        let p1 = b1 * a1.conj();
        let s = &mut self.sums;
        s.c.0 += f64::from(p1.re - p0.re);
        s.c.1 += f64::from(p1.im - p0.im);
        s.m1.0 += f64::from(a1.re - a0.re);
        s.m1.1 += f64::from(a1.im - a0.im);
        s.m2.0 += f64::from(b1.re - b0.re);
        s.m2.1 += f64::from(b1.im - b0.im);
        s.e1 += f64::from(a1.norm_sqr() - a0.norm_sqr());
        s.e2 += f64::from(b1.norm_sqr() - b0.norm_sqr());
        let e = f64::from(a1.norm_sqr()) / 16.0;
        self.avg += (e - self.avg) * 1e-6;
        self.det += 1;
        self.steps += 1;
        if self.steps >= 4096 {
            self.recompute();
        }
    }

    /// Whether the input of the window at `n` is as wide as the width: the magnitude of its covariance at a lag of one symbol,
    /// over its variance, is below [`NARROW`]. A preamble of the width (a chirp over the whole band) gives about 0; a narrower
    /// signal, such as one of a smaller width, gives much more and is no candidate.
    fn white(&self, n: u64) -> bool {
        let Some(i) = n.checked_sub(self.base).map(|i| i as usize) else {
            return false;
        };
        if i + WIN + SPS > self.x.len() {
            return true;
        }
        let xs = &self.x[i..i + WIN + SPS];
        let (mut c, mut m0, mut m1, mut e) = (Complex32::new(0.0, 0.0), Complex32::new(0.0, 0.0), Complex32::new(0.0, 0.0), 0f32);
        for k in 0..WIN {
            let (a, b) = (xs[k], xs[k + SPS]);
            c += b * a.conj();
            m0 += a;
            m1 += b;
            e += a.norm_sqr();
        }
        let w = WIN as f32;
        let cov = (c - m1 * m0.conj() / w).norm();
        let var = e - m0.norm_sqr() / w;
        var > 0.0 && cov < NARROW * var
    }

    /// Runs the detector over the input that has arrived; returns a candidate start.
    fn detect(&mut self) -> Option<u64> {
        let need = (WIN + LAG + BOX) as u64;
        if self.det < self.base {
            self.det = self.base;
            self.fresh = true;
        }
        while self.det + need < self.end() {
            if self.fresh {
                if self.avg == 0.0 {
                    let i = (self.det - self.base) as usize;
                    let span = &self.x[i..i + WIN];
                    self.avg = span.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / WIN as f64;
                }
                self.recompute();
                self.fresh = false;
            }
            let m = self.metric();
            let n = self.det;
            let mut cand = None;
            if m > DETECT && n >= self.quiet_until {
                match &mut self.run {
                    None => self.run = Some((n, n, m)),
                    Some(r) => {
                        if m > r.2 {
                            r.1 = n;
                            r.2 = m;
                        }
                    }
                }
                if self.run.is_some_and(|r| n - r.0 > RUN_CAP) {
                    cand = self.run.take().map(|r| r.1);
                }
            } else if let Some(r) = self.run.take() {
                cand = Some(r.1);
            }
            if m <= DETECT {
                self.quiet_until = 0;
            }
            self.slide();
            if let Some(c) = cand {
                if self.white(c) {
                    return cand;
                }
            }
        }
        None
    }

    fn run_states(&mut self, out: &mut Vec<RxFrame>) {
        loop {
            let end = self.end();
            if let State::Sync { n0, span, next, .. } = self.state {
                let need = (n0 + span + (4 * FIXED + MF_LEN + 8) as i64).max(0) as u64;
                if end < need {
                    break;
                }
                match self.sync(n0, span) {
                    Some(f) => {
                        self.headers += 1;
                        let np = payload_symbols(&f.header);
                        let last = f.tau + (SPS * (FIXED + np)) as f64 + MF_HALF as f64 + 2.0;
                        self.state = State::Payload { need: last.max(0.0).ceil() as u64, found: Box::new(f) };
                    }
                    None => {
                        if !next {
                            // The same run does not give another candidate for a while.
                            self.quiet_until = self.det + QUIET;
                        }
                        self.state = State::Search;
                    }
                }
                continue;
            }
            if let State::Payload { need, .. } = self.state {
                if end < need {
                    break;
                }
                let State::Payload { found, .. } = std::mem::replace(&mut self.state, State::Search) else {
                    break;
                };
                let f = self.payload(&found);
                let np = payload_symbols(&found.header);
                let after = found.tau + (SPS * (FIXED + np)) as f64;
                let more = found.header.kind == FrameKind::Data && !found.header.last;
                out.push(f);
                if more {
                    self.state = State::Sync { n0: after.round() as i64, span: NEXT_SPAN, next: true };
                } else {
                    // Detection goes on behind the frame.
                    self.det = (after.max(0.0) as u64).max(self.base);
                    self.fresh = true;
                    self.run = None;
                }
                continue;
            }
            match self.detect() {
                Some(n0) => {
                    self.candidates += 1;
                    self.state = State::Sync { n0: n0 as i64, span: SPAN, next: false };
                }
                None => break,
            }
        }
        self.trim();
    }

    fn trim(&mut self) {
        let keep = match &self.state {
            State::Search => self.det.min(self.run.map_or(u64::MAX, |r| r.1)).saturating_sub(SPAN as u64 + 256),
            State::Sync { n0, span, .. } => (*n0 - span - 256).max(0) as u64,
            State::Payload { found, .. } => (found.tau as u64).saturating_sub(256),
        };
        let drop = keep.saturating_sub(self.base) as usize;
        if drop > 1 << 16 {
            let drop = drop.min(self.bx.len());
            self.x.drain(..drop);
            self.bx.drain(..drop);
            self.base += drop as u64;
        }
    }

    /// The symbols `from .. from + n` of a frame at `tau` with the carrier `w` removed.
    fn symbols(&self, tau: f64, w: f64, from: usize, n: usize, out: &mut Vec<Complex32>) {
        out.clear();
        let step = rot(-w);
        let mut r = rot(-w * from as f64);
        for k in from..from + n {
            out.push(mf_at(&self.x, self.base, tau + (SPS * k) as f64) * r);
            r *= step;
            if k % 256 == 255 {
                r = rot(-w * (k + 1) as f64);
            }
        }
    }

    /// Timing, carrier and header of a candidate start near `n0`.
    fn sync(&mut self, n0: i64, span: i64) -> Option<Found> {
        let pre = preamble(self.width);
        let lo = n0 - span;
        if n0 + span < self.base as i64 {
            return None;
        }
        // The matched filter at every whole sample of the region.
        let len = (2 * span) as usize + 4 * (PREAMBLE - 1) + 1;
        let z: Vec<Complex32> = (0..len).map(|j| mf_at(&self.x, self.base, (lo + j as i64) as f64)).collect();
        let blocks = |d: usize| -> [Complex32; 32] {
            let mut b = [Complex32::new(0.0, 0.0); 32];
            for (j, v) in b.iter_mut().enumerate() {
                for k in 0..16 {
                    let s = 16 * j + k;
                    *v += z[d + 4 * s] * pre[s].conj();
                }
            }
            b
        };
        let mut best = (0usize, -1.0f64);
        for d in 0..=(2 * span) as usize {
            let p: f64 = blocks(d).iter().map(|v| f64::from(v.norm_sqr())).sum();
            if p > best.1 {
                best = (d, p);
            }
        }
        let b = blocks(best.0);
        let step: Complex32 = b.windows(2).map(|p| p[1] * p[0].conj()).sum();
        let wc = f64::from(step.arg()) / 16.0;
        // The whole preamble at whole samples near the block timing, then at tenths of a sample.
        let corr = |s: &Self, tau: f64| -> f64 {
            let mut acc = Complex32::new(0.0, 0.0);
            let st = rot(-wc);
            let mut r = Complex32::new(1.0, 0.0);
            for (k, p) in pre.iter().enumerate() {
                acc += mf_at(&s.x, s.base, tau + (SPS * k) as f64) * r * p.conj();
                r *= st;
            }
            f64::from(acc.norm_sqr())
        };
        let corr_z = |d: usize| -> f64 {
            let mut acc = Complex32::new(0.0, 0.0);
            let st = rot(-wc);
            let mut r = Complex32::new(1.0, 0.0);
            for (k, p) in pre.iter().enumerate() {
                acc += z[d + 4 * k] * r * p.conj();
                r *= st;
            }
            f64::from(acc.norm_sqr())
        };
        let lo_d = best.0.saturating_sub(6).max(1);
        let hi_d = (best.0 + 6).min((2 * span) as usize - 1);
        let cz: Vec<f64> = (lo_d - 1..=hi_d + 1).map(corr_z).collect();
        let bi = (1..cz.len() - 1).max_by(|&a, &b| cz[a].total_cmp(&cz[b]))?;
        let di = lo_d - 1 + bi;
        // A start whose whole-preamble correlation at whole samples is far below the acceptance level stops here.
        let ez: f64 = (0..PREAMBLE).map(|k| f64::from(z[di + 4 * k].norm_sqr())).sum();
        if cz[bi] < (0.7 * ACCEPT).powi(2) * ez * PREAMBLE as f64 {
            return None;
        }
        // The fraction of a sample: a parabola through the whole samples, then through tenths of a sample round it.
        let t0 = (lo + di as i64) as f64 + crate::family::fit::parabola(cz[bi - 1], cz[bi], cz[bi + 1]);
        let vals = [corr(self, t0 - 0.1), corr(self, t0), corr(self, t0 + 0.1)];
        let tau = t0 + 0.1 * crate::family::fit::parabola(vals[0], vals[1], vals[2]);
        // The carrier from the two copies of the sequence.
        let mut y = Vec::with_capacity(FIXED);
        self.symbols(tau, wc, 0, PREAMBLE, &mut y);
        let r: Complex32 = (10..10 + ZC_LEN).map(|k| y[k + ZC_LEN] * y[k].conj()).sum();
        let w = wc + f64::from(r.arg()) / ZC_LEN as f64;
        self.symbols(tau, w, 0, FIXED, &mut y);
        let c: Complex32 = y[..PREAMBLE].iter().zip(pre).map(|(a, p)| a * p.conj()).sum();
        let e: f64 = y[..PREAMBLE].iter().map(|a| f64::from(a.norm_sqr())).sum();
        if f64::from(c.norm()) < ACCEPT * (e * PREAMBLE as f64).sqrt() {
            return None;
        }
        let h0 = c / PREAMBLE as f32;
        let var = (y[..PREAMBLE].iter().zip(pre).map(|(a, p)| f64::from((a - h0 * p).norm_sqr())).sum::<f64>()
            / PREAMBLE as f64)
            .max(1e-12);
        let n0 = var / f64::from(h0.norm_sqr()).max(1e-30);
        let llr: Vec<f32> = y[PREAMBLE..FIXED].iter().map(|v| (4.0 * f64::from((v / h0).re) / n0) as f32).collect();
        let mean = llr.iter().map(|v| v.abs()).sum::<f32>() / HEADER_SYMBOLS as f32;
        let q: Vec<i8> = llr.iter().map(|v| (v * 20.0 / mean.max(1e-12)).round().clamp(-127.0, 127.0) as i8).collect();
        let mut bits = [0u8; HEADER_INFO];
        self.vit.decode(&q, Ends::Zero, &mut bits).ok()?;
        let header = parse_header(&bits)?;
        if header.width != self.width || !header.blocks_ok() {
            return None;
        }
        // The header symbols, now known, refine the carrier.
        let known = header_symbols(&header);
        let v: Vec<(f64, Complex32)> = y
            .iter()
            .zip(pre.iter().chain(&known))
            .enumerate()
            .map(|(k, (a, p))| (k as f64 - (FIXED / 2) as f64, a * p.conj()))
            .collect();
        let dw = crate::family::fit::phase_slope(&v, 0.0, 2e-3, 1e-4).0;
        Some(Found { header, tau, w: w + dw })
    }

    fn decoder(&mut self, rate: usize) -> &mut LdpcDecoder {
        if let Some(i) = self.decs.iter().position(|d| d.0 == rate) {
            return &mut self.decs[i].1;
        }
        let c = ldpc::code(N, rate).expect("a code of the family");
        self.decs.push((rate, c.decoder()));
        &mut self.decs.last_mut().expect("just pushed").1
    }

    /// The blocks of a frame whose header was read.
    fn payload(&mut self, f: &Found) -> RxFrame {
        let h = &f.header;
        let p = h.profile();
        let nb = usize::from(h.blocks);
        let d = nb * p.symbols();
        let g = pilots_of(d);
        let np = d + g;
        let mut y = Vec::with_capacity(np);
        self.symbols(f.tau, f.w, FIXED, np, &mut y);
        let pil = pilots(g);
        let raw: Vec<Complex32> = (0..g).map(|j| y[32 * j] * pil[j]).collect();
        // A straight line through the gains of the pilots within ±12, read at each pilot.
        let smooth: Vec<Complex32> = (0..g)
            .map(|j| {
                let (a, b) = (j.saturating_sub(12), (j + 13).min(g));
                let n = (b - a) as f64;
                let mt = (a..b).map(|i| i as f64 - j as f64).sum::<f64>() / n;
                let my = raw[a..b].iter().sum::<Complex32>() / n as f32;
                let (mut stt, mut sty) = (0.0f64, Complex32::new(0.0, 0.0));
                for i in a..b {
                    let t = i as f64 - j as f64 - mt;
                    stt += t * t;
                    sty += (raw[i] - my) * t as f32;
                }
                let slope = if stt > 0.0 { sty / stt as f32 } else { Complex32::new(0.0, 0.0) };
                my - slope * mt as f32
            })
            .collect();
        let mut resid = raw.iter().zip(&smooth).map(|(a, b)| f64::from((a - b).norm_sqr())).sum::<f64>() / g as f64;
        if g >= 8 {
            resid *= 1.09;
        }
        let amp2 = smooth.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / g as f64;
        let resid = resid.max(amp2 * 1e-6).max(1e-20);
        let n0 = (resid / amp2.max(1e-30)) as f32;
        let esn0_db = 10.0 * (amp2.max(1e-30) / resid).log10();
        let mut data = Vec::with_capacity(d);
        for (i, &v) in y.iter().enumerate() {
            if i % 32 == 0 {
                continue;
            }
            let j = i / 32;
            let t = (i % 32) as f32 / 32.0;
            let gain = if j + 1 < g { smooth[j] * (1.0 - t) + smooth[j + 1] * t } else { smooth[j] };
            data.push(if gain.norm_sqr() > 1e-20 { v / gain } else { v });
        }
        let c = qam::constellation(p.bits).expect("a family set");
        let k = p.k();
        let per = p.symbols();
        let mut blocks = Vec::with_capacity(nb);
        self.llr.resize(N, 0.0);
        self.q.resize(N, 0);
        for b in 0..nb {
            let _ = c.demap_block(&data[b * per..(b + 1) * per], n0.max(1e-6), Demap::MaxLog, &mut self.llr);
            for (q, l) in self.q.iter_mut().zip(&self.llr) {
                *q = (l.clamp(-LLR_CLIP, LLR_CLIP) * (127.0 / LLR_CLIP)).round() as i8;
            }
            let mut hard = std::mem::take(&mut self.hard);
            let q = std::mem::take(&mut self.q);
            let ok = self.decoder(p.rate).decode(&q, ITERATIONS, &mut hard).is_ok();
            self.q = q;
            let info = &mut hard[..k];
            scramble(info, h.frame, b);
            let bytes = family::pack(info);
            self.hard = hard;
            let pl = p.payload();
            let crc = u32::from_le_bytes([bytes[pl], bytes[pl + 1], bytes[pl + 2], bytes[pl + 3]]);
            let good = ok && family::crc32(&bytes[..pl]) == crc;
            blocks.push(good.then(|| bytes[..pl].to_vec()));
        }
        RxFrame {
            width: self.width,
            header: h.clone(),
            blocks,
            esn0_db,
            offset_hz: f.w * self.width.symbol_rate() / TAU,
            time: (f.tau - MF_HALF as f64) / self.rate(),
        }
    }
}

/// Payload symbols of the frame of `h`.
fn payload_symbols(h: &FrameHeader) -> usize {
    let d = usize::from(h.blocks) * h.profile().symbols();
    d + pilots_of(d)
}

/// The rates from which the receiver converts: the input rate, or the input rate rounded to a whole kHz when the ratio of
/// any width would need more than 20 000 input phases.
pub fn conversion_rate(input: f64, widths: &[Width]) -> f64 {
    let r = input.round();
    let big = widths.iter().any(|w| super::ratio(r, w.rate()).1 > 20_000);
    if big {
        (input / 1000.0).round() * 1000.0
    } else {
        r
    }
}

/// The receiver of every width it searches, side by side.
pub struct Receiver {
    lanes: Vec<Lane>,
}

impl Receiver {
    /// A receiver of input at `rate` that searches `widths`.
    pub fn new(rate: f64, widths: &[Width]) -> Receiver {
        let r = conversion_rate(rate, widths);
        let lanes = widths
            .iter()
            .map(|&w| {
                let (up, down) = super::ratio(r, w.rate());
                Lane::new(w, up, down)
            })
            .collect();
        Receiver { lanes }
    }

    /// A receiver of every width.
    pub fn all(rate: f64) -> Receiver {
        Receiver::new(rate, &WIDTHS)
    }

    /// The lanes.
    pub fn lanes(&self) -> &[Lane] {
        &self.lanes
    }

    /// Back to the start of a stream.
    pub fn reset(&mut self) {
        self.lanes.iter_mut().for_each(Lane::reset);
    }

    /// Takes input samples and appends the frames they complete, in the order of the lanes.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<RxFrame>) {
        for l in &mut self.lanes {
            l.process(input, out);
        }
    }

    /// Ends the stream.
    pub fn finish(&mut self, out: &mut Vec<RxFrame>) {
        for l in &mut self.lanes {
            l.finish(out);
        }
    }
}
