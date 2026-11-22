//! The symbol layer of the FSK stacks: frame sync search on every sample, polarity, level and offset fit, decisions with soft
//! values, the symbol clock loop between syncs and the measurements taken on the syncs.

use sdrkit_algo_dsp::interp::{CatmullRom, Interpolator};
use sdrkit_algo_dsp::loopfilter::LoopFilter;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_modem::timing::{detector_gain, Gardner, TimingDetector};

/// A frame sync pattern.
#[derive(Clone, Debug)]
pub struct SyncPattern {
    pub id: u8,
    /// Nominal levels (+3, +1, -1, -3, or +1/-1 for two-level stacks).
    pub symbols: Vec<i8>,
    /// Symbols before the sync that the protocol requires with the sync event.
    pub before: usize,
    /// Most symbols that can differ (default L/8).
    pub max_err: usize,
    /// The inverted pattern is another valid sync (so polarity is not taken from the sign).
    pub no_auto_invert: bool,
}

impl SyncPattern {
    /// From a bit string of dibits (01 +3, 00 +1, 10 -1, 11 -3) given as hex digits.
    pub fn from_hex_dibits(id: u8, hex: &str, before: usize) -> SyncPattern {
        let mut symbols = Vec::new();
        for c in hex.chars() {
            let v = c.to_digit(16).expect("a constant hex digit") as u8;
            symbols.push(sdrkit_algo_fec::conv::tcm::dibit_symbol(v >> 2));
            symbols.push(sdrkit_algo_fec::conv::tcm::dibit_symbol(v & 3));
        }
        let max_err = symbols.len() / 8;
        SyncPattern {
            id,
            symbols,
            before,
            max_err,
            no_auto_invert: false,
        }
    }
    /// From bits for a two-level stack (1 is +1).
    pub fn from_bits(id: u8, bits: &str, before: usize) -> SyncPattern {
        let symbols: Vec<i8> = bits
            .chars()
            .filter(|c| *c == '0' || *c == '1')
            .map(|c| if c == '1' { 1 } else { -1 })
            .collect();
        let max_err = symbols.len() / 8;
        SyncPattern {
            id,
            symbols,
            before,
            max_err,
            no_auto_invert: false,
        }
    }
    pub fn with_max_err(mut self, e: usize) -> Self {
        self.max_err = e;
        self
    }
    pub fn no_auto_invert(mut self) -> Self {
        self.no_auto_invert = true;
        self
    }
}

/// One decided symbol.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sym {
    /// Normalised soft value (nominal levels -3, -1, +1, +3, or -1, +1).
    pub v: f32,
    /// The decision.
    pub d: i8,
}

impl Sym {
    /// The dibit (01 +3, 00 +1, 10 -1, 11 -3).
    pub fn dibit(&self) -> u8 {
        sdrkit_algo_fec::conv::tcm::symbol_dibit(self.d).unwrap_or(0b11)
    }
    /// For two-level stacks: 1 for +1.
    pub fn bit(&self) -> u8 {
        (self.d > 0) as u8
    }
    /// Soft value of the first bit of the dibit (+1 sure one) and the second.
    pub fn soft_dibit(&self) -> (f32, f32) {
        // First bit is 1 for -1 and -3 (negative). Second bit is 1 for -3 and +3 (outer).
        let b1 = (-self.v / 2.0).clamp(-1.0, 1.0);
        let b2 = ((self.v.abs() - 2.0) / 1.0).clamp(-1.0, 1.0);
        (b1, b2)
    }
}

/// Returns whether the carrier was lost within these symbols: more than a quarter are at the centre (a discriminator fed
/// silence outputs zero).
pub fn carrier_gone(s: &[Sym]) -> bool {
    carrier_gone_levels(s.iter().map(|x| x.v))
}

/// [`carrier_gone`] on the soft values of the symbols.
pub fn carrier_gone_levels(v: impl Iterator<Item = f32> + Clone) -> bool {
    let mut run = 0;
    let mut longest = 0;
    for x in v.clone() {
        if x.abs() < 0.3 {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let (weak, n) = v.fold((0, 0), |(w, n), x| (w + (x.abs() < 0.25) as usize, n + 1));
    longest >= 6 || weak * 4 > n
}

/// What the protocol answers to a sync.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncAnswer {
    /// Taken: deliver the following symbols.
    Take,
    /// Not needed (the protocol is busy and this does not fit): continue as before.
    Ignore,
}

/// What the protocol answers to a symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymAnswer {
    Continue,
    /// The carrier is gone: back to searching.
    Stop,
    /// The symbol belongs to no burst of the stream (the silent slot of a TDMA channel): tracking goes on, and the level,
    /// centre and clock loops do not move on it.
    Hold,
}

/// A protocol fed by the symbol layer.
pub trait SymbolSink {
    /// A sync `id` whose last symbol is at sample `at`. `pre` holds the requested symbols before it and the sync itself.
    fn sync(&mut self, id: u8, inverted: bool, pre: &[Sym], at: u64, tracking: bool) -> SyncAnswer;
    /// The next symbol, centred at sample `at`.
    fn symbol(&mut self, s: Sym, at: u64) -> SymAnswer;
}

struct Template {
    id: u8,
    p: Vec<f32>,
    mean: f32,
    norm: f32,
    before: usize,
    max_err: usize,
    auto_invert: bool,
}

const RING: usize = 1 << 15;

/// The noise bandwidth of the symbol clock loop, per symbol: it follows a clock error of a few hundred ppm between syncs and
/// averages the detector over about a hundred symbols.
const CLOCK_BN_T: f64 = 0.004;
/// The gain of the Gardner detector on the symbol layer's pulses (the slope of its S-curve for a raised cosine pulse of roll-off
/// 0.2, per symbol of timing error).
fn clock_loop() -> LoopFilter {
    let k = detector_gain::<f32, Gardner>(&Gardner, Shape::Rc { beta: 0.2 }, false).unwrap_or(1.0);
    let mut l = LoopFilter::with_gain(CLOCK_BN_T, std::f64::consts::FRAC_1_SQRT_2, k)
        .expect("a valid loop bandwidth");
    // A clock error of at most 1000 ppm, as a timing step per symbol.
    l.set_clamp(1e-3);
    l
}

/// The symbol layer.
pub struct SymbolLayer {
    pub sps: usize,
    pub two_level: bool,
    ring: Vec<f32>,
    n: u64,
    tpl: Vec<Template>,
    max_len: usize,
    /// Polarity: the discriminator is turned over.
    pub inverted: bool,
    tracking: bool,
    next_pos: f64,
    gain: f32,
    dc: f32,
    prev_v: f32,
    cands: Vec<Option<(u64, f32, bool)>>,
    /// Templates by decreasing length (the longer sync wins a tie).
    order: Vec<usize>,
    /// Where each template last reported a sync.
    reported: Vec<u64>,
    pre: Vec<Sym>,
    /// The symbol clock loop between syncs: a Gardner detector and a second-order loop filter (its output is a timing
    /// correction in symbols).
    ted: Gardner,
    clock: LoopFilter,
    /// Mean power of the nominal levels (5 for four levels, 1 for two), to bring the detector's input to unit power.
    level_power: f32,
    /// The clock loop runs (it needs a midpoint between symbols: two samples a symbol or more).
    clock_on: bool,
    // Measurements.
    scatter_acc: f64,
    scatter_n: u64,
    pub eye: Vec<f32>,
    eye_pos: usize,
    pub hist: [u32; 40],
    pub syncs_seen: u64,
    clock_err_acc: f64,
    clock_err_n: u64,
    last_sync_at: Option<u64>,
    last_sync_symbols: u64,
    symbols_since_sync: u64,
    clock_ppm: f32,
    /// Sync threshold (normalised correlation).
    pub threshold: f32,
    sync_dc: f32,
    sync_gain: f32,
    pending: Vec<(f32, f32)>,
    pending_acc: f64,
    pending_n: u64,
    committed: u64,
}

impl SymbolLayer {
    pub fn new(sps: usize, two_level: bool, patterns: &[SyncPattern]) -> SymbolLayer {
        let tpl: Vec<Template> = patterns
            .iter()
            .map(|p| {
                let pf: Vec<f32> = p.symbols.iter().map(|&s| s as f32).collect();
                let mean = pf.iter().sum::<f32>() / pf.len() as f32;
                let norm = pf.iter().map(|x| (x - mean).powi(2)).sum::<f32>().sqrt();
                Template {
                    id: p.id,
                    p: pf,
                    mean,
                    norm,
                    before: p.before,
                    max_err: p.max_err,
                    auto_invert: !p.no_auto_invert,
                }
            })
            .collect();
        let max_len = tpl.iter().map(|t| t.p.len() + t.before).max().unwrap_or(0);
        assert!(
            (max_len + 4) * sps < RING,
            "sync history longer than the ring"
        );
        SymbolLayer {
            sps,
            two_level,
            ring: vec![0.0; RING],
            n: 0,
            tpl,
            max_len,
            inverted: false,
            tracking: false,
            next_pos: 0.0,
            gain: 1.0,
            dc: 0.0,
            prev_v: 0.0,
            cands: vec![None; patterns.len()],
            reported: vec![0; patterns.len()],
            order: {
                let mut o: Vec<usize> = (0..patterns.len()).collect();
                o.sort_by_key(|&i| std::cmp::Reverse(patterns[i].symbols.len()));
                o
            },
            pre: Vec::with_capacity(max_len + 4),
            ted: Gardner,
            clock: clock_loop(),
            level_power: if two_level { 1.0 } else { 5.0 },
            clock_on: sps >= 2,
            scatter_acc: 0.0,
            scatter_n: 0,
            eye: vec![0.0; 800],
            eye_pos: 0,
            hist: [0; 40],
            syncs_seen: 0,
            clock_err_acc: 0.0,
            clock_err_n: 0,
            last_sync_at: None,
            last_sync_symbols: 0,
            symbols_since_sync: 0,
            clock_ppm: 0.0,
            threshold: 0.85,
            sync_dc: 0.0,
            sync_gain: 1.0,
            pending: Vec::with_capacity(64),
            pending_acc: 0.0,
            pending_n: 0,
            committed: 0,
        }
    }

    /// Turns the clock loop between syncs on or off (it is on when there are two samples a symbol or more).
    pub fn set_clock_loop(&mut self, on: bool) {
        self.clock_on = on && self.sps >= 2;
        self.clock.reset();
    }

    /// Forget sync and history (a retune).
    pub fn reset(&mut self) {
        self.tracking = false;
        self.clock.reset();
        self.cands.iter_mut().for_each(|c| *c = None);
        self.ring.iter_mut().for_each(|x| *x = 0.0);
        self.last_sync_at = None;
    }

    /// Keeps (when the protocol decoded something since the last call) or drops the measurements of the syncs found since.
    pub fn commit(&mut self, good: bool) {
        if good {
            for &(dc, g) in &self.pending {
                let a = if self.committed == 0 { 1.0 } else { 0.2 };
                self.sync_dc += a * (dc - self.sync_dc);
                self.sync_gain += a * (g - self.sync_gain);
                self.committed += 1;
            }
            self.scatter_acc += self.pending_acc;
            self.scatter_n += self.pending_n;
        }
        self.pending.clear();
        self.pending_acc = 0.0;
        self.pending_n = 0;
    }

    /// No carrier: stop delivering symbols until the next sync.
    pub fn stop(&mut self) {
        self.tracking = false;
        self.cands.iter_mut().for_each(|c| *c = None);
    }

    pub fn tracking(&self) -> bool {
        self.tracking
    }
    /// The fitted outer level, in hertz (the deviation of the outer symbols).
    pub fn deviation_hz(&self) -> f32 {
        self.sync_gain.abs() * if self.two_level { 1.0 } else { 3.0 }
    }
    /// The fitted centre, in hertz (the residual carrier offset), averaged over the syncs.
    pub fn dc_hz(&self) -> f32 {
        self.sync_dc
    }
    /// The present centre (for the fine NCO loop).
    pub fn dc_now(&self) -> f32 {
        if self.tracking {
            self.dc
        } else {
            0.0
        }
    }
    /// Moves the fitted centre after the front end moved the carrier.
    pub fn shift_dc(&mut self, hz: f32) {
        self.dc -= hz;
        self.sync_dc -= hz;
    }
    /// Symbol SNR (dB) since the last call, then restarts the average.
    pub fn take_snr_db(&mut self) -> Option<f32> {
        if self.scatter_n < 20 {
            return None;
        }
        let rms = (self.scatter_acc / self.scatter_n as f64).sqrt().max(1e-4);
        self.scatter_acc = 0.0;
        self.scatter_n = 0;
        Some((20.0 * (2.0 / rms).log10()) as f32)
    }
    pub fn clock_ppm(&self) -> f32 {
        self.clock_ppm
    }
    /// The eye, oldest first.
    pub fn eye_ordered(&self, out: &mut Vec<f32>) {
        out.clear();
        out.extend_from_slice(&self.eye[self.eye_pos..]);
        out.extend_from_slice(&self.eye[..self.eye_pos]);
    }
    pub fn samples(&self) -> u64 {
        self.n
    }

    #[inline]
    fn at(&self, idx: u64) -> f32 {
        self.ring[(idx as usize) & (RING - 1)]
    }

    /// The signal at a fractional position (Catmull-Rom. Needs the sample after the next one, or uses the next).
    #[inline]
    fn interp(&self, pos: f64) -> f32 {
        let i = pos.floor();
        let f = (pos - i) as f32;
        let i = i as u64;
        let y0 = if i > 0 { self.at(i - 1) } else { self.at(i) };
        let y1 = self.at(i);
        let y2 = self.at(i + 1);
        let y3 = if i + 2 < self.n { self.at(i + 2) } else { y2 };
        CatmullRom.at(&[y0, y1, y2, y3], f)
    }

    fn sign(&self) -> f32 {
        if self.inverted {
            -1.0
        } else {
            1.0
        }
    }

    fn decide(&self, raw: f32) -> Sym {
        let v = (raw - self.dc) / self.gain * self.sign();
        let d = if self.two_level {
            if v >= 0.0 {
                1
            } else {
                -1
            }
        } else if v >= 2.0 {
            3
        } else if v >= 0.0 {
            1
        } else if v >= -2.0 {
            -1
        } else {
            -3
        };
        Sym { v, d }
    }

    fn record(&mut self, s: Sym) {
        self.eye[self.eye_pos] = s.v;
        self.eye_pos = (self.eye_pos + 1) % self.eye.len();
        let b = ((s.v + 5.0) / 0.25).floor();
        if (0.0..40.0).contains(&b) {
            self.hist[b as usize] += 1;
        }
    }

    /// Pearson correlation of template `t` with the window whose last symbol is at sample `end`.
    fn correlate(&self, t: &Template, end: u64) -> f32 {
        let l = t.p.len();
        let sps = self.sps as u64;
        let first = end - (l as u64 - 1) * sps;
        let mut sx = 0f32;
        let mut sxx = 0f32;
        let mut sxp = 0f32;
        for (k, &p) in t.p.iter().enumerate() {
            let x = self.at(first + k as u64 * sps);
            sx += x;
            sxx += x * x;
            sxp += x * (p - t.mean);
        }
        let var = sxx - sx * sx / l as f32;
        if var <= 1e-6 || t.norm <= 0.0 {
            return 0.0;
        }
        sxp / (var.sqrt() * t.norm)
    }

    /// Feeds filtered samples. Calls the protocol for syncs and symbols.
    pub fn process<S: SymbolSink>(&mut self, x: &[f32], sink: &mut S) {
        for &v in x {
            let v = if v.is_finite() { v } else { 0.0 };
            let idx = self.n;
            self.ring[(idx as usize) & (RING - 1)] = v;
            self.n += 1;
            // Symbol output while tracking.
            if self.tracking && (idx as f64) >= self.next_pos.floor() + 1.0 && self.next_pos >= 1.0
            {
                let pos = self.next_pos;
                let raw = self.interp(pos);
                let s = self.decide(raw);
                let answer = sink.symbol(s, pos as u64);
                let moves = answer != SymAnswer::Hold;
                // Gardner timing error with the midpoint between this and the previous symbol, at unit power.
                if self.clock_on && moves {
                    let mid = self.interp(pos - self.sps as f64 / 2.0);
                    let midn = (mid - self.dc) / self.gain * self.sign();
                    let err = self.ted.error(self.prev_v, midn, s.v) / self.level_power;
                    if err.is_finite() {
                        let step = self.clock.update(err.clamp(-1.0, 1.0) as f64);
                        self.next_pos += step * self.sps as f64;
                    }
                }
                // Slow level and centre refinement from the decisions.
                let target = self.dc + self.gain * self.sign() * s.d as f32;
                let e = raw - target;
                if moves {
                    self.dc += 0.002 * e;
                }
                if moves && (s.d.abs() == 3 || self.two_level) {
                    let g = (raw - self.dc) * self.sign() / s.d as f32;
                    if g.is_finite() && g * self.gain > 0.0 {
                        self.gain += 0.002 * (g - self.gain);
                    }
                }
                self.prev_v = s.v;
                self.record(s);
                self.next_pos += self.sps as f64;
                self.symbols_since_sync += 1;
                if answer == SymAnswer::Stop {
                    self.tracking = false;
                }
            }
            // Sync search.
            if idx < (self.max_len as u64 + 2) * self.sps as u64 {
                continue;
            }
            // Each pattern keeps its own candidate (the strongest correlation within half a symbol), so patterns of different
            // lengths that overlap (a short one inside a long one) do not hide each other.
            for ti in 0..self.tpl.len() {
                let t = &self.tpl[ti];
                let c = self.correlate(t, idx) * self.sign();
                let (mag, neg) = if t.auto_invert {
                    (c.abs(), c < 0.0)
                } else {
                    (c, false)
                };
                // A sync just reported is not found again in the wings of its own correlation peak.
                if mag >= self.threshold
                    && idx > self.reported[ti] + (t.p.len() as u64 * self.sps as u64) / 2
                {
                    match self.cands[ti] {
                        Some((_, cc, _)) if cc >= mag => {}
                        _ => self.cands[ti] = Some((idx, mag, neg)),
                    }
                }
            }
            let half = (self.sps as u64) / 2 + 1;
            for k in 0..self.order.len() {
                let ti = self.order[k];
                if let Some((at, _, neg)) = self.cands[ti] {
                    if idx >= at + half {
                        self.cands[ti] = None;
                        self.reported[ti] = at;
                        self.try_sync(at, ti, neg, sink);
                    }
                }
            }
        }
    }

    fn try_sync<S: SymbolSink>(&mut self, at: u64, ti: usize, neg: bool, sink: &mut S) {
        // Fractional peak of the correlation (parabola through three samples. None at one sample per symbol).
        let frac = if self.sps < 4 {
            0.0
        } else {
            let t = &self.tpl[ti];
            let (a, b, c) = (
                self.correlate(t, at - 1).abs(),
                self.correlate(t, at).abs(),
                self.correlate(t, at + 1).abs(),
            );
            let den = a - 2.0 * b + c;
            if den < 0.0 {
                (0.5 * (a - c) / den).clamp(-0.5, 0.5) as f64
            } else {
                0.0
            }
        };
        let t = &self.tpl[ti];
        let l = t.p.len();
        let sps = self.sps as u64;
        let first = at - (l as u64 - 1) * sps;
        // Level and centre fit: raw = dc + g * p (sign included).
        let pol = self.sign() * if neg { -1.0 } else { 1.0 };
        let mut sx = 0f32;
        let mut sp = 0f32;
        let mut spp = 0f32;
        let mut sxp = 0f32;
        for (k, &p) in t.p.iter().enumerate() {
            let x = self.interp((first + k as u64 * sps) as f64 + frac);
            let p = p * pol;
            sx += x;
            sp += p;
            spp += p * p;
            sxp += x * p;
        }
        let lf = l as f32;
        let den = spp - sp * sp / lf;
        if den <= 0.0 {
            return;
        }
        let g = (sxp - sx * sp / lf) / den;
        if g <= 0.0 {
            return;
        }
        let dc = (sx - g * sp) / lf;
        // Symbol check.
        let mut errs = 0;
        for (k, &p) in t.p.iter().enumerate() {
            let x = self.interp((first + k as u64 * sps) as f64 + frac);
            let v = (x - dc) / g;
            let d = if self.two_level {
                v.signum()
            } else if v >= 2.0 {
                3.0
            } else if v >= 0.0 {
                1.0
            } else if v >= -2.0 {
                -1.0
            } else {
                -3.0
            };
            if (d - p * pol).abs() > 0.5 {
                errs += 1;
            }
        }
        if errs > t.max_err {
            return;
        }
        let (id, before) = (t.id, t.before);
        let save = (self.gain, self.dc, self.inverted);
        if neg {
            self.inverted = !self.inverted;
        }
        self.gain = g;
        self.dc = dc;
        // Sample the requested symbols back from history.
        let total = before + l;
        let start = at as i64 - (total as i64 - 1) * sps as i64;
        self.pre.clear();
        for k in 0..total {
            let p = start + (k as i64) * sps as i64;
            let raw = if p >= 1 && (self.n - p as u64) < RING as u64 - 2 {
                self.interp(p as f64 + frac)
            } else {
                self.dc
            };
            let s = self.decide(raw);
            self.pre.push(s);
        }
        let pre = std::mem::take(&mut self.pre);
        let tracking = self.tracking;
        let ans = sink.sync(id, self.inverted, &pre, at, tracking);
        self.pre = pre;
        match ans {
            SyncAnswer::Take => {
                self.syncs_seen += 1;
                // Symbol SNR, centre and deviation from the sync symbols. Kept once the protocol decodes something
                // ([`SymbolLayer::commit`]), so a sync found in noise does not count.
                let l = self.tpl[ti].p.len();
                self.pending.push((self.dc, self.gain));
                for s in self.pre[self.pre.len() - l..].iter() {
                    let e = (s.v - s.d as f32) as f64;
                    self.pending_acc += e * e;
                    self.pending_n += 1;
                }
                // Clock: symbols between this sync and the last one against samples.
                if let Some(last) = self.last_sync_at {
                    let syms = self.symbols_since_sync + self.last_sync_symbols;
                    let _ = syms;
                    let dt = (at - last) as f64;
                    let nominal = (dt / self.sps as f64).round() * self.sps as f64;
                    if nominal > 0.0 && (dt - nominal).abs() < 3.0 {
                        let ppm = (nominal - dt) / nominal * 1e6;
                        self.clock_err_acc = 0.9 * self.clock_err_acc + 0.1 * ppm;
                        self.clock_err_n += 1;
                        self.clock_ppm = self.clock_err_acc as f32;
                    }
                }
                self.last_sync_at = Some(at);
                self.symbols_since_sync = 0;
                self.tracking = true;
                self.next_pos = at as f64 + frac + self.sps as f64;
                self.prev_v = self.pre.last().map_or(0.0, |s| s.v);
            }
            SyncAnswer::Ignore => {
                (self.gain, self.dc, self.inverted) = save;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Collect {
        syncs: Vec<(u8, bool, usize)>,
        syms: Vec<i8>,
        stop_after: usize,
    }
    impl SymbolSink for Collect {
        fn sync(&mut self, id: u8, inv: bool, pre: &[Sym], _at: u64, _t: bool) -> SyncAnswer {
            self.syncs.push((id, inv, pre.len()));
            self.syms.clear();
            SyncAnswer::Take
        }
        fn symbol(&mut self, s: Sym, _at: u64) -> SymAnswer {
            self.syms.push(s.d);
            if self.syms.len() >= self.stop_after {
                SymAnswer::Stop
            } else {
                SymAnswer::Continue
            }
        }
    }

    #[test]
    fn finds_a_sync_with_offset_and_scaled_levels_and_reads_what_follows() {
        let pat = SyncPattern::from_hex_dibits(1, "755FD7DF75F7", 4);
        let mut syms: Vec<i8> = vec![1, -1, 3, -3, 1, 1, -3, 3];
        syms.extend(&pat.symbols);
        let payload: Vec<i8> = (0..40).map(|i| [3, 1, -1, -3][(i * 7 + 2) % 4]).collect();
        syms.extend(&payload);
        syms.extend([1; 20]);
        // 10 samples per symbol, deviation 1700 Hz for +3 (off by 12 %), 300 Hz carrier offset, inverted.
        let mut x = vec![0f32; 300];
        for &s in &syms {
            for _ in 0..10 {
                x.push(-(s as f32 * 1700.0 / 3.0) + 300.0);
            }
        }
        // Through an integrate-and-dump filter, as the symbol filter does.
        let x: Vec<f32> = (0..x.len())
            .map(|i| x[i.saturating_sub(9)..=i].iter().sum::<f32>() / 10.0)
            .collect();
        let mut layer = SymbolLayer::new(10, false, &[pat]);
        let mut c = Collect {
            syncs: vec![],
            syms: vec![],
            stop_after: 40,
        };
        layer.process(&x, &mut c);
        assert_eq!(c.syncs, vec![(1, true, 28)]);
        assert_eq!(c.syms, payload);
        // The measurements count once the protocol decoded something.
        layer.commit(true);
        assert!(
            (layer.deviation_hz() - 1700.0).abs() < 50.0,
            "{}",
            layer.deviation_hz()
        );
        assert!((layer.dc_hz() - 300.0).abs() < 20.0, "{}", layer.dc_hz());
    }

    /// Four-level symbols after one sync, as the symbol filter outputs them: raised cosine pulses at 10 samples a symbol, with
    /// the transmitter's clock `ppm` fast.
    fn drifting_signal(pat: &SyncPattern, payload: &[i8], ppm: f64) -> Vec<f32> {
        use sdrkit_core::Complex32;
        let mut syms: Vec<i8> = vec![1, -1, 3, -3, 1, 1, -3, 3];
        syms.extend(&pat.symbols);
        syms.extend(payload);
        syms.extend([1; 20]);
        let mut m = sdrkit_algo_sim::modulate::Linear::new(
            Shape::Rc { beta: 0.2 },
            16.0,
            10.0 / (1.0 + ppm * 1e-6),
        )
        .unwrap();
        let mut out = Vec::new();
        let c: Vec<Complex32> = syms
            .iter()
            .map(|&s| Complex32::new(s as f32, 0.0))
            .collect();
        m.push(&c, &mut out);
        m.flush(&mut out);
        let mut x = vec![0f32; 300];
        x.extend(out.iter().map(|z| z.re * 600.0));
        x
    }

    #[test]
    fn the_symbol_clock_is_tracked_between_syncs() {
        // 3000 symbols after the sync with the clock 300 ppm fast: the sampling instant moves by 0.9 symbol without the
        // clock loop.
        let pat = SyncPattern::from_hex_dibits(1, "755FD7DF75F7", 4);
        let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
        let payload: Vec<i8> = (0..3000)
            .map(|_| [3, 1, -1, -3][rng.below(4) as usize])
            .collect();
        let x = drifting_signal(&pat, &payload, 300.0);
        let mut layer = SymbolLayer::new(10, false, std::slice::from_ref(&pat));
        let mut c = Collect {
            syncs: vec![],
            syms: vec![],
            stop_after: 3000,
        };
        layer.process(&x, &mut c);
        assert_eq!(c.syncs.len(), 1);
        let wrong = c.syms[2900..]
            .iter()
            .zip(&payload[2900..])
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(wrong, 0, "{wrong} of the last 100 symbols are wrong");
    }
}
