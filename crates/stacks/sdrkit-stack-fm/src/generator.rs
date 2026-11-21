//! The FM test signal: a broadcast station with a stereo multiplex, a pilot, and RDS carrying PI, PS, RT, PTY, TP, TA and M/S:
//! the multiplex of [`crate::mpx`] with two tones or pushed audio. It is the clean signal at unit power: the framework adds
//! noise, offsets and the other impairments from the `gen_*` settings.
//!
//! Standard: ITU-R BS.450 (multiplex levels), IEC 62106 (RDS).

use crate::mpx::{Multiplex, MpxParams, Programme};
use crate::rds::encode::RdsContent;
use crate::settings::*;
use sdrkit_algo_dsp::sample::Stereo;
use sdrkit_core::{
    Complex32, GenConfig, Generator, Hz, SettingError, SettingSet, SignalInfo, Values,
};

pub use crate::mpx::{SCA_DEVIATION_HZ, SCA_HZ, SCA_INJECTION};

/// The deviation of a full-scale multiplex, Hz.
pub const FULL_DEVIATION_HZ: f64 = 75e3;
/// The bandwidth the signal's power is stated in (the carrier-to-noise of the tests is measured in 200 kHz), Hz.
pub const SIGNAL_BANDWIDTH_HZ: f64 = 200e3;
/// The level of the audio part of the test signal when a channel is at full scale: 0.45 (L+R) + 0.45 (L-R) sin 2θ.
pub const AUDIO_SHARE: f64 = 0.9;

/// Everything the generator sends.
#[derive(Clone, Debug, PartialEq)]
pub struct GenParams {
    /// Output rate.
    pub rate_hz: f64,
    /// The offset of the station from the centre, Hz (a scene of several stations). The framework's `gen_offset_hz` moves the
    /// whole signal.
    pub offset_hz: f64,
    /// Stereo (with a pilot); mono sends no pilot.
    pub stereo: bool,
    /// Left tone frequency.
    pub left_hz: f64,
    /// Left tone amplitude (0 = silent).
    pub left_amp: f64,
    /// Right tone frequency.
    pub right_hz: f64,
    /// Right tone amplitude (0 = silent).
    pub right_amp: f64,
    /// Pilot injection in percent of full deviation.
    pub pilot_pct: f64,
    /// Send RDS.
    pub rds: bool,
    /// RDS injection in kHz of deviation.
    pub rds_khz: f64,
    /// Programme service name (padded or cut to 8 characters).
    pub ps: String,
    /// RadioText (cut to 64 characters).
    pub rt: String,
    /// The RadioText A/B flag.
    pub rt_ab: bool,
    /// Programme identification.
    pub pi: u16,
    /// Programme type code.
    pub pty: u8,
    /// Traffic programme.
    pub tp: bool,
    /// Traffic announcement.
    pub ta: bool,
    /// Music (true) or speech.
    pub music: bool,
    /// Pre-emphasis time constant in seconds (0 = none).
    pub preemphasis_s: f64,
    /// The carrier's amplitude (1 is unit power).
    pub gain: f64,
    /// The alternative frequencies as AF codes (87.5 MHz + 0.1 MHz times the code, 1 to 204), at most 25. The 0A
    /// groups send the count and the codes in pairs (method A), again and again.
    pub af_codes: Vec<u8>,
    /// Extra groups as blocks B, C, D (B carries its own type, TP and PTY), sent as every group k with k mod 6 = 5.
    pub extra_groups: Vec<[u16; 3]>,
    /// Use audio pushed with [`FmGenerator::push_audio`] (48 kHz stereo) instead of the tones.
    pub live_audio: bool,
    /// The deviation of a full-scale multiplex, Hz: the multiplex levels are shares of it.
    pub deviation_hz: f64,
    /// Send the two SCA subcarriers, each at [`SCA_INJECTION`] of full deviation.
    pub sca: bool,
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            rate_hz: 2e6,
            offset_hz: 0.0,
            stereo: true,
            left_hz: 1000.0,
            left_amp: 0.5,
            right_hz: 3000.0,
            right_amp: 0.5,
            pilot_pct: 9.0,
            rds: true,
            rds_khz: 2.0,
            ps: "SDRKIT  ".into(),
            rt: "sdrkit FM test signal".into(),
            rt_ab: false,
            pi: 0x4A21,
            pty: 10,
            tp: true,
            ta: false,
            music: true,
            preemphasis_s: 50e-6,
            gain: 1.0,
            // 98.1 MHz and 103.5 MHz.
            af_codes: vec![106, 160],
            extra_groups: Vec::new(),
            live_audio: false,
            deviation_hz: FULL_DEVIATION_HZ,
            sca: false,
        }
    }
}

/// Parses extra groups written as hexadecimal B, C and D words, groups separated by `;` or new lines.
///
/// # Errors
///
/// [`SettingError::Malformed`] of `gen_extra_groups`, at the byte where the first group that is not three hexadecimal words
/// starts.
pub fn parse_extra_groups(text: &str) -> Result<Vec<[u16; 3]>, SettingError> {
    let mut out = Vec::new();
    let mut at = 0;
    for raw in text.split([';', '\n']) {
        let start = at + (raw.len() - raw.trim_start().len());
        at += raw.len() + 1;
        let part = raw.trim();
        if part.is_empty() {
            continue;
        }
        let mut words = part.split_whitespace().map(|h| u16::from_str_radix(h, 16));
        match (words.next(), words.next(), words.next(), words.next()) {
            (Some(Ok(b)), Some(Ok(c)), Some(Ok(d)), None) => out.push([b, c, d]),
            _ => {
                return Err(SettingError::Malformed {
                    id: FmSetting::GenExtraGroups.id(),
                    at: Some(start),
                })
            }
        }
    }
    Ok(out)
}

impl GenParams {
    /// Parameters from the stack's settings (the `gen_*` settings). A PI or an extra group text that does not parse keeps
    /// the default of [`GenParams`].
    pub fn from_values(rate_hz: f64, v: &Values) -> Self {
        let d = GenParams::default();
        GenParams {
            rate_hz,
            stereo: v.get(&GEN_STEREO),
            left_hz: v.get(&GEN_LEFT_HZ),
            left_amp: v.get(&GEN_LEFT_AMP),
            right_hz: v.get(&GEN_RIGHT_HZ),
            right_amp: v.get(&GEN_RIGHT_AMP),
            pilot_pct: v.get(&GEN_PILOT),
            rds: v.get(&GEN_RDS),
            rds_khz: v.get(&GEN_RDS_KHZ),
            ps: v.get(&GEN_PS).to_owned(),
            rt: v.get(&GEN_RT).to_owned(),
            pi: u16::from_str_radix(v.get(&GEN_PI).trim(), 16).unwrap_or(d.pi),
            pty: v.get(&GEN_PTY) as u8,
            tp: v.get(&GEN_TP),
            ta: v.get(&GEN_TA),
            music: v.get(&GEN_MUSIC),
            preemphasis_s: v.get(&GEN_PREEMPHASIS).tau_s(),
            extra_groups: parse_extra_groups(v.get(&GEN_EXTRA_GROUPS)).unwrap_or_default(),
            ..d
        }
    }

    /// The built-in demo station at `offset_hz` from the centre, with output gain 0.3.
    pub fn demo(rate_hz: f64, offset_hz: f64) -> Self {
        GenParams {
            rate_hz,
            rt: "sdrkit FM test signal: 1 kHz left, 3 kHz right".into(),
            gain: 0.3,
            offset_hz,
            ..Default::default()
        }
    }
}

impl GenParams {
    /// The RDS content of the parameters.
    pub fn rds_content(&self) -> RdsContent {
        RdsContent {
            pi: self.pi,
            ps: self.ps.clone(),
            rt: self.rt.clone(),
            rt_ab: self.rt_ab,
            pty: self.pty,
            tp: self.tp,
            ta: self.ta,
            music: self.music,
            stereo: self.stereo,
            af_codes: self.af_codes.clone(),
            extra_groups: self.extra_groups.clone(),
            clock_offset: None,
        }
    }

    /// The levels of the multiplex.
    pub fn mpx_params(&self) -> MpxParams {
        MpxParams {
            rate_hz: self.rate_hz,
            deviation_hz: self.deviation_hz,
            stereo: self.stereo,
            pilot: self.pilot_pct / 100.0,
            rds: if self.rds {
                self.rds_khz * 1e3 / self.deviation_hz
            } else {
                0.0
            },
            audio: AUDIO_SHARE,
            offset: self.offset_hz / self.deviation_hz,
            sca: self.sca,
        }
    }

    /// The audio: pushed audio without the limiter (a clip at full scale), or the two tones.
    pub fn programme(&self) -> Programme {
        if self.live_audio {
            Programme::Live {
                preemphasis_s: self.preemphasis_s,
                limiter: false,
            }
        } else {
            Programme::Tones {
                left: (self.left_hz, self.left_amp),
                right: (self.right_hz, self.right_amp),
                preemphasis_s: self.preemphasis_s,
            }
        }
    }
}

/// The FM test-signal generator.
#[derive(Clone, Debug)]
pub struct FmGenerator {
    p: GenParams,
    mpx: Multiplex,
    frames: Vec<Stereo>,
}

impl FmGenerator {
    /// A generator for these parameters.
    ///
    /// # Panics
    ///
    /// If the rate is not above twice the deviation, or the pre-emphasis time constant is negative.
    pub fn new(p: GenParams) -> Self {
        let mpx = Multiplex::new(p.mpx_params(), &p.programme(), p.rds_content())
            .unwrap_or_else(|e| panic!("the FM generator needs a rate above twice the deviation: {e}"));
        FmGenerator {
            p,
            mpx,
            frames: Vec::new(),
        }
    }

    /// A generator from the stack's settings.
    pub fn from_config(cfg: &GenConfig) -> Self {
        Self::new(GenParams::from_values(cfg.rate_hz, &cfg.settings))
    }

    /// The parameters.
    pub fn params(&self) -> &GenParams {
        &self.p
    }

    /// Queues live audio (interleaved stereo at 48 kHz) with `live_audio` set. When the queue runs dry the audio is silent.
    pub fn push_audio(&mut self, interleaved: &[f32]) {
        self.frames.clear();
        self.frames.extend(
            interleaved
                .as_chunks::<2>()
                .0
                .iter()
                .map(|f| Stereo::new(f[0], f[1])),
        );
        self.mpx.push_audio(&self.frames);
    }
}

impl Generator for FmGenerator {
    fn rate_hz(&self) -> Hz {
        self.p.rate_hz
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.mpx.generate(out);
        let g = self.p.gain as f32;
        if g != 1.0 {
            out.iter_mut().for_each(|v| *v *= g);
        }
        out.len()
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(SIGNAL_BANDWIDTH_HZ).with_power(self.p.gain * self.p.gain))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_carrier_has_unit_power_and_states_it() {
        let mut g = FmGenerator::new(GenParams::default());
        let mut x = vec![Complex32::default(); 10_000];
        g.generate(&mut x);
        assert!(x.iter().all(|v| (v.norm() - 1.0).abs() < 1e-5));
        let info = g.signal_info().unwrap();
        assert_eq!((info.bandwidth_hz, info.power), (200e3, 1.0));
        let mut g = FmGenerator::new(GenParams::demo(2e6, 0.0));
        g.generate(&mut x);
        assert!(x.iter().all(|v| (v.norm() - 0.3).abs() < 1e-5));
        assert!((g.signal_info().unwrap().power - 0.09).abs() < 1e-12);
    }

    #[test]
    fn the_schedule_and_first_group() {
        use crate::rds::encode::RdsEncoder;
        let mut e = RdsEncoder::new(GenParams::default().rds_content());
        let groups: Vec<[u16; 4]> = (0..10).map(|_| e.next_group(None)).collect();
        // The count of two alternative frequencies and the first, then the second and the filler.
        assert_eq!(groups[0], [0x4A21, 0x0548, 0xE26A, 0x5344]);
        assert_eq!(groups[1][2], 0xA0CD);
        assert_eq!(groups[2][2], 0xE26A);
        assert_eq!(groups[3][1] & 0x1F, 0x08 | 0x04 | 3);
        assert_eq!(groups[4][1] >> 11, 2 << 1);
        assert_eq!(groups[4][2], u16::from_be_bytes(*b"sd"));
        assert_eq!(groups[9][2], u16::from_be_bytes(*b"it"));
        let mut e = RdsEncoder::new(
            GenParams {
                extra_groups: vec![[0x1000, 0x00E2, 0xB945]],
                ..Default::default()
            }
            .rds_content(),
        );
        let groups: Vec<[u16; 4]> = (0..12).map(|_| e.next_group(None)).collect();
        assert_eq!(groups[5][1], 0x1000);
        assert_eq!(groups[11][1], 0x1000);
        assert_eq!(
            parse_extra_groups("1000 00E2 B945; A400 524F 434B")
                .unwrap()
                .len(),
            2
        );
        assert!(parse_extra_groups("1000 00E2").is_err());
    }
}
