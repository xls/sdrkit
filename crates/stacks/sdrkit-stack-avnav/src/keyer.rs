//! Keying: when an identification tone or a carrier is on and off. The tone keyer measures the tone with the shared Goertzel
//! tone bank, and the shared key state turns decisions into the lengths of marks and gaps.

use crate::typed::KeyerFields;
use sdrkit_algo_dsp::goertzel::ToneBank;
use sdrkit_algo_modem::key::KeyEvent;
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, ConfigError, Published, Results, Setting, SettingError, SettingId, SettingSet, Value,
    Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};

/// One finished mark or gap (or a gap still going, `closed == false`, reported every half second so a word can end).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keying {
    /// Key down (tone on) during the element.
    pub on: bool,
    /// How long it lasted, seconds.
    pub duration_s: f64,
    /// The element ended (a gap still going is reported with `false`).
    pub closed: bool,
    /// The mean level during the element (modulation depth for a tone, 0..1).
    pub level: f32,
}

impl Item for Keying {
    const KIND: &'static str = "keying";
}

/// The tones an automatic keyer listens for: 400 and 1020 Hz identification, and 1350 Hz (DME).
pub const ID_TONES: [f64; 3] = [400.0, 1020.0, 1350.0];

pub use sdrkit_algo_modem::key::KeyState;

/// A new key state for decisions on samples at `rate`: a change must last two decisions, and a space still going is reported
/// every half second so that a word can end.
pub fn key_state(rate: f64) -> KeyState {
    KeyState::new(rate, 2, 0.5).unwrap_or_default()
}

/// Adds a key event, if any, to `out` as a timed [`Keying`].
pub fn push_key(out: &mut Vec<Timed<Keying>>, e: Option<KeyEvent>) {
    if let Some(e) = e {
        out.push(Timed {
            time: e.end,
            value: Keying {
                on: e.on,
                duration_s: e.seconds,
                closed: e.closed,
                level: e.level,
            },
        });
    }
}

const TONE: Setting<field::Hz> = Setting::new("tone", "Tone", "Hz", 0.0)
    .range(0.0, 5000.0, 0.0)
    .help("The identification tone. 0 finds it (400, 1020 or 1350 Hz).");
const MIN_DEPTH: Setting<field::F64> = Setting::new("min_depth", "Least depth", "", 0.04)
    .range(0.0, 1.0, 0.0)
    .help("The least modulation depth of the tone that counts as key down.");
const PERIOD: Setting<field::Duration> =
    Setting::new("period", "Decision time", "s", 0.01).range(0.002, 0.1, 0.0);

sdrkit_core::settings! {
    /// The settings of [`ToneKeyer`].
    pub enum ToneKeyerSetting {
        /// The identification tone, 0 to find it.
        Tone = TONE,
        /// The least modulation depth that counts as a mark.
        MinDepth = MIN_DEPTH,
        /// The time of each decision.
        Period = PERIOD,
    }
}

/// The identification tone keyer (`avnav.tone_keyer`): real (AM detected and divided by the carrier, so a tone's amplitude is its
/// modulation depth) -> [`Keying`]. In each period it measures every tone it listens for and the noise 180 Hz either side of it.
/// A period is key down when the tone stands above the noise, the least depth and a share of the recent peak.
pub struct ToneKeyer {
    tone: f64,
    min_depth: f64,
    period_s: f64,
    rate: f64,
    period: usize,
    first: bool,
    /// Periods since the keyer was configured.
    periods: u32,
    /// For each tone: the tone, 180 Hz below and 180 Hz above.
    bank: Option<ToneBank>,
    freqs: Vec<f64>,
    long: Vec<f64>,
    chosen: usize,
    noise: f64,
    peak: f64,
    state: KeyState,
    published: Published,
    depth: f64,
}

impl Default for ToneKeyer {
    fn default() -> Self {
        ToneKeyer::make(TONE.default)
    }
}

impl ToneKeyer {
    /// Listening for `tone_hz` (0: automatic).
    ///
    /// # Errors
    ///
    /// The [`SettingError`] of the `tone` setting for a tone outside its range.
    pub fn new(tone_hz: f64) -> Result<Self, SettingError> {
        ToneKeyerSetting::Tone.check(&Value::Hz(tone_hz))?;
        Ok(ToneKeyer::make(tone_hz))
    }
    fn make(tone_hz: f64) -> Self {
        ToneKeyer {
            tone: tone_hz,
            min_depth: MIN_DEPTH.default,
            period_s: PERIOD.default,
            rate: 0.0,
            period: 1,
            first: true,
            periods: 0,
            bank: None,
            freqs: Vec::new(),
            long: Vec::new(),
            chosen: 0,
            noise: 0.0,
            peak: 0.0,
            state: KeyState::default(),
            published: Published::new(KeyerFields::SHAPE.fields),
            depth: 0.0,
        }
    }
    fn build(&mut self) -> Result<(), ConfigError> {
        let freqs: Vec<f64> = if self.tone > 0.0 {
            vec![self.tone]
        } else {
            ID_TONES
                .iter()
                .copied()
                .filter(|f| *f < 0.45 * self.rate)
                .collect()
        };
        let side = 180.0;
        let all: Vec<f64> = freqs
            .iter()
            .flat_map(|&f| [f, f - side, f + side])
            .collect();
        self.period = ((self.rate * self.period_s).round() as usize).max(8);
        self.bank = Some(ToneBank::with_rate(self.rate, &all, self.period)?);
        self.long = vec![0.0; freqs.len()];
        self.freqs = freqs;
        self.chosen = 0;
        self.first = true;
        self.periods = 0;
        self.noise = 0.0;
        self.peak = 0.0;
        self.state = key_state(self.rate);
        Ok(())
    }
    /// The tone listened to now, Hz.
    pub fn tone_hz(&self) -> f64 {
        self.freqs.get(self.chosen).copied().unwrap_or(0.0)
    }
    /// One period's amplitudes (three for each tone) ending at `time`.
    fn close_period(
        &mut self,
        amps: &[num_complex::Complex32],
        time: f64,
        out: &mut Vec<Timed<Keying>>,
    ) {
        let mut chosen = [0.0f64; 2];
        let mut best = 0;
        for (i, t) in amps.as_chunks::<3>().0.iter().enumerate() {
            let a = t[0].norm() as f64;
            let s = 0.5 * (t[1].norm() + t[2].norm()) as f64;
            self.long[i] += 0.01 * ((a - s).max(0.0) - self.long[i]);
            if self.long[i] > self.long[best] {
                best = i;
            }
            if i == self.chosen {
                chosen = [a, s];
            }
        }
        // In the first second the strongest tone wins at once, so the first identification is read on its tone. After that,
        // change tone only when another is clearly stronger.
        self.periods = self.periods.saturating_add(1);
        let settled = self.periods as f64 * self.period_s > 1.0;
        if best != self.chosen
            && (!settled || self.long[best] > 1.5 * self.long[self.chosen] + 1e-3)
        {
            self.chosen = best;
        }
        let [a, s] = chosen;
        // The noise starts at the first period that has any (the carrier stage outputs silence while its level settles).
        if self.first && s > 0.0 {
            self.noise = s;
            self.first = false;
        }
        self.noise += 0.05 * (s - self.noise);
        self.peak = (self.peak * 0.995).max(a.min(1.0));
        let thr = self.min_depth.max(3.5 * self.noise).max(0.35 * self.peak);
        // No key down while the noise estimate settles (0.2 s).
        let warm = self.periods as f64 * self.period_s > 0.2;
        let want = warm
            && if self.state.is_on() {
                a > 0.7 * thr
            } else {
                a > thr
            };
        if want {
            self.depth += 0.1 * (a - self.depth);
        }
        push_key(
            out,
            self.state.decide(want, a as f32, self.period as u64, time),
        );
    }
}

impl Component for ToneKeyer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.tone_keyer",
            vec![Port::real("in")],
            vec![Port::typed::<Keying>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(ToneKeyerSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        self.rate = f.rate;
        self.build()?;
        Ok(vec![Format {
            rate: 0.0,
            ..f.with_kind(Kind::typed::<Keying>())
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let x = io.real(0);
        let out = io.items_out::<Keying>(0);
        let rate = self.rate;
        if let Some(mut bank) = self.bank.take() {
            bank.process(x, |end, amps| {
                self.close_period(amps, (first + end as u64) as f64 / rate, out)
            });
            self.bank = Some(bank);
        }
        let tone = self.tone_hz();
        self.published.put(&KeyerFields {
            id_tone_hz: Some(Hertz(tone)),
            id_depth_pct: Some(self.depth * 100.0),
        });
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ToneKeyerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ToneKeyerSetting::Tone => {
                self.tone = TONE.read(id, v)?;
                Ok(Applied::Reformat)
            }
            ToneKeyerSetting::MinDepth => {
                self.min_depth = MIN_DEPTH.read(id, v)?;
                Ok(Applied::Live)
            }
            ToneKeyerSetting::Period => {
                self.period_s = PERIOD.read(id, v)?;
                Ok(Applied::Reformat)
            }
        }
    }
    fn values(&self) -> Values {
        values_of([
            (ToneKeyerSetting::Tone, Value::Hz(self.tone)),
            (ToneKeyerSetting::MinDepth, Value::F64(self.min_depth)),
            (ToneKeyerSetting::Period, Value::Duration(self.period_s)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

sdrkit_core::choice_enum! {
    /// Which keying stream [`KeySelect`] passes on.
    pub enum KeySource {
        /// The second stream while it keys, else the first.
        Auto = ("auto", "Automatic"),
        /// The first stream (a keyed tone).
        A = ("a", "Keyed tone (A2A)"),
        /// The second stream (a keyed carrier).
        B = ("b", "Keyed carrier (A1A)"),
    }
}

const SOURCE: Setting<field::Choice<KeySource>> =
    Setting::choice("source", "Keying", KeySource::Auto);

sdrkit_core::settings! {
    /// The settings of [`KeySelect`].
    pub enum KeySelectSetting {
        /// The stream passed on.
        Source = SOURCE,
    }
}

/// Passes one of two keying streams on (`avnav.key_select`): the first (`a`, a keyed tone), the second (`b`, a keyed
/// carrier), or with [`KeySource::Auto`] the one that keys deeper. The depth of a stream is the mean level of its marks over
/// the mean level of its gaps. The selector leaves the tone for the carrier when the carrier keys [`TO_CARRIER`] times
/// deeper, and the carrier for the tone when the tone keys [`TO_TONE`] times deeper; a stream that has not keyed for 15 s has
/// no depth.
pub struct KeySelect {
    source: KeySource,
    last: [f64; 2],
    chosen: usize,
    /// The mean level of the marks and of the gaps of each stream.
    marks: [f32; 2],
    gaps: [f32; 2],
}

/// The carrier must key this many times deeper than the tone for the selector to leave the tone.
pub const TO_CARRIER: f32 = 1.3;
/// The tone must key this many times deeper than the carrier for the selector to leave the carrier.
pub const TO_TONE: f32 = 2.5;

impl Default for KeySelect {
    fn default() -> Self {
        KeySelect::new(SOURCE.default)
    }
}

impl KeySelect {
    /// Passing on `source`.
    pub fn new(source: KeySource) -> Self {
        KeySelect {
            source,
            last: [f64::MIN; 2],
            chosen: 0,
            marks: [0.0; 2],
            gaps: [0.0; 2],
        }
    }

    /// The keying depth of stream `i` at time `now`: 1 (no keying) for a stream without a mark in the last 15 s.
    fn depth(&self, i: usize, now: f64) -> f32 {
        if now - self.last[i] < 15.0 && self.gaps[i] > 0.0 {
            (self.marks[i] / self.gaps[i]).max(1.0)
        } else {
            1.0
        }
    }
}

impl Component for KeySelect {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.key_select",
            vec![Port::typed::<Keying>("a"), Port::typed::<Keying>("b")],
            vec![Port::typed::<Keying>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(KeySelectSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.last = [f64::MIN; 2];
        self.chosen = 0;
        self.marks = [0.0; 2];
        self.gaps = [0.0; 2];
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let a = io.items::<Keying>(0);
        let b = io.items::<Keying>(1);
        for (i, s) in [a, b].iter().enumerate() {
            if let Some(m) = s.iter().rev().find(|k| k.value.on) {
                self.last[i] = m.time;
            }
            for k in s.iter().filter(|k| k.value.closed) {
                let mean = if k.value.on {
                    &mut self.marks[i]
                } else {
                    &mut self.gaps[i]
                };
                *mean = if *mean > 0.0 {
                    *mean + 0.2 * (k.value.level - *mean)
                } else {
                    k.value.level
                };
            }
        }
        let pick = match self.source {
            KeySource::A => 0,
            KeySource::B => 1,
            KeySource::Auto => {
                let now = a
                    .last()
                    .map_or(f64::MIN, |k| k.time)
                    .max(b.last().map_or(f64::MIN, |k| k.time));
                let (tone, carrier) = (self.depth(0, now), self.depth(1, now));
                match self.chosen {
                    0 if carrier >= TO_CARRIER * tone => 1,
                    1 if tone >= TO_TONE * carrier || carrier <= 1.0 => 0,
                    chosen => chosen,
                }
            }
        };
        let src = if pick == 0 { a } else { b };
        let out = io.items_out::<Keying>(0);
        if pick != self.chosen {
            // A word break between the two sources, so nothing of one is read with the other.
            self.chosen = pick;
            let time = src.first().map_or(0.0, |k| k.time);
            out.push(Timed {
                time,
                value: Keying {
                    on: false,
                    duration_s: 60.0,
                    closed: false,
                    level: 0.0,
                },
            });
        }
        out.extend_from_slice(src);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match KeySelectSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            KeySelectSetting::Source => self.source = SOURCE.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(KeySelectSetting::Source, Value::Choice(self.source.index()))])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morse::MorseDecoder;
    use sdrkit_util_text::morse::Keyer;

    #[test]
    fn key_state_reports_lengths() {
        let mut k = key_state(1000.0);
        let mut out = Vec::new();
        let mut t = 0.0;
        // 100 ms on, 300 ms off, in decisions of 10 samples.
        for i in 0..60 {
            t += 0.01;
            push_key(&mut out, k.decide((10..20).contains(&i), 1.0, 10, t));
        }
        let closed: Vec<_> = out.iter().filter(|k| k.value.closed).collect();
        assert!(
            (closed[1].value.duration_s - 0.1).abs() < 1e-9,
            "{closed:?}"
        );
        assert!(closed[1].value.on);
    }

    #[test]
    fn keyer_and_decoder_read_a_keyed_tone() {
        let rate = 8000.0;
        let mut x = Vec::new();
        let keyer = Keyer::new(10.0).unwrap();
        for _ in 0..3 {
            for k in keyer.key("ABC") {
                let len = (k.seconds * rate) as usize;
                for _ in 0..len {
                    let t = x.len() as f64 / rate;
                    let v = if k.on {
                        0.3 * (std::f64::consts::TAU * 1020.0 * t).cos()
                    } else {
                        0.0
                    };
                    x.push(v as f32);
                }
            }
            x.extend(std::iter::repeat_n(0.0f32, (3.0 * rate) as usize));
        }
        let mut keyer = ToneKeyer {
            rate,
            ..Default::default()
        };
        keyer.build().unwrap();
        let mut bank = keyer.bank.take().unwrap();
        let mut out = Vec::new();
        bank.process(&x, |end, amps| {
            keyer.close_period(amps, end as f64 / rate, &mut out)
        });
        let mut words = Vec::new();
        let mut dec = MorseDecoder::default();
        for k in out {
            if let Some(w) = dec.push(k.time, &k.value) {
                words.push(w.text);
            }
        }
        assert_eq!(keyer.tone_hz(), 1020.0);
        assert!(
            words.iter().filter(|w| *w == "ABC").count() >= 2,
            "{words:?}"
        );
    }
}
