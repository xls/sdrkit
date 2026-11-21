//! `dvbt.input`: the channel width in force, removal of non-finite samples and DC, the channel locator for an off-centre
//! channel, and the resampler to the natural rate with sample-clock steering.

use crate::bandwidth::{initial_guess, next_width, BandwidthDetector, WidthSource, WIDTH_TRY_S};
use crate::items::{Link, Restart};
use crate::level::{LevelSteps, HUNTING_STEPS};
use crate::params::{Bandwidth, Mode};
use crate::settings::{BandwidthChoice, BANDWIDTH, CHANNELS, CHANNELS_MAX};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_dsp::DspError;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Complex32, ConfigError, HostRequest, Results, Setting, SettingError, SettingId,
    SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, TagKind,
    TagSource, Timed,
};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// A receiver within this distance of the centre of a channel is on that channel, Hz.
const ON_CHANNEL_HZ: f64 = 100e3;
/// The DC estimate is updated per group of this many samples.
const DC_GROUP: usize = 1024;
/// At the natural rate, a total clock correction below this leaves the resampler out, ppm. The timing loop of
/// `dvbt.sync` sends a correction only for a drift above 2 ppm; a drift below that it follows with whole-sample steps
/// of the symbol window.
pub const BYPASS_PPM: f64 = 1.0;
/// The resampler is left only when its next output is within this of an input sample, in samples.
const LEAVE_WITHIN: f64 = 1e-3;
/// The fewest newest input samples kept to start and to leave a resampler without a jump.
const KEEP: usize = 64;
/// The locator's FFT size.
const LOC_FFT: usize = 1024;
/// Spectra averaged per locator decision.
const LOC_AVG: usize = 32;

/// The channel locator: it finds the band of a channel that is not at the centre of the input.
#[derive(Clone)]
pub struct Locator {
    fft: Fft,
    window: Vec<f32>,
    buf: Vec<Complex32>,
    acc: Vec<f32>,
    frames: usize,
    hop: usize,
    since: usize,
    fill: Vec<Complex32>,
    rate: f64,
    occupied_hz: f64,
}

impl Locator {
    /// A locator for input at `rate` and a channel `occupied_hz` wide.
    pub fn new(rate: f64, occupied_hz: f64) -> Self {
        let window = (0..LOC_FFT)
            .map(|i| {
                (0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / LOC_FFT as f64).cos()) as f32
            })
            .collect();
        Locator {
            fft: Fft::forward(LOC_FFT),
            window,
            buf: vec![Complex32::default(); LOC_FFT],
            acc: vec![0.0; LOC_FFT],
            frames: 0,
            hop: ((0.0015 * rate) as usize).max(LOC_FFT),
            since: 0,
            fill: Vec::with_capacity(LOC_FFT),
            rate,
            occupied_hz,
        }
    }

    /// Feeds samples (after DC removal, before the mixer). Returns the centre of the channel (Hz from the input centre) when a
    /// decision is due and accepted, and the in-band power.
    pub fn push(&mut self, x: &[Complex32]) -> Option<(f64, f64)> {
        let mut result = None;
        for &s in x {
            if self.since < self.hop.saturating_sub(LOC_FFT) {
                self.since += 1;
                continue;
            }
            self.fill.push(s);
            if self.fill.len() == LOC_FFT {
                for (i, b) in self.buf.iter_mut().enumerate() {
                    *b = self.fill[i] * self.window[i];
                }
                self.fill.clear();
                self.since = 0;
                self.fft.process(&mut self.buf);
                for (a, b) in self.acc.iter_mut().zip(&self.buf) {
                    *a += b.norm_sqr();
                }
                self.frames += 1;
                if self.frames == LOC_AVG {
                    result = self.decide();
                    self.acc.iter_mut().for_each(|a| *a = 0.0);
                    self.frames = 0;
                }
            }
        }
        result
    }

    fn decide(&self) -> Option<(f64, f64)> {
        // The spectrum from the lowest frequency to the highest.
        let n = LOC_FFT;
        let spec: Vec<f32> = (0..n).map(|i| self.acc[(i + n / 2) % n]).collect();
        let wb = ((self.occupied_hz / self.rate * n as f64).round() as usize).clamp(8, n);
        let mut prefix = vec![0f64; n + 1];
        for i in 0..n {
            prefix[i + 1] = prefix[i] + spec[i] as f64;
        }
        let mut best = (0usize, -1.0f64);
        for s in 0..=(n - wb) {
            let p = prefix[s + wb] - prefix[s];
            if p > best.1 {
                best = (s, p);
            }
        }
        let (s, p) = best;
        let inside = p / wb as f64;
        let strip = (wb / 25).max(2);
        let mean = |a: usize, b: usize| (prefix[b] - prefix[a]) / (b - a) as f64;
        let left_ok = if s >= 2 + strip {
            mean(s - 2 - strip, s - 2) < inside / 4.0
        } else {
            true
        };
        let right_ok = if s + wb + 2 + strip <= n {
            mean(s + wb + 2, s + wb + 2 + strip) < inside / 4.0
        } else {
            true
        };
        if !(left_ok && right_ok) {
            return None;
        }
        let centre_bin = s as f64 + wb as f64 / 2.0 - n as f64 / 2.0;
        let power = p / (n as f64 * n as f64 * LOC_AVG as f64 * 0.375);
        Some((centre_bin * self.rate / n as f64, power))
    }
}

/// The frequencies of a channel list: Hz separated by ";". `None` for a malformed text or more than [`CHANNELS_MAX`]
/// entries.
fn parse_channels(text: &str) -> Option<Vec<f64>> {
    let hz: Vec<f64> = text
        .split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|e| e.parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0))
        .collect::<Option<_>>()?;
    (hz.len() <= CHANNELS_MAX).then_some(hz)
}

/// The channel locator is on.
pub const LOCATOR: Setting<field::Bool> = Setting::new("locator", "Channel locator", "", true);

sdrkit_core::settings! {
    /// The settings of `dvbt.input`.
    pub enum InputSetting {
        /// The channel bandwidth.
        Bandwidth = BANDWIDTH,
        /// The channel list.
        Channels = CHANNELS,
        /// The channel locator.
        Locator = LOCATOR,
    }
}

/// `dvbt.input`.
pub struct DvbtInput {
    link: Arc<Link>,
    choice: BandwidthChoice,
    channels_text: String,
    channels: Vec<f64>,
    tuned_hz: f64,
    bandwidth: Bandwidth,
    source: WidthSource,
    found: bool,
    tried: Vec<Bandwidth>,
    since_width: u64,
    detector: Option<BandwidthDetector>,
    /// The measured width brought no lock in the time a width has, and the spectrum is measured again.
    remeasure: bool,
    /// Set when this stage requested a new configure to change the width: the width search goes on.
    reformatting: bool,
    /// The channel list changed: the receiver goes to a channel of the list at the next block if it is on none.
    to_channel: bool,
    in_rate: f64,
    nat_rate: f64,
    dc: Complex32,
    dc_acc: Complex32,
    dc_n: usize,
    dc_weight: f32,
    locator: Option<Locator>,
    locator_on: bool,
    offset_hz: f64,
    nco: Option<Nco>,
    resampler: Option<FractionalResampler<Complex32>>,
    correction: f64,
    /// The time of the next output of the resampler, in input samples after the next input.
    ahead: f64,
    /// How many of the newest input samples `recent` holds.
    keep: usize,
    /// The host was asked for the natural rate of the measured width since the last configure.
    asked_rate: bool,
    clean: Vec<Complex32>,
    power: f64,
    power_n: usize,
    steps: LevelSteps,
    /// The hunting gain control was reported since the last configure.
    steps_told: bool,
    /// Non-finite samples replaced.
    pub scrubbed: u64,
    p: Published,
    restart_pending: bool,
    restarts: TagSource<Restart>,
    recent: Vec<Complex32>,
}

impl Default for DvbtInput {
    fn default() -> Self {
        DvbtInput::new(Link::new())
    }
}

impl DvbtInput {
    /// The input stage at the default settings, sharing `link`.
    pub fn new(link: Arc<Link>) -> Self {
        DvbtInput {
            link,
            choice: BandwidthChoice::Auto,
            channels_text: String::new(),
            channels: Vec::new(),
            tuned_hz: 0.0,
            bandwidth: Bandwidth::B8,
            source: WidthSource::Setting,
            found: false,
            tried: Vec::with_capacity(Bandwidth::ALL.len()),
            since_width: 0,
            detector: None,
            remeasure: false,
            reformatting: false,
            to_channel: false,
            in_rate: 0.0,
            nat_rate: Bandwidth::B8.natural_rate(),
            dc: Complex32::default(),
            dc_acc: Complex32::default(),
            dc_n: 0,
            dc_weight: 1.0,
            locator: None,
            locator_on: true,
            offset_hz: 0.0,
            nco: None,
            resampler: None,
            correction: 1.0,
            ahead: 0.0,
            keep: KEEP,
            asked_rate: false,
            clean: Vec::new(),
            power: 0.0,
            power_n: 0,
            steps: LevelSteps::default(),
            steps_told: false,
            scrubbed: 0,
            p: Published::new(&[]),
            restart_pending: false,
            restarts: TagSource::new(),
            recent: Vec::new(),
        }
    }

    /// The input stage with the bandwidth fixed to `bandwidth`, sharing `link`.
    pub fn with_bandwidth(link: Arc<Link>, bandwidth: Bandwidth) -> Self {
        DvbtInput {
            choice: BandwidthChoice::of(bandwidth),
            bandwidth,
            nat_rate: bandwidth.natural_rate(),
            ..DvbtInput::new(link)
        }
    }

    /// The bandwidth in use.
    pub fn bandwidth(&self) -> Bandwidth {
        self.bandwidth
    }

    /// The occupied width of the channel (K carriers).
    pub fn occupied_hz(&self) -> f64 {
        Mode::K8.carriers() as f64 * self.nat_rate / Mode::K8.fft() as f64
    }

    /// Requests the host to tune to the lowest channel of the list, unless the receiver is on a channel of the list.
    fn move_to_channel(&mut self, io: &mut Io) {
        let on_list = self
            .channels
            .iter()
            .any(|c| (c - self.tuned_hz).abs() <= ON_CHANNEL_HZ);
        let Some(lowest) = self.channels.iter().copied().reduce(f64::min) else {
            return;
        };
        if on_list || self.tuned_hz <= 0.0 {
            return;
        }
        let time = io.input(0).first() as f64 / self.in_rate.max(1.0);
        io.items_out::<HostRequest>(1).push(Timed {
            time,
            value: HostRequest::Retune {
                tuned_hz: lowest,
                reason: "the lowest channel of the list".into(),
            },
        });
    }

    /// Picks the width to start with at `rate`: the setting, else a guess.
    fn start_bandwidth(&mut self, rate: f64) {
        self.found = false;
        self.remeasure = false;
        self.tried.clear();
        self.since_width = 0;
        match self.choice.bandwidth() {
            Some(b) => {
                self.bandwidth = b;
                self.source = WidthSource::Setting;
            }
            None => {
                self.bandwidth = initial_guess(rate);
                self.source = WidthSource::Guess;
            }
        }
        // The spectrum is measured either way: the occupancy is reported, and the width is used only with Auto.
        self.detector = Some(BandwidthDetector::new(rate));
    }

    /// Makes the resampler to the natural rate. It keeps only the occupied band clean: the stop band starts where aliases or
    /// images fold into it, 48 dB down.
    fn build_resampler(&mut self) -> Result<(), DspError> {
        let mid_stream = self.resampler.is_none() && !self.recent.is_empty();
        let w = self.occupied_hz();
        if self.at_natural_rate() && self.correction == 1.0 {
            self.resampler = None;
            return Ok(());
        }
        let low = self.in_rate.min(self.nat_rate);
        let pass = (w / 2.0).min(0.47 * low);
        let stop = (low - pass).max(pass * 1.05);
        let mut r = FractionalResampler::new(self.in_rate, self.nat_rate, pass, stop, 48.0)?;
        r.set_correction(self.correction);
        self.ahead = -r.delay();
        self.keep = KEEP.max(r.delay().ceil() as usize + 8);
        if mid_stream {
            // Switched in for the clock correction: the recent samples fill the history, and the next output falls on the
            // next input with the filter delay removed, so there is no jump in time and no transient.
            let mut primed = Vec::new();
            r.process(&self.recent, &mut primed);
            r.set_start(0.0)?;
            self.ahead = 0.0;
        }
        self.resampler = Some(r);
        Ok(())
    }

    /// True if the input has the natural rate of the width.
    fn at_natural_rate(&self) -> bool {
        (self.in_rate - self.nat_rate).abs() < 1e-6 * self.nat_rate
    }

    /// Keeps the newest samples of `x` in `recent`.
    fn remember(&mut self, x: std::ops::Range<usize>) {
        let x = &self.clean[x];
        if x.len() >= self.keep {
            self.recent.clear();
            self.recent.extend_from_slice(&x[x.len() - self.keep..]);
        } else {
            self.recent.extend_from_slice(x);
            let n = self.recent.len();
            if n > self.keep {
                self.recent.drain(..n - self.keep);
            }
        }
    }

    /// Resamples `clean[x]` to `out` and keeps the time of the next output.
    fn resample(&mut self, x: std::ops::Range<usize>, out: &mut Vec<Complex32>) {
        let Some(r) = self.resampler.as_mut() else {
            return;
        };
        let before = out.len();
        r.process(&self.clean[x.clone()], out);
        let made = (out.len() - before) as f64;
        self.ahead += made * self.correction * self.in_rate / self.nat_rate - x.len() as f64;
    }

    /// How many of the next `n` input samples the resampler takes before its output falls on an input sample, at the
    /// natural rate with a clock correction below [`BYPASS_PPM`]. `None` if that is not within `n` samples.
    fn until_leave(&self, n: usize) -> Option<usize> {
        let step = self.correction - 1.0;
        if !self.at_natural_rate() || step.abs() * 1e6 >= BYPASS_PPM {
            return None;
        }
        let off = self.ahead - self.ahead.round();
        if off.abs() <= LEAVE_WITHIN {
            return Some(0);
        }
        if step == 0.0 {
            return None;
        }
        // The output moves `step` input samples against the input with each sample.
        let to_go = if (off > 0.0) == (step > 0.0) {
            1.0 - off.abs()
        } else {
            off.abs()
        };
        let samples = (to_go / step.abs()).ceil();
        (samples <= n as f64).then_some(samples as usize)
    }

    /// Takes the resampler out where its next output falls on an input sample: the kept input samples that it has not
    /// given yet go to `out` as they are. The time of the samples has no jump. False if it stays.
    fn leave_resampler(&mut self, out: &mut Vec<Complex32>) -> bool {
        let off = self.ahead - self.ahead.round();
        let held = -self.ahead.round();
        if off.abs() > LEAVE_WITHIN || held < 0.0 || held > self.recent.len() as f64 {
            return false;
        }
        out.extend_from_slice(&self.recent[self.recent.len() - held as usize..]);
        self.resampler = None;
        self.correction = 1.0;
        self.link.set_clock_ppm(0.0);
        true
    }

    /// With the width on Auto, asks the host once for the natural rate of the width that the stack measured and
    /// locked on, if the input has another rate: the input stage then needs no resampler.
    fn ask_natural_rate(&mut self, io: &mut Io) {
        let wanted = self.choice == BandwidthChoice::Auto
            && !self.asked_rate
            && self.in_rate > 0.0
            && !self.at_natural_rate()
            && self.link.locked();
        if !wanted {
            return;
        }
        self.asked_rate = true;
        let time = io.input(0).first() as f64 / self.in_rate;
        io.items_out::<HostRequest>(1).push(Timed {
            time,
            value: HostRequest::Tune {
                session: String::new(),
                freq_hz: None,
                target: sdrkit_core::TuneTarget::Running,
                submode: String::new(),
                rate_hz: Some(self.nat_rate),
                bandwidth_hz: None,
                settings: Vec::new(),
            },
        });
    }

    /// The level in dBFS since the last call.
    pub fn take_level_dbfs(&mut self) -> Option<f32> {
        if self.power_n == 0 {
            return None;
        }
        let p = self.power / self.power_n as f64;
        self.power = 0.0;
        self.power_n = 0;
        Some((10.0 * p.max(1e-20).log10()) as f32)
    }

    fn reset_state(&mut self, keep_dc: bool) {
        if !keep_dc {
            self.dc = Complex32::default();
        }
        self.dc_acc = Complex32::default();
        self.dc_n = 0;
        self.locator = (self.locator_on && self.in_rate > 0.0)
            .then(|| Locator::new(self.in_rate, self.occupied_hz()));
    }

    /// The width decision of one pass of input samples `x`, the only one of the receiver: the width measured from
    /// the spectrum while nothing is locked; while the spectrum gives none, the other widths in turn. Returns a new
    /// width when the graph must change to it.
    fn choose_width(&mut self, x: &[Complex32]) -> Option<Bandwidth> {
        let auto = self.choice == BandwidthChoice::Auto;
        let measuring = auto && (self.source != WidthSource::Measured || self.remeasure);
        let unlocked = self.link.state.load(Ordering::Relaxed) < 2;
        let mut change = None;
        if let Some(d) = self.detector.as_mut() {
            // The occupancy is measured once even when the channel locks before the first estimate.
            if (measuring && unlocked) || d.occupancy_db.is_none() {
                if let Some(b) = d.push(x) {
                    if measuring && unlocked {
                        change = Some((b, WidthSource::Measured));
                    }
                }
            }
        }
        self.since_width += x.len() as u64;
        // A measured width that brings no lock is measured again on new samples: the first spectrum may be of samples
        // from before a change of the input.
        if auto
            && unlocked
            && self.source == WidthSource::Measured
            && self.since_width as f64 > WIDTH_TRY_S * self.in_rate
        {
            if let Some(d) = self.detector.as_mut() {
                d.reset();
            }
            self.remeasure = true;
            self.since_width = 0;
        }
        // A measured width stays: the other widths are tried only while the spectrum gave none.
        let trying = auto && self.source != WidthSource::Measured;
        if !unlocked {
            self.tried.clear();
            self.since_width = 0;
        } else if change.is_none() && trying && self.since_width as f64 > WIDTH_TRY_S * self.in_rate
        {
            // No lock at this width: the next standard width that the rate allows, nearest first, then round again.
            if !self.tried.contains(&self.bandwidth) {
                self.tried.push(self.bandwidth);
            }
            let next = next_width(self.bandwidth, &self.tried, self.in_rate).or_else(|| {
                self.tried.clear();
                self.tried.push(self.bandwidth);
                next_width(self.bandwidth, &self.tried, self.in_rate)
            });
            match next {
                Some(b) => change = Some((b, WidthSource::Tried)),
                None => self.since_width = 0,
            }
        }
        let (b, source) = change?;
        if std::mem::take(&mut self.remeasure) && b == self.bandwidth {
            self.since_width = 0;
            return None;
        }
        let r = self.p.results_mut();
        match source {
            WidthSource::Measured => {
                let w = self
                    .detector
                    .as_ref()
                    .and_then(|d| d.width_hz)
                    .unwrap_or(0.0);
                r.log(format!(
                    "signal is {:.2} MHz wide: {} MHz channel",
                    w / 1e6,
                    b.mhz()
                ));
            }
            _ => r.log(format!(
                "no lock at {} MHz: trying {} MHz",
                self.bandwidth.mhz(),
                b.mhz()
            )),
        }
        let old = self.bandwidth;
        self.bandwidth = b;
        self.found = true;
        self.source = source;
        self.since_width = 0;
        (b != old).then_some(b)
    }

    fn report_width(&self) {
        if let Ok(mut t) = self.link.telemetry.lock() {
            let w = &mut t.width;
            w.bandwidth = Some(self.bandwidth);
            w.source = Some(self.source);
            w.auto = self.choice == BandwidthChoice::Auto;
            w.measured_hz = self.detector.as_ref().and_then(|d| d.width_hz);
            w.occupancy_db = self.detector.as_ref().and_then(|d| d.occupancy_db);
            w.input_rate = self.in_rate;
            w.natural_rate = self.nat_rate;
            if self.power_n > 0 {
                t.level_dbfs =
                    Some((10.0 * (self.power / self.power_n as f64).max(1e-20).log10()) as f32);
            }
            t.scrubbed = self.scrubbed;
        }
    }
}

impl Component for DvbtInput {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbt.input",
            vec![Port::iq("in")],
            vec![
                Port::iq("out"),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(InputSetting::schema())
        .manual_tags()
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.in_rate = f.rate;
        self.tuned_hz = f.tuned_hz;
        if std::mem::take(&mut self.reformatting) {
            self.link.request_restart();
        } else {
            self.start_bandwidth(f.rate);
            self.link.reset();
            if let Ok(mut t) = self.link.telemetry.lock() {
                let width = std::mem::take(&mut t.width);
                *t = crate::items::Telemetry {
                    width,
                    ..Default::default()
                };
            }
        }
        let need = self.choice.bandwidth().unwrap_or(Bandwidth::B5).min_rate();
        if f.rate < need * 0.9999 {
            return Err(ConfigError::RateTooLow { need, got: f.rate });
        }
        if f.rate < self.bandwidth.min_rate() * 0.9999 {
            self.bandwidth = Bandwidth::ALL
                .into_iter()
                .rev()
                .find(|b| b.min_rate() <= f.rate * 1.0001)
                .unwrap_or(Bandwidth::B5);
        }
        self.nat_rate = self.bandwidth.natural_rate();
        self.dc_weight = (DC_GROUP as f64 / (0.05 * f.rate)).min(1.0) as f32;
        self.offset_hz = 0.0;
        self.link.set_locator_hz(0.0);
        self.nco = None;
        self.correction = 1.0;
        self.link.set_clock_ppm(0.0);
        self.steps = LevelSteps::default();
        self.steps_told = false;
        self.resampler = None;
        self.recent.clear();
        self.asked_rate = false;
        self.build_resampler()?;
        self.reset_state(false);
        self.report_width();
        Ok(vec![
            Format {
                rate: self.nat_rate,
                bandwidth_hz: self.occupied_hz(),
                ..f.clone()
            },
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        // A retune: everything restarts.
        let retuned = io
            .input(0)
            .tags()
            .iter()
            .any(|t| matches!(t.kind, TagKind::Retuned { .. }));
        if retuned {
            self.offset_hz = 0.0;
            self.link.set_locator_hz(0.0);
            self.nco = None;
            self.correction = 1.0;
            self.link.set_clock_ppm(0.0);
            self.steps = LevelSteps::default();
            self.steps_told = false;
            // A resampler that configure accepted builds again at the same rates.
            let _ = self.build_resampler();
            self.reset_state(false);
            self.restart_pending = true;
        }
        if std::mem::take(&mut self.to_channel) {
            self.move_to_channel(io);
        }
        self.ask_natural_rate(io);
        if self.choose_width(io.iq(0)).is_some() {
            self.reformatting = true;
            io.request_reformat();
        }
        let x = io.iq(0);
        // Clock steering from the timing loop.
        let d = self.link.take_clock();
        let corrected = d != 0.0;
        if corrected {
            self.correction *= 1.0 + d * 1e-6;
            self.link.set_clock_ppm((self.correction - 1.0) * 1e6);
            match self.resampler.as_mut() {
                Some(r) => r.set_correction(self.correction),
                // A resampler that configure accepted builds again at the same rates.
                None => {
                    let _ = self.build_resampler();
                }
            }
        }
        if self.link.locked() {
            self.locator = None;
        } else if self.locator.is_none() && self.locator_on {
            self.locator = Some(Locator::new(self.in_rate, self.occupied_hz()));
        }
        self.clean.clear();
        self.clean.reserve(x.len());
        let mut dc_acc = self.dc_acc;
        let mut dc_n = self.dc_n;
        for &s in x {
            let s = if s.re.is_finite() && s.im.is_finite() {
                s
            } else {
                self.scrubbed += 1;
                Complex32::default()
            };
            dc_acc += s;
            dc_n += 1;
            let y = s - self.dc;
            self.power += y.norm_sqr() as f64;
            self.power_n += 1;
            self.clean.push(y);
            if dc_n == DC_GROUP {
                let mean = dc_acc / DC_GROUP as f32;
                self.dc += (mean - self.dc) * self.dc_weight;
                dc_acc = Complex32::default();
                dc_n = 0;
            }
        }
        self.dc_acc = dc_acc;
        self.dc_n = dc_n;
        if let Some((steps, db)) = self.steps.push(&self.clean, self.in_rate) {
            if steps >= HUNTING_STEPS && !self.steps_told {
                self.steps_told = true;
                self.p.results_mut().log(format!(
                    "the input level steps {steps} times a second by up to {db:.1} dB: the gain control of the radio \
                     hunts, hold its gain"
                ));
            }
        }
        if let Some(loc) = self.locator.as_mut() {
            if let Some((centre, _p)) = loc.push(&self.clean) {
                if (centre - self.offset_hz).abs() > 40e3 {
                    self.offset_hz = centre;
                    self.link.set_locator_hz(centre);
                    self.nco = Some(Nco::new(self.in_rate, -centre));
                    self.restart_pending = true;
                    self.p.results_mut().log(format!(
                        "channel found {:.0} kHz from the centre",
                        centre / 1e3
                    ));
                }
            }
        }
        if let Some(n) = self.nco.as_mut() {
            n.mix_in_place(&mut self.clean);
        }
        let out_first = io.output(0).first();
        if corrected {
            self.link.set_clock_from(out_first);
        }
        let meter = io.meter();
        let out = io.iq_out(0);
        let n = self.clean.len();
        if self.resampler.is_some() {
            let _timer = meter.section("resample");
            // At the natural rate with a small correction the resampler goes out at the next sample where that makes
            // no jump.
            let leave = self.until_leave(n);
            let first = leave.unwrap_or(n);
            self.resample(0..first, out);
            if self.at_natural_rate() {
                self.remember(0..first);
            }
            if leave.is_some() && self.leave_resampler(out) {
                out.extend_from_slice(&self.clean[first..]);
                self.remember(first..n);
            } else {
                self.resample(first..n, out);
                if self.at_natural_rate() {
                    self.remember(first..n);
                }
            }
        } else {
            out.extend_from_slice(&self.clean);
            // The newest samples, to start a resampler without a jump when the clock correction needs one.
            self.remember(0..n);
        }
        self.report_width();
        self.power = 0.0;
        self.power_n = 0;
        if self.restart_pending {
            self.restart_pending = false;
            let tag = TagKind::Typed(self.restarts.tag(Restart));
            io.output(0).push_tag(out_first, tag);
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match InputSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            InputSetting::Bandwidth => {
                self.choice = BANDWIDTH.read(id, v)?;
                Ok(Applied::Reformat)
            }
            InputSetting::Channels => {
                let t = CHANNELS.read(id, v)?;
                self.channels =
                    parse_channels(t).ok_or(SettingError::Malformed { id, at: None })?;
                t.clone_into(&mut self.channels_text);
                self.to_channel = true;
                Ok(Applied::Live)
            }
            InputSetting::Locator => {
                self.locator_on = LOCATOR.read(id, v)?;
                if !self.locator_on {
                    self.locator = None;
                }
                Ok(Applied::Live)
            }
        }
    }

    fn values(&self) -> Values {
        values_of([
            (InputSetting::Bandwidth, Value::Choice(self.choice.index())),
            (
                InputSetting::Channels,
                Value::Text(self.channels_text.clone()),
            ),
            (InputSetting::Locator, Value::Bool(self.locator_on)),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}
