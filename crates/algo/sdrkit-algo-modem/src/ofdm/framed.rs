//! The framed OFDM modem: a frame layout given as data, the modulator, and a streaming demodulator that finds the frames, follows
//! them and emits equalised cells with their weights, one frame at a time. Positions are absolute input indices.

use super::{equalise, ClockDrift, FftFramer, LagCorrelator, OfdmLayout, PilotPattern};
use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir::ComplexFir;
use std::f64::consts::TAU;
use std::sync::Arc;

/// A frame layout: what a standard fixes about its OFDM frame.
#[derive(Clone, Debug)]
pub struct FrameLayout {
    /// Sample rate.
    pub rate: f64,
    /// FFT size.
    pub nfft: usize,
    /// Guard interval of the symbols after the first.
    pub cp: usize,
    /// Guard interval of the first symbol of a frame (a beacon can have its own).
    pub cp0: usize,
    /// Symbols per frame.
    pub nsym: usize,
    /// Lowest carrier.
    pub kmin: i32,
    /// Highest carrier.
    pub kmax: i32,
    /// Known cells per symbol (the same in every frame): (carrier index from kmin, value).
    pub known: Vec<Vec<(usize, Complex32)>>,
    /// Cells that carry nothing (index from kmin), per symbol, besides the known ones. Usually DC.
    pub null: Vec<usize>,
    /// The largest whole-carrier offset searched.
    pub max_shift: i32,
    /// How far the FFT window starts inside the guard interval.
    pub adv: usize,
    /// Acquisition threshold on the known-cell correlation.
    pub sync_threshold: f32,
    /// Guard correlation threshold for a signal.
    pub guard_threshold: f32,
    /// A filter for the guard correlation only (complex taps, centred): keeps a strong analogue host or carrier out of it.
    pub search_taps: Option<Vec<Complex32>>,
    /// Only known cells with |f| in this range (Hz) count for the frame start and the lock (a band beside an analogue programme).
    pub pair_band: Option<(f64, f64)>,
}

impl FrameLayout {
    /// Carriers.
    pub fn ncarriers(&self) -> usize {
        (self.kmax - self.kmin + 1) as usize
    }
    /// Carrier spacing.
    pub fn spacing(&self) -> f64 {
        self.rate / self.nfft as f64
    }
    /// Samples per frame.
    pub fn frame_len(&self) -> usize {
        self.cp0 + self.nfft + self.nsym.saturating_sub(1) * (self.cp + self.nfft)
    }
    /// Start of symbol `s` in a frame (its guard).
    pub fn sym_start(&self, s: usize) -> usize {
        if s == 0 {
            0
        } else {
            self.cp0 + self.nfft + (s - 1) * (self.cp + self.nfft)
        }
    }
    /// Guard of symbol `s`.
    pub fn sym_cp(&self, s: usize) -> usize {
        if s == 0 {
            self.cp0
        } else {
            self.cp
        }
    }
    /// A map of known cells per symbol: `Some(value)` where known.
    pub fn known_grid(&self) -> Vec<Option<Complex32>> {
        let k = self.ncarriers();
        let mut g = vec![None; self.nsym * k];
        for (s, cells) in self.known.iter().enumerate().take(self.nsym) {
            for &(c, v) in cells {
                if c < k {
                    g[s * k + c] = Some(v);
                }
            }
        }
        g
    }
    /// The cells that carry data (not known, not null), in symbol then carrier order.
    pub fn data_cells(&self) -> Vec<(usize, usize)> {
        let k = self.ncarriers();
        let g = self.known_grid();
        let mut v = Vec::new();
        for s in 0..self.nsym {
            for c in 0..k {
                if g[s * k + c].is_none() && !self.null.contains(&c) {
                    v.push((s, c));
                }
            }
        }
        v
    }
    /// The layout as an [`OfdmLayout`]: one guard per symbol of the frame (`cp0`, then `cp`), carriers `kmin..=kmax`, the null
    /// carriers (reserved carriers where a known cell is on one), and the known cells as a pilot pattern with the period of one
    /// frame. Errors when the frame has no symbols or
    /// `OfdmLayout::new` refuses it (a guard of `nfft` or more, carriers that do not fit the FFT, a known cell on a null carrier
    /// or with the value 0).
    pub fn ofdm(&self) -> Result<OfdmLayout, ModemError> {
        if self.nsym == 0 {
            return Param::Symbols.at_least(1.0, 0.0);
        }
        if self.known.len() > self.nsym {
            return Param::Pilots.conflict(Param::Symbols);
        }
        let cp: Vec<usize> = (0..self.nsym).map(|s| self.sym_cp(s)).collect();
        // A null carrier with a known cell in some symbol carries no data: a reserved carrier of the shared layout.
        let known_somewhere = |c: usize| self.known.iter().any(|row| row.iter().any(|x| x.0 == c));
        let (reserved, nulls): (Vec<usize>, Vec<usize>) =
            self.null.iter().partition(|&&c| known_somewhere(c));
        let at = |v: Vec<usize>| -> Vec<i32> { v.iter().map(|&c| self.kmin + c as i32).collect() };
        let (reserved, nulls) = (at(reserved), at(nulls));
        let cells = (0..self.nsym)
            .map(|s| {
                self.known.get(s).map_or_else(Vec::new, |row| {
                    row.iter()
                        .map(|&(c, v)| (self.kmin + c as i32, v))
                        .collect()
                })
            })
            .collect();
        OfdmLayout::new(
            self.nfft,
            &cp,
            self.kmin,
            self.kmax,
            &nulls,
            &reserved,
            &PilotPattern { cells },
        )
    }
}

/// Brings a delay difference into (-period/2, period/2].
fn wrap_delay(x: f32, period: f32) -> f32 {
    x - period * (x / period).round()
}

/// The modulator: frames of cells to baseband samples.
pub struct FrameModulator {
    layout: FrameLayout,
    ifft: Fft,
    buf: Vec<Complex32>,
    scale: f32,
}

impl FrameModulator {
    /// A modulator for `layout`. The output has a mean power of about the mean power of the active cells.
    pub fn new(layout: FrameLayout) -> FrameModulator {
        let active = layout.ncarriers().saturating_sub(layout.null.len()).max(1);
        let scale = 1.0 / (active as f32).sqrt();
        FrameModulator {
            ifft: Fft::inverse(layout.nfft),
            buf: vec![Complex32::new(0.0, 0.0); layout.nfft],
            layout,
            scale,
        }
    }
    /// The layout.
    pub fn layout(&self) -> &FrameLayout {
        &self.layout
    }
    /// Modulates one symbol of `ncarriers` cells (known cells must already be in place), appending guard and useful part.
    pub fn symbol(&mut self, s: usize, cells: &[Complex32], out: &mut Vec<Complex32>) {
        let l = &self.layout;
        let n = l.nfft;
        self.buf.fill(Complex32::new(0.0, 0.0));
        for (i, &c) in cells.iter().enumerate() {
            let k = l.kmin + i as i32;
            let bin = k.rem_euclid(n as i32) as usize;
            self.buf[bin] = c * self.scale;
        }
        self.ifft.process(&mut self.buf);
        let cp = l.sym_cp(s);
        // The guard can be longer than the symbol (a beacon of two halves): repeat cyclically.
        for i in 0..cp {
            out.push(self.buf[(n - (cp - i) % n) % n]);
        }
        out.extend_from_slice(&self.buf);
    }
    /// Modulates a frame of `nsym * ncarriers` cells.
    pub fn frame(&mut self, cells: &[Complex32], out: &mut Vec<Complex32>) {
        let k = self.layout.ncarriers();
        for s in 0..self.layout.nsym {
            let Some(row) = cells.get(s * k..(s + 1) * k) else {
                return;
            };
            self.symbol(s, row, out);
        }
    }
}

/// One demodulated frame.
#[derive(Clone, Debug, Default)]
pub struct CellFrame {
    /// Frames since the lock (0 is the first).
    pub index: u64,
    /// The first frame after a (re)lock.
    pub first: bool,
    /// Absolute input index of the frame start.
    pub start: u64,
    /// Signal time of the frame start, seconds.
    pub time: f64,
    /// Equalised cells, `nsym * ncarriers`.
    pub z: Vec<Complex32>,
    /// Their weights (|H|^2 / noise).
    pub w: Vec<f32>,
    /// The channel of symbol 0 per carrier.
    pub h: Vec<Complex32>,
    /// Pilot SNR, dB.
    pub snr_db: f32,
    /// Carrier offset, Hz.
    pub cfo_hz: f64,
    /// Sample clock offset, ppm.
    pub ppm: f64,
    /// Correlation of the known cells, 0..1.
    pub sync: f32,
    /// Delay of the channel against the window, samples.
    pub delay: f32,
    /// The input was conjugated (mirrored spectrum).
    pub mirrored: bool,
    /// Mean input power, dBFS.
    pub level_db: f32,
}

/// What the demodulator is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameState {
    /// Looking for a signal.
    Searching,
    /// Following one.
    Tracking,
}

/// Two known cells whose product identifies the frame start and the carrier offset.
struct Pair {
    s1: usize,
    k1: usize,
    s2: usize,
    k2: usize,
    r: Complex32,
    /// Samples between the two windows (0 for a pair in one symbol).
    dt: f64,
    /// 0 for a pair in time, else the carrier gap.
    group: usize,
}

/// The shared pieces the demodulator runs on: absent when the layout is not a valid OFDM frame.
struct Engine {
    framer: FftFramer,
    corr: LagCorrelator,
    fir: Option<(ComplexFir, usize)>,
}

/// The streaming demodulator.
pub struct FrameDemodulator {
    l: FrameLayout,
    engine: Option<Engine>,
    known: Vec<Option<Complex32>>,
    /// Per carrier: the symbols (0..=nsym, nsym being the next frame's symbol 0) where it is known.
    ktime: Vec<Vec<usize>>,
    /// The carriers known in some symbol.
    has: Vec<usize>,
    /// Per symbol: the known carriers that count for the measures.
    known_in_band: Vec<Vec<usize>>,
    pairs: Vec<Pair>,
    /// Per carrier: its place among the carriers that have pairs in time, or `usize::MAX`.
    time_slot: Vec<usize>,
    /// The number of carriers that have pairs in time.
    time_slots: usize,
    freq_gap: usize,
    buf: Vec<Complex32>,
    base: u64,
    state: FrameState,
    frame_start: u64,
    cfo: f64,
    mirror: bool,
    frames: u64,
    shifts: i64,
    lock_at: u64,
    bad_frames: u32,
    search_from: u64,
    failed_searches: u32,
    clock: ClockDrift,
    /// Scratch.
    y: Vec<Complex32>,
    hraw: Vec<Complex32>,
    hgrid: Vec<Complex32>,
    have: Vec<bool>,
    src: Vec<Complex32>,
    out: Option<Arc<CellFrame>>,
    blocked: Vec<(i64, f64)>,
    delay_s: f32,
    level: f64,
    /// Searches made.
    pub searches: u64,
    /// Locks found.
    pub locks: u64,
    /// Locks lost.
    pub losses: u64,
    /// Whole-sample timing moves.
    pub timing_moves: u64,
    hold: bool,
    /// Carriers whose known cells count for the delay, noise, offset and lock measures.
    in_band: Vec<bool>,
}

impl FrameDemodulator {
    /// A demodulator for `layout`. If [`FrameDemodulator::try_new`] refuses the layout, the demodulator never locks.
    pub fn new(layout: FrameLayout) -> FrameDemodulator {
        match Self::engine(&layout) {
            Ok(e) => Self::build(layout, Some(e)),
            Err(_) => Self::build(layout, None),
        }
    }

    /// A demodulator for `layout`. Errors when [`FrameLayout::ofdm`] refuses the layout, or `adv` is longer than the shortest guard.
    pub fn try_new(layout: FrameLayout) -> Result<FrameDemodulator, ModemError> {
        let e = Self::engine(&layout)?;
        Ok(Self::build(layout, Some(e)))
    }

    fn engine(layout: &FrameLayout) -> Result<Engine, ModemError> {
        let ofdm = layout.ofdm()?;
        let framer = FftFramer::new(&ofdm, layout.adv)?;
        let corr = LagCorrelator::new(layout.nfft, layout.cp.max(1), 1.0, 0.5)?;
        // The family filter is a centred correlation: the reversed taps make it a convolution.
        let fir = layout
            .search_taps
            .as_ref()
            .filter(|h| !h.is_empty())
            .map(|h| {
                let rev: Vec<Complex32> = h.iter().rev().copied().collect();
                (ComplexFir::new(&rev), (h.len() - 1) / 2)
            });
        Ok(Engine { framer, corr, fir })
    }

    fn build(layout: FrameLayout, engine: Option<Engine>) -> FrameDemodulator {
        // A refused layout gets no tables: its sizes are not to be trusted.
        let (k, ns) = if engine.is_some() {
            (layout.ncarriers(), layout.nsym)
        } else {
            (0, 0)
        };
        let known = if engine.is_some() {
            layout.known_grid()
        } else {
            Vec::new()
        };
        let mut ktime = vec![Vec::new(); k];
        for s in 0..ns {
            for (c, kt) in ktime.iter_mut().enumerate() {
                if known[s * k + c].is_some() {
                    kt.push(s);
                }
            }
        }
        for (c, kt) in ktime.iter_mut().enumerate() {
            if known[c].is_some() {
                kt.push(ns);
            }
        }
        let has: Vec<usize> = (0..k).filter(|&c| !ktime[c].is_empty()).collect();
        let sp = layout.spacing();
        let inside = |c: usize| match layout.pair_band {
            Some((a, b)) => {
                let f = ((layout.kmin + c as i32) as f64 * sp).abs();
                f >= a && f <= b
            }
            None => true,
        };
        let in_band: Vec<bool> = (0..k).map(inside).collect();
        let known_in_band = (0..ns)
            .map(|s| {
                (0..k)
                    .filter(|&c| known[s * k + c].is_some() && in_band[c])
                    .collect()
            })
            .collect();
        // Sync pairs: neighbours in time on one carrier, and neighbours in frequency in one symbol.
        let mut pairs = Vec::new();
        // Neighbours in frequency at any gap up to 8 (each gap its own group: a timing slope turns each gap by its own angle).
        let mut per_gap = [0usize; 65];
        for s in 0..ns {
            let row: Vec<(usize, Complex32)> = (0..k)
                .filter_map(|c| known[s * k + c].map(|v| (c, v)))
                .collect();
            for w in row.windows(2) {
                let g = w[1].0 - w[0].0;
                if g <= 64 {
                    per_gap[g] += 1;
                }
                if g <= 8 {
                    pairs.push(Pair {
                        s1: s,
                        k1: w[0].0,
                        s2: s,
                        k2: w[1].0,
                        r: w[0].1 * w[1].1.conj(),
                        dt: 0.0,
                        group: g,
                    });
                }
            }
        }
        // The delay is measured on the most common gap.
        let freq_gap = (1..65)
            .max_by_key(|&g| (per_gap[g], usize::MAX - g))
            .filter(|&g| per_gap[g] > 0)
            .unwrap_or(1);
        for s in 0..ns.saturating_sub(1) {
            for c in 0..k {
                if let (Some(a), Some(b)) = (known[s * k + c], known[(s + 1) * k + c]) {
                    pairs.push(Pair {
                        s1: s,
                        k1: c,
                        s2: s + 1,
                        k2: c,
                        r: a * b.conj(),
                        dt: (layout.sym_start(s + 1) + layout.sym_cp(s + 1)) as f64
                            - (layout.sym_start(s) + layout.sym_cp(s)) as f64,
                        group: 0,
                    });
                }
            }
        }
        pairs.retain(|p| in_band[p.k1] && in_band[p.k2]);
        let mut time_slot = vec![usize::MAX; k];
        let mut time_slots = 0;
        for p in pairs.iter().filter(|p| p.group == 0) {
            if time_slot[p.k1] == usize::MAX {
                time_slot[p.k1] = time_slots;
                time_slots += 1;
            }
        }
        let rows = if engine.is_some() { (ns + 1) * k } else { 0 };
        FrameDemodulator {
            engine,
            known,
            ktime,
            has,
            known_in_band,
            pairs,
            time_slot,
            time_slots,
            freq_gap,
            buf: Vec::new(),
            base: 0,
            state: FrameState::Searching,
            frame_start: 0,
            cfo: 0.0,
            mirror: false,
            frames: 0,
            shifts: 0,
            lock_at: 0,
            bad_frames: 0,
            search_from: 0,
            failed_searches: 0,
            clock: ClockDrift::new(1.0).unwrap_or_else(|_| unreachable!("lambda 1 is valid")),
            y: vec![Complex32::new(0.0, 0.0); rows],
            hraw: vec![Complex32::new(0.0, 0.0); rows],
            hgrid: vec![Complex32::new(0.0, 0.0); rows],
            have: vec![false; rows],
            src: Vec::new(),
            out: None,
            blocked: Vec::new(),
            delay_s: layout.adv as f32,
            level: 0.0,
            searches: 0,
            locks: 0,
            losses: 0,
            timing_moves: 0,
            hold: false,
            in_band,
            l: layout,
        }
    }

    /// The layout.
    pub fn layout(&self) -> &FrameLayout {
        &self.l
    }
    /// The state.
    pub fn state(&self) -> FrameState {
        self.state
    }
    /// The carrier offset being followed.
    pub fn cfo(&self) -> f64 {
        self.cfo
    }
    /// Whether the input is conjugated.
    pub fn mirrored(&self) -> bool {
        self.mirror
    }
    /// Conjugate the input from now on (or not).
    pub fn set_mirror(&mut self, m: bool) {
        if m != self.mirror {
            self.mirror = m;
            for s in &mut self.buf {
                *s = s.conj();
            }
        }
    }
    /// Freezes the frequency, timing and clock loops (a fade).
    pub fn set_hold(&mut self, hold: bool) {
        self.hold = hold;
    }
    /// Drops the lock and searches again from the newest samples.
    pub fn restart(&mut self) {
        if self.state == FrameState::Tracking {
            self.losses += 1;
        }
        self.state = FrameState::Searching;
        let keep = (self.l.frame_len() * 2 + 2 * self.l.nfft).min(self.buf.len());
        let drop = self.buf.len() - keep;
        self.buf.drain(..drop);
        self.base += drop as u64;
        self.search_from = self.base;
        self.bad_frames = 0;
    }
    /// Refuses the current lock (offset, mirror) for a while. The next search looks elsewhere.
    pub fn block_current(&mut self, seconds: f64) {
        let q = (self.cfo / self.l.spacing()).round() as i64;
        let until = self.base as f64 / self.l.rate + seconds;
        self.blocked.push((q, until));
        self.restart();
    }
    /// Forgets everything (a retune).
    pub fn reset(&mut self) {
        self.buf.clear();
        self.state = FrameState::Searching;
        self.search_from = self.base;
        self.cfo = 0.0;
        self.frames = 0;
        self.shifts = 0;
        self.bad_frames = 0;
        self.blocked.clear();
        self.failed_searches = 0;
        self.clock.reset();
    }
    /// Absolute index of the next input sample.
    pub fn position(&self) -> u64 {
        self.base + self.buf.len() as u64
    }

    /// Appends input samples (conjugated when mirrored).
    pub fn push(&mut self, x: &[Complex32]) {
        let start = self.buf.len();
        self.buf.extend_from_slice(x);
        let mut p = 0.0f64;
        for s in &mut self.buf[start..] {
            if self.mirror {
                *s = s.conj();
            }
            p += s.norm_sqr() as f64;
        }
        if !x.is_empty() {
            let a = (x.len() as f64 / self.l.rate / 0.5).min(1.0);
            self.level += a * (p / x.len() as f64 - self.level);
        }
    }

    /// Mean input power, dBFS.
    pub fn level_db(&self) -> f32 {
        (10.0 * self.level.max(1e-20).log10()) as f32
    }

    /// Runs the demodulator as far as the input allows. Returns a frame when one is complete.
    pub fn next_frame(&mut self) -> Option<Arc<CellFrame>> {
        self.engine.as_ref()?;
        loop {
            match self.state {
                FrameState::Searching => {
                    if !self.try_search() {
                        return None;
                    }
                }
                FrameState::Tracking => return self.try_frame(),
            }
        }
    }

    fn try_search(&mut self) -> bool {
        let l = &self.l;
        let ts = l.cp + l.nfft;
        let flen = l.frame_len();
        let need = 2 * flen + 2 * ts + l.nfft;
        if self.search_from < self.base {
            self.search_from = self.base;
        }
        let avail = self.position().saturating_sub(self.search_from) as usize;
        if avail < need {
            return false;
        }
        self.searches += 1;
        let found = self.search_at(self.search_from);
        if !found {
            self.failed_searches += 1;
            // Every other failed search looks at the conjugate.
            if self.failed_searches.is_multiple_of(2) {
                let m = !self.mirror;
                self.set_mirror(m);
            }
            let adv = (flen / 2) as u64;
            self.search_from += adv;
            let drop = (self.search_from - self.base) as usize;
            self.buf.drain(..drop);
            self.base = self.search_from;
        }
        true
    }

    /// The guard correlation folded over the symbol period, from `i0` in the buffer over `nsy` symbols: the offset in the
    /// period with the highest normalised correlation, that correlation, and its phase sum.
    fn guard_search(&mut self, i0: usize, nsy: usize) -> Option<(f32, usize, Complex32)> {
        let (n, cp) = (self.l.nfft, self.l.cp);
        let ts = cp + n;
        let len = nsy * ts + n + cp;
        let e = self.engine.as_mut()?;
        // The samples the correlation reads, filtered when the layout requires it.
        self.src.clear();
        match &mut e.fir {
            Some((fir, d)) => {
                fir.reset();
                let lead = fir.len() - 1;
                for q in 0..len + lead {
                    let idx = (i0 + q) as isize - *d as isize;
                    let v = if idx >= 0 && (idx as usize) < self.buf.len() {
                        self.buf[idx as usize]
                    } else {
                        Complex32::new(0.0, 0.0)
                    };
                    let o = fir.process_sample(v);
                    if q >= lead {
                        self.src.push(o);
                    }
                }
            }
            None => self
                .src
                .extend_from_slice(self.buf.get(i0..i0 + len).unwrap_or(&[])),
        }
        if self.src.len() < len {
            return None;
        }
        // P(t) = sum over the last cp samples of x[i] conj(x[i - n]). The window energy is |P| over the normalised metric.
        e.corr.reset();
        let mut acc = vec![Complex32::new(0.0, 0.0); ts];
        let mut en = vec![0.0f32; ts];
        let t0 = n + cp - 1;
        for (t, &v) in self.src.iter().enumerate() {
            let (m, p) = e.corr.push(v);
            if t < t0 {
                continue;
            }
            let off = t - t0;
            if off >= nsy * ts {
                break;
            }
            acc[off % ts] += p.conj();
            if m > 0.0 {
                en[off % ts] += p.norm() / m;
            }
        }
        // A continuous tone (a carrier beside or under the signal) correlates the same at every timing, a guard interval
        // only within one guard of its place. The tone's share is the correlation half a period from the peak. It is
        // taken out, so a tone alone scores near 0 and a tone under a signal does not pull the timing or the offset.
        let zero = Complex32::new(0.0, 0.0);
        let mean = acc.iter().sum::<Complex32>() / ts as f32;
        let peak = |base: Complex32| {
            let mut best = (0.0f32, 0usize, zero);
            for off in 0..ts {
                let v = acc[off] - base;
                let r = v.norm() / en[off].max(1e-20);
                if r > best.0 {
                    best = (r, off, v);
                }
            }
            best
        };
        let first = peak(mean).1;
        let half = (ts / 16).max(1);
        let far = first + ts / 2;
        let base = (far - half..=far + half)
            .map(|o| acc[o % ts])
            .sum::<Complex32>()
            / (2 * half + 1) as f32;
        Some(peak(base))
    }

    /// The FFT of the symbol whose useful part starts at absolute `useful`, the offset `cfo` removed: carriers `k0..` go to
    /// `out`. Returns false when the samples are not in the buffer.
    fn fft_wide(&mut self, useful: u64, cfo: f64, k0: i32, out: &mut [Complex32]) -> bool {
        let Some(e) = self.engine.as_mut() else {
            return false;
        };
        let Some(rel) = useful.checked_sub(self.base) else {
            return false;
        };
        e.framer.set_freq(cfo / self.l.rate);
        // The row of used cells is scratch here: the search reads the whole FFT.
        let used = self.l.ncarriers();
        let row = &mut self.hraw[..used];
        if e.framer
            .demodulate(&self.buf, rel as usize, self.base, row)
            .is_err()
        {
            return false;
        }
        let n = self.l.nfft;
        let norm = 1.0 / (n as f32).sqrt();
        let bins = e.framer.bins();
        for (c, o) in out.iter_mut().enumerate() {
            let kk = k0 + c as i32;
            *o = bins[kk.rem_euclid(n as i32) as usize] * norm;
        }
        true
    }

    fn search_at(&mut self, from: u64) -> bool {
        let (n, cp, ns) = (self.l.nfft, self.l.cp, self.l.nsym);
        let ts = cp + n;
        let i0 = (from - self.base) as usize;
        let nsy = ns.max(4);
        let Some(best) = self.guard_search(i0, nsy) else {
            return false;
        };
        if best.0 < self.l.guard_threshold {
            return false;
        }
        let frac = -(best.2.arg() as f64) * self.l.rate / (TAU * n as f64);
        let n0 = from + best.1 as u64;
        // FFTs of every uniform symbol position over two frames, and the candidates' symbol 0.
        let k = self.l.ncarriers();
        let cands = ns;
        // Wide enough for every whole-carrier offset searched.
        let q_max = self.l.max_shift.max(0);
        let kw = k + 2 * q_max as usize;
        let k0 = self.l.kmin - q_max;
        let rows = 2 * ns + 1;
        let mut y = vec![Complex32::new(0.0, 0.0); rows * kw];
        let mut tmp = vec![Complex32::new(0.0, 0.0); kw];
        for j in 0..rows {
            let useful = n0 + (j * ts) as u64 + cp as u64;
            if !self.fft_wide(useful, frac, k0, &mut tmp) {
                break;
            }
            y[j * kw..(j + 1) * kw].copy_from_slice(&tmp);
        }
        let mut y0 = vec![Complex32::new(0.0, 0.0); cands * kw];
        let cp0 = self.l.cp0;
        if cp0 != cp {
            for m in 0..cands {
                // Symbol 1 of candidate m is at uniform position m + 1. The useful part of symbol 0 ends where it starts.
                let s1 = n0 + (m as u64 + 1) * ts as u64;
                let Some(useful) = s1.checked_sub(n as u64) else {
                    continue;
                };
                if self.fft_wide(useful, frac, k0, &mut tmp) {
                    y0[m * kw..(m + 1) * kw].copy_from_slice(&tmp);
                }
            }
        }
        // Pairs weigh by their power up to a cap (the square of the mean cell power): cells outside the occupied band (noise) weigh
        // little, and one strong cell cannot weigh more than the others.
        let mean_p = y.iter().map(|v| v.norm_sqr()).sum::<f32>() / y.len().max(1) as f32;
        let cap = (mean_p * mean_p).max(1e-30);
        let now = self.base as f64 / self.l.rate;
        self.blocked.retain(|b| b.1 > now);
        let mut best_c = (0.0f32, 0usize, 0i32);
        let nf = n as f64;
        // The weight of the pairs in time, per carrier: one carrier may hold a third of the weight at most (its fair share
        // when the layout has fewer than three such carriers). A tone on one pilot carrier then cannot stand in for all.
        let mut per_carrier = vec![0.0f32; self.time_slots];
        let fair = self.time_slots.clamp(1, 3) as f32;
        for m in 0..cands {
            for q in -q_max..=q_max {
                let qq = (q as f64 + frac / self.l.spacing()).round() as i64;
                if self.blocked.iter().any(|b| b.0 == qq) {
                    continue;
                }
                let mut accs = [Complex32::new(0.0, 0.0); 9];
                let mut mag = 0.0f32;
                per_carrier.iter_mut().for_each(|w| *w = 0.0);
                for p in &self.pairs {
                    let c1 = (p.k1 as i32 + q + q_max) as usize;
                    let c2 = (p.k2 as i32 + q + q_max) as usize;
                    let row = |s: usize| -> &[Complex32] {
                        if s == 0 && cp0 != cp {
                            &y0[m * kw..(m + 1) * kw]
                        } else {
                            // Uniform positions: symbol s of candidate m is at m + s.
                            let j = m + s;
                            &y[j * kw..(j + 1) * kw]
                        }
                    };
                    let v = row(p.s1)[c1] * row(p.s2)[c2].conj();
                    let vm = v.norm() * p.r.norm();
                    if vm > 1e-30 {
                        // A whole-carrier offset turns the carriers between two symbols by 2π q dt / N.
                        let th = (TAU * q as f64 * p.dt / nf) as f32;
                        let wgt = (v.norm() / cap).min(1.0);
                        accs[p.group] +=
                            v * p.r.conj() * Complex32::new(th.cos(), th.sin()) * (wgt / vm);
                        mag += wgt;
                        if p.group == 0 {
                            per_carrier[self.time_slot[p.k1]] += wgt;
                        }
                    }
                }
                let most = per_carrier.iter().fold(0.0f32, |a, &w| a.max(w));
                let sc =
                    accs.iter().map(|a| a.norm()).sum::<f32>() / mag.max(fair * most).max(1e-20);
                if sc > best_c.0 {
                    best_c = (sc, m, q);
                }
            }
        }
        if best_c.0 < self.l.sync_threshold {
            return false;
        }
        let m = best_c.1;
        let start = if cp0 != cp {
            n0 + (m as u64 + 1) * ts as u64 - (cp0 + n) as u64
        } else {
            n0 + (m * ts) as u64
        };
        self.cfo = frac + best_c.2 as f64 * self.l.spacing();
        self.frame_start = start;
        self.state = FrameState::Tracking;
        self.frames = 0;
        self.shifts = 0;
        self.lock_at = start;
        self.locks += 1;
        self.bad_frames = 0;
        self.failed_searches = 0;
        self.delay_s = self.l.adv as f32;
        self.clock.reset();
        self.clock.push(start as f64, 0.0);
        true
    }

    /// The channel of every cell of `rows` symbols from the raw estimates at the known cells: in time per carrier, then in
    /// frequency, with the phase ramp of a delay of `ramp` samples taken out for the frequency step.
    fn interp(&mut self, rows: usize, ramp: f32) {
        let k = self.l.ncarriers();
        let n = self.l.nfft as f32;
        for s in 0..rows {
            for c in 0..k {
                if self.have[s * k + c] {
                    let kk = (self.l.kmin + c as i32) as f32;
                    let ph = TAU as f32 * kk * ramp / n;
                    self.hraw[s * k + c] *= Complex32::new(ph.cos(), ph.sin());
                }
            }
        }
        // In time, per carrier.
        for c in 0..k {
            let kt = &self.ktime[c];
            if kt.is_empty() {
                for s in 0..rows {
                    self.hgrid[s * k + c] = Complex32::new(0.0, 0.0);
                }
                continue;
            }
            for s in 0..rows {
                let pos = kt.partition_point(|&t| t < s);
                let v = if pos < kt.len() && kt[pos] == s {
                    self.hraw[s * k + c]
                } else if pos == 0 {
                    self.hraw[kt[0] * k + c]
                } else if pos == kt.len() {
                    self.hraw[kt[pos - 1] * k + c]
                } else {
                    let (a, b) = (kt[pos - 1], kt[pos]);
                    let f = (s - a) as f32 / (b - a) as f32;
                    self.hraw[a * k + c] * (1.0 - f) + self.hraw[b * k + c] * f
                };
                self.hgrid[s * k + c] = v;
            }
        }
        // In frequency, between the carriers that have estimates.
        let has = &self.has;
        if has.is_empty() {
            return;
        }
        for s in 0..rows {
            let row = &mut self.hgrid[s * k..(s + 1) * k];
            let mut j = 0;
            for c in 0..k {
                while j + 1 < has.len() && has[j + 1] <= c {
                    j += 1;
                }
                if has[j] == c {
                    continue;
                }
                let v = if c < has[0] {
                    row[has[0]]
                } else if j + 1 >= has.len() {
                    row[has[j]]
                } else {
                    let (a, b) = (has[j], has[j + 1]);
                    let f = (c - a) as f32 / (b - a) as f32;
                    row[a] * (1.0 - f) + row[b] * f
                };
                row[c] = v;
            }
            // Put the ramp back.
            for (c, h) in row.iter_mut().enumerate() {
                let kk = (self.l.kmin + c as i32) as f32;
                let ph = -(TAU as f32) * kk * ramp / n;
                *h *= Complex32::new(ph.cos(), ph.sin());
            }
        }
    }

    fn try_frame(&mut self) -> Option<Arc<CellFrame>> {
        let (n, ns) = (self.l.nfft, self.l.nsym);
        let k = self.l.ncarriers();
        let flen = self.l.frame_len() as u64;
        let end = self.frame_start + flen + (self.l.cp0 + n) as u64 + 8;
        if self.position() < end {
            return None;
        }
        if self.frame_start < self.base {
            self.restart();
            return None;
        }
        let adv = self.l.adv as f32;
        let e = self.engine.as_mut()?;
        e.framer.set_freq(self.cfo / self.l.rate);
        for s in 0..=ns {
            let useful = if s == ns {
                self.frame_start + flen + self.l.cp0 as u64
            } else {
                self.frame_start + (self.l.sym_start(s) + self.l.sym_cp(s)) as u64
            };
            let row = &mut self.y[s * k..(s + 1) * k];
            let rel = (useful - self.base) as usize;
            if e.framer.demodulate(&self.buf, rel, self.base, row).is_err() {
                row.fill(Complex32::new(0.0, 0.0));
            }
        }
        // Raw channel at the known cells.
        self.have.fill(false);
        for s in 0..=ns {
            let srow = if s == ns { 0 } else { s };
            for c in 0..k {
                if let Some(p) = self.known[srow * k + c] {
                    self.hraw[s * k + c] = self.y[s * k + c] / p;
                    self.have[s * k + c] = true;
                }
            }
        }
        // The next frame's first symbol can be missing (the transmission ended, a fade began): then the channel of this frame's
        // first symbol stands in for it.
        let p0: f32 = (0..k)
            .filter(|&c| self.have[c])
            .map(|c| self.hraw[c].norm_sqr())
            .sum();
        let pn: f32 = (0..k)
            .filter(|&c| self.have[ns * k + c])
            .map(|c| self.hraw[ns * k + c].norm_sqr())
            .sum();
        if pn < 0.25 * p0 {
            let (a, b) = self.hraw.split_at_mut(ns * k);
            b[..k].copy_from_slice(&a[..k]);
        }
        // Delay from the phase slope of neighbouring known cells in frequency. The framer removes the window's own `adv`.
        let g = self.freq_gap;
        let mut acc = Complex32::new(0.0, 0.0);
        for (s, row) in self.known_in_band.iter().enumerate() {
            for w in row.windows(2) {
                if w[1] - w[0] == g {
                    acc += self.hraw[s * k + w[1]] * self.hraw[s * k + w[0]].conj();
                }
            }
        }
        // Measured against the delay being followed, so a long guard cannot alias it. Nothing to measure keeps it.
        let slope = acc.arg() / g as f32;
        let d = if acc.norm() > 0.0 {
            self.delay_s
                + wrap_delay(
                    adv - slope * n as f32 / TAU as f32 - self.delay_s,
                    n as f32 / g as f32,
                )
        } else {
            self.delay_s
        };
        if !self.hold {
            self.delay_s += 0.5 * (d - self.delay_s);
        }
        let ramp = d - adv;
        // Noise from the second differences of the known cells (in time, else in frequency), and the pilot coherence.
        let mut noise = 0.0f64;
        let mut sig = 0.0f64;
        let mut cnt = 0usize;
        let mut cohs = [Complex32::new(0.0, 0.0); 9];
        let mut cohm = 0.0f32;
        let mean_p =
            self.y[..ns * k].iter().map(|v| v.norm_sqr()).sum::<f32>() / (ns * k).max(1) as f32;
        let cap = (mean_p * mean_p).max(1e-30);
        for p in &self.pairs {
            let v = self.y[p.s1 * k + p.k1] * self.y[p.s2 * k + p.k2].conj();
            let vm = v.norm() * p.r.norm();
            if vm > 1e-30 {
                let wgt = (v.norm() / cap).min(1.0);
                cohs[p.group] += v * p.r.conj() * (wgt / vm);
                cohm += wgt;
            }
        }
        let nf = n as f32;
        for c in 0..k {
            if !self.in_band[c] {
                continue;
            }
            for w in self.ktime[c].windows(3) {
                if w[1] - w[0] == w[2] - w[1] && w[2] - w[0] <= 6 {
                    let a = self.hraw[w[0] * k + c];
                    let b = self.hraw[w[1] * k + c];
                    let cc = self.hraw[w[2] * k + c];
                    let e = b - (a + cc) * 0.5;
                    noise += e.norm_sqr() as f64 / 1.5;
                    sig += b.norm_sqr() as f64;
                    cnt += 1;
                }
            }
        }
        if cnt < 8 {
            for (s, row) in self.known_in_band.iter().enumerate() {
                for w in row.windows(3) {
                    if w[1] - w[0] == w[2] - w[1] && w[1] - w[0] <= 4 {
                        let rot = |c: usize| {
                            let kk = (self.l.kmin + c as i32) as f32;
                            let ph = TAU as f32 * kk * ramp / nf;
                            self.hraw[s * k + c] * Complex32::new(ph.cos(), ph.sin())
                        };
                        let e = rot(w[1]) - (rot(w[0]) + rot(w[2])) * 0.5;
                        noise += e.norm_sqr() as f64 / 1.5;
                        sig += rot(w[1]).norm_sqr() as f64;
                        cnt += 1;
                    }
                }
            }
        }
        let sigma2 = if cnt > 0 {
            (noise / cnt as f64).max(1e-12)
        } else {
            1e-3
        };
        let sp = if cnt > 0 { sig / cnt as f64 } else { 1.0 };
        let snr = ((sp - sigma2).max(sp * 1e-4) / sigma2) as f32;
        let sync = cohs.iter().map(|a| a.norm()).sum::<f32>() / cohm.max(1e-20);
        // Carrier offset from the known cells on one carrier in neighbouring symbols.
        let mut rot = Complex32::new(0.0, 0.0);
        let span = self
            .ktime
            .iter()
            .flat_map(|kt| kt.windows(2).map(|w| w[1] - w[0]))
            .min();
        if let Some(span) = span {
            let skip0 = self.l.cp0 != self.l.cp;
            for c in (0..k).filter(|&c| self.in_band[c]) {
                for w in self.ktime[c].windows(2) {
                    if w[1] - w[0] == span && w[1] < ns && !(skip0 && w[0] == 0) {
                        rot += self.hraw[w[1] * k + c] * self.hraw[w[0] * k + c].conj();
                    }
                }
            }
        }
        let ts = (self.l.cp + n) as f64;
        let dfo = match span {
            Some(span) if rot.norm() > 0.0 => {
                rot.arg() as f64 * self.l.rate / (TAU * span as f64 * ts)
            }
            _ => 0.0,
        };
        // Channel estimate and equalisation.
        self.interp(ns + 1, ramp);
        let mut fr = self
            .out
            .take()
            .and_then(|a| Arc::try_unwrap(a).ok())
            .unwrap_or_default();
        fr.z.clear();
        fr.z.resize(ns * k, Complex32::new(0.0, 0.0));
        fr.w.clear();
        fr.w.resize(ns * k, 0.0);
        for s in 0..ns {
            let r = s * k..(s + 1) * k;
            if equalise(
                &self.y[r.clone()],
                &self.hgrid[r.clone()],
                sigma2 as f32,
                &mut fr.z[r.clone()],
                &mut fr.w[r.clone()],
            )
            .is_err()
            {
                fr.z[r.clone()].fill(Complex32::new(0.0, 0.0));
                fr.w[r].fill(0.0);
            }
        }
        fr.h.clear();
        fr.h.extend_from_slice(&self.hgrid[..k]);
        fr.index = self.frames;
        fr.first = self.frames == 0;
        fr.start = self.frame_start;
        fr.time = self.frame_start as f64 / self.l.rate;
        fr.snr_db = 10.0 * snr.max(1e-6).log10();
        fr.sync = sync;
        fr.delay = d;
        fr.mirrored = self.mirror;
        fr.level_db = self.level_db();
        // Loops.
        let mut next = self.frame_start + flen;
        if !self.hold && sync > 0.2 {
            self.cfo += 0.5 * dfo;
            let e = self.delay_s - adv;
            if e.abs() >= 0.75 {
                let mv = e.round() as i64;
                next = (next as i64 + mv) as u64;
                self.shifts += mv;
                self.delay_s -= mv as f32;
                self.timing_moves += 1;
            }
        }
        // The timing since the lock: whole moves plus the delay followed.
        self.clock.push(
            (self.frame_start + flen) as f64,
            self.shifts as f64 + (self.delay_s - adv) as f64,
        );
        fr.ppm = self.clock.ppm().unwrap_or(0.0);
        fr.cfo_hz = self.cfo;
        self.frames += 1;
        if sync < self.l.sync_threshold * 0.5 {
            self.bad_frames += 1;
        } else {
            self.bad_frames = 0;
        }
        self.frame_start = next;
        // Drop the samples before the kept range.
        let keep_from = self.frame_start.saturating_sub(self.l.nfft as u64);
        if keep_from > self.base + (4 * flen) {
            let drop = (keep_from - self.base) as usize;
            self.buf.drain(..drop);
            self.base = keep_from;
        }
        let a = Arc::new(fr);
        self.out = Some(a.clone());
        if self.bad_frames >= 6 {
            self.restart();
        }
        Some(a)
    }

    /// Sample clock offset since the lock, ppm.
    pub fn ppm(&self) -> f64 {
        self.clock.ppm().unwrap_or(0.0)
    }
    /// The current frame start (absolute).
    pub fn frame_start(&self) -> u64 {
        self.frame_start
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test frame: 256-point FFT at 48 kHz, guards of 64 (first) and 32 samples, 12 symbols, carriers -60 to 60 with DC unused.
    /// Known cells sit on every sixth carrier, moved by 2 each symbol, with values of a fixed phase pattern.
    fn layout() -> FrameLayout {
        let k = 121;
        let known = (0..12)
            .map(|s| {
                (0..k)
                    .filter(|&c| c != 60 && (c + 2 * s) % 6 == 0)
                    .map(|c| {
                        let ph = ((c * 7 + s * 3) % 4) as f32 * std::f32::consts::FRAC_PI_2;
                        (c, Complex32::from_polar(1.0, ph))
                    })
                    .collect()
            })
            .collect();
        FrameLayout {
            rate: 48e3,
            nfft: 256,
            cp: 32,
            cp0: 64,
            nsym: 12,
            kmin: -60,
            kmax: 60,
            known,
            null: vec![60],
            max_shift: 4,
            adv: 16,
            sync_threshold: 0.45,
            guard_threshold: 0.35,
            search_taps: None,
            pair_band: None,
        }
    }

    #[test]
    fn frame_arithmetic() {
        let l = layout();
        assert_eq!(l.ncarriers(), 121);
        assert_eq!(l.spacing(), 187.5);
        // 64 + 256 for symbol 0, then 11 symbols of 32 + 256.
        assert_eq!(l.frame_len(), 320 + 11 * 288);
        assert_eq!(l.sym_start(0), 0);
        assert_eq!(l.sym_start(1), 320);
        assert_eq!(l.sym_start(2), 608);
        assert_eq!((l.sym_cp(0), l.sym_cp(5)), (64, 32));
        let data = l.data_cells();
        let known: usize = l.known.iter().map(|r| r.len()).sum();
        assert_eq!(data.len(), 12 * 120 - known);
        assert!(l.ofdm().is_ok());
    }

    #[test]
    fn modulator_returns_the_inverse_transform_with_a_cyclic_guard() {
        // One cell of value v on carrier 5: x[n] = v exp(j 2 pi 5 n / 256) / sqrt(120), with 120 active carriers.
        let l = layout();
        let mut m = FrameModulator::new(l.clone());
        let mut cells = vec![Complex32::default(); 121];
        let v = Complex32::new(0.6, -0.8);
        cells[(5 - l.kmin) as usize] = v;
        let mut out = Vec::new();
        m.symbol(0, &cells, &mut out);
        assert_eq!(out.len(), 64 + 256);
        let scale = 1.0 / 120f32.sqrt();
        for n in 0..256 {
            let want =
                v * Complex32::from_polar(scale, std::f32::consts::TAU * 5.0 * n as f32 / 256.0);
            assert!((out[64 + n] - want).norm() < 1e-5, "sample {n}");
        }
        for i in 0..64 {
            assert_eq!(out[i], out[256 + i]);
        }
    }

    #[test]
    fn demodulator_returns_the_cells_through_an_offset() {
        let l = layout();
        let k = l.ncarriers();
        let grid = l.known_grid();
        let mut seed = 1u32;
        let mut rnd = || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (seed >> 16) & 3
        };
        let mut m = FrameModulator::new(l.clone());
        let mut frames = Vec::new();
        let mut x = vec![Complex32::default(); 500];
        for _ in 0..8 {
            let mut cells = vec![Complex32::default(); l.nsym * k];
            for s in 0..l.nsym {
                for c in 0..k {
                    cells[s * k + c] = match grid[s * k + c] {
                        Some(v) => v,
                        None if c == 60 => Complex32::default(),
                        None => Complex32::from_polar(
                            1.0,
                            std::f32::consts::FRAC_PI_4 * (2 * rnd() + 1) as f32,
                        ),
                    };
                }
            }
            m.frame(&cells, &mut x);
            frames.push(cells);
        }
        // A carrier offset of 0.3 of the spacing.
        let f = 0.3 * l.spacing() / l.rate;
        for (n, v) in x.iter_mut().enumerate() {
            *v *= Complex32::from_polar(1.0, (std::f64::consts::TAU * f * n as f64) as f32);
        }
        let mut d = FrameDemodulator::try_new(l.clone()).unwrap();
        d.push(&x);
        let mut got = Vec::new();
        while let Some(fr) = d.next_frame() {
            got.push(fr);
        }
        assert!(got.len() >= 4, "{} frames", got.len());
        let last = got.last().unwrap();
        assert!(
            (last.cfo_hz - 0.3 * l.spacing()).abs() < 2.0,
            "cfo {}",
            last.cfo_hz
        );
        // The frame start identifies which frame was sent.
        let idx = ((last.start - 500) as usize) / l.frame_len();
        let sent = &frames[idx];
        let mut err = 0.0f32;
        let mut n = 0;
        for s in 0..l.nsym {
            for c in 0..k {
                if grid[s * k + c].is_none() && c != 60 {
                    err += (last.z[s * k + c] - sent[s * k + c]).norm_sqr();
                    n += 1;
                }
            }
        }
        assert!(err / (n as f32) < 1e-3, "mean error {}", err / n as f32);
    }
}
