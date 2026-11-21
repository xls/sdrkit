//! The satellite front end: the spectrum and carrier survey (several carriers, the one decoded, its
//! symbol rate from the width and the cyclic line, the roll-off), then the mixer with DC removal, half-band decimators, the matched
//! filter with Gardner timing of `sdrkit-algo-modem` at 1 output per symbol (the clock steered once per run of symbols), and the
//! rate dither. Input samples are kept in a
//! ring, so symbols start from the samples the survey looked at.

use crate::receive::{run_timing, RunTiming};
use crate::spectrum::{measure_carrier, quantile, Psd};
use sdrkit_algo_dsp::decim::HalfBand;
use sdrkit_algo_dsp::interp::{Polyphase, Reader};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::spectrum::{bridge_dc, cyclic_lines};
use sdrkit_core::Complex32;
use sdrkit_core::ConfigError;

/// Samples kept for the cyclic line and to start the symbols from.
pub const RING: usize = 1 << 18;
/// Input samples of each piece of a replay.
const REPLAY_PIECE: usize = 8192;
/// Survey segments averaged before a search.
pub const SEGMENTS: usize = 48;
/// The dither of the nominal rate after failed hunts.
pub const DITHER: [f64; 7] = [0.012, -0.012, 0.024, -0.024, 0.036, -0.036, 0.0];

/// The noise bandwidth of the timing loop once locked, per symbol.
const TRACK_BN_T: f64 = 0.002;
/// Symbols that the mean of the loop's rate offset runs over.
const PPM_SYMBOLS: f64 = 20_000.0;
/// Symbols between two corrections of the symbol clock once locked: the correction is late by this many symbols at most,
/// which the loop does not feel at this bandwidth.
const CLOCK_RUN: usize = 4;

/// How the timing loop searches the symbol clock before its lock test holds: its noise bandwidth per symbol, and the
/// range of the clock about the nominal rate, ppm.
#[derive(Clone, Copy, Debug)]
struct ClockSearch {
    pull_in: f64,
    max_ppm: f64,
}

/// For a symbol rate set by hand or taken from the width of the carrier, which can be some percent off.
const WIDE: ClockSearch = ClockSearch {
    pull_in: 0.04,
    max_ppm: 40_000.0,
};
/// The noise bandwidth of the timing loop per symbol before its lock test holds, for a symbol rate taken from the cyclic
/// line. With the wide search the loop wanders off the rate at a low Es/N0, and at a small roll-off its noise closes the
/// eye of a constellation with rings, whose lock test does not hold before frames are followed.
const NEAR_PULL_IN: f64 = 0.004;
/// The limits of the range of the clock about a rate taken from the cyclic line, ppm.
const NEAR_PPM: (f64, f64) = (150.0, 1_000.0);

/// The limits of the measured width of a carrier over its symbol rate. The width is taken at half power from a smoothed
/// spectrum, so it is not exact; the smoothing also widens a narrow carrier ([`WIDTH_SMOOTH`]).
const WIDTH_FIT: (f64, f64) = (0.7, 1.3);
/// The width that the smoothing of the survey can add to a carrier, as a share of the sample rate: 1.5 times the 1/256 of
/// the spectrum that the carrier measurement averages over.
const WIDTH_SMOOTH: f64 = 1.5 / 256.0;
/// The width of what is left of a blanked carrier is under this share of the width measured for that carrier.
const REST_SHARE: f64 = 0.25;
/// The share of the sample rate above which the cyclic line search works on samples interpolated by 2.
const FOLD_SHARE: f64 = 0.45;

/// The least plateau of a carrier over the noise floor, dB. The carrier of the lowest QPSK code rates stands this low:
/// its Es/N0 is about 0.5 dB over the figure measured here.
const WEAK_SNR_DB: f64 = -4.0;
/// Under this plateau over the noise floor, dB, a hump of the spectrum is taken as a carrier only if it has a cyclic
/// line.
const CLEAR_SNR_DB: f64 = 3.0;

/// The share of the difference to a sample that the DC estimate moves by.
const DC_RATE: f32 = 1e-4;
/// Samples that share one DC estimate.
const DC_RUN: usize = 8;

/// The sum of the squared magnitudes of `x`.
fn energy(x: &[Complex32]) -> f64 {
    let mut total = 0f64;
    for part in x.chunks(1024) {
        let (eights, rest) = part.as_chunks::<8>();
        let mut acc = [0f32; 8];
        for e in eights {
            for k in 0..8 {
                acc[k] += e[k].norm_sqr();
            }
        }
        let rest: f32 = rest.iter().map(|v| v.norm_sqr()).sum();
        total += f64::from(acc.iter().sum::<f32>() + rest);
    }
    total
}

/// The DC of a signal, followed and taken off. The estimate is held over [`DC_RUN`] samples, then moves towards their mean by
/// [`DC_RATE`] per sample. The runs are counted over the stream, so the result does not depend on how the samples are cut
/// into calls.
#[derive(Clone, Copy, Debug, Default)]
struct Dc {
    level: Complex32,
    /// The sum of the samples of the run in work.
    sum: Complex32,
    /// The samples of the run in work.
    held: usize,
}

impl Dc {
    /// Takes the DC off `x` in place.
    fn remove(&mut self, mut x: &mut [Complex32]) {
        // The rest of a run that an earlier call began.
        if self.held > 0 {
            let k = (DC_RUN - self.held).min(x.len());
            let (head, rest) = std::mem::take(&mut x).split_at_mut(k);
            self.part(head);
            if self.held < DC_RUN {
                return;
            }
            self.step();
            x = rest;
        }
        let (runs, rest) = x.as_chunks_mut::<DC_RUN>();
        for run in runs {
            let mut sum = Complex32::new(0.0, 0.0);
            for v in run.iter_mut() {
                sum += *v;
                *v -= self.level;
            }
            self.sum = sum;
            self.step();
        }
        self.part(rest);
    }

    /// Takes the DC off a part of a run.
    fn part(&mut self, x: &mut [Complex32]) {
        for v in x.iter_mut() {
            self.sum += *v;
            *v -= self.level;
        }
        self.held += x.len();
    }

    /// Ends a run.
    fn step(&mut self) {
        self.level += (self.sum - self.level * DC_RUN as f32) * DC_RATE;
        self.sum = Complex32::new(0.0, 0.0);
        self.held = 0;
    }
}

/// One carrier of the survey.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Carrier {
    /// Offset from the centre, Hz.
    pub offset_hz: f64,
    /// Symbol rate (-3 dB width), Hz.
    pub rate_hz: f64,
    /// Roll-off (0 when not measured).
    pub rolloff: f64,
    /// Plateau over the noise floor, dB.
    pub snr_db: f64,
    /// Level relative to the strongest, dB.
    pub level_db: f64,
    /// Fit misfit.
    pub misfit: f64,
    /// -3 dB edges.
    pub lo_hz: f64,
    /// Upper edge.
    pub hi_hz: f64,
    /// Being decoded.
    pub decoded: bool,
}

/// What became of a carrier that the survey tried to take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tried {
    /// The chain runs on it.
    Taken,
    /// Its symbol rate is not one the input allows.
    Refused,
    /// It is near the noise floor and has no cyclic line: it is no carrier, and the next one is tried.
    NoLine,
}

/// What the survey decided.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Acquisition {
    /// The carrier.
    pub carrier: Carrier,
    /// The symbol rate used (manual, cyclic line or width).
    pub rate_hz: f64,
    /// Rate from the cyclic line, when found: the symbol clock starts from it.
    pub cyclic_hz: Option<f64>,
    /// Roll-off for the matched filter and its source (0 assumed, 1 measured, 2 signalled).
    pub rolloff: f64,
    /// Source of the roll-off.
    pub rolloff_source: u8,
}

/// Front-end settings.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrontConfig {
    /// Manual symbol rate, Hz (0 auto).
    pub manual_rs: f64,
    /// Roll-off setting (0 auto).
    pub rolloff: f64,
    /// Picked carrier offset, Hz.
    pub pick_hz: Option<f64>,
}

/// The survey and the front end.
pub struct Front {
    fs: f64,
    /// Settings.
    pub cfg: FrontConfig,
    psd: Psd,
    ring: Vec<Complex32>,
    rpos: usize,
    rfill: usize,
    /// The carriers found by the last survey.
    pub carriers: Vec<Carrier>,
    /// The input spectrum (512 points, dB, 0 at the peak).
    pub spectrum: Vec<f32>,
    /// The acquisition in use.
    pub acq: Option<Acquisition>,
    /// Log lines.
    pub log: Vec<String>,
    /// Signal time with no carrier, seconds.
    pub no_carrier_s: f64,
    // Running chain.
    mix: Nco,
    dc: Dc,
    halves: Vec<HalfBand<Complex32>>,
    sync: Option<RunTiming>,
    /// The loop's rate offset, ppm; its mean once `ppm_held`.
    ppm: f64,
    ppm_held: bool,
    rate_nominal: f64,
    dither: Option<usize>,
    tmp: Vec<Complex32>,
    tmp2: Vec<Complex32>,
    /// The front end (re)started: the symbols that follow are a new stream.
    pub restarted: bool,
    /// Symbols since the front end started.
    pub symbols: u64,
    /// Level of the input, dBFS.
    pub level_dbfs: f64,
    replay: Vec<Complex32>,
}

impl Front {
    /// For input at `fs`.
    ///
    /// # Errors
    ///
    /// [`ConfigError::NoRate`] for a rate that is not positive and finite.
    pub fn new(fs: f64, cfg: FrontConfig) -> Result<Front, ConfigError> {
        if !(fs > 0.0 && fs.is_finite()) {
            return Err(ConfigError::NoRate);
        }
        let hop = 4096usize.max((0.004 * fs) as usize);
        Ok(Front {
            fs,
            cfg,
            psd: Psd::new(4096, hop),
            ring: vec![Complex32::new(0.0, 0.0); RING],
            rpos: 0,
            rfill: 0,
            carriers: Vec::new(),
            spectrum: Vec::new(),
            acq: None,
            log: Vec::new(),
            no_carrier_s: 0.0,
            mix: Nco::new(fs.max(1.0), 0.0),
            dc: Dc::default(),
            halves: Vec::new(),
            sync: None,
            ppm: 0.0,
            ppm_held: false,
            rate_nominal: 0.0,
            dither: None,
            tmp: Vec::new(),
            tmp2: Vec::new(),
            restarted: false,
            symbols: 0,
            level_dbfs: -100.0,
            replay: Vec::new(),
        })
    }
    /// The highest symbol rate the input allows.
    pub fn max_rate(&self) -> f64 {
        self.fs / (1.3 * 1.35)
    }
    /// The lowest automatic symbol rate.
    pub fn min_auto_rate(&self) -> f64 {
        (self.fs * 40.0 / 4096.0 * 2.5).max(60e3)
    }
    /// Forgets the acquisition: the next survey starts a new one.
    pub fn restart(&mut self) {
        self.acq = None;
        self.sync = None;
        self.psd.clear();
        self.dither = None;
    }
    /// Starts the chain again at the nominal rate times the next dither step (after failed hunts). Returns false after a full
    /// round.
    pub fn next_dither(&mut self) -> bool {
        let i = self.dither.map_or(0, |d| d + 1);
        if i >= DITHER.len() {
            self.dither = None;
            return false;
        }
        self.dither = Some(i);
        if let Some(a) = self.acq {
            self.start_chain(
                a.carrier.offset_hz,
                a.rate_hz * (1.0 + DITHER[i]),
                a.rolloff,
                WIDE,
                false,
            );
        }
        true
    }
    /// The symbol rate the loop runs at now: the nominal rate with the loop's offset ([`Front::loop_ppm`]).
    pub fn rate_now(&self) -> f64 {
        match &self.sync {
            Some(_) => self.rate_nominal / (1.0 + self.ppm * 1e-6),
            None => 0.0,
        }
    }
    /// The timing loop's rate offset, ppm (positive: symbols longer than nominal). Once the loop tracks it is the mean
    /// over about [`PPM_SYMBOLS`] symbols, since the loop's own value moves with the noise.
    pub fn loop_ppm(&self) -> f64 {
        match &self.sync {
            Some(_) => self.ppm,
            None => 0.0,
        }
    }
    /// Says whether the stage after the front end follows frames or packets. While it does, the timing loop stays in its
    /// tracking state: the frames say more about the clock than the eye of the symbols.
    pub fn hold_timing(&mut self, on: bool) {
        if let Some(s) = self.sync.as_mut() {
            s.hold(on);
        }
    }
    /// The nominal rate.
    pub fn nominal(&self) -> f64 {
        self.rate_nominal
    }

    /// Starts the mixer, the decimators and the timing loop for a carrier at `centre` Hz of `rate` symbols per second
    /// and roll-off `alpha`; `search` is the pull-in bandwidth and the range of the timing loop. With `replay`, the
    /// samples of the ring run through the new chain first.
    fn start_chain(&mut self, centre: f64, rate: f64, alpha: f64, search: ClockSearch, replay: bool) {
        self.mix = Nco::new(self.fs, -centre);
        self.halves.clear();
        let mut r = self.fs;
        while r / 2.0 >= 2.4 * rate && self.halves.len() < 12 {
            let pass = (rate * (1.0 + alpha) / 2.0).min(0.2 * r);
            self.halves
                .push(HalfBand::design(r, pass, 70.0).expect("the filter settings are valid"));
            r /= 2.0;
        }
        self.sync = match run_timing::<64>(r / rate, alpha, search.pull_in, TRACK_BN_T, search.max_ppm, CLOCK_RUN) {
            Ok(s) => Some(s),
            Err(e) => {
                self.log
                    .push(format!("the symbol timing cannot start: {e}"));
                None
            }
        };
        self.rate_nominal = rate;
        self.restarted = true;
        self.symbols = 0;
        self.ppm = 0.0;
        self.ppm_held = false;
        if replay {
            // Run the ring's samples (oldest first) through the new chain.
            let n = self.rfill;
            let start = (self.rpos + RING - n) % RING;
            let mut v = Vec::with_capacity(n);
            for i in 0..n {
                v.push(self.ring[(start + i) % RING]);
            }
            self.replay = v;
        }
    }

    /// Runs input samples; appends symbols (unit mean power) to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        // Ring and level.
        let newest = &x[x.len().saturating_sub(RING)..];
        let (a, b) = newest.split_at(newest.len().min(RING - self.rpos));
        self.ring[self.rpos..self.rpos + a.len()].copy_from_slice(a);
        self.ring[..b.len()].copy_from_slice(b);
        self.rpos = (self.rpos + newest.len()) % RING;
        if !x.is_empty() {
            self.level_dbfs = sdrkit_algo_dsp::meter::power_to_db(energy(x) / x.len() as f64);
        }
        self.rfill = (self.rfill + x.len()).min(RING);
        self.psd.push(x);
        if self.psd.count >= SEGMENTS {
            self.survey();
            self.psd.clear();
            if self.acq.is_none() {
                self.no_carrier_s += SEGMENTS as f64 * 4096f64.max(0.004 * self.fs) / self.fs;
            }
        }
        if !self.replay.is_empty() {
            // In pieces, so the replay runs as live samples do.
            let r = std::mem::take(&mut self.replay);
            for piece in r.chunks(REPLAY_PIECE) {
                self.chain(piece, out);
            }
        } else if self.sync.is_some() {
            self.chain(x, out);
        }
    }

    fn chain(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        self.tmp.clear();
        self.tmp.extend_from_slice(x);
        self.dc.remove(&mut self.tmp);
        self.mix.mix_in_place(&mut self.tmp);
        for h in self.halves.iter_mut() {
            self.tmp2.clear();
            h.process(&self.tmp, &mut self.tmp2);
            std::mem::swap(&mut self.tmp, &mut self.tmp2);
        }
        let Some(s) = self.sync.as_mut() else {
            return;
        };
        let before = out.len();
        s.process(&self.tmp, out);
        let n = (out.len() - before) as u64;
        self.symbols += n;
        let ppm = s.clock_ppm();
        if s.tracking() && self.ppm_held {
            self.ppm += (n as f64 / PPM_SYMBOLS).min(1.0) * (ppm - self.ppm);
        } else {
            self.ppm = ppm;
            self.ppm_held = s.tracking();
        }
    }

    /// Whether a carrier of the measured width `width_hz` can be the carrier of the symbol rate set by hand. True when no
    /// rate is set. The width of a carrier is its symbol rate within [`WIDTH_FIT`], and up to [`WIDTH_SMOOTH`] more.
    fn fits_manual_rate(&self, width_hz: f64) -> bool {
        let m = self.cfg.manual_rs;
        m <= 0.0 || (width_hz >= WIDTH_FIT.0 * m && width_hz <= WIDTH_FIT.1 * m + WIDTH_SMOOTH * self.fs)
    }

    /// The points of the spectra of the cyclic line search per sample rate of the input: a point is the sample rate
    /// over this number wide.
    fn cyclic_points(&self) -> usize {
        if self.rfill >= 1 << 16 {
            1 << 16
        } else {
            1 << 14
        }
    }

    /// The cyclic lines of the newest samples of the ring between 0.6 and 1.25 of the width of `c`, the strongest first.
    ///
    /// The line of the symbol rate is a line of the squared envelope, whose band is twice that of the carrier. If the top
    /// of the search range is near half the sample rate or above it, the samples are first interpolated by 2: at the input
    /// rate the line would fold about half the sample rate, and at 2 samples per symbol it falls on that frequency, where
    /// its level depends on the sampling phase and no search finds it. So no alias of a line is in the range.
    fn cyclic(&self, c: &Carrier) -> Vec<(f64, f64)> {
        let n = self.rfill.min(RING);
        let start = (self.rpos + RING - n) % RING;
        let w = 2.0 * std::f64::consts::PI * c.offset_hz / self.fs;
        let mut v = Vec::with_capacity(n);
        for k in 0..n {
            let ph = -w * k as f64;
            v.push(self.ring[(start + k) % RING] * Complex32::new(ph.cos() as f32, ph.sin() as f32));
        }
        let len = self.cyclic_points();
        let (lo, hi) = (0.6 * c.rate_hz, 1.25 * c.rate_hz);
        if hi < FOLD_SHARE * self.fs {
            return cyclic_lines(&v, self.fs, len, lo, hi, 20.0);
        }
        let Ok(p) = Polyphase::<32, 64>::design(0.85, 60.0) else {
            return Vec::new();
        };
        let mut up = vec![Complex32::default(); 2 * n];
        Reader::new(p).read(&v, 0.0, 0.5, 0.0, &mut up);
        cyclic_lines(&up, 2.0 * self.fs, 2 * len, lo, hi, 20.0)
    }

    fn survey(&mut self) {
        let mut s = Vec::new();
        self.psd.spectrum(&mut s);
        bridge_dc(&mut s);
        let n = s.len();
        let bin = self.fs / n as f64;
        // Noise floor: a low percentile, corrected for its bias in an averaged periodogram.
        let pk = s.iter().cloned().fold(1e-30, f64::max);
        // The floor is at most 50 dB under the peak, the dynamic range of the fit. A lower floor makes the far skirts fail the fit.
        let floor = (quantile(&s, 0.1) * (1.0 + 1.28 / (SEGMENTS as f64).sqrt())).max(pk * 1e-5);
        self.spectrum = s
            .chunks(n / 512)
            .map(|c| sdrkit_algo_dsp::meter::power_to_db(c.iter().cloned().fold(0.0, f64::max) / pk) as f32)
            .collect();
        // Several carriers: find the strongest, blank it, again.
        let mut work = s.clone();
        let mut found: Vec<Carrier> = Vec::new();
        // The spans blanked so far (first and last bin) and the width measured in each, Hz.
        let mut blanked: Vec<(f64, f64, f64)> = Vec::new();
        for _ in 0..8 {
            let Some(c) = measure_carrier(&work, self.fs, floor, SEGMENTS, None) else {
                break;
            };
            if c.width_hz < 4.0 * bin {
                break;
            }
            let lo = ((c.centre_hz - 0.75 * c.width_hz) / bin + (n / 2) as f64).max(0.0) as usize;
            let hi =
                (((c.centre_hz + 0.75 * c.width_hz) / bin + (n / 2) as f64) as usize).min(n - 1);
            for v in &mut work[lo..=hi] {
                *v = floor;
            }
            // What is left of a carrier that was measured too narrow stands at the edge of its blanked span.
            let (el, eh) = (
                c.lo_hz / bin + (n / 2) as f64,
                c.hi_hz / bin + (n / 2) as f64,
            );
            let near = (n / 256) as f64;
            let rest = blanked.iter().any(|&(a, b, width)| {
                c.width_hz < REST_SHARE * width && ((el - b).abs() <= near || (eh - a).abs() <= near)
            });
            blanked.push((lo as f64, hi as f64, c.width_hz));
            if rest || c.snr_db < WEAK_SNR_DB || c.misfit > 2.5 {
                continue;
            }
            if found.iter().any(|f| {
                (f.offset_hz - c.centre_hz).abs() < 0.75 * (f.rate_hz + c.width_hz)
                    && f.snr_db >= c.snr_db + 6.0
            }) {
                continue;
            }
            found.push(Carrier {
                offset_hz: c.centre_hz,
                rate_hz: c.width_hz,
                rolloff: c.rolloff,
                snr_db: c.snr_db,
                level_db: 0.0,
                misfit: c.misfit,
                lo_hz: c.lo_hz,
                hi_hz: c.hi_hz,
                decoded: false,
            });
        }
        if let Some(top) = found.iter().map(|c| c.snr_db).reduce(f64::max) {
            for c in found.iter_mut() {
                c.level_db = c.snr_db - top;
            }
        }
        found.sort_by(|a, b| a.offset_hz.total_cmp(&b.offset_hz));
        // The order in which the carriers are tried: of those that fit a symbol rate set by hand, the nearest to the
        // picked offset first, or the nearest to the centre of those within 10 dB of the strongest.
        let mut order: Vec<usize> = (0..found.len())
            .filter(|&i| self.fits_manual_rate(found[i].rate_hz))
            .filter(|&i| self.cfg.pick_hz.is_some() || found[i].level_db > -10.0)
            .collect();
        let from = self.cfg.pick_hz.unwrap_or(0.0);
        order.sort_by(|&a, &b| {
            (found[a].offset_hz - from)
                .abs()
                .total_cmp(&(found[b].offset_hz - from).abs())
        });
        if let Some(a) = self.acq.as_ref() {
            // Keep the decoded flag on the carrier in use.
            let near = found
                .iter_mut()
                .filter(|c| (c.offset_hz - a.carrier.offset_hz).abs() < 0.3 * a.rate_hz)
                .min_by(|x, y| {
                    (x.offset_hz - a.carrier.offset_hz)
                        .abs()
                        .total_cmp(&(y.offset_hz - a.carrier.offset_hz).abs())
                });
            if let Some(c) = near {
                c.decoded = true;
            }
        }
        self.carriers = found.clone();
        if self.acq.is_some() {
            return;
        }
        // A hump without a line costs a line search: no more than two of them are tried in one survey.
        let mut humps = 0;
        for i in order {
            if found[i].snr_db < CLEAR_SNR_DB {
                if humps == 2 {
                    continue;
                }
                humps += 1;
            }
            if self.acquire(i, found[i]) != Tried::NoLine {
                return;
            }
        }
    }

    /// Takes the carrier `c`, number `i` of the list, and starts the chain on it, if its symbol rate is one the input
    /// allows and, for a carrier under [`CLEAR_SNR_DB`], if it has a cyclic line.
    fn acquire(&mut self, i: usize, c: Carrier) -> Tried {
        // Rate: manual, else the cyclic line near the width-based rate, else the width.
        let mut rate = c.rate_hz;
        let mut cyc = None;
        if self.cfg.manual_rs > 0.0 {
            rate = self.cfg.manual_rs;
            // The strongest line within the range that the clock loop searches about a rate set by hand: the clock
            // then starts from the line.
            let about = Carrier { rate_hz: rate, ..c };
            cyc = self
                .cyclic(&about)
                .into_iter()
                .map(|(f, _)| f)
                .find(|f| (f / rate - 1.0).abs() < WIDE.max_ppm * 1e-6);
        } else {
            let lines = self.cyclic(&c);
            // The search range holds no alias of a line, so the strongest line is the rate.
            if let Some(&(f, _)) = lines.first() {
                cyc = Some(f);
                if ((f - c.rate_hz) / c.rate_hz).abs() > 0.01 {
                    self.log.push(format!(
                        "symbol rate {:.3} Msym/s from the spectrum's width, {:.4} from its cyclic line",
                        c.rate_hz / 1e6,
                        f / 1e6
                    ));
                }
                rate = f;
            }
        }
        if c.snr_db < CLEAR_SNR_DB && cyc.is_none() {
            return Tried::NoLine;
        }
        let low = self.cfg.manual_rs <= 0.0 && rate < self.min_auto_rate();
        if low || rate > 1.02 * self.max_rate() || rate <= 0.0 {
            return Tried::Refused;
        }
        let (rolloff, src) = if self.cfg.rolloff > 0.0 {
            (self.cfg.rolloff, 1)
        } else if c.snr_db > 15.0 && c.rolloff > 0.0 {
            let snap = [0.35, 0.25, 0.20, 0.15, 0.10, 0.05]
                .into_iter()
                .min_by(|a, b| (a - c.rolloff).abs().total_cmp(&(b - c.rolloff).abs()))
                .unwrap_or(0.35);
            (snap, 1)
        } else {
            (0.35, 0)
        };
        self.log.push(format!(
            "carrier found: {:.3} Msym/s, {:+.0} kHz from the centre",
            rate / 1e6,
            c.offset_hz / 1e3
        ));
        self.carriers[i].decoded = true;
        self.no_carrier_s = 0.0;
        self.acq = Some(Acquisition {
            carrier: c,
            rate_hz: rate,
            cyclic_hz: cyc,
            rolloff,
            rolloff_source: src,
        });
        // The line is right within a point of its search. The range of the clock is not wider: at a low Es/N0 the
        // loop moves over its whole range before frames are followed, and the rate it then holds must be near enough
        // for the narrow loop to take over without a slip of a symbol.
        let near = ClockSearch {
            pull_in: NEAR_PULL_IN,
            max_ppm: (1e6 * self.fs / self.cyclic_points() as f64 / rate).clamp(NEAR_PPM.0, NEAR_PPM.1),
        };
        let search = if cyc.is_some() { near } else { WIDE };
        self.start_chain(c.offset_hz, cyc.unwrap_or(rate), rolloff, search, true);
        Tried::Taken
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The DC of a signal is followed and taken off, and the result is the same for every way the samples are cut into
    /// calls.
    #[test]
    fn dc_is_removed() {
        let offset = Complex32::new(0.3, -0.2);
        let x: Vec<Complex32> = (0..200_000)
            .map(|i| offset + Complex32::new((i as f32 * 0.7).sin(), (i as f32 * 1.1).cos()) * 0.5)
            .collect();
        let mut whole = x.clone();
        let mut dc = Dc::default();
        dc.remove(&mut whole);
        assert!((dc.level - offset).norm() < 0.01, "{}", dc.level);
        let n = 50_000;
        let mean = whole[x.len() - n..].iter().sum::<Complex32>() / n as f32;
        assert!(mean.norm() < 0.01, "{mean}");
        for piece in [1usize, 5, 8, 13, 4096] {
            let mut dc = Dc::default();
            let mut y = x.clone();
            for c in y.chunks_mut(piece) {
                dc.remove(c);
            }
            assert!(y == whole, "pieces of {piece}");
        }
    }

    #[test]
    fn energy_is_the_sum_of_the_squared_magnitudes() {
        let x: Vec<Complex32> = (0..3001)
            .map(|i| Complex32::new(i as f32 * 1e-3, -0.5))
            .collect();
        let want: f64 = x.iter().map(|v| f64::from(v.norm_sqr())).sum();
        assert!((energy(&x) - want).abs() < 1e-3 * want);
        assert_eq!(energy(&[]), 0.0);
    }
}
