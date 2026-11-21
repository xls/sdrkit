//! OFDM synchronisation and demodulation of mode I. [`Ofdm`] takes 2.048 Msps samples in pieces of any size and outputs soft
//! bits in transmission order: the 9216 FIC bits, then the four CIFs. While searching it finds the null symbol and the PRS;
//! once locked it tracks each frame's PRS and the cyclic prefixes.

mod demod;
mod search;

pub use demod::Detection;

use crate::tables::{self, FRAME, TG, TNULL, TU};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32 as C;
use sdrkit_graph::SlicePool;
use std::sync::Arc;

/// The receiver's synchronisation state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SyncState {
    /// No frame sync.
    #[default]
    Searching,
    /// A null was found and the PRS is being searched.
    Syncing,
    /// Frames are being demodulated.
    Locked,
}

impl SyncState {
    /// The number of the state: 0, 1 or 2.
    pub fn number(self) -> u8 {
        match self {
            SyncState::Searching => 0,
            SyncState::Syncing => 1,
            SyncState::Locked => 2,
        }
    }
}

/// One demodulated transmission frame: its soft bits (FIC then the four CIFs, 230 400 values) and where it was.
#[derive(Clone, Debug)]
pub struct FrameSoft {
    /// Frames demodulated since the reset, counting from 0.
    pub frame: u64,
    /// The absolute sample index (at 2.048 Msps) of the start of the frame's null symbol.
    pub start: i64,
    /// The frame was demodulated through a fade (its PRS was not found): its soft bits are weak.
    pub faded: bool,
    /// The soft bits.
    pub soft: Arc<[i8]>,
}

/// What the demodulator measured, published after every frame (and every frame length while searching).
#[derive(Clone, Debug, Default)]
pub struct OfdmStats {
    /// The state.
    pub state: SyncState,
    /// Locked and the last frame's PRS was found.
    pub sync: bool,
    /// PRS peak-to-median ratio.
    pub prs_peak: f32,
    /// Carrier offset in Hz (without the centring shift).
    pub cfo_hz: f64,
    /// The SNR measure (section 5.6).
    pub snr_db: f32,
    /// Sample clock error in ppm.
    pub clock_ppm: f64,
    /// Coherent or differential, for the last frame.
    pub detection: Detection,
    /// Frames demodulated since the reset.
    pub frames: u64,
    /// Input searched without a null, in ms.
    pub quiet_ms: f64,
    /// Sync losses.
    pub sync_losses: u64,
    /// Sync losses recovered by the quick re-acquisition.
    pub relocks: u64,
    /// The spectrum is mirrored (I and Q swapped) and corrected.
    pub mirrored: bool,
    /// Searches that failed in a row.
    pub failed_searches: u32,
    /// The decoding state downstream must be reset (a full search after failed quick re-acquisitions).
    pub reset_decoding: bool,
    /// A quick re-acquisition is under way: downstream keeps its state.
    pub reacquiring: bool,
    /// Signal seconds (2.048 Msps samples seen / rate).
    pub time_s: f64,
    /// Time of the first lock, seconds; 0 before.
    pub lock_time_s: f64,
    /// The constellation: 1536 differential cells, radius square-rooted, unit mean radius.
    pub constellation: Option<Arc<[C]>>,
    /// The impulse response magnitude of the last PRS correlation (2048).
    pub impulse: Option<Arc<[f32]>>,
}

/// The null symbol's carrier powers of one frame (for transmitter identification).
#[derive(Clone, Debug)]
pub struct NullSpectrum {
    /// The frame number.
    pub frame: u64,
    /// Power per carrier, index k + 768 (k = −768 … 768, the entry for 0 unused), from an FFT window in the middle of the null.
    pub power: Arc<[f32]>,
    /// The mean power of the active symbols' carriers, for scale.
    pub signal_power: f32,
}

/// Settings of the demodulator.
#[derive(Clone, Copy, Debug)]
pub struct OfdmSettings {
    /// Use the channel estimate when reliable (otherwise always differential).
    pub coherent: bool,
    /// Measure the null symbols for transmitter identification.
    pub tii: bool,
}

impl Default for OfdmSettings {
    fn default() -> Self {
        OfdmSettings {
            coherent: true,
            tii: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Quick {
    tries: u32,
    shift: i32,
}

/// The mode I synchroniser and demodulator.
pub struct Ofdm {
    settings: OfdmSettings,
    fft: Fft,
    ifft: Fft,
    /// PRS, by FFT bin.
    prs: Vec<C>,
    buf: Vec<C>,
    /// Absolute index of `buf[0]`.
    buf_start: i64,
    /// Absolute index of the next sample to arrive.
    end: i64,
    mirror: bool,
    state: SyncState,
    // Search.
    failed: u32,
    wait_until: i64,
    quick: Option<Quick>,
    // Tracking.
    /// Predicted useful start of the next frame's PRS (absolute, fractional).
    pred: f64,
    /// Carrier offset in carrier spacings.
    cfo: f64,
    /// Relative sample clock error.
    eps: f64,
    first_frame: bool,
    fades: u32,
    slow_power: f64,
    slow_cell: f64,
    slow_llr: f64,
    have_slow: bool,
    last_found: Option<i64>,
    // Output.
    frames: u64,
    pool: SlicePool<i8>,
    pool_c: SlicePool<C>,
    pool_f: SlicePool<f32>,
    stats: OfdmStats,
    quiet: f64,
    since_report: i64,
    pub(crate) demod: demod::Demod,
    win: Vec<C>,
    scratch: Vec<C>,
    mags: Vec<f32>,
    powers: Vec<f32>,
    paths: Vec<(i64, f32)>,
}

/// What [`Ofdm::push`] produced.
#[derive(Debug, Default)]
pub struct OfdmOut {
    /// Demodulated frames.
    pub frames: Vec<FrameSoft>,
    /// Stats, one per frame or frame length of searching.
    pub stats: Vec<OfdmStats>,
    /// Null symbol spectra.
    pub nulls: Vec<NullSpectrum>,
}

impl Ofdm {
    /// A demodulator.
    pub fn new(settings: OfdmSettings) -> Self {
        Ofdm {
            settings,
            fft: Fft::forward(TU),
            ifft: Fft::inverse(TU),
            prs: tables::prs_bins(),
            buf: Vec::new(),
            buf_start: 0,
            end: 0,
            mirror: false,
            state: SyncState::Searching,
            failed: 0,
            wait_until: 0,
            quick: None,
            pred: 0.0,
            cfo: 0.0,
            eps: 0.0,
            first_frame: false,
            fades: 0,
            slow_power: 0.0,
            slow_cell: 0.0,
            slow_llr: 0.0,
            have_slow: false,
            last_found: None,
            frames: 0,
            pool: SlicePool::default(),
            pool_c: SlicePool::default(),
            pool_f: SlicePool::default(),
            stats: OfdmStats::default(),
            quiet: 0.0,
            since_report: 0,
            demod: demod::Demod::new(),
            win: vec![C::default(); TU],
            scratch: vec![C::default(); TU],
            mags: Vec::with_capacity(TU),
            powers: Vec::with_capacity(TU),
            paths: Vec::with_capacity(TU),
        }
    }

    /// Changes the settings (live).
    pub fn set_settings(&mut self, s: OfdmSettings) {
        self.settings = s;
    }

    /// Forgets everything: searching from scratch.
    pub fn reset(&mut self) {
        let s = self.settings;
        *self = Ofdm::new(s);
    }

    /// Drops the buffered samples and searches again (for use after the input frequency changes); keeps the counters.
    pub fn restart_search(&mut self) {
        self.buf.clear();
        self.buf_start = self.end;
        self.state = SyncState::Searching;
        self.quick = None;
        self.failed = 0;
        self.wait_until = self.end;
    }

    /// The state.
    pub fn state(&self) -> SyncState {
        self.state
    }

    /// The carrier offset in Hz (negated while mirrored, so it is the offset of the signal as received).
    pub fn cfo_hz(&self) -> f64 {
        let f = self.cfo * 1000.0;
        if self.mirror {
            -f
        } else {
            f
        }
    }

    /// Feeds samples at 2.048 Msps.
    pub fn push(&mut self, input: &[C], out: &mut OfdmOut) {
        if self.mirror {
            self.buf.extend(input.iter().map(|c| c.conj()));
        } else {
            self.buf.extend_from_slice(input);
        }
        self.end += input.len() as i64;
        self.since_report += input.len() as i64;
        loop {
            let progressed = match self.state {
                SyncState::Searching | SyncState::Syncing => self.search(out),
                SyncState::Locked => self.track(out),
            };
            if !progressed {
                break;
            }
        }
        while self.state != SyncState::Locked && self.since_report >= FRAME as i64 {
            self.since_report -= FRAME as i64;
            self.publish(out);
        }
    }

    fn publish(&mut self, out: &mut OfdmOut) {
        let s = &mut self.stats;
        s.state = self.state;
        s.cfo_hz = if self.mirror {
            -self.cfo * 1000.0
        } else {
            self.cfo * 1000.0
        };
        s.clock_ppm = self.eps * 1e6;
        s.frames = self.frames;
        s.quiet_ms = if self.state == SyncState::Locked {
            0.0
        } else {
            self.quiet * 1000.0 / tables::RATE
        };
        s.mirrored = self.mirror;
        s.failed_searches = self.failed;
        s.time_s = self.end as f64 / tables::RATE;
        out.stats.push(s.clone());
        s.reset_decoding = false;
    }

    /// Trims the buffer so it starts at absolute index `from` (never past what has arrived).
    fn trim_to(&mut self, from: i64) {
        let n = (from - self.buf_start).clamp(0, self.buf.len() as i64) as usize;
        if n > 0 {
            self.buf.drain(..n);
            self.buf_start += n as i64;
        }
    }

    /// Loads TU samples from absolute index `at` into `self.win`, derotated by the carrier offset, and transforms them. Samples
    /// outside the buffer read as 0.
    fn load_fft(&mut self, at: i64, cfo: f64) {
        load_window(&self.buf, self.buf_start, at, cfo, &mut self.win);
        self.fft.process(&mut self.win);
    }

    /// The PRS correlation of the spectrum in `self.win` at an integer carrier shift: the impulse response
    /// (complex, 2048 taps) in `self.scratch`.
    fn prs_correlate(&mut self, shift: i32) {
        for v in self.scratch.iter_mut() {
            *v = C::default();
        }
        for k in (-tables::HALF..=tables::HALF).filter(|&k| k != 0) {
            let b = tables::bin(k);
            let x = self.win[tables::bin(k + shift)];
            self.scratch[b] = x * self.prs[b].conj();
        }
        self.ifft.process(&mut self.scratch);
    }

    /// Peak index and peak-to-median ratio of the magnitudes of `self.scratch`.
    fn peak_median(&mut self) -> (usize, f32, f32) {
        self.mags.clear();
        let mut peak = 0usize;
        let mut pv = 0f32;
        for (i, v) in self.scratch.iter().enumerate() {
            let m = v.norm();
            if m > pv {
                pv = m;
                peak = i;
            }
            self.mags.push(m);
        }
        let mid = self.mags.len() / 2;
        let (_, med, _) = self.mags.select_nth_unstable_by(mid, |a, b| a.total_cmp(b));
        let med = (*med).max(1e-12);
        (peak, pv / med, pv)
    }
}

/// The active-carrier index of carrier k (0 … 1535, in order of k).
pub fn carrier_index(k: i32) -> usize {
    if k < 0 {
        (k + 768) as usize
    } else {
        (k + 767) as usize
    }
}

/// The carrier of active-carrier index i.
pub fn carrier_of(i: usize) -> i32 {
    if i < 768 {
        i as i32 - 768
    } else {
        i as i32 - 767
    }
}

/// Copies `TU` samples from absolute index `at` (samples outside `buf` read as 0), derotated by `cfo` carrier spacings with the
/// phase taken from the absolute index, so it is continuous.
fn load_window(buf: &[C], buf_start: i64, at: i64, cfo: f64, win: &mut [C]) {
    let step = -std::f64::consts::TAU * cfo / TU as f64;
    let base = (cfo * at as f64 / TU as f64).rem_euclid(1.0);
    let mut ph = -std::f64::consts::TAU * base;
    let mut rot = C::new(ph.cos() as f32, ph.sin() as f32);
    let inc = C::new(step.cos() as f32, step.sin() as f32);
    for (i, w) in win.iter_mut().enumerate() {
        if i % 64 == 0 && i > 0 {
            ph = -std::f64::consts::TAU * base + step * i as f64;
            rot = C::new(ph.cos() as f32, ph.sin() as f32);
        }
        let j = at + i as i64 - buf_start;
        *w = if j >= 0 && (j as usize) < buf.len() {
            buf[j as usize] * rot
        } else {
            C::default()
        };
        rot *= inc;
    }
}

/// The cyclic prefix correlation of the symbol whose useful part starts at absolute `u`.
fn cp_corr(buf: &[C], buf_start: i64, u: i64) -> C {
    let a = u - TG as i64 - buf_start;
    if a < 0 || (a as usize + TG + TU) > buf.len() {
        return C::default();
    }
    let a = a as usize;
    let mut s = C::default();
    for n in 0..TG {
        s += buf[a + n] * buf[a + n + TU].conj();
    }
    s
}

/// The normalised cyclic prefix correlation of the symbol whose useful part starts at absolute `u` (near 1 for a clean symbol, 0
/// for noise or a window that does not hold a symbol).
fn cp_ratio(buf: &[C], buf_start: i64, u: i64) -> f32 {
    let a = u - TG as i64 - buf_start;
    if a < 0 || (a as usize + TG + TU) > buf.len() {
        return 0.0;
    }
    let a = a as usize;
    let mut s = C::default();
    let mut e = 0f32;
    for n in 0..TG {
        let (x, y) = (buf[a + n], buf[a + n + TU]);
        s += x * y.conj();
        e += 0.5 * (x.norm_sqr() + y.norm_sqr());
    }
    if e > 0.0 {
        s.norm() / e
    } else {
        0.0
    }
}

/// The symbols of a frame whose cyclic prefix stopped matching (samples lost or inserted inside the frame): from the first of a run
/// of symbols well below the first symbols' correlation to the end of the frame.
fn cut_symbols(
    buf: &[C],
    buf_start: i64,
    u0: f64,
    k: f64,
    out: &mut [bool; crate::tables::SYMBOLS],
) {
    use crate::tables::{SYMBOLS, TS};
    let mut r = [0f32; SYMBOLS];
    for (l, v) in r.iter_mut().enumerate() {
        *v = cp_ratio(buf, buf_start, (u0 + (l * TS) as f64 * k).round() as i64);
    }
    // The baseline: the first symbols, whose timing the PRS has just confirmed.
    let baseline = r[..4].iter().copied().fold(0f32, f32::max);
    out.fill(false);
    if baseline < 0.2 {
        return;
    }
    let med = baseline;
    // A cut makes every later symbol mismatch: erase from the first of three mismatching symbols in a row.
    for l in 1..SYMBOLS {
        let bad = |i: usize| i < SYMBOLS && r[i] < 0.35 * med;
        if bad(l) && (l + 2 >= SYMBOLS || (bad(l + 1) && bad(l + 2))) {
            for o in out[l..].iter_mut() {
                *o = true;
            }
            return;
        }
    }
}

/// Wraps to (−0.5, 0.5].
fn wrap_half(x: f64) -> f64 {
    let mut y = x - x.round();
    if y <= -0.5 {
        y += 1.0;
    }
    y
}

const _: () = assert!(TNULL + TG == 3160);
