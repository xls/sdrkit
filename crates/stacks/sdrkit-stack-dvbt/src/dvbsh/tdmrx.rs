//! The components of the SH-B receiver: `dvbsh.tdm_front` (the matched filter and the symbol timing at one symbol rate of
//! table 5.13 after another, or at a set one), `dvbsh.tdm` (the slot timing and the carrier from the pilot fields, the
//! SOF and the modulation, the signalling field and the soft de-mapping) and `dvbsh.tdm_deint` (the time
//! de-interleaver). `dvbsh.tdm` steers `dvbsh.tdm_front` through the link of the stack, so the two run in one stage.
//! The code words go to `dvbsh.decode`, which the two forms share.

use super::coding::{ShCodec, IU_BITS, SIGNALLING_BITS};
use super::ofdm::Width;
use super::rx::{Refusal, ShLink, ShWord};
use super::settings::{
    TdmFrontSetting, TdmSetting, WidthChoice, MAX_INTERLEAVER_MB, TDM_GOLD, TDM_SYMBOL_RATE, WIDTH,
};
use super::shframe::{FrameRx, FrameShape};
use super::tdm::{
    data_place, pilot_fields, quarter, scrambling, sof_bits, symbol_rate, Modulation, RollOff,
    Signalling, TdmFrame, HEADER_BITS, PILOTS, SECOND_FIELD, SIGNALLING_CODED, SLOT, SLOT_DATA,
    SOF_BITS,
};
use crate::params::Guard;
use sdrkit_algo_dsp::decim::HalfBand;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_modem::constellation::Demap;
use sdrkit_algo_modem::lock::Gears;
use sdrkit_algo_modem::timing::{MatchedBank, RunSymbolSync, SymbolSyncConfig};
use sdrkit_algo_modem::Constellation;
use sdrkit_core::{
    Choice, Complex32, ConfigError, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom,
    SlicePool, Timed,
};
use std::f64::consts::TAU;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Signal seconds a symbol rate is tried without slots found.
pub const SLOT_TRY_S: f64 = 0.1;
/// Signal seconds a symbol rate of the 5 MHz channel is kept with slots found but no SH frame; a narrower channel has
/// longer frames and is kept longer in the same ratio.
pub const FRAME_TRY_S: f64 = 0.7;
/// Signal seconds the slot and frame grids run on without pilots before the search starts again.
pub const FLYWHEEL_S: f64 = 2.0;
/// Slots that the slot search looks at.
pub const SEARCH_SLOTS: usize = 8;
/// The least coherent share of the pilot energy, in blocks of 20 symbols, that the slot search takes: 3 times that of
/// noise.
pub const SEARCH_MIN: f32 = 0.15;
/// The least coherent share of the energy of the first block of every pilot field at an offset that the slot search
/// measures in full: 1.5 times that of noise.
pub const SEARCH_FIRST: f32 = SEARCH_MIN / 2.0;
/// The least coherent share of the energy of the pilot fields of a slot that is not erased.
pub const PILOT_MIN: f32 = 0.15;
/// The least correlation of the first data symbols of a slot with the SOF.
pub const SOF_MIN: f32 = 0.5;
/// Frames in a row without the SOF after which the frame search starts again.
pub const SOF_MISSES: u32 = 3;
/// The carrier offset, in cycles per symbol, above which the front end is asked to move its mixer.
pub const SHIFT_MIN: f64 = 2e-3;
/// Slots between two checks of the carrier against the phase slope inside the pilot fields.
pub const WATCH_SLOTS: u32 = 16;
/// The noise bandwidths of the timing loop per symbol, to pull in and to track, and its clock range in ppm.
const TIMING: (f64, f64, f64) = (0.01, 0.001, 300.0);
/// Symbols that the mean of the clock error runs over.
const PPM_SYMBOLS: f64 = 200_000.0;
/// Symbols between two corrections of the symbol clock once the loop tracks.
const CLOCK_RUN: usize = 32;
/// The clock range in ppm about the held rate while slots are erased.
const HELD_PPM: f64 = 5.0;
/// Symbols either side of its place that the grid of an erased slot is looked for: the symbols that a new timing loop
/// can lose or add.
pub const GRID: usize = 96;
/// The least coherent share of the energy of a pilot field that moves the grid.
pub const GRID_MIN: f32 = 0.2;
/// Symbols of a block of the slot search.
const BLOCK: usize = 20;

/// The sum of the symbols `x` times the conjugates of the pilots `p`, and the energy of `x`. Symbol `i` goes to sum
/// `i mod 4`; the four sums are added at the end as (0 + 1) + (2 + 3). Every form keeps this order, so every target
/// returns the same bits.
#[cfg(any(test, not(target_arch = "aarch64")))]
#[inline]
fn block_corr_plain(x: &[Complex32; BLOCK], p: &[Complex32; BLOCK]) -> (Complex32, f32) {
    let (mut re, mut im, mut e) = ([0f32; 4], [0f32; 4], [0f32; 4]);
    for (xs, ps) in x.as_chunks::<4>().0.iter().zip(p.as_chunks::<4>().0) {
        for l in 0..4 {
            let (a, q) = (xs[l], ps[l]);
            re[l] += a.re * q.re + a.im * q.im;
            im[l] += a.im * q.re - a.re * q.im;
            e[l] += a.re * a.re + a.im * a.im;
        }
    }
    let sum = |v: [f32; 4]| (v[0] + v[1]) + (v[2] + v[3]);
    (Complex32::new(sum(re), sum(im)), sum(e))
}

/// [`block_corr_plain`] with NEON: the same operations in the same order, four symbols at a time.
#[cfg(target_arch = "aarch64")]
#[inline]
fn block_corr_neon(x: &[Complex32; BLOCK], p: &[Complex32; BLOCK]) -> (Complex32, f32) {
    use std::arch::aarch64::{
        vaddq_f32, vdupq_n_f32, vgetq_lane_f32, vld2q_f32, vmulq_f32, vsubq_f32,
    };
    const { assert!(BLOCK.is_multiple_of(4)) };
    let (xp, pp) = (x.as_ptr().cast::<f32>(), p.as_ptr().cast::<f32>());
    // SAFETY: a `Complex32` is two `f32` in the order re, im (`repr(C)`), so `x` and `p` are `2 * BLOCK` values each.
    // Load `g` reads the 8 values from `8 * g`, and `8 * g + 8 <= 2 * BLOCK`. NEON is part of every aarch64 target.
    unsafe {
        let zero = vdupq_n_f32(0.0);
        let (mut re, mut im, mut e) = (zero, zero, zero);
        for g in 0..BLOCK / 4 {
            let (a, q) = (vld2q_f32(xp.add(8 * g)), vld2q_f32(pp.add(8 * g)));
            re = vaddq_f32(re, vaddq_f32(vmulq_f32(a.0, q.0), vmulq_f32(a.1, q.1)));
            im = vaddq_f32(im, vsubq_f32(vmulq_f32(a.1, q.0), vmulq_f32(a.0, q.1)));
            e = vaddq_f32(e, vaddq_f32(vmulq_f32(a.0, a.0), vmulq_f32(a.1, a.1)));
        }
        let sum = |v| {
            (vgetq_lane_f32::<0>(v) + vgetq_lane_f32::<1>(v))
                + (vgetq_lane_f32::<2>(v) + vgetq_lane_f32::<3>(v))
        };
        (Complex32::new(sum(re), sum(im)), sum(e))
    }
}

/// The correlation of a block of the slot search with its pilots and the energy of the block, in the fastest form of
/// the target.
#[inline]
fn block_corr(x: &[Complex32; BLOCK], p: &[Complex32; BLOCK]) -> (Complex32, f32) {
    #[cfg(target_arch = "aarch64")]
    return block_corr_neon(x, p);
    #[cfg(not(target_arch = "aarch64"))]
    block_corr_plain(x, p)
}

type Timing = RunSymbolSync<Complex32, MatchedBank<64, 128>>;

/// A symbol rate that the front end tries, with the roll-off and the width of its row of table 5.13.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    /// Symbols per second.
    pub rate: f64,
    /// The roll-off.
    pub rolloff: RollOff,
    /// The channel width.
    pub width: Width,
}

/// The symbol rates for input at `in_rate`: every row of table 5.13 of `width` (of every width when `None`) whose signal
/// fits the input, or the rows that have the rate `set` (the three roll-offs at that rate when no row has it).
pub fn candidates(in_rate: f64, width: Option<Width>, set: f64) -> Vec<Candidate> {
    let fits = |c: &Candidate| c.rate * (1.0 + c.rolloff.value()) <= 1.02 * in_rate;
    let mut all = Vec::new();
    for w in Width::SEARCH {
        if width.is_some_and(|x| x != w) {
            continue;
        }
        for guard in [Guard::G4, Guard::G8, Guard::G16, Guard::G32] {
            for rolloff in RollOff::ALL {
                let c = Candidate {
                    rate: symbol_rate(w, guard, rolloff),
                    rolloff,
                    width: w,
                };
                if !all.contains(&c) {
                    all.push(c);
                }
            }
        }
    }
    if set > 0.0 {
        all.retain(|c| (c.rate / set - 1.0).abs() < 1e-4);
        if all.is_empty() {
            let width = width.unwrap_or(Width::W5);
            all.extend(RollOff::ALL.map(|rolloff| Candidate {
                rate: set,
                rolloff,
                width,
            }));
        }
    }
    all.retain(fits);
    all
}

/// Symbols of the timing loop at one symbol rate. The time of the item is that of the input of the stack after the
/// pass that made it.
#[derive(Clone, Debug)]
pub struct TdmSymbols {
    /// The symbols, at unit mean power.
    pub sym: Arc<[Complex32]>,
    /// The symbols start a new stream: another symbol rate, a moved mixer, or samples after a gap.
    pub start: bool,
    /// The symbol rate in use.
    pub candidate: Candidate,
    /// The clock error that the timing loop holds, ppm.
    pub clock_ppm: f32,
    /// The frequency that the mixer takes off, Hz.
    pub mix_hz: f64,
}

impl Item for TdmSymbols {
    const KIND: &'static str = "dvbsh.tdm_symbols";
}

/// The running chain of the front end at one symbol rate.
#[derive(Clone)]
struct Chain {
    mix: Nco,
    halves: Vec<HalfBand<Complex32>>,
    timing: Timing,
    alpha: f64,
    /// The mean samples per symbol of the loop while the demodulator follows the slots.
    sps: f64,
    /// The loop is one that holds the rate of a blockage.
    held: bool,
}

/// The matched filter and the timing loop at `sps` samples per symbol for roll-off `alpha`, within `max_ppm`.
fn timing(sps: f64, alpha: f64, max_ppm: f64) -> Option<Timing> {
    let mut cfg = SymbolSyncConfig::new(sps);
    cfg.pulse = Shape::Rc { beta: alpha };
    cfg.bn_t = TIMING.1;
    cfg.gears = Some(Gears {
        pull_in_bn_t: TIMING.0,
        track_bn_t: TIMING.1,
    });
    cfg.max_ppm = max_ppm;
    MatchedBank::new(Shape::Rrc { beta: alpha }, sps)
        .and_then(|bank| Timing::new(bank, cfg, CLOCK_RUN))
        .ok()
}

/// `dvbsh.tdm_front`.
pub struct TdmFront {
    sh: Arc<ShLink>,
    choice: WidthChoice,
    set_rate: f64,
    in_rate: f64,
    cands: Vec<Candidate>,
    at: usize,
    chain: Option<Chain>,
    /// The chain of each symbol rate as designed, before its first sample; made at the first turn of the rate.
    designs: Vec<Option<Chain>>,
    /// Signal seconds at this symbol rate since the demodulator last moved forward.
    since: f64,
    stage: u8,
    /// The stage at the pass before, for the mean of the clock error.
    stage_before: u8,
    epoch: u32,
    shifts: u32,
    mix_hz: f64,
    start: bool,
    /// The clock error of the timing loop, ppm: its mean while the loop is held.
    ppm: f64,
    tmp: Vec<Complex32>,
    tmp2: Vec<Complex32>,
    syms: Vec<Complex32>,
    pool: SlicePool<Complex32>,
}

impl TdmFront {
    /// A front end on `sh`.
    pub fn new(sh: Arc<ShLink>) -> Self {
        TdmFront {
            sh,
            choice: WidthChoice::Auto,
            set_rate: 0.0,
            in_rate: 0.0,
            cands: Vec::new(),
            at: 0,
            chain: None,
            designs: Vec::new(),
            since: 0.0,
            stage: 0,
            stage_before: 0,
            epoch: 0,
            shifts: 0,
            mix_hz: 0.0,
            start: true,
            ppm: 0.0,
            tmp: Vec::new(),
            tmp2: Vec::new(),
            syms: Vec::new(),
            pool: SlicePool::default(),
        }
    }

    /// Starts the chain of the symbol rate in turn from its design.
    fn start_chain(&mut self) {
        self.since = 0.0;
        self.mix_hz = 0.0;
        self.start = true;
        self.ppm = 0.0;
        if self.designs.len() != self.cands.len() {
            self.designs.clear();
            self.designs.resize_with(self.cands.len(), || None);
        }
        if let Some(d) = self.designs.get_mut(self.at) {
            if d.is_none() {
                *d = Self::design(self.in_rate, self.cands[self.at]);
            }
        }
        self.chain = self.designs.get(self.at).cloned().flatten();
    }

    /// The chain for `c` on input at `in_rate`: half-band decimators down to less than 4.8 samples per symbol, then
    /// the matched filter and the timing loop.
    fn design(in_rate: f64, c: Candidate) -> Option<Chain> {
        let alpha = c.rolloff.value();
        let mut halves = Vec::new();
        let mut r = in_rate;
        while r / 2.0 >= 2.4 * c.rate && halves.len() < 8 {
            let pass = (c.rate * (1.0 + alpha) / 2.0).min(0.2 * r);
            match HalfBand::design(r, pass, 70.0) {
                Ok(h) => halves.push(h),
                Err(_) => break,
            }
            r /= 2.0;
        }
        let sps = r / c.rate;
        timing(sps, alpha, TIMING.2).map(|timing| Chain {
            mix: Nco::new(in_rate, 0.0),
            halves,
            timing,
            alpha,
            sps,
            held: false,
        })
    }

    /// The seconds the symbol rate in turn is kept in stage `stage` of the demodulator.
    fn limit(&self, stage: u8) -> f64 {
        if stage == 0 {
            return SLOT_TRY_S;
        }
        let mhz = self.cands.get(self.at).map_or(5.0, |c| c.width.mhz());
        FRAME_TRY_S * (5.0 / mhz).max(1.0)
    }
}

impl Component for TdmFront {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbsh.tdm_front",
            vec![Port::iq("in")],
            vec![Port::typed::<TdmSymbols>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(TdmFrontSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.in_rate = inputs[0].rate;
        self.cands = candidates(self.in_rate, self.choice.width(), self.set_rate);
        self.designs.clear();
        self.at = 0;
        self.stage = 0;
        self.epoch = self.sh.tdm_epoch.load(Ordering::Relaxed);
        self.shifts = self.sh.tdm_shifts.load(Ordering::Relaxed);
        self.start_chain();
        Ok(vec![Format::new(Kind::typed::<TdmSymbols>(), 0.0)])
    }

    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        if x.is_empty() {
            return;
        }
        let epoch = self.sh.tdm_epoch.load(Ordering::Relaxed);
        if epoch != self.epoch {
            // Samples after a gap: the chain starts again at the same symbol rate.
            self.epoch = epoch;
            self.start_chain();
        }
        let stage = self.sh.tdm_stage.load(Ordering::Relaxed);
        if stage == 2 {
            self.since = 0.0;
        }
        self.stage = stage;
        if stage == 0 && self.chain.as_ref().is_some_and(|c| c.held) {
            // The search starts again: the loop gets its whole clock range back.
            self.start_chain();
        }
        self.since += x.len() as f64 / self.in_rate.max(1.0);
        if self.since > self.limit(stage) && self.cands.len() > 1 {
            self.at = (self.at + 1) % self.cands.len();
            self.start_chain();
        }
        let shifts = self.sh.tdm_shifts.load(Ordering::Relaxed);
        if shifts != self.shifts {
            self.shifts = shifts;
            self.mix_hz += f64::from_bits(self.sh.tdm_shift.load(Ordering::Relaxed));
            self.start = true;
            if let Some(c) = self.chain.as_mut() {
                c.mix.set_freq(-self.mix_hz);
            }
        }
        let (Some(c), Some(&candidate)) = (self.chain.as_mut(), self.cands.get(self.at)) else {
            return;
        };
        if !c.held && stage >= 1 && self.sh.tdm_blocked.load(Ordering::Relaxed) {
            // Slots without pilots: a loop that stays at the rate held so far takes over.
            if let Some(t) = timing(c.sps, c.alpha, HELD_PPM) {
                c.timing = t;
                c.held = true;
            }
        }
        c.timing.hold(stage >= 1);
        self.tmp.clear();
        self.tmp.extend_from_slice(x);
        c.mix.mix_in_place(&mut self.tmp);
        for h in c.halves.iter_mut() {
            self.tmp2.clear();
            h.process(&self.tmp, &mut self.tmp2);
            std::mem::swap(&mut self.tmp, &mut self.tmp2);
        }
        self.syms.clear();
        c.timing.process(&self.tmp, &mut self.syms);
        if self.syms.is_empty() {
            return;
        }
        if !c.held {
            let (ppm, sps) = (c.timing.clock_ppm(), c.timing.sps_now());
            if stage >= 1 && self.stage_before >= 1 {
                let w = (self.syms.len() as f64 / PPM_SYMBOLS).min(1.0);
                self.ppm += w * (ppm - self.ppm);
                c.sps += w * (sps - c.sps);
            } else {
                (self.ppm, c.sps) = (ppm, sps);
            }
        }
        self.stage_before = stage;
        let syms = &self.syms;
        let mut finite = true;
        let sym = self.pool.exact(syms.len(), |v| {
            for (o, s) in v.iter_mut().zip(syms) {
                *o = if s.re.is_finite() && s.im.is_finite() {
                    *s
                } else {
                    finite = false;
                    Complex32::default()
                };
            }
        });
        let value = TdmSymbols {
            sym,
            start: std::mem::take(&mut self.start),
            candidate,
            clock_ppm: self.ppm as f32,
            mix_hz: self.mix_hz,
        };
        io.items_out::<TdmSymbols>(0).push(Timed {
            time: self.sh.samples.load(Ordering::Relaxed) as f64 / self.in_rate.max(1.0),
            value,
        });
        if !finite {
            // The loop holds a value that is not a number: it starts again.
            self.start_chain();
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TdmFrontSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TdmFrontSetting::Width => self.choice = WIDTH.read(id, v)?,
            TdmFrontSetting::SymbolRate => self.set_rate = TDM_SYMBOL_RATE.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (TdmFrontSetting::Width, Value::Choice(self.choice.index())),
            (TdmFrontSetting::SymbolRate, Value::F64(self.set_rate)),
        ])
    }
}

/// Soft values of the DATA part of a running data path, in the order sent. The time of the item is that of the symbols
/// that held them.
#[derive(Clone, Debug)]
pub struct TdmUnits {
    /// The soft values: whole interleaving units of 126.
    pub soft: Arc<[i8]>,
    /// The shape of the frame of the data path.
    pub shape: FrameShape,
    /// The Punct_Pat_ID.
    pub id: u8,
    /// The count of the data paths started so far. A new count starts the de-interleaver again.
    pub starts: u32,
}

impl Item for TdmUnits {
    const KIND: &'static str = "dvbsh.tdm_units";
}

/// The state of `dvbsh.tdm` that the results show.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TdmState {
    /// 0 searching, 1 slots found, 2 an SH frame with its signalling field.
    pub stage: u8,
    /// The symbol rate in turn.
    pub candidate: Option<Candidate>,
    /// The modulation that the SOF shows.
    pub modulation: Option<Modulation>,
    /// The signalling field in force.
    pub signalling: Option<Signalling>,
    /// The layout of the frame of the running data path.
    pub frame: Option<TdmFrame>,
    /// Why the data path does not run.
    pub refused: Option<Refusal>,
    /// The carrier offset, Hz.
    pub cfo_hz: f64,
    /// The clock error, ppm.
    pub clock_ppm: f32,
    /// The power of a pilot symbol over the noise in the symbol rate, dB.
    pub mer_db: Option<f32>,
    /// The data paths started so far.
    pub starts: u32,
    /// Slots demodulated.
    pub slots: u64,
    /// Slots erased for want of pilots.
    pub slots_erased: u64,
    /// Signalling fields that did not decode.
    pub signalling_failed: u64,
}

impl Item for TdmState {
    const KIND: &'static str = "dvbsh.tdm_state";
}

/// The carrier and the levels of a tracked slot grid.
#[derive(Clone, Copy, Debug, Default)]
struct Track {
    /// The carrier offset left in the symbols, cycles per symbol.
    f: f64,
    /// The smoothed power of a pilot symbol and of the noise.
    signal: f32,
    noise: f32,
    /// The phase slope inside the pilot fields, summed since the last check, and the slots in the sum.
    slope: Complex32,
    slope_slots: u32,
    /// Slots erased in a row.
    erased: u64,
    /// A slot with pilots was seen since the search.
    seen: bool,
}

/// The place in the SH frame and the data path.
struct Frame {
    modulation: Modulation,
    /// The place in the frame of the next bit.
    bit: usize,
    /// The bits of the frame, once a signalling field decoded.
    bits: Option<usize>,
    layout: Option<TdmFrame>,
    /// The data path runs: the units of the DATA part go to the de-interleaver.
    run: bool,
    misses: u32,
    /// Bits in `unit`.
    unit_n: usize,
}

/// `dvbsh.tdm`.
pub struct TdmDemod {
    sh: Arc<ShLink>,
    gold: u32,
    max_mb: f64,
    scr: Vec<u8>,
    pilots: [[Complex32; PILOTS]; 2],
    cons: Vec<Constellation>,
    /// The SOF in the symbols of each modulation.
    sof: Vec<Vec<Complex32>>,
    codec: Option<ShCodec>,
    /// Symbols not yet used; a tracked slot starts at `base`.
    buf: Vec<Complex32>,
    base: usize,
    track: Option<Track>,
    /// Symbols are dropped until the front end starts a new stream.
    waiting: bool,
    frame: Option<Frame>,
    signalling: Option<Signalling>,
    refused: Option<Refusal>,
    candidate: Option<Candidate>,
    mix_hz: f64,
    clock_ppm: f32,
    rot: Vec<Complex32>,
    data: Vec<Complex32>,
    soft: Vec<i8>,
    cell: [f32; 4],
    sig_llr: Vec<i8>,
    sig_bits: Vec<u8>,
    unit: [i8; IU_BITS],
    /// The whole units of the running data path not yet passed on.
    units: Vec<i8>,
    /// The frame shape and the Punct_Pat_ID of the data path started last.
    path: Option<(FrameShape, u8)>,
    pool: SlicePool<i8>,
    out: Vec<Timed<TdmUnits>>,
    starts: u32,
    slots: u64,
    slots_erased: u64,
    signalling_failed: u64,
    now: f64,
}

impl TdmDemod {
    /// A demodulator on `sh`.
    pub fn new(sh: Arc<ShLink>) -> Self {
        let sof_bits = sof_bits();
        let sets = Modulation::ALL.map(Modulation::point_set);
        let sof = sets
            .iter()
            .map(|set| {
                let m = set.bits() as usize;
                sof_bits
                    .chunks_exact(m)
                    .map(|g| {
                        let label = g.iter().fold(0usize, |a, &b| a << 1 | b as usize);
                        set.point(label).unwrap_or_default()
                    })
                    .collect()
            })
            .collect();
        let scr = scrambling(0);
        TdmDemod {
            sh,
            gold: 0,
            max_mb: 768.0,
            pilots: pilot_fields(&scr),
            scr,
            cons: sets
                .into_iter()
                .filter_map(|s| Constellation::new(s).ok())
                .collect(),
            sof,
            codec: ShCodec::signalling().ok(),
            buf: Vec::new(),
            base: 0,
            track: None,
            waiting: false,
            frame: None,
            signalling: None,
            refused: None,
            candidate: None,
            mix_hz: 0.0,
            clock_ppm: 0.0,
            rot: Vec::new(),
            data: Vec::new(),
            soft: Vec::new(),
            cell: [0.0; 4],
            sig_llr: vec![0; SIGNALLING_CODED],
            sig_bits: vec![0; SIGNALLING_BITS],
            unit: [0; IU_BITS],
            units: Vec::new(),
            path: None,
            pool: SlicePool::default(),
            out: Vec::new(),
            starts: 0,
            slots: 0,
            slots_erased: 0,
            signalling_failed: 0,
            now: 0.0,
        }
    }

    fn set_gold(&mut self, n: u32) {
        self.gold = n;
        self.scr = scrambling(n);
        self.pilots = pilot_fields(&self.scr);
    }

    /// Back to the slot search: the grids, the signalling and the data path are dropped.
    fn restart(&mut self) {
        self.buf.clear();
        self.base = 0;
        self.track = None;
        self.frame = None;
        self.signalling = None;
        self.refused = None;
        self.sh.tdm_blocked.store(false, Ordering::Relaxed);
    }

    fn stage(&self) -> u8 {
        match (&self.track, &self.frame) {
            (None, _) => 0,
            (Some(_), Some(f)) if f.run => 2,
            _ => 1,
        }
    }

    /// The coherent share, in blocks of 20 symbols, of the energy of the pilot fields of [`SEARCH_SLOTS`] slots from
    /// symbol `o` of the buffer.
    #[inline]
    fn share(&self, o: usize) -> f32 {
        let z = &self.buf;
        let (mut m, mut e) = (0f32, 0f32);
        for s in 0..SEARCH_SLOTS {
            for (k, field) in self.pilots.iter().enumerate() {
                let at = o + s * SLOT + k * SECOND_FIELD;
                let blocks = z[at..at + PILOTS].as_chunks::<BLOCK>().0;
                for (xb, pb) in blocks.iter().zip(field.as_chunks::<BLOCK>().0) {
                    let (c, eb) = block_corr(xb, pb);
                    m += c.norm_sqr();
                    e += eb;
                }
            }
        }
        m / (BLOCK as f32 * e).max(1e-20)
    }

    /// Looks for the slot grid in the first [`SEARCH_SLOTS`] slots of the buffer: the offset whose pilot fields hold the
    /// most coherent energy in blocks of 20 symbols, and the carrier offset there from the phase steps between the blocks
    /// and then between the fields. Only an offset whose first blocks reach [`SEARCH_FIRST`] is measured in full.
    /// `None` when no offset reaches [`SEARCH_MIN`].
    fn search(&self) -> Option<(usize, f64)> {
        let z = &self.buf;
        let first = self.pilots.map(|f| f.as_chunks::<BLOCK>().0[0]);
        let mut best = (0usize, 0f32);
        for o in 0..SLOT {
            let (mut m, mut e) = (0f32, 0f32);
            for s in 0..SEARCH_SLOTS {
                for (k, pb) in first.iter().enumerate() {
                    let at = o + s * SLOT + k * SECOND_FIELD;
                    for xb in z[at..at + BLOCK].as_chunks::<BLOCK>().0 {
                        let (c, eb) = block_corr(xb, pb);
                        m += c.norm_sqr();
                        e += eb;
                    }
                }
            }
            if m < SEARCH_FIRST * BLOCK as f32 * e {
                continue;
            }
            let r = self.share(o);
            if r > best.1 {
                best = (o, r);
            }
        }
        let (o, r) = best;
        if r < SEARCH_MIN {
            return None;
        }
        const BLOCKS: usize = PILOTS / BLOCK;
        let mut c = [[Complex32::default(); BLOCKS]; 2 * SEARCH_SLOTS];
        for (j, row) in c.iter_mut().enumerate() {
            let at = o + (j / 2) * SLOT + (j % 2) * SECOND_FIELD;
            let blocks = z[at..at + PILOTS].as_chunks::<BLOCK>().0;
            let pilots = self.pilots[j % 2].as_chunks::<BLOCK>().0;
            for ((cb, x), p) in row.iter_mut().zip(blocks).zip(pilots) {
                *cb = block_corr(x, p).0;
            }
        }
        let steps: Complex32 = c
            .iter()
            .flat_map(|row| row.windows(2).map(|w| w[1] * w[0].conj()))
            .sum();
        let f1 = f64::from(steps.arg()) / (TAU * BLOCK as f64);
        let turn = |cycles: f64| {
            let (s, c) = (-TAU * cycles).sin_cos();
            Complex32::new(c as f32, s as f32)
        };
        let fields: Vec<Complex32> = c
            .iter()
            .map(|row| {
                row.iter()
                    .enumerate()
                    .map(|(b, &cb)| cb * turn(f1 * (b * BLOCK) as f64))
                    .sum()
            })
            .collect();
        let between: Complex32 = fields.windows(2).map(|w| w[1] * w[0].conj()).sum();
        let rest = between * turn(f1 * SECOND_FIELD as f64);
        Some((o, f1 + f64::from(rest.arg()) / (TAU * SECOND_FIELD as f64)))
    }

    /// The sums of the two halves of the pilot field `k` of the symbols `z` (a field from its first symbol), turned by
    /// `rot`, and the energy of the field.
    fn field(&self, k: usize, z: &[Complex32], rot: &[Complex32]) -> (Complex32, Complex32, f32) {
        let mut h = [Complex32::default(); 2];
        let mut e = 0f32;
        for (i, ((a, r), p)) in z.iter().zip(rot).zip(&self.pilots[k]).enumerate() {
            h[i / (PILOTS / 2)] += a * r * p.conj();
            e += a.norm_sqr();
        }
        (h[0], h[1], e)
    }

    /// Demodulates the slot at `base`, of which the buffer holds every symbol, the first pilot field of the slot after
    /// it and [`GRID`] symbols more. The phase of the carrier runs in a line from the middle of one pilot field to the middle of the next. A slot
    /// whose pilot fields hold too little coherent energy is erased.
    fn slot(&mut self, time: f64) {
        let Some(mut t) = self.track else {
            return;
        };
        let n = SLOT + PILOTS;
        // The carrier offset as a turn per symbol from the start of the slot.
        self.rot.clear();
        let (s, c) = (-TAU * t.f).sin_cos();
        let step = (c, s);
        let mut r = (1.0f64, 0.0f64);
        for _ in 0..n {
            self.rot.push(Complex32::new(r.0 as f32, r.1 as f32));
            r = (r.0 * step.0 - r.1 * step.1, r.0 * step.1 + r.1 * step.0);
        }
        let z = &self.buf[self.base..self.base + n];
        let rot = &self.rot;
        let at = [0, SECOND_FIELD, SLOT];
        let f: Vec<(Complex32, Complex32, f32)> = at
            .iter()
            .enumerate()
            .map(|(j, &a)| self.field(j % 2, &z[a..a + PILOTS], &rot[a..a + PILOTS]))
            .collect();
        let p: Vec<Complex32> = f.iter().map(|x| x.0 + x.1).collect();
        let energy = f[0].2 + f[1].2;
        let coherent = (p[0].norm_sqr() + p[1].norm_sqr()) / PILOTS as f32;
        let good = energy > 0.0 && coherent > PILOT_MIN * energy;
        self.slots += 1;
        self.data.clear();
        if good {
            let signal = coherent / (2 * PILOTS) as f32;
            let noise = (energy / (2 * PILOTS) as f32 - signal).max(1e-4 * signal);
            if t.seen {
                t.signal += (signal - t.signal) / 16.0;
                t.noise += (noise - t.noise) / 16.0;
            } else {
                (t.signal, t.noise) = (signal, noise);
            }
            t.seen = true;
            t.erased = 0;
            let d12 = (p[1] * p[0].conj()).arg();
            let next_good = p[2].norm_sqr() > PILOT_MIN * f[2].2 * PILOTS as f32;
            let d23 = if next_good {
                (p[2] * p[1].conj()).arg()
            } else {
                d12
            };
            let gain = 1.0 / signal.sqrt();
            let mid = (PILOTS as f32 - 1.0) / 2.0;
            let theta = p[0].arg();
            for (half, slope) in [d12, d23].into_iter().enumerate() {
                let per = slope / SECOND_FIELD as f32;
                let first = data_place(half * (SLOT_DATA / 2));
                let start = theta + half as f32 * d12 + per * (first as f32 - mid - (half * SECOND_FIELD) as f32);
                let mut turn = Complex32::from_polar(gain, -start);
                let inc = Complex32::from_polar(1.0, -per);
                for i in first..first + SLOT_DATA / 2 {
                    self.data
                        .push(quarter(z[i] * rot[i] * turn, 4 - (self.scr[i] & 3)));
                    turn *= inc;
                }
            }
            // The carrier follows the phase that is left over the slot; the slope inside the fields tells a carrier
            // that is off by a multiple of one turn between two fields.
            t.f += 0.05 * f64::from(d12 + d23) / (TAU * SLOT as f64);
            t.slope += f[0].1 * f[0].0.conj() + f[1].1 * f[1].0.conj();
            t.slope_slots += 1;
            if t.slope_slots >= WATCH_SLOTS {
                let one = TAU * (PILOTS / 2) as f64 / SECOND_FIELD as f64;
                let k = (f64::from(t.slope.arg()) / one).round();
                t.f += k / SECOND_FIELD as f64;
                t.slope = Complex32::default();
                t.slope_slots = 0;
            }
        } else {
            self.slots_erased += 1;
            t.erased += 1;
        }
        self.track = Some(t);
        self.sh
            .tdm_blocked
            .store(t.seen && t.erased >= 2, Ordering::Relaxed);
        let weight = if good {
            (t.signal / t.noise).min(1e4)
        } else {
            0.0
        };
        self.frame_slot(good, weight, time);
        // The grid of a slot without pilots is looked for within `GRID` symbols.
        let mut shift = 0isize;
        if !good && energy > 0.0 && self.base >= GRID {
            let z = &self.buf;
            let mut best = GRID_MIN;
            for s in (-(GRID as isize)..=GRID as isize).filter(|&s| s != 0) {
                let b = (self.base as isize + s) as usize;
                let (h0, h1, e) = self.field(0, &z[b..b + PILOTS], &self.rot[..PILOTS]);
                let q = (h0 + h1).norm_sqr() / (PILOTS as f32 * e).max(1e-20);
                if q > best {
                    (best, shift) = (q, s);
                }
            }
        }
        let next = (self.base + SLOT) as isize + shift;
        self.buf.drain(..next as usize - GRID);
        self.base = GRID;
        let rate = self.candidate.map_or(1.0, |c| c.rate);
        let lost = t.erased as f64 * SLOT as f64 / rate > FLYWHEEL_S;
        if lost || (!t.seen && t.erased >= 4) {
            self.restart();
        }
    }

    /// The correlation of the first data symbols of the slot in work with the SOF in modulation `m`, 1 for the SOF
    /// without noise.
    fn sof(&self, m: Modulation) -> f32 {
        let s = &self.sof[m.index() as usize];
        let y = &self.data[..s.len().min(self.data.len())];
        let c: f32 = y.iter().zip(s).map(|(a, b)| (a * b.conj()).re).sum();
        let e: f32 = y.iter().map(|a| a.norm_sqr()).sum();
        let es: f32 = s.iter().map(|a| a.norm_sqr()).sum();
        c / (e * es).sqrt().max(1e-20)
    }

    /// Takes the data symbols of a slot (`self.data`; none for an erased slot) with the weight of their soft values:
    /// looks for the SOF, or de-maps the slot and passes its soft bits on by their place in the frame.
    fn frame_slot(&mut self, good: bool, weight: f32, time: f64) {
        if self.frame.is_none() {
            if !good {
                return;
            }
            let found = Modulation::ALL
                .into_iter()
                .map(|m| (m, self.sof(m)))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .filter(|x| x.1 >= SOF_MIN);
            let Some((modulation, _)) = found else {
                return;
            };
            self.frame = Some(Frame {
                modulation,
                bit: 0,
                bits: None,
                layout: None,
                run: false,
                misses: 0,
                unit_n: 0,
            });
        } else if let Some((m, 0)) = self.frame.as_ref().map(|f| (f.modulation, f.bit)) {
            // The slot starts a frame: the SOF is there, or the frame counts a miss.
            let hit = !good || self.sof(m) >= SOF_MIN;
            let lost = self.frame.as_mut().is_some_and(|f| {
                f.misses = if hit { 0 } else { f.misses + 1 };
                f.misses >= SOF_MISSES
            });
            if lost {
                self.frame = None;
                self.signalling = None;
                return;
            }
        }
        let Some(m) = self.frame.as_ref().map(|f| f.modulation) else {
            return;
        };
        let bits = m.bits();
        self.soft.clear();
        if good {
            let cons = &self.cons[m.index() as usize];
            for &y in &self.data {
                if cons
                    .demap_weighted(y, weight, Demap::MaxLog, &mut self.cell[..bits])
                    .is_err()
                {
                    self.cell[..bits].fill(0.0);
                }
                for &l in &self.cell[..bits] {
                    self.soft.push((4.0 * l).round().clamp(-127.0, 127.0) as i8);
                }
            }
        } else {
            self.soft.resize(SLOT_DATA * bits, 0);
        }
        let soft = std::mem::take(&mut self.soft);
        self.frame_bits(&soft, time);
        self.soft = soft;
    }

    /// Passes on the whole units held, as one item at `time`, for the data path that they belong to.
    fn pass_units(&mut self, time: f64) {
        if let (false, Some((shape, id))) = (self.units.is_empty(), self.path) {
            let units = &self.units;
            let soft = self.pool.exact(units.len(), |v| v.copy_from_slice(units));
            self.out.push(Timed {
                time,
                value: TdmUnits {
                    soft,
                    shape,
                    id,
                    starts: self.starts,
                },
            });
        }
        self.units.clear();
    }

    /// Takes soft bits by their place in the frame: the signalling field goes to its decoder, the DATA part is cut in
    /// units of 126 for the time de-interleaver.
    fn frame_bits(&mut self, mut soft: &[i8], time: f64) {
        while !soft.is_empty() {
            let Some(f) = self.frame.as_mut() else {
                return;
            };
            let data_end = HEADER_BITS + f.layout.map_or(0, |l| l.data_bits());
            let end = f.bits.unwrap_or(usize::MAX);
            if f.bit < SOF_BITS {
                let n = soft.len().min(SOF_BITS - f.bit);
                (soft, f.bit) = (&soft[n..], f.bit + n);
            } else if f.bit < HEADER_BITS {
                let n = soft.len().min(HEADER_BITS - f.bit);
                let at = f.bit - SOF_BITS;
                self.sig_llr[at..at + n].copy_from_slice(&soft[..n]);
                (soft, f.bit) = (&soft[n..], f.bit + n);
                if f.bit == HEADER_BITS {
                    // The units so far are those of the data path before this field.
                    self.pass_units(time);
                    self.signalling_field();
                    continue;
                }
            } else if f.bit < data_end {
                let n = soft.len().min(data_end - f.bit).min(IU_BITS - f.unit_n);
                self.unit[f.unit_n..f.unit_n + n].copy_from_slice(&soft[..n]);
                (soft, f.bit, f.unit_n) = (&soft[n..], f.bit + n, f.unit_n + n);
                if f.unit_n == IU_BITS {
                    f.unit_n = 0;
                    if f.run {
                        self.units.extend_from_slice(&self.unit);
                    }
                }
            } else {
                let n = soft.len().min(end - f.bit);
                (soft, f.bit) = (&soft[n..], f.bit + n);
            }
            if f.bit >= end {
                f.bit = 0;
            }
        }
    }

    /// Decodes the signalling field of the frame in work. A field that decodes with other values than those in force
    /// starts the data path again from this frame; one that does not decode leaves the values in force, and ends a frame
    /// that has none.
    fn signalling_field(&mut self) {
        let Some(f) = self.frame.as_mut() else {
            return;
        };
        let read = self.codec.as_mut().and_then(|c| {
            c.decode(&self.sig_llr, 8, Signalling::crc_holds, &mut self.sig_bits)
                .ok()?;
            Signalling::read(&self.sig_bits)
        });
        let Some(s) = read else {
            self.signalling_failed += 1;
            if self.signalling.is_none() {
                self.frame = None;
            }
            return;
        };
        if self.signalling == Some(s) && f.bits.is_some() {
            return;
        }
        self.signalling = Some(s);
        f.unit_n = 0;
        f.run = false;
        f.layout = TdmFrame::new(s.width_cu as usize, f.modulation, s.id, s.ti);
        f.bits = f.layout.map(|l| l.bits());
        self.refused = None;
        let Some(layout) = f.layout else {
            // The frame cannot be laid out: its end is not known, so the next SOF is searched for.
            self.refused = Some(if super::tdm::n_bil(s.id).is_none() {
                Refusal::Pattern(s.id)
            } else {
                Refusal::Profile
            });
            self.frame = None;
            return;
        };
        match FrameRx::memory(&layout.shape).map(|b| b as f64 / 1e6) {
            Ok(mb) if mb <= self.max_mb => {}
            Ok(mb) => {
                self.refused = Some(Refusal::Memory(mb));
                return;
            }
            Err(_) => {
                self.refused = Some(Refusal::Profile);
                return;
            }
        }
        f.run = true;
        self.path = Some((layout.shape, s.id));
        self.starts = self.starts.wrapping_add(1);
    }

    fn push(&mut self, s: &TdmSymbols, time: f64) {
        if s.start {
            self.restart();
            self.waiting = false;
        }
        self.candidate = Some(s.candidate);
        (self.mix_hz, self.clock_ppm) = (s.mix_hz, s.clock_ppm);
        if self.waiting {
            return;
        }
        self.buf.extend_from_slice(&s.sym);
        loop {
            if self.track.is_some() {
                if self.buf.len() < self.base + SLOT + PILOTS + GRID {
                    break;
                }
                self.slot(time);
                continue;
            }
            if self.buf.len() < (SEARCH_SLOTS + 1) * SLOT {
                break;
            }
            match self.search() {
                Some((_, f)) if f.abs() > SHIFT_MIN => {
                    // The mixer of the front end takes the offset off, and the search starts on its new symbols.
                    let hz = f * s.candidate.rate;
                    self.sh.tdm_shift.store(hz.to_bits(), Ordering::Relaxed);
                    self.sh.tdm_shifts.fetch_add(1, Ordering::Relaxed);
                    self.buf.clear();
                    self.waiting = true;
                    break;
                }
                Some((o, f)) => {
                    // Symbols stay before the slot for the check of the grid.
                    let o = if o < GRID { o + SLOT } else { o };
                    self.buf.drain(..o - GRID);
                    self.base = GRID;
                    self.track = Some(Track {
                        f,
                        ..Track::default()
                    });
                }
                None => {
                    self.buf.drain(..SEARCH_SLOTS * SLOT);
                }
            }
        }
        self.pass_units(time);
    }

    fn state(&self) -> TdmState {
        let t = self.track.filter(|t| t.seen);
        let rate = self.candidate.map_or(0.0, |c| c.rate);
        TdmState {
            stage: self.stage(),
            candidate: self.candidate,
            modulation: self.frame.as_ref().map(|f| f.modulation),
            signalling: self.signalling,
            frame: self.frame.as_ref().and_then(|f| f.layout),
            refused: self.refused,
            cfo_hz: self.mix_hz + t.map_or(0.0, |t| t.f * rate),
            clock_ppm: self.clock_ppm,
            mer_db: t.map(|t| 10.0 * (t.signal / t.noise).log10()),
            starts: self.starts,
            slots: self.slots,
            slots_erased: self.slots_erased,
            signalling_failed: self.signalling_failed,
        }
    }
}

impl Component for TdmDemod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbsh.tdm",
            vec![Port::typed::<TdmSymbols>("in")],
            vec![
                Port::typed::<TdmUnits>("out").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<TdmState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(TdmSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.restart();
        self.units.clear();
        self.waiting = false;
        self.candidate = None;
        self.sh.tdm_stage.store(0, Ordering::Relaxed);
        Ok(vec![
            Format::new(Kind::typed::<TdmUnits>(), 0.0),
            Format::new(Kind::typed::<TdmState>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        self.out = std::mem::take(io.items_out::<TdmUnits>(0));
        let mut n = 0;
        for item in io.items::<TdmSymbols>(0) {
            self.now = item.time;
            self.push(&item.value, item.time);
            n += 1;
        }
        *io.items_out::<TdmUnits>(0) = std::mem::take(&mut self.out);
        self.sh.tdm_stage.store(self.stage(), Ordering::Relaxed);
        if n > 0 {
            let value = self.state();
            io.items_out::<TdmState>(1).push(Timed {
                time: self.now,
                value,
            });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TdmSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TdmSetting::Gold => {
                self.set_gold(TDM_GOLD.read(id, v)? as u32);
                self.restart();
            }
            TdmSetting::MaxInterleaverMb => self.max_mb = MAX_INTERLEAVER_MB.read(id, v)?,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (TdmSetting::Gold, Value::I64(i64::from(self.gold))),
            (TdmSetting::MaxInterleaverMb, Value::F64(self.max_mb)),
        ])
    }
}

/// `dvbsh.tdm_deint`: the time de-interleaver of the data path that `dvbsh.tdm` runs. The state of `dvbsh.tdm` passes
/// through, so that it reaches the decoder in step with the code words.
#[derive(Default)]
pub struct TdmDeint {
    /// The count of the data path in work, and its de-interleaver unless the shape was refused.
    path: Option<(u32, Option<FrameRx>)>,
    pool: SlicePool<i8>,
}

impl TdmDeint {
    /// A de-interleaver with no data path.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Component for TdmDeint {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbsh.tdm_deint",
            vec![
                Port::typed::<TdmUnits>("in"),
                Port::typed::<TdmState>("state"),
            ],
            vec![
                Port::typed::<ShWord>("out").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<TdmState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.path = None;
        Ok(vec![
            Format::new(Kind::typed::<ShWord>(), 0.0),
            Format::new(Kind::typed::<TdmState>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let mut words = std::mem::take(io.items_out::<ShWord>(0));
        for item in io.items::<TdmUnits>(0) {
            let u = &item.value;
            if self.path.as_ref().map(|p| p.0) != Some(u.starts) {
                self.path = Some((u.starts, FrameRx::new(u.shape).ok()));
            }
            let Some((_, Some(rx))) = self.path.as_mut() else {
                continue;
            };
            let pool = &mut self.pool;
            for unit in u.soft.as_chunks::<IU_BITS>().0 {
                rx.push(unit, |cw| {
                    let llr = pool.exact(cw.len(), |v| v.copy_from_slice(cw));
                    words.push(Timed {
                        time: item.time,
                        value: ShWord { llr, id: u.id },
                    });
                });
            }
        }
        *io.items_out::<ShWord>(0) = words;
        let mut state = std::mem::take(io.items_out::<TdmState>(1));
        state.extend(io.items::<TdmState>(1).iter().cloned());
        *io.items_out::<TdmState>(1) = state;
    }

    fn values(&self) -> Values {
        no_values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A demodulator whose buffer holds noise with the pilot fields of the default scrambling from symbol `at`.
    fn with_pilots(at: usize) -> TdmDemod {
        with_pilots_at(at, 1.0)
    }

    /// [`with_pilots`] with the pilot fields at the amplitude `level`.
    fn with_pilots_at(at: usize, level: f32) -> TdmDemod {
        let mut d = TdmDemod::new(Arc::new(ShLink::default()));
        let mut s = 0x2545_f491_4f6c_dd1d_u64;
        let mut noise = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 40) as f32 / (1u64 << 23) as f32 - 1.0
        };
        d.buf = (0..(SEARCH_SLOTS + 1) * SLOT)
            .map(|_| Complex32::new(noise(), noise()))
            .collect();
        for slot in 0..SEARCH_SLOTS {
            for (k, field) in d.pilots.iter().enumerate() {
                let from = at + slot * SLOT + k * SECOND_FIELD;
                for (x, p) in d.buf[from..from + PILOTS].iter_mut().zip(field) {
                    *x += p * level;
                }
            }
        }
        d
    }

    #[test]
    fn the_block_correlation_is_the_straight_sum_and_the_same_bits_in_every_form() {
        let d = with_pilots(0);
        for (i, x) in d.buf.as_chunks::<BLOCK>().0.iter().take(500).enumerate() {
            let p = &d.pilots[i % 2].as_chunks::<BLOCK>().0[i % 4];
            let (c, e) = block_corr_plain(x, p);
            let straight: Complex32 = x.iter().zip(p).map(|(a, p)| a * p.conj()).sum();
            let energy: f32 = x.iter().map(|a| a.norm_sqr()).sum();
            assert!((c - straight).norm() <= 1e-5 * (1.0 + straight.norm()), "block {i}");
            assert!((e - energy).abs() <= 1e-5 * energy, "block {i}");
            let fast = block_corr(x, p);
            assert_eq!(
                (fast.0.re.to_bits(), fast.0.im.to_bits(), fast.1.to_bits()),
                (c.re.to_bits(), c.im.to_bits(), e.to_bits()),
                "block {i}"
            );
        }
    }

    #[test]
    fn the_slot_search_finds_the_pilot_fields_in_noise() {
        let d = with_pilots(1234);
        let (o, f) = d.search().expect("slots");
        assert_eq!(o, 1234);
        assert!(f.abs() < 1e-4, "carrier {f}");
    }

    #[test]
    fn the_slot_search_finds_the_offset_of_the_highest_share_near_its_threshold() {
        for at in [1234, 7, 2100] {
            let d = with_pilots_at(at, 0.3);
            let shares: Vec<f32> = (0..SLOT).map(|o| d.share(o)).collect();
            let best = (0..SLOT).fold(0, |b, o| if shares[o] > shares[b] { o } else { b });
            assert_eq!(best, at);
            assert!((SEARCH_MIN..1.3 * SEARCH_MIN).contains(&shares[best]), "share {}", shares[best]);
            assert_eq!(d.search().map(|(o, _)| o), Some(best));
        }
    }

    #[test]
    #[ignore = "a measurement: run it alone in an optimised build"]
    fn the_time_of_a_slot_search() {
        let d = with_pilots(1234);
        let best = (0..20)
            .map(|_| {
                let t = std::time::Instant::now();
                std::hint::black_box(d.search());
                t.elapsed().as_secs_f64()
            })
            .fold(f64::MAX, f64::min);
        // A search takes the symbols of `SEARCH_SLOTS` slots.
        let symbols = (SEARCH_SLOTS * SLOT) as f64;
        eprintln!(
            "a slot search: {:.2} ms, {:.3} of a core at 3.889 Msym/s",
            best * 1e3,
            best / (symbols / 3.889e6)
        );
    }
}
