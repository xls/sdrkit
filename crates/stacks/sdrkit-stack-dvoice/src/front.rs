//! The narrowband front end (the channel at 48 kHz with its coarse carrier search) and the demodulation chains that turn the
//! samples of the radio into the input of the symbol layer: an FM discriminator for FSK and GMSK, a differential detector for
//! π/4-DQPSK, and the FDMDV demodulator for FreeDV.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::{Complex32, ConfigError};

/// The working rate of every narrowband stack.
pub const RATE: f64 = 48_000.0;

/// The coarse carrier search on the 48 kHz channel, before the fine NCO.
#[derive(Clone, Debug)]
pub struct CoarseSearch {
    fft: Fft,
    buf: Vec<Complex32>,
    fill: usize,
    acc: Vec<f32>,
    frames: usize,
    window: Vec<f32>,
    scratch: Vec<f32>,
    /// Half width of the window searched for the carrier (Hz): 2500, or 1250 for 6.25 kHz stacks.
    pub half_hz: f64,
    /// The channel raster (12 500 or 6 250 Hz).
    pub raster_hz: f64,
    last: Option<f64>,
    /// Frames summed since the start (for the fast start).
    since_start: usize,
    /// The correction the front applies after the search (Hz).
    centre_hz: f64,
    /// A frame layer decodes.
    locked: bool,
}

/// What one search frame concluded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SearchResult {
    /// Not enough frames yet.
    Pending,
    /// Nothing to move to.
    Stay,
    /// Move the carrier estimate by this many hertz (relative to the present centre).
    Move(f64),
}

const NFFT: usize = 1024;
const SUM: usize = 8;

impl CoarseSearch {
    pub fn new(half_hz: f64, raster_hz: f64) -> CoarseSearch {
        let window = (0..NFFT)
            .map(|i| (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / NFFT as f64).cos()) as f32)
            .collect();
        CoarseSearch {
            fft: Fft::forward(NFFT),
            buf: vec![Complex32::new(0.0, 0.0); NFFT],
            fill: 0,
            acc: vec![0.0; NFFT],
            frames: 0,
            window,
            scratch: Vec::with_capacity(NFFT),
            half_hz,
            raster_hz,
            last: None,
            since_start: 0,
            centre_hz: 0.0,
            locked: false,
        }
    }

    pub fn reset(&mut self) {
        self.fill = 0;
        self.frames = 0;
        self.acc.iter_mut().for_each(|a| *a = 0.0);
        self.last = None;
        self.since_start = 0;
    }

    /// Feeds 48 kHz samples whose carrier the front corrects by `centre_hz` after the search. Returns a decision each time 8
    /// frames are summed (or at the fast start): a move is relative to that centre. While `locked` (a frame layer decodes) only
    /// a carrier 3 kHz or more from the centre moves it, so the spectrum of a passing data pattern does not.
    pub fn push(&mut self, x: &[Complex32], centre_hz: f64, locked: bool) -> SearchResult {
        self.centre_hz = centre_hz;
        self.locked = locked;
        let mut result = SearchResult::Pending;
        for &s in x {
            self.buf[self.fill] = s;
            self.fill += 1;
            if self.fill == NFFT {
                self.fill = 0;
                for (b, w) in self.buf.iter_mut().zip(&self.window) {
                    *b *= *w;
                }
                self.fft.process(&mut self.buf);
                for (a, b) in self.acc.iter_mut().zip(&self.buf) {
                    *a += b.norm_sqr();
                }
                self.frames += 1;
                self.since_start += 1;
                let fast = self.since_start == 3;
                if self.frames == SUM || fast {
                    let r = self.decide(fast);
                    if !fast || matches!(r, SearchResult::Move(_)) {
                        self.frames = 0;
                        self.acc.iter_mut().for_each(|a| *a = 0.0);
                    }
                    if !matches!(r, SearchResult::Pending) {
                        result = r;
                    }
                }
            }
        }
        result
    }

    fn bin_hz(i: usize) -> f64 {
        let k = if i < NFFT / 2 {
            i as f64
        } else {
            i as f64 - NFFT as f64
        };
        k * RATE / NFFT as f64
    }

    fn power_at(&self, hz: f64) -> f32 {
        let k = (hz / RATE * NFFT as f64).round() as i64;
        self.acc[k.rem_euclid(NFFT as i64) as usize]
    }

    fn decide(&mut self, fast: bool) -> SearchResult {
        let df = RATE / NFFT as f64;
        // Noise floor: lower quartile of the bins within +-21 kHz, times 1.3.
        self.scratch.clear();
        for i in 0..NFFT {
            if Self::bin_hz(i).abs() <= 21_000.0 {
                self.scratch.push(self.acc[i]);
            }
        }
        self.scratch
            .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let floor = self.scratch[self.scratch.len() / 4].max(1e-30) * 1.3;
        let hw = (self.half_hz / df).round() as i64;
        let window_power = |c: i64| -> f32 {
            (c - hw..=c + hw)
                .map(|k| self.power_at(k as f64 * df))
                .sum::<f32>()
        };
        let best_in = |lo: f64, hi: f64| -> (i64, f32) {
            let (a, b) = ((lo / df).round() as i64, (hi / df).round() as i64);
            let mut best = (0i64, f32::MIN);
            for c in a..=b {
                let p = window_power(c);
                if p > best.1 {
                    best = (c, p);
                }
            }
            best
        };
        let near_lim = 8_000.0;
        let (mut c, _) = best_in(-near_lim, near_lim);
        // A peak at the edge of the near range with more power beyond it is a neighbour's skirt: search the whole range.
        let edge = ((near_lim / df).round() as i64 - 2).max(1);
        if c.abs() >= edge {
            let beyond = window_power(c + c.signum() * 3 * hw);
            if beyond > window_power(c) {
                c = best_in(-18_000.0, 18_000.0).0;
            }
        }
        // Carrier power over the window against the noise in the same width.
        let width = (2 * hw + 1) as f32;
        let peak = window_power(c) / width;
        let snr = peak / floor;
        // Refine with the power centroid above the floor.
        let mut num = 0f64;
        let mut den = 0f64;
        for k in c - hw..=c + hw {
            let p = (self.power_at(k as f64 * df) - floor).max(0.0) as f64;
            num += p * k as f64 * df;
            den += p;
        }
        let est = if den > 0.0 { num / den } else { c as f64 * df };
        // A lone carrier exactly on the raster of the tuned frequency (a neighbouring channel) is not taken.
        let on_raster = |f: f64| {
            let n = (f / self.raster_hz).round();
            n != 0.0 && (f - n * self.raster_hz).abs() < 300.0
        };
        // The move from the centre the front applies.
        let rel = est - self.centre_hz;
        let least = if self.locked { 3_000.0 } else { 1_500.0 };
        if fast {
            if snr >= 20.0 && rel.abs() >= least && !on_raster(est) {
                self.last = None;
                return SearchResult::Move(rel);
            }
            return SearchResult::Pending;
        }
        if snr < 4.0 || on_raster(est) {
            self.last = None;
            return SearchResult::Stay;
        }
        let agree = self.last.is_some_and(|l| (l - est).abs() < 500.0);
        self.last = Some(est);
        if agree && rel.abs() >= least {
            self.last = None;
            SearchResult::Move(rel)
        } else {
            SearchResult::Stay
        }
    }
}

/// What the front end did with the latest piece of signal.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrontInfo {
    /// The correction applied, coarse plus fine (Hz).
    pub offset_hz: f32,
    /// The output index where a replay starts, and its length (0: no replay in this piece).
    pub replay_at: u64,
    pub replay_n: u64,
}

/// The front: shifts the channel to 0 Hz, follows the carrier, decimates to 48 kHz, removes DC and applies the fine correction.
#[derive(Debug)]
pub struct NarrowFront {
    coarse_enabled: bool,
    dc_enabled: bool,
    shift: Nco,
    resampler: Option<Resampler<Complex32>>,
    fine: Nco,
    search: CoarseSearch,
    big_hz: f64,
    small_hz: f64,
    fine_hz: f64,
    signal_offset_hz: f64,
    dc: Complex32,
    dc_count: u64,
    mixed: Vec<Complex32>,
    dec: Vec<Complex32>,
    replay: Vec<Complex32>,
    replay_pos: usize,
    replay_len: usize,
    locked: bool,
    produced: u64,
}

impl NarrowFront {
    /// A front for a channel `raster_hz` wide (12 500 or 6 250).
    pub fn new(raster_hz: f64) -> NarrowFront {
        let half_hz = if raster_hz < 10_000.0 {
            1_250.0
        } else {
            2_500.0
        };
        NarrowFront {
            coarse_enabled: true,
            dc_enabled: true,
            shift: Nco::new(1.0, 0.0),
            resampler: None,
            fine: Nco::new(RATE, 0.0),
            search: CoarseSearch::new(half_hz, raster_hz),
            big_hz: 0.0,
            small_hz: 0.0,
            fine_hz: 0.0,
            signal_offset_hz: 0.0,
            dc: Complex32::new(0.0, 0.0),
            dc_count: 0,
            mixed: Vec::new(),
            dec: Vec::new(),
            replay: vec![Complex32::new(0.0, 0.0); (0.3 * RATE) as usize],
            replay_pos: 0,
            replay_len: 0,
            locked: false,
            produced: 0,
        }
    }
    /// Turns the DC removal off (TETRAPOL's carrier can sit on the centre).
    pub fn without_dc_removal(mut self) -> Self {
        self.dc_enabled = false;
        self
    }
    /// Turns the coarse search off (a stack that follows its carrier otherwise).
    pub fn without_search(mut self) -> Self {
        self.coarse_enabled = false;
        self
    }

    /// The total correction now applied (Hz).
    pub fn offset_hz(&self) -> f64 {
        self.big_hz + self.small_hz + self.fine_hz
    }

    /// Prepares for input at `rate_hz` with the channel `signal_offset_hz` from its centre. An error when no resampler to 48 kHz
    /// can be made for the rate.
    pub fn configure(&mut self, rate_hz: f64, signal_offset_hz: f64) -> Result<(), ConfigError> {
        self.shift = Nco::new(rate_hz, 0.0);
        self.resampler = if (rate_hz - RATE).abs() < 1e-6 {
            None
        } else {
            Some(
                Resampler::with_quality(
                    rate_hz,
                    RATE,
                    (22_000.0 / rate_hz.min(RATE)).min(0.46),
                    60.0,
                )
                .map_err(ConfigError::from)?,
            )
        };
        self.retune(signal_offset_hz);
        Ok(())
    }

    /// A retune: the channel is now `signal_offset_hz` from the centre. The carrier estimate and the filters start again.
    pub fn retune(&mut self, signal_offset_hz: f64) {
        self.big_hz = 0.0;
        self.small_hz = 0.0;
        self.fine_hz = 0.0;
        self.dc = Complex32::new(0.0, 0.0);
        self.dc_count = 0;
        self.search.reset();
        self.replay_len = 0;
        self.replay_pos = 0;
        self.locked = false;
        self.fine.set_freq(0.0);
        if let Some(r) = self.resampler.as_mut() {
            r.reset();
        }
        self.signal_offset_hz = signal_offset_hz;
        self.set_shift();
    }

    fn set_shift(&mut self) {
        self.shift.set_freq(-(self.signal_offset_hz + self.big_hz));
    }

    /// A frame layer is locked (the coarse search then moves only a carrier 3 kHz or more off) or not.
    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    /// Moves the fine NCO by `hz` (a residual offset the symbol layer measured). Steps of 3 kHz or more are refused.
    pub fn nudge_fine(&mut self, hz: f64) {
        if hz.is_finite() && hz.abs() < 3_000.0 {
            self.fine_hz += hz;
            self.fine.set_freq(-(self.small_hz + self.fine_hz));
        }
    }

    /// Runs `x` (at the configured rate) and appends the 48 kHz channel to `out`. When the coarse search moved the carrier, the
    /// kept 0.3 s goes out again through the corrected oscillator before the new samples.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) -> FrontInfo {
        self.mixed.clear();
        self.mixed.extend_from_slice(x);
        self.shift.mix_in_place(&mut self.mixed);
        self.dec.clear();
        match self.resampler.as_mut() {
            Some(r) => r.process(&self.mixed, &mut self.dec),
            None => self.dec.extend_from_slice(&self.mixed),
        }
        // DC removal: one-pole running mean, 5 ms for the first 50 ms, then 0.1 s.
        if self.dc_enabled {
            for s in self.dec.iter_mut() {
                let tau = if self.dc_count < (0.05 * RATE) as u64 {
                    0.005
                } else {
                    0.1
                };
                let a = (1.0 / (tau * RATE)) as f32;
                self.dc += (*s - self.dc) * a;
                *s -= self.dc;
                self.dc_count += 1;
            }
        }
        // Coarse search (before the fine NCO), while no frame layer is locked.
        let mut moved = None;
        if self.coarse_enabled {
            let centre = self.small_hz + self.fine_hz;
            if let SearchResult::Move(d) = self.search.push(&self.dec, centre, self.locked) {
                moved = Some(d);
            }
        }
        // Keep the last 0.3 s before the fine NCO, for the replay.
        let n = self.replay.len();
        for &s in &self.dec {
            self.replay[self.replay_pos] = s;
            self.replay_pos = (self.replay_pos + 1) % n;
            self.replay_len = (self.replay_len + 1).min(n);
        }
        let at = self.produced;
        let mut replay_n = 0usize;
        if let Some(d) = moved {
            // The move is relative to the whole correction after the search, the fine part too.
            let total = self.small_hz + self.fine_hz + d;
            if total.abs() > 7_000.0 {
                // Mixed out at the input rate: future samples only.
                self.big_hz += total;
                self.small_hz = 0.0;
                self.fine_hz = 0.0;
                self.set_shift();
            } else {
                self.small_hz = total;
                self.fine_hz = 0.0;
            }
            self.fine.set_freq(-(self.small_hz + self.fine_hz));
            self.search.reset();
            replay_n = self.replay_len.saturating_sub(self.dec.len());
        }
        let start = out.len();
        if replay_n > 0 {
            let first = (self.replay_pos + n - self.replay_len) % n;
            out.extend((0..replay_n).map(|i| self.replay[(first + i) % n]));
        }
        out.extend_from_slice(&self.dec);
        self.fine.mix_in_place(&mut out[start..]);
        self.produced += (self.dec.len() + replay_n) as u64;
        FrontInfo {
            offset_hz: self.offset_hz() as f32,
            replay_at: at,
            replay_n: replay_n as u64,
        }
    }
}

/// The pulse the symbol filter matches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pulse {
    /// Root raised cosine with this roll-off.
    RootRaisedCosine(f64),
    /// Integrate and dump over one symbol.
    IntegrateDump,
    /// A linear-phase low-pass with this cut-off (Hz): for a transmitter whose pulse is already a Nyquist (raised cosine) pulse,
    /// it removes the noise beyond the signal's band and keeps the symbols free of interference.
    LowPass(f64),
}

/// The taps of a symbol filter at `RATE` for symbols at `baud`, with unit gain at DC (so the output stays in hertz).
pub fn symbol_filter(pulse: Pulse, baud: f64) -> Fir<f32> {
    let sps = (RATE / baud).round().max(1.0);
    let t = match pulse {
        Pulse::RootRaisedCosine(a) => {
            taps(Shape::Rrc { beta: a }, sps, 8.0, Norm::Dc).unwrap_or_else(|_| vec![1.0])
        }
        Pulse::IntegrateDump => vec![1.0 / sps as f32; sps as usize],
        Pulse::LowPass(c) => sdrkit_algo_dsp::fir::lowpass(RATE, c * 0.85, c * 1.2, 50.0)
            .expect("the filter settings are valid"),
    };
    Fir::new(&t)
}

/// The front-end choices of an FSK or GMSK stack.
#[derive(Clone, Copy, Debug)]
pub struct FskChain {
    pub raster_hz: f64,
    pub chan_pass_hz: f64,
    pub chan_stop_hz: f64,
    pub pulse: Pulse,
    pub baud: f64,
    pub dc_removal: bool,
    /// The symbol filter of a second lane (a second symbol rate): its pulse and rate.
    pub lane2: Option<(Pulse, f64)>,
}

/// The front-end choices of a π/4-DQPSK stack.
#[derive(Clone, Copy, Debug)]
pub struct DqpskChain {
    pub raster_hz: f64,
    pub chan_pass_hz: f64,
    pub chan_stop_hz: f64,
    /// The rate the symbols are read at (a whole number of samples per symbol).
    pub rate: f64,
    pub alpha: f64,
    pub baud: f64,
}

/// Which demodulation chain a stack uses.
#[derive(Clone, Copy, Debug)]
pub enum ChainSpec {
    Fsk(FskChain),
    Dqpsk(DqpskChain),
    /// FreeDV: the upper sideband channel at 8 kHz into the FDMDV demodulator of the 1600 waveform. The protocol takes the
    /// same channel for its OFDM waveforms ([`Chain::channel_8k`]).
    Fdmdv,
}

/// The differential detector of the π/4-DQPSK stacks: angle(x[n] conj(x[n - sps])) scaled so a step of π/4 reads
/// [`crate::dqpsk::UNITS_PER_QUARTER`].
#[derive(Debug)]
struct DiffDetector {
    hist: Vec<Complex32>,
    pos: usize,
}

impl DiffDetector {
    fn process(&mut self, x: &[Complex32], out: &mut Vec<f32>) {
        let n = self.hist.len();
        let k = crate::dqpsk::UNITS_PER_QUARTER / std::f32::consts::FRAC_PI_4;
        for &s in x {
            let old = self.hist[self.pos];
            self.hist[self.pos] = s;
            self.pos = (self.pos + 1) % n;
            let d = s * old.conj();
            out.push(sdrkit_algo_dsp::approx::atan2(d.im, d.re) * k);
        }
    }
}

#[derive(Debug)]
enum Stages {
    Fsk {
        disc: Discriminator,
        sym: Fir<f32>,
        sym2: Option<Fir<f32>>,
        hz: Vec<f32>,
    },
    Dqpsk {
        resampler: Option<Resampler<Complex32>>,
        mf: Fir<Complex32>,
        diff: DiffDetector,
        rated: Vec<Complex32>,
    },
    Fdmdv {
        resampler: Option<Resampler<Complex32>>,
        demod: Box<crate::freedv::FdmdvDemod>,
        rated: Vec<Complex32>,
    },
}

/// A demodulation chain: the front end, the channel filter and the stack's demodulator, from the radio's samples to the symbol
/// layer's input (one or two lanes of real values at the stream rate).
#[derive(Debug)]
pub struct Chain {
    pub front: NarrowFront,
    chan: Fir<Complex32>,
    stages: Stages,
    /// The 48 kHz channel of the latest piece, before the channel filter (for the level, noise and spectrum measurements).
    pub iq: Vec<Complex32>,
    filtered: Vec<Complex32>,
    /// The symbol layer's input of the latest piece, per lane.
    pub lane1: Vec<f32>,
    pub lane2: Vec<f32>,
    /// Hertz of carrier offset per unit of the symbol layer's input (0: the input does not measure the carrier).
    hz_per_unit: f32,
}

impl Chain {
    /// The chain of `spec`.
    pub fn new(spec: ChainSpec) -> Chain {
        let (front, pass, stop) = match spec {
            ChainSpec::Fsk(c) => {
                let mut f = NarrowFront::new(c.raster_hz);
                if !c.dc_removal {
                    f = f.without_dc_removal();
                }
                (f, c.chan_pass_hz, c.chan_stop_hz)
            }
            ChainSpec::Dqpsk(c) => (
                NarrowFront::new(c.raster_hz).without_search(),
                c.chan_pass_hz,
                c.chan_stop_hz,
            ),
            ChainSpec::Fdmdv => (NarrowFront::new(3_000.0).without_search(), 3_000.0, 3_600.0),
        };
        // A DQPSK step of pi/4 reads UNITS_PER_QUARTER units, and an offset f turns each symbol by 2 pi f / baud.
        let hz_per_unit = match spec {
            ChainSpec::Fsk(_) => 1.0,
            ChainSpec::Dqpsk(c) => (c.baud / (8.0 * crate::dqpsk::UNITS_PER_QUARTER as f64)) as f32,
            ChainSpec::Fdmdv => 0.0,
        };
        let stages = match spec {
            ChainSpec::Fsk(c) => Stages::Fsk {
                disc: Discriminator::new(RATE, 1.0).expect("the rate and deviation are positive"),
                sym: symbol_filter(c.pulse, c.baud),
                sym2: c.lane2.map(|(p, b)| symbol_filter(p, b)),
                hz: Vec::new(),
            },
            ChainSpec::Dqpsk(c) => {
                let sps = (c.rate / c.baud).round().max(1.0);
                let mf = taps(Shape::Rrc { beta: c.alpha }, sps, 8.0, Norm::Dc)
                    .unwrap_or_else(|_| vec![1.0]);
                Stages::Dqpsk {
                    resampler: Resampler::with_quality(RATE, c.rate, 0.45, 60.0).ok(),
                    mf: Fir::new(&mf),
                    diff: DiffDetector {
                        hist: vec![Complex32::new(0.0, 0.0); sps as usize],
                        pos: 0,
                    },
                    rated: Vec::new(),
                }
            }
            ChainSpec::Fdmdv => Stages::Fdmdv {
                resampler: Resampler::with_quality(RATE, crate::freedv::AUDIO_RATE, 0.45, 60.0)
                    .ok(),
                demod: Box::default(),
                rated: Vec::new(),
            },
        };
        Chain {
            front,
            chan: Fir::new(
                &sdrkit_algo_dsp::fir::lowpass(RATE, pass, stop, 60.0)
                    .expect("the filter settings are valid"),
            ),
            stages,
            iq: Vec::new(),
            filtered: Vec::new(),
            lane1: Vec::new(),
            lane2: Vec::new(),
            hz_per_unit,
        }
    }

    /// The carrier offset, Hz, of a centre the symbol layer measured in its input's units. The FSK lanes are in hertz already.
    /// The DQPSK lanes are phase steps. The FreeDV lane is soft bits, which do not measure the carrier (its demodulator follows
    /// the pilot itself), so the result is 0.
    pub fn lane_hz(&self, units: f32) -> f32 {
        units * self.hz_per_unit
    }

    /// Prepares for input at `rate_hz` with the channel `signal_offset_hz` from its centre.
    pub fn configure(&mut self, rate_hz: f64, signal_offset_hz: f64) -> Result<(), ConfigError> {
        self.front.configure(rate_hz, signal_offset_hz)?;
        self.reset_filters();
        Ok(())
    }

    /// A retune to `signal_offset_hz` from the centre.
    pub fn retune(&mut self, signal_offset_hz: f64) {
        self.front.retune(signal_offset_hz);
        self.reset_filters();
    }

    fn reset_filters(&mut self) {
        self.chan.reset();
        match &mut self.stages {
            Stages::Fsk {
                disc, sym, sym2, ..
            } => {
                disc.reset();
                sym.reset();
                if let Some(s) = sym2 {
                    s.reset();
                }
            }
            Stages::Dqpsk {
                resampler,
                mf,
                diff,
                ..
            } => {
                if let Some(r) = resampler {
                    r.reset();
                }
                mf.reset();
                diff.hist.fill(Complex32::new(0.0, 0.0));
            }
            Stages::Fdmdv {
                resampler, demod, ..
            } => {
                if let Some(r) = resampler {
                    r.reset();
                }
                **demod = crate::freedv::FdmdvDemod::default();
            }
        }
    }

    /// The 8 kHz channel of the latest piece (0 Hz is the dial frequency), for a FreeDV chain; empty for the other chains.
    pub fn channel_8k(&self) -> &[Complex32] {
        match &self.stages {
            Stages::Fdmdv { rated, .. } => rated,
            _ => &[],
        }
    }

    /// The rate of the symbol layer's input.
    pub fn stream_rate(&self, dqpsk_rate: f64) -> f64 {
        match self.stages {
            Stages::Fsk { .. } => RATE,
            Stages::Dqpsk { .. } => dqpsk_rate,
            Stages::Fdmdv { .. } => crate::freedv::STREAM_RATE,
        }
    }

    /// Runs a piece of the radio's samples through the chain: `iq` holds the 48 kHz channel, `lane1` and `lane2` the symbol
    /// layer's input.
    pub fn process(&mut self, x: &[Complex32]) -> FrontInfo {
        self.iq.clear();
        let info = self.front.process(x, &mut self.iq);
        self.filtered.clear();
        self.filtered.extend_from_slice(&self.iq);
        self.chan.process_in_place(&mut self.filtered);
        self.lane1.clear();
        self.lane2.clear();
        match &mut self.stages {
            Stages::Fsk {
                disc,
                sym,
                sym2,
                hz,
            } => {
                hz.clear();
                disc.process(&self.filtered, hz);
                for v in hz.iter_mut() {
                    if !v.is_finite() {
                        *v = 0.0;
                    }
                }
                if let Some(s2) = sym2 {
                    self.lane2.extend_from_slice(hz);
                    s2.process_in_place(&mut self.lane2);
                }
                self.lane1.extend_from_slice(hz);
                sym.process_in_place(&mut self.lane1);
            }
            Stages::Dqpsk {
                resampler,
                mf,
                diff,
                rated,
            } => {
                rated.clear();
                match resampler {
                    Some(r) => r.process(&self.filtered, rated),
                    None => rated.extend_from_slice(&self.filtered),
                }
                mf.process_in_place(rated);
                diff.process(rated, &mut self.lane1);
                self.lane2.extend_from_slice(&self.lane1);
            }
            Stages::Fdmdv {
                resampler,
                demod,
                rated,
            } => {
                rated.clear();
                match resampler {
                    Some(r) => r.process(&self.filtered, rated),
                    None => rated.extend_from_slice(&self.filtered),
                }
                demod.process(rated, &mut self.lane1);
                self.lane2.extend_from_slice(&self.lane1);
            }
        }
        info
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, f: f64) -> Vec<Complex32> {
        let mut nco = Nco::new(RATE, f);
        let mut v = vec![Complex32::new(0.0, 0.0); n];
        nco.fill(&mut v);
        v
    }

    #[test]
    fn the_search_finds_an_offset_carrier_and_ignores_the_raster() {
        let mut s = CoarseSearch::new(2_500.0, 12_500.0);
        let mut moves = Vec::new();
        for chunk in tone(48_000, 5_000.0).chunks(4800) {
            if let SearchResult::Move(d) = s.push(chunk, 0.0, false) {
                moves.push(d);
            }
        }
        assert!(!moves.is_empty());
        assert!((moves[0] - 5_000.0).abs() < 100.0, "{moves:?}");
        let mut s = CoarseSearch::new(2_500.0, 12_500.0);
        for chunk in tone(48_000, 12_500.0).chunks(4800) {
            assert!(!matches!(s.push(chunk, 0.0, false), SearchResult::Move(_)));
        }
    }

    #[test]
    fn a_move_of_the_search_keeps_the_fine_correction() {
        let mut f = NarrowFront::new(12_500.0);
        f.configure(RATE, 0.0).unwrap();
        let mut out = Vec::new();
        for chunk in tone(48_000, 5_000.0).chunks(4800) {
            out.clear();
            f.process(chunk, &mut out);
        }
        assert!((f.offset_hz() - 5_000.0).abs() < 100.0, "{}", f.offset_hz());
        f.nudge_fine(800.0);
        for chunk in tone(96_000, 7_800.0).chunks(4800) {
            out.clear();
            f.process(chunk, &mut out);
        }
        assert!((f.offset_hz() - 7_800.0).abs() < 100.0, "{}", f.offset_hz());
    }

    #[test]
    fn the_symbol_filters_have_unit_gain() {
        for p in [
            Pulse::RootRaisedCosine(0.2),
            Pulse::IntegrateDump,
            Pulse::LowPass(3_200.0),
        ] {
            let mut f = symbol_filter(p, 4800.0);
            let mut x = vec![1.0f32; 400];
            f.process_in_place(&mut x);
            assert!((x[399] - 1.0).abs() < 1e-3, "{p:?}: {}", x[399]);
        }
    }
}
