//! Named channel profiles, transcribed from the standards that define them.
//!
//! | Profile | Source | Taps | Doppler |
//! |---|---|---|---|
//! | [`Profile::Drm`] 1 to 6 | ETSI ES 201 980 V4.2.1 Annex B.1, table B.1 (robustness modes A to D, LF/MF/HF) | 1 to 4 | Gaussian, with shifts |
//! | [`Profile::DrmVhf`] 7 to 12 | ES 201 980 V4.2.1 Annex B.2, table B.2 (robustness mode E, VHF) | 1 to 12 | classical, Gauss1/Gauss2 |
//! | [`Profile::Watterson`] good, moderate, poor, flutter | Recommendation ITU-R F.520-2 (1992, withdrawn 2000) Annex 1 section 2.2 | 2 equal | Gaussian |
//! | [`Profile::F1487`] low, mid, high latitudes | Recommendation ITU-R F.1487 (05/2000) Annex 3 sections 2 to 4 | 2 equal | Gaussian |
//! | [`Profile::Gsm`] RA6, TU6, HT6, TU12, HT12, EQ6 | 3GPP TS 45.005 V17.0.0 Annex C.3 (the COST 207 models), tap setting (1) | 6 or 12 | CLASS, RICE |
//! | [`Profile::Rayleigh`], [`Profile::Rician`] | one tap | 1 | classical, Rice |
//! | [`Profile::Echo`] | a direct path and one steady echo | 2 | none |
//! | [`Profile::echoes`] | a direct path and up to [`EchoPath::MAX`] steady echoes, pre-echoes included | 1 to 17 | none |
//!
//! Conventions taken from the standards:
//!
//! - ES 201 980 states each path's gain as an rms amplitude ("Path gain, rms"), so its power is the square; the Doppler spread is
//!   two-sided and `2 sigma` (equation B.4).
//! - F.520-2 and F.1487 use two independently fading paths of equal mean power, no frequency shift, and the "differential time delay"
//!   between them; F.1487 Annex 3 says the frequency spread is the `2 sigma` value.
//! - TS 45.005 states powers in dB and two Doppler types: CLASS (classical) and RICE, a classical part of power `A0 = 0.17` plus a
//!   direct path of power `A1 = 0.83` at `0.7 f_d` for the six-tap RA model (clause C.2), so its Rice factor is `0.83 / 0.17`.
//!   Profiles that move (`GSM`, DRM channels 8 to 12) take the maximum Doppler `f_d = v / lambda` as a parameter, since it depends
//!   on the speed and the carrier frequency.
//! - Gauss1 and Gauss2 (ES 201 980 equations B.9 and B.10) are Gaussians at `+0.7 f_d` and `-0.7 f_d` with standard deviation
//!   `0.1 f_d`, so a spread of `0.2 f_d`.
//!
//! The delay line scales the tap powers to sum to 1 (the standards give relative values).

use super::{Doppler, Tap};
use crate::{EchoFault, Param, SimError};
use sdrkit_core::Choice;

/// The latitude bands of ITU-R F.1487.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Latitude {
    /// Low latitudes (Annex 3 section 2).
    Low,
    /// Mid-latitudes (section 3).
    Mid,
    /// High latitudes (section 4).
    High,
}

/// The conditions of ITU-R F.1487.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum F1487 {
    /// Quiet conditions.
    Quiet,
    /// Moderate conditions.
    Moderate,
    /// Disturbed conditions.
    Disturbed,
    /// Disturbed near vertical incidence (mid-latitudes only, section 3.4).
    DisturbedNvis,
}

/// The conditions of ITU-R F.520-2 Annex 1 section 2.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Watterson {
    /// Good: 0.5 ms, 0.1 Hz.
    Good,
    /// Moderate: 1 ms, 0.5 Hz.
    Moderate,
    /// Poor: 2 ms, 1 Hz.
    Poor,
    /// Flutter fading: 0.5 ms, 10 Hz.
    Flutter,
}

/// The GSM (COST 207) models of TS 45.005 Annex C.3, tap setting (1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gsm {
    /// Rural area, 6 taps (C.3.1).
    Ra6,
    /// Hilly terrain, 12 taps (C.3.2).
    Ht12,
    /// Hilly terrain, reduced 6-tap setting (C.3.2).
    Ht6,
    /// Typical urban, 12 taps (C.3.3).
    Tu12,
    /// Typical urban, reduced 6-tap setting (C.3.3).
    Tu6,
    /// Equalisation test, 6 equal taps 3.2 us apart (C.3.4).
    Eq6,
}

/// A channel profile.
#[derive(Clone, Debug, PartialEq)]
pub enum Profile {
    /// No channel: one steady tap.
    None,
    /// DRM channels 1 to 6 (ES 201 980 table B.1).
    Drm(u8),
    /// DRM channels 7 to 12 (ES 201 980 table B.2) with the maximum Doppler of the chosen speed.
    DrmVhf {
        /// 7 to 12.
        channel: u8,
        /// `f_d = v / lambda`, Hz.
        max_doppler_hz: f64,
    },
    /// The HF conditions of ITU-R F.520-2.
    Watterson(Watterson),
    /// The HF conditions of ITU-R F.1487.
    F1487 {
        /// The latitude band.
        latitude: Latitude,
        /// The conditions (near vertical incidence only at mid-latitudes).
        condition: F1487,
    },
    /// A GSM (COST 207) model.
    Gsm {
        /// Which.
        model: Gsm,
        /// `f_d = v / lambda`, Hz.
        max_doppler_hz: f64,
    },
    /// Flat Rayleigh fading with a classical spectrum.
    Rayleigh {
        /// The maximum Doppler, Hz.
        max_doppler_hz: f64,
    },
    /// Flat Rice fading: a classical part and a direct path.
    Rician {
        /// The Rice factor (direct over scattered power), dB.
        k_db: f64,
        /// The maximum Doppler of the scattered part, Hz.
        max_doppler_hz: f64,
        /// The frequency of the direct path, Hz.
        los_hz: f64,
    },
    /// A direct path and one steady echo `delay_s` later, `level_db` relative to it, its carrier turned by `phase_deg`.
    Echo {
        /// The echo's delay, seconds.
        delay_s: f64,
        /// The echo's power relative to the direct path, dB.
        level_db: f64,
        /// The echo's carrier phase, degrees.
        phase_deg: f64,
    },
    /// Any taps.
    Taps(Vec<Tap>),
}

/// `(delay ms, rms amplitude, Doppler shift Hz, Doppler spread Hz)` per path of DRM channels 1 to 6, ES 201 980 V4.2.1 table B.1.
/// A spread of 0 is a steady path.
pub const DRM_HF: [&[(f64, f64, f64, f64)]; 6] = [
    // Channel 1: AWGN.
    &[(0.0, 1.0, 0.0, 0.0)],
    // Channel 2: Rice with delay.
    &[(0.0, 1.0, 0.0, 0.0), (1.0, 0.5, 0.0, 0.1)],
    // Channel 3: US Consortium.
    &[
        (0.0, 1.0, 0.1, 0.1),
        (0.7, 0.7, 0.2, 0.5),
        (1.5, 0.5, 0.5, 1.0),
        (2.2, 0.25, 1.0, 2.0),
    ],
    // Channel 4: CCIR Poor.
    &[(0.0, 1.0, 0.0, 1.0), (2.0, 1.0, 0.0, 1.0)],
    // Channel 5.
    &[(0.0, 1.0, 0.0, 2.0), (4.0, 1.0, 0.0, 2.0)],
    // Channel 6.
    &[
        (0.0, 0.5, 0.0, 0.1),
        (2.0, 1.0, 1.2, 2.4),
        (4.0, 0.25, 2.4, 4.8),
        (6.0, 0.0625, 3.6, 7.2),
    ],
];

/// The path types of ES 201 980 table B.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrmPath {
    /// Steady (channel 7: Rice with A = 0, B = 1, f_D = 0).
    Steady,
    /// Classical (equation B.7).
    Classical,
    /// Gauss1 (equation B.10, `+0.7 f_d`).
    Gauss1,
    /// Gauss2 (equation B.10, `-0.7 f_d`).
    Gauss2,
}

/// `(delay us, relative power dB, path type)` per path of DRM channels 7 to 12, ES 201 980 V4.2.1 table B.2.
pub const DRM_VHF: [&[(f64, f64, DrmPath)]; 6] = {
    use DrmPath::*;
    [
        // Channel 7: AWGN.
        &[(0.0, 0.0, Steady)],
        // Channel 8: Urban (2 and 60 km/h).
        &[
            (0.0, -2.0, Classical),
            (0.2, 0.0, Classical),
            (0.5, -3.0, Classical),
            (0.9, -4.0, Classical),
            (1.2, -2.0, Classical),
            (1.4, 0.0, Classical),
            (2.0, -3.0, Classical),
            (2.4, -5.0, Classical),
            (3.0, -10.0, Classical),
        ],
        // Channel 9: Rural (150 km/h).
        &[
            (0.0, -4.0, Classical),
            (0.3, -8.0, Classical),
            (0.5, 0.0, Classical),
            (0.9, -5.0, Classical),
            (1.2, -16.0, Classical),
            (1.9, -18.0, Classical),
            (2.1, -14.0, Classical),
            (2.5, -20.0, Classical),
            (3.0, -25.0, Classical),
        ],
        // Channel 10: Terrain obstructed (60 km/h).
        &[
            (0.0, -8.0, Classical),
            (1.0, -2.0, Classical),
            (2.5, 0.0, Classical),
            (3.5, -1.0, Classical),
            (5.0, -2.0, Classical),
            (8.0, -3.0, Classical),
            (12.0, 0.0, Classical),
            (14.0, -6.0, Classical),
            (16.0, -3.0, Classical),
        ],
        // Channel 11: Hilly terrain (100 km/h).
        &[
            (0.0, 0.0, Classical),
            (0.5, -5.7, Classical),
            (1.3, -12.7, Classical),
            (1.9, -20.6, Classical),
            (30.0, -3.1, Gauss1),
            (31.3, -5.4, Gauss1),
            (34.9, -11.6, Gauss1),
            (37.2, -15.9, Gauss1),
            (39.1, -18.9, Gauss1),
            (40.0, -25.7, Gauss1),
            (80.0, -4.5, Gauss2),
            (82.7, -11.5, Gauss2),
        ],
        // Channel 12: SFN (150 km/h).
        &[
            (0.0, 0.0, Classical),
            (100.0, -13.0, Gauss1),
            (220.0, -18.0, Gauss2),
            (290.0, -22.0, Gauss1),
            (385.0, -26.0, Gauss2),
            (480.0, -31.0, Gauss1),
            (600.0, -32.0, Gauss2),
        ],
    ]
};

/// `(delay us, power dB, rice)` per tap of the GSM models, TS 45.005 V17.0.0 Annex C.3, setting (1). `rice` marks the RICE tap.
pub fn gsm_taps(model: Gsm) -> &'static [(f64, f64, bool)] {
    match model {
        Gsm::Ra6 => &[
            (0.0, 0.0, true),
            (0.1, -4.0, false),
            (0.2, -8.0, false),
            (0.3, -12.0, false),
            (0.4, -16.0, false),
            (0.5, -20.0, false),
        ],
        Gsm::Ht12 => &[
            (0.0, -10.0, false),
            (0.1, -8.0, false),
            (0.3, -6.0, false),
            (0.5, -4.0, false),
            (0.7, 0.0, false),
            (1.0, 0.0, false),
            (1.3, -4.0, false),
            (15.0, -8.0, false),
            (15.2, -9.0, false),
            (15.7, -10.0, false),
            (17.2, -12.0, false),
            (20.0, -14.0, false),
        ],
        Gsm::Ht6 => &[
            (0.0, 0.0, false),
            (0.1, -1.5, false),
            (0.3, -4.5, false),
            (0.5, -7.5, false),
            (15.0, -8.0, false),
            (17.2, -17.7, false),
        ],
        Gsm::Tu12 => &[
            (0.0, -4.0, false),
            (0.1, -3.0, false),
            (0.3, 0.0, false),
            (0.5, -2.6, false),
            (0.8, -3.0, false),
            (1.1, -5.0, false),
            (1.3, -7.0, false),
            (1.7, -5.0, false),
            (2.3, -6.5, false),
            (3.1, -8.6, false),
            (3.2, -11.0, false),
            (5.0, -10.0, false),
        ],
        Gsm::Tu6 => &[
            (0.0, -3.0, false),
            (0.2, 0.0, false),
            (0.5, -2.0, false),
            (1.6, -6.0, false),
            (2.3, -8.0, false),
            (5.0, -10.0, false),
        ],
        Gsm::Eq6 => &[
            (0.0, 0.0, false),
            (3.2, 0.0, false),
            (6.4, 0.0, false),
            (9.6, 0.0, false),
            (12.8, 0.0, false),
            (16.0, 0.0, false),
        ],
    }
}

/// The RICE spectrum of TS 45.005 clause C.2 for the six-tap RA model: scattered power `A0`, direct power `A1` at `0.7 f_d`.
pub const GSM_RICE_A0: f64 = 0.17;
/// See [`GSM_RICE_A0`].
pub const GSM_RICE_A1: f64 = 0.83;

/// `(differential delay ms, frequency spread Hz)` of ITU-R F.520-2 Annex 1 section 2.2.
pub fn watterson(c: Watterson) -> (f64, f64) {
    match c {
        Watterson::Good => (0.5, 0.1),
        Watterson::Moderate => (1.0, 0.5),
        Watterson::Poor => (2.0, 1.0),
        Watterson::Flutter => (0.5, 10.0),
    }
}

/// `(differential delay ms, frequency spread Hz)` of ITU-R F.1487 Annex 3; `None` for near vertical incidence outside the
/// mid-latitudes.
pub fn f1487(latitude: Latitude, condition: F1487) -> Option<(f64, f64)> {
    use Latitude::*;
    use F1487::*;
    Some(match (latitude, condition) {
        (Low, Quiet) => (0.5, 0.5),
        (Low, Moderate) => (2.0, 1.5),
        (Low, Disturbed) => (6.0, 10.0),
        (Mid, Quiet) => (0.5, 0.1),
        (Mid, Moderate) => (1.0, 0.5),
        (Mid, Disturbed) => (2.0, 1.0),
        (Mid, DisturbedNvis) => (7.0, 1.0),
        (High, Quiet) => (1.0, 0.5),
        (High, Moderate) => (3.0, 10.0),
        (High, Disturbed) => (7.0, 30.0),
        (_, DisturbedNvis) => return None,
    })
}

fn two_paths(delay_ms: f64, spread_hz: f64) -> Vec<Tap> {
    let d = Doppler::Gaussian {
        shift_hz: 0.0,
        spread_hz,
    };
    vec![Tap::new(0.0, 0.0, d), Tap::new(delay_ms * 1e-3, 0.0, d)]
}

sdrkit_core::choice_enum! {
    /// A channel profile that [`Profile::from_name`] builds, in the order a settings list shows them.
    pub enum ProfileName {
        /// None.
        None = ("none", "None"),
        /// Rayleigh (flat).
        Rayleigh = ("rayleigh", "Rayleigh (flat)"),
        /// Rician (flat).
        Rician = ("rician", "Rician (flat)"),
        /// Steady echo.
        Echo = ("echo", "Steady echo"),
        /// Steady echoes (a list).
        Echoes = ("echoes", "Steady echoes (a list)"),
        /// DRM channel 1 (AWGN).
        Drm1 = ("drm1", "DRM channel 1 (AWGN)"),
        /// DRM channel 2 (Rice with delay).
        Drm2 = ("drm2", "DRM channel 2 (Rice with delay)"),
        /// DRM channel 3 (US Consortium).
        Drm3 = ("drm3", "DRM channel 3 (US Consortium)"),
        /// DRM channel 4 (CCIR poor).
        Drm4 = ("drm4", "DRM channel 4 (CCIR poor)"),
        /// DRM channel 5.
        Drm5 = ("drm5", "DRM channel 5"),
        /// DRM channel 6.
        Drm6 = ("drm6", "DRM channel 6"),
        /// DRM channel 7 (VHF AWGN).
        Drm7 = ("drm7", "DRM channel 7 (VHF AWGN)"),
        /// DRM channel 8 (VHF urban).
        Drm8 = ("drm8", "DRM channel 8 (VHF urban)"),
        /// DRM channel 9 (VHF rural).
        Drm9 = ("drm9", "DRM channel 9 (VHF rural)"),
        /// DRM channel 10 (VHF terrain obstructed).
        Drm10 = ("drm10", "DRM channel 10 (VHF terrain obstructed)"),
        /// DRM channel 11 (VHF hilly terrain).
        Drm11 = ("drm11", "DRM channel 11 (VHF hilly terrain)"),
        /// DRM channel 12 (VHF SFN).
        Drm12 = ("drm12", "DRM channel 12 (VHF SFN)"),
        /// HF good (F.520).
        HfGood = ("hf_good", "HF good (F.520)"),
        /// HF moderate (F.520).
        HfModerate = ("hf_moderate", "HF moderate (F.520)"),
        /// HF poor (F.520).
        HfPoor = ("hf_poor", "HF poor (F.520)"),
        /// HF flutter (F.520).
        HfFlutter = ("hf_flutter", "HF flutter (F.520)"),
        /// HF low latitude, quiet (F.1487).
        F1487LowQuiet = ("f1487_low_quiet", "HF low latitude, quiet (F.1487)"),
        /// HF low latitude, moderate (F.1487).
        F1487LowModerate = ("f1487_low_moderate", "HF low latitude, moderate (F.1487)"),
        /// HF low latitude, disturbed (F.1487).
        F1487LowDisturbed = ("f1487_low_disturbed", "HF low latitude, disturbed (F.1487)"),
        /// HF mid-latitude, quiet (F.1487).
        F1487MidQuiet = ("f1487_mid_quiet", "HF mid-latitude, quiet (F.1487)"),
        /// HF mid-latitude, moderate (F.1487).
        F1487MidModerate = ("f1487_mid_moderate", "HF mid-latitude, moderate (F.1487)"),
        /// HF mid-latitude, disturbed (F.1487).
        F1487MidDisturbed = ("f1487_mid_disturbed", "HF mid-latitude, disturbed (F.1487)"),
        /// HF mid-latitude, disturbed NVIS (F.1487).
        F1487MidNvis = ("f1487_mid_nvis", "HF mid-latitude, disturbed NVIS (F.1487)"),
        /// HF high latitude, quiet (F.1487).
        F1487HighQuiet = ("f1487_high_quiet", "HF high latitude, quiet (F.1487)"),
        /// HF high latitude, moderate (F.1487).
        F1487HighModerate = ("f1487_high_moderate", "HF high latitude, moderate (F.1487)"),
        /// HF high latitude, disturbed (F.1487).
        F1487HighDisturbed = ("f1487_high_disturbed", "HF high latitude, disturbed (F.1487)"),
        /// Rural area, 6 taps (COST 207).
        GsmRa6 = ("gsm_ra6", "Rural area, 6 taps (COST 207)"),
        /// Typical urban, 6 taps (COST 207).
        GsmTu6 = ("gsm_tu6", "Typical urban, 6 taps (COST 207)"),
        /// Typical urban, 12 taps (COST 207).
        GsmTu12 = ("gsm_tu12", "Typical urban, 12 taps (COST 207)"),
        /// Hilly terrain, 6 taps (COST 207).
        GsmHt6 = ("gsm_ht6", "Hilly terrain, 6 taps (COST 207)"),
        /// Hilly terrain, 12 taps (COST 207).
        GsmHt12 = ("gsm_ht12", "Hilly terrain, 12 taps (COST 207)"),
        /// Equaliser test, 6 taps (GSM).
        GsmEq6 = ("gsm_eq6", "Equaliser test, 6 taps (GSM)"),
    }
}

impl Profile {
    /// The taps of this profile (relative powers; [`super::Tdl`] scales them to sum to 1). Errors for a channel number outside the
    /// standard's, or a Doppler, delay or Rice factor that is not usable.
    pub fn taps(&self) -> Result<Vec<Tap>, SimError> {
        Ok(match self {
            Profile::None => vec![Tap::new(0.0, 0.0, Doppler::Static)],
            Profile::Drm(ch) => {
                if !(1..=6).contains(ch) {
                    return Param::DrmChannel.range(1.0, 6.0, *ch as f64);
                }
                DRM_HF[*ch as usize - 1]
                    .iter()
                    .map(|&(ms, rho, shift, spread)| {
                        let doppler = if spread == 0.0 {
                            Doppler::Steady {
                                shift_hz: shift,
                                phase_rad: 0.0,
                            }
                        } else {
                            Doppler::Gaussian {
                                shift_hz: shift,
                                spread_hz: spread,
                            }
                        };
                        Tap::new(ms * 1e-3, 20.0 * rho.log10(), doppler)
                    })
                    .collect()
            }
            Profile::DrmVhf {
                channel,
                max_doppler_hz,
            } => {
                if !(7..=12).contains(channel) {
                    return Param::DrmChannel.range(7.0, 12.0, *channel as f64);
                }
                let fd = Param::MaxDoppler.check_positive(*max_doppler_hz)?;
                DRM_VHF[*channel as usize - 7]
                    .iter()
                    .map(|&(us, db, kind)| {
                        let doppler = match kind {
                            DrmPath::Steady => Doppler::Static,
                            DrmPath::Classical => Doppler::Jakes { max_hz: fd },
                            DrmPath::Gauss1 => Doppler::Gaussian {
                                shift_hz: 0.7 * fd,
                                spread_hz: 0.2 * fd,
                            },
                            DrmPath::Gauss2 => Doppler::Gaussian {
                                shift_hz: -0.7 * fd,
                                spread_hz: 0.2 * fd,
                            },
                        };
                        Tap::new(us * 1e-6, db, doppler)
                    })
                    .collect()
            }
            Profile::Watterson(c) => {
                let (ms, hz) = watterson(*c);
                two_paths(ms, hz)
            }
            Profile::F1487 {
                latitude,
                condition,
            } => match f1487(*latitude, *condition) {
                Some((ms, hz)) => two_paths(ms, hz),
                None => return Param::Latitude.conflict(Param::Doppler),
            },
            Profile::Gsm {
                model,
                max_doppler_hz,
            } => {
                let fd = Param::MaxDoppler.check_positive(*max_doppler_hz)?;
                gsm_taps(*model)
                    .iter()
                    .map(|&(us, db, rice)| {
                        let doppler = if rice {
                            Doppler::Rice {
                                max_hz: fd,
                                los_hz: 0.7 * fd,
                                k: GSM_RICE_A1 / GSM_RICE_A0,
                            }
                        } else {
                            Doppler::Jakes { max_hz: fd }
                        };
                        Tap::new(us * 1e-6, db, doppler)
                    })
                    .collect()
            }
            Profile::Rayleigh { max_doppler_hz } => vec![Tap::new(
                0.0,
                0.0,
                Doppler::Jakes {
                    max_hz: Param::MaxDoppler.check_positive(*max_doppler_hz)?,
                },
            )],
            Profile::Rician {
                k_db,
                max_doppler_hz,
                los_hz,
            } => {
                Param::RiceFactor.check_finite(*k_db)?;
                Param::LosHz.check_finite(*los_hz)?;
                vec![Tap::new(
                    0.0,
                    0.0,
                    Doppler::Rice {
                        max_hz: Param::MaxDoppler.check_positive(*max_doppler_hz)?,
                        los_hz: *los_hz,
                        k: 10f64.powf(k_db / 10.0),
                    },
                )]
            }
            Profile::Echo {
                delay_s,
                level_db,
                phase_deg,
            } => {
                Param::EchoDelay.check_in(*delay_s, 0.0, f64::INFINITY)?;
                Param::EchoLevel.check_finite(*level_db)?;
                Param::EchoPhase.check_finite(*phase_deg)?;
                vec![
                    Tap::new(0.0, 0.0, Doppler::Static),
                    Tap::new(
                        *delay_s,
                        *level_db,
                        Doppler::Steady {
                            shift_hz: 0.0,
                            phase_rad: phase_deg.to_radians(),
                        },
                    ),
                ]
            }
            Profile::Taps(t) => t.clone(),
        })
    }

    /// A direct path and steady echoes: [`Profile::Taps`] with the direct path at 0 dB and one static tap per echo. If an echo
    /// comes before the direct path (a negative delay, a pre-echo), every path is delayed by the earliest pre-echo, so the
    /// earliest path has delay 0. Errors for more than [`EchoPath::MAX`] echoes or an echo out of the ranges of
    /// [`EchoPath::check`].
    ///
    /// ```
    /// use sdrkit_algo_sim::fading::profiles::{EchoPath, Profile};
    /// let p = Profile::echoes(&[EchoPath::new(2e-6, -8.0, 0.0), EchoPath::new(-1e-6, -10.0, 90.0)]).unwrap();
    /// let taps = p.taps().unwrap();
    /// // The direct path waits 1 us for the pre-echo, which comes first.
    /// assert_eq!((taps[0].delay_s, taps[2].delay_s), (1e-6, 0.0));
    /// assert!((taps[1].delay_s - 3e-6).abs() < 1e-15);
    /// ```
    pub fn echoes(paths: &[EchoPath]) -> Result<Profile, SimError> {
        if paths.len() > EchoPath::MAX {
            return Param::Echoes.range(0.0, EchoPath::MAX as f64, paths.len() as f64);
        }
        for e in paths {
            e.check()?;
        }
        let shift = paths.iter().map(|e| -e.delay_s).fold(0.0f64, f64::max);
        let mut taps = Vec::with_capacity(paths.len() + 1);
        taps.push(Tap::new(shift, 0.0, Doppler::Static));
        for e in paths {
            taps.push(Tap::new(
                e.delay_s + shift,
                e.level_db,
                Doppler::Steady {
                    shift_hz: 0.0,
                    phase_rad: e.phase_deg.to_radians(),
                },
            ));
        }
        Ok(Profile::Taps(taps))
    }

    /// The profile `name`, with the parameters the moving and flat profiles need. The
    /// name `echoes` returns the direct path alone: its list comes from [`Profile::echoes`] (the setting `gen_echoes`).
    pub fn from_name(name: ProfileName, p: &ProfileParams) -> Result<Profile, SimError> {
        use Latitude::*;
        let gsm = |model| Profile::Gsm {
            model,
            max_doppler_hz: p.max_doppler_hz,
        };
        let f = |latitude, condition| Profile::F1487 {
            latitude,
            condition,
        };
        Ok(match name {
            ProfileName::None => Profile::None,
            ProfileName::Rayleigh => Profile::Rayleigh {
                max_doppler_hz: p.max_doppler_hz,
            },
            ProfileName::Rician => Profile::Rician {
                k_db: p.k_db,
                max_doppler_hz: p.max_doppler_hz,
                los_hz: p.los_hz,
            },
            ProfileName::Echo => Profile::Echo {
                delay_s: p.echo_delay_s,
                level_db: p.echo_db,
                phase_deg: p.echo_phase_deg,
            },
            ProfileName::Echoes => Profile::echoes(&[])?,
            ProfileName::Drm1 => Profile::Drm(1),
            ProfileName::Drm2 => Profile::Drm(2),
            ProfileName::Drm3 => Profile::Drm(3),
            ProfileName::Drm4 => Profile::Drm(4),
            ProfileName::Drm5 => Profile::Drm(5),
            ProfileName::Drm6 => Profile::Drm(6),
            ProfileName::Drm7
            | ProfileName::Drm8
            | ProfileName::Drm9
            | ProfileName::Drm10
            | ProfileName::Drm11
            | ProfileName::Drm12 => Profile::DrmVhf {
                channel: 7 + (name.index() - ProfileName::Drm7.index()) as u8,
                max_doppler_hz: p.max_doppler_hz,
            },
            ProfileName::HfGood => Profile::Watterson(Watterson::Good),
            ProfileName::HfModerate => Profile::Watterson(Watterson::Moderate),
            ProfileName::HfPoor => Profile::Watterson(Watterson::Poor),
            ProfileName::HfFlutter => Profile::Watterson(Watterson::Flutter),
            ProfileName::F1487LowQuiet => f(Low, F1487::Quiet),
            ProfileName::F1487LowModerate => f(Low, F1487::Moderate),
            ProfileName::F1487LowDisturbed => f(Low, F1487::Disturbed),
            ProfileName::F1487MidQuiet => f(Mid, F1487::Quiet),
            ProfileName::F1487MidModerate => f(Mid, F1487::Moderate),
            ProfileName::F1487MidDisturbed => f(Mid, F1487::Disturbed),
            ProfileName::F1487MidNvis => f(Mid, F1487::DisturbedNvis),
            ProfileName::F1487HighQuiet => f(High, F1487::Quiet),
            ProfileName::F1487HighModerate => f(High, F1487::Moderate),
            ProfileName::F1487HighDisturbed => f(High, F1487::Disturbed),
            ProfileName::GsmRa6 => gsm(Gsm::Ra6),
            ProfileName::GsmTu6 => gsm(Gsm::Tu6),
            ProfileName::GsmTu12 => gsm(Gsm::Tu12),
            ProfileName::GsmHt6 => gsm(Gsm::Ht6),
            ProfileName::GsmHt12 => gsm(Gsm::Ht12),
            ProfileName::GsmEq6 => gsm(Gsm::Eq6),
        })
    }
}

/// One steady echo of [`Profile::echoes`]: its delay after the direct path (negative for a pre-echo), its power relative to the
/// direct path, and its carrier phase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EchoPath {
    /// The delay after the direct path, seconds (negative: the echo comes first).
    pub delay_s: f64,
    /// The power relative to the direct path, dB.
    pub level_db: f64,
    /// The carrier phase, degrees.
    pub phase_deg: f64,
}

impl EchoPath {
    /// The most echoes a profile takes.
    pub const MAX: usize = 16;

    /// An echo `delay_s` after the direct path, `level_db` relative to it, its carrier turned by `phase_deg`.
    pub const fn new(delay_s: f64, level_db: f64, phase_deg: f64) -> EchoPath {
        EchoPath {
            delay_s,
            level_db,
            phase_deg,
        }
    }

    /// An error unless the delay is in [-1, 1] s, the level in [-80, 20] dB and the phase in [-180, 180] degrees (the ranges
    /// of the single-echo settings, with negative delays added).
    pub fn check(&self) -> Result<(), SimError> {
        Param::EchoDelay.check_in(self.delay_s, -1.0, 1.0)?;
        Param::EchoLevel.check_in(self.level_db, -80.0, 20.0)?;
        Param::EchoPhase.check_in(self.phase_deg, -180.0, 180.0)?;
        Ok(())
    }

    /// Reads a list of echoes as the setting `gen_echoes` holds it: echoes separated by `;`, each the delay in seconds, the
    /// level in dB and, if present, the phase in degrees (0 if absent), separated by spaces or commas. Empty text provides no
    /// echoes. Errors for a field that is not a number, a wrong count of fields, a value out of range, or more than
    /// [`EchoPath::MAX`] echoes.
    ///
    /// ```
    /// use sdrkit_algo_sim::fading::profiles::EchoPath;
    /// let e = EchoPath::parse_list("0.8e-6 -9; 3e-6, -12, 45").unwrap();
    /// assert_eq!(e, [EchoPath::new(0.8e-6, -9.0, 0.0), EchoPath::new(3e-6, -12.0, 45.0)]);
    /// assert!(EchoPath::parse_list("3e-6").is_err());
    /// ```
    pub fn parse_list(text: &str) -> Result<Vec<EchoPath>, SimError> {
        let mut out = Vec::new();
        for (index, entry) in text.split(';').enumerate() {
            let fail = |fault| SimError::EchoText {
                entry: index,
                fault,
            };
            if entry.trim().is_empty() {
                continue;
            }
            let mut v = [0.0f64; 3];
            let mut n = 0;
            for field in entry
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|f| !f.is_empty())
            {
                if n == 3 {
                    return Err(fail(EchoFault::TooManyFields));
                }
                v[n] = match field.parse::<f64>() {
                    Ok(x) if x.is_finite() => x,
                    _ => return Err(fail(EchoFault::NotANumber)),
                };
                n += 1;
            }
            if n < 2 {
                return Err(fail(EchoFault::TooFewFields));
            }
            let e = EchoPath::new(v[0], v[1], v[2]);
            e.check()?;
            out.push(e);
        }
        if out.len() > EchoPath::MAX {
            return Param::Echoes.range(0.0, EchoPath::MAX as f64, out.len() as f64);
        }
        Ok(out)
    }
}

/// The parameters [`Profile::from_name`] fills in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProfileParams {
    /// The maximum Doppler of moving and flat profiles, Hz.
    pub max_doppler_hz: f64,
    /// The Rice factor of `rician`, dB.
    pub k_db: f64,
    /// The direct path's frequency of `rician`, Hz.
    pub los_hz: f64,
    /// The echo's delay, seconds.
    pub echo_delay_s: f64,
    /// The echo's level, dB.
    pub echo_db: f64,
    /// The echo's carrier phase, degrees.
    pub echo_phase_deg: f64,
}

impl Default for ProfileParams {
    fn default() -> Self {
        ProfileParams {
            max_doppler_hz: 10.0,
            k_db: 6.0,
            los_hz: 0.0,
            echo_delay_s: 1e-3,
            echo_db: -6.0,
            echo_phase_deg: 0.0,
        }
    }
}
