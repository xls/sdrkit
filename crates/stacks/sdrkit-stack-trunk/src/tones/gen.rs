//! The tone test signal: a scene of tone signals made by `sdrkit-proto-tones`, as audio (1.0 is 5 kHz of deviation) or frequency
//! modulated onto a carrier at the centre.

use crate::fmtx::{AudioFm, Looping};
use num_complex::Complex32;
use sdrkit_core::{GenConfig, Generator, SignalInfo};
use sdrkit_proto_tones::ctcss::ctcss_sequence;
use sdrkit_proto_tones::dcs::DcsEncoder;
use sdrkit_proto_tones::dtmf::dtmf_sequence;
use sdrkit_proto_tones::selcal::selcal_sequence;
use sdrkit_proto_tones::selcall::selcall_sequence;
use sdrkit_proto_tones::twotone::two_tone_sequence;
use sdrkit_proto_tones::TonesError;
use sdrkit_util_ids::tones::dcs::Dcs;
use sdrkit_util_ids::tones::selcal::Code;
use sdrkit_util_ids::tones::selcall::{self, ToneSet};

/// The audio rate of the scenes.
pub const AUDIO_RATE: f64 = 8000.0;

/// One part of a scene. Times and lengths are in seconds, amplitudes in units of 5 kHz of deviation.
#[derive(Clone, PartialEq)]
pub enum Part {
    /// A steady tone (a CTCSS tone): frequency, start, length, amplitude.
    Tone(f64, f64, f64, f32),
    /// A DCS code: the code, inverted, start, length, amplitude. The 134 Hz turn-off code follows it.
    Dcs(Dcs, bool, f64, f64, f32),
    /// DTMF keys: the keys, start, tone length, gap, amplitude of each of the two tones.
    Dtmf(String, f64, f64, f64, f32),
    /// A sequential call: the tone set, the digits, start, tone length (`None`: the set's usual length), amplitude.
    Selcall(&'static ToneSet, String, f64, Option<f64>, f32),
    /// A SELCAL call: the code, start, amplitude of each tone.
    Selcal(Code, f64, f32),
    /// A two-tone page: tone A, tone B, start, length of A, length of B, amplitude.
    TwoTone(f64, f64, f64, f64, f64, f32),
}

/// A scene: parts laid on audio of a given length.
#[derive(Clone, PartialEq)]
pub struct Scene {
    /// The length, s.
    pub length_s: f64,
    /// The parts.
    pub parts: Vec<Part>,
}

impl Scene {
    /// An empty scene `length_s` long.
    pub fn new(length_s: f64) -> Scene {
        Scene {
            length_s,
            parts: Vec::new(),
        }
    }
    /// The scene with one more part.
    pub fn with(mut self, p: Part) -> Scene {
        self.parts.push(p);
        self
    }
    /// The audio at `rate`.
    ///
    /// # Errors
    ///
    /// [`TonesError`] for a part that `sdrkit-proto-tones` refuses (a digit outside the set, a tone above half the rate).
    pub fn audio(&self, rate: f64) -> Result<Vec<f32>, TonesError> {
        let n = (self.length_s * rate).round() as usize;
        let mut out = vec![0.0f32; n];
        let mut buf = Vec::new();
        for p in &self.parts {
            let start = match p {
                Part::Tone(_, s, ..)
                | Part::Dcs(_, _, s, ..)
                | Part::Dtmf(_, s, ..)
                | Part::Selcall(_, _, s, ..)
                | Part::Selcal(_, s, _)
                | Part::TwoTone(_, _, s, ..) => *s,
            };
            let at = (start * rate).round() as usize;
            if at >= n {
                continue;
            }
            buf.clear();
            buf.resize(n - at, 0.0);
            match p {
                Part::Tone(hz, _, len, amp) => {
                    ctcss_sequence(rate, *hz, *amp, *len, None)?.fill(&mut buf);
                }
                Part::Dcs(code, inverted, _, len, amp) => {
                    DcsEncoder::new(rate, *code, *inverted, *amp, *len, true)?.fill(&mut buf);
                }
                Part::Dtmf(keys, _, tone_s, gap_s, amp) => {
                    dtmf_sequence(rate, keys, *amp, *amp, *tone_s, *gap_s)?.fill(&mut buf);
                }
                Part::Selcall(set, digits, _, tone_s, amp) => {
                    selcall_sequence(rate, set, digits, *amp, *tone_s)?.fill(&mut buf);
                }
                Part::Selcal(code, _, amp) => {
                    selcal_sequence(rate, *code, *amp, 1.0, 0.2, 0.0)?.fill(&mut buf);
                }
                Part::TwoTone(a, b, _, a_s, b_s, amp) => {
                    two_tone_sequence(rate, *a, *b, *a_s, *b_s, *amp)?.fill(&mut buf);
                }
            }
            for (o, v) in out[at..].iter_mut().zip(&buf) {
                *o += v;
            }
        }
        Ok(out)
    }
}

/// The demonstration scene (35 s): CTCSS 123.0 Hz, DCS 023N, DTMF 159#, ZVEI1 12345, SELCAL AM-CP and a two-tone page.
pub fn demo() -> Scene {
    let d023 = Dcs::from_octal("023").expect("023 is a DCS code");
    Scene::new(35.0)
        .with(Part::Tone(123.0, 0.0, 10.0, 0.12))
        .with(Part::Dcs(d023, false, 10.2, 7.6, 0.15))
        .with(Part::Dtmf("159#".into(), 18.5, 0.12, 0.1, 0.25))
        .with(Part::Selcall(
            selcall::by_name("ZVEI1").expect("ZVEI1 is a tone set"),
            "12345".into(),
            22.0,
            None,
            0.5,
        ))
        .with(Part::Selcal(
            Code::parse("AM-CP").expect("AM-CP is a SELCAL code"),
            24.5,
            0.25,
        ))
        .with(Part::TwoTone(682.5, 1000.0, 29.5, 1.0, 3.0, 0.4))
}

/// The stack's generator: a scene's 8 kHz audio, looping, frequency modulated at the centre with 5 kHz for 1.0 (or, for audio
/// input, the audio itself as the real part).
pub struct TonesGenerator {
    rate: f64,
    audio: Looping,
    tx: Option<AudioFm>,
    as_audio: bool,
    piece: Vec<f32>,
}

impl TonesGenerator {
    /// From the stack's generator config: the demonstration, as FM, or as audio when `audio_input` is set.
    pub fn from_config(cfg: &GenConfig) -> TonesGenerator {
        let audio = demo()
            .audio(AUDIO_RATE)
            .expect("the demonstration scene is valid");
        if cfg.settings.get(&super::front::AUDIO_INPUT) {
            TonesGenerator::audio(audio, cfg.rate_hz)
        } else {
            TonesGenerator::looping(audio, cfg.rate_hz)
        }
    }
    /// Loops this 8 kHz audio as FM at `rate`. A rate too low for the deviation produces silence.
    pub fn looping(audio: Vec<f32>, rate: f64) -> TonesGenerator {
        TonesGenerator {
            rate,
            audio: Looping::new(audio),
            tx: AudioFm::new(AUDIO_RATE, rate, 5000.0, 0.0).ok(),
            as_audio: false,
            piece: Vec::new(),
        }
    }
    /// Loops this 8 kHz audio, resampled to `rate`, as the real part of the samples.
    pub fn audio(audio: Vec<f32>, rate: f64) -> TonesGenerator {
        TonesGenerator {
            rate,
            audio: Looping::new(audio),
            tx: AudioFm::new(AUDIO_RATE, rate, 1.0, 0.0).ok(),
            as_audio: true,
            piece: Vec::new(),
        }
    }
}

impl Generator for TonesGenerator {
    /// The station is made at 0 Hz. The framework places it on the tuned frequency.
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let audio = &mut self.audio;
        match self.tx.as_mut() {
            Some(tx) if self.as_audio => {
                self.piece.resize(out.len(), 0.0);
                tx.fill_audio(&mut self.piece, |a| audio.fill(a));
                for (z, &v) in out.iter_mut().zip(&self.piece) {
                    *z = Complex32::new(v, 0.0);
                }
            }
            Some(tx) => tx.fill(out, |a| audio.fill(a)),
            None => out.iter_mut().for_each(|z| *z = Complex32::new(0.0, 0.0)),
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // Carson's rule: 2 × (5 kHz deviation + 3.4 kHz audio).
        Some(SignalInfo::new(if self.as_audio { 3.4e3 } else { 16.8e3 }))
    }
}
