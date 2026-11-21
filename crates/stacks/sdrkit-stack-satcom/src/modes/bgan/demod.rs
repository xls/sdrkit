//! The demodulator of one BGAN forward carrier: channel filter to 4 samples per symbol, root raised cosine matched filter, the
//! search of the pilot comb and the unique word, frame tracking (timing, frequency, channel and noise from the unique word and
//! the pilots), soft demapping, turbo decoding of the FEC blocks and the reading of their broadcast PDUs.

use super::codec::{BlockDecoder, IqLabels};
use super::pdu::{self, Avp, BlockContent, CodeRate, Sdu};
use super::tables::{self, Bearer, BEARERS, SUBTYPES, UW_LEN};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::pulse::{self, Norm, Shape};
use sdrkit_core::Complex32;
use std::collections::HashMap;
use std::f64::consts::TAU;

/// Samples per symbol after the channel filter.
pub const SPS: f64 = 4.0;
/// The least comb metric (|sum of pilot pair products| over pilot energy) of a frame start.
pub const COMB_MIN: f64 = 0.35;
/// The least unique-word metric (|sum y s|^2 / sum |y|^2, 40 for a clean word) of a frame start.
pub const UW_MIN: f64 = 16.0;
/// The unique-word metric under which a tracked frame counts as missed.
pub const UW_LOST: f64 = 8.0;
/// Frames missed in a row before the lock is dropped.
pub const MISSES_TO_LOSE: u32 = 3;
/// Turbo iterations at most.
pub const ITERATIONS: u32 = 8;
/// Matched filter span, symbols.
const MF_SPAN: f64 = 12.0;

/// What the demodulator is doing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum DemodState {
    /// Looking for the pilot comb.
    #[default]
    Searching,
    /// Frames in sync.
    Sync,
    /// Frames in sync and broadcast signalling decoded.
    Decoded,
}

/// The options of a demodulator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DemodOptions {
    /// Decode every FEC block (else block 0 only).
    pub decode_all: bool,
    /// The first symbol of a unique word is the most significant bit of its hex.
    pub uw_msb_first: bool,
    /// The labelling of the I and Q columns.
    pub labels: IqLabels,
    /// Only this bearer (`None` for any).
    pub forced: Option<&'static Bearer>,
}

impl Default for DemodOptions {
    fn default() -> Self {
        DemodOptions {
            decode_all: true,
            uw_msb_first: true,
            labels: IqLabels::InOrder,
            forced: None,
        }
    }
}

/// What one frame gave.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameResult {
    /// The bearer (the most likely one until a block decodes).
    pub bearer: &'static str,
    /// The subtype of block 0 (from the unique word).
    pub subtype: &'static str,
    /// No published table decodes this bearer: frame structure only.
    pub structure_only: bool,
    /// Blocks decoded.
    pub blocks_ok: u32,
    /// Blocks that failed.
    pub blocks_bad: u32,
    /// Blocks that carried PDUs.
    pub blocks_used: u32,
    /// The contents of the decoded blocks.
    pub contents: Vec<BlockContent>,
    /// Es/N0, dB.
    pub esn0_db: f64,
    /// The unique-word metric.
    pub uw_metric: f64,
}

/// The tracking of a frame-synchronous carrier.
#[derive(Clone, Debug)]
struct Lock {
    /// The bearers the frame structure and polarity allow, most likely first.
    candidates: Vec<&'static Bearer>,
    /// The bearer whose tables decoded.
    chosen: Option<&'static Bearer>,
    /// Sample index (filter output) of symbol 0 of the next frame.
    t0: f64,
    /// Samples per symbol.
    sps: f64,
    /// Frequency left after the channel filter, cycles per symbol.
    freq: f64,
    misses: u32,
    /// The ForwardBearerCodeRate entries that apply to the next frame.
    next_rates: Vec<CodeRate>,
    esn0_db: f64,
}

/// The demodulator of one carrier.
pub struct Demod {
    /// Symbols per second.
    pub rate: f64,
    /// The carrier, Hz from the centre of the input.
    pub offset_hz: f64,
    opts: DemodOptions,
    chan: Channelizer,
    mf: Fir<Complex32>,
    mixed: Vec<Complex32>,
    filtered: Vec<Complex32>,
    buf: Vec<Complex32>,
    /// The sample index of `buf[0]`.
    base: u64,
    /// Bearers at this rate.
    bearers: Vec<&'static Bearer>,
    lock: Option<Lock>,
    decoders: HashMap<usize, BlockDecoder>,
    /// Searches that found nothing in a row.
    pub failed_searches: u32,
    /// Samples to skip before the next search.
    hold: usize,
    /// Frames in sync so far.
    pub frames: u64,
    /// The last state.
    pub state: DemodState,
    /// The frequency left after the channel filter, Hz.
    pub cfo_hz: f64,
    /// Es/N0 of the last frame, dB.
    pub esn0_db: f64,
    /// The bearer and subtype of the last frame.
    pub bearer: Option<&'static str>,
    /// The subtype of the last frame.
    pub subtype: &'static str,
    /// No table decodes the bearer.
    pub structure_only: bool,
    zs: Vec<Complex32>,
    ws: Vec<f32>,
    y: Vec<Complex32>,
}

impl Demod {
    /// A demodulator for a carrier of `rate` symbols per second at `offset_hz` from the centre of an input at `input_rate`.
    /// `None` when no bearer has that rate or the channel filter cannot be built.
    pub fn new(input_rate: f64, offset_hz: f64, rate: f64, opts: DemodOptions) -> Option<Self> {
        let bearers: Vec<&'static Bearer> = BEARERS
            .iter()
            .filter(|b| (b.rate - rate).abs() < 1.0 && opts.forced.is_none_or(|f| f.name == b.name))
            .collect();
        let first = *bearers.first()?;
        let mut cfg = ChannelConfig::new(
            input_rate,
            offset_hz,
            (1.0 + first.rolloff) * rate + 4e3,
            SPS * rate,
        );
        cfg.exact_rate = true;
        cfg.atten_db = 60.0;
        let chan = Channelizer::with_stop(cfg.clone(), 0.5 * SPS * rate)
            .or_else(|_| Channelizer::new(cfg))
            .ok()?;
        let taps = pulse::taps(
            Shape::Rrc {
                beta: first.rolloff,
            },
            SPS,
            MF_SPAN,
            Norm::Energy,
        )
        .ok()?;
        Some(Demod {
            rate,
            offset_hz,
            opts,
            chan,
            mf: Fir::new(&taps),
            mixed: Vec::new(),
            filtered: Vec::new(),
            buf: Vec::new(),
            base: 0,
            bearers,
            lock: None,
            decoders: HashMap::new(),
            failed_searches: 0,
            hold: 0,
            frames: 0,
            state: DemodState::Searching,
            cfo_hz: 0.0,
            esn0_db: 0.0,
            bearer: None,
            subtype: "",
            structure_only: false,
            zs: Vec::new(),
            ws: Vec::new(),
            y: Vec::new(),
        })
    }

    /// Symbols of a frame at this rate.
    fn mos(&self) -> usize {
        self.bearers[0].mos
    }

    /// The carrier is in frame sync.
    pub fn locked(&self) -> bool {
        self.lock.is_some()
    }

    /// The samples after the matched filter that the demodulator holds (for the carrier tests of the band).
    pub fn recent(&self) -> &[Complex32] {
        &self.buf
    }

    /// Takes input samples and returns the frames completed.
    pub fn process(&mut self, x: &[Complex32]) -> Vec<FrameResult> {
        self.mixed.clear();
        self.chan.process(x, &mut self.mixed);
        self.filtered.resize(self.mixed.len(), Complex32::default());
        self.mf.process(&self.mixed, &mut self.filtered);
        self.buf.extend_from_slice(&self.filtered);
        let mut out = Vec::new();
        let frame_len = (self.mos() as f64 * SPS) as usize;
        loop {
            if self.lock.is_none() {
                if self.hold > 0 {
                    let d = self.hold.min(self.buf.len());
                    self.drop_front(d);
                    self.hold -= d;
                    if self.hold > 0 {
                        break;
                    }
                }
                if self.buf.len() < 3 * frame_len + 64 {
                    break;
                }
                if !self.search() {
                    self.failed_searches += 1;
                    self.state = DemodState::Searching;
                    // Slide by two frames; after many failures look less often.
                    let extra = if self.failed_searches > 6 { 8 } else { 0 };
                    self.hold = (2 + extra) * frame_len;
                    continue;
                }
                self.failed_searches = 0;
            }
            let Some(l) = self.lock.as_ref() else { break };
            let end = l.t0 + (self.mos() as f64 + 24.0) * l.sps;
            if end > (self.base + self.buf.len() as u64) as f64 {
                break;
            }
            if let Some(r) = self.frame() {
                out.push(r);
            }
            if let Some(l) = self.lock.as_ref() {
                let keep = (l.t0 - 32.0 * SPS).max(self.base as f64) as u64;
                self.drop_front((keep - self.base) as usize);
            }
        }
        // Keep the buffer bounded while searching.
        if self.lock.is_none() && self.buf.len() > 6 * frame_len {
            let d = self.buf.len() - 4 * frame_len;
            self.drop_front(d);
        }
        out
    }

    fn drop_front(&mut self, n: usize) {
        let n = n.min(self.buf.len());
        self.buf.drain(..n);
        self.base += n as u64;
    }

    /// The filter output at sample index `t` (absolute), by 4-point Lagrange interpolation.
    fn at(&self, t: f64) -> Complex32 {
        let r = t - self.base as f64;
        let i = r.floor() as isize;
        let mu = (r - i as f64) as f32;
        let n = self.buf.len() as isize;
        if i < 1 || i + 2 >= n {
            return Complex32::default();
        }
        let i = i as usize;
        let (xm, x0, x1, x2) = (self.buf[i - 1], self.buf[i], self.buf[i + 1], self.buf[i + 2]);
        let cm = -mu * (mu - 1.0) * (mu - 2.0) / 6.0;
        let c0 = (mu + 1.0) * (mu - 1.0) * (mu - 2.0) / 2.0;
        let c1 = -(mu + 1.0) * mu * (mu - 2.0) / 2.0;
        let c2 = (mu + 1.0) * mu * (mu - 1.0) / 6.0;
        xm * cm + x0 * c0 + x1 * c1 + x2 * c2
    }

    /// The comb metric at buffer index `s` of the structure (`ds`, `ps`) over two frames: the summed pilot-pair products and
    /// the pilot energy.
    fn comb(&self, s: usize, ds: usize, ps: usize, frame: usize) -> (Complex32, f32) {
        let step = (ds + 1) * SPS as usize;
        let mut d = Complex32::default();
        let mut e = 0.0f32;
        for f in 0..2 {
            let o = s + f * frame;
            let mut prev = self.buf[o];
            e += prev.norm_sqr();
            for k in 1..ps {
                let x = self.buf[o + k * step];
                d += prev * x.conj();
                e += x.norm_sqr();
                prev = x;
            }
        }
        (d, e)
    }

    /// Looks for the pilot comb and the unique word in the first three frames of the buffer. Sets the lock on success.
    fn search(&mut self) -> bool {
        let mos = self.mos();
        let frame = (mos as f64 * SPS) as usize;
        let mut structures: Vec<(usize, usize)> = self.bearers.iter().map(|b| (b.ds, b.ps)).collect();
        structures.dedup();
        let mut best: Option<(f64, usize, usize, usize, Complex32)> = None;
        for &(ds, ps) in &structures {
            let span = (ps - 1) * (ds + 1) * SPS as usize;
            if frame + span + frame >= self.buf.len() {
                continue;
            }
            let metric = |s: usize| -> (f64, Complex32) {
                let (d, e) = self.comb(s, ds, ps, frame);
                (f64::from(d.norm()) / f64::from(e.max(1e-30)), d)
            };
            let mut top = (0.0, 0usize);
            for s in (0..frame).step_by(2) {
                let (m, _) = metric(s);
                if m > top.0 {
                    top = (m, s);
                }
            }
            let lo = top.1.saturating_sub(2);
            for s in lo..(top.1 + 3).min(frame) {
                let (m, d) = metric(s);
                if best.is_none_or(|b| m > b.0) {
                    best = Some((m, s, ds, ps, d));
                }
            }
        }
        let Some((m, s, ds, ps, d)) = best else {
            return false;
        };
        if m < COMB_MIN {
            return false;
        }
        // Fractional timing from the metric around the peak.
        let mm = |s: usize| {
            let (d, e) = self.comb(s, ds, ps, frame);
            f64::from(d.norm()) / f64::from(e.max(1e-30))
        };
        let (a, b, c) = (
            mm(s.saturating_sub(1)),
            m,
            mm((s + 1).min(self.buf.len() - 1)),
        );
        let den = a - 2.0 * b + c;
        let frac = if den.abs() > 1e-12 {
            (0.5 * (a - c) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let pilot0 = self.base as f64 + s as f64 + frac;
        let f0 = -f64::from(d.arg()) / (TAU * (ds + 1) as f64);
        // The unique word at the frame starts the comb allows, over frequency aliases.
        let mut found: Option<(f64, f64, f64, usize, bool)> = None;
        for j in -1i32..=1 {
            let mut t = pilot0 - ((UW_LEN + ds) as f64 - f64::from(j) * (ds + 1) as f64) * SPS;
            if t < self.base as f64 + 4.0 {
                t += frame as f64;
            }
            for a in -1i32..=1 {
                let f = f0 + f64::from(a) / (ds + 1) as f64;
                let (metric, word, upper_zero) = self.unique_word(t, SPS, f, ds, ps);
                let penalty = if a == 0 { 1.0 } else { 0.8 };
                if found.is_none_or(|x| metric * penalty > x.0) {
                    found = Some((metric * penalty, t, f, word, upper_zero));
                }
            }
        }
        let Some((metric, t0, f, word, zero_upper)) = found else {
            return false;
        };
        if metric < UW_MIN {
            return false;
        }
        let mut candidates: Vec<&'static Bearer> = self
            .bearers
            .iter()
            .copied()
            .filter(|b| b.ds == ds && b.ps == ps && b.zero_upper == zero_upper)
            .collect();
        if candidates.is_empty() {
            return false;
        }
        // A bearer with dynamic modulation starts its frames with QPSK subtypes L8 to L3: try it first for those words.
        candidates.sort_by_key(|b| {
            let qpsk_first = b.dynamic() && word <= 5;
            (!qpsk_first, b.family.is_none())
        });
        self.lock = Some(Lock {
            candidates,
            chosen: None,
            t0,
            sps: SPS,
            freq: f,
            misses: 0,
            next_rates: Vec::new(),
            esn0_db: 10.0,
        });
        true
    }

    /// The unique-word metric at frame start `t` with `sps` and frequency `f` (cycles per symbol): (the best metric, its word,
    /// bit 0 is the upper-right point).
    fn unique_word(&self, t: f64, sps: f64, f: f64, ds: usize, ps: usize) -> (f64, usize, bool) {
        let rot = |n: f64| {
            let p = -TAU * f * n;
            Complex32::new(p.cos() as f32, p.sin() as f32)
        };
        let y: Vec<Complex32> = (0..UW_LEN)
            .map(|i| self.at(t + i as f64 * sps) * rot(i as f64))
            .collect();
        let energy: f64 = y.iter().map(|v| f64::from(v.norm_sqr())).sum();
        let mut pilot = Complex32::default();
        for k in 0..ps {
            let n = (UW_LEN + k * (ds + 1) + ds) as f64;
            pilot += self.at(t + n * sps) * rot(n);
        }
        let mut best = (0.0, 0usize, Complex32::default());
        for w in 0..SUBTYPES.len() {
            let c: Complex32 = y
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    if tables::uw_bit(w, i, self.opts.uw_msb_first) == 1 {
                        *v
                    } else {
                        -*v
                    }
                })
                .sum();
            let m = f64::from(c.norm_sqr()) / energy.max(1e-30);
            if m > best.0 {
                best = (m, w, c);
            }
        }
        // Bit 1 in the direction of the pilots: the bit 1 point is the upper-right one.
        let one_upper = (best.2 * pilot.conj()).re > 0.0;
        (best.0, best.1, !one_upper)
    }

    /// Demodulates and decodes the frame at the lock's `t0` and moves the lock to the next frame.
    fn frame(&mut self) -> Option<FrameResult> {
        let mut l = self.lock.clone()?;
        let b0 = l.chosen.unwrap_or(l.candidates[0]);
        let (ds, ps, mos) = (b0.ds, b0.ps, b0.mos);
        // Timing: the coherent pilot sums early and late over the frame. The offset moves this frame; what is left of it frame
        // after frame is a clock error, which an integrator takes into the samples per symbol.
        let mut moved = 0.0;
        for _ in 0..2 {
            let e = self.timing_error(&l, b0);
            l.t0 += e * l.sps;
            moved += e;
            if e.abs() < 0.03 {
                break;
            }
        }
        l.sps *= 1.0 + (0.25 * moved / mos as f64).clamp(-1e-4, 1e-4);
        // The symbols of the frame, the frequency left removed.
        let rot = |f: f64, n: usize| {
            let p = -TAU * f * n as f64;
            Complex32::new(p.cos() as f32, p.sin() as f32)
        };
        let mut y = std::mem::take(&mut self.y);
        y.clear();
        y.extend((0..mos).map(|n| self.at(l.t0 + n as f64 * l.sps) * rot(l.freq, n)));
        let pilot = b0.pilot();
        let mut h: Vec<Complex32> = (0..ps).map(|k| y[b0.pilot_position(k)] / pilot).collect();
        // Frequency from the rotation between pilots.
        let rotsum: Complex32 = h.windows(2).map(|w| w[1] * w[0].conj()).sum();
        let df = f64::from(rotsum.arg()) / (TAU * (ds + 1) as f64);
        if df.abs() > 1e-7 {
            l.freq += df;
            for (n, v) in y.iter_mut().enumerate() {
                *v *= rot(df, n);
            }
            for (k, v) in h.iter_mut().enumerate() {
                *v *= rot(df, b0.pilot_position(k));
            }
        }
        // The unique word: the subtype of block 0 and a channel value at its middle.
        let mut best = (0.0f64, 0usize);
        let uw_energy: f64 = y[..UW_LEN].iter().map(|v| f64::from(v.norm_sqr())).sum();
        for w in 0..SUBTYPES.len() {
            let c: Complex32 = (0..UW_LEN)
                .map(|i| y[i] * b0.uw_symbol(tables::uw_bit(w, i, self.opts.uw_msb_first)).conj())
                .sum();
            let m = f64::from(c.norm_sqr()) / f64::from(pilot.norm_sqr()) / uw_energy.max(1e-30);
            if m > best.0 {
                best = (m, w);
            }
        }
        let (uw_metric, word) = best;
        let h_uw: Complex32 = (0..UW_LEN)
            .map(|i| y[i] / b0.uw_symbol(tables::uw_bit(word, i, self.opts.uw_msb_first)))
            .sum::<Complex32>()
            / UW_LEN as f32;
        // Smoothing of the pilot values over a time that depends on Es/N0.
        let window_s = if l.esn0_db < 5.0 {
            0.012
        } else if l.esn0_db < 10.0 {
            0.005
        } else {
            0.002
        };
        let m = ((window_s * self.rate / (ds + 1) as f64 / 2.0).round() as usize).min(ps / 2);
        let hs: Vec<Complex32> = (0..ps)
            .map(|k| {
                let a = k.saturating_sub(m);
                let b = (k + m).min(ps - 1);
                h[a..=b].iter().sum::<Complex32>() / (b - a + 1) as f32
            })
            .collect();
        let mut noise = 0.0f64;
        for k in 0..ps {
            noise += f64::from((h[k] - hs[k]).norm_sqr() * pilot.norm_sqr());
        }
        for i in 0..UW_LEN {
            let s = b0.uw_symbol(tables::uw_bit(word, i, self.opts.uw_msb_first));
            noise += f64::from((y[i] - h_uw * s).norm_sqr());
        }
        let corr = if m > 0 { (2 * m + 1) as f64 / (2 * m) as f64 } else { 1.0 };
        let noise = (noise / (ps + UW_LEN) as f64 * corr).max(1e-12);
        let gain = hs.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / ps as f64;
        let esn0 = 10.0 * (gain / noise).max(1e-6).log10();
        l.esn0_db = esn0;
        if uw_metric < UW_LOST {
            l.misses += 1;
        } else {
            l.misses = 0;
        }
        let total = b0.data_symbols();
        let mut zs = std::mem::take(&mut self.zs);
        let mut ws = std::mem::take(&mut self.ws);
        zs.resize(total, Complex32::default());
        ws.resize(total, 0.0);
        // The channel at a frame position: between the unique word, the pilots and the last pilot.
        let chan_at = |n: usize| -> Complex32 {
            let first = b0.pilot_position(0);
            if n < first {
                let a = (n as f32 - 19.5) / (first as f32 - 19.5);
                return h_uw * (1.0 - a) + hs[0] * a;
            }
            let k = (n - first) / (ds + 1);
            if k + 1 >= ps {
                return hs[ps - 1];
            }
            let a = ((n - first) % (ds + 1)) as f32 / (ds + 1) as f32;
            hs[k] * (1.0 - a) + hs[k + 1] * a
        };
        for d in 0..total {
            let n = b0.data_position(d);
            let hh = chan_at(n);
            let p = hh.norm_sqr().max(1e-20);
            let (blk, s) = b0.block_symbol(d);
            let idx = blk * b0.teo + s;
            zs[idx] = y[n] * hh.conj() / p;
            ws[idx] = (f64::from(p) / noise) as f32;
        }
        self.y = y;
        // Blocks: block 0 first (its signalling may set the others).
        let mut res = FrameResult {
            subtype: SUBTYPES[word],
            uw_metric,
            esn0_db: esn0,
            ..Default::default()
        };
        let candidates: Vec<&'static Bearer> = match l.chosen {
            Some(c) => vec![c],
            None => l.candidates.clone(),
        };
        let mut bearer = candidates[0];
        let mut block0: Option<BlockContent> = None;
        for &c in &candidates {
            let Some(fam) = c.family else { continue };
            if let Some(content) = self.decode_block(&zs[..c.teo], &ws[..c.teo], &fam, word) {
                bearer = c;
                l.chosen = Some(c);
                block0 = Some(content);
                break;
            }
        }
        let decodable = candidates.iter().any(|c| c.family.is_some());
        res.structure_only = l.chosen.is_none() && !decodable;
        if l.chosen.is_none() && candidates.len() > 1 {
            // Nothing decoded: name the bearer without tables when there is one, else the most likely.
            if let Some(c) = candidates.iter().find(|c| c.family.is_none()) {
                bearer = c;
                res.structure_only = true;
            }
        }
        res.bearer = bearer.name;
        // The code rates of the blocks: from this frame's block 0 on bearers that apply them at once, else from the last frame.
        let rates_now = block0.as_ref().map(code_rates).unwrap_or_default();
        let rates = if bearer.fr() {
            std::mem::replace(&mut l.next_rates, rates_now)
        } else {
            rates_now
        };
        match block0 {
            Some(c) => {
                res.blocks_ok += 1;
                if !c.empty {
                    res.blocks_used += 1;
                }
                res.contents.push(c);
            }
            None if bearer.family.is_some() => res.blocks_bad += 1,
            None => {}
        }
        if self.opts.decode_all && l.chosen.is_some() {
            for blk in 1..bearer.blocks {
                let (qam, sub) = match rates.iter().find(|r| usize::from(r.block) == blk) {
                    Some(r) => (
                        r.higher,
                        (tables::R as i32 + i32::from(r.rate)).clamp(0, 14) as usize,
                    ),
                    None => (false, word),
                };
                let Some(fam) = bearer.block_family(qam && bearer.dynamic()) else {
                    continue;
                };
                let range = blk * bearer.teo..(blk + 1) * bearer.teo;
                match self.decode_block(&zs[range.clone()], &ws[range], &fam, sub) {
                    Some(c) => {
                        res.blocks_ok += 1;
                        if !c.empty {
                            res.blocks_used += 1;
                        }
                        res.contents.push(c);
                    }
                    None => res.blocks_bad += 1,
                }
            }
        }
        self.zs = zs;
        self.ws = ws;
        // Next frame.
        l.t0 += mos as f64 * l.sps;
        self.frames += 1;
        self.cfo_hz = l.freq * self.rate;
        self.esn0_db = esn0;
        self.bearer = Some(res.bearer);
        self.subtype = res.subtype;
        self.structure_only = res.structure_only;
        self.state = if res.blocks_ok > 0 {
            DemodState::Decoded
        } else {
            DemodState::Sync
        };
        if l.misses >= MISSES_TO_LOSE {
            self.lock = None;
            self.state = DemodState::Searching;
            return None;
        }
        self.lock = Some(l);
        Some(res)
    }

    /// The timing error of the frame, symbols: the peak of a parabola through the coherent pilot sums (in runs of 8 pilots) a
    /// quarter symbol early, on time and a quarter symbol late.
    fn timing_error(&self, l: &Lock, b: &Bearer) -> f64 {
        let seg = 8usize;
        let c = |delta: f64| -> f64 {
            let mut total = 0.0f64;
            let mut k = 0;
            while k < b.ps {
                let e = (k + seg).min(b.ps);
                let mut s = Complex32::default();
                for kk in k..e {
                    let n = b.pilot_position(kk) as f64;
                    let p = -TAU * l.freq * n;
                    s += self.at(l.t0 + (n + delta) * l.sps) * Complex32::new(p.cos() as f32, p.sin() as f32);
                }
                total += f64::from(s.norm_sqr());
                k = e;
            }
            total
        };
        let (a, m, z) = (c(-0.25), c(0.0), c(0.25));
        let den = a - 2.0 * m + z;
        if den >= -1e-30 {
            return 0.0;
        }
        (0.25 * 0.5 * (a - z) / den).clamp(-0.25, 0.25)
    }

    /// Decodes one block of equalised symbols with the table of `family` and subtype `sub`; the block's content when it decodes.
    fn decode_block(
        &mut self,
        z: &[Complex32],
        w: &[f32],
        family: &tables::Family,
        sub: usize,
    ) -> Option<BlockContent> {
        let index = tables::table_index(family, sub)?;
        let labels = self.opts.labels;
        let dec = match self.decoders.entry(index) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => e.insert(BlockDecoder::new(index, labels)?),
        };
        if dec.table().symbols != z.len() {
            return None;
        }
        for (s, (&zz, &ww)) in z.iter().zip(w).enumerate() {
            dec.put(s, zz, ww);
        }
        let octets = dec.decode(ITERATIONS, pdu::block_check)?;
        Some(pdu::read_block(octets))
    }
}

/// The ForwardBearerCodeRate entries of a block's broadcast SDUs.
pub fn code_rates(c: &BlockContent) -> Vec<CodeRate> {
    let mut out = Vec::new();
    for s in &c.sdus {
        let avps = match s {
            Sdu::Bulletin(b) => &b.avps,
            Sdu::AvpList(a) => a,
            _ => continue,
        };
        for a in avps {
            if let Avp::CodeRates(list) = a {
                out.extend_from_slice(list);
            }
        }
    }
    out
}
