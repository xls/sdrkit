//! The automatic TV choice: a verdict on the TV standard of a channel, and the controller that looks up to three times and then
//! stops waiting, or requests the signal type of the standard, tuned onto a channel more than 0.4 MHz off the centre.

use crate::identify::Identification;
use crate::signature::ModClass;
use crate::table::dvbt_width_for;
use crate::verify::Check;
use serde::{Deserialize, Serialize};

/// A TV standard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TvStandard {
    /// DVB-T.
    DvbT,
    /// DVB-T2.
    DvbT2,
    /// DVB-C.
    DvbC,
    /// DVB-S or DVB-S2.
    DvbS,
    /// ISDB-T.
    IsdbT,
    /// ATSC 1.0 (8-VSB).
    Atsc,
    /// ATSC 3.0.
    Atsc3,
    /// DTMB.
    Dtmb,
    /// Analogue, with its system letters ("B/G", "I", "M", "D/K", "L").
    Analog(String),
}

impl TvStandard {
    /// The key of the signal type of the standard. A DVB-T verdict that is not sure returns the key for DVB-T or DVB-T2.
    pub fn signal(&self, sureness: TvSureness) -> &'static str {
        match self {
            TvStandard::DvbT if sureness == TvSureness::Sure => "dvb_t",
            TvStandard::DvbT => "dvb_t_or_t2",
            TvStandard::DvbT2 => "dvb_t2",
            TvStandard::DvbC => "dvb_c",
            TvStandard::DvbS => "dvb_s",
            TvStandard::IsdbT => "isdb_t",
            TvStandard::Atsc => "atsc_1_0",
            TvStandard::Atsc3 => "atsc_3_0",
            TvStandard::Dtmb => "dtmb",
            TvStandard::Analog(sys) => match sys.as_str() {
                "B/G" => "analog_tv_bg",
                "I" => "analog_tv_i",
                "M" | "M/N" => "analog_tv_m",
                "D/K" => "analog_tv_dk",
                "L" => "analog_tv_l",
                _ => "analog_tv",
            },
        }
    }
    /// Its name.
    pub fn name(&self) -> String {
        match self {
            TvStandard::DvbT => "DVB-T".into(),
            TvStandard::DvbT2 => "DVB-T2".into(),
            TvStandard::DvbC => "DVB-C".into(),
            TvStandard::DvbS => "DVB-S/S2".into(),
            TvStandard::IsdbT => "ISDB-T".into(),
            TvStandard::Atsc => "ATSC".into(),
            TvStandard::Atsc3 => "ATSC 3.0".into(),
            TvStandard::Dtmb => "DTMB".into(),
            TvStandard::Analog(s) => format!("Analog, system {s}"),
        }
    }
}

/// How sure a TV verdict is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TvSureness {
    /// No signal.
    NoSignal,
    /// A signal, standard unclear.
    Unsure,
    /// Likely.
    Likely,
    /// Sure.
    Sure,
}

/// What a TV detector found.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TvVerdict {
    /// The standard, when found.
    pub standard: Option<TvStandard>,
    /// How sure.
    pub sureness: TvSureness,
    /// 0 to 1.
    pub confidence: f64,
    /// FFT size (OFDM).
    pub fft: Option<usize>,
    /// Guard interval as a fraction ("1/8"), for OFDM.
    pub guard: Option<String>,
    /// Channel width (MHz).
    pub width_mhz: Option<f64>,
    /// Symbol rate (single carrier, symbols a second).
    pub symbol_rate: Option<f64>,
    /// The channel's offset from the capture's centre (Hz).
    pub offset_hz: f64,
    /// "DVB-T2, 32K, GI 1/8, 8 MHz (sure, 97 %)".
    pub headline: String,
    /// One line per step.
    pub evidence: Vec<String>,
}

const GUARDS: [(f64, &str); 7] = [
    (0.25, "1/4"),
    (19.0 / 128.0, "19/128"),
    (0.125, "1/8"),
    (19.0 / 256.0, "19/256"),
    (1.0 / 16.0, "1/16"),
    (1.0 / 32.0, "1/32"),
    (1.0 / 128.0, "1/128"),
];

fn fft_word(n: usize) -> String {
    format!("{}K", n / 1024)
}

/// A TV verdict from an identification of the channel: the OFDM standards by their useful symbol and guard, sure when the cyclic
/// prefix was confirmed at the exact symbol length and the width fits.
pub fn tv_verdict(id: &Identification) -> TvVerdict {
    let f = &id.features;
    let mut v = TvVerdict {
        standard: None,
        sureness: TvSureness::NoSignal,
        confidence: 0.0,
        fft: None,
        guard: None,
        width_mhz: None,
        symbol_rate: None,
        offset_hz: f.offset_hz,
        headline: "no TV signal found".into(),
        evidence: Vec::new(),
    };
    if f.class == ModClass::Noise || f.snr_db < 4.0 || f.width_99_hz < 1e6 {
        v.evidence.push(format!(
            "no signal of TV width above the noise floor ({:.1} MHz, SNR {:.1} dB)",
            f.width_99_hz / 1e6,
            f.snr_db
        ));
        return v;
    }
    v.sureness = TvSureness::Unsure;
    v.confidence = 0.5;
    v.headline = "a signal, but the standard is unclear".into();
    let Some(o) = f.ofdm.filter(|_| f.class == ModClass::Ofdm) else {
        v.evidence.push(format!("{}: not OFDM", id.summary));
        return v;
    };
    let tu_us = o.tu_s * 1e6;
    let guard = o.guard_fraction.and_then(|g| {
        GUARDS
            .iter()
            .find(|(x, _)| (g / x - 1.0).abs() < 0.15)
            .map(|(_, w)| w.to_string())
    });
    v.evidence.push(format!(
        "OFDM, useful symbol {tu_us:.1} µs, guard {}",
        guard.as_deref().unwrap_or("not found")
    ));
    let (w, n, e) = dvbt_width_for(o.tu_s);
    let isdb = [6.0, 7.0, 8.0]
        .iter()
        .flat_map(|w| (1..=3).map(move |m| (*w, m, 252.0 * (1 << (m - 1)) as f64 * 6.0 / w)))
        .map(|(w, m, tu)| (w, m, (tu_us / tu - 1.0).abs()))
        .fold((0.0, 0, f64::MAX), |a, b| if b.2 < a.2 { b } else { a });
    let dtmb = (tu_us / 500.0 - 1.0).abs();
    let fits_width =
        |mhz: f64, share: f64| (f.width_99_hz / (mhz * 1e6 * share) - 1.0).abs() < 0.08;
    let cp_confirmed = id
        .candidates
        .iter()
        .filter_map(|c| c.verification.as_ref())
        .any(|v| {
            v.check == Check::Confirmed && v.notes.iter().any(|n| n.contains("cyclic prefix"))
        });
    let (std, width, extra) = if dtmb < 0.01 && fits_width(8.0, 0.945) {
        (TvStandard::Dtmb, 8.0, "PN".to_string())
    } else if isdb.2 < 0.01 && isdb.2 < e && fits_width(isdb.0, 0.929) {
        (TvStandard::IsdbT, isdb.0, format!("mode {}", isdb.1))
    } else if e < 0.01 {
        let t2_only = matches!(n, 1024 | 16384 | 32768)
            || matches!(
                guard.as_deref(),
                Some("19/128") | Some("19/256") | Some("1/128")
            );
        let s = if t2_only {
            TvStandard::DvbT2
        } else {
            TvStandard::DvbT
        };
        (s, w, fft_word(n))
    } else {
        v.evidence
            .push(format!("useful symbol {tu_us:.1} µs fits no TV standard"));
        return v;
    };
    let share = match std {
        TvStandard::IsdbT => 0.929,
        TvStandard::Dtmb => 0.945,
        _ if n >= 16384 => 0.97,
        _ => 0.952,
    };
    let width_ok = fits_width(width, share);
    v.evidence.push(format!(
        "{} wide against {:.2} MHz expected for {} MHz{}",
        crate::features::fmt_width(f.width_99_hz),
        width * share,
        width,
        if width_ok { "" } else { " (does not fit)" }
    ));
    let (sureness, conf) = if cp_confirmed && width_ok && o.z >= 12.0 {
        (TvSureness::Sure, 0.97)
    } else {
        (TvSureness::Likely, 0.8)
    };
    // DVB-T from a short capture is at most likely: a DVB-T2 P1 symbol could have been missed.
    let (sureness, conf) =
        if std == TvStandard::DvbT && f.capture_s < 0.27 && sureness == TvSureness::Sure {
            v.evidence
                .push("capture shorter than 0.27 s: DVB-T2 not ruled out".into());
            (TvSureness::Likely, 0.8)
        } else {
            (sureness, conf)
        };
    v.fft = (matches!(std, TvStandard::DvbT | TvStandard::DvbT2)).then_some(n);
    v.guard = guard.clone();
    v.width_mhz = Some(width);
    v.sureness = sureness;
    v.confidence = conf;
    v.headline = format!(
        "{}, {}{}, {} MHz ({}, {:.0} %)",
        std.name(),
        extra,
        guard.map_or(String::new(), |g| format!(", GI {g}")),
        width,
        match sureness {
            TvSureness::Sure => "sure",
            _ => "likely",
        },
        conf * 100.0
    );
    v.standard = Some(std);
    v
}

/// What the engine should do for TV (auto).
#[derive(Clone, Debug, PartialEq)]
pub enum TvAction {
    /// Wait for the radio to settle, then capture this long (s) and detect.
    Capture {
        /// Seconds to wait first.
        after_s: f64,
        /// Seconds of samples to capture.
        capture_s: f64,
        /// The status line.
        message: String,
    },
    /// Stop looking; show the message and a "Try again" button.
    GiveUp {
        /// The status line.
        message: String,
    },
    /// Open the signal type of the standard (or open it again with a new channel width).
    Switch {
        /// The key of the signal type.
        signal: &'static str,
        /// The channel width, Hz, when the verdict has it.
        bandwidth_hz: Option<f64>,
        /// Tune here first (Hz), when the channel is off the centre.
        tune_hz: Option<f64>,
        /// The status line.
        message: String,
        /// The log line.
        log: String,
    },
    /// The receiver already running is the right one.
    Keep {
        /// The status line.
        message: String,
    },
}

/// The TV (auto) controller.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TvAuto {
    tries: u32,
    current: Option<(String, Option<u64>)>,
    active: bool,
}

/// Tries before TV (auto) stops waiting.
pub const TV_TRIES: u32 = 3;
/// Seconds for the radio to settle.
pub const TV_SETTLE_S: f64 = 1.0;
/// Seconds captured (a DVB-T2 frame is at most 250 ms).
pub const TV_CAPTURE_S: f64 = 0.35;

impl TvAuto {
    /// Starts (the user picked TV (auto), or pressed "Try again" or "Look again").
    pub fn start(&mut self) -> TvAction {
        self.tries = 0;
        self.active = true;
        TvAction::Capture {
            after_s: TV_SETTLE_S,
            capture_s: TV_CAPTURE_S,
            message: "Listening to the channel...".into(),
        }
    }

    /// Whether TV (auto) is running.
    pub fn active(&self) -> bool {
        self.active
    }

    /// Ends the automatic TV choice (the user chose a stack by hand).
    pub fn stop(&mut self) {
        self.active = false;
        self.current = None;
    }

    /// A detector's verdict for the samples captured at `rf_hz` (the capture's centre).
    pub fn on_verdict(&mut self, v: &TvVerdict, rf_hz: f64) -> TvAction {
        self.tries += 1;
        let again = self.tries < TV_TRIES;
        match (v.sureness, &v.standard) {
            (TvSureness::NoSignal, _) => {
                if again {
                    TvAction::Capture {
                        after_s: 1.0,
                        capture_s: TV_CAPTURE_S,
                        message: "No TV signal on this frequency: still listening...".into(),
                    }
                } else {
                    self.active = false;
                    TvAction::GiveUp {
                        message: "No TV signal found here. Tune to a TV channel (or pick the standard by hand in the stack list)."
                            .into(),
                    }
                }
            }
            (TvSureness::Unsure, _) | (_, None) => {
                if again {
                    TvAction::Capture {
                        after_s: 1.0,
                        capture_s: TV_CAPTURE_S,
                        message: format!("Unsure what this is ({}): looking again...", v.headline),
                    }
                } else {
                    self.active = false;
                    TvAction::GiveUp {
                        message: format!(
                            "Unsure: {}. Pick the standard by hand in the stack list.",
                            v.headline
                        ),
                    }
                }
            }
            (_, Some(std)) => {
                let signal = std.signal(v.sureness);
                let bandwidth_hz = v.width_mhz.map(|w| w * 1e6);
                let analog = matches!(std, TvStandard::Analog(_));
                let tune_hz = (!analog && v.offset_hz.abs() > 0.4e6)
                    .then(|| ((rf_hz + v.offset_hz) / 1e3).round() * 1e3);
                let key = (signal.to_string(), bandwidth_hz.map(|b| b.round() as u64));
                if tune_hz.is_none() && self.current.as_ref() == Some(&key) {
                    return TvAction::Keep {
                        message: format!(
                            "Found {}. The {} stack is locking...",
                            v.headline,
                            std.name()
                        ),
                    };
                }
                self.current = Some(key);
                TvAction::Switch {
                    signal,
                    bandwidth_hz,
                    tune_hz,
                    message: format!(
                        "Found {}. The {} stack is locking...",
                        v.headline,
                        std.name()
                    ),
                    log: format!("TV (auto): {}", v.headline),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(std: Option<TvStandard>, sure: TvSureness, offset: f64) -> TvVerdict {
        TvVerdict {
            standard: std,
            sureness: sure,
            confidence: 0.9,
            fft: Some(8192),
            guard: Some("1/8".into()),
            width_mhz: Some(8.0),
            symbol_rate: None,
            offset_hz: offset,
            headline: "DVB-T, 8K, GI 1/8, 8 MHz (sure, 97 %)".into(),
            evidence: vec![],
        }
    }

    #[test]
    fn three_tries_then_returns_up() {
        let mut t = TvAuto::default();
        assert!(matches!(t.start(), TvAction::Capture { .. }));
        let none = verdict(None, TvSureness::NoSignal, 0.0);
        assert!(matches!(
            t.on_verdict(&none, 500e6),
            TvAction::Capture { .. }
        ));
        assert!(matches!(
            t.on_verdict(&none, 500e6),
            TvAction::Capture { .. }
        ));
        assert!(matches!(
            t.on_verdict(&none, 500e6),
            TvAction::GiveUp { .. }
        ));
        assert!(!t.active());
        t.start();
        let unsure = verdict(None, TvSureness::Unsure, 0.0);
        t.on_verdict(&unsure, 500e6);
        t.on_verdict(&unsure, 500e6);
        match t.on_verdict(&unsure, 500e6) {
            TvAction::GiveUp { message } => assert!(message.starts_with("Unsure:")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn switches_once_and_tunes_onto_an_offset_channel() {
        let mut t = TvAuto::default();
        t.start();
        let v = verdict(Some(TvStandard::DvbT), TvSureness::Sure, 1.2e6);
        match t.on_verdict(&v, 498e6) {
            TvAction::Switch {
                signal,
                bandwidth_hz,
                tune_hz,
                log,
                ..
            } => {
                assert_eq!(signal, "dvb_t");
                assert_eq!(bandwidth_hz, Some(8e6));
                assert_eq!(tune_hz, Some(499.2e6));
                assert!(log.starts_with("TV (auto): DVB-T"));
            }
            other => panic!("{other:?}"),
        }
        // Centred now: the same receiver keeps running.
        let v = verdict(Some(TvStandard::DvbT), TvSureness::Sure, 10e3);
        assert!(matches!(t.on_verdict(&v, 499.2e6), TvAction::Keep { .. }));
        // Likely DVB-T opens the signal type of either generation.
        let mut t = TvAuto::default();
        t.start();
        let v = verdict(Some(TvStandard::DvbT), TvSureness::Likely, 0.0);
        assert!(
            matches!(t.on_verdict(&v, 498e6), TvAction::Switch { signal, .. } if signal == "dvb_t_or_t2")
        );
        assert_eq!(
            TvStandard::Analog("B/G".into()).signal(TvSureness::Likely),
            "analog_tv_bg"
        );
    }
}
