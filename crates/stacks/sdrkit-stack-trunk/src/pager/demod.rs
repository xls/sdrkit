//! `pager.demod`: the sync searchers (POCSAG at 512, 1200 and 2400 bit/s, FLEX sync 1), the lock, the symbol timing, the 2-level
//! and 4-level slicing, the POCSAG batch framing and the FLEX frame framing. It takes the channel at 48 kHz, keeps its samples
//! and its discriminator's phase in rings, and returns the code words with their bit reliabilities, the lock state and the busy
//! flag for the AFC.
//!
//! After a lock the symbols are read from the tone energies of the channel samples (`sdrkit_algo_modem::fsk::TonePair`), not from
//! the discriminator: the two tones are fitted on the 32 known symbols of the POCSAG sync word or the FLEX marker. A 4-level
//! FLEX symbol is read from four energies (a second pair at a third of the deviation) as the LLRs of its two bits
//! (`sdrkit_algo_modem::fsk::FskLlr`).
//! A fitted centre beyond about 3 kHz keeps the discriminator.
//!
//! POCSAG: a sync candidate with 3 to 5 wrong bits needs a tone correlation of at least `0.52 + 0.025 x wrong`, and the candidate
//! of the best tone correlation at any speed is locked. The first batch of a lock is kept when 10 of its 16 words are code words
//! or one bit from one. With the tones the symbol clock runs free in a batch, and the sync word of each batch sets it. FLEX keeps
//! its sync search and its clock on the discriminator.

use crate::front::{pager_phase_step, Busy};
use crate::util::Ring;
use crate::{typed_format, typed_port};
use num_complex::Complex32;
use sdrkit_algo_modem::fsk::{tone_gains, FskLlr, TonePair};
use sdrkit_core::publish::field;
use sdrkit_core::{ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{values_of, Applied, Component, ComponentInfo, Format, Io, Item, Port, Timed};
use sdrkit_proto_pager::flex::{self, Fiw, Speed as FlexSpeed, CODES};
use sdrkit_proto_pager::pocsag::{self, Rate};
use sdrkit_proto_pager::word::{self, SoftRepair};
use std::f64::consts::TAU;

/// FLEX word A's marker and word B, as the sync search reads them (a 1 is the lower frequency).
const MARKER: u32 = ((flex::MARKER as u32) << 16) | flex::B_WORD as u32;

/// The speed codes of a FLEX speed.
fn codes(speed: FlexSpeed) -> impl Iterator<Item = u16> {
    CODES.iter().filter(move |c| c.1 == speed).map(|c| c.0)
}

/// The POCSAG rate of a bit rate.
fn rate_of(baud: u16) -> Rate {
    match baud {
        512 => Rate::B512,
        2400 => Rate::B2400,
        _ => Rate::B1200,
    }
}

/// The demodulator's rate.
pub const RATE: f64 = 48000.0;

/// Hz per rad/sample at [`RATE`].
const HZ_PER_RAD: f64 = RATE / TAU;
/// The tone correlation of a sync candidate: centres from the discriminator's mean, rad/sample.
const CORR_CENTRES: [f32; 3] = [-0.09, 0.0, 0.09];
/// The tone correlation of a sync candidate: deviations, rad/sample.
const CORR_DEVIATIONS: [f32; 3] = [0.45, 0.58, 0.70];
/// A POCSAG sync candidate has at most this many wrong bits (or this many right ones, inverted).
const MAX_WRONG: u32 = 5;
/// A candidate with more wrong bits than this needs the tone correlation.
const SURE_WRONG: u32 = 2;
/// The tone fit after a lock: centre steps from the discriminator's mean (rad/sample), `-6..=6` of them.
const FIT_CENTRE_STEP: f32 = 0.03;
/// The tone fit after a lock: the least deviation and its step (rad/sample), 13 of them.
const FIT_DEVIATION: (f32, f32) = (0.30, 0.05);
/// A fitted centre farther than this from the channel centre (rad/sample, about 3 kHz) keeps the discriminator: the channel
/// filter cuts the upper tone there.
const TONE_MAX_CENTRE: f32 = 0.4;
/// The samples kept for the tone energies of the sync searches (more than 32 symbols at 512 bit/s).
const SYNC_HISTORY: usize = 4096;
/// The samples kept for the tone energies of the batches.
const BATCH_HISTORY: usize = 256;

/// What the demodulator passes to the parser.
#[derive(Clone, Debug, PartialEq)]
pub enum Decoded {
    /// POCSAG code words from batch slot `first_slot` on, with each bit's reliability.
    PocsagWords {
        /// Bit rate.
        baud: u16,
        /// The batch slot of the first word.
        first_slot: usize,
        /// The words.
        words: Vec<(u32, [f32; 32])>,
    },
    /// A POCSAG batch is complete.
    PocsagBatchEnd {
        /// Bit rate.
        baud: u16,
        /// Words valid as received in it.
        valid: u32,
    },
    /// The POCSAG transmission ended.
    PocsagEnd {
        /// Bit rate.
        baud: u16,
    },
    /// A FLEX frame: its frame information word and its data symbols.
    FlexFrame {
        /// The speed.
        speed: FlexSpeed,
        /// The frame information word.
        fiw: Fiw,
        /// The data symbols, normalised so that the outer levels are near +1 and -1 (positive is the upper frequency).
        symbols: Vec<f32>,
        /// 4-level symbols read from the tone energies: `symbols` then carries the first bit of each symbol alone (the level
        /// `-L / 3` for its LLR `L`) and this the second bit (the level `(2 + L) / 3`). Empty otherwise.
        second: Vec<f32>,
    },
}

/// Code words with the lock they came from.
#[derive(Clone, Debug, PartialEq)]
pub struct PagerWords {
    /// Signal time, s.
    pub time: f64,
    /// The carrier offset measured at the lock, Hz.
    pub offset_hz: f64,
    /// What it carries.
    pub what: Decoded,
}

impl Item for PagerWords {
    const KIND: &'static str = "pager.words";
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Pocsag(u16),
    Flex,
}

#[derive(Clone, Copy, Debug)]
struct Searcher {
    kind: Kind,
    sps: f64,
    on: bool,
}

#[derive(Clone, Copy, Debug)]
struct Hit {
    which: usize,
    until: u64,
    best: f32,
    /// The tone correlation (POCSAG).
    tone: f32,
    t: u64,
    m: f32,
    inv: bool,
    flex: Option<FlexSpeed>,
}

/// What a sync test found: the discriminator score, the mean, the polarity, the FLEX speed and the tone correlation.
#[derive(Clone, Copy, Debug)]
struct Found {
    score: f32,
    mean: f32,
    inv: bool,
    flex: Option<FlexSpeed>,
    tone: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FlexStage {
    CodeComplement,
    Skip,
    Fiw,
    Sync2,
    Data,
}

#[derive(Clone, Debug)]
enum State {
    Search,
    Hold(Hit),
    Pocsag {
        baud: u16,
        first: bool,
        in_sync: bool,
        reg: u32,
        rel: [f32; 32],
        nbits: usize,
        slot: usize,
        batch: Vec<(u32, [f32; 32])>,
        valid: u32,
        /// Words of the batch at most one bit from a code word.
        near: u32,
    },
    Flex {
        speed: FlexSpeed,
        stage: FlexStage,
        count: usize,
        reg: u32,
        rel: [f32; 32],
        fiw: Fiw,
        sync2: Vec<f32>,
    },
}

/// The paging demodulator.
pub struct PagerDemod {
    searchers: [Searcher; 4],
    phi: Ring<f64>,
    iq: Ring<Complex32>,
    last: Complex32,
    /// Replays ring samples for the sync tone correlation and the fit.
    probe: TonePair,
    /// The tones of the symbols after a lock, while `tone` is on: the two tones of 2-level FSK, the outer two of 4-level.
    pair: TonePair,
    /// The inner two tones of 4-level FLEX, at a third of the deviation.
    inner: TonePair,
    tone: bool,
    /// The end of the symbol being decided.
    t_sym: f64,
    acc: f64,
    n: u64,
    state: State,
    sps: f64,
    t: f64,
    m: f32,
    a: f32,
    inv: bool,
    offset_hz: f64,
    afc_hz: f64,
    afc_change: Option<(u64, f32)>,
    m_at: u64,
    vals: Vec<f32>,
    data: Vec<f32>,
    second: Vec<f32>,
    /// The four tone energies of each 4-level symbol of the frame, and the LLRs of their bits.
    energies: Vec<f32>,
    llrs: Vec<f32>,
    levels: FskLlr,
    unlocked_at: Option<u64>,
    last_speed: &'static str,
}

impl Default for PagerDemod {
    fn default() -> Self {
        PagerDemod {
            searchers: [
                Searcher {
                    kind: Kind::Pocsag(512),
                    sps: RATE / 512.0,
                    on: true,
                },
                Searcher {
                    kind: Kind::Pocsag(1200),
                    sps: RATE / 1200.0,
                    on: true,
                },
                Searcher {
                    kind: Kind::Pocsag(2400),
                    sps: RATE / 2400.0,
                    on: true,
                },
                Searcher {
                    kind: Kind::Flex,
                    sps: RATE / 1600.0,
                    on: true,
                },
            ],
            phi: Ring::new(8192),
            iq: Ring::new(8192),
            last: Complex32::new(0.0, 0.0),
            probe: TonePair::new(RATE, 0.0, 1.0, SYNC_HISTORY).expect("valid tone pair"),
            pair: TonePair::new(RATE, 0.0, 1.0, BATCH_HISTORY).expect("valid tone pair"),
            inner: TonePair::new(RATE, 0.0, 1.0, BATCH_HISTORY).expect("valid tone pair"),
            tone: false,
            t_sym: 0.0,
            acc: 0.0,
            n: 0,
            state: State::Search,
            sps: 1.0,
            t: 0.0,
            m: 0.0,
            a: 0.1,
            inv: false,
            offset_hz: 0.0,
            afc_hz: 0.0,
            afc_change: None,
            m_at: 0,
            vals: Vec::with_capacity(48),
            data: Vec::new(),
            second: Vec::new(),
            energies: Vec::new(),
            llrs: Vec::new(),
            // The labels of the levels -3, -1, +1, +3: the first bit is 1 for the upper two, the second for the inner two.
            levels: FskLlr::new(&[0b00, 0b01, 0b11, 0b10]).expect("four labels"),
            unlocked_at: None,
            last_speed: "",
        }
    }
}

impl PagerDemod {
    /// Turns a searcher on or off: "pocsag_512", "pocsag_1200", "pocsag_2400", "flex".
    pub fn enable(&mut self, which: Searched, on: bool) {
        self.searchers[which as usize].on = on;
    }
    /// Whether a transmission is being read.
    pub fn locked(&self) -> bool {
        !matches!(self.state, State::Search | State::Hold(_))
    }
    /// Locked now or less than 2 s ago.
    pub fn receiving(&self) -> bool {
        self.locked()
            || self
                .unlocked_at
                .is_some_and(|u| self.n.saturating_sub(u) < (2.0 * RATE) as u64)
    }
    /// The AFC's correction (added to the offsets reported). A change while a transmission is held or read moves the centre with
    /// it, so the slicer follows the shift the correction makes.
    pub fn set_afc(&mut self, hz: f64) {
        self.set_afc_at(hz, self.n);
    }
    /// The same, for a change that acts from sample `at` (it can be behind already).
    pub fn set_afc_at(&mut self, hz: f64, at: u64) {
        let d = ((hz - self.afc_hz) * TAU / RATE) as f32;
        if d != 0.0 {
            // Only a change after the centre was last measured moves it.
            if self.locked() && at > self.m_at {
                self.m -= d;
                self.set_tone_centre();
            }
            self.afc_change = Some((at, d));
        }
        self.afc_hz = hz;
    }
    /// Moves the tones to the centre `m`.
    fn set_tone_centre(&mut self) {
        let hz = f64::from(self.m) * HZ_PER_RAD;
        self.pair.set_centre(hz);
        self.inner.set_centre(hz);
    }
    /// The phase at a fractional sample position.
    #[inline]
    fn phase_at(&self, t: f64) -> f64 {
        let i = t.floor();
        let f = t - i;
        let i = i as u64;
        let a = self.phi.at(i);
        let b = self.phi.at(i + 1);
        a + f * (b - a)
    }
    /// The mean frequency (rad/sample) over one symbol ending at `t`.
    #[inline]
    fn sym(&self, t: f64, sps: f64) -> f32 {
        ((self.phase_at(t) - self.phase_at(t - sps)) / sps) as f32
    }
    /// Values of `n` symbols ending at sample `t` (index 0 the latest).
    fn symbols(&mut self, t: f64, sps: f64, n: usize) {
        self.vals.clear();
        let mut p1 = self.phase_at(t);
        for k in 0..n {
            let p0 = self.phase_at(t - (k + 1) as f64 * sps);
            self.vals.push(((p1 - p0) / sps) as f32);
            p1 = p0;
        }
    }
    /// Replays the samples of the 32 symbols of `sps` that end at sample `t`, and `margin` samples before them, through the
    /// probe at `centre` +- `dev` (rad/sample). False when the ring does not hold them.
    fn replay(&mut self, t: u64, sps: f64, centre: f32, dev: f32, margin: u64) -> bool {
        let Some(start) = t.checked_sub((32.0 * sps).ceil() as u64 + 1 + margin) else {
            return false;
        };
        if !self.iq.holds(start) || t >= self.iq.next {
            return false;
        }
        self.probe.reset();
        self.probe.set_centre(f64::from(centre) * HZ_PER_RAD);
        self.probe.set_deviation(f64::from(dev) * HZ_PER_RAD);
        for i in start..=t {
            self.probe.push(self.iq.at(i));
        }
        true
    }
    /// After a replay: the energies of the tone that symbol `k` (0 the latest) of the known word `pattern` sent and of the
    /// other tone, for a word that ends `early` samples before the end of the replay. A 1 is the lower tone, the upper when
    /// inverted.
    fn sync_energies(
        &self,
        pattern: u32,
        k: usize,
        sps: f64,
        inv: bool,
        early: usize,
    ) -> (f32, f32) {
        let back = early + (k as f64 * sps).round() as usize;
        let [eu, el] = self
            .probe
            .energies(sps.round() as usize, back)
            .unwrap_or_default();
        let one = (pattern >> k) & 1 == 1;
        if one == inv {
            (eu, el)
        } else {
            (el, eu)
        }
    }
    /// The tone correlation of a POCSAG sync that ends at `t`: the best over the centres `m + CORR_CENTRES` and the
    /// deviations `CORR_DEVIATIONS` of the mean over the 32 symbols of `(E_sent - E_other) / (E_sent + E_other)`.
    fn tone_correlation(&mut self, t: u64, sps: f64, m: f32, inv: bool) -> f32 {
        let mut best = f32::MIN;
        for dc in CORR_CENTRES {
            for dev in CORR_DEVIATIONS {
                if !self.replay(t, sps, m + dc, dev, 0) {
                    return 0.0;
                }
                let mut sum = 0.0;
                for k in 0..32 {
                    let (e, o) = self.sync_energies(pocsag::SYNC, k, sps, inv, 0);
                    if e + o > 0.0 {
                        sum += (e - o) / (e + o);
                    }
                }
                best = best.max(sum / 32.0);
            }
        }
        best
    }
    /// Tests searcher `i` at sample `t`.
    fn test(&mut self, i: usize, t: u64) -> Option<Found> {
        let s = self.searchers[i];
        let n = if s.kind == Kind::Flex { 48 } else { 32 };
        self.symbols(t as f64, s.sps, n);
        let mean = self.vals[..32].iter().sum::<f32>() / 32.0;
        let mut word = 0u32;
        for k in (0..32).rev() {
            word = (word << 1) | (self.vals[k] < mean) as u32;
        }
        let (pattern, lo, hi) = match s.kind {
            Kind::Pocsag(_) => (pocsag::SYNC, MAX_WRONG, 32 - MAX_WRONG),
            Kind::Flex => (MARKER, 3, 29),
        };
        let d = (word ^ pattern).count_ones();
        let inv = if d <= lo {
            false
        } else if d >= hi {
            true
        } else {
            return None;
        };
        let mut speed = None;
        if s.kind == Kind::Flex {
            let mut code = 0u16;
            for k in (32..48).rev() {
                code = (code << 1) | (self.vals[k] < mean) as u16;
            }
            if inv {
                code = !code;
            }
            speed = Some(FlexSpeed::from_code(code, 3)?);
        }
        let mut score = 0.0f32;
        for k in 0..32 {
            let bit = (pattern >> k) & 1 == 1;
            let dv = mean - self.vals[k];
            score += if bit { dv } else { -dv };
        }
        score /= 32.0;
        if inv {
            score = -score;
        }
        // POCSAG candidates are judged by the tone correlation instead.
        if score < 0.2 && s.kind == Kind::Flex {
            return None;
        }
        let mut tone = 0.0;
        if s.kind != Kind::Flex {
            tone = self.tone_correlation(t, s.sps, mean, inv);
            let wrong = d.min(32 - d);
            if wrong > SURE_WRONG && tone < 0.52 + 0.025 * wrong as f32 {
                return None;
            }
        }
        Some(Found {
            score,
            mean,
            inv,
            flex: speed,
            tone,
        })
    }
    /// After a lock on the known word `pattern` (the POCSAG sync word, the FLEX marker) that ends at `t`: fits the two tones
    /// on its 32 symbols (centre `m + 0.03 k`, deviation `0.30 + 0.05 j` rad/sample, the pair of the largest sum of
    /// `E_sent - E_other`), sets the per-tone gains from the mean energy of each tone on the symbols that sent it, and turns the
    /// tone decisions on unless the centre is beyond [`TONE_MAX_CENTRE`]. The inner pair has a third of the deviation and gains
    /// between those of the outer tones.
    fn fit_tones(&mut self, pattern: u32, t: u64, sps: f64, inv: bool) {
        self.tone = false;
        let m0 = self.m;
        let mut best: Option<(f32, f32, f32)> = None;
        for k in -6..=6 {
            for j in 0..13 {
                let c = m0 + FIT_CENTRE_STEP * k as f32;
                let dev = FIT_DEVIATION.0 + FIT_DEVIATION.1 * j as f32;
                if !self.replay(t, sps, c, dev, 0) {
                    return;
                }
                let sum: f32 = (0..32)
                    .map(|k| {
                        let (e, o) = self.sync_energies(pattern, k, sps, inv, 0);
                        e - o
                    })
                    .sum();
                if best.is_none_or(|b| sum > b.0) {
                    best = Some((sum, c, dev));
                }
            }
        }
        let Some((_, c, dev)) = best else { return };
        if c.abs() > TONE_MAX_CENTRE || !self.replay(t, sps, c, dev, 0) {
            return;
        }
        let (mut up, mut nu, mut low, mut nl) = (0.0, 0, 0.0, 0);
        for k in 0..32 {
            let (e, _) = self.sync_energies(pattern, k, sps, inv, 0);
            let one = (pattern >> k) & 1 == 1;
            if one == inv {
                up += e;
                nu += 1;
            } else {
                low += e;
                nl += 1;
            }
        }
        let [gu, gl] = tone_gains(up / nu.max(1) as f32, low / nl.max(1) as f32);
        self.pair.reset();
        self.pair.set_centre(f64::from(c) * HZ_PER_RAD);
        self.pair.set_deviation(f64::from(dev) * HZ_PER_RAD);
        self.pair.set_gains(gu, gl);
        self.inner.reset();
        self.inner.set_centre(f64::from(c) * HZ_PER_RAD);
        self.inner.set_deviation(f64::from(dev) * HZ_PER_RAD / 3.0);
        let (mid, slope) = (0.5 * (gu + gl), (gu - gl) / 6.0);
        self.inner.set_gains(mid + slope, mid - slope);
        let now = self.iq.next - 1;
        for i in now.saturating_sub(BATCH_HISTORY as u64 - 1)..=now {
            if self.iq.holds(i) {
                self.pair.push(self.iq.at(i));
                self.inner.push(self.iq.at(i));
            }
        }
        self.m = c;
        self.tone = true;
    }
    /// The shift of the symbol clock (samples, at most an eighth of a symbol either way) that puts the POCSAG sync word whose
    /// last symbol ends at `self.t_sym` best on the two tones: the largest sum of `E_sent - E_other` over its 32 symbols.
    fn sync_shift(&mut self) -> f64 {
        let sps = self.sps;
        let reach = (sps / 8.0).floor() as u64;
        let end = self.t_sym.round() as u64 + reach;
        let (centre, dev) = (self.m, (self.pair.deviation() / HZ_PER_RAD) as f32);
        if reach == 0 || !self.replay(end, sps, centre, dev, 2 * reach) {
            return 0.0;
        }
        let mut best = (f32::MIN, 0i64);
        for shift in -(reach as i64)..=reach as i64 {
            let early = (reach as i64 - shift) as usize;
            let sum: f32 = (0..32)
                .map(|k| {
                    let (e, o) = self.sync_energies(pocsag::SYNC, k, sps, self.inv, early);
                    e - o
                })
                .sum();
            if sum > best.0 {
                best = (sum, shift);
            }
        }
        best.1 as f64
    }
    fn lock(&mut self, h: Hit) {
        let s = self.searchers[h.which];
        self.sps = s.sps;
        self.t = h.t as f64 + s.sps;
        self.m = h.m;
        self.m_at = h.t;
        // An AFC change inside the sync's window: the window's mean is partly from before it.
        if let Some((at, d)) = self.afc_change {
            let w0 = h.t as f64 - 32.0 * s.sps;
            if at > h.t {
                self.m -= d;
            } else if (at as f64) > w0 {
                self.m -= d * ((at as f64 - w0) / (32.0 * s.sps)) as f32;
            }
        }
        self.a = h.best.max(0.1);
        self.inv = h.inv;
        self.offset_hz = h.m as f64 * RATE / TAU + self.afc_hz;
        self.tone = false;
        let pattern = match s.kind {
            Kind::Pocsag(_) => pocsag::SYNC,
            Kind::Flex => MARKER,
        };
        self.fit_tones(pattern, h.t, s.sps, h.inv);
        self.state = match s.kind {
            Kind::Pocsag(baud) => {
                self.last_speed = rate_of(baud).name();
                State::Pocsag {
                    baud,
                    first: true,
                    in_sync: false,
                    reg: 0,
                    rel: [0.0; 32],
                    nbits: 0,
                    slot: 0,
                    batch: Vec::with_capacity(16),
                    valid: 0,
                    near: 0,
                }
            }
            Kind::Flex => {
                let speed = h.flex.unwrap_or(FlexSpeed::F1600_2);
                self.last_speed = speed.name();
                State::Flex {
                    speed,
                    stage: FlexStage::CodeComplement,
                    count: 0,
                    reg: 0,
                    rel: [0.0; 32],
                    fiw: Fiw::default(),
                    sync2: Vec::with_capacity(80),
                }
            }
        };
    }
    /// The energies of the four tones of a 4-level symbol, from the lowest level to the highest (polarity applied), each over
    /// the gain of its tone.
    fn four_energies(&self, outer: [f32; 2], inner: [f32; 2]) -> [f32; 4] {
        let [go, gl] = self.pair.gains();
        let [gi, gj] = self.inner.gains();
        let e = [outer[1] / gl, inner[1] / gj, inner[0] / gi, outer[0] / go];
        if self.inv {
            [e[3], e[2], e[1], e[0]]
        } else {
            e
        }
    }
    /// The values of the 4-level symbols of a frame from their tone energies, in place of `symbols`, and the second values:
    /// the LLRs of the two bits of each symbol (`sdrkit_algo_modem::fsk::FskLlr` with the amplitude and noise of the frame),
    /// over their mean size and held to +-2, written as the levels that give them back. False when the energies give no LLR.
    fn four_level_values(&mut self, symbols: &mut [f32], second: &mut Vec<f32>) -> bool {
        let n = symbols.len();
        self.llrs.clear();
        self.llrs.resize(2 * n, 0.0);
        let Some((amplitude, noise)) = FskLlr::estimate(&self.energies, 4) else {
            return false;
        };
        if self.energies.len() != 4 * n
            || self
                .levels
                .llrs(&self.energies, amplitude, noise, &mut self.llrs)
                .is_err()
        {
            return false;
        }
        let mean = self.llrs.iter().map(|l| l.abs()).sum::<f32>() / (2 * n) as f32;
        if !(mean > 0.0 && mean.is_finite()) {
            return false;
        }
        second.clear();
        for (y, l) in symbols.iter_mut().zip(self.llrs.as_chunks::<2>().0) {
            let (a, b) = (
                (l[0] / mean).clamp(-2.0, 2.0),
                (l[1] / mean).clamp(-2.0, 2.0),
            );
            *y = -a / 3.0;
            second.push((2.0 + b) / 3.0);
        }
        true
    }
    /// The speed of the last lock.
    pub fn last_speed(&self) -> &'static str {
        self.last_speed
    }
    /// The carrier offset of the last lock, Hz.
    pub fn offset_hz(&self) -> f64 {
        self.offset_hz
    }
    fn emit(&self, what: Decoded) -> Timed<PagerWords> {
        let time = self.n as f64 / RATE;
        Timed {
            time,
            value: PagerWords {
                time,
                offset_hz: self.offset_hz,
                what,
            },
        }
    }
    /// One symbol at the clock: its value, with the early–late timing update. A POCSAG batch read from the tones keeps its
    /// clock: the sync word of each batch sets it, and the discriminator's timing error is noise at the levels the tones read.
    fn next_symbol(&mut self) -> f32 {
        let sps = self.sps;
        self.t_sym = self.t;
        let v = self.sym(self.t, sps);
        if self.tone && matches!(self.state, State::Pocsag { .. }) {
            self.t += sps;
            return v;
        }
        let q = (sps / 4.0).max(1.0);
        let e = (self.sym(self.t - q, sps) - self.m).abs();
        let l = (self.sym(self.t + q, sps) - self.m).abs();
        let adj = (0.02 * (sps / 2.0) * ((l - e) / self.a) as f64).clamp(-1.0, 1.0);
        self.t += sps + adj;
        v
    }
    fn on_symbol(&mut self, v: f32, out: &mut Vec<Timed<PagerWords>>) {
        let st = std::mem::replace(&mut self.state, State::Search);
        self.state = match st {
            State::Pocsag {
                baud,
                mut first,
                mut in_sync,
                mut reg,
                mut rel,
                mut nbits,
                mut slot,
                mut batch,
                mut valid,
                mut near,
            } => {
                let tone = if self.tone {
                    let back = (self.n as f64 - self.t_sym).round().max(0.0) as usize;
                    self.pair.value(self.sps.round() as usize, back)
                } else {
                    None
                };
                let (bit, r) = match tone {
                    // A 1 is the lower tone.
                    Some(x) => ((x < 0.0) != self.inv, x.abs()),
                    None => ((v < self.m) != self.inv, (v - self.m).abs() / self.a),
                };
                // The centre follows the discriminator: the tone value does not carry the offset.
                self.m += 0.004 * (v - self.m);
                if self.tone {
                    self.set_tone_centre();
                }
                reg = (reg << 1) | bit as u32;
                rel[nbits] = r;
                nbits += 1;
                let mut next = None;
                if nbits == 32 {
                    nbits = 0;
                    if in_sync {
                        if (reg ^ pocsag::SYNC).count_ones() <= 4 {
                            if self.tone {
                                self.t += self.sync_shift();
                            }
                            in_sync = false;
                            slot = 0;
                            valid = 0;
                            near = 0;
                        } else {
                            out.push(self.emit(Decoded::PocsagEnd { baud }));
                            next = Some(State::Search);
                        }
                    } else {
                        if word::is_valid(reg) {
                            valid += 1;
                        }
                        if word::repair(reg).is_some_and(|(_, bits)| bits <= 1) {
                            near += 1;
                        }
                        if first {
                            batch.push((reg, rel));
                        } else {
                            out.push(self.emit(Decoded::PocsagWords {
                                baud,
                                first_slot: slot,
                                words: vec![(reg, rel)],
                            }));
                        }
                        slot += 1;
                        if slot == 16 {
                            if first {
                                // The first batch: most words must be code words, or one bit from one.
                                if near >= 10 {
                                    out.push(self.emit(Decoded::PocsagWords {
                                        baud,
                                        first_slot: 0,
                                        words: std::mem::take(&mut batch),
                                    }));
                                } else {
                                    batch.clear();
                                    next = Some(State::Search);
                                }
                                first = false;
                            }
                            if next.is_none() {
                                out.push(self.emit(Decoded::PocsagBatchEnd { baud, valid }));
                                if valid == 0 {
                                    out.push(self.emit(Decoded::PocsagEnd { baud }));
                                    next = Some(State::Search);
                                }
                            }
                            in_sync = true;
                        }
                    }
                    reg = 0;
                }
                match next {
                    Some(s) => {
                        self.unlocked_at = Some(self.n);
                        self.tone = false;
                        s
                    }
                    None => State::Pocsag {
                        baud,
                        first,
                        in_sync,
                        reg,
                        rel,
                        nbits,
                        slot,
                        batch,
                        valid,
                        near,
                    },
                }
            }
            State::Flex {
                speed,
                mut stage,
                mut count,
                mut reg,
                mut rel,
                mut fiw,
                mut sync2,
            } => {
                let mut done = false;
                // With the tones, the value of a 2-level symbol is the tone value on the scale of the discriminator.
                let back = (self.n as f64 - self.t_sym).round().max(0.0) as usize;
                let len = self.sps.round() as usize;
                let toned = self.tone;
                let v = match self.pair.value(len, back) {
                    Some(x) if toned => self.m + self.a * x,
                    _ => v,
                };
                match stage {
                    FlexStage::CodeComplement => {
                        // The values are kept: with the 16 after them they recentre the slicer (an AFC step can land in the
                        // marker that the lock was measured on).
                        if count == 0 {
                            sync2.clear();
                        }
                        sync2.push(v);
                        count += 1;
                        if count == 16 {
                            let thr = if toned {
                                self.m
                            } else {
                                sync2.iter().sum::<f32>() / 16.0
                            };
                            let code = sync2
                                .iter()
                                .fold(0u16, |a, &x| (a << 1) | ((x < thr) != self.inv) as u16);
                            match codes(speed).find(|&c| (code ^ !c).count_ones() <= 4) {
                                Some(c) => {
                                    reg = (!c) as u32;
                                    stage = FlexStage::Skip;
                                    count = 0;
                                }
                                None => done = true,
                            }
                        }
                    }
                    FlexStage::Skip => {
                        sync2.push(v);
                        count += 1;
                        if count == 16 {
                            // Known bits: the code complement and the complement of the marker's first half (1 = lower).
                            let pattern = (reg << 16) | (!(MARKER >> 16) & 0xFFFF);
                            let ones = pattern.count_ones() as f32;
                            let bias = (ones - (32.0 - ones)) / 32.0;
                            let mean = sync2.iter().sum::<f32>() / 32.0;
                            let sign = if self.inv { -1.0 } else { 1.0 };
                            let m = mean + sign * self.a * bias;
                            // The tone fit centred already, and a tone value does not carry the offset.
                            if !toned && (m - self.m).abs() < 1.2 * self.a {
                                self.m = m;
                                self.m_at = self.n;
                            }
                            stage = FlexStage::Fiw;
                            count = 0;
                            reg = 0;
                        }
                    }
                    FlexStage::Fiw => {
                        let bit = (v > self.m) != self.inv;
                        reg = (reg << 1) | bit as u32;
                        rel[count] = (v - self.m).abs() / self.a;
                        count += 1;
                        if count == 32 {
                            match Fiw::from_word(reg, Some(&rel), SoftRepair::HEADER) {
                                Some(f) => {
                                    fiw = f;
                                    stage = FlexStage::Sync2;
                                    count = 0;
                                    // The data rate from the next symbol on.
                                    let sps = RATE / speed.baud() as f64;
                                    self.t += sps - self.sps;
                                    self.sps = sps;
                                    sync2.clear();
                                }
                                None => done = true,
                            }
                        }
                    }
                    FlexStage::Sync2 => {
                        count += 1;
                        if count > 6 {
                            sync2.push(v);
                        }
                        let n = speed.sync2_symbols();
                        if count == n {
                            if !sync2.is_empty() && !toned {
                                let c = sync2.iter().sum::<f32>() / sync2.len() as f32;
                                if (c - self.m).abs() < 0.35 * self.a {
                                    let amp = sync2.iter().map(|x| (x - self.m).abs()).sum::<f32>()
                                        / sync2.len() as f32;
                                    self.m = c;
                                    self.m_at = self.n;
                                    if amp > 0.6 * self.a && amp < 1.5 * self.a {
                                        self.a = amp;
                                    }
                                }
                            }
                            stage = FlexStage::Data;
                            count = 0;
                            self.data.clear();
                            self.second.clear();
                            self.energies.clear();
                        }
                    }
                    FlexStage::Data => {
                        let four = toned && speed.levels() == 4;
                        let mut y = (v - self.m) / self.a;
                        if self.inv {
                            y = -y;
                        }
                        self.data.push(y);
                        if four {
                            let energies = self
                                .pair
                                .energies(len, back)
                                .zip(self.inner.energies(len, back));
                            if let Some((outer, inner)) = energies {
                                let e = self.four_energies(outer, inner);
                                self.energies.extend_from_slice(&e);
                            }
                        }
                        count += 1;
                        if count == speed.data_symbols() {
                            let mut symbols = std::mem::take(&mut self.data);
                            let mut second = std::mem::take(&mut self.second);
                            if !(four && self.four_level_values(&mut symbols, &mut second)) {
                                second.clear();
                            }
                            out.push(self.emit(Decoded::FlexFrame {
                                speed,
                                fiw,
                                symbols,
                                second,
                            }));
                            done = true;
                        }
                    }
                }
                if done {
                    self.unlocked_at = Some(self.n);
                    self.tone = false;
                    State::Search
                } else {
                    State::Flex {
                        speed,
                        stage,
                        count,
                        reg,
                        rel,
                        fiw,
                        sync2,
                    }
                }
            }
            s => s,
        };
    }
    /// Takes channel samples at 48 kHz, appends the code words read.
    pub fn run(&mut self, x: &[Complex32], out: &mut Vec<Timed<PagerWords>>) {
        for &z in x {
            let d = pager_phase_step(z, self.last);
            self.last = z;
            self.acc += d as f64;
            self.phi.push(self.acc);
            self.iq.push(z);
            if self.tone {
                self.pair.push(z);
                self.inner.push(z);
            }
            let n = self.n;
            match self.state {
                State::Search => {
                    if n >= 4000 {
                        for i in 0..4 {
                            if !self.searchers[i].on {
                                continue;
                            }
                            if let Some(f) = self.test(i, n) {
                                let s = self.searchers[i];
                                self.state = State::Hold(Hit {
                                    which: i,
                                    until: n + (0.6 * s.sps).ceil() as u64,
                                    best: f.score,
                                    tone: f.tone,
                                    t: n,
                                    m: f.mean,
                                    inv: f.inv,
                                    flex: f.flex,
                                });
                                break;
                            }
                        }
                    }
                }
                State::Hold(mut h) => {
                    if self.searchers[h.which].kind == Kind::Flex {
                        // FLEX: the best discriminator score of the same polarity.
                        if let Some(f) = self.test(h.which, n) {
                            if f.score > h.best && f.inv == h.inv {
                                h.best = f.score;
                                h.t = n;
                                h.m = f.mean;
                                if f.flex.is_some() {
                                    h.flex = f.flex;
                                }
                            }
                        }
                    } else {
                        // POCSAG: the candidate of the best tone correlation at any speed.
                        for i in 0..3 {
                            if !self.searchers[i].on {
                                continue;
                            }
                            let Some(f) = self.test(i, n) else { continue };
                            if f.tone > h.tone && (i != h.which || f.inv == h.inv) {
                                let sps = self.searchers[i].sps;
                                h.until = h.until.max(n + (0.6 * sps).ceil() as u64);
                                h.which = i;
                                h.best = f.score;
                                h.tone = f.tone;
                                h.t = n;
                                h.m = f.mean;
                                h.inv = f.inv;
                            }
                        }
                    }
                    if n >= h.until {
                        self.lock(h);
                    } else {
                        self.state = State::Hold(h);
                    }
                }
                _ => {
                    while self.locked() && self.t + (self.sps / 4.0).max(1.0) + 1.0 <= n as f64 {
                        let v = self.next_symbol();
                        self.on_symbol(v, out);
                    }
                }
            }
            self.n += 1;
        }
    }
}

/// What a searcher of the demodulator looks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Searched {
    /// POCSAG at 512 bit/s.
    Pocsag512 = 0,
    /// POCSAG at 1200 bit/s.
    Pocsag1200 = 1,
    /// POCSAG at 2400 bit/s.
    Pocsag2400 = 2,
    /// FLEX.
    Flex = 3,
}

/// Look for POCSAG syncs.
pub const POCSAG: Setting<field::Bool> =
    Setting::new("pocsag", "POCSAG", "", true).help("Look for POCSAG syncs.");
/// Look for FLEX syncs.
pub const FLEX: Setting<field::Bool> =
    Setting::new("flex", "FLEX", "", true).help("Look for FLEX syncs.");
/// Look for POCSAG at 512 bit/s.
pub const POCSAG_512: Setting<field::Bool> = Setting::new("pocsag_512", "POCSAG 512", "", true);
/// Look for POCSAG at 1200 bit/s.
pub const POCSAG_1200: Setting<field::Bool> = Setting::new("pocsag_1200", "POCSAG 1200", "", true);
/// Look for POCSAG at 2400 bit/s.
pub const POCSAG_2400: Setting<field::Bool> = Setting::new("pocsag_2400", "POCSAG 2400", "", true)
    .help("Turn off 2400 when only a slower speed is on air, to avoid false decodes.");

sdrkit_core::settings! {
    /// The settings of `pager.demod`.
    pub enum DemodSetting {
        /// Look for POCSAG syncs.
        Pocsag = POCSAG,
        /// Look for FLEX syncs.
        Flex = FLEX,
        /// POCSAG at 512 bit/s.
        Pocsag512 = POCSAG_512,
        /// POCSAG at 1200 bit/s.
        Pocsag1200 = POCSAG_1200,
        /// POCSAG at 2400 bit/s.
        Pocsag2400 = POCSAG_2400,
    }
}

/// The component.
pub struct PagerDemodComponent {
    demod: PagerDemod,
    pocsag: bool,
    rates: [bool; 3],
    flex: bool,
}

impl Default for PagerDemodComponent {
    fn default() -> Self {
        PagerDemodComponent {
            demod: PagerDemod::default(),
            pocsag: true,
            rates: [true; 3],
            flex: true,
        }
    }
}

impl PagerDemodComponent {
    fn enable(&mut self) {
        self.demod
            .enable(Searched::Pocsag512, self.pocsag && self.rates[0]);
        self.demod
            .enable(Searched::Pocsag1200, self.pocsag && self.rates[1]);
        self.demod
            .enable(Searched::Pocsag2400, self.pocsag && self.rates[2]);
        self.demod.enable(Searched::Flex, self.flex);
    }
}

/// The lock state, reported by the demodulator every pass.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PagerLock {
    /// Locked now.
    pub locked: bool,
    /// Locked now or less than 2 s ago.
    pub receiving: bool,
    /// The speed of the last lock.
    pub speed: &'static str,
    /// The carrier offset at the last lock, Hz.
    pub offset_hz: f64,
}

impl Item for PagerLock {
    const KIND: &'static str = "pager.lock";
}

impl Component for PagerDemodComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "pager.demod",
            vec![
                Port::iq("in"),
                Port::typed::<crate::front::CarrierState>("carrier"),
                Port::typed::<crate::front::AfcChange>("afc"),
            ],
            vec![
                typed_port::<PagerWords>("words"),
                typed_port::<Busy>("busy"),
                typed_port::<PagerLock>("lock"),
            ],
        )
        .with_settings(DemodSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.demod = PagerDemod::default();
        self.enable();
        Ok(vec![
            typed_format::<PagerWords>(&inputs[0]),
            typed_format::<Busy>(&inputs[0]),
            typed_format::<PagerLock>(&inputs[0]),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let first = io.input(0).first();
        let mut out = std::mem::take(io.items_out::<PagerWords>(0));
        // Apply each AFC change at the sample it acts from.
        let mut done = 0usize;
        for c in io.items::<crate::front::AfcChange>(2) {
            let rel = c.value.at.saturating_sub(first) as usize;
            let at = rel.clamp(done, x.len());
            self.demod.run(&x[done..at], &mut out);
            done = at;
            self.demod.set_afc_at(c.value.hz, c.value.at);
        }
        self.demod.run(&x[done..], &mut out);
        *io.items_out::<PagerWords>(0) = out;
        let t = io.input(0).end() as f64 / RATE;
        let locked = self.demod.locked();
        io.items_out::<Busy>(1).push(Timed {
            time: t,
            value: Busy(locked),
        });
        let st = PagerLock {
            locked,
            receiving: self.demod.receiving(),
            speed: self.demod.last_speed(),
            offset_hz: self.demod.offset_hz(),
        };
        io.items_out::<PagerLock>(2)
            .push(Timed { time: t, value: st });
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DemodSetting::Pocsag => self.pocsag = POCSAG.read(id, v)?,
            DemodSetting::Flex => self.flex = FLEX.read(id, v)?,
            DemodSetting::Pocsag512 => self.rates[0] = POCSAG_512.read(id, v)?,
            DemodSetting::Pocsag1200 => self.rates[1] = POCSAG_1200.read(id, v)?,
            DemodSetting::Pocsag2400 => self.rates[2] = POCSAG_2400.read(id, v)?,
        }
        self.enable();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (DemodSetting::Pocsag, Value::Bool(self.pocsag)),
            (DemodSetting::Flex, Value::Bool(self.flex)),
            (DemodSetting::Pocsag512, Value::Bool(self.rates[0])),
            (DemodSetting::Pocsag1200, Value::Bool(self.rates[1])),
            (DemodSetting::Pocsag2400, Value::Bool(self.rates[2])),
        ])
    }
}
