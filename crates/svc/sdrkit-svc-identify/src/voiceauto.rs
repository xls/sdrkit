//! The automatic digital voice choice: every narrowband signal in the span that the voice signatures name, the strongest sure
//! one played, and a new look every 20 s.

use crate::identify::{Identification, Identifier, Sureness, Verdict};
use crate::signature::Signature;
use crate::table::builtin;
use serde::{Deserialize, Serialize};

/// Seconds between looks.
pub const VOICE_RESCAN_S: f64 = 20.0;
/// Signals wider than this are not narrowband voice (TETRA is 25 kHz).
pub const VOICE_MAX_WIDTH_HZ: f64 = 30_000.0;
/// The key of the signal type of analogue narrow FM voice.
const ANALOGUE_SIGNAL: &str = "narrow_fm_voice";

/// The identifier for voice: the digital voice signatures and analogue narrow FM.
pub fn voice_identifier() -> Identifier {
    let sigs: Vec<Signature> = builtin()
        .into_iter()
        .filter(|s| s.family == "Digital voice" || s.signal == ANALOGUE_SIGNAL)
        .collect();
    Identifier::with_signatures(sigs)
}

/// One system found.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceSystem {
    /// Its name ("DMR", "Analogue FM").
    pub name: String,
    /// Where (Hz).
    pub freq_hz: f64,
    /// Width (Hz).
    pub width_hz: f64,
    /// Its power (dBFS).
    pub level_dbfs: f64,
    /// SNR (dB).
    pub snr_db: f64,
    /// How sure (sure: its frame sync was found).
    pub sureness: Sureness,
    /// Analogue FM rather than a digital system.
    pub analogue: bool,
    /// The key of its signal type.
    pub signal: String,
}

/// The voice systems among identifications of every signal: the narrowband ones that a voice signature names as likely or
/// ambiguous, loudest first.
pub fn voice_systems(ids: &[Identification]) -> Vec<VoiceSystem> {
    let mut out: Vec<VoiceSystem> = ids
        .iter()
        .filter(|r| r.features.width_99_hz <= VOICE_MAX_WIDTH_HZ)
        .filter(|r| matches!(r.verdict, Verdict::Likely | Verdict::Ambiguous))
        .filter_map(|r| {
            let c = r.candidates.first()?;
            let analogue = c.signal == ANALOGUE_SIGNAL;
            Some(VoiceSystem {
                name: if analogue {
                    "Analogue FM".into()
                } else {
                    c.name.clone()
                },
                freq_hz: r.features.rf_hz,
                width_hz: r.features.width_99_hz,
                level_dbfs: r.features.power_dbfs,
                snr_db: r.features.snr_db,
                sureness: r.sureness,
                analogue,
                signal: c.signal.clone(),
            })
        })
        .collect();
    out.sort_by(|a, b| b.level_dbfs.total_cmp(&a.level_dbfs));
    out
}

/// What the engine should do.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum VoiceAction {
    /// Play this system (open its signal type at its frequency).
    Play {
        /// The system.
        system: VoiceSystem,
    },
    /// Keep playing what plays.
    Keep,
    /// Nothing sure to play; show the message and the list.
    Nothing {
        /// The status line.
        message: String,
    },
}

/// The controller.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VoiceAuto {
    last_scan_s: Option<f64>,
    systems: Vec<VoiceSystem>,
    playing: Option<(String, f64)>,
}

impl VoiceAuto {
    /// Whether a look is due at `now_s`.
    pub fn due(&self, now_s: f64) -> bool {
        self.last_scan_s.is_none_or(|t| now_s - t >= VOICE_RESCAN_S)
    }
    /// Every system of the last look.
    pub fn systems(&self) -> &[VoiceSystem] {
        &self.systems
    }
    /// The result of a look at `now_s`.
    pub fn on_scan(&mut self, now_s: f64, found: Vec<VoiceSystem>) -> VoiceAction {
        self.last_scan_s = Some(now_s);
        self.systems = found;
        let best = self
            .systems
            .iter()
            .filter(|s| !s.analogue && s.sureness == Sureness::Sure)
            .max_by(|a, b| a.level_dbfs.total_cmp(&b.level_dbfs))
            .cloned();
        match best {
            Some(s) => {
                let key = (s.signal.clone(), s.freq_hz);
                if self
                    .playing
                    .as_ref()
                    .is_some_and(|(m, f)| *m == key.0 && (f - key.1).abs() < s.width_hz / 2.0)
                {
                    VoiceAction::Keep
                } else {
                    self.playing = Some(key);
                    VoiceAction::Play { system: s }
                }
            }
            None => {
                self.playing = None;
                VoiceAction::Nothing {
                    message: if self.systems.is_empty() {
                        "No voice signal in the span: looking again in 20 s".into()
                    } else {
                        format!(
                            "{} signal(s), none sure to be a digital voice system: looking again in 20 s",
                            self.systems.len()
                        )
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sys(name: &str, level: f64, sure: Sureness, analogue: bool) -> VoiceSystem {
        VoiceSystem {
            name: name.into(),
            freq_hz: 446e6 + level * 1e3,
            width_hz: 9e3,
            level_dbfs: level,
            snr_db: 20.0,
            sureness: sure,
            analogue,
            signal: name.to_lowercase(),
        }
    }

    #[test]
    fn plays_the_strongest_sure_and_rescans_every_20_s() {
        let mut v = VoiceAuto::default();
        assert!(v.due(0.0));
        let a = v.on_scan(
            0.0,
            vec![
                sys("FM", -10.0, Sureness::Likely, true),
                sys("DMR", -30.0, Sureness::Sure, false),
                sys("P25", -20.0, Sureness::Sure, false),
                sys("NXDN", -5.0, Sureness::Likely, false),
            ],
        );
        assert!(matches!(a, VoiceAction::Play { system } if system.name == "P25"));
        assert_eq!(v.systems().len(), 4);
        assert!(!v.due(10.0) && v.due(20.0));
        let again = v.on_scan(20.0, vec![sys("P25", -20.0, Sureness::Sure, false)]);
        assert_eq!(again, VoiceAction::Keep);
        assert!(matches!(
            v.on_scan(40.0, vec![]),
            VoiceAction::Nothing { .. }
        ));
    }
}
