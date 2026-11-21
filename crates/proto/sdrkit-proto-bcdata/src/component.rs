//! `bcdata.playout`: the [`Playout`] as a graph component.
//!
//! The input carries [`Pcm`] items: decoded sound, or the number of lost frames. The output is stereo audio at the `rate`
//! setting. The item times are the signal clock: by the time of an item, the component has released `rate × elapsed` frames
//! since the first item.

use crate::playout::{Concealment, Config, Playout};
use sdrkit_algo_dsp::Frame;
use sdrkit_core::interface::{Fields, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, ConfigError, Results, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, ChannelsFrom, Component, ComponentInfo, Every, Format, Io, Item, Kept,
    Port, RateFrom, Registry,
};
use std::sync::Arc;

/// Decoded sound, or lost frames, for the playout.
#[derive(Clone, Debug)]
pub enum Pcm {
    /// Real sound at the playout rate, as stereo frames.
    Sound(Arc<[Frame<2>]>),
    /// This many lost frames.
    Lost(usize),
}

impl Item for Pcm {
    const KIND: &'static str = "bcdata.pcm";
}

/// The counts of the playout since the start or the last change of frequency, written once a second while sound
/// plays. No value before the first report.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PlayoutFields {
    /// Time filled by concealment above silence.
    #[field(label = "Concealed", unit = "s")]
    pub concealed: Option<Seconds>,
    /// Time of silence (faded concealment, or waiting to start).
    #[field(label = "Muted", unit = "s")]
    pub muted: Option<Seconds>,
    /// Concealments started.
    #[field(label = "Concealments")]
    pub concealments: Option<i64>,
    /// Times the queue ran empty.
    #[field(label = "Underruns")]
    pub underruns: Option<i64>,
}

impl PlayoutFields {
    /// The set without a value.
    const NONE: PlayoutFields = PlayoutFields {
        concealed: None,
        muted: None,
        concealments: None,
        underruns: None,
    };
}

const CONCEALMENT: Setting<field::Choice<Concealment>> =
    Setting::choice("concealment", "Concealment", Concealment::Pitch)
        .help("How lost audio frames are filled.");
const RATE: Setting<field::Hz> = Setting::new("rate", "Rate", "Hz", 48_000.0)
    .range(8_000.0, 96_000.0, 1.0)
    .help("The audio rate of the decoded sound and of the output.");

sdrkit_core::settings! {
    /// The settings of [`PlayoutComponent`].
    pub enum PlayoutSetting {
        /// How lost frames are filled.
        Concealment = CONCEALMENT,
        /// The audio rate.
        Rate = RATE,
    }
}

/// The playout component.
pub struct PlayoutComponent {
    play: Playout,
    t0: Option<f64>,
    released: u64,
    scratch: Vec<Frame<2>>,
    results: Published,
    every: Every,
}

impl Default for PlayoutComponent {
    fn default() -> Self {
        Self::build(RATE.default as u32, CONCEALMENT.default)
    }
}

impl PlayoutComponent {
    /// A playout component at `rate` Hz with pitch concealment.
    ///
    /// # Errors
    ///
    /// [`SettingError::OutOfRange`] for a rate outside the `rate` setting.
    pub fn new(rate: u32) -> Result<Self, SettingError> {
        PlayoutSetting::Rate.check(&Value::Hz(f64::from(rate)))?;
        Ok(Self::build(rate, CONCEALMENT.default))
    }

    fn build(rate: u32, concealment: Concealment) -> Self {
        PlayoutComponent {
            play: Playout::new(Config::new(rate, concealment)),
            t0: None,
            released: 0,
            scratch: Vec::with_capacity(rate as usize),
            results: Published::new(PlayoutFields::SHAPE.fields),
            every: Every::new(f64::from(rate), 1.0),
        }
    }

    /// The playout inside.
    pub fn playout(&self) -> &Playout {
        &self.play
    }
}

impl Component for PlayoutComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "bcdata.playout",
            vec![Port::typed::<Pcm>("in")],
            vec![Port::audio("out")
                .rate_from(RateFrom::Setting(PlayoutSetting::Rate.id()))
                .channels_from(ChannelsFrom::Fixed(2))],
        )
        .with_settings(PlayoutSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.play.reset();
        self.t0 = None;
        self.released = 0;
        Ok(vec![Format::audio(f64::from(self.play.config().rate), 2)])
    }

    fn process(&mut self, io: &mut Io) {
        let rate = f64::from(self.play.config().rate);
        let mut out = std::mem::take(&mut self.scratch);
        out.clear();
        for it in io.items::<Pcm>(0) {
            match &it.value {
                Pcm::Sound(f) => self.play.push_sound(f),
                Pcm::Lost(n) => self.play.push_lost(*n),
            }
            let t0 = *self.t0.get_or_insert(it.time);
            let due = ((it.time - t0) * rate).round().max(0.0) as u64;
            if due > self.released {
                self.play.pull((due - self.released) as usize, &mut out);
                self.released = due;
            }
        }
        let n = out.len();
        io.audio_out(0).samples.extend_from_slice(Frame::flat(&out));
        self.scratch = out;
        if self.every.tick(n) {
            let s = self.play.stats();
            self.results.put(&PlayoutFields {
                concealed: Some(Seconds(s.concealed_s)),
                muted: Some(Seconds(s.muted_s)),
                concealments: Some(s.concealments as i64),
                underruns: Some(s.underruns as i64),
            });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PlayoutSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PlayoutSetting::Concealment => {
                self.play.set_concealment(CONCEALMENT.read(id, v)?);
                Ok(Applied::Live)
            }
            PlayoutSetting::Rate => {
                let rate = RATE.read(id, v)? as u32;
                *self = Self::build(rate, self.play.config().concealment);
                Ok(Applied::Reformat)
            }
        }
    }

    fn values(&self) -> Values {
        let c = self.play.config();
        values_of([
            (
                PlayoutSetting::Concealment,
                Value::Choice(c.concealment.index()),
            ),
            (PlayoutSetting::Rate, Value::Hz(f64::from(c.rate))),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }

    /// The counts belong to the frequency before: they start again from zero, and the set has no value until the
    /// next report.
    fn forget(&mut self, _kept: &Kept) {
        self.play.clear_counts();
        self.results.put(&PlayoutFields::NONE);
    }
}

/// Adds `bcdata.playout`.
pub fn register(r: &mut Registry) {
    r.add("bcdata.playout", || Box::new(PlayoutComponent::default()));
}
