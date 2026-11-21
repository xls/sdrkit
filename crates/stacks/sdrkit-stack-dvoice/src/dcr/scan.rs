//! The scan of the plan channels in one capture. The shared carrier finder marks the plan channels that carry a signal, and
//! each such channel gets a decoder of its own (the narrowband chain, the symbol layer and a [`Link`]) while it decodes. A
//! new decoder first takes the last 0.6 s of the capture, so the start of the transmission that woke it is not lost.

use super::link::Link;
use super::{chain_spec, nearest, patterns, PlanChannel, Shared};
use crate::decoder::Ctx;
use crate::front::{Chain, ChainSpec, RATE};
use crate::symbols::{Sym, SymAnswer, SymbolLayer, SymbolSink, SyncAnswer};
use sdrkit_core::Complex32;
use sdrkit_graph::{Format, Timed};
use sdrkit_proto_channels::{Carriers, Finder, FinderConfig};

/// The part of the rate on each side of the centre that the scan watches.
pub const SPAN: f64 = 0.42;
/// The least sample rate of a scan, Hz.
pub const MIN_RATE_HZ: f64 = 100_000.0;
/// The raster of the finder, Hz: every channel of the plan data lies on it.
const STEP_HZ: f64 = 3_125.0;
/// A decoder without a good frame for this long, started this long ago and without a call, is removed, s.
const REMOVE_S: f64 = 8.0;
/// A channel gets a new decoder at most once in this time, s.
const RESTART_S: f64 = 20.0;
/// A decoder counts as decoding while its last good frame is younger than this, s.
const DECODING_S: f64 = 6.0;
/// The capture kept for a new decoder, s.
const HISTORY_S: f64 = 0.6;
/// The piece a new decoder takes the kept capture in, samples.
const PIECE: usize = 8192;

/// The decoder of one scanned channel.
struct Unit {
    plan: usize,
    hz: f64,
    link: Link,
    chain: Chain,
    layer: SymbolLayer,
    /// Signal time of the first sample the decoder took, s.
    start_s: f64,
    /// Signal time when the decoder was made, s.
    created_s: f64,
    fine_acc: f32,
    front_offset: f32,
}

/// The symbol layer's calls into a link, at the signal time of the decoder.
struct Sink<'a> {
    link: &'a mut Link,
    sh: &'a mut Shared,
    ctx: &'a mut Ctx,
    start_s: f64,
}

impl SymbolSink for Sink<'_> {
    fn sync(&mut self, _id: u8, _inverted: bool, pre: &[Sym], at: u64, tracking: bool) -> SyncAnswer {
        self.ctx.now = self.start_s + at as f64 / RATE;
        self.link.sync(pre, tracking)
    }
    fn symbol(&mut self, s: Sym, at: u64) -> SymAnswer {
        self.ctx.now = self.start_s + at as f64 / RATE;
        self.link.symbol(s, self.sh, self.ctx)
    }
}

impl Unit {
    /// Runs the samples `x` of the capture through the decoder, as the family decoder runs its own channel.
    fn run(&mut self, x: &[Complex32], sh: &mut Shared, ctx: &mut Ctx) {
        ctx.carrier_hz = self.hz;
        let info = self.chain.process(x);
        let moved = info.offset_hz - self.front_offset;
        if moved != 0.0 {
            self.layer.shift_dc(moved);
        }
        self.front_offset = info.offset_hz;
        let iq = &self.chain.iq;
        if !iq.is_empty() && iq.iter().map(|s| s.norm_sqr()).sum::<f32>() / (iq.len() as f32) < 1e-10 {
            self.layer.stop();
        }
        let before = self.link.good_frames;
        let was = self.layer.tracking();
        let mut sink = Sink {
            link: &mut self.link,
            sh,
            ctx,
            start_s: self.start_s,
        };
        self.layer.process(&self.chain.lane1, &mut sink);
        let good = self.link.good_frames > before;
        self.layer.commit(good);
        let locked = self.layer.tracking();
        if locked != was {
            self.chain.front.set_locked(locked);
        }
        if good {
            let dc = self.chain.lane_hz(self.layer.dc_hz());
            if dc.abs() > 15.0 && dc.abs() < 3_000.0 {
                self.fine_acc += 0.3 * dc;
            }
            if self.fine_acc.abs() > 10.0 {
                self.chain.front.nudge_fine(f64::from(self.fine_acc));
                self.fine_acc = 0.0;
            }
        }
    }
}

/// The scan.
#[derive(Default)]
pub struct Scan {
    /// The scan runs: a plan with two channels or more in the span, a known centre and a rate of 100 ksps or more.
    pub active: bool,
    /// The places in the plan of the channels in the span.
    pub in_span: Vec<usize>,
    finder: Option<Finder>,
    found: Vec<Timed<Carriers>>,
    units: Vec<Unit>,
    started: Vec<(usize, f64)>,
    history: Vec<Complex32>,
    hist_pos: usize,
    hist_len: usize,
    rate: f64,
    centre_hz: f64,
    /// Signal time of the capture taken so far, s. It runs on over a retune, as the time of the family decoder does.
    elapsed_s: f64,
}

impl Scan {
    /// Starts again on the capture of format `f` with the channels `plan`; `on` is the setting `scan`.
    pub fn configure(&mut self, f: &Format, plan: &[PlanChannel], on: bool) {
        self.drop_units();
        self.started.clear();
        self.finder = None;
        self.history.clear();
        self.hist_pos = 0;
        self.hist_len = 0;
        self.rate = f.rate;
        self.centre_hz = f.centre_hz;
        self.in_span.clear();
        if f.centre_hz > 0.0 {
            let half = SPAN * f.rate;
            self.in_span
                .extend((0..plan.len()).filter(|&i| (plan[i].hz - f.centre_hz).abs() <= half));
        }
        self.active = on && f.centre_hz > 0.0 && f.rate >= MIN_RATE_HZ && self.in_span.len() >= 2;
        if !self.active {
            return;
        }
        let mut cfg = FinderConfig::new(STEP_HZ, plan[self.in_span[0]].hz);
        cfg.span = SPAN;
        cfg.resolution_hz = f.rate / 4096.0;
        cfg.cell_half_hz = 2_500.0;
        cfg.width_half_hz = 3_125.0;
        cfg.on_db = 10.0;
        cfg.off_db = 7.0;
        cfg.known = self.in_span.iter().map(|&i| plan[i].hz).collect();
        cfg.known_db = 0.0;
        cfg.report_after_s = 0.0;
        cfg.estimate_s = 0.25;
        cfg.keep_s = 1.0;
        cfg.line_share = Some(0.5);
        cfg.max = 64;
        let mut finder = Finder::new(cfg);
        finder.configure(f);
        self.finder = Some(finder);
        self.history = vec![Complex32::default(); (HISTORY_S * f.rate).ceil() as usize];
    }

    /// Removes every decoder.
    pub fn drop_units(&mut self) {
        self.units.clear();
    }

    /// The decoders that run.
    pub fn decoders(&self) -> usize {
        self.units.len()
    }

    /// The decoders whose last good frame is younger than 6 s.
    pub fn decoding(&self) -> usize {
        let now = self.now();
        self.units
            .iter()
            .filter(|u| now - u.link.last_good_s < DECODING_S)
            .count()
    }

    fn now(&self) -> f64 {
        self.elapsed_s
    }

    /// Takes the samples `x` of the capture: runs the decoders, then starts one on each plan channel that the finder marks
    /// active and that has none, at most `max` in all.
    pub fn process(&mut self, x: &[Complex32], plan: &[PlanChannel], sh: &mut Shared, ctx: &mut Ctx, max: usize) {
        if !self.active || x.is_empty() {
            return;
        }
        let saved = (ctx.now, ctx.carrier_hz);
        for u in &mut self.units {
            u.run(x, sh, ctx);
        }
        self.elapsed_s += x.len() as f64 / self.rate;
        self.remember(x);
        let now = self.now();
        let mut found = std::mem::take(&mut self.found);
        found.clear();
        if let Some(f) = self.finder.as_mut() {
            f.process(x, &mut found);
        }
        for est in &found {
            for c in est.value.0.iter().filter(|c| c.active) {
                if let Some(p) = nearest(plan, c.freq_hz) {
                    self.start(p, plan, now, max, sh, ctx);
                }
            }
        }
        self.found = found;
        (ctx.now, ctx.carrier_hz) = saved;
    }

    /// About every 0.25 s: the calls of the scanned channels that lost their signal end, and decoders that found nothing
    /// for 8 s go.
    pub fn tick(&mut self, sh: &mut Shared, ctx: &mut Ctx) {
        if !self.active {
            return;
        }
        let saved = ctx.now;
        let now = self.now();
        ctx.now = now;
        for u in &mut self.units {
            u.link.tick(sh, ctx);
        }
        self.units.retain(|u| {
            u.link.in_call() || now - u.link.last_good_s <= REMOVE_S || now - u.created_s <= REMOVE_S
        });
        ctx.now = saved;
    }

    fn start(&mut self, p: usize, plan: &[PlanChannel], now: f64, max: usize, sh: &mut Shared, ctx: &mut Ctx) {
        if self.units.len() >= max || self.units.iter().any(|u| u.plan == p) {
            return;
        }
        if self.started.iter().any(|&(q, t)| q == p && now - t < RESTART_S) {
            return;
        }
        self.started.retain(|&(q, _)| q != p);
        self.started.push((p, now));
        let mut chain = Chain::new(ChainSpec::Fsk(chain_spec()));
        if chain.configure(self.rate, plan[p].hz - self.centre_hz).is_err() {
            return;
        }
        let mut layer = SymbolLayer::new((RATE / super::gen::BAUD).round() as usize, false, &patterns());
        layer.set_clock_loop(true);
        let kept = self.kept();
        let mut u = Unit {
            plan: p,
            hz: plan[p].hz,
            link: Link::new(1 + p as u32, Some(plan[p].label.clone())),
            chain,
            layer,
            start_s: now - kept.len() as f64 / self.rate,
            created_s: now,
            fine_acc: 0.0,
            front_offset: 0.0,
        };
        for piece in kept.chunks(PIECE) {
            u.run(piece, sh, ctx);
        }
        self.units.push(u);
    }

    fn remember(&mut self, x: &[Complex32]) {
        let n = self.history.len();
        if n == 0 {
            return;
        }
        for &s in x {
            self.history[self.hist_pos] = s;
            self.hist_pos = (self.hist_pos + 1) % n;
        }
        self.hist_len = (self.hist_len + x.len()).min(n);
    }

    /// The kept capture, oldest first.
    fn kept(&self) -> Vec<Complex32> {
        let n = self.history.len();
        if n == 0 {
            return Vec::new();
        }
        let first = (self.hist_pos + n - self.hist_len) % n;
        (0..self.hist_len).map(|i| self.history[(first + i) % n]).collect()
    }
}
