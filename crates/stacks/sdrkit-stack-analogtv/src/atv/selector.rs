//! The sound selector: NICAM when it is valid, else A2 in stereo or dual mode, else the FM or AM carrier. The carrier chain is the
//! audio clock, and the second-sound decoders are taken at its pace.

use super::{last, A2Report, NicamReport, SelectorReport, SoundReport, Tune};
use crate::nicam::Mode;
use crate::results::{A2Mode, SoundPlaying};
use crate::settings::{DualSound, SoundChoice, DUAL_SOUND, SOUND_SYSTEM};
use sdrkit_core::{Choice, ConfigError, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    values_of, Applied, ChannelRule, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom,
    Timed,
};
use std::collections::VecDeque;

sdrkit_core::settings! {
    /// The settings of the sound selector.
    pub enum SelectorSetting {
        /// The sound played.
        SoundSystem = SOUND_SYSTEM,
        /// The language of a two-language second sound.
        DualSound = DUAL_SOUND,
    }
}

/// Where the sound comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Carrier,
    Nicam,
    A2,
}

/// The sound selector.
pub struct SoundSelector {
    carrier_only: bool,
    second_lang: bool,
    nicam: VecDeque<[f32; 2]>,
    a2: VecDeque<[f32; 2]>,
    source: Source,
    playing: bool,
    name: SoundPlaying,
    am: bool,
    lock: bool,
    nicam_rep: Option<NicamReport>,
    a2_rep: Option<A2Report>,
}

impl Default for SoundSelector {
    fn default() -> Self {
        SoundSelector {
            carrier_only: false,
            second_lang: false,
            nicam: VecDeque::with_capacity(24_000),
            a2: VecDeque::with_capacity(24_000),
            source: Source::Carrier,
            playing: false,
            name: SoundPlaying::Fm,
            am: false,
            lock: false,
            nicam_rep: None,
            a2_rep: None,
        }
    }
}

const START: usize = 960;
const CAP: usize = 14_400;

impl Component for SoundSelector {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.selector",
            vec![
                Port::audio("carrier").channels(ChannelRule::Exact(2)),
                Port::audio("nicam").channels(ChannelRule::Exact(2)),
                Port::audio("a2").channels(ChannelRule::Exact(2)),
                Port::typed::<SoundReport>("sound"),
                Port::typed::<NicamReport>("nicam_report"),
                Port::typed::<A2Report>("a2_report"),
                Port::typed::<Tune>("control"),
            ],
            vec![
                Port::audio("out").same_format_as(0),
                Port::typed::<SelectorReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(SelectorSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.nicam.clear();
        self.a2.clear();
        self.source = Source::Carrier;
        self.playing = false;
        self.lock = false;
        self.nicam_rep = None;
        self.a2_rep = None;
        Ok(vec![
            inputs[0].clone(),
            Format::new(Kind::typed::<SelectorReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(t) = last::<Tune>(io, 6) {
            self.lock = t.second_sound;
        }
        self.nicam.extend(io.audio(1).stereo());
        self.a2.extend(io.audio(2).stereo());
        while self.nicam.len() > CAP {
            self.nicam.pop_front();
        }
        while self.a2.len() > CAP {
            self.a2.pop_front();
        }
        if let Some(s) = last::<SoundReport>(io, 3) {
            self.am = s.am;
        }
        if let Some(n) = last::<NicamReport>(io, 4) {
            self.nicam_rep = Some(n);
        }
        if let Some(a) = last::<A2Report>(io, 5) {
            self.a2_rep = Some(a);
        }
        let nicam = &self.nicam_rep;
        let a2 = &self.a2_rep;
        if io.tick() {
            let lock = self.lock;
            let n_ok = nicam.as_ref().is_some_and(|n| n.valid) && lock;
            let a_ok = a2.as_ref().is_some_and(|a| {
                a.present && matches!(a.mode, Some(A2Mode::Stereo | A2Mode::Dual))
            }) && lock;
            let src = if self.carrier_only {
                Source::Carrier
            } else if n_ok {
                Source::Nicam
            } else if a_ok {
                Source::A2
            } else {
                Source::Carrier
            };
            if src != self.source {
                self.source = src;
                self.playing = false;
            }
            self.name = match src {
                Source::Nicam => match nicam.as_ref().and_then(|n| n.mode) {
                    Some(Mode::Stereo) => SoundPlaying::NicamStereo,
                    Some(Mode::DualMono) if self.second_lang => SoundPlaying::NicamDual2,
                    Some(Mode::DualMono) => SoundPlaying::NicamDual1,
                    _ => SoundPlaying::NicamMono,
                },
                Source::A2 => match a2.as_ref().and_then(|a| a.mode) {
                    Some(A2Mode::Stereo) => SoundPlaying::A2Stereo,
                    _ if self.second_lang => SoundPlaying::A2DualB,
                    _ => SoundPlaying::A2DualA,
                },
                Source::Carrier => {
                    if !self.carrier_only
                        && a2
                            .as_ref()
                            .is_some_and(|a| a.present && a.mode == Some(A2Mode::Mono))
                    {
                        SoundPlaying::A2Mono
                    } else if self.am {
                        SoundPlaying::Am
                    } else {
                        SoundPlaying::Fm
                    }
                }
            };
        }
        let q = match self.source {
            Source::Carrier => None,
            Source::Nicam => Some(&mut self.nicam),
            Source::A2 => Some(&mut self.a2),
        };
        match q {
            None => {
                io.audio_pass_through(0, 0);
            }
            Some(q) => {
                if !self.playing && q.len() >= START {
                    self.playing = true;
                }
                let carrier = io.audio(0);
                let out = io.audio_out(0);
                for &[l, r] in carrier.stereo() {
                    if self.playing {
                        let [a, b] = q.pop_front().unwrap_or([0.0, 0.0]);
                        out.push_stereo(a, b);
                    } else {
                        out.push_stereo(l, r);
                    }
                }
                if q.is_empty() {
                    self.playing = false;
                }
            }
        }
        // Keep the other queue from growing.
        if self.source != Source::Nicam {
            while self.nicam.len() > START {
                self.nicam.pop_front();
            }
        }
        if self.source != Source::A2 {
            while self.a2.len() > START {
                self.a2.pop_front();
            }
        }
        if io.tick() {
            let t = io.input_format(0).time_of(io.input(0).first());
            io.items_out::<SelectorReport>(1).push(Timed {
                time: t,
                value: SelectorReport { system: self.name },
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SelectorSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SelectorSetting::SoundSystem => {
                self.carrier_only = SOUND_SYSTEM.read(id, v)? == SoundChoice::Carrier;
            }
            SelectorSetting::DualSound => {
                self.second_lang = DUAL_SOUND.read(id, v)? == DualSound::Second;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let sound = if self.carrier_only {
            SoundChoice::Carrier
        } else {
            SoundChoice::Auto
        };
        let dual = if self.second_lang {
            DualSound::Second
        } else {
            DualSound::First
        };
        values_of([
            (SelectorSetting::SoundSystem, Value::Choice(sound.index())),
            (SelectorSetting::DualSound, Value::Choice(dual.index())),
        ])
    }
}
