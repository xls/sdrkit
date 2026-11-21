//! The PACTOR-I receiver components: the tone pair finder, the burst detector and the burst reader.

use super::proto::{self, Packet};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::peak::parabolic;
use sdrkit_algo_dsp::spectrum::{Scaling, Welch};
use sdrkit_algo_dsp::stats::quantile_f64;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;
use sdrkit_core::ConfigError;
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Item, Kind, Port, Timed};

/// The audio rate.
pub const FS: f64 = 8000.0;

/// A look of the tone pair finder (`pactor.pair`).
#[derive(Clone, Debug, PartialEq)]
pub struct PactorPair {
    /// The audio sample count at the look.
    pub sample: u64,
    /// The locked lower tone, Hz (when locked).
    pub low_hz: f64,
    pub locked: bool,
    /// This look made a new lock (or moved it): the burst detector starts again on the stored audio.
    pub new_lock: bool,
    /// The weaker peak over the median, dB (0 to 40).
    pub snr_db: f64,
    /// The pair this look saw, if any.
    pub seen_hz: Option<f64>,
}

impl Item for PactorPair {
    const KIND: &'static str = "pactor.pair";
}

/// A burst (`pactor.burst`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PactorBurst {
    /// First and last audio sample.
    pub start: u64,
    pub end: u64,
    pub low_hz: f64,
}

impl Item for PactorBurst {
    const KIND: &'static str = "pactor.burst";
}

/// What a frame is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameKind {
    Data,
    Connect,
    Qrt,
    /// A control signal, 0 to 3 for CS1 to CS4.
    Cs(u8),
    /// A packet whose CRC failed (its soft bits are kept).
    Bad,
}

/// A frame (`pactor.frame`).
#[derive(Clone, Debug, PartialEq)]
pub struct PactorFrame {
    /// Signal time of its first bit, seconds.
    pub time: f64,
    pub kind: FrameKind,
    pub baud: u32,
    pub packet: Option<Packet>,
    pub inverted: bool,
    /// Soft bits of a failed packet (positive = the higher tone), in sending order.
    pub soft: Vec<f32>,
    pub snr_db: f64,
    pub low_hz: f64,
    /// Bit errors of a control signal.
    pub cs_errors: u32,
}

impl Item for PactorFrame {
    const KIND: &'static str = "pactor.frame";
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Tone pair finder

const LOOK_N: usize = 4096;
const LOOK_HOP: u64 = 2048;

/// The tone pair finder: `Real` 8 kHz in, `pactor.pair` out.
pub struct TonePairFinder {
    ring: Vec<f32>,
    n: u64,
    ps: Welch,
    centred: Vec<f32>,
    spec: Vec<f64>,
    scratch: Vec<f64>,
    win: Vec<f32>,
    prev: Option<f64>,
    locked: Option<f64>,
    other: Option<(f64, u32)>,
    last_seen: u64,
    looks: Vec<PactorPair>,
}

impl Default for TonePairFinder {
    fn default() -> Self {
        TonePairFinder {
            ring: vec![0.0; LOOK_N],
            n: 0,
            ps: Welch::new(LOOK_N, Window::Hann, 0.5, Scaling::Tone)
                .expect("4096 points is a valid Welch size"),
            centred: vec![0.0; LOOK_N],
            spec: Vec::with_capacity(LOOK_N / 2 + 1),
            scratch: Vec::with_capacity(LOOK_N),
            win: vec![0.0; LOOK_N],
            prev: None,
            locked: None,
            other: None,
            last_seen: 0,
            looks: Vec::new(),
        }
    }
}

/// A pair seen in a spectrum: the lower tone (Hz) and the weaker peak over the median.
pub fn find_pair(spec: &[f64], scratch: &mut Vec<f64>) -> Option<(f64, f64)> {
    let bin = FS / (2.0 * (spec.len() - 1) as f64);
    let lo = (300.0 / bin) as usize;
    let hi = ((3300.0 / bin) as usize).min(spec.len() - 1);
    let med = quantile_f64(&spec[lo..hi], 0.5, scratch)
        .unwrap_or(0.0)
        .max(1e-30);
    let a = (150.0 / bin) as usize;
    let b = ((3600.0 / bin) as usize).min(spec.len() - 3);
    let is_peak = |k: usize| -> bool {
        let v = spec[k];
        v >= 40.0 * med && (k - 2..=k + 2).all(|j| j == k || spec[j] <= v)
    };
    let sep = 200.0 / bin;
    let mut best: Option<(f64, f64, f64)> = None; // weaker, f1, f2
    for k in a.max(2)..b {
        if !is_peak(k) {
            continue;
        }
        let c = (k as f64 + sep).round() as usize;
        for q in c.saturating_sub(3)..=(c + 3).min(b - 1) {
            if q <= k + 2 || !is_peak(q) {
                continue;
            }
            let weaker = spec[k].min(spec[q]);
            if best.is_none_or(|x| weaker > x.0) {
                let r = |j: usize| -> f64 {
                    let l = |x: f64| x.max(1e-30).ln();
                    (j as f64 + parabolic(l(spec[j - 1]), l(spec[j]), l(spec[j + 1])).offset) * bin
                };
                best = Some((weaker, r(k), r(q)));
            }
        }
    }
    best.map(|(w, f1, f2)| {
        (
            (f1 + f2) / 2.0 - 100.0,
            (10.0 * (w / med).log10()).clamp(0.0, 40.0),
        )
    })
}

impl TonePairFinder {
    fn look(&mut self) -> PactorPair {
        let pos = (self.n % LOOK_N as u64) as usize;
        for i in 0..LOOK_N {
            self.win[i] = self.ring[(pos + i) % LOOK_N];
        }
        let _ = self.ps.compute_real(&self.win, &mut self.centred);
        // Bins 0 to n/2 of the centred spectrum (bin k at n/2 + k, the last one at index 0).
        self.spec.clear();
        self.spec
            .extend((0..=LOOK_N / 2).map(|k| self.centred[(LOOK_N / 2 + k) % LOOK_N] as f64));
        let seen = find_pair(&self.spec, &mut self.scratch);
        let mut new_lock = false;
        let mut snr = 0.0;
        if let Some((f, s)) = seen {
            snr = s;
            self.last_seen = self.n;
            match self.locked {
                None => {
                    if let Some(p) = self.prev {
                        if (p - f).abs() <= 6.0 {
                            self.locked = Some((p + f) / 2.0);
                            new_lock = true;
                            self.other = None;
                        }
                    }
                }
                Some(l) => {
                    if (l - f).abs() > 8.0 {
                        let cnt = match self.other {
                            Some((o, c)) if (o - f).abs() <= 8.0 => c + 1,
                            _ => 1,
                        };
                        self.other = Some((f, cnt));
                        if cnt >= 3 {
                            self.locked = Some(f);
                            self.other = None;
                            new_lock = true;
                        }
                    } else {
                        self.other = None;
                    }
                }
            }
        } else if self.locked.is_some() && self.n - self.last_seen >= 8 * FS as u64 {
            self.locked = None;
        }
        self.prev = seen.map(|x| x.0);
        PactorPair {
            sample: self.n,
            low_hz: self.locked.unwrap_or(0.0),
            locked: self.locked.is_some(),
            new_lock,
            snr_db: snr,
            seen_hz: seen.map(|x| x.0),
        }
    }
    /// Feeds audio, appending looks.
    pub fn process(&mut self, x: &[f32], out: &mut Vec<PactorPair>) {
        for &v in x {
            self.ring[(self.n % LOOK_N as u64) as usize] = v;
            self.n += 1;
            if self.n >= LOOK_N as u64 && self.n.is_multiple_of(LOOK_HOP) {
                out.push(self.look());
            }
        }
    }
}

impl Component for TonePairFinder {
    fn values(&self) -> sdrkit_core::Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.pactor_pair_finder",
            vec![Port::real("in")],
            vec![Port::typed::<PactorPair>("out")],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<PactorPair>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let mut v = std::mem::take(&mut self.looks);
        v.clear();
        self.process(x, &mut v);
        let o = io.items_out::<PactorPair>(0);
        for p in v.drain(..) {
            o.push(Timed {
                time: p.sample as f64 / FS,
                value: p,
            });
        }
        self.looks = v;
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Burst detector

/// The oscillators that move the lower and the upper tone of a pair to 0 Hz.
#[derive(Clone, Debug)]
struct TonePair {
    lo: Nco,
    hi: Nco,
}

impl Default for TonePair {
    fn default() -> Self {
        TonePair {
            lo: Nco::new(FS, 0.0),
            hi: Nco::new(FS, 0.0),
        }
    }
}

impl TonePair {
    /// Starts both oscillators at phase 0 for the pair whose lower tone is `low` Hz. A magnitude over any window does not depend
    /// on where the phase starts.
    fn start(&mut self, low: f64) {
        self.lo.set_freq(-low);
        self.hi.set_freq(-(low + 200.0));
        self.lo.set_phase_cycles(0.0);
        self.hi.set_phase_cycles(0.0);
    }

    /// The phasors of the next sample.
    #[inline]
    fn next(&mut self) -> (Complex32, Complex32) {
        (self.lo.next_sample(), self.hi.next_sample())
    }
}

const HIST: usize = 65_536;

/// The burst detector: `Real` 8 kHz and `pactor.pair` in, `pactor.burst` out.
pub struct BurstDetector {
    ring: Vec<f32>,
    n: u64,
    low: Option<f64>,
    noise: f64,
    blocks: [(f64, f64, f64, f64); 4],
    bi: usize,
    on: bool,
    start: u64,
    last_above: u64,
    below_ms: u32,
    /// Next sample the live detector handles.
    done: u64,
    bursts: Vec<PactorBurst>,
    energies: Vec<f64>,
    sort_scratch: Vec<f64>,
    /// The oscillators of the lower and the upper tone.
    osc: TonePair,
}

impl Default for BurstDetector {
    fn default() -> Self {
        BurstDetector {
            ring: vec![0.0; HIST],
            n: 0,
            low: None,
            noise: 0.0,
            blocks: [(0.0, 0.0, 0.0, 0.0); 4],
            bi: 0,
            on: false,
            start: 0,
            last_above: 0,
            below_ms: 0,
            done: 0,
            bursts: Vec::new(),
            energies: Vec::new(),
            sort_scratch: Vec::new(),
            osc: TonePair::default(),
        }
    }
}

impl BurstDetector {
    /// The energy of the 1 ms block starting at absolute sample `b` (8 samples), updating the 4 ms sums. The blocks of one pass
    /// must follow each other, since the oscillators step on from block to block.
    fn block(&mut self, b: u64) -> f64 {
        let (mut lr, mut li, mut hr, mut hi) = (0f64, 0f64, 0f64, 0f64);
        for k in 0..8u64 {
            let x = self.ring[((b + k) % HIST as u64) as usize] as f64;
            let (l, h) = self.osc.next();
            lr += x * l.re as f64;
            li += x * l.im as f64;
            hr += x * h.re as f64;
            hi += x * h.im as f64;
        }
        self.blocks[self.bi] = (lr, li, hr, hi);
        self.bi = (self.bi + 1) % 4;
        let (mut a, mut bb, mut c, mut d) = (0.0, 0.0, 0.0, 0.0);
        for x in &self.blocks {
            a += x.0;
            bb += x.1;
            c += x.2;
            d += x.3;
        }
        a * a + bb * bb + c * c + d * d
    }

    fn detect(&mut self, b: u64, e: f64) {
        let end_block = b + 8;
        if !self.on {
            if e < 3.0 * self.noise {
                self.noise += 0.01 * (e - self.noise);
            }
            if e > 6.0 * self.noise {
                self.on = true;
                self.start = b.saturating_sub(32);
                self.last_above = end_block;
                self.below_ms = 0;
            }
        } else {
            if e > 3.0 * self.noise {
                self.below_ms = 0;
                self.last_above = end_block;
            } else {
                self.below_ms += 1;
            }
            if self.below_ms >= 24 {
                self.on = false;
                self.emit(self.last_above);
            } else if end_block - self.start > 6 * FS as u64 {
                self.emit(end_block);
                self.start = end_block;
            }
        }
    }

    fn emit(&mut self, end: u64) {
        if end > self.start && end - self.start >= 560 {
            self.bursts.push(PactorBurst {
                start: self.start,
                end,
                low_hz: self.low.unwrap_or(0.0),
            });
        }
    }

    /// Starts on the stored audio of the last 4 s: the noise from the lower quartile of the energies, then the real pass.
    fn relock(&mut self, low: f64) {
        self.low = Some(low);
        self.on = false;
        self.blocks = [(0.0, 0.0, 0.0, 0.0); 4];
        let from = self.n.saturating_sub(4 * FS as u64) / 8 * 8;
        let to = self.n / 8 * 8;
        self.energies.clear();
        self.osc.start(low);
        let mut b = from;
        while b < to {
            let e = self.block(b);
            self.energies.push(e);
            b += 8;
        }
        let q = quantile_f64(&self.energies, 0.25, &mut self.sort_scratch).unwrap_or(0.0);
        self.noise = (q / 0.38).max(1e-20);
        self.blocks = [(0.0, 0.0, 0.0, 0.0); 4];
        self.osc.start(low);
        let mut b = from;
        while b < to {
            let e = self.block(b);
            self.detect(b, e);
            b += 8;
        }
        self.done = to;
    }

    /// Feeds audio and the finder's looks, appending bursts.
    pub fn process(&mut self, x: &[f32], pairs: &[PactorPair], out: &mut Vec<PactorBurst>) {
        let mut pi = 0;
        for &v in x {
            self.ring[(self.n % HIST as u64) as usize] = v;
            self.n += 1;
            while pi < pairs.len() && pairs[pi].sample <= self.n {
                let p = &pairs[pi];
                if p.new_lock {
                    self.relock(p.low_hz);
                } else if !p.locked && self.low.is_some() && !self.on {
                    self.low = None;
                }
                pi += 1;
            }
            if self.low.is_some() {
                while self.done + 8 <= self.n {
                    let b = self.done;
                    let e = self.block(b);
                    self.detect(b, e);
                    self.done += 8;
                }
            } else {
                self.done = self.n / 8 * 8;
            }
        }
        out.append(&mut self.bursts);
    }
}

/// The burst detector as a component.
#[derive(Default)]
pub struct BurstDetectorComponent {
    det: BurstDetector,
    out: Vec<PactorBurst>,
    pairs: Vec<PactorPair>,
}

impl Component for BurstDetectorComponent {
    fn values(&self) -> sdrkit_core::Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.pactor_burst_detector",
            vec![Port::real("audio"), Port::typed::<PactorPair>("pair")],
            vec![Port::typed::<PactorBurst>("out")],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<PactorBurst>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        self.pairs.clear();
        self.pairs
            .extend(io.items::<PactorPair>(1).iter().map(|p| p.value.clone()));
        self.out.clear();
        self.det.process(x, &self.pairs, &mut self.out);
        let o = io.items_out::<PactorBurst>(0);
        for b in self.out.drain(..) {
            o.push(Timed {
                time: b.start as f64 / FS,
                value: b,
            });
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Burst reader

const BEFORE: u64 = 400; // 50 ms
const AFTER: u64 = 600; // 75 ms
const PACKET_LEN: usize = 7680; // 0.96 s
const CS_LEN: usize = 960; // 0.12 s

/// Prefix sums of the audio times the two tone phasors over a burst.
#[derive(Default)]
struct Prefix {
    lo: Vec<(f64, f64)>,
    hi: Vec<(f64, f64)>,
}

impl Prefix {
    fn len(&self) -> usize {
        self.lo.len() - 1
    }
    #[inline]
    fn mag(v: &[(f64, f64)], a: usize, b: usize) -> f64 {
        let (x, y) = (v[b].0 - v[a].0, v[b].1 - v[a].1);
        (x * x + y * y).sqrt()
    }
    /// Soft bit of window a..b (positive = the higher tone) and the two tone energies.
    #[inline]
    fn soft(&self, a: usize, b: usize) -> (f32, f64, f64) {
        let u = Self::mag(&self.hi, a, b);
        let l = Self::mag(&self.lo, a, b);
        let s = if u + l > 0.0 { (u - l) / (u + l) } else { 0.0 };
        (s as f32, u * u, l * l)
    }
}

/// The burst reader: `Real` 8 kHz and `pactor.burst` in, `pactor.frame` out.
#[derive(Default)]
pub struct BurstReader {
    ring: Vec<f32>,
    n: u64,
    pending: Vec<PactorBurst>,
    pre: Prefix,
    soft: Vec<f32>,
    bits: Vec<u8>,
    bytes: Vec<u8>,
    last_failed: Vec<f32>,
    osc: TonePair,
}

impl BurstReader {
    fn new() -> Self {
        BurstReader {
            ring: vec![0.0; HIST],
            ..Default::default()
        }
    }

    /// The prefix sums of the audio `from..to` times the two tone phasors, which start at phase 0 at `from`.
    fn prepare(&mut self, from: u64, to: u64, low: f64) {
        self.pre.lo.clear();
        self.pre.hi.clear();
        self.pre.lo.push((0.0, 0.0));
        self.pre.hi.push((0.0, 0.0));
        self.osc.start(low);
        let (mut a, mut b, mut c, mut d) = (0f64, 0f64, 0f64, 0f64);
        for i in from..to {
            let x = self.ring[(i % HIST as u64) as usize] as f64;
            let (l, h) = self.osc.next();
            a += x * l.re as f64;
            b += x * l.im as f64;
            c += x * h.re as f64;
            d += x * h.im as f64;
            self.pre.lo.push((a, b));
            self.pre.hi.push((c, d));
        }
    }

    /// Soft bits of `n` bits of `len` samples from `s` into `self.soft`; the stronger and weaker tone energy sums.
    fn soft_bits(&mut self, s: usize, n: usize, len: usize) -> (f64, f64) {
        self.soft.clear();
        let (mut strong, mut weak) = (0.0, 0.0);
        for i in 0..n {
            let (v, u, l) = self.pre.soft(s + i * len, s + (i + 1) * len);
            self.soft.push(v);
            strong += u.max(l);
            weak += u.min(l);
        }
        (strong, weak)
    }

    /// A packet at sample `s` (relative): the first speed and polarity that passes.
    fn try_packet_at(&mut self, s: usize) -> Option<(Packet, u32, bool, f64)> {
        if s + PACKET_LEN > self.pre.len() {
            return None;
        }
        for &(baud, len, n) in &[(100u32, 80usize, 96usize), (200, 40, 192)] {
            // A quick look at the header first.
            let mut h = 0u8;
            for k in 0..8 {
                let (v, _, _) = self.pre.soft(s + k * len, s + (k + 1) * len);
                h |= ((v > 0.0) as u8) << k;
            }
            let near = |x: u8| (x ^ 0x55).count_ones().min((x ^ 0xAA).count_ones()) <= 1;
            let pol = [false, true].map(|inv| near(if inv { !h } else { h }));
            if !pol.contains(&true) {
                continue;
            }
            let (strong, weak) = self.soft_bits(s, n, len);
            for inv in [false, true]
                .into_iter()
                .filter(|&inv| pol[usize::from(inv)])
            {
                self.bits.clear();
                self.bits
                    .extend(self.soft.iter().map(|&v| ((v > 0.0) != inv) as u8));
                if !proto::check_packet(&self.bits, &mut self.bytes) {
                    continue;
                }
                let p = proto::read_packet(&self.bits);
                if proto::packet_ok(&p) {
                    let snr = if weak > 0.0 {
                        (10.0 * (strong / weak).log10()).min(40.0)
                    } else {
                        40.0
                    };
                    return Some((p, baud, inv, snr));
                }
            }
        }
        None
    }

    fn offsets() -> impl Iterator<Item = i64> {
        std::iter::once(0).chain((1..=30).flat_map(|k| [k, -k]))
    }

    fn try_packet(&mut self, pos: usize) -> Option<(usize, Packet, u32, bool, f64)> {
        for off in Self::offsets() {
            let s = pos as i64 + off * 8;
            if s < 0 {
                continue;
            }
            if let Some((p, b, inv, snr)) = self.try_packet_at(s as usize) {
                return Some((s as usize, p, b, inv, snr));
            }
        }
        None
    }

    fn try_cs(&mut self, pos: usize) -> Option<(usize, usize, u32, bool)> {
        let mut best: Option<(usize, usize, u32, bool, f32)> = None;
        for off in Self::offsets() {
            let s = pos as i64 + off * 8;
            if s < 0 || s as usize + CS_LEN > self.pre.len() {
                continue;
            }
            let s = s as usize;
            self.soft_bits(s, 12, 80);
            let conf: f32 = self.soft.iter().map(|v| v.abs()).sum();
            for inv in [false, true] {
                self.bits.clear();
                self.bits
                    .extend(self.soft.iter().map(|&v| ((v > 0.0) != inv) as u8));
                let (i, d) = proto::cs_match(&self.bits);
                if d <= 1 && best.is_none_or(|b| d < b.2 || (d == b.2 && conf > b.4)) {
                    best = Some((s, i, d, inv, conf));
                }
            }
        }
        best.map(|b| (b.0, b.1, b.2, b.3))
    }

    fn read(&mut self, b: PactorBurst, out: &mut Vec<PactorFrame>) {
        let from = b.start.saturating_sub(BEFORE);
        let to = b.end + AFTER;
        if b.low_hz <= 0.0 || self.n < to || self.n - from > HIST as u64 {
            return;
        }
        self.prepare(from, to, b.low_hz);
        let burst_end = (b.end - from) as usize;
        let mut pos = (b.start - from) as usize;
        let time = |s: usize| (from + s as u64) as f64 / FS;
        for _ in 0..12 {
            let left = burst_end.saturating_sub(pos);
            if left >= 6400 {
                if let Some((s, p, baud, inv, snr)) = self.try_packet(pos) {
                    let kind = if p.status.qrt {
                        FrameKind::Qrt
                    } else if baud == 100
                        && p.header == 0x55
                        && proto::connect_call(&p.data).is_some()
                    {
                        FrameKind::Connect
                    } else {
                        FrameKind::Data
                    };
                    out.push(PactorFrame {
                        time: time(s),
                        kind,
                        baud,
                        packet: Some(p),
                        inverted: inv,
                        soft: Vec::new(),
                        snr_db: snr,
                        low_hz: b.low_hz,
                        cs_errors: 0,
                    });
                    pos = s + PACKET_LEN;
                    continue;
                }
            }
            if left >= 720 {
                if let Some((s, i, d, inv)) = self.try_cs(pos) {
                    let after = burst_end.saturating_sub(s + CS_LEN);
                    let follows_packet = after >= 6400 && self.try_packet(s + CS_LEN).is_some();
                    if after < 720 || follows_packet || (7280..=8000).contains(&after) {
                        out.push(PactorFrame {
                            time: time(s),
                            kind: FrameKind::Cs(i as u8),
                            baud: 100,
                            packet: None,
                            inverted: inv,
                            soft: Vec::new(),
                            snr_db: 0.0,
                            low_hz: b.low_hz,
                            cs_errors: d,
                        });
                        pos = s + CS_LEN;
                        continue;
                    }
                }
            }
            if left >= 6800 {
                let soft = self.failed_guess(pos);
                out.push(PactorFrame {
                    time: time(pos),
                    kind: FrameKind::Bad,
                    baud: if soft.len() == 192 { 200 } else { 100 },
                    packet: None,
                    inverted: false,
                    soft: soft.clone(),
                    snr_db: 0.0,
                    low_hz: b.low_hz,
                    cs_errors: 0,
                });
                self.last_failed = soft;
                pos += PACKET_LEN;
                continue;
            }
            break;
        }
    }

    /// The best guess of a failed packet's soft bits: the speed and timing whose bits are most confident, plus how well the first 8
    /// follow the alternating header; or the hypothesis that correlates best with an earlier failed packet (over 0.2 per bit).
    fn failed_guess(&mut self, pos: usize) -> Vec<f32> {
        let mut best: (f32, Vec<f32>) = (f32::MIN, Vec::new());
        let mut best_corr: (f32, Vec<f32>) = (0.2, Vec::new());
        for off in Self::offsets() {
            let s = pos as i64 + off * 8;
            if s < 0 || s as usize + PACKET_LEN > self.pre.len() {
                continue;
            }
            for &(len, n) in &[(80usize, 96usize), (40, 192)] {
                self.soft_bits(s as usize, n, len);
                let conf: f32 = self.soft.iter().map(|v| v.abs()).sum::<f32>() / n as f32;
                let alt: f32 = (0..8)
                    .map(|k| self.soft[k] * if k % 2 == 0 { 1.0 } else { -1.0 })
                    .sum::<f32>()
                    .abs()
                    / 8.0;
                let score = conf + alt;
                if score > best.0 {
                    best = (score, self.soft.clone());
                }
                if self.last_failed.len() == n {
                    let c: f32 = self
                        .soft
                        .iter()
                        .zip(&self.last_failed)
                        .map(|(a, b)| a * b)
                        .sum::<f32>()
                        .abs()
                        / n as f32;
                    if c > best_corr.0 {
                        best_corr = (c, self.soft.clone());
                    }
                }
            }
        }
        if !best_corr.1.is_empty() {
            best_corr.1
        } else {
            best.1
        }
    }

    /// Feeds audio and bursts, appending the frames of every burst whose audio is complete.
    pub fn process(&mut self, x: &[f32], bursts: &[PactorBurst], out: &mut Vec<PactorFrame>) {
        if self.ring.is_empty() {
            self.ring = vec![0.0; HIST];
        }
        for &v in x {
            self.ring[(self.n % HIST as u64) as usize] = v;
            self.n += 1;
        }
        self.pending.extend_from_slice(bursts);
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].end + AFTER <= self.n {
                let b = self.pending.remove(i);
                self.read(b, out);
            } else {
                i += 1;
            }
        }
    }
}

/// The burst reader as a component.
pub struct BurstReaderComponent {
    rd: BurstReader,
    bursts: Vec<PactorBurst>,
    out: Vec<PactorFrame>,
}

impl Default for BurstReaderComponent {
    fn default() -> Self {
        BurstReaderComponent {
            rd: BurstReader::new(),
            bursts: Vec::new(),
            out: Vec::new(),
        }
    }
}

impl Component for BurstReaderComponent {
    fn values(&self) -> sdrkit_core::Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.pactor_burst_reader",
            vec![Port::real("audio"), Port::typed::<PactorBurst>("bursts")],
            vec![Port::typed::<PactorFrame>("out")],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<PactorFrame>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        self.bursts.clear();
        self.bursts
            .extend(io.items::<PactorBurst>(1).iter().map(|b| b.value));
        self.out.clear();
        self.rd.process(x, &self.bursts, &mut self.out);
        let o = io.items_out::<PactorFrame>(0);
        for f in self.out.drain(..) {
            o.push(Timed {
                time: f.time,
                value: f,
            });
        }
    }
}

/// Decodes summed soft bits as a packet, trying both polarities (for memory ARQ).
pub fn decode_soft(soft: &[f32]) -> Option<(Packet, bool)> {
    for inv in [false, true] {
        let bits: Vec<u8> = soft.iter().map(|&v| ((v > 0.0) != inv) as u8).collect();
        let p = proto::read_packet(&bits);
        if proto::packet_ok(&p) {
            return Some((p, inv));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// A tone of amplitude 1 at `f` Hz in `ring` for the absolute samples `from..to`.
    fn tone(ring: &mut [f32], from: u64, to: u64, f: f64) {
        for i in from..to {
            ring[(i % HIST as u64) as usize] = (TAU * f * i as f64 / FS).cos() as f32;
        }
    }

    // Sample 8 000 000 is 1000 s of audio. 1000 s at the pair frequency is not a whole number of cycles.
    const LOW: f64 = 1433.3456;
    const EDGE: u64 = 8_000_000;

    #[test]
    fn the_burst_detector_phase_is_continuous_at_1000_s() {
        let mut det = BurstDetector {
            n: EDGE + 16,
            ..Default::default()
        };
        tone(&mut det.ring, det.n - 4 * FS as u64, det.n, LOW + 200.0);
        det.relock(LOW);
        let e = &det.energies;
        let steady = e[100];
        let across = e[e.len() - 1];
        assert!(
            (across / steady - 1.0).abs() < 0.02,
            "energy {across} across 1000 s, {steady} before"
        );
    }

    #[test]
    fn the_burst_reader_phase_is_continuous_at_1000_s() {
        let mut rd = BurstReader::new();
        let (from, to) = (EDGE - 1000, EDGE + 1000);
        tone(&mut rd.ring, from, to, LOW + 200.0);
        rd.n = to;
        rd.prepare(from, to, LOW);
        let inside = rd.pre.soft(0, 80).1;
        let across = rd.pre.soft(960, 1040).1;
        assert!(
            (across / inside - 1.0).abs() < 0.02,
            "energy {across} across 1000 s, {inside} before"
        );
    }
}
