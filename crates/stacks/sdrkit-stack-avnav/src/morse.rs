//! Morse identifications: the decoder component that turns keying into words. The decoder and the list that confirms an
//! identification heard several times are the shared ones of `sdrkit-util-text`.
//! Standard: ITU-R M.1677-1 Part I.

use crate::keyer::Keying;
use crate::typed::{IdentWordEvent, IDENT_WORD};
use sdrkit_core::interface::Fields as _;
use sdrkit_core::publish::{field, keys};
use sdrkit_core::{ConfigError, EventDef, Record, Setting, SettingError, SettingId, SettingSet};
use sdrkit_core::{Value, Values};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};
pub use sdrkit_util_text::morse::IdentTracker;
use sdrkit_util_text::morse::{Decoder, DecoderConfig, Event};

/// A decoded Morse word. It leaves the graph as the event `morse.word`.
#[derive(Clone, Debug, PartialEq)]
pub struct MorseWord {
    /// The characters (`?` for a code that is not in the table).
    pub text: String,
    /// The speed, words a minute (PARIS: 1.2 s for each dot length).
    pub wpm: f32,
    /// The mean level of the word's marks over the mean level of the gaps before and between them, dB (20 log of the ratio).
    /// `None` when no gap was measured.
    pub contrast_db: Option<f32>,
}

impl Item for MorseWord {
    const KIND: &'static str = "morse.word";
    fn event_def() -> Option<&'static EventDef> {
        Some(IDENT_WORD.def())
    }
    fn write_fields(&self, r: &mut Record) {
        IdentWordEvent {
            source: &self.text,
            text: &self.text,
            family: keys::Family::Other,
            callsign: &self.text,
            locator: None,
            snr_db: None,
            freq_hz: None,
            mode: "CW",
            wpm: f64::from(self.wpm),
        }
        .write(r);
    }
}

const WORD_GAP: Setting<field::Duration> = Setting::new("word_gap", "Word gap", "s", 2.0)
    .range(0.3, 10.0, 0.0)
    .help("A gap this long ends a word, whatever the speed.");
const MIN_WPM: Setting<field::F64> =
    Setting::new("min_wpm", "Slowest", "wpm", 2.0).range(1.0, 60.0, 0.0);
const MAX_WPM: Setting<field::F64> =
    Setting::new("max_wpm", "Fastest", "wpm", 40.0).range(1.0, 80.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`MorseDecoder`].
    pub enum MorseDecoderSetting {
        /// A gap this long ends a word.
        WordGap = WORD_GAP,
        /// A word slower than this is dropped as noise.
        MinWpm = MIN_WPM,
        /// A word faster than this is dropped as noise.
        MaxWpm = MAX_WPM,
    }
}

/// The Morse decoder (`avnav.morse_decoder`): [`Keying`] in, [`MorseWord`] out. It runs the shared decoder for identifications,
/// which fits the dot length to each whole word, so a single identification at any speed reads right.
pub struct MorseDecoder {
    dec: Decoder,
    word: String,
    start: f64,
    /// The level times the length of the marks and of the closed gaps since the last word, and their lengths.
    marks: (f64, f64),
    gaps: (f64, f64),
}

impl Default for MorseDecoder {
    fn default() -> Self {
        let mut c = DecoderConfig::idents();
        c.word_gap_s = WORD_GAP.default;
        c.min_wpm = MIN_WPM.default;
        c.max_wpm = MAX_WPM.default;
        MorseDecoder {
            dec: Decoder::new(c).unwrap_or_else(|_| unreachable!("the default settings are valid")),
            word: String::with_capacity(64),
            start: 0.0,
            marks: (0.0, 0.0),
            gaps: (0.0, 0.0),
        }
    }
}

impl MorseDecoder {
    /// Takes one keying element that ends at `time`. Returns a finished word, if this element ends one.
    pub fn push(&mut self, time: f64, k: &Keying) -> Option<MorseWord> {
        let mut v = Vec::new();
        self.step(time, k, &mut v);
        v.pop().map(|t| t.value)
    }

    fn step(&mut self, time: f64, k: &Keying, out: &mut Vec<Timed<MorseWord>>) {
        if k.on && !self.dec.is_busy() {
            self.start = time - k.duration_s;
        }
        if k.closed && k.duration_s > 0.0 {
            let sums = if k.on {
                &mut self.marks
            } else {
                &mut self.gaps
            };
            sums.0 += f64::from(k.level) * k.duration_s;
            sums.1 += k.duration_s;
        }
        let word = &mut self.word;
        let mut put = |e: Event| {
            if let Event::Char(s) = e {
                word.push_str(s.text_or("?"));
            }
        };
        if k.on || k.closed {
            self.dec.push(k.on, k.duration_s, &mut put);
        } else {
            self.dec.idle(k.duration_s, &mut put);
        }
        // The decoder outputs the characters of a word together, when the word ends.
        if !self.word.is_empty() {
            let (marks, gaps) = (self.marks, self.gaps);
            self.marks = (0.0, 0.0);
            self.gaps = (0.0, 0.0);
            let contrast_db = (marks.1 > 0.0 && gaps.1 > 0.0 && gaps.0 > 0.0 && marks.0 > 0.0)
                .then(|| (20.0 * ((marks.0 / marks.1) / (gaps.0 / gaps.1)).log10()) as f32);
            out.push(Timed {
                time: self.start.max(0.0).min(time),
                value: MorseWord {
                    text: std::mem::take(&mut self.word),
                    wpm: self.dec.wpm() as f32,
                    contrast_db,
                },
            });
        }
    }

    /// Makes the decoder again with `f` applied to its settings. A slowest speed above the fastest is refused with
    /// [`SettingError::Conflict`] on `id`.
    fn set_config(
        &mut self,
        id: SettingId,
        f: impl FnOnce(&mut DecoderConfig),
    ) -> Result<(), SettingError> {
        let mut c = *self.dec.config();
        f(&mut c);
        let with = if id == MorseDecoderSetting::MinWpm.id() {
            MorseDecoderSetting::MaxWpm.id()
        } else {
            MorseDecoderSetting::MinWpm.id()
        };
        if c.min_wpm > c.max_wpm {
            return Err(SettingError::Conflict { id, with });
        }
        c.wpm = c.wpm.clamp(c.min_wpm, c.max_wpm);
        self.dec = Decoder::new(c).map_err(|_| SettingError::Conflict { id, with })?;
        Ok(())
    }
}

impl Component for MorseDecoder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.morse_decoder",
            vec![Port::typed::<Keying>("in")],
            vec![Port::typed::<MorseWord>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(MorseDecoderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.dec.reset();
        self.word.clear();
        self.marks = (0.0, 0.0);
        self.gaps = (0.0, 0.0);
        Ok(vec![Format {
            rate: 0.0,
            ..inputs[0].with_kind(Kind::typed::<MorseWord>())
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let input = io.items::<Keying>(0);
        let out = io.items_out::<MorseWord>(0);
        for k in input {
            self.step(k.time, &k.value, out);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match MorseDecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MorseDecoderSetting::WordGap => {
                let x = WORD_GAP.read(id, v)?;
                self.set_config(id, |c| c.word_gap_s = x)?;
            }
            MorseDecoderSetting::MinWpm => {
                let x = MIN_WPM.read(id, v)?;
                self.set_config(id, |c| c.min_wpm = x)?;
            }
            MorseDecoderSetting::MaxWpm => {
                let x = MAX_WPM.read(id, v)?;
                self.set_config(id, |c| c.max_wpm = x)?;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let c = self.dec.config();
        values_of([
            (MorseDecoderSetting::WordGap, Value::Duration(c.word_gap_s)),
            (MorseDecoderSetting::MinWpm, Value::F64(c.min_wpm)),
            (MorseDecoderSetting::MaxWpm, Value::F64(c.max_wpm)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_util_text::morse::Keyer;

    fn run(text: &str, wpm: f64) -> Vec<String> {
        let mut d = MorseDecoder::default();
        let mut words = Vec::new();
        let mut t = 0.0;
        let keyer = Keyer::new(wpm).unwrap();
        for _ in 0..2 {
            let gaps = keyer.key(text).chain([sdrkit_util_text::morse::Key {
                on: false,
                seconds: 3.0,
            }]);
            for k in gaps {
                t += k.seconds;
                let e = Keying {
                    on: k.on,
                    duration_s: k.seconds,
                    closed: true,
                    level: 0.0,
                };
                if let Some(w) = d.push(t, &e) {
                    words.push(w.text);
                }
            }
        }
        words
    }

    #[test]
    fn decodes_idents_at_any_speed() {
        for (w, wpm) in [
            ("SYD", 7.0),
            ("TOM", 10.0),
            ("EEE", 12.0),
            ("OMO", 6.0),
            ("IGN", 15.0),
        ] {
            assert_eq!(run(w, wpm), vec![w.to_string(), w.to_string()], "{w}");
        }
    }

    #[test]
    fn a_slowest_speed_above_the_fastest_is_refused() {
        let mut d = MorseDecoder::default();
        let id = MorseDecoderSetting::MinWpm.id();
        assert_eq!(
            d.apply(id, &Value::F64(50.0)),
            Err(SettingError::Conflict {
                id,
                with: MorseDecoderSetting::MaxWpm.id()
            })
        );
    }
}
