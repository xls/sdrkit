//! The shared front ends of the crate: the narrow FM front end (`trunk.nfm_front`), the envelope tracker (`trunk.eye`), the paging
//! front end (`pager.front`) with its carrier detector and coarse AFC, and the paging discriminator (`pager.disc`).

use crate::typed_port;
use num_complex::Complex32;
use sdrkit_algo_dsp::approx::atan2;
use sdrkit_algo_dsp::decim::{Decimator, FirDecimator};
use sdrkit_algo_dsp::fir;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, Timed,
};
use std::f64::consts::TAU;

use sdrkit_algo_dsp::iir::Butterworth4;
use sdrkit_algo_modem::follow::{SpectralConfig, SpectralFollow};

/// The level of a channel, reported a few times a second by a front end.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Level {
    /// dBFS of the filtered channel (-200 before anything was measured).
    pub dbfs: f32,
}

impl Item for Level {
    const KIND: &'static str = "trunk.level";
}

/// `trunk.nfm_front`: mixer, resampler to the working rate, fourth-order Butterworth channel filter, discriminator in Hz, level.
/// With [`NfmFront::product_lowpass`] the discriminator takes the angle of the low-passed lag product. With [`NfmFront::follow`] the spectral carrier follow-up in closed loop measures the resampled channel before the channel
/// filter and moves the mixer onto the carrier.
pub struct NfmFront {
    work: f64,
    half_width: f64,
    offset: f64,
    rate: f64,
    nco: Option<Nco>,
    res: Option<Resampler<Complex32>>,
    fi: Butterworth4,
    fq: Butterworth4,
    follow: Option<CarrierFollow>,
    /// The low-pass of the lag product (I and Q) and its cut-off, Hz.
    product: Option<(Butterworth4, Butterworth4, f64)>,
    last: Complex32,
    power: f64,
    measured: bool,
    mixed: Vec<Complex32>,
    worked: Vec<Complex32>,
    since_level: usize,
    keep: bool,
    filtered: Vec<Complex32>,
}

/// The follow-up of [`NfmFront`]: it sees the resampled channel with the mixer's correction taken back out, so its own mixer
/// (closed loop) gives the channel as the mixer now delivers it.
struct CarrierFollow {
    follow: SpectralFollow,
    undo: Nco,
    probe: Vec<Complex32>,
    applied: f64,
}

impl NfmFront {
    /// A front end with this working rate and channel half-width, for the channel at the format's signal offset.
    pub fn new(work: f64, half_width: f64) -> NfmFront {
        NfmFront {
            work,
            half_width,
            offset: 0.0,
            rate: 0.0,
            nco: None,
            res: None,
            fi: Butterworth4::lowpass(work, half_width),
            fq: Butterworth4::lowpass(work, half_width),
            follow: None,
            product: None,
            last: Complex32::new(0.0, 0.0),
            power: 0.0,
            measured: false,
            mixed: Vec::new(),
            worked: Vec::new(),
            since_level: 0,
            keep: false,
            filtered: Vec::new(),
        }
    }

    /// Turns on the carrier follow-up in closed loop: search `+-search_hz`, correction at most `+-max_hz`, the other numbers of
    /// [`SpectralConfig::closed_loop`].
    pub fn follow(mut self, search_hz: f64, max_hz: f64) -> NfmFront {
        let cfg = SpectralConfig {
            search_hz,
            max_hz,
            ..SpectralConfig::closed_loop(self.work)
        };
        let follow = SpectralFollow::new(cfg).expect("the follow-up settings are valid");
        self.follow = Some(CarrierFollow {
            follow,
            undo: Nco::new(self.work, 0.0),
            probe: Vec::new(),
            applied: 0.0,
        });
        self
    }

    /// Low-passes the lag product `y[n] conj(y[n-1])` at `lowpass_hz` before its angle is taken. The angle of the mean product
    /// over many samples has no clicks at a carrier-to-noise ratio where the angle of each sample has them. `lowpass_hz` must
    /// pass the modulation that the consumer reads; faster modulation only shortens the mean product.
    pub fn product_lowpass(mut self, lowpass_hz: f64) -> NfmFront {
        self.product = Some((
            Butterworth4::lowpass(self.work, lowpass_hz),
            Butterworth4::lowpass(self.work, lowpass_hz),
            lowpass_hz,
        ));
        self
    }

    /// The carrier error the follow-up corrects, Hz (0 without the follow-up).
    pub fn correction_hz(&self) -> f64 {
        self.follow.as_ref().map_or(0.0, |f| f.applied)
    }

    /// The mixer frequency for the channel offset and the correction.
    fn mixer_hz(&self) -> f64 {
        -(self.offset + self.correction_hz())
    }

    /// The level, dBFS (-200 before anything was measured).
    pub fn level_dbfs(&self) -> f32 {
        if self.measured && self.power > 0.0 {
            (10.0 * self.power.log10()) as f32
        } else {
            -200.0
        }
    }
    /// Keeps the filtered channel of each [`NfmFront::run`] for [`NfmFront::channel`] (off by default).
    pub fn keep_channel(&mut self, on: bool) {
        self.keep = on;
        self.filtered.clear();
    }

    /// The filtered channel at the working rate of the last [`NfmFront::run`], when [`NfmFront::keep_channel`] is on.
    pub fn channel(&self) -> &[Complex32] {
        &self.filtered
    }

    /// Runs samples (already at the channel offset given by `offset`) and appends the discriminator output (Hz).
    pub fn run(&mut self, iq: &[Complex32], out: &mut Vec<f32>) {
        let (Some(nco), Some(res)) = (self.nco.as_mut(), self.res.as_mut()) else {
            return;
        };
        self.mixed.resize(iq.len(), Complex32::new(0.0, 0.0));
        nco.mix(iq, &mut self.mixed);
        self.worked.clear();
        res.process(&self.mixed, &mut self.worked);
        if let Some(f) = self.follow.as_mut() {
            f.probe.clear();
            f.probe.extend_from_slice(&self.worked);
            f.undo.mix_in_place(&mut f.probe);
            f.follow.process(&mut f.probe);
            let c = f.follow.correction_hz();
            if c != f.applied {
                f.applied = c;
                f.undo.set_freq(c);
                nco.set_freq(-(self.offset + c));
            }
        }
        let k = (self.work / TAU) as f32;
        out.reserve(self.worked.len());
        self.filtered.clear();
        for &z in &self.worked {
            let y = Complex32::new(self.fi.process_sample(z.re), self.fq.process_sample(z.im));
            if self.keep {
                self.filtered.push(y);
            }
            let mut d = y * self.last.conj();
            self.last = y;
            if let Some((fi, fq, _)) = self.product.as_mut() {
                d = Complex32::new(fi.process_sample(d.re), fq.process_sample(d.im));
            }
            out.push(atan2(d.im, d.re) * k);
            let p = y.norm_sqr() as f64;
            if self.measured {
                self.power += 0.0005 * (p - self.power);
            } else {
                self.power = p;
                self.measured = true;
            }
        }
    }
    /// Sets up for an input rate and channel offset, and clears the state.
    pub fn setup(&mut self, rate: f64, offset: f64) -> Result<(), ConfigError> {
        if rate < self.work {
            return Err(ConfigError::RateTooLow {
                need: self.work,
                got: rate,
            });
        }
        if rate > self.work * 4096.0 {
            return Err(ConfigError::RateTooHigh {
                max: self.work * 4096.0,
                got: rate,
            });
        }
        self.rate = rate;
        self.offset = offset;
        if let Some(f) = self.follow.as_mut() {
            f.follow.retune();
            f.undo = Nco::new(self.work, 0.0);
            f.applied = 0.0;
        }
        self.nco = Some(Nco::new(rate, self.mixer_hz()));
        self.res = Some(Resampler::with_quality(rate, self.work, 0.45, 70.0)?);
        self.fi = Butterworth4::lowpass(self.work, self.half_width);
        self.fq = Butterworth4::lowpass(self.work, self.half_width);
        if let Some((fi, fq, hz)) = self.product.as_mut() {
            *fi = Butterworth4::lowpass(self.work, *hz);
            *fq = Butterworth4::lowpass(self.work, *hz);
        }
        self.last = Complex32::new(0.0, 0.0);
        self.power = 0.0;
        self.measured = false;
        self.since_level = 0;
        Ok(())
    }
    /// Moves the channel to another offset (live, phase continuous); clears the follow-up's correction.
    pub fn set_offset(&mut self, offset: f64) {
        self.offset = offset;
        if let Some(f) = self.follow.as_mut() {
            f.follow.retune();
            f.undo.set_freq(0.0);
            f.applied = 0.0;
        }
        let hz = self.mixer_hz();
        if let Some(n) = self.nco.as_mut() {
            n.set_freq(hz);
        }
    }
    /// Clears the filters and the level, and restarts the follow-up's averages (an interruption of the stream).
    pub fn clear(&mut self) {
        if let Some(r) = self.res.as_mut() {
            r.reset();
        }
        if let Some(f) = self.follow.as_mut() {
            f.follow.restart_averages();
        }
        self.fi.reset();
        self.fq.reset();
        if let Some((fi, fq, _)) = self.product.as_mut() {
            fi.reset();
            fq.reset();
        }
        self.last = Complex32::new(0.0, 0.0);
        self.power = 0.0;
        self.measured = false;
    }
    /// The working rate.
    pub fn work_rate(&self) -> f64 {
        self.work
    }
}

impl Component for NfmFront {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "trunk.nfm_front",
            vec![Port::iq("in")],
            vec![Port::real("out"), typed_port::<Level>("level")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.setup(f.rate, f.signal_offset_hz())?;
        Ok(vec![
            Format {
                kind: Kind::Real,
                rate: self.work,
                ..f.clone()
            },
            Format::new(Kind::typed::<Level>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let iq = io.iq(0);
        let mut out = std::mem::take(io.real_out(0));
        let before = out.len();
        self.run(iq, &mut out);
        let made = out.len() - before;
        *io.real_out(0) = out;
        self.since_level += made;
        let every = (self.work / 10.0) as usize;
        if self.since_level >= every {
            self.since_level = 0;
            let t = io.input(0).end() as f64 / self.rate.max(1.0);
            let dbfs = self.level_dbfs();
            io.items_out::<Level>(1).push(Timed {
                time: t,
                value: Level { dbfs },
            });
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// `trunk.eye`: follows the high and the low of a signal (fast attack 0.05, slow decay 0.0005).
#[derive(Clone, Copy, Debug, Default)]
pub struct Eye {
    hi: f32,
    lo: f32,
    started: bool,
}

impl Eye {
    /// Takes one sample.
    #[inline]
    pub fn update(&mut self, x: f32) {
        if !self.started {
            self.hi = x;
            self.lo = x;
            self.started = true;
            return;
        }
        let ah = if x > self.hi { 0.05 } else { 0.0005 };
        self.hi += ah * (x - self.hi);
        let al = if x < self.lo { 0.05 } else { 0.0005 };
        self.lo += al * (x - self.lo);
    }
    /// The slicer threshold: the middle.
    #[inline]
    pub fn mid(&self) -> f32 {
        0.5 * (self.hi + self.lo)
    }
    /// The range (high - low), the data swing.
    #[inline]
    pub fn range(&self) -> f32 {
        self.hi - self.lo
    }
    /// Forgets.
    pub fn reset(&mut self) {
        *self = Eye::default();
    }
}

/// The paging front end's decimation: an integer between floor(fs / 130 kHz) and floor(fs / 96 kHz), from the largest
/// down the first whose largest prime factor is at most 7, else the one with the smallest largest prime factor.
pub fn pager_decimation(fs: f64) -> usize {
    let lo = (fs / 130e3).floor().max(1.0) as usize;
    let hi = (fs / 96e3).floor().max(1.0) as usize;
    let lpf = |mut n: usize| {
        let mut best = 1;
        let mut p = 2;
        while p * p <= n {
            while n.is_multiple_of(p) {
                best = p;
                n /= p;
            }
            p += 1;
        }
        if n > 1 {
            best = best.max(n);
        }
        best
    };
    let (lo, hi) = (lo.min(hi), hi.max(lo));
    for d in (lo..=hi).rev() {
        if lpf(d) <= 7 {
            return d;
        }
    }
    (lo..=hi).min_by_key(|&d| lpf(d)).unwrap_or(1)
}

/// What the paging front end reports every 10 ms: the carrier detector and the AFC.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CarrierState {
    /// A carrier is present (with the 0.3 s hold).
    pub present: bool,
    /// Smoothed SNR, dB (0 without a carrier).
    pub snr_db: f32,
    /// The noise floor, linear power.
    pub floor: f32,
    /// The interval's power, linear.
    pub power: f32,
    /// The level of the whole input, dBFS.
    pub input_dbfs: f32,
    /// The AFC correction applied, Hz (the carrier is this far from the channel centre).
    pub afc_hz: f32,
    /// The 48 kHz output index from which the current AFC correction acts (its last change, through the channel filter).
    pub afc_at: u64,
}

impl Item for CarrierState {
    const KIND: &'static str = "pager.carrier";
}

/// That a decoder is reading a transmission (the AFC holds still meanwhile).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Busy(pub bool);

impl Item for Busy {
    const KIND: &'static str = "pager.busy";
}

/// The carrier detector, on 10 ms intervals at 48 kHz.
#[derive(Clone, Debug, Default)]
pub struct CarrierDetector {
    acc: f64,
    n: usize,
    floor: f64,
    started: bool,
    hold: u32,
    snr: f64,
    /// Present now (with the hold).
    pub present: bool,
    /// The last interval's power.
    pub power: f64,
}

impl CarrierDetector {
    /// Takes a sample's power. Returns true at the end of an interval.
    #[inline]
    pub fn push(&mut self, p: f32) -> bool {
        self.acc += p as f64;
        self.n += 1;
        if self.n < 480 {
            return false;
        }
        let p = self.acc / self.n as f64;
        self.acc = 0.0;
        self.n = 0;
        self.power = p;
        if !self.started {
            self.floor = p;
            self.started = true;
        } else {
            self.floor = p.min(self.floor * 1.0005);
        }
        let floor = self.floor.max(1e-20);
        if p > 2.0 * floor {
            self.hold = 30;
            let snr = 10.0 * ((p - floor) / floor).log10();
            self.snr = if self.present {
                0.9 * self.snr + 0.1 * snr
            } else {
                snr
            };
            self.present = true;
        } else if self.hold > 0 {
            self.hold -= 1;
        } else {
            self.present = false;
            self.snr = 0.0;
        }
        true
    }
    /// The smoothed SNR, dB.
    pub fn snr_db(&self) -> f64 {
        if self.present {
            self.snr
        } else {
            0.0
        }
    }
    /// The floor (linear).
    pub fn floor(&self) -> f64 {
        self.floor
    }
}

/// The AFC applies a correction when the carrier error is beyond this, Hz.
const AFC_ON_HZ: f64 = 3000.0;
/// The AFC takes its correction back when the carrier error is under this, Hz.
const AFC_OFF_HZ: f64 = 1500.0;
/// The largest correction of the AFC, Hz.
const AFC_MAX_HZ: f64 = 30e3;
/// The AFC looks for the signal this far from the corrected centre, Hz.
const AFC_SEARCH_HZ: f64 = 15e3;
/// An applied correction follows the estimate when the two differ by more than this, Hz.
const AFC_STEP_HZ: f64 = 250.0;

/// The coarse AFC, at 96 kHz. It measures the carrier error on the power spectrum of the corrected stream (the shared spectral
/// follow-up in open loop), which takes the noise floor out, so the measure holds at a level where the mean of a discriminator
/// reads a fraction of the error. A look that finds a signal while no transmission is read sets the estimate.
struct Afc {
    follow: SpectralFollow,
    frame: Vec<Complex32>,
    estimate: f64,
    applied: f64,
}

impl Afc {
    fn new() -> Afc {
        let cfg = SpectralConfig {
            search_hz: AFC_SEARCH_HZ,
            max_hz: AFC_MAX_HZ,
            ..SpectralConfig::open_loop(96000.0)
        };
        let follow = SpectralFollow::new(cfg).expect("the follow-up settings are valid");
        Afc {
            frame: Vec::with_capacity(follow.fft_len()),
            follow,
            estimate: 0.0,
            applied: 0.0,
        }
    }

    /// Takes one sample of the corrected stream; `busy` while a transmission is read.
    fn push(&mut self, z: Complex32, busy: bool) {
        self.frame.push(z);
        if self.frame.len() < self.follow.fft_len() {
            return;
        }
        let look = self.follow.process(&mut self.frame);
        self.frame.clear();
        let Some(look) = look else { return };
        if busy || !look.found {
            return;
        }
        self.estimate = (self.applied + look.centroid_hz).clamp(-AFC_MAX_HZ, AFC_MAX_HZ);
        let before = self.applied;
        if self.estimate.abs() < AFC_OFF_HZ {
            self.applied = 0.0;
        } else if self.estimate.abs() > AFC_ON_HZ
            && (self.estimate - self.applied).abs() > AFC_STEP_HZ
        {
            self.applied = self.estimate;
        }
        if self.applied != before {
            // The spectrum moves under the averages.
            self.follow.restart_averages();
        }
    }
}

/// `pager.front`: bad samples, mixer, decimation, resampler to 96 kHz, coarse AFC (`pager` only), channel filter and
/// decimation to 48 kHz, carrier detector. Outputs the channel at 48 kHz and the carrier state every 10 ms. With the AFC it has
/// two more inputs, the busy flags of the two decoders (`busy`, `narrow_busy`, feedback edges), and a second output `narrow`:
/// the same samples behind a 7 kHz channel filter for the narrow decoder, on a 25 kHz channel while [`NARROW_DECODER`] is on
/// (empty otherwise). The carrier state is present when either channel has a carrier; its SNR is the narrow channel's only
/// while the first has none.
pub struct PagerFront {
    afc_on: bool,
    half_channel: bool,
    narrow_on: bool,
    narrow_chan: FirDecimator<Complex32>,
    narrow_det: CarrierDetector,
    narrow_busy: bool,
    d: Vec<Complex32>,
    rate: f64,
    nco: Option<Nco>,
    decim: Option<Decimator<Complex32>>,
    res: Option<Resampler<Complex32>>,
    afc: Afc,
    afc_nco: Nco,
    chan: FirDecimator<Complex32>,
    det: CarrierDetector,
    busy: bool,
    in_power: f64,
    in_n: usize,
    a: Vec<Complex32>,
    b: Vec<Complex32>,
    c: Vec<Complex32>,
    input_dbfs: f32,
    n96: u64,
    afc_at: u64,
    chan_delay: u64,
    changes: Vec<AfcChange>,
}

/// A change of the AFC's correction, at the 48 kHz output index from which it acts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AfcChange {
    /// The 48 kHz output index.
    pub at: u64,
    /// The new correction, Hz.
    pub hz: f64,
}

impl Item for AfcChange {
    const KIND: &'static str = "pager.afc";
}

/// The channel filter of the narrow decoder: 7 kHz pass, 11 kHz stop, 60 dB, at 96 kHz.
fn narrow_taps() -> Vec<f32> {
    fir::lowpass(96000.0, 7000.0, 11000.0, 60.0).expect("the filter settings are valid")
}

fn channel_taps(narrow: bool) -> Vec<f32> {
    if narrow {
        fir::lowpass(96000.0, 6500.0, 9500.0, 60.0).expect("the filter settings are valid")
    } else {
        fir::lowpass(96000.0, 10000.0, 14000.0, 60.0).expect("the filter settings are valid")
    }
}

impl PagerFront {
    /// With the AFC (`pager`) or without (`multitone`).
    pub fn new(afc: bool) -> PagerFront {
        PagerFront {
            afc_on: afc,
            half_channel: false,
            narrow_on: afc,
            narrow_chan: FirDecimator::new(&narrow_taps(), 2),
            narrow_det: CarrierDetector::default(),
            narrow_busy: false,
            d: Vec::new(),
            rate: 0.0,
            nco: None,
            decim: None,
            res: None,
            afc: Afc::new(),
            afc_nco: Nco::new(96000.0, 0.0),
            chan: FirDecimator::new(&channel_taps(false), 2),
            det: CarrierDetector::default(),
            busy: false,
            in_power: 0.0,
            in_n: 0,
            a: Vec::new(),
            b: Vec::new(),
            c: Vec::new(),
            input_dbfs: -200.0,
            n96: 0,
            afc_at: 0,
            chan_delay: (channel_taps(false).len() as u64 - 1) / 2,
            changes: Vec::new(),
        }
    }
    /// The 12.5 kHz channel filter instead of the 25 kHz one.
    pub fn set_narrow(&mut self, narrow: bool) {
        if narrow != self.half_channel {
            self.half_channel = narrow;
            self.chan = FirDecimator::new(&channel_taps(narrow), 2);
            self.chan_delay = (channel_taps(narrow).len() as u64 - 1) / 2;
        }
    }
    /// Sets up for an input rate and channel offset.
    pub fn setup(&mut self, rate: f64, offset: f64) -> Result<(), ConfigError> {
        if rate < 96000.0 {
            return Err(ConfigError::RateTooLow {
                need: 96000.0,
                got: rate,
            });
        }
        self.rate = rate;
        self.nco = Some(Nco::new(rate, -offset));
        let d = pager_decimation(rate);
        let mid = rate / d as f64;
        self.decim = (d > 1).then(|| Decimator::new(rate, d, 38e3, 60.0));
        self.res = if (mid - 96000.0).abs() < 0.01 {
            None
        } else {
            Some(Resampler::with_quality(mid, 96000.0, 0.4, 60.0)?)
        };
        self.afc = Afc::new();
        self.afc_nco = Nco::new(96000.0, 0.0);
        self.chan = FirDecimator::new(&channel_taps(self.half_channel), 2);
        self.det = CarrierDetector::default();
        self.narrow_chan = FirDecimator::new(&narrow_taps(), 2);
        self.narrow_det = CarrierDetector::default();
        self.in_power = 0.0;
        self.in_n = 0;
        self.n96 = 0;
        self.afc_at = 0;
        Ok(())
    }
    /// Runs samples, appends the 48 kHz channel to `out` and the carrier states to `states` (with the 48 kHz index they end at).
    pub fn run(
        &mut self,
        iq: &[Complex32],
        out: &mut Vec<Complex32>,
        on_state: impl FnMut(usize, CarrierState),
    ) {
        let mut narrow = std::mem::take(&mut self.d);
        self.run_both(iq, out, &mut narrow, on_state);
        narrow.clear();
        self.d = narrow;
    }

    /// Whether the narrow decoder's channel is made: with the AFC, on a 25 kHz channel, while the setting is on.
    fn narrow_active(&self) -> bool {
        self.afc_on && self.narrow_on && !self.half_channel
    }

    /// As [`PagerFront::run`], and appends the narrow decoder's channel at 48 kHz to `narrow` while it is made.
    pub fn run_both(
        &mut self,
        iq: &[Complex32],
        out: &mut Vec<Complex32>,
        narrow: &mut Vec<Complex32>,
        mut on_state: impl FnMut(usize, CarrierState),
    ) {
        let Some(nco) = self.nco.as_mut() else {
            return;
        };
        self.a.clear();
        self.a.extend(iq.iter().map(|z| {
            if z.re.is_finite() && z.im.is_finite() && z.re.abs() <= 1e4 && z.im.abs() <= 1e4 {
                *z
            } else {
                Complex32::new(0.0, 0.0)
            }
        }));
        for z in &self.a {
            self.in_power += z.norm_sqr() as f64;
        }
        self.in_n += self.a.len();
        if self.in_n as f64 >= self.rate * 0.1 {
            let p = self.in_power / self.in_n as f64;
            self.input_dbfs = if p > 0.0 {
                (10.0 * p.log10()) as f32
            } else {
                -200.0
            };
            self.in_power = 0.0;
            self.in_n = 0;
        }
        nco.mix_in_place(&mut self.a);
        let src: &[Complex32] = if let Some(d) = self.decim.as_mut() {
            self.b.clear();
            d.process(&self.a, &mut self.b);
            &self.b
        } else {
            &self.a
        };
        let active = self.narrow_active();
        let at96: &mut Vec<Complex32> = &mut self.c;
        at96.clear();
        if let Some(r) = self.res.as_mut() {
            r.process(src, at96);
        } else {
            at96.extend_from_slice(src);
        }
        if self.afc_on {
            for z in at96.iter_mut() {
                // The AFC measures what is left after its own correction.
                *z *= self.afc_nco.next_sample();
                self.afc.push(*z, self.busy || self.narrow_busy);
                self.n96 += 1;
                let f = -self.afc.applied;
                if f != self.afc_nco.freq() {
                    self.afc_nco.set_freq(f);
                    self.afc_at = (self.n96 + self.chan_delay) / 2;
                    self.changes.push(AfcChange {
                        at: self.afc_at,
                        hz: self.afc.applied,
                    });
                }
            }
        }
        let start = out.len();
        self.chan.process(at96, out);
        let narrow_start = narrow.len();
        if active {
            self.narrow_chan.process(at96, narrow);
        }
        for (i, z) in out[start..].iter().enumerate() {
            if active {
                if let Some(y) = narrow.get(narrow_start + i) {
                    self.narrow_det.push(y.norm_sqr());
                }
            }
            if self.det.push(z.norm_sqr()) {
                let narrow_only = active && !self.det.present && self.narrow_det.present;
                on_state(
                    i,
                    CarrierState {
                        present: self.det.present || narrow_only,
                        snr_db: if narrow_only {
                            self.narrow_det.snr_db() as f32
                        } else {
                            self.det.snr_db() as f32
                        },
                        floor: self.det.floor() as f32,
                        power: self.det.power as f32,
                        input_dbfs: self.input_dbfs,
                        afc_hz: self.afc.applied as f32,
                        afc_at: self.afc_at,
                    },
                );
            }
        }
    }
    /// The AFC correction now, Hz.
    pub fn afc_hz(&self) -> f64 {
        self.afc.applied
    }
    /// Sets whether a transmission is being read (an input of the AFC).
    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    /// Turns the narrow decoder's channel on or off.
    pub fn set_narrow_decoder(&mut self, on: bool) {
        if on != self.narrow_on {
            self.narrow_on = on;
            self.narrow_chan = FirDecimator::new(&narrow_taps(), 2);
            self.narrow_det = CarrierDetector::default();
        }
    }
}

impl Component for PagerFront {
    fn info(&self) -> ComponentInfo {
        let mut inputs = vec![Port::iq("in")];
        let mut outputs = vec![
            Port::iq("out"),
            typed_port::<CarrierState>("carrier"),
            typed_port::<AfcChange>("afc"),
        ];
        if self.afc_on {
            inputs.push(Port::typed::<Busy>("busy"));
            inputs.push(Port::typed::<Busy>("narrow_busy"));
            outputs.push(Port::iq("narrow"));
        }
        ComponentInfo::processor("pager.front", inputs, outputs)
            .with_settings(PagerFrontSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.setup(f.rate, f.signal_offset_hz())?;
        let at48 = Format {
            rate: 48000.0,
            ..f.clone()
        };
        let mut out = vec![
            at48.clone(),
            Format::new(Kind::typed::<CarrierState>(), 0.0),
            Format::new(Kind::typed::<AfcChange>(), 0.0),
        ];
        if self.afc_on {
            out.push(at48);
        }
        Ok(out)
    }
    fn process(&mut self, io: &mut Io) {
        if self.afc_on {
            if let Some(b) = io.items::<Busy>(1).last() {
                self.busy = b.value.0;
            }
            if let Some(b) = io.items::<Busy>(2).last() {
                self.narrow_busy = b.value.0;
            }
        }
        let iq = io.iq(0);
        let first = io.output(0).first();
        let mut out = std::mem::take(io.iq_out(0));
        let mut narrow = if self.afc_on {
            std::mem::take(io.iq_out(3))
        } else {
            std::mem::take(&mut self.d)
        };
        let mut states = std::mem::take(io.items_out::<CarrierState>(1));
        let base = out.len();
        self.run_both(iq, &mut out, &mut narrow, |i, s| {
            states.push(Timed {
                time: (first + (base + i) as u64) as f64 / 48000.0,
                value: s,
            })
        });
        *io.iq_out(0) = out;
        if self.afc_on {
            *io.iq_out(3) = narrow;
        } else {
            narrow.clear();
            self.d = narrow;
        }
        *io.items_out::<CarrierState>(1) = states;
        let o = io.items_out::<AfcChange>(2);
        for c in self.changes.drain(..) {
            o.push(Timed {
                time: c.at as f64 / 48000.0,
                value: c,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PagerFrontSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PagerFrontSetting::Channel => {
                self.set_narrow(CHANNEL.read(id, v)? == ChannelWidth::Narrow)
            }
            PagerFrontSetting::NarrowDecoder => {
                self.set_narrow_decoder(NARROW_DECODER.read(id, v)?)
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let w = if self.half_channel {
            ChannelWidth::Narrow
        } else {
            ChannelWidth::Wide
        };
        values_of([
            (PagerFrontSetting::Channel, Value::Choice(w.index())),
            (
                PagerFrontSetting::NarrowDecoder,
                Value::Bool(self.narrow_on),
            ),
        ])
    }
}

sdrkit_core::choice_enum! {
    /// The channel filter of the paging front end.
    pub enum ChannelWidth {
        /// A 25 kHz channel.
        Wide = ("25k", "25 kHz"),
        /// A 12.5 kHz channel: it cuts a neighbour better but needs the signal on the channel centre.
        Narrow = ("12.5k", "12.5 kHz"),
    }
}

/// The channel filter.
pub const CHANNEL: Setting<field::Choice<ChannelWidth>> =
    Setting::choice("channel", "Channel", ChannelWidth::Wide)
        .help("12.5 kHz cuts a neighbour better but needs the signal on the channel centre.");

/// A second decoder behind a 7 kHz channel filter, on a 25 kHz channel.
pub const NARROW_DECODER: Setting<field::Bool> =
    Setting::new("narrow_decoder", "Narrow second decoder", "", true).help(
        "On a 25 kHz channel, a second POCSAG and FLEX decoder behind a 7 kHz filter reads weak pages the first misses.",
    );

sdrkit_core::settings! {
    /// The settings of `pager.front`.
    pub enum PagerFrontSetting {
        /// The channel filter.
        Channel = CHANNEL,
        /// The second decoder.
        NarrowDecoder = NARROW_DECODER,
    }
}

/// The phase step of `z[n] conj(z[n-1])` in rad/sample, limited to +-1.4 rad (about 10.7 kHz at 48 kHz): a larger step is a
/// click of noise, not deviation, and is limited so that it does not add a whole turn to the symbol it falls in.
#[inline]
pub fn pager_phase_step(z: Complex32, last: Complex32) -> f32 {
    let d = z * last.conj();
    atan2(d.im, d.re).clamp(-1.4, 1.4)
}

/// `pager.disc`: [`pager_phase_step`] at 48 kHz (the discriminator of the sequential calls).
#[derive(Default)]
pub struct PagerDisc {
    last: Complex32,
}

impl Component for PagerDisc {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "pager.disc",
            vec![Port::iq("in")],
            vec![Port::real("out").same_format_as(0)],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.last = Complex32::new(0.0, 0.0);
        Ok(vec![inputs[0].with_kind(Kind::Real)])
    }
    fn process(&mut self, io: &mut Io) {
        let iq = io.iq(0);
        let mut last = self.last;
        let out = io.real_out(0);
        out.reserve(iq.len());
        for &z in iq {
            out.push(pager_phase_step(z, last));
            last = z;
        }
        self.last = last;
    }
    fn values(&self) -> Values {
        no_values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimation_choice() {
        assert_eq!(pager_decimation(2e6), 20);
        assert_eq!(pager_decimation(192e3), 2);
        assert_eq!(pager_decimation(96e3), 1);
        assert_eq!(pager_decimation(1e6), 10);
    }

    #[test]
    fn nfm_front_reads_a_deviation() {
        let rate = 240e3;
        let x: Vec<Complex32> = crate::fmtx::fm(&vec![1.0; 480000], rate, 1000.0, -50e3)
            .unwrap()
            .into_iter()
            .map(|z| z * 0.5)
            .collect();
        let mut f = NfmFront::new(9600.0, 4500.0);
        f.setup(rate, -50e3).unwrap();
        let mut out = Vec::new();
        f.run(&x, &mut out);
        let tail = &out[out.len() / 2..];
        let mean = tail.iter().sum::<f32>() / tail.len() as f32;
        assert!((mean - 1000.0).abs() < 5.0, "{mean}");
        assert!((f.level_dbfs() + 6.0).abs() < 1.0, "{}", f.level_dbfs());
    }
}
