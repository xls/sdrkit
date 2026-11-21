//! Voice inversion (`voice_inversion`): detects full-band frequency inversion on one narrow-FM channel, estimates the
//! inversion carrier and descrambles. An audio frequency `f` of the scrambled speech is `carrier - f`. This is not encryption.
//! The shape of long-term speech follows Byrne et al., "An international comparison of long-term average speech spectra",
//! JASA 96(4), 1994.
//!
//! ```text
//! iq -> [guard] -> [front, 12 kHz] -> [voice_inversion: discriminator, long-term spectrum, fit, mixer and low-pass]
//!                                        +--> audio, 12 kHz --> [resampler, 48 kHz] --> audio
//!                                        +--> events
//! ```

use crate::common::{self, Placer, Report, Stage, MIN_RATE_HZ, RATE};
use crate::typed::{
    VoiceInversionClearEvent, VoiceInversionFields, VoiceInversionFoundEvent,
    VOICE_INVERSION_CLEAR, VOICE_INVERSION_FOUND,
};
use sdrkit_algo_dsp::fir::{design_beta, windowed_sinc, Fir};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_components::dsp::AudioResampler;
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::field;
use sdrkit_core::results::SeriesId;
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, GenConfig, Generator,
    Record, Results, ResultsDecl, Setting, SettingError, SettingId, SettingSet, SignalInfo,
    StackConfig, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, DecoderCommand, Format, Graph,
    GraphBuilder, GraphError, Io, Kept, Kind, Port, RateFrom, RunContext, StackGraph,
};
use std::collections::VecDeque;
use std::f64::consts::{PI, TAU};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "voice_inversion";
/// The name in a status line.
pub const NAME: &str = "Voice inversion";
/// The deviation that reads 1.0.
pub const DEVIATION_HZ: f64 = 2500.0;
/// The samples of a frame (20 ms).
const FRAME: usize = 240;
/// The points of the spectrum: 0 to 4800 Hz in steps of 50 Hz.
pub const POINTS: usize = 97;
/// The step of the spectrum, Hz.
pub const POINT_HZ: f64 = 50.0;
/// The lowest and highest inversion carrier searched, Hz, and the step.
const SEARCH_LOW_HZ: f64 = 2700.0;
const SEARCH_HIGH_HZ: f64 = 3700.0;
const SEARCH_STEP_HZ: f64 = 25.0;
/// The taps of the audio low-pass.
const LOWPASS_TAPS: usize = 161;
/// The highest cut-off of the audio low-pass, Hz.
const LOWPASS_MAX_HZ: f64 = 3400.0;
/// The series of the long-term spectrum.
pub const SPECTRUM_SERIES: &str = "spectrum";

/// The long-term shape of speech at `f` Hz, dB: a peak near 500 Hz, about 7 dB an octave down above it, a 300 Hz high-pass
/// edge and a 3 kHz low-pass edge.
pub fn speech_template_db(f: f64) -> f64 {
    let mut t = if f < 300.0 {
        -3.0 - 22.0 * (300.0 - f) / 150.0
    } else if f < 500.0 {
        -3.0 * (500.0 - f) / 200.0
    } else {
        -7.0 * (f / 500.0).log2()
    };
    if f > 3000.0 {
        t -= 20.0 * (f - 3000.0) / 200.0;
    }
    t
}

/// The level of `spectrum` at `f` Hz, dB: mirrored below 0 Hz, held above the last point, linear between points.
fn level_at(spectrum: &[f64; POINTS], f: f64) -> f64 {
    let x = (f.abs() / POINT_HZ).min((POINTS - 1) as f64);
    let i = (x.floor() as usize).min(POINTS - 2);
    let frac = x - i as f64;
    spectrum[i] + frac * (spectrum[i + 1] - spectrum[i])
}

/// The Pearson correlation of the speech template with `spectrum` for `f` from `low` to `high` Hz in steps of 50 Hz: as heard,
/// or flipped about `carrier` (then `f` is not above the carrier). `None` for fewer than 20 points.
pub fn fit(spectrum: &[f64; POINTS], carrier: Option<f64>, low: f64, high: f64) -> Option<f64> {
    let (mut n, mut sx, mut sy, mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let mut f = low;
    while f <= high + 1e-9 {
        if carrier.is_some_and(|c| f > c) {
            break;
        }
        let x = speech_template_db(f);
        let y = level_at(spectrum, carrier.map_or(f, |c| c - f));
        n += 1.0;
        sx += x;
        sy += y;
        sxx += x * x;
        syy += y * y;
        sxy += x * y;
        f += POINT_HZ;
    }
    if n < 20.0 {
        return None;
    }
    let (vx, vy) = (sxx - sx * sx / n, syy - sy * sy / n);
    (vx > 0.0 && vy > 0.0).then(|| (sxy - sx * sy / n) / (vx * vy).sqrt())
}

/// The carrier from 2700 to 3700 Hz in steps of 25 Hz with the best flipped fit from `low` to `high` Hz, and the fit.
fn best_carrier(spectrum: &[f64; POINTS], low: f64, high: f64) -> Option<(f64, f64)> {
    let steps = ((SEARCH_HIGH_HZ - SEARCH_LOW_HZ) / SEARCH_STEP_HZ).round() as usize;
    (0..=steps)
        .filter_map(|k| {
            let c = SEARCH_LOW_HZ + SEARCH_STEP_HZ * k as f64;
            fit(spectrum, Some(c), low, high).map(|v| (c, v))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

/// The carrier from the top edge of the spectrum: the highest point within 12 dB of the median level from 500 to 2000 Hz
/// whose next higher point is within 30 dB of it, plus 50 Hz, plus the 300 Hz where clear speech starts.
fn edge_carrier(spectrum: &[f64; POINTS]) -> Option<f64> {
    let mut mid: Vec<f64> = spectrum[10..=40].to_vec();
    mid.sort_by(f64::total_cmp);
    let median = mid[mid.len() / 2];
    (11..=95)
        .rev()
        .find(|&i| spectrum[i] >= median - 12.0 && spectrum[i + 1] >= median - 30.0)
        .map(|i| i as f64 * POINT_HZ + POINT_HZ + 300.0)
}

/// What a judgement says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    /// Speech as it is spoken.
    Clear,
    /// Inverted speech with this carrier estimate, Hz.
    Inverted(f64),
}

/// A change of the detector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change {
    /// Inverted speech is confirmed, with this carrier, Hz.
    Found(f64),
    /// Speech that is not inverted.
    Clear,
}

/// The detector: the 12 kHz channel to the discriminator audio, the long-term spectrum and the verdict.
pub struct InversionDetector {
    disc: Discriminator,
    mean: f64,
    window: Vec<f32>,
    cos: Vec<f32>,
    sin: Vec<f32>,
    frame: Vec<f32>,
    floor: f64,
    voiced: [f64; POINTS],
    weight: f64,
    noise: Option<[f64; POINTS]>,
    voiced_frames: u64,
    frames: u64,
    last_voiced: Option<u64>,
    snr_db: Option<f64>,
    spectrum: [f64; POINTS],
    fit_heard: Option<f64>,
    fit_inverted: Option<f64>,
    judgements: u64,
    clear: bool,
    before: Option<f64>,
    agree: u32,
    carrier: Option<f64>,
}

impl Default for InversionDetector {
    fn default() -> Self {
        let window = Window::Hann.coefficients(FRAME);
        let mut cos = Vec::with_capacity(POINTS * FRAME);
        let mut sin = Vec::with_capacity(POINTS * FRAME);
        for p in 0..POINTS {
            for n in 0..FRAME {
                let (s, c) = (TAU * POINT_HZ * p as f64 * n as f64 / RATE).sin_cos();
                cos.push(c as f32);
                sin.push(s as f32);
            }
        }
        InversionDetector {
            disc: Discriminator::new(RATE, DEVIATION_HZ)
                .expect("the rate and the deviation are positive"),
            mean: 0.0,
            window,
            cos,
            sin,
            frame: Vec::with_capacity(FRAME),
            floor: f64::INFINITY,
            voiced: [0.0; POINTS],
            weight: 0.0,
            noise: None,
            voiced_frames: 0,
            frames: 0,
            last_voiced: None,
            snr_db: None,
            spectrum: [0.0; POINTS],
            fit_heard: None,
            fit_inverted: None,
            judgements: 0,
            clear: false,
            before: None,
            agree: 0,
            carrier: None,
        }
    }
}

impl InversionDetector {
    /// The confirmed inversion carrier, Hz.
    pub fn carrier_hz(&self) -> Option<f64> {
        self.carrier
    }
    /// The last verdict was speech that is not inverted.
    pub fn clear_speech(&self) -> bool {
        self.clear
    }
    /// Voiced frames were seen in the last 3 s.
    pub fn activity(&self) -> bool {
        self.last_voiced.is_some_and(|l| self.frames - l < 150)
    }
    /// The fit of the spectrum as heard, of the last judgement.
    pub fn fit_heard(&self) -> Option<f64> {
        self.fit_heard
    }
    /// The best fit of the flipped spectrum, of the last judgement.
    pub fn fit_inverted(&self) -> Option<f64> {
        self.fit_inverted
    }
    /// The audio signal to noise estimate, dB.
    pub fn snr_db(&self) -> Option<f64> {
        self.snr_db
    }
    /// Judgements made.
    pub fn judgements(&self) -> u64 {
        self.judgements
    }
    /// The long-term spectrum of the last judgement, dB, 97 points 50 Hz apart.
    pub fn spectrum_db(&self) -> &[f64; POINTS] {
        &self.spectrum
    }

    /// Forgets everything.
    pub fn reset(&mut self) {
        let InversionDetector {
            window, cos, sin, ..
        } = std::mem::take(self);
        (self.window, self.cos, self.sin) = (window, cos, sin);
    }

    /// The powers of the frame at the 97 points.
    fn powers(&self) -> [f64; POINTS] {
        let mut out = [0.0; POINTS];
        for (p, o) in out.iter_mut().enumerate() {
            let row = p * FRAME..(p + 1) * FRAME;
            let (mut re, mut im) = (0.0f32, 0.0f32);
            for (((&v, &w), &c), &s) in self
                .frame
                .iter()
                .zip(&self.window)
                .zip(&self.cos[row.clone()])
                .zip(&self.sin[row])
            {
                let x = v * w;
                re += x * c;
                im += x * s;
            }
            *o = f64::from(re * re + im * im);
        }
        out
    }

    /// Judges the long-term spectrum. Returns the verdict, if there is one.
    fn judge(&mut self) -> Option<Verdict> {
        let noise = self.noise.as_ref()?;
        if self.voiced_frames < 60 || self.snr_db.is_none_or(|s| s < 1.0) {
            return None;
        }
        let mut s = [0.0; POINTS];
        let mut sum = 0.0;
        for i in 0..POINTS {
            s[i] = (self.voiced[i] - self.weight * noise[i]).max(0.003 * self.voiced[i]);
            sum += s[i];
        }
        if sum <= 0.0 {
            return None;
        }
        for (o, v) in self.spectrum.iter_mut().zip(&s) {
            *o = 10.0 * (v / sum).max(1e-30).log10();
        }
        self.judgements += 1;
        let heard = fit(&self.spectrum, None, 450.0, 2700.0);
        let inverted = best_carrier(&self.spectrum, 450.0, 2700.0).map(|b| b.1);
        self.fit_heard = heard;
        self.fit_inverted = inverted;
        let (h, v) = (heard?, inverted?);
        if h >= 0.45 && h > v + 0.1 {
            return Some(Verdict::Clear);
        }
        if v >= 0.45 && v > h + 0.1 {
            let (a, _) = best_carrier(&self.spectrum, 150.0, 3450.0)?;
            let estimate = match edge_carrier(&self.spectrum) {
                Some(b) if (b - a).abs() <= 450.0 => 0.5 * (a + b),
                _ => a,
            };
            return Some(Verdict::Inverted(estimate));
        }
        None
    }

    /// Takes the verdict of a judgement.
    fn decide(&mut self, verdict: Verdict, changes: &mut Vec<Change>) {
        match verdict {
            Verdict::Clear => {
                let was = self.carrier.take().is_some() || !self.clear;
                self.clear = true;
                self.before = None;
                self.agree = 0;
                if was {
                    changes.push(Change::Clear);
                }
            }
            Verdict::Inverted(estimate) => {
                self.clear = false;
                if let Some(c) = self.carrier {
                    if (estimate - c).abs() <= 100.0 {
                        self.carrier = Some(0.8 * c + 0.2 * estimate);
                    }
                    return;
                }
                self.agree = match self.before {
                    Some(b) if (estimate - b).abs() <= 100.0 => self.agree + 1,
                    _ => 0,
                };
                self.before = Some(estimate);
                if self.agree >= 2 {
                    self.carrier = Some(estimate);
                    changes.push(Change::Found(estimate));
                }
            }
        }
    }

    /// Takes the frame that is full.
    fn end_frame(&mut self, changes: &mut Vec<Change>) {
        let e: f64 = self.frame.iter().map(|&v| f64::from(v * v)).sum();
        self.floor = e.min(self.floor * 1.003 + 1e-9);
        let voiced = e > 1.5 * self.floor && e > 0.02;
        let quiet = e <= 1.2 * self.floor;
        if voiced || quiet {
            let power = self.powers();
            if voiced {
                for (p, v) in self.voiced.iter_mut().zip(&power) {
                    *p = 0.997 * *p + v;
                }
                self.weight = 0.997 * self.weight + 1.0;
                self.voiced_frames += 1;
                self.last_voiced = Some(self.frames);
                let snr = 10.0 * (e / self.floor.max(1e-30)).log10();
                self.snr_db = Some(self.snr_db.map_or(snr, |s| s + 0.1 * (snr - s)));
            } else {
                match self.noise.as_mut() {
                    Some(n) => {
                        for (n, v) in n.iter_mut().zip(&power) {
                            *n = 0.97 * *n + 0.03 * v;
                        }
                    }
                    None => self.noise = Some(power),
                }
            }
        }
        self.frames += 1;
        self.frame.clear();
        if self.frames.is_multiple_of(25) {
            if let Some(v) = self.judge() {
                self.decide(v, changes);
            }
        }
        // The signal went away: start again.
        if self.last_voiced.is_some_and(|l| self.frames - l > 250) {
            self.reset();
        }
    }

    /// Takes `x`. Appends the discriminator audio (2.5 kHz of deviation is 1.0, the mean removed, limited to 2) to `audio` and
    /// each change to `changes`.
    pub fn process(&mut self, x: &[Complex32], audio: &mut Vec<f32>, changes: &mut Vec<Change>) {
        for &s in x {
            let d = f64::from(self.disc.process_sample(s));
            self.mean += 0.002 * (d - self.mean);
            let v = (d - self.mean).clamp(-2.0, 2.0) as f32;
            audio.push(v);
            self.frame.push(v);
            if self.frame.len() == FRAME {
                self.end_frame(changes);
            }
        }
    }
}

/// The taps of the audio low-pass at `cutoff_hz` for 12 kHz samples.
fn lowpass_taps(cutoff_hz: f64) -> Vec<f32> {
    windowed_sinc(LOWPASS_TAPS, cutoff_hz / RATE, design_beta(50.0))
        .expect("the cut-off is below half the rate")
        .into_iter()
        .map(|t| t as f32)
        .collect()
}

/// The inversion mixer and its low-pass, for 12 kHz audio: `out = lowpass(in x 2 cos(2 pi carrier t)) / 2`, the low-pass at the
/// carrier or 3400 Hz, whichever is lower. Without a carrier it is the low-pass at 3400 Hz alone. The same operation scrambles
/// and descrambles.
pub struct Inverter {
    carrier: Option<f64>,
    cutoff: f64,
    phase: f64,
    fir: Fir<f32>,
    gain: f32,
}

impl Inverter {
    /// A mixer with `carrier` Hz, or none, and the output gain `gain`.
    pub fn new(carrier: Option<f64>, gain: f32) -> Self {
        let cutoff = carrier.map_or(LOWPASS_MAX_HZ, |c| c.min(LOWPASS_MAX_HZ));
        Inverter {
            carrier,
            cutoff,
            phase: 0.0,
            fir: Fir::new(&lowpass_taps(cutoff)),
            gain,
        }
    }
    /// The carrier in use, Hz.
    pub fn carrier(&self) -> Option<f64> {
        self.carrier
    }
    /// Sets the carrier. The low-pass is made again when its cut-off moves by more than 10 Hz.
    pub fn set_carrier(&mut self, carrier: Option<f64>) {
        self.carrier = carrier;
        let cutoff = carrier.map_or(LOWPASS_MAX_HZ, |c| c.min(LOWPASS_MAX_HZ));
        if (cutoff - self.cutoff).abs() > 10.0 {
            self.cutoff = cutoff;
            self.fir.set_taps(&lowpass_taps(cutoff));
        }
    }
    /// Appends the output for `x` to `out`.
    pub fn process(&mut self, x: &[f32], out: &mut Vec<f32>) {
        for &v in x {
            let mixed = match self.carrier {
                Some(c) => {
                    self.phase = (self.phase + c / RATE).fract();
                    v * 2.0 * (TAU * self.phase).cos() as f32
                }
                None => v,
            };
            out.push(self.fir.process_sample(mixed) * self.gain);
        }
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The results of the detector component: the shared fields, its own fields and the spectrum.
fn detector_declaration() -> ResultsDecl {
    common::declaration(VoiceInversionFields::SHAPE.fields).series(
        SPECTRUM_SERIES,
        "Long-term spectrum",
        "Hz",
        "dB",
    )
}

/// The carrier to descramble with. 0 uses the carrier found.
pub const CARRIER_SETTING: Setting<field::Hz> =
    Setting::new("carrier_hz", "Carrier (Hz, 0 = automatic)", "Hz", 0.0)
        .range(0.0, 4300.0, 0.0)
        .help("0 uses the carrier that the detector finds. A carrier is from 2000 to 4300 Hz.");
/// The lowest carrier a user can set, Hz.
const CARRIER_MIN_HZ: f64 = 2000.0;

sdrkit_core::settings! {
    /// The settings of [`VoiceInversion`].
    pub enum DescramblerSetting {
        /// The carrier.
        Carrier = CARRIER_SETTING,
    }
}

/// The detector and the descrambler as a component (`voice.inversion`).
pub struct VoiceInversion {
    det: InversionDetector,
    inverter: Inverter,
    forced: f64,
    report: Report,
    series: SeriesId,
    audio: Vec<f32>,
    out: Vec<f32>,
    changes: Vec<Change>,
}

impl Default for VoiceInversion {
    fn default() -> Self {
        let decl = detector_declaration();
        let series = decl
            .series_id(SPECTRUM_SERIES)
            .expect("the declaration holds the series");
        VoiceInversion {
            det: InversionDetector::default(),
            inverter: Inverter::new(None, 0.5),
            forced: 0.0,
            report: Report::new(NAME, decl),
            series,
            audio: Vec::new(),
            out: Vec::new(),
            changes: Vec::new(),
        }
    }
}

impl VoiceInversion {
    /// The carrier in use: the one set, else the one found.
    fn carrier(&self) -> Option<f64> {
        if self.forced > 0.0 {
            Some(self.forced)
        } else {
            self.det.carrier_hz()
        }
    }

    fn publish(&mut self) {
        let d = &self.det;
        let found = d.carrier_hz();
        let stage = if found.is_some() {
            Stage::Confirmed
        } else if d.activity() {
            Stage::Signal
        } else {
            Stage::Searching
        };
        let text = if self.forced > 0.0 {
            format!(
                "descrambling with the carrier you set, {:.0} Hz",
                self.forced
            )
        } else if let Some(c) = found {
            format!("inverted speech, carrier {c:.0} Hz, descrambled")
        } else if d.clear_speech() && d.activity() {
            "speech, not inverted".to_string()
        } else if d.activity() {
            "voice activity, no speech spectrum yet".to_string()
        } else {
            "searching".to_string()
        };
        let p = &mut self.report.p;
        p.put(&VoiceInversionFields {
            inverted: found.is_some(),
            carrier_hz: found.map(Hertz),
            fit_heard: d.fit_heard(),
            fit_inverted: d.fit_inverted(),
        });
        let spectrum = d.spectrum_db();
        let judged = d.judgements() > 0;
        p.results_mut().update_series(self.series, |s| {
            s.fill(
                true,
                spectrum
                    .iter()
                    .enumerate()
                    .filter(|_| judged)
                    .map(|(i, &v)| [(i as f64 * POINT_HZ) as f32, v as f32]),
            )
        });
        let (quality, good) = (d.snr_db(), d.judgements());
        self.report.publish(stage, &text, quality, good, 0);
    }
}

impl Component for VoiceInversion {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "voice.inversion",
            vec![Port::iq("in"), Port::iq("raw")],
            vec![
                Port::audio("audio").rate_from(RateFrom::Fixed(RATE)),
                Port::new("events", Kind::Messages),
            ],
        )
        .with_settings(DescramblerSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if (inputs[0].rate - RATE).abs() > 1e-6 {
            return Err(ConfigError::WrongRate {
                need: RATE,
                got: inputs[0].rate,
            });
        }
        self.det.reset();
        self.report.configure(inputs[1].rate);
        Ok(vec![
            Format::new(Kind::Audio, RATE),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.report.attach(ctx);
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        self.audio.clear();
        self.changes.clear();
        self.det.process(x, &mut self.audio, &mut self.changes);
        self.report.take(x);
        self.inverter.set_carrier(self.carrier());
        self.out.clear();
        self.inverter.process(&self.audio, &mut self.out);
        io.audio_out(0).samples.extend_from_slice(&self.out);
        for i in 0..self.changes.len() {
            let e = match self.changes[i] {
                Change::Found(c) => {
                    let text = format!("inverted speech found, carrier about {c:.0} Hz");
                    let t = self.report.note(&text);
                    VOICE_INVERSION_FOUND.event(
                        t,
                        0.0,
                        &VoiceInversionFoundEvent {
                            text: &text,
                            carrier_hz: Hertz(c),
                        },
                    )
                }
                Change::Clear => {
                    let text = "speech, not inverted";
                    let t = self.report.note(text);
                    VOICE_INVERSION_CLEAR.event(t, 0.0, &VoiceInversionClearEvent { text })
                }
            };
            self.report.send(e);
        }
        self.report.drain(io.messages_out(1));
        if io.tick() || !self.changes.is_empty() {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DescramblerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DescramblerSetting::Carrier => {
                let c = CARRIER_SETTING.read(id, v)?;
                if c != 0.0 && c < CARRIER_MIN_HZ {
                    return Err(SettingError::OutOfRange {
                        id,
                        min: CARRIER_MIN_HZ,
                        max: 4300.0,
                        got: c,
                    });
                }
                self.forced = c;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(DescramblerSetting::Carrier, Value::Hz(self.forced))])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.report.results())
    }
    fn commands(&self) -> &Commands {
        DecoderCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match DecoderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DecoderCommand::Clear => {
                self.det.reset();
                self.report.clear();
                self.publish();
            }
        }
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        self.det.reset();
        self.report.forget(kept);
        self.publish();
    }
}

/// The graph of the stack.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    let audio = AudioResampler::new(48e3).map_err(|error| GraphError::Setting {
        key: "audio.rate".into(),
        error,
    })?;
    common::add_front(GraphBuilder::new())
        .add("descrambler", VoiceInversion::default())
        .add("audio", audio)
        .connect("front", "descrambler.in")
        .input("descrambler.raw")
        .connect("descrambler.audio", "audio")
        .output("audio")
        .output("descrambler.events")
        .clock(0.25)
        .status_from("descrambler")
        .catalog(catalog)
        .build()
}

/// The stack graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a setting of `cfg` is refused.
pub fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            MIN_RATE_HZ,
            "Voice inversion: the sample rate is too low (250 kHz at least)",
        )
        .route(CARRIER_SETTING.key(), &["descrambler.carrier_hz"])?
        .command(
            "clear",
            sdrkit_graph::CommandTarget::of("descrambler", "clear"),
        )
        .on_retune(sdrkit_graph::CommandTarget::of("descrambler", "clear"))?
        .with_settings(&cfg.settings)
        .build()
}

const GEN: &str = "Test signal";
/// The inversion carrier of the test signal. 0 sends clear speech.
pub const GEN_INVERT_HZ: Setting<field::Hz> = Setting::new(
    "gen_invert_hz",
    "Test signal: inversion carrier",
    "Hz",
    3000.0,
)
.range(0.0, 4300.0, 0.0)
.section(GEN)
.help("0 sends clear speech.")
.advanced();
/// The length of the test signal before it starts again.
pub const GEN_SECONDS: Setting<field::Duration> =
    Setting::new("gen_seconds", "Test signal: length", "s", 12.0)
        .range(2.0, 120.0, 0.0)
        .section(GEN)
        .advanced();
/// The peak deviation of the test signal.
pub const GEN_DEVIATION_HZ: Setting<field::Hz> = Setting::new(
    "gen_deviation_hz",
    "Test signal: deviation",
    "Hz",
    DEVIATION_HZ,
)
.range(100.0, 5000.0, 0.0)
.section(GEN)
.advanced();

sdrkit_core::settings! {
    /// The settings of the stack.
    pub enum InversionSetting {
        /// The carrier.
        Carrier = CARRIER_SETTING,
        /// Inversion carrier of the test signal.
        GenInvert = GEN_INVERT_HZ,
        /// Length of the test signal.
        GenSeconds = GEN_SECONDS,
        /// Deviation of the test signal.
        GenDeviation = GEN_DEVIATION_HZ,
    }
}

/// The formants of five vowels, Hz.
const VOWELS: [[f64; 3]; 5] = [
    [730.0, 1090.0, 2440.0],
    [270.0, 2290.0, 3010.0],
    [300.0, 870.0, 2240.0],
    [530.0, 1840.0, 2480.0],
    [570.0, 840.0, 2410.0],
];
/// The widths of the three formants, Hz, and their weights.
const FORMANT_WIDTH_HZ: [f64; 3] = [90.0, 110.0, 160.0];
const FORMANT_WEIGHT: [f64; 3] = [1.0, 0.55, 0.3];

/// Speech-like audio at 12 kHz for `seconds`, peak 0.5: syllables of harmonics under three formants with a gliding pitch,
/// pauses and fricatives, band-limited to 300 to 3000 Hz.
pub fn speech(seconds: f64, seed: u64) -> Vec<f32> {
    let n = (seconds * RATE).round() as usize;
    let mut rng = Rng::new(seed);
    let mut out: Vec<f32> = Vec::with_capacity(n + RATE as usize);
    while out.len() < n {
        if rng.uniform() < 0.15 {
            let pause = (rng.range(0.2, 0.45) * RATE) as usize;
            out.resize(out.len() + pause, 0.0);
        }
        let len = (rng.range(0.14, 0.26) * RATE) as usize;
        let amplitude = rng.range(0.6, 1.0);
        let pitch_from = rng.range(105.0, 190.0);
        let pitch_to = (pitch_from * rng.range(0.85, 1.15)).clamp(105.0, 190.0);
        let vowel = VOWELS[rng.below(VOWELS.len() as u64) as usize];
        let mut phase = 0.0f64;
        for i in 0..len {
            let u = i as f64 / len as f64;
            let pitch = pitch_from + (pitch_to - pitch_from) * u;
            phase += pitch / RATE;
            let mut v = 0.0;
            let mut k = 1.0;
            while k * pitch <= 3200.0 {
                let f = k * pitch;
                let weight: f64 = (0..3)
                    .map(|j| {
                        let d = (f - vowel[j]) / FORMANT_WIDTH_HZ[j];
                        FORMANT_WEIGHT[j] / (1.0 + d * d)
                    })
                    .sum();
                v += weight / k * (TAU * k * phase).sin();
                k += 1.0;
            }
            out.push((amplitude * (PI * u).sin() * v) as f32);
        }
        if rng.uniform() < 0.35 {
            let len = (rng.range(0.05, 0.1) * RATE) as usize;
            let mut before = 0.0;
            for i in 0..len {
                let noise = rng.normal();
                let env = (PI * i as f64 / len as f64).sin();
                out.push((0.15 * env * (noise - before)) as f32);
                before = noise;
            }
        }
    }
    out.truncate(n);
    let mut filters = [
        Biquad::new(BiquadCoeffs::highpass(RATE, 300.0, 0.707)),
        Biquad::new(BiquadCoeffs::highpass(RATE, 300.0, 0.707)),
        Biquad::new(BiquadCoeffs::lowpass(RATE, 3000.0, 0.707)),
        Biquad::new(BiquadCoeffs::lowpass(RATE, 3000.0, 0.707)),
    ];
    for f in &mut filters {
        f.process(&mut out);
    }
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if peak > 0.0 {
        out.iter_mut().for_each(|v| *v *= 0.5 / peak);
    }
    out
}

/// `audio` (12 kHz) inverted about `carrier_hz`.
pub fn scramble(audio: &[f32], carrier_hz: f64) -> Vec<f32> {
    let mut out = Vec::with_capacity(audio.len());
    Inverter::new(Some(carrier_hz), 1.0).process(audio, &mut out);
    out
}

/// The amplitude of the test signal.
pub const GEN_AMPLITUDE: f32 = 0.5;

/// The test signal: speech-like audio, inverted about a carrier or clear, as narrow FM, again and again.
/// [`InversionGenerator::new`] puts the carrier at 0 Hz, [`InversionGenerator::from_config`] 20 kHz below the centre, where the
/// radio of the stack sees it.
pub struct InversionGenerator {
    rate: f64,
    deviation: f64,
    audio: Vec<f32>,
    at: usize,
    resampler: Resampler<f32>,
    ready: VecDeque<f32>,
    scratch: Vec<f32>,
    phase: f64,
    placer: Placer,
}

impl InversionGenerator {
    /// A generator at `rate` of `seconds` of speech inverted about `invert_hz` (0 for clear speech), where an audio peak of
    /// 0.5 is `deviation_hz`.
    ///
    /// # Errors
    ///
    /// [`ConfigError`] if the resampler refuses the rate.
    pub fn new(
        rate: f64,
        invert_hz: f64,
        seconds: f64,
        deviation_hz: f64,
        seed: u64,
    ) -> Result<Self, ConfigError> {
        let clear = speech(seconds, seed.wrapping_add(1));
        let audio = if invert_hz > 0.0 {
            scramble(&clear, invert_hz)
        } else {
            clear
        };
        Ok(InversionGenerator {
            rate,
            deviation: deviation_hz,
            audio,
            at: 0,
            resampler: Resampler::new(RATE, rate)?,
            ready: VecDeque::new(),
            scratch: Vec::new(),
            phase: 0.0,
            placer: Placer::new(rate, 0.0),
        })
    }
    /// The generator of the stack's settings.
    ///
    /// # Errors
    ///
    /// As [`InversionGenerator::new`].
    pub fn from_config(cfg: &GenConfig) -> Result<Self, ConfigError> {
        let s = &cfg.settings;
        let mut g = InversionGenerator::new(
            cfg.rate_hz,
            s.get(&GEN_INVERT_HZ),
            s.get(&GEN_SECONDS),
            s.get(&GEN_DEVIATION_HZ),
            cfg.seed,
        )?;
        g.placer = Placer::below_centre(cfg.rate_hz);
        Ok(g)
    }
}

impl Generator for InversionGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(3000.0).with_power(f64::from(GEN_AMPLITUDE * GEN_AMPLITUDE)))
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            while self.ready.is_empty() {
                let end = (self.at + 240).min(self.audio.len());
                self.scratch.clear();
                self.resampler
                    .process(&self.audio[self.at..end], &mut self.scratch);
                self.ready.extend(self.scratch.iter().copied());
                self.at = if end == self.audio.len() { 0 } else { end };
            }
            let a = f64::from(self.ready.pop_front().unwrap_or(0.0));
            self.phase = (self.phase + self.deviation * (a / 0.5) / self.rate).fract();
            let (s, c) = (TAU * self.phase).sin_cos();
            *o = Complex32::new(c as f32, s as f32) * GEN_AMPLITUDE;
        }
        self.placer.apply(out);
        out.len()
    }
}
