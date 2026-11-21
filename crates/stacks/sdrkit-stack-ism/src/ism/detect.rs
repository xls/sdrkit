//! The frequency-resolved burst detector (`ism.burstdetect`), the burst channeliser (`ism.channelise`) and the pulse detector on
//! each burst (`ism.pulses`).

use super::analyser::{Submode, SubmodeSetting, SUBMODE};
use crate::typed::IsmDetectFields;
use sdrkit_algo_dsp::fir::Convolver;
use sdrkit_algo_modem::bandburst::{
    BandConfig, BandDetector, BusyFloor, FloorRule, Grouping, LongBurst,
};
use sdrkit_algo_modem::burstmix::BurstMixer;
use sdrkit_algo_modem::fsk::TonePair;
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Complex32, ConfigError, Results, Setting, SettingError, SettingId, SettingSet, Value,
    Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Port,
    RateFrom, Timed,
};
use sdrkit_proto_ookdev::{
    burst_tones, level_train, tone_train, Decoding, DetectorConfig, Modulation, ProtocolList,
    Pulse, PulseDetector, Reading, Train,
};
use std::sync::Arc;

/// One burst (`ism.burst`): its samples, the frequency of its signal and the noise level around it.
#[derive(Clone, Debug, Default)]
pub struct IsmBurst {
    /// The samples at `rate`: at the input rate with the signal at `offset_hz` before the channeliser, at 0 Hz after it.
    pub samples: Vec<Complex32>,
    /// The rate of `samples`, Hz.
    pub rate: f64,
    /// The signal's centre, Hz from the input centre.
    pub offset_hz: f64,
    /// The input sample index of the burst's first sample.
    pub start: u64,
    /// The input rate, Hz.
    pub input_rate: f64,
    /// Samples of `samples` before the detector saw the burst start (quiet samples for the noise floor).
    pub lead: usize,
}

impl Item for IsmBurst {
    const KIND: &'static str = "ism.burst";
}

/// The detection threshold.
pub const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold", "Detection threshold", "dB", 13.0)
        .range(8.0, 25.0, 1.0)
        .help("How far above the noise floor of a frequency bin a burst must rise.");

sdrkit_core::settings! {
    /// The settings of [`BurstDetect`].
    pub enum BurstDetectSetting {
        /// The detection threshold.
        Threshold = THRESHOLD,
        /// The submode.
        Submode = SUBMODE,
    }
}

/// The samples kept after the last lit frame of a burst, seconds: 0.5 ms for the device decoders, 5 ms for the pulse
/// analyser. The bins next to 0 Hz are left out of the detection, so a pulse of a carrier at the centre lights only the frames
/// of its edges, and a longer tail keeps the whole of the last pulse.
pub fn post_s(submode: Submode) -> f64 {
    match submode {
        Submode::Devices => 0.5e-3,
        Submode::PulseAnalyser => 5e-3,
    }
}

/// Finds bursts anywhere in the input band with short FFTs and a noise floor for each bin
/// (`sdrkit_algo_modem::bandburst`, one burst over the band), and outputs up to three signals of each burst with their centre
/// frequencies.
pub struct BurstDetect {
    threshold_db: f64,
    submode: Submode,
    rate: f64,
    det: Option<BandDetector>,
    /// The stream index of the detector's first sample (`None` until the first piece after a configure).
    base: Option<u64>,
    level_acc: f64,
    level_n: u64,
    published: Published,
    out: Vec<Timed<IsmBurst>>,
}

impl Default for BurstDetect {
    fn default() -> Self {
        BurstDetect {
            threshold_db: THRESHOLD.default,
            submode: SUBMODE.default,
            rate: 0.0,
            det: None,
            base: None,
            level_acc: 0.0,
            level_n: 0,
            published: Published::new(IsmDetectFields::SHAPE.fields),
            out: vec![],
        }
    }
}

/// The detector settings of the ISM bursts at `fs`: frames of the smallest power of two from 64 that spans 512 µs (at most
/// 4096; a mark of 500 µs at 4 dB in 100 kHz is then 19 dB over the floor of its bin, against 13 dB with 128 µs), no overlap, a floor that learns over 30 frames, bins next to 0 Hz left out, a burst closed after 25 ms of quiet (at
/// least 4 frames), a carrier that stays longer than three quarters of the history taken into the floor, 1 ms before and
/// `post_s` after each burst, three signals at most.
pub fn band_config(fs: f64, threshold_db: f64, post_s: f64) -> BandConfig {
    let mut n = 64;
    while n < 4096 && (n as f64) < fs * 512e-6 {
        n *= 2;
    }
    let history = ((1.2 * fs).ceil() as usize)
        .next_power_of_two()
        .min(1 << 23);
    BandConfig {
        rate_hz: fs,
        frame: n,
        hop: n,
        smooth: false,
        dc_guard: 1,
        threshold_db,
        floor: FloorRule {
            learn_frames: 30,
            learn_alpha: 0.2,
            start_median: false,
            idle_alpha: 0.01,
            busy: BusyFloor::Follow(0.00005),
            hold_while_open: true,
        },
        floor_min: 0.0,
        grouping: Grouping::Band {
            signals: 3,
            centroid_hz: 120e3,
            clear_hz: 150e3,
            min_share: 0.02,
        },
        close_frames: ((25e-3 * fs / n as f64).ceil() as u32).max(4),
        min_frames: 1,
        max_len: 3 * history / 4,
        long: LongBurst::Absorb,
        pre: (1e-3 * fs) as usize,
        post: (post_s * fs) as usize,
        history,
    }
}

impl BurstDetect {
    fn report(&mut self) {
        let (mut nf, mut bursts) = (None, 0);
        if let Some(d) = self.det.as_ref() {
            let n = d.config().frame;
            if n > 4 && d.frames() > 0 {
                let fm = d.floor()[2..n - 2].iter().sum::<f32>() as f64 / (n - 4) as f64;
                nf = Some(10.0 * (fm / (n as f64 * n as f64 * 0.375)).max(1e-30).log10());
            }
            bursts = d.bursts();
        }
        let lvl = (self.level_n > 0)
            .then(|| 10.0 * (self.level_acc / self.level_n as f64).max(1e-20).log10());
        self.published.put(&IsmDetectFields {
            bursts_detected: i64::try_from(bursts).unwrap_or(i64::MAX),
            noise_floor_dbfs: nf,
            input_level_dbfs: lvl,
        });
        self.level_acc = 0.0;
        self.level_n = 0;
    }
}

impl Component for BurstDetect {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ism.burstdetect",
            vec![Port::iq("in")],
            vec![Port::typed::<IsmBurst>("bursts").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(BurstDetectSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let fs = inputs[0].rate;
        if !(fs > 0.0 && fs.is_finite()) {
            return Err(ConfigError::NoRate);
        }
        self.rate = fs;
        self.det = Some(BandDetector::new(band_config(
            fs,
            self.threshold_db,
            post_s(self.submode),
        ))?);
        self.base = None;
        Ok(vec![inputs[0]
            .with_kind(Kind::typed::<IsmBurst>())
            .with_rate(0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if self.base.is_none() {
            self.base = Some(io.input(0).first());
        }
        let base = self.base.unwrap_or(0);
        let rate = self.rate;
        let x = io.iq(0);
        for &s in x {
            self.level_acc += s.norm_sqr() as f64;
        }
        self.level_n += x.len() as u64;
        if let Some(d) = self.det.as_mut() {
            let out = &mut self.out;
            d.process(x, |b| {
                let start = b.start + base;
                out.push(Timed {
                    time: start as f64 / rate,
                    value: IsmBurst {
                        samples: b.samples.to_vec(),
                        rate,
                        offset_hz: b.offset_hz,
                        start,
                        input_rate: rate,
                        lead: b.lead,
                    },
                });
            });
        }
        if !self.out.is_empty() {
            io.items_out::<IsmBurst>(0).append(&mut self.out);
        }
        if io.tick() {
            self.report();
        }
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        match BurstDetectSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BurstDetectSetting::Threshold => {
                self.threshold_db = THRESHOLD.read(id, value)?;
                if let Some(d) = self.det.as_mut() {
                    d.set_threshold_db(self.threshold_db);
                }
            }
            BurstDetectSetting::Submode => {
                let s = SUBMODE.read(id, value)?;
                if s != self.submode && self.rate > 0.0 {
                    self.det =
                        BandDetector::new(band_config(self.rate, self.threshold_db, post_s(s)))
                            .ok();
                    self.base = None;
                }
                self.submode = s;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (BurstDetectSetting::Threshold, Value::F64(self.threshold_db)),
            (
                BurstDetectSetting::Submode,
                Value::Choice(self.submode.index()),
            ),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    /// Drops the noise floor and the input level until the next report. The count of bursts stays.
    fn forget(&mut self, _kept: &Kept) {
        self.level_acc = 0.0;
        self.level_n = 0;
        let bursts = self.det.as_ref().map_or(0, |d| d.bursts());
        self.published.put(&IsmDetectFields {
            bursts_detected: i64::try_from(bursts).unwrap_or(i64::MAX),
            noise_floor_dbfs: None,
            input_level_dbfs: None,
        });
    }
}

/// Mixes each burst's signal to 0 Hz, filters and decimates to about 500 ksps (`sdrkit_algo_modem::burstmix`).
#[derive(Default)]
pub struct Channelise {
    mixer: Option<BurstMixer>,
}

impl Channelise {
    /// Channelises one burst at the input rate `b.rate`.
    pub fn run(&mut self, b: &IsmBurst) -> IsmBurst {
        if self.mixer.as_ref().is_none_or(|m| m.rate_hz() != b.rate) {
            let fs = b.rate;
            let decim = ((fs / 500e3).round() as usize).max(1);
            let taps = (3.3 * fs / 80e3).ceil() as usize;
            self.mixer = BurstMixer::new(fs, decim, 110e3_f64.min(fs / 2.0), taps).ok();
        }
        let mut out = Vec::new();
        let mut decim = 1;
        if let Some(m) = self.mixer.as_mut() {
            out.reserve(b.samples.len() / m.decimation() + 1);
            m.run(&b.samples, b.start, b.offset_hz, &mut out);
            decim = m.decimation();
        }
        IsmBurst {
            samples: out,
            rate: b.rate / decim as f64,
            offset_hz: b.offset_hz,
            start: b.start,
            input_rate: b.input_rate,
            lead: b.lead / decim,
        }
    }
}

impl Component for Channelise {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ism.channelise",
            vec![Port::typed::<IsmBurst>("in")],
            vec![Port::typed::<IsmBurst>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.mixer = None;
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let items = io.items::<IsmBurst>(0);
        if items.is_empty() {
            return;
        }
        let res: Vec<Timed<IsmBurst>> = items
            .iter()
            .map(|t| Timed {
                time: t.time,
                value: self.run(&t.value),
            })
            .collect();
        io.items_out::<IsmBurst>(0).extend(res);
    }
}

/// The average that makes the narrow channel the OOK detector reads, seconds: 20 µs of complex samples keep about ±25 kHz around
/// the signal's centre, and the edges of a pulse stay 20 µs long whatever its level.
const OOK_AVERAGE_S: f64 = 20e-6;

/// The boxcars of the narrow-envelope retry, seconds: about +-12.5, 6 and 3 kHz around the burst's carrier.
const RETRY_BOXCARS_S: [f64; 3] = [40e-6, 80e-6, 160e-6];
/// The windows of the retry's tone energies, seconds: about a chip of 19.2, 10 and 5 kchip/s and of 40 kchip/s.
const RETRY_TONE_WINDOWS_S: [f64; 4] = [48e-6, 96e-6, 192e-6, 24e-6];
/// The average of the retry's envelope, seconds.
const RETRY_ENVELOPE_S: f64 = 8e-6;
/// The share of the burst's span the marks must fill for the FSK retry (the first pass needs 0.9).
const RETRY_FSK_FILL: f64 = 0.5;

/// Turns each channelised burst into pulse trains with the pulse detector of `sdrkit-proto-ookdev`, and places each train in the
/// input's time and frequency. When the carrier stays up over the burst, the FSK detector reads the whole
/// channel. Otherwise, or when it finds no FSK carrier, the OOK detector reads a narrow channel around the signal's centre.
///
/// With the protocols given ([`Pulses::with_protocols`]), a burst whose trains no protocol decodes is tried again, and only
/// trains a protocol decodes are kept from the retries (so the protocols' own checks hold the false-decode rate):
/// 1. the burst mixed to 0 Hz by its own phase step (`arg(sum z[k] conj(z[k-1]))`); for boxcars of 40, 80 and 160 us, the
///    magnitude of the boxcar mean, averaged over 8 us, sliced between its two levels
///    (`sdrkit_proto_ookdev::level_train`); the first width a protocol decodes is kept;
/// 2. else the two tones of the burst's spectrum (`sdrkit_proto_ookdev::burst_tones`) read by their energies over sliding
///    windows of 48, 96, 192 and 24 us (`sdrkit_proto_ookdev::tone_train`); the first window a protocol decodes is kept;
/// 3. else the FSK detector with the marks filling at least half the span and the drop-outs bridged (the samples over the span
///    held at unit magnitude, so the whole span counts as carrier);
/// 4. FSK trains of either discriminator pass that no protocol reads are read again chip by chip from the tone energies
///    (`sdrkit_algo_modem::fsk::TonePair`) over half a chip centred on each chip, the tones at the train's midpoint plus and
///    minus its deviation and the chip the shortest run of the train (at least 4 samples).
#[derive(Default)]
pub struct Pulses {
    fsk: Option<PulseDetector>,
    ook: Option<PulseDetector>,
    narrow: Option<Convolver<Complex32>>,
    rate: f64,
    buf: Vec<Complex32>,
    found: Vec<Train>,
    protocols: Option<Arc<ProtocolList>>,
    work: Decoding,
    readings: Vec<Reading>,
    mixed: Vec<Complex32>,
    env: Vec<f32>,
    /// The submode `pulse_analyser`: no protocol is tried and no burst is tried again.
    analyse: bool,
}

/// The mean power of the quiet samples before a burst (after the filter has settled), or 0 when there are too few.
fn quiet_floor(x: &[Complex32], lead: usize) -> f32 {
    let a = 12.min(lead);
    let quiet = &x[a..lead.max(a).min(x.len())];
    if quiet.len() < 8 {
        return 0.0;
    }
    (quiet.iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / quiet.len() as f64) as f32
}

impl Pulses {
    /// A pulse stage that retries the bursts these protocols do not decode.
    pub fn with_protocols(list: Arc<ProtocolList>) -> Self {
        Pulses {
            protocols: Some(list),
            ..Self::default()
        }
    }

    /// Whether a protocol decodes `t`.
    fn decodes(&mut self, t: &Train) -> bool {
        let Some(list) = &self.protocols else {
            return false;
        };
        self.readings.clear();
        self.work.run(list, t, &mut self.readings).unwrap_or(0) > 0
    }

    /// The retries of a burst no protocol decoded (type docs); the trains found go to `self.found`. `span` is the first and
    /// last sample of the marks of the first OOK pass, `fill` their share of it.
    fn retry(
        &mut self,
        b: &IsmBurst,
        span: Option<(usize, usize)>,
        fill: f64,
        first: &[Train],
    ) -> f64 {
        let x = &b.samples;
        let mut fsk: Vec<Train> = first
            .iter()
            .filter(|t| t.modulation == Modulation::Fsk)
            .cloned()
            .collect();
        let n = x.len();
        // 1. The carrier from the burst's own phase step, and the narrow envelopes around it.
        let step: Complex32 = x.windows(2).map(|w| w[1] * w[0].conj()).sum();
        let fc = f64::from(step.im.atan2(step.re)) * b.rate / std::f64::consts::TAU;
        self.mixed.clear();
        let w = -std::f64::consts::TAU * fc / b.rate;
        self.mixed.extend(x.iter().enumerate().map(|(k, &z)| {
            z * Complex32::from_polar(1.0, (w * k as f64).rem_euclid(std::f64::consts::TAU) as f32)
        }));
        let avg = ((RETRY_ENVELOPE_S * b.rate).round() as usize).max(1);
        for width in RETRY_BOXCARS_S {
            let len = ((width * b.rate).round() as usize).max(1);
            centred_mean_magnitude(&self.mixed, len, &mut self.env);
            let env = std::mem::take(&mut self.env);
            centred_mean(&env, avg, &mut self.env);
            let trains = level_train(&self.env, b.rate);
            let good: Vec<Train> = trains.into_iter().filter(|t| self.decodes(t)).collect();
            if !good.is_empty() {
                self.found = good;
                return fc;
            }
        }
        // 2. The two tones of the spectrum over sliding windows.
        if let Some((mid, dev)) = burst_tones(x, b.rate, 4000.0) {
            for width in RETRY_TONE_WINDOWS_S {
                let win = ((width * b.rate).round() as usize).max(2);
                if let Some(t) = tone_train(x, b.rate, mid, dev, win).filter(|t| self.decodes(t)) {
                    self.found.push(t);
                    return 0.0;
                }
            }
        }
        // 3. FSK over the span with the drop-outs bridged.
        if let Some((a, e)) = span.filter(|_| fill >= RETRY_FSK_FILL) {
            self.buf.clear();
            self.buf.extend(x.iter().enumerate().map(|(k, &z)| {
                let m = z.norm();
                if (a..=e.min(n - 1)).contains(&k) && m > 0.0 {
                    z / m
                } else {
                    z
                }
            }));
            let floor = quiet_floor(&self.buf, b.lead);
            let mut trains = Vec::new();
            if let Some(det) = self.fsk.as_mut() {
                det.run_burst(&self.buf, floor, |t| trains.push(t.clone()));
            }
            fsk.extend(trains);
        }
        // 4. The FSK trains of the discriminator again from the tone energies.
        for t in &fsk {
            if self.decodes(t) {
                self.found.push(t.clone());
            } else if let Some(c) = tone_chips(x, t) {
                if self.decodes(&c) {
                    self.found.push(c);
                }
            }
        }
        0.0
    }

    fn setup(&mut self, rate: f64) {
        let det = |m: Modulation| {
            let mut c = DetectorConfig::new(rate);
            c.modulation = Some(m);
            // One burst is one transmission: a train ends only with the burst (the burst closes after 25 ms of quiet).
            c.reset_us = 25_000.0;
            PulseDetector::new(c).ok()
        };
        self.fsk = det(Modulation::Fsk);
        self.ook = det(Modulation::Ook);
        let n = ((OOK_AVERAGE_S * rate).round() as usize).max(1);
        self.narrow = Convolver::new(&vec![1.0 / n as f32; n]).ok();
        self.rate = rate;
    }

    /// The trains of one channelised burst, each with its start time (seconds) and its offset from the input centre.
    pub fn run(&mut self, b: &IsmBurst, mut f: impl FnMut(Train)) {
        if b.samples.len() < 64 {
            return;
        }
        if self.fsk.is_none() || self.rate != b.rate {
            self.setup(b.rate);
        }
        self.found.clear();
        let floor = quiet_floor(&b.samples, b.lead);
        // The carrier stayed up (FSK) when the marks of the whole channel fill at least 90 % of the span from the first to the
        // last: noise in the gaps of an OOK burst can look like two tones, the gaps themselves cannot.
        let mut fill = 0.0f64;
        let mut marks_span: Option<(usize, usize)> = None;
        if let Some(det) = self.ook.as_mut() {
            let dropped = det.dropped();
            det.run_burst(&b.samples, floor, |t| {
                let span: f64 = t.len_samples() - t.pulses.last().map_or(0.0, |p| p.space as f64);
                let marks: f64 = t.pulses.iter().map(|p| p.mark as f64).sum();
                if span > 0.0 {
                    fill = fill.max(marks / span);
                    let (a, e) = (
                        t.start.max(0.0) as usize,
                        (t.start + span).max(0.0) as usize,
                    );
                    marks_span = Some(marks_span.map_or((a, e), |(x, y)| (x.min(a), y.max(e))));
                }
            });
            // A carrier that stays up is one long mark: too few pulses for a train.
            if det.dropped() > dropped {
                fill = 1.0;
            }
        }
        let found = &mut self.found;
        if fill >= 0.9 {
            if let Some(det) = self.fsk.as_mut() {
                det.run_burst(&b.samples, floor, |t| found.push(t.clone()));
            }
        }
        if self.found.is_empty() {
            if let (Some(det), Some(conv)) = (self.ook.as_mut(), self.narrow.as_ref()) {
                self.buf.resize(b.samples.len(), Complex32::default());
                if conv.centred(&b.samples, &mut self.buf).is_ok() {
                    let found = &mut self.found;
                    det.run_burst(&self.buf, quiet_floor(&self.buf, b.lead), |t| {
                        found.push(t.clone())
                    });
                }
            }
        }
        let mut carrier = 0.0;
        if self.protocols.is_some() && !self.analyse {
            let found = std::mem::take(&mut self.found);
            let any = found.iter().any(|t| self.decodes(t));
            self.found = found;
            if !any {
                let first = std::mem::take(&mut self.found);
                carrier = self.retry(b, marks_span, fill, &first);
                if self.found.is_empty() {
                    self.found = first;
                    carrier = 0.0;
                }
            }
        }
        let t0 = b.start as f64 / b.input_rate;
        for mut t in self.found.drain(..) {
            t.offset_hz += b.offset_hz + carrier;
            t.start = (t0 + t.start / b.rate) * b.rate;
            f(t);
        }
    }
}

/// The shortest chip [`tone_chips`] reads, samples.
const TONE_CHIP_MIN: f32 = 4.0;

/// An FSK train `t` of the burst `x` read again from the tone energies: the chip is the shortest run of the train; each chip is
/// the upper tone when the soft value of a [`TonePair`] at the train's midpoint +- its deviation, over `max(2, chip / 2)`
/// samples centred on the chip, is positive; runs of chips become the marks (upper) and spaces (lower). `None` when the chip
/// is under 4 samples or the tones cannot be set.
fn tone_chips(x: &[Complex32], t: &Train) -> Option<Train> {
    let k = t.pulses.len();
    let chip = t
        .pulses
        .iter()
        .enumerate()
        .flat_map(|(i, p)| [Some(p.mark), (i + 1 < k).then_some(p.space)])
        .flatten()
        .filter(|&v| v > 0.0)
        .fold(f32::MAX, f32::min);
    if !(TONE_CHIP_MIN..f32::MAX).contains(&chip) {
        return None;
    }
    let chip = f64::from(chip);
    let span = t.len_samples() - t.pulses.last().map_or(0.0, |p| f64::from(p.space));
    let win = ((chip / 2.0).round() as usize).max(2);
    let mut pair = TonePair::new(t.rate_hz, t.offset_hz, t.deviation_hz, win).ok()?;
    let chips = (span / chip).round() as usize;
    let mut bits = Vec::with_capacity(chips);
    let mut at = 0usize;
    for c in 0..chips {
        // The window ends half a window after the chip's centre.
        let end = (t.start + (c as f64 + 0.5) * chip + win as f64 / 2.0).round() as usize;
        if end > x.len() {
            break;
        }
        while at < end {
            pair.push(x[at]);
            at += 1;
        }
        bits.push(pair.upper(win, 0)?);
    }
    let first = bits.iter().position(|&u| u)?;
    let mut out = Train::new(Modulation::Fsk, t.rate_hz);
    out.start = t.start + first as f64 * chip;
    out.offset_hz = t.offset_hz;
    out.deviation_hz = t.deviation_hz;
    out.level_dbfs = t.level_dbfs;
    out.snr_db = t.snr_db;
    let mut i = first;
    while i < bits.len() {
        let m0 = i;
        while i < bits.len() && bits[i] {
            i += 1;
        }
        let g0 = i;
        while i < bits.len() && !bits[i] {
            i += 1;
        }
        out.pulses.push(Pulse::new(
            ((g0 - m0) as f64 * chip) as f32,
            ((i - g0) as f64 * chip) as f32,
        ));
    }
    Some(out)
}

/// `out[k]` = the magnitude of the mean of the `len` samples of `x` centred on `k` (clipped at the ends).
fn centred_mean_magnitude(x: &[Complex32], len: usize, out: &mut Vec<f32>) {
    let n = x.len();
    out.clear();
    let mut acc = vec![Complex32::default(); n + 1];
    for k in 0..n {
        acc[k + 1] = acc[k] + x[k];
    }
    let h = len / 2;
    out.extend((0..n).map(|k| {
        let (a, e) = (k.saturating_sub(h), (k + len - h).min(n));
        ((acc[e] - acc[a]) / (e - a).max(1) as f32).norm()
    }));
}

/// `out[k]` = the mean of the `len` values of `x` centred on `k` (clipped at the ends).
fn centred_mean(x: &[f32], len: usize, out: &mut Vec<f32>) {
    let n = x.len();
    out.clear();
    let mut acc = vec![0f64; n + 1];
    for k in 0..n {
        acc[k + 1] = acc[k] + f64::from(x[k]);
    }
    let h = len / 2;
    out.extend((0..n).map(|k| {
        let (a, e) = (k.saturating_sub(h), (k + len - h).min(n));
        ((acc[e] - acc[a]) / (e - a).max(1) as f64) as f32
    }));
}

impl Component for Pulses {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ism.pulses",
            vec![Port::typed::<IsmBurst>("in")],
            vec![Port::typed::<Train>("trains").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(SubmodeSetting::schema())
    }
    fn values(&self) -> Values {
        let s = if self.analyse {
            Submode::PulseAnalyser
        } else {
            Submode::Devices
        };
        values_of([(SubmodeSetting::Submode, Value::Choice(s.index()))])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        match SubmodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SubmodeSetting::Submode => {
                self.analyse = SUBMODE.read(id, value)? == Submode::PulseAnalyser;
            }
        }
        Ok(Applied::Live)
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fsk = None;
        Ok(vec![inputs[0].with_kind(Kind::typed::<Train>())])
    }
    fn process(&mut self, io: &mut Io) {
        let items = io.items::<IsmBurst>(0);
        if items.is_empty() {
            return;
        }
        let mut res = Vec::new();
        for b in items {
            self.run(&b.value, |t| {
                res.push(Timed {
                    time: t.time_s(),
                    value: t,
                })
            });
        }
        io.items_out::<Train>(0).extend(res);
    }
}
