//! `dtmb.search` finds the frame-header grid in the symbols. `dtmb.track` follows it frame by frame: the header fit produces
//! the channel impulse response and C/N, the header's repetition produces the fine carrier offset, and the fractional position
//! of the strongest path drives the timing loop that corrects the front end's clock. The tracker also knows the PN phase of
//! each header, finds the grid again after lost samples, and says which frames the decoder must erase.

use super::estimate::{tracking, Estimator, Fit};
use super::front::FrontSetting;
use super::pn::{Header, HeaderSeq};
use super::{Width, BODY, WIDTH};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::loopfilter::LoopFilter;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    apply_commands, values_of, Applied, Command, Component, ComponentInfo, Format, Io, Item, Kind,
    Port, RateFrom, SlicePool, Timed,
};
use std::collections::VecDeque;
use std::sync::Arc;

/// Search for the header grid again. `dtmb.track` sets it when it loses the grid. The value reads back as off.
pub const SEARCH: Setting<field::Bool> = Setting::new("search", "Search again", "", false);

sdrkit_core::settings! {
    /// The settings of `dtmb.search`.
    pub enum SearchSetting {
        /// The channel width.
        Width = WIDTH,
        /// Search again.
        Search = SEARCH,
    }
}

/// A header grid found by the search.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeaderFound {
    /// The header.
    pub header: Header,
    /// The symbol (counted from the start of the stream) where a frame header starts.
    pub start: u64,
    /// The fine carrier offset left after the front end, Hz.
    pub fine_hz: f64,
}

impl Item for HeaderFound {
    const KIND: &'static str = "dtmb.header_found";
}

/// What the tracker measured, after each frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackState {
    /// The header grid holds.
    pub tracking: bool,
    /// The header ("PN420" ...), when tracking.
    pub header: Option<Header>,
    /// The header's phase changes from frame to frame.
    pub rotates: bool,
    /// Carrier offset left after the front end, Hz.
    pub fine_hz: f64,
    /// C/N from the header fit, dB.
    pub cn_db: f64,
    /// Echo span, seconds.
    pub echo_span_s: f64,
    /// Frames with a usable header.
    pub frames: u64,
    /// Frames erased: the header window was exact silence, so the grid coasted over the frame.
    pub erased: u64,
    /// Share of the last 50 frames without a usable header, %.
    pub no_header_pct: f64,
    /// The clock correction the timing loop requests, ppm.
    pub clock_ppm: f64,
    /// Channel impulse response, dB below the strongest tap, one value per symbol.
    pub cir_db: Arc<[f32]>,
    /// Log lines since the last state.
    pub log: Vec<String>,
}

impl Item for TrackState {
    const KIND: &'static str = "dtmb.track_state";
}

/// One frame on the grid for `dtmb.body`: the symbols of header A, the body and header B (`2 L + 3780` symbols, the carrier
/// offset removed), or an erased frame without symbols.
#[derive(Clone, Debug)]
pub struct FrameSyms {
    /// The grid: a new number each time a grid is found, so everything after the tracker starts afresh.
    pub grid: u32,
    /// The frame's number in the grid (header A is header `index`, header B header `index + 1`).
    pub index: u64,
    /// The header.
    pub header: Header,
    /// The PN phases of header A and header B; `None` while the place in the super-frame is not known.
    pub phases: Option<(usize, usize)>,
    /// Frames lost before this one with the samples of a gap: the decoder erases as many.
    pub lost: u32,
    /// The symbols; empty for a frame that the decoder must erase.
    pub syms: Arc<[Complex32]>,
}

impl Item for FrameSyms {
    const KIND: &'static str = "dtmb.frame_syms";
}

/// The most frames a gap may take with it: a rotating header shows no longer one.
const MAX_GAP_FRAMES: usize = 200;

/// The PN phase of each header of a grid: fixed (phase 0), or rotating with a known place in the super-frame.
#[derive(Clone, Debug, Default)]
pub(crate) struct Phases {
    /// Header number and strongest correlation lag of recent clear headers.
    hist: VecDeque<(u64, usize)>,
    /// Fixed (`Some(true)`), rotating (`Some(false)`) or not known.
    pub fixed: Option<bool>,
    /// A header number and its frame number in the super-frame.
    anchor: (u64, usize),
    /// The place of the tracked path: its correlation lag plus the phase.
    pub place: usize,
    /// The anchor and the place before a gap, kept to count the frames lost and the move of the grid.
    before: Option<((u64, usize), usize)>,
}

/// What [`Phases::identify`] found after a gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Regained {
    /// Frames lost with the gap.
    pub lost: usize,
    /// Symbols by which the tracked path moved (within half a period).
    pub shift: isize,
}

impl Phases {
    /// The phase of header `n`, once known.
    pub fn phase(&self, kind: Header, n: u64) -> Option<usize> {
        match (kind, self.fixed) {
            (Header::Pn595, _) | (_, Some(true)) => Some(0),
            (_, Some(false)) => {
                let f = kind.frames_per_superframe() as u64;
                let (h, s) = self.anchor;
                Some(kind.phase(((s as u64 + n.wrapping_sub(h) % f) % f) as usize))
            }
            _ => None,
        }
    }

    /// Forgets the schedule after a gap of a rotating header and keeps what it was, so that [`Phases::identify`] can count
    /// the frames lost.
    pub fn gap(&mut self) {
        if self.fixed == Some(false) {
            self.before.get_or_insert((self.anchor, self.place));
            self.fixed = None;
        }
        self.hist.clear();
    }

    /// Takes the strongest lag `lag` of the clear header `n` while the schedule is not known. Four headers in a row at one
    /// lag are a fixed phase; four that fit one place of the super-frame give the schedule. After a gap the result says how
    /// many frames were lost and how far the tracked path moved.
    pub fn identify(&mut self, kind: Header, n: u64, lag: usize) -> Option<Regained> {
        let p = kind.period();
        let near = |a: usize, b: usize| {
            let d = (a + p - b) % p;
            d.min(p - d) <= 1
        };
        if self.hist.back().is_some_and(|&(m, _)| m + 1 != n) {
            self.hist.clear();
        }
        self.hist.push_back((n, lag));
        while self.hist.len() > 4 {
            self.hist.pop_front();
        }
        if self.hist.len() < 4 {
            return None;
        }
        let (n0, l0) = self.hist[0];
        if self.before.is_none() && self.hist.iter().all(|&(_, l)| near(l, l0)) {
            self.fixed = Some(true);
            self.place = self.hist[3].1;
            return None;
        }
        let f = kind.frames_per_superframe();
        // After a gap the places are tried from no frame lost upwards, so the least loss that fits counts.
        let (first, tries) = match self.before {
            Some(((h, s), _)) => (
                ((s as u64 + n0.wrapping_sub(h) % f as u64) % f as u64) as usize,
                MAX_GAP_FRAMES + 1,
            ),
            None => (0, f),
        };
        for lost in 0..tries.min(f) {
            let s = (first + lost) % f;
            let place = |&(m, l): &(u64, usize)| (l + kind.phase((s + (m - n0) as usize) % f)) % p;
            let d0 = place(&self.hist[0]);
            // The places of the four headers lie within one symbol of each other.
            let fits = (0..4)
                .all(|a| (a..4).all(|b| near(place(&self.hist[a]), place(&self.hist[b]))))
                && self.hist.iter().all(|h| near(place(h), d0));
            if !fits {
                continue;
            }
            self.fixed = Some(false);
            self.anchor = (n0, s);
            self.place = place(&self.hist[3]);
            let (_, old) = self.before.take()?;
            let mut shift = self.place as isize - old as isize;
            if shift > p as isize / 2 {
                shift -= p as isize;
            } else if shift < -(p as isize) / 2 {
                shift += p as isize;
            }
            return Some(Regained { lost, shift });
        }
        None
    }
}

/// Symbols kept before a grid is found: four of the longest frames and a margin.
const SEARCH_NEED: usize = 4 * (945 + BODY) + 600;

/// Carrier offsets the search resolves around the front end's coarse estimate, Hz: a strong echo pulls the band estimate by
/// tens of kHz.
const SEARCH_SPAN_HZ: f64 = 150e3;

/// Positions each side of the frame-repeat peak at which the PN595 sequence is tried.
const PN595_REFINE: usize = 64;

/// Points of the transform that reads the carrier offset from a PN595 header.
const PN595_FFT: usize = 1024;

/// Searches between two correlations with the PN595 sequence at every position while the frames repeat everywhere.
const PN595_FULL_EVERY: u32 = 8;

/// The share of a header's energy that must correlate with the PN595 sequence.
const PN595_MIN_SHARE: f32 = 0.06;

/// The running sums of a lag correlation and its value per position.
#[derive(Default)]
struct LagFold {
    fold: Vec<(f64, f64, f64)>,
    run: Vec<(f64, f64, f64)>,
    mags: Vec<f32>,
    sorted: Vec<f32>,
}

impl LagFold {
    /// The normalised correlation of `sym` with itself `lag` symbols later, summed over `win` symbols from each of `span`
    /// positions and over `folds` stretches `span` apart. Returns the strongest position, its value (1 for a perfect repeat),
    /// the median over the positions and the phase of the sum at the strongest. A window counts with the mean power of the
    /// stretch at least, so a window that holds only the edge of a repeat between silence does not read as a perfect one.
    /// `sym` holds `folds x span + win + lag` symbols or more.
    fn run(
        &mut self,
        sym: &[Complex32],
        lag: usize,
        win: usize,
        span: usize,
        folds: usize,
    ) -> (usize, f32, f32, f64) {
        self.fold.clear();
        self.fold.resize(span, (0.0, 0.0, 0.0));
        self.run.clear();
        self.run.resize(span + win + 1, (0.0, 0.0, 0.0));
        let mut floor = 0f64;
        for f in 0..folds {
            let s0 = f * span;
            for k in 0..span + win {
                let i = s0 + k;
                let c = sym[i + lag] * sym[i].conj();
                let e = 0.5 * (sym[i].norm_sqr() + sym[i + lag].norm_sqr()) as f64;
                let r = self.run[k];
                self.run[k + 1] = (r.0 + c.re as f64, r.1 + c.im as f64, r.2 + e);
            }
            floor += self.run[span + win].2 * win as f64 / (span + win) as f64;
            for (p, slot) in self.fold.iter_mut().enumerate() {
                let (a, b) = (self.run[p], self.run[p + win]);
                slot.0 += b.0 - a.0;
                slot.1 += b.1 - a.1;
                slot.2 += b.2 - a.2;
            }
        }
        self.mags.clear();
        self.mags.extend(
            self.fold
                .iter()
                .map(|(r, i, e)| ((r * r + i * i).sqrt() / e.max(floor).max(1e-20)) as f32),
        );
        let (pk, v) = self
            .mags
            .iter()
            .copied()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0, 0.0));
        self.sorted.clear();
        self.sorted.extend_from_slice(&self.mags);
        let mid = self.sorted.len() / 2;
        let med = *self.sorted.select_nth_unstable_by(mid, f32::total_cmp).1;
        (pk, v, med, self.fold[pk].1.atan2(self.fold[pk].0))
    }
}

/// The correlation of received symbols with the PN595 header at any carrier offset: the products with the chips go through a
/// transform, whose strongest bin is the offset.
struct Pn595Probe {
    chips: Vec<f32>,
    fft: Fft,
    z: Vec<Complex32>,
}

impl Pn595Probe {
    fn new() -> Pn595Probe {
        let mut chips = vec![0f32; 595];
        HeaderSeq::new(Header::Pn595, true).chips(0, &mut chips);
        Pn595Probe {
            chips,
            fft: Fft::forward(PN595_FFT),
            z: vec![Complex32::new(0.0, 0.0); PN595_FFT],
        }
    }

    /// The header at `y` (595 symbols): the share of its energy that correlates with the sequence at the best carrier
    /// offset, and that offset in cycles per symbol.
    fn at(&mut self, y: &[Complex32]) -> (f32, f64) {
        let mut e = 0f32;
        for ((z, &v), &c) in self.z.iter_mut().zip(y).zip(&self.chips) {
            *z = v * c;
            e += v.norm_sqr();
        }
        self.z[595..].fill(Complex32::new(0.0, 0.0));
        self.fft.process(&mut self.z);
        let n = PN595_FFT;
        let (mut k, mut best) = (0usize, 0f32);
        for (i, v) in self.z.iter().enumerate() {
            let m = v.norm_sqr();
            if m > best {
                best = m;
                k = i;
            }
        }
        let amp = |i: usize| self.z[i % n].norm();
        let (l, c, r) = (amp(k + n - 1), amp(k), amp(k + 1));
        let den = l - 2.0 * c + r;
        let frac = if den < 0.0 {
            (0.5 * (l - r) / den).clamp(-0.5, 0.5) as f64
        } else {
            0.0
        };
        let mut bin = k as f64 + frac;
        if bin > n as f64 / 2.0 {
            bin -= n as f64;
        }
        (best / (595.0 * e).max(1e-20), bin / n as f64)
    }
}

/// `dtmb.search`. Input `in`: symbols. Input `control`: `search` from the tracker when it loses the grid. Output `found`: one
/// [`HeaderFound`] per search that succeeds. The search then rests until told to search again.
pub struct Search {
    width: Width,
    fsym: f64,
    sym: Vec<Complex32>,
    base: u64,
    active: bool,
    lags: LagFold,
    probe: Pn595Probe,
    /// Searches in a row in which the frames repeated everywhere and no PN595 peak stood out.
    flat: u32,
}

impl Default for Search {
    fn default() -> Self {
        Search::new(Width::W8)
    }
}

impl Search {
    /// For a channel width.
    pub fn new(width: Width) -> Search {
        Search {
            width,
            fsym: width.symbol_rate(),
            sym: Vec::new(),
            base: 0,
            active: true,
            lags: LagFold::default(),
            probe: Pn595Probe::new(),
            flat: 0,
        }
    }

    /// Searches the symbols held for a header grid: the header kind, a frame start (index into the symbols) and the carrier
    /// offset left after the front end, Hz.
    ///
    /// PN420 and PN945 repeat their core inside the header, so the product of each symbol with the one a core later adds up
    /// over the cyclic part. PN595 is the same in every frame, so the product with the symbol a frame later adds up over the
    /// header; the sequence itself then gives the exact place and the offset. Both products cost a fixed number of operations
    /// per symbol whatever the carrier offset.
    pub fn find(&mut self, sym: &[Complex32]) -> Option<(Header, usize, f64)> {
        let fsym = self.fsym;
        let mut best: Option<(f32, Header, usize, f64)> = None;
        for h in [Header::Pn420, Header::Pn945] {
            let (core, pre, post) = h.core_pre_post();
            let period = h.len() + BODY;
            let win = pre + post;
            if sym.len() < 3 * period + core + win {
                continue;
            }
            let (pk, v, med, ph) = self.lags.run(sym, core, win, period, 2);
            if v > 0.3 && v > 4.0 * med && best.as_ref().is_none_or(|b| v > b.0) {
                let f = ph / std::f64::consts::TAU * fsym / core as f64;
                best = Some((v, h, pk, f));
            }
        }
        if let Some((_, h, pk, f)) = best {
            return Some((h, pk, resolve_offset(sym, h, pk, f, fsym)));
        }
        let period = 595 + BODY;
        if sym.len() < 3 * period + 595 + PN595_REFINE {
            return None;
        }
        let (pk, v, med, _) = self.lags.run(sym, period, 595, period, 2);
        let from = if v > 0.3 && v > 4.0 * med {
            self.flat = 0;
            pk.saturating_sub(PN595_REFINE)..pk + PN595_REFINE + 1
        } else if med > 0.3 {
            // Every frame equals the next (a payload of stuffing only, or a plain carrier): the repeat marks no place, and the
            // sequence is tried at every position of a frame, on one search in eight.
            self.flat += 1;
            if self.flat % PN595_FULL_EVERY != 1 {
                return None;
            }
            0..period
        } else {
            self.flat = 0;
            return None;
        };
        let (mut at, mut share, mut cycles) = (0usize, 0f32, 0f64);
        for p in from {
            let (s, c) = self.probe.at(&sym[p..p + 595]);
            if s > share {
                (at, share, cycles) = (p, s, c);
            }
        }
        (share > PN595_MIN_SHARE).then(|| (Header::Pn595, at % period, cycles * fsym))
    }
}

/// The carrier offset of a PN420 or PN945 grid at `pk`: the offset `f` from the phase of the header's repeat is known only
/// within one turn per core, so the core of the header is correlated with the sequence at `f` plus each whole number of
/// turns within [`SEARCH_SPAN_HZ`], and the strongest correlation decides.
fn resolve_offset(sym: &[Complex32], h: Header, pk: usize, f: f64, fsym: f64) -> f64 {
    let (core, pre, _) = h.core_pre_post();
    let step = fsym / core as f64;
    let w = -std::f64::consts::TAU * f / fsym;
    let mut y: Vec<Complex32> = sym[pk + pre..pk + pre + core]
        .iter()
        .enumerate()
        .map(|(n, v)| {
            let ph = w * n as f64;
            v * Complex32::new(ph.cos() as f32, ph.sin() as f32)
        })
        .collect();
    Fft::forward(core).process(&mut y);
    let mut c = HeaderSeq::new(h, false).core().to_vec();
    Fft::forward(core).process(&mut c);
    let mut ifft = Fft::inverse(core);
    let mut x = vec![Complex32::new(0.0, 0.0); core];
    let turns = (SEARCH_SPAN_HZ / step).floor() as isize;
    let (mut best, mut at) = (0f32, 0isize);
    for k in -turns..=turns {
        // An offset of k turns per core moves the spectrum of the core by k bins.
        for (i, x) in x.iter_mut().enumerate() {
            *x = y[(i as isize + k).rem_euclid(core as isize) as usize] * c[i].conj();
        }
        ifft.process(&mut x);
        let m = x.iter().map(|v| v.norm_sqr()).fold(0f32, f32::max);
        if m > best {
            (best, at) = (m, k);
        }
    }
    f + at as f64 * step
}

impl Component for Search {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dtmb.search",
            vec![Port::iq("in"), Port::command("control")],
            vec![Port::typed::<HeaderFound>("found").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(SearchSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([
            (SearchSetting::Width, Value::Choice(self.width.index())),
            (SearchSetting::Search, Value::Bool(false)),
        ])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fsym = self.width.symbol_rate();
        self.sym.clear();
        self.base = 0;
        self.active = true;
        self.flat = 0;
        Ok(vec![Format::new(Kind::typed::<HeaderFound>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        // The tracker sends only `search` on, which the schema takes.
        let _ = apply_commands(self, io, 1);
        let x = io.iq(0);
        let first = io.input(0).first();
        if !self.active {
            return;
        }
        if self.sym.is_empty() {
            self.base = first;
        }
        self.sym.extend_from_slice(x);
        while self.sym.len() >= SEARCH_NEED {
            let sym = std::mem::take(&mut self.sym);
            let found = self.find(&sym);
            self.sym = sym;
            match found {
                Some((header, pos, fine_hz)) => {
                    let start = self.base + pos as u64;
                    io.items_out::<HeaderFound>(0).push(Timed {
                        time: start as f64 / self.fsym,
                        value: HeaderFound {
                            header,
                            start,
                            fine_hz,
                        },
                    });
                    self.active = false;
                    self.sym.clear();
                    return;
                }
                None => {
                    // Drop all but about a frame of what was searched and keep looking.
                    let d = 3 * (945 + BODY);
                    self.sym.drain(..d);
                    self.base += d as u64;
                }
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SearchSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SearchSetting::Width => {
                self.width = WIDTH.read(id, v)?;
                Ok(Applied::Reformat)
            }
            SearchSetting::Search => {
                if SEARCH.read(id, v)? {
                    self.active = true;
                    self.sym.clear();
                    self.flat = 0;
                }
                Ok(Applied::Live)
            }
        }
    }
}

/// The system information is decoded. `dtmb.si` sets it; the tracker's lock rules read it.
pub const SI_KNOWN: Setting<field::Bool> =
    Setting::new("si_known", "System information known", "", false);

sdrkit_core::settings! {
    /// The settings of `dtmb.track`.
    pub enum TrackSetting {
        /// The channel width.
        Width = WIDTH,
        /// The system information is decoded.
        SiKnown = SI_KNOWN,
    }
}

/// The timing loop's noise bandwidth, per frame.
const TIMING_BN_T: f64 = 0.02;

/// PN595: taps to each side of the centre of the paths that the timing measure weighs.
const CLUSTER_HALF: f64 = 5.0;

/// Steps of the timing loop after which it counts as settled.
const SETTLE_STEPS: u32 = 50;

/// Silent frames in a row the grid coasts over (about 2 s of PN945); after that the source counts as dead and the lock goes.
pub const MAX_ERASED: usize = 3200;

/// Frames in a row without a usable header (and not silent) after which the lock goes.
const MAX_LOST: usize = 12;

/// Frames a stronger path must last before it becomes the tracked path.
const TAKE_OVER: u32 = 16;

/// Frames erased after a gap at most while the place in the super-frame is looked for: a gap of more frames than a rotating
/// header shows, or headers that never fit, lose the lock.
const MAX_REGAIN_FRAMES: u32 = 32;

/// Frames in a row that must show the same move of the tracked path before the grid follows it.
const JUMP_FRAMES: u32 = 3;

/// Times the tracker swaps I and Q when a clean header gives no system information.
const MAX_SWAPS: u32 = 4;

/// The window is exact silence: a source that fills a lost stretch with zeros gives one constant after the front end, so the
/// largest `|x|^2` is within `1.02 x smallest + 1e-12`.
pub fn is_silent(w: &[Complex32]) -> bool {
    let (lo, hi) = w.iter().fold((f32::MAX, 0f32), |(lo, hi), v| {
        let p = v.norm_sqr();
        (lo.min(p), hi.max(p))
    });
    !w.is_empty() && hi <= 1.02 * lo + 1e-12
}

/// The strongest value of `p` within 3 places of `at` (around the end when `wrap`) and its place.
fn near_peak(p: &[f32], at: usize, wrap: bool) -> (usize, f32) {
    let n = p.len() as isize;
    let (mut best, mut place) = (-1f32, at);
    for d in -3isize..=3 {
        let i = at as isize + d;
        let i = if wrap {
            i.rem_euclid(n)
        } else if i < 0 || i >= n {
            continue;
        } else {
            i
        };
        if p[i as usize] > best {
            (best, place) = (p[i as usize], i as usize);
        }
    }
    (place, best.max(0.0))
}

/// What the header that closes a frame shows.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Seen {
    /// The tracked path is where the grid has it; its lag.
    Held(usize),
    /// A usable header whose PN phase is not known yet; its strongest lag.
    Unplaced(usize),
    /// A usable header without the tracked path: the strongest path lies this many symbols from it.
    Shifted(isize),
    /// No usable header.
    Missing,
}

/// `dtmb.track`. Input `in`: symbols. Input `found`: the grid from `dtmb.search`. Input `control`: `si_known` from
/// `dtmb.si`. Outputs: `front` (the clock correction and the I/Q order for `dtmb.front`), `search` (`search` on when the grid
/// is lost), `state` ([`TrackState`] after each frame), `frames` (the [`FrameSyms`] of each frame for `dtmb.body`).
///
/// Each frame is judged by the header that closes it, and a frame is erased in four cases:
///
/// 1. The header window is exact silence ([`is_silent`]): the grid advances by one frame with the old timing, and the frame
///    does not count towards the limit of missing headers. After [`MAX_ERASED`] such frames in a row the lock goes.
/// 2. The header is not usable and is found nowhere else: the grid coasts. 12 such frames in a row lose the lock.
/// 3. The header is found earlier than the grid has it, because the radio lost samples: the grid moves there. A rotating
///    header then shows its place in the super-frame after four headers, which gives the frames lost with the samples (200
///    at most); the decoder erases those too. When the place is not found within 32 frames the lock goes. A fixed header
///    cannot show lost frames.
/// 4. The header is usable but its tracked path lies 4 symbols or more from its place: the grid follows after three
///    frames that agree, and the frames until then are erased.
///
/// The tracked path is the strongest path at lock. The grid stays with it while it is within 3 dB of the strongest; a
/// stronger path takes over after 16 frames.
///
/// Without system information the lock goes after 1.5 s at a header C/N below 6 dB and after 200 frames below -15 dB. At 6 dB
/// or more the tracker swaps I and Q after 0.4 s and searches again, at most four times; after that the lock stays.
pub struct Track {
    width: Width,
    fsym: f64,
    sym: Vec<Complex32>,
    base: u64,
    hdr: Option<HeaderSeq>,
    next: f64,
    lost_frames: usize,
    erased_run: usize,
    recent: VecDeque<bool>,
    rot_pos: usize,
    rot_phase: f64,
    last_lag: Option<usize>,
    rotating_votes: i32,
    fft: Fft,
    ifft: Fft,
    timing: LoopFilter,
    t_prev: Option<f64>,
    /// PN595: the centre of the group of paths at the last header, as a tap index of the fit.
    cluster: Option<f64>,
    t_acc: f64,
    t_ref: f64,
    d_prev: f64,
    steps: u32,
    since_step: usize,
    applied_ppm: f64,
    state: TrackState,
    core_f: Vec<Complex32>,
    fit_r: Vec<Complex32>,
    fit_h: Vec<Complex32>,
    fit_p: Vec<f32>,
    fit_sorted: Vec<f32>,
    /// The noise power of one tap of the last fit.
    fit_noise: f32,
    /// The delay the phase slope of the last fit gives, symbols, and whether the C/N lets it count.
    fit_slope: Option<f64>,
    cir_pool: SlicePool<f32>,
    grid: u32,
    index: u64,
    frame_pool: SlicePool<Complex32>,
    no_syms: Arc<[Complex32]>,
    /// The least-squares fit of PN595, which is not periodic.
    ls: Option<Estimator>,
    /// PN595: the offset the two halves of the last header give (Hz), and the tracked tap of the last usable header with
    /// the frames since.
    ls_half: f64,
    ls_prev: Option<(Complex32, usize)>,
    ls_fit: Fit,
    chips: Vec<f32>,
    phases: Phases,
    /// PN595: the tracked path has a place.
    placed: bool,
    /// A path stronger than the tracked path: its place and the frames it has lasted.
    challenger: (usize, u32),
    /// A move of the tracked path not yet followed: the symbols and the frames that agree.
    jump: (isize, u32),
    /// Frames lost with a gap, for the next frame that is not erased.
    lost_pending: u32,
    /// The symbols the grid moved by and the frames erased since a gap began, for the log line.
    gap: Option<(isize, u32)>,
    lags: LagFold,
    si_known: bool,
    no_si: u32,
    swaps: u32,
    swapped: bool,
}

impl Default for Track {
    fn default() -> Self {
        Track::new(Width::W8)
    }
}

impl Track {
    /// For a channel width.
    pub fn new(width: Width) -> Track {
        Track {
            width,
            fsym: width.symbol_rate(),
            sym: Vec::new(),
            base: 0,
            hdr: None,
            next: 0.0,
            lost_frames: 0,
            erased_run: 0,
            recent: Default::default(),
            rot_pos: 0,
            rot_phase: 0.0,
            last_lag: None,
            rotating_votes: 0,
            fft: Fft::forward(255),
            ifft: Fft::inverse(255),
            timing: Self::timing_loop(),
            t_prev: None,
            cluster: None,
            t_acc: 0.0,
            t_ref: 0.0,
            d_prev: 0.0,
            steps: 0,
            since_step: 0,
            applied_ppm: 0.0,
            state: TrackState::default(),
            core_f: Vec::new(),
            fit_r: Vec::new(),
            fit_h: Vec::new(),
            fit_p: Vec::new(),
            fit_sorted: Vec::new(),
            fit_noise: 0.0,
            fit_slope: None,
            cir_pool: SlicePool::default(),
            grid: 0,
            index: 0,
            frame_pool: SlicePool::default(),
            no_syms: Arc::from(Vec::new()),
            ls: None,
            ls_half: 0.0,
            ls_prev: None,
            ls_fit: Fit::default(),
            chips: Vec::new(),
            phases: Phases::default(),
            placed: false,
            challenger: (0, 0),
            jump: (0, 0),
            lost_pending: 0,
            gap: None,
            lags: LagFold::default(),
            si_known: false,
            no_si: 0,
            swaps: 0,
            swapped: false,
        }
    }

    /// Follows the grid of header `kind`: the transforms and the spectrum of its core.
    fn set_header(&mut self, kind: Header) {
        let seq = HeaderSeq::new(kind, false);
        let n = seq.core().len();
        if self.fft.len() != n {
            self.fft = Fft::forward(n);
            self.ifft = Fft::inverse(n);
        }
        self.core_f.clear();
        self.core_f.extend_from_slice(seq.core());
        self.fft.process(&mut self.core_f);
        self.hdr = Some(seq);
        self.ls = (kind == Header::Pn595).then(|| Estimator::new(kind));
        self.ls_prev = None;
        self.chips.resize(kind.len(), 0.0);
        seq_chips(kind, &mut self.chips);
        self.grid = self.grid.wrapping_add(1);
        self.index = 0;
        self.phases = Phases::default();
        self.placed = false;
        self.challenger = (0, 0);
        self.jump = (0, 0);
        self.lost_pending = 0;
        self.gap = None;
        self.si_known = false;
        self.no_si = 0;
    }

    /// Hands the frame whose header A starts at `at` to `dtmb.body`, with its symbols or, with `erased` or while the PN phase
    /// is not known, without. Returns the frames lost before it that a frame with symbols reports, `None` for an erased one.
    fn send_frame(
        &mut self,
        io: &mut Io,
        at: usize,
        h: Header,
        time: f64,
        erased: bool,
    ) -> Option<u32> {
        let phases = self
            .phases
            .phase(h, self.index)
            .zip(self.phases.phase(h, self.index + 1));
        let erased = erased || phases.is_none();
        let syms = if erased {
            self.no_syms.clone()
        } else {
            let n = 2 * h.len() + BODY;
            let src = &self.sym[at..at + n];
            self.frame_pool.exact(n, |d| d.copy_from_slice(src))
        };
        if let Some(g) = self.gap.as_mut() {
            g.1 += u32::from(erased);
        }
        let lost = (!erased).then(|| std::mem::take(&mut self.lost_pending));
        io.items_out::<FrameSyms>(3).push(Timed {
            time,
            value: FrameSyms {
                grid: self.grid,
                index: self.index,
                header: h,
                phases,
                lost: lost.unwrap_or(0),
                syms,
            },
        });
        self.index += 1;
        lost
    }

    fn timing_loop() -> LoopFilter {
        LoopFilter::new(TIMING_BN_T, std::f64::consts::FRAC_1_SQRT_2)
            .unwrap_or_else(|_| LoopFilter::from_gains(0.05, 0.002))
    }

    fn restart(&mut self) {
        self.hdr = None;
        self.lost_frames = 0;
        self.erased_run = 0;
        self.recent.clear();
        self.last_lag = None;
        self.rotating_votes = 0;
        self.timing = Self::timing_loop();
        self.t_prev = None;
        self.cluster = None;
        self.t_acc = 0.0;
        self.steps = 0;
        self.since_step = 0;
        let clock = self.state.clock_ppm;
        let log = std::mem::take(&mut self.state.log);
        self.state = TrackState {
            clock_ppm: clock,
            log,
            ..Default::default()
        };
    }

    /// The autocorrelation at `lag` over `win` symbols starting at `at`.
    #[inline]
    fn lag_corr(&self, at: usize, lag: usize, win: usize) -> Complex32 {
        let mut c = Complex32::new(0.0, 0.0);
        for i in at..at + win {
            c += self.sym[i + lag] * self.sym[i].conj();
        }
        c
    }

    /// Fits the header that starts at `at`: for PN420 and PN945 the circular correlation of the cyclic part with the known
    /// sequence, for PN595 the least-squares fit. Returns (C/N dB, strongest lag, usable). The impulse response power is
    /// left in `fit_p`, the noise power of one tap in `fit_noise` and the delay from the phase slope in `fit_slope`.
    fn fit(&mut self, at: usize, h: Header) -> (f64, usize, bool) {
        if let Some(ls) = self.ls.as_mut() {
            let (pre, post) = tracking(h);
            ls.fit(&self.sym[at..at + h.len()], 0, pre, post, &mut self.ls_fit);
            let f = &self.ls_fit;
            let (lag, _) = f.strongest();
            let j = (lag + pre as isize) as usize;
            self.fit_p.clear();
            self.fit_p.extend(f.taps.iter().map(|t| t.norm_sqr()));
            self.fit_noise = f.var;
            self.fit_slope = None;
            let cn = 10.0
                * (f.energy() as f64 / f.noise.max(1e-20) as f64)
                    .max(1e-30)
                    .log10();
            return (cn, j, f.usable);
        }
        let (core, pre, post) = h.core_pre_post();
        let n = core;
        let r = &mut self.fit_r;
        r.clear();
        r.extend_from_slice(&self.sym[at + pre..at + pre + n]);
        self.fft.process(r);
        let pw = h.power_ratio() as f64;
        let hf = &mut self.fit_h;
        hf.clear();
        hf.extend(r.iter().zip(&self.core_f).map(|(a, b)| a * b.conj()));
        // The delay from the phase step between neighbouring frequencies: a delay of tau symbols turns bin k by -2 pi k tau / n.
        // Only frequencies within 0.4 of the symbol rate count: nearer the band edge the sampled pulse aliases and bends the phase.
        let edge = (0.4 * n as f64) as isize;
        let mut slope = Complex32::new(0.0, 0.0);
        for k in -edge..edge {
            let a = k.rem_euclid(n as isize) as usize;
            let b = (k + 1).rem_euclid(n as isize) as usize;
            slope += hf[b] * hf[a].conj();
        }
        let delay = -(slope.arg() as f64) * n as f64 / std::f64::consts::TAU;
        self.ifft.process(hf);
        let scale = 1.0 / (n as f64 * n as f64 * pw);
        let p = &mut self.fit_p;
        p.clear();
        p.extend(
            hf.iter()
                .map(|v| (v.norm_sqr() as f64 * scale * scale) as f32),
        );
        let (lag, &pmax) = p
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap_or((0, &0.0));
        // Noise: the median tap power (taps of a sparse channel are few. Noise taps are exponential, median = ln 2 x mean).
        let sorted = &mut self.fit_sorted;
        sorted.clear();
        sorted.extend_from_slice(p);
        sorted.sort_unstable_by(f32::total_cmp);
        let tap_noise = (sorted[n / 2] as f64 / std::f64::consts::LN_2).max(1e-30);
        self.fit_noise = tap_noise as f32;
        let usable = pmax as f64 > 25.0 * tap_noise;
        // The cyclic extensions repeat the core whatever the channel: their difference is noise only.
        let (mut d2, mut p2) = (0f64, 0f64);
        // A margin at each end, for the frame position's last fraction of a symbol and short echoes.
        let (m0, m1) = (at + 12, at + pre + post - 6);
        for i in m0..m1 {
            let a = self.sym[i];
            let b = self.sym[i + n];
            d2 += (a - b).norm_sqr() as f64;
            p2 += 0.5 * (a.norm_sqr() + b.norm_sqr()) as f64;
        }
        let m = (m1 - m0) as f64;
        let noise = (d2 / m / 2.0).max(1e-30);
        let hdr = (p2 / m - noise).max(1e-30);
        let cn = 10.0 * (hdr / pw / noise).log10();
        // The phase slope is exact at a good C/N; below 15 dB its noise reaches symbols.
        let clear = if self.state.frames == 0 {
            cn
        } else {
            self.state.cn_db
        } > 15.0;
        self.fit_slope = clear.then_some(delay);
        (cn, lag, usable)
    }

    /// The delay of the path at `lag` of the last fit, symbols: the phase slope of the channel when it counts and agrees
    /// with the path within a quarter of a symbol, else the parabola through the amplitudes of the lag and its neighbours (a
    /// peak far above the noise).
    fn delay_at(&self, lag: usize, wrap: bool) -> f64 {
        let p = &self.fit_p;
        let n = p.len() as isize;
        let amp = |i: isize| {
            if wrap {
                p[i.rem_euclid(n) as usize].sqrt()
            } else if i < 0 || i >= n {
                0.0
            } else {
                p[i as usize].sqrt()
            }
        };
        let (l, c, r) = (
            amp(lag as isize - 1),
            amp(lag as isize),
            amp(lag as isize + 1),
        );
        let den = l - 2.0 * c + r;
        let parabola = if den < 0.0 {
            (0.5 * (l - r) / den).clamp(-0.5, 0.5) as f64
        } else {
            0.0
        };
        if let Some(delay) = self.fit_slope {
            let mut gap = (Track::fraction(delay, lag, p.len()) - parabola).abs();
            gap = gap.min(1.0 - gap);
            if gap < 0.25 {
                return delay;
            }
        }
        lag as f64 + parabola
    }

    /// PN595: the delay of the group of paths about the strongest lag `lag` of the last fit, symbols: the centre of the tap
    /// powers over the noise under a raised-cosine window `CLUSTER_HALF` taps to each side, the window moved onto the centre
    /// it finds. It starts from the centre of the last header while that is within two taps of `lag`. A received pulse
    /// that is several taps wide and flat has no one strongest tap: its peak hops between taps from header to header, while
    /// this centre moves with the clock only.
    fn cluster_delay(&mut self, lag: usize) -> f64 {
        let p = &self.fit_p;
        let noise = self.fit_noise;
        let mut c = match self.cluster {
            Some(c) if (c - lag as f64).abs() <= 2.0 => c,
            _ => lag as f64,
        };
        for _ in 0..4 {
            let lo = (c - CLUSTER_HALF).ceil().max(0.0) as usize;
            let hi = ((c + CLUSTER_HALF).floor() as usize).min(p.len().saturating_sub(1));
            let (mut m0, mut m1) = (0f64, 0f64);
            for (i, &v) in p.iter().enumerate().take(hi + 1).skip(lo) {
                let d = i as f64 - c;
                let w = 0.5 * (1.0 + (std::f64::consts::PI * d / CLUSTER_HALF).cos());
                let e = w * (v - noise).max(0.0) as f64;
                m0 += e;
                m1 += e * d;
            }
            if m0 <= 0.0 {
                break;
            }
            c += m1 / m0;
        }
        self.cluster = Some(c);
        c
    }

    /// PN595: the tap at index `j` of the last fit, and the carrier offset within the header at `at` from the phase step
    /// between the correlations of its two halves at that tap.
    fn pn595_carrier(&mut self, at: usize, h: Header, j: usize) -> Complex32 {
        let (pre, _) = tracking(h);
        let lag = j as isize - pre as isize;
        let n = h.len() as isize;
        let (mut z1, mut z2) = (Complex32::new(0.0, 0.0), Complex32::new(0.0, 0.0));
        for t in lag.max(0)..(n + lag).min(n) {
            let v = self.sym[at + t as usize] * self.chips[(t - lag) as usize];
            if t < n / 2 {
                z1 += v;
            } else {
                z2 += v;
            }
        }
        self.ls_half =
            (z2 * z1.conj()).arg() as f64 / std::f64::consts::TAU * self.fsym / (n / 2) as f64;
        self.ls_fit.taps[j]
    }

    /// Removes the fine carrier offset from the symbols up to `end` (once each, phase continuous).
    fn derotate(&mut self, end: usize) {
        let end = end.min(self.sym.len());
        let w = std::f64::consts::TAU * self.state.fine_hz / self.fsym;
        for i in self.rot_pos..end {
            self.rot_phase -= w;
            let ph = self.rot_phase;
            self.sym[i] *= Complex32::new(ph.cos() as f32, ph.sin() as f32);
        }
        self.rot_phase %= std::f64::consts::TAU;
        self.rot_pos = self.rot_pos.max(end);
    }

    /// The fractional position (−0.5 to 0.5 symbol) of the channel's delay `delay` (symbols, from the phase slope) against the
    /// strongest lag `lag` of an impulse response `n` taps long.
    pub fn fraction(delay: f64, lag: usize, n: usize) -> f64 {
        let n = n as f64;
        let mut d = delay - lag as f64;
        d -= n * (d / n).round();
        d - d.round()
    }

    /// One step of the timing loop with the strongest path at `frac` (symbols) of a frame of `period` symbols, `frames` frames
    /// after the last step. Returns the clock correction to ask for, ppm, or `None` when the step is left out: once the loop has
    /// settled, a move of the fraction that lies near half a symbol from the expected one cannot be unwrapped safely.
    fn timing_step(&mut self, frac: f64, period: usize, frames: usize) -> Option<f64> {
        let t = match self.t_prev {
            None => {
                // The loop starts from the correction in use, so a new grid does not undo what was learnt.
                self.timing
                    .set_integrator(self.applied_ppm * 1e-6 * period as f64);
                self.t_acc = frac;
                self.t_ref = frac;
                self.d_prev = 0.0;
                frac
            }
            Some(prev) => {
                // Unwrap the move of the fraction to the whole symbols nearest the move the last frames showed. The expected
                // move is smoothed, more once the loop has settled, so noise of the fraction at a low C/N does not slip.
                let want = self.d_prev * frames as f64;
                let mut d = frac - prev;
                let slip = (d - want).round();
                let settled = self.steps >= SETTLE_STEPS;
                if settled && (d - want - slip).abs() > 0.35 {
                    return None;
                }
                d -= slip;
                let a = if settled { 0.1 } else { 0.3 };
                self.d_prev += a * (d / frames.max(1) as f64 - self.d_prev);
                self.t_acc += d;
                self.t_acc
            }
        };
        self.steps += 1;
        self.t_prev = Some(frac);
        // A later path means the radio's clock is fast: take more input per symbol. The loop's output is a rate in symbols per
        // frame. Its integrator is the clock error it has learnt.
        let u = self.timing.update(t - self.t_ref);
        let lim = super::front::MAX_CLOCK_PPM;
        self.state.clock_ppm = (self.timing.integrator() / period as f64 * 1e6).clamp(-lim, lim);
        self.applied_ppm = (u / period as f64 * 1e6).clamp(-lim, lim);
        Some(self.applied_ppm)
    }

    /// The grid moved: the timing loop and the carrier phase start again from the new place and keep the clock they learnt.
    fn retime(&mut self) {
        self.t_prev = None;
        self.cluster = None;
        self.ls_prev = None;
    }

    /// Judges the header whose fit is in `fit_p`: `lag` its strongest lag, `n` the number of the header.
    fn classify(&mut self, h: Header, n: u64, lag: usize, usable: bool) -> Seen {
        if !usable {
            return Seen::Missing;
        }
        let wrap = h != Header::Pn595;
        let (period, ph) = if wrap {
            match self.phases.phase(h, n) {
                Some(ph) => (h.period(), ph),
                None => return Seen::Unplaced(lag),
            }
        } else {
            if !self.placed {
                self.placed = true;
                self.phases.place = lag;
            }
            (self.fit_p.len(), 0)
        };
        let place_of = |l: usize| if wrap { (l + ph) % period } else { l };
        let want = if wrap {
            (self.phases.place + period - ph) % period
        } else {
            self.phases.place
        };
        let p = &self.fit_p;
        let pmax = p[lag];
        let (near, p_near) = near_peak(p, want, wrap);
        if p_near >= 0.5 * pmax {
            self.challenger = (0, 0);
            self.phases.place = place_of(near);
            return Seen::Held(near);
        }
        if p_near > 25.0 * self.fit_noise {
            // The tracked path is there under a stronger one.
            let place = place_of(lag);
            let d = (place as isize - self.challenger.0 as isize).abs();
            self.challenger = if self.challenger.1 > 0 && d.min(period as isize - d) <= 1 {
                (place, self.challenger.1 + 1)
            } else {
                (place, 1)
            };
            if self.challenger.1 < TAKE_OVER {
                return Seen::Held(near);
            }
            self.challenger = (0, 0);
            self.phases.place = place;
            self.t_prev = None;
            self.ls_prev = None;
            return Seen::Held(lag);
        }
        let mut e = lag as isize - want as isize;
        if wrap {
            let n = period as isize;
            e = (e + n / 2).rem_euclid(n) - n / 2;
        }
        Seen::Shifted(e)
    }

    /// A PN420 or PN945 header that the fit places `e` symbols from `hb`: the fit knows a place only within one period of
    /// the core, so the header may start one period earlier or later. Returns the move, of the three, at which the cyclic
    /// parts of the header repeat most. A place whose symbols are not held yet does not count.
    fn whole_move(&self, hb: usize, e: isize, h: Header) -> isize {
        let (core, pre, post) = h.core_pre_post();
        let n = h.period() as isize;
        let (mut best, mut at) = (-1f32, e);
        for k in [0, -1, 1] {
            let c = hb as isize + e + k * n;
            if c < 0 || c as usize + h.len() > self.sym.len() {
                continue;
            }
            let m = self.lag_corr(c as usize, core, pre + post).norm();
            if m > best {
                (best, at) = (m, e + k * n);
            }
        }
        at
    }

    /// Looks for the header that should start at `hb` over the body before it (the frame starts at `at` with a header that
    /// was received whole): a radio that lost samples delivers the header early by the samples lost. Returns where the
    /// header starts, and whether that place is exact (PN595 and a fixed phase) or within a few symbols (a rotating phase,
    /// whose place the next headers settle).
    fn relock(&mut self, at: usize, hb: usize, h: Header) -> Option<(usize, bool)> {
        let l = h.len();
        if h == Header::Pn595 {
            // The correlation with the sequence at every position; the carrier offset is already removed.
            let (mut best, mut pos) = (0f32, 0usize);
            for p in at + l..hb {
                let mut c = Complex32::new(0.0, 0.0);
                for (v, &chip) in self.sym[p..p + l].iter().zip(&self.chips) {
                    c += v * chip;
                }
                if c.norm_sqr() > best {
                    (best, pos) = (c.norm_sqr(), p);
                }
            }
            let e: f32 = self.sym[pos..pos + l].iter().map(|v| v.norm_sqr()).sum();
            if best < PN595_MIN_SHARE * l as f32 * e {
                return None;
            }
            // The strongest path belongs at the tracked tap of the fit.
            let (pre, _) = tracking(h);
            let start = (pos + pre).checked_sub(self.phases.place)?;
            let (_, lag, usable) = self.fit(start, h);
            let ok =
                usable && start >= at + l && start + 3 < hb && lag.abs_diff(self.phases.place) <= 3;
            return ok.then_some((start, true));
        }
        let (core, pre, post) = h.core_pre_post();
        let (pk, v, med, _) = self
            .lags
            .run(&self.sym[at + l..], core, pre + post, BODY, 1);
        if v < 0.4 || v < 4.0 * med {
            return None;
        }
        let coarse = at + l + pk;
        if self.phases.fixed != Some(true) {
            return (coarse + 16 < hb).then_some((coarse, false));
        }
        let (_, lag, usable) = self.fit(coarse, h);
        if !usable {
            return None;
        }
        let n = h.period() as isize;
        let e = (lag as isize - self.phases.place as isize + n / 2).rem_euclid(n) - n / 2;
        let start = coarse as isize + e;
        if start < (at + l) as isize || start + 3 >= hb as isize {
            return None;
        }
        // The header must fit at the place found, with the tracked path where the grid has it.
        let start = start as usize;
        let (_, lag, usable) = self.fit(start, h);
        let d = (lag as isize - self.phases.place as isize).rem_euclid(n);
        (usable && d.min(n - d) <= 3).then_some((start, true))
    }

    /// The lock rules that need the system information, once per frame. True when the lock went.
    fn si_rules(&mut self, io: &mut Io, time: f64, period: usize) -> bool {
        if self.si_known {
            self.no_si = 0;
            self.swaps = 0;
            return false;
        }
        if self.state.frames == 0 {
            return false;
        }
        self.no_si += 1;
        let frames = |s: f64| (s * self.fsym / period as f64) as u32;
        let (weak, slow, clean) = (200, frames(1.5), frames(0.4));
        let cn = self.state.cn_db;
        if cn < -15.0 && self.no_si >= weak {
            self.lose(io, time, "frame lock lost: signal too weak");
        } else if cn < 6.0 && self.no_si >= slow {
            self.lose(io, time, "frame lock lost: no system information");
        } else if cn >= 6.0 && self.no_si >= clean && self.swaps < MAX_SWAPS {
            // The header survives a mirrored spectrum, the body does not.
            self.swaps += 1;
            self.swapped = !self.swapped;
            io.items_out::<Command>(0).push(Timed {
                time,
                value: Command::new(FrontSetting::IqSwap.id(), Value::Bool(self.swapped)),
            });
            self.lose(
                io,
                time,
                "no system information: I and Q swapped, searching again",
            );
        } else {
            return false;
        }
        true
    }

    fn frames(&mut self, io: &mut Io) {
        loop {
            let Some(h) = self.hdr.as_ref().map(|s| s.kind) else {
                return;
            };
            let l = h.len();
            let period = l + BODY;
            let (core, pre, post) = h.core_pre_post();
            let at = self.next.round() as usize;
            let hb = at + period;
            if hb + l + 4 > self.sym.len() {
                return;
            }
            self.derotate(hb + l + 4);
            let time = (self.base as f64 + at as f64) / self.fsym;
            if self.si_rules(io, time, period) {
                self.emit(io, time, period);
                continue;
            }
            if is_silent(&self.sym[hb..hb + l]) {
                self.erased_run += 1;
                self.since_step += 1;
                self.state.erased += 1;
                self.send_frame(io, at, h, time, true);
                self.next += period as f64;
                self.ls_prev = None;
                if self.erased_run >= MAX_ERASED {
                    self.lose(io, time, "frame lock lost: no signal");
                }
                self.emit(io, time, period);
                continue;
            }
            self.erased_run = 0;
            let (cn, lag, usable) = self.fit(hb, h);
            let seen = self.classify(h, self.index + 1, lag, usable);
            self.recent.push_back(seen != Seen::Missing);
            while self.recent.len() > 50 {
                self.recent.pop_front();
            }
            let bad = self.recent.iter().filter(|u| !**u).count();
            self.state.no_header_pct = 100.0 * bad as f64 / self.recent.len().max(1) as f64;
            // Where the next frame starts, and whether this one is erased.
            let mut next = hb;
            let mut erase = false;
            match seen {
                Seen::Missing => {
                    self.lost_frames += 1;
                    self.since_step += 1;
                    erase = true;
                    if let Some((start, exact)) = self.relock(at, hb, h) {
                        let g = self.gap.get_or_insert((0, 0));
                        g.0 += start as isize - hb as isize;
                        next = start;
                        self.retime();
                        if exact {
                            self.lost_frames = 0;
                        } else {
                            self.phases.gap();
                        }
                    }
                }
                Seen::Shifted(_) if self.phases.fixed == Some(false) => {
                    // A rotating header: the move of the path and the frames lost show in the place of the super-frame.
                    self.since_step += 1;
                    erase = true;
                    self.gap.get_or_insert((0, 0));
                    self.phases.gap();
                    self.phases.identify(h, self.index + 1, lag);
                }
                Seen::Shifted(e) => {
                    self.since_step += 1;
                    erase = true;
                    let e = if h == Header::Pn595 {
                        e
                    } else {
                        self.whole_move(hb, e, h)
                    };
                    self.jump = if self.jump.1 > 0 && (self.jump.0 - e).abs() <= 1 {
                        (e, self.jump.1 + 1)
                    } else {
                        (e, 1)
                    };
                    if self.jump.1 >= JUMP_FRAMES {
                        self.jump = (0, 0);
                        let g = self.gap.get_or_insert((0, 0));
                        g.0 += e;
                        next = (hb as isize + e) as usize;
                        self.retime();
                    }
                }
                Seen::Held(_) | Seen::Unplaced(_) => {}
            }
            if let Seen::Unplaced(lag) = seen {
                if let Some(r) = self.phases.identify(h, self.index + 1, lag) {
                    self.lost_pending += r.lost as u32;
                    let shift = self.whole_move(hb, r.shift, h);
                    if shift != 0 {
                        // The grid is off by the move of the path: this frame is not decoded at the wrong place, and the
                        // path is back at its place once the grid has moved.
                        erase = true;
                        next = (hb as isize + shift) as usize;
                        let n = h.period() as isize;
                        self.phases.place =
                            (self.phases.place as isize - shift).rem_euclid(n) as usize;
                        self.retime();
                    }
                    let g = self.gap.get_or_insert((0, 0));
                    g.0 += shift;
                }
            }
            if let Seen::Held(lag) | Seen::Unplaced(lag) = seen {
                self.jump = (0, 0);
                self.lost_frames = 0;
                // Fine carrier offset from the header's own repetition (what is left after the derotation).
                if h != Header::Pn595 {
                    let c = self.lag_corr(hb, core, pre + post);
                    let f = c.arg() as f64 / std::f64::consts::TAU * self.fsym / core as f64;
                    self.state.fine_hz += 0.25 * f;
                } else {
                    // PN595: the phase of the tracked tap from one usable header to the next, unwrapped to the offset its
                    // two halves give.
                    let tap = self.pn595_carrier(hb, h, lag);
                    let half = self.ls_half;
                    let f = match self.ls_prev {
                        Some((prev, k)) => {
                            let span = (k * period) as f64 / self.fsym;
                            let dphi = (tap * prev.conj()).arg() as f64;
                            let turns = (half * span - dphi / std::f64::consts::TAU).round();
                            (dphi / std::f64::consts::TAU + turns) / span
                        }
                        None => half,
                    };
                    self.state.fine_hz += 0.25 * f;
                    self.ls_prev = Some((tap, 0));
                }
                let p = &self.fit_p;
                let st = &mut self.state;
                st.cn_db += if st.frames == 0 {
                    cn - st.cn_db
                } else {
                    0.2 * (cn - st.cn_db)
                };
                // Rotation: the strongest lag moves from frame to frame while the grid holds.
                if let Some(l) = self.last_lag {
                    self.rotating_votes =
                        (self.rotating_votes + if l != lag { 1 } else { -1 }).clamp(-20, 20);
                }
                self.last_lag = Some(lag);
                st.rotates = self.rotating_votes > 0;
                let pmax = p.iter().copied().fold(1e-30f32, f32::max);
                st.cir_db = self.cir_pool.exact(p.len(), |c| {
                    for (c, v) in c.iter_mut().zip(p) {
                        *c = 10.0 * (v / pmax).max(1e-6).log10();
                    }
                });
                let rel = |i: usize| {
                    let x = (i + p.len() - lag) % p.len();
                    if x > p.len() / 2 {
                        x as f64 - p.len() as f64
                    } else {
                        x as f64
                    }
                };
                let (mut lo, mut hi) = (0f64, 0f64);
                for (i, &v) in p.iter().enumerate() {
                    if v > 0.03 * pmax {
                        lo = lo.min(rel(i));
                        hi = hi.max(rel(i));
                    }
                }
                st.echo_span_s = (hi - lo) / self.fsym;
                st.frames += 1;
                if next == hb {
                    let delay = if h == Header::Pn595 {
                        self.cluster_delay(lag)
                    } else {
                        self.delay_at(lag, true)
                    };
                    let frac = Track::fraction(delay, lag, self.fit_p.len());
                    if let Some(ppm) = self.timing_step(frac, period, self.since_step + 1) {
                        self.since_step = 0;
                        io.items_out::<Command>(0).push(Timed {
                            time: (self.base as f64 + hb as f64) / self.fsym,
                            value: Command::new(FrontSetting::ClockPpm.id(), Value::F64(ppm)),
                        });
                    } else {
                        self.since_step += 1;
                    }
                }
            }
            if let Some(p) = self.ls_prev.as_mut() {
                p.1 += 1;
            }
            if let Some(lost) = self.send_frame(io, at, h, time, erase) {
                if let Some((moved, erased)) = self.gap.take() {
                    self.state.log.push(format!(
                        "DTMB: samples lost: frame grid moved by {moved} symbols, {} frame(s) erased",
                        erased + lost
                    ));
                }
            }
            self.next += next as f64 - at as f64;
            if self.lost_frames >= MAX_LOST {
                self.lose(io, time, "frame lock lost: no PN header");
            } else if self.gap.is_some_and(|g| g.1 > MAX_REGAIN_FRAMES) {
                self.lose(io, time, "frame lock lost: the gap is too long to count");
            }
            self.emit(io, time, period);
        }
    }

    /// Gives up the grid and asks the search to look again; `why` goes to the log.
    fn lose(&mut self, io: &mut Io, time: f64, why: &str) {
        self.state.log.push(format!("DTMB: {why}"));
        self.restart();
        io.items_out::<Command>(1).push(Timed {
            time,
            value: Command::new(SearchSetting::Search.id(), Value::Bool(true)),
        });
    }

    /// Sends the state of the frame at `time` and drops the symbols the grid has passed.
    fn emit(&mut self, io: &mut Io, time: f64, period: usize) {
        let st = self.state.clone();
        self.state.log.clear();
        io.items_out::<TrackState>(2)
            .push(Timed { time, value: st });
        let keep_from = (self.next as usize).saturating_sub(16);
        if keep_from > 8 * period {
            self.sym.drain(..keep_from);
            self.rot_pos -= keep_from;
            self.base += keep_from as u64;
            self.next -= keep_from as f64;
        }
    }
}

impl Component for Track {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dtmb.track",
            vec![
                Port::iq("in"),
                Port::typed::<HeaderFound>("found"),
                Port::command("control"),
            ],
            vec![
                Port::command("front").rate_from(RateFrom::Fixed(0.0)),
                Port::command("search").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<TrackState>("state").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<FrameSyms>("frames").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(TrackSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([
            (TrackSetting::Width, Value::Choice(self.width.index())),
            (TrackSetting::SiKnown, Value::Bool(self.si_known)),
        ])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fsym = self.width.symbol_rate();
        self.sym.clear();
        self.base = 0;
        self.rot_pos = 0;
        self.rot_phase = 0.0;
        self.state = TrackState::default();
        self.applied_ppm = 0.0;
        self.si_known = false;
        self.no_si = 0;
        self.swaps = 0;
        self.swapped = false;
        self.restart();
        Ok(vec![
            Format::new(Kind::typed::<Command>(), 0.0),
            Format::new(Kind::typed::<Command>(), 0.0),
            Format::new(Kind::typed::<TrackState>(), 0.0),
            Format::new(Kind::typed::<FrameSyms>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        // `dtmb.si` sends only `si_known`, which the schema takes.
        let _ = apply_commands(self, io, 2);
        let x = io.iq(0);
        if self.sym.is_empty() {
            self.base = io.input(0).first();
            self.rot_pos = 0;
        }
        self.sym.extend_from_slice(x);
        let found: Option<HeaderFound> = io.items::<HeaderFound>(1).last().map(|f| f.value);
        if let Some(f) = found {
            if self.hdr.is_none() {
                // The first frame start at or after the oldest symbol held.
                let period = (f.header.len() + BODY) as u64;
                let mut start = f.start;
                while start < self.base {
                    start += period;
                }
                self.set_header(f.header);
                self.next = (start - self.base) as f64;
                self.state.fine_hz = f.fine_hz;
                self.state.header = Some(f.header);
                self.state.tracking = true;
                self.lost_frames = 0;
                // The fine offset is removed in the symbol domain from the grid's start on.
                self.rot_pos = self.next as usize;
                self.rot_phase = 0.0;
                self.state
                    .log
                    .push(format!("DTMB: {} headers found", f.header.name()));
            }
        }
        if self.hdr.is_none() {
            // Keep a few frames, so the start the search reports is still held.
            let keep = 6 * (945 + BODY);
            if self.sym.len() > 2 * keep {
                let d = self.sym.len() - keep;
                self.sym.drain(..d);
                self.base += d as u64;
            }
            return;
        }
        self.frames(io);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TrackSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TrackSetting::Width => {
                self.width = WIDTH.read(id, v)?;
                Ok(Applied::Reformat)
            }
            TrackSetting::SiKnown => {
                self.si_known = SI_KNOWN.read(id, v)?;
                Ok(Applied::Live)
            }
        }
    }
}

/// The chips (+1 or -1) of the phase-0 header of `kind`.
fn seq_chips(kind: Header, out: &mut [f32]) {
    HeaderSeq::new(kind, true).chips(0, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_one_constant_magnitude() {
        assert!(is_silent(&[Complex32::new(0.0, 0.0); 420]));
        // A constant after DC removal and derotation keeps its magnitude while its phase turns.
        let turning: Vec<Complex32> = (0..420)
            .map(|k| Complex32::from_polar(1e-3, 0.01 * k as f32))
            .collect();
        assert!(is_silent(&turning));
        let mut signal = turning.clone();
        signal[200] *= 1.2;
        assert!(!is_silent(&signal));
        assert!(!is_silent(&[]));
    }

    #[test]
    fn the_schedule_gives_the_frames_lost_and_the_move_of_the_path() {
        // The lags of a path at place 40 under the rotating phase of PN945: four headers give the schedule.
        let kind = Header::Pn945;
        let p = kind.period();
        let lag = |place: usize, frame: usize| (place + p - kind.phase(frame)) % p;
        let mut ph = Phases::default();
        for n in 0..4u64 {
            assert_eq!(ph.phase(kind, n), None);
            assert_eq!(ph.identify(kind, n, lag(40, 30 + n as usize)), None);
        }
        assert_eq!(ph.fixed, Some(false));
        assert_eq!(ph.place, 40);
        assert_eq!(ph.phase(kind, 10), Some(kind.phase(40)));
        // A gap after header 9: 7 frames lost and the path 12 symbols earlier.
        ph.gap();
        assert_eq!(ph.phase(kind, 10), None);
        let mut got = None;
        for n in 10..14u64 {
            got = ph.identify(kind, n, lag(28, 30 + n as usize + 7));
        }
        assert_eq!(
            got,
            Some(Regained {
                lost: 7,
                shift: -12
            })
        );
        assert_eq!(ph.phase(kind, 14), Some(kind.phase(30 + 14 + 7)));
        // A fixed phase: four headers at one lag; a gap changes nothing.
        let mut ph = Phases::default();
        for n in 0..4u64 {
            ph.identify(kind, n, 17);
        }
        assert_eq!((ph.fixed, ph.place), (Some(true), 17));
        ph.gap();
        assert_eq!(ph.phase(kind, 99), Some(0));
        // A header missing between two others starts the count again.
        let mut ph = Phases::default();
        for n in [0u64, 1, 3, 4, 5] {
            ph.identify(kind, n, lag(40, n as usize));
        }
        assert_eq!(ph.fixed, None);
        ph.identify(kind, 6, lag(40, 6));
        assert_eq!(ph.fixed, Some(false));
    }

    #[test]
    fn the_tracked_path_is_looked_for_within_three_places() {
        let mut p = vec![0f32; 255];
        p[2] = 5.0;
        p[250] = 9.0;
        assert_eq!(near_peak(&p, 0, true), (2, 5.0));
        assert_eq!(near_peak(&p, 253, true), (250, 9.0));
        assert_eq!(near_peak(&p, 1, false), (2, 5.0));
        assert_eq!(near_peak(&p, 100, true), (97, 0.0));
    }
}
