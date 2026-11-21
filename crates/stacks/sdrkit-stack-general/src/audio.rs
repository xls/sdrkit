//! The audio components of the general receiver: the detector selector, the clip, the squelch gate with its 5 ms ramp, the
//! audio filters and the voice noise remover.

use crate::toneid::ToneId;
use crate::types::{RestartCommand, SpeechProbability, SquelchBlock};
use sdrkit_components::tx::TxKeyed;
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use sdrkit_algo_dsp::ring::DelayLine;
use sdrkit_core::publish::field;
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, ConfigError, Record, Setting, SettingError,
    SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom,
    Timed,
};

const B: Setting<field::Bool> = Setting::new("b", "Second input", "", false);

sdrkit_core::settings! {
    /// The settings of [`Select`].
    pub enum SelectSetting {
        /// The second input is chosen.
        B = B,
    }
}

/// Chooses one of two `Real` streams (`general.select`): `a` (the demodulator) or `b` (the AM processor).
#[derive(Default)]
pub struct Select {
    b: bool,
}

impl Component for Select {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.select",
            vec![Port::real("a"), Port::real("b")],
            vec![Port::real("out").same_format_as(0)],
        )
        .with_settings(SelectSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(if self.b { 1 } else { 0 });
        io.real_out(0).extend_from_slice(x);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SelectSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SelectSetting::B => self.b = B.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(SelectSetting::B, Value::Bool(self.b))])
    }
}

/// Clamps a `Real` stream to +-1 (`general.clip`), replacing non-finite samples by 0.
#[derive(Default)]
pub struct Clip;

impl Component for Clip {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.clip",
            vec![Port::real("in")],
            vec![Port::real("out").same_format_as(0)],
        )
        .in_place(0, 0)
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        for s in io.pass_through::<f32>(0, 0) {
            *s = if s.is_finite() {
                s.clamp(-1.0, 1.0)
            } else {
                0.0
            };
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// The squelch gate (`general.gate`): audio (`in`), the squelch decisions (`sq`) and the tone gate (`tone`, optional) in. The audio
/// is clamped to +-1, times a gain that moves towards 1 (open) or 0 (closed) by 1/240 per sample at 48 kHz (5 ms), clamped again.
/// A squelch decision applies from the sample at its time on. The gate is open when the squelch is open, the tone gate is open and
/// the transmitter (`tx`, optional) does not mute the receive audio; a mute applies from the pass in which it comes.
/// Its command `restart` closes the gate at once (gain 0).
pub struct Gate {
    rate: f64,
    step: f32,
    gain: f32,
    open: bool,
    tone_open: bool,
    muted: bool,
    pending: Vec<Timed<SquelchBlock>>,
    /// Samples since the last configure (aligned with the squelch's block times).
    n: u64,
}

impl Default for Gate {
    fn default() -> Self {
        Gate {
            rate: 48e3,
            step: 1.0 / 240.0,
            gain: 0.0,
            open: false,
            tone_open: true,
            muted: false,
            pending: Vec::new(),
            n: 0,
        }
    }
}

impl Component for Gate {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.gate",
            vec![
                Port::real("in"),
                Port::typed::<SquelchBlock>("sq"),
                Port::typed::<ToneId>("tone").optional("the tone gate is always open"),
                Port::typed::<TxKeyed>("tx").optional("never muted by a transmitter"),
            ],
            vec![Port::real("out").same_format_as(0)],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.step = (1.0 / (0.005 * self.rate)) as f32;
        self.gain = 0.0;
        self.open = false;
        self.pending.clear();
        self.n = 0;
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(t) = io.items::<ToneId>(2).last() {
            self.tone_open = t.value.gate_open;
        }
        if let Some(k) = io.items::<TxKeyed>(3).last() {
            self.muted = k.value.0;
        }
        for b in io.items::<SquelchBlock>(1) {
            if self.pending.len() >= 1024 {
                self.pending.remove(0);
            }
            self.pending.push(b.clone());
        }
        let first = self.n;
        let x = io.real(0);
        self.n += x.len() as u64;
        let rate = self.rate;
        let out = io.real_out(0);
        let mut pi = 0;
        for (i, &s) in x.iter().enumerate() {
            let at = first + i as u64;
            while pi < self.pending.len() && (self.pending[pi].time * rate).round() as u64 <= at {
                self.open = self.pending[pi].value.open;
                pi += 1;
            }
            let target = if self.open && self.tone_open && !self.muted {
                1.0
            } else {
                0.0
            };
            if self.gain < target {
                self.gain = (self.gain + self.step).min(1.0);
            } else if self.gain > target {
                self.gain = (self.gain - self.step).max(0.0);
            }
            let v = if s.is_finite() {
                s.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            out.push((v * self.gain).clamp(-1.0, 1.0));
        }
        self.pending.drain(..pi);
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        RestartCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match RestartCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RestartCommand::Restart => {
                self.gain = 0.0;
                self.open = false;
                self.pending.clear();
            }
        }
        Ok(None)
    }
}

/// The response of an [`OptBiquad`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiquadKind {
    /// A high-pass.
    HighPass,
    /// A low-pass.
    LowPass,
    /// A notch.
    Notch,
}

/// One optional biquad: active only when its frequency is above 20 Hz and below 23 900 Hz (and below the Nyquist).
#[derive(Clone, Debug)]
pub struct OptBiquad {
    freq: f64,
    q: f64,
    kind: BiquadKind,
    bq: Option<Biquad>,
}

impl OptBiquad {
    /// An optional biquad of `kind` with quality `q`, off.
    pub fn new(kind: BiquadKind, q: f64) -> Self {
        OptBiquad {
            freq: 0.0,
            q,
            kind,
            bq: None,
        }
    }
    /// A new frequency (0 = off) at `rate`: redesigns the biquad and clears its state.
    pub fn set(&mut self, freq: f64, rate: f64) {
        self.freq = freq;
        self.bq = (freq > 20.0 && freq < 23_900.0 && freq < rate / 2.0).then(|| {
            Biquad::new(match self.kind {
                BiquadKind::HighPass => BiquadCoeffs::highpass(rate, freq, self.q),
                BiquadKind::LowPass => BiquadCoeffs::lowpass(rate, freq, self.q),
                BiquadKind::Notch => BiquadCoeffs::notch(rate, freq, self.q),
            })
        });
    }
    /// The frequency (0 = off).
    pub fn freq(&self) -> f64 {
        self.freq
    }
    /// Filters in place.
    #[inline]
    pub fn process(&mut self, buf: &mut [f32]) {
        if let Some(b) = &mut self.bq {
            b.process(buf);
        }
    }
}

const HIGHPASS: Setting<field::Hz> =
    Setting::new("highpass", "High-pass", "Hz", 0.0).range(0.0, 24_000.0, 0.0);
const LOWPASS: Setting<field::Hz> =
    Setting::new("lowpass", "Low-pass", "Hz", 0.0).range(0.0, 24_000.0, 0.0);
const NOTCH: Setting<field::Hz> =
    Setting::new("notch", "Notch", "Hz", 0.0).range(0.0, 24_000.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`AudioFilters`]: the frequency of each filter (Hz, 0 for off).
    pub enum AudioFiltersSetting {
        /// The high-pass.
        Highpass = HIGHPASS,
        /// The low-pass.
        Lowpass = LOWPASS,
        /// The notch.
        Notch = NOTCH,
    }
}

/// The audio filters (`general.afilters`): a high-pass, a low-pass (Q 0.707) and a notch (Q 12), in that order. A changed
/// frequency designs that biquad again and clears its state.
pub struct AudioFilters {
    rate: f64,
    f: [OptBiquad; 3],
}

impl Default for AudioFilters {
    fn default() -> Self {
        AudioFilters {
            rate: 48e3,
            f: [
                OptBiquad::new(BiquadKind::HighPass, 0.707),
                OptBiquad::new(BiquadKind::LowPass, 0.707),
                OptBiquad::new(BiquadKind::Notch, 12.0),
            ],
        }
    }
}

impl Component for AudioFilters {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.afilters",
            vec![Port::real("in")],
            vec![Port::real("out").same_format_as(0)],
        )
        .in_place(0, 0)
        .with_settings(AudioFiltersSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        for f in &mut self.f {
            let fr = f.freq();
            f.set(fr, self.rate);
        }
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let buf = io.pass_through::<f32>(0, 0);
        for f in &mut self.f {
            f.process(buf);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let (i, fr) = match AudioFiltersSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AudioFiltersSetting::Highpass => (0, HIGHPASS.read(id, v)?),
            AudioFiltersSetting::Lowpass => (1, LOWPASS.read(id, v)?),
            AudioFiltersSetting::Notch => (2, NOTCH.read(id, v)?),
        };
        if fr != self.f[i].freq() {
            self.f[i].set(fr, self.rate);
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (AudioFiltersSetting::Highpass, Value::Hz(self.f[0].freq())),
            (AudioFiltersSetting::Lowpass, Value::Hz(self.f[1].freq())),
            (AudioFiltersSetting::Notch, Value::Hz(self.f[2].freq())),
        ])
    }
}

/// The neural voice noise remover (RNNoise, Valin 2018) on 48 kHz audio, with a dry/wet strength.
///
/// While on, the audio is delayed by two 10 ms frames. While off it passes untouched (bit-identical, no delay).
pub struct VoiceDenoiser {
    state: Box<nnnoiseless::DenoiseState<'static>>,
    inbuf: Vec<f32>,
    frame_out: Vec<f32>,
    outq: Vec<f32>,
    pos: usize,
    dry: DelayLine<f32>,
    /// Dry/wet strength, 0 to 1.
    pub strength: f32,
    /// The speech probability of the last frame.
    pub speech: f32,
}

impl Default for VoiceDenoiser {
    fn default() -> Self {
        Self::new()
    }
}

impl VoiceDenoiser {
    /// The frame length (10 ms at 48 kHz).
    pub const FRAME: usize = nnnoiseless::DenoiseState::FRAME_SIZE;

    /// A new remover.
    pub fn new() -> Self {
        VoiceDenoiser {
            state: nnnoiseless::DenoiseState::new(),
            inbuf: Vec::with_capacity(Self::FRAME),
            frame_out: vec![0.0; Self::FRAME],
            outq: vec![0.0; Self::FRAME],
            pos: 0,
            dry: DelayLine::new(2 * Self::FRAME),
            strength: 0.8,
            speech: 0.0,
        }
    }
    /// The delay while on (samples).
    pub fn delay(&self) -> usize {
        2 * Self::FRAME
    }
    /// Clears the state.
    pub fn reset(&mut self) {
        let s = self.strength;
        *self = VoiceDenoiser::new();
        self.strength = s;
    }
    /// Processes `buf` in place. `on_frame(offset)` is called at each completed frame (offset into `buf`).
    pub fn process(&mut self, buf: &mut [f32], mut on_frame: impl FnMut(usize, f32)) {
        let k = self.strength.clamp(0.0, 1.0);
        for (i, s) in buf.iter_mut().enumerate() {
            let x = *s;
            self.inbuf.push(x * 32768.0);
            let wet = self.outq[self.pos] / 32768.0;
            self.pos += 1;
            let dry = self.dry.push(x);
            *s = (dry * (1.0 - k) + wet * k).clamp(-1.0, 1.0);
            if self.inbuf.len() == Self::FRAME {
                self.speech = self.state.process_frame(&mut self.frame_out, &self.inbuf);
                self.outq.copy_from_slice(&self.frame_out);
                for v in &mut self.outq {
                    if !v.is_finite() {
                        *v = 0.0;
                    }
                }
                self.inbuf.clear();
                self.pos = 0;
                on_frame(i + 1, self.speech);
            }
        }
    }
}

const ENABLED: Setting<field::Bool> = Setting::new("enabled", "Noise remover", "", false);
const STRENGTH: Setting<field::F64> =
    Setting::new("strength", "Strength", "%", 80.0).range(0.0, 100.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`VoiceNr`].
    pub enum VoiceNrSetting {
        /// The remover works.
        Enabled = ENABLED,
        /// The share of the cleaned sound in the output (%).
        Strength = STRENGTH,
    }
}

/// The voice noise remover component (`general.voice_nr`): `Real` 48 kHz in and out, and the speech probability per 10 ms.
pub struct VoiceNr {
    enabled: bool,
    strength: f64,
    rate: f64,
    d: Option<Box<VoiceDenoiser>>,
}

impl Default for VoiceNr {
    fn default() -> Self {
        VoiceNr {
            enabled: false,
            strength: 80.0,
            rate: 48e3,
            d: None,
        }
    }
}

impl Component for VoiceNr {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.voice_nr",
            vec![Port::real("in")],
            vec![
                Port::real("out").same_format_as(0),
                Port::typed::<SpeechProbability>("speech").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .in_place(0, 0)
        .with_settings(VoiceNrSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        if let Some(d) = &mut self.d {
            d.reset();
        }
        Ok(vec![
            inputs[0].clone(),
            Format::new(Kind::typed::<SpeechProbability>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.output(0).end();
        let n = io.input_len(0);
        if !self.enabled || (self.rate - 48e3).abs() > 1e-6 {
            io.pass_through::<f32>(0, 0);
            // The speech probability reads 0 while the remover is off.
            if io.tick() {
                let t = (first + n as u64) as f64 / self.rate;
                io.items_out::<SpeechProbability>(1).push(Timed {
                    time: t,
                    value: SpeechProbability(0.0),
                });
            }
            return;
        }
        let strength = (self.strength / 100.0) as f32;
        let d = self.d.get_or_insert_with(|| Box::new(VoiceDenoiser::new()));
        d.strength = strength;
        let mut probs: [(usize, f32); 64] = [(0, 0.0); 64];
        let mut np = 0;
        {
            let out = io.pass_through::<f32>(0, 0);
            d.process(out, |at, p| {
                if np < probs.len() {
                    probs[np] = (at, p);
                    np += 1;
                }
            });
        }
        let rate = self.rate;
        let sp = io.items_out::<SpeechProbability>(1);
        for &(at, p) in &probs[..np] {
            sp.push(Timed {
                time: (first + at as u64) as f64 / rate,
                value: SpeechProbability(p),
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match VoiceNrSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            VoiceNrSetting::Enabled => {
                let e = ENABLED.read(id, v)?;
                if e && !self.enabled {
                    if let Some(d) = &mut self.d {
                        d.reset();
                    }
                }
                self.enabled = e;
            }
            VoiceNrSetting::Strength => self.strength = STRENGTH.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (VoiceNrSetting::Enabled, Value::Bool(self.enabled)),
            (VoiceNrSetting::Strength, Value::F64(self.strength)),
        ])
    }
}
