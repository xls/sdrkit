//! Tuning steps, the utility and digital keys of the shortwave screen, what a key does, and the S-meter mappings. The
//! band keys, the shortwave broadcast bands and the amateur voice segments are data of the host (`sdrkit-presets`).

use crate::amproc::AmDetector;
use crate::types::Demod;

/// The step list (Hz).
pub const STEPS_HZ: [f64; 13] = [
    10.0, 100.0, 1000.0, 2500.0, 5000.0, 6250.0, 8333.333, 9000.0, 10_000.0, 12_500.0, 25_000.0,
    50_000.0, 100_000.0,
];

/// The steps of the phone screen (Hz).
pub const PHONE_STEPS_HZ: [f64; 6] = [100.0, 1000.0, 5000.0, 12_500.0, 25_000.0, 100_000.0];

/// Knob steps per turn on the phone screen.
pub const KNOB_STEPS_PER_TURN: u32 = 40;

/// The lowest listening frequency.
pub const MIN_LISTEN_HZ: f64 = 1000.0;

/// Snaps `f` to the nearest multiple of `step`.
pub fn snap(f: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return f;
    }
    (f / step).round() * step
}

/// Up (`dir` > 0) or down: the frequency is first snapped to the step, then one step is added or taken away. Never below 1 kHz.
pub fn step_frequency(f: f64, step: f64, dir: i32) -> f64 {
    (snap(f, step) + step * dir.signum() as f64).max(MIN_LISTEN_HZ)
}

/// What a new listening frequency does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TuneAction {
    /// The channel moves inside the capture: the new channel offset (Hz from the dial).
    MoveChannel(f64),
    /// The radio is retuned to this frequency and the channel offset becomes 0.
    Retune(f64),
}

/// Where a new listening frequency goes. `centre` and `rate` describe the capture, `dial` the dial frequency, `edges` the filter
/// edges relative to the channel. With `offset_only` (the "decode offset only" tuning style) the channel is clamped into the capture
/// and the radio never moves.
pub fn tune_to(
    f: f64,
    dial: f64,
    centre: f64,
    rate: f64,
    edges: (f64, f64),
    offset_only: bool,
) -> TuneAction {
    let f = f.max(MIN_LISTEN_HZ);
    let (lo, hi) = (centre - 0.45 * rate, centre + 0.45 * rate);
    if f + edges.0 >= lo && f + edges.1 <= hi {
        TuneAction::MoveChannel(f - dial)
    } else if offset_only {
        let c = f.clamp(lo - edges.0, (hi - edges.1).max(lo - edges.0));
        TuneAction::MoveChannel(c - dial)
    } else {
        TuneAction::Retune(f)
    }
}

/// A key of the phone screen's band row.
#[derive(Clone, Debug, PartialEq)]
pub struct BandKey {
    /// The label ("40 m", "MARINE", "88.1").
    pub label: String,
    /// The frequency it tunes (Hz).
    pub freq_hz: f64,
    /// The demodulator it sets, when it sets one.
    pub demod: Option<Demod>,
}

fn key(label: &str, mhz: f64, demod: Option<Demod>) -> BandKey {
    BandKey {
        label: label.into(),
        freq_hz: mhz * 1e6,
        demod,
    }
}

/// The utility keys of the shortwave screen (time signals AM, NAVTEX and fax USB).
pub fn utility_keys() -> Vec<BandKey> {
    let mut v: Vec<BandKey> = [2.5, 5.0, 10.0, 15.0, 20.0]
        .iter()
        .map(|&f| key(&format!("WWV {f}"), f, Some(Demod::Am)))
        .collect();
    v.extend(
        [3.33, 7.85, 14.67]
            .iter()
            .map(|&f| key(&format!("CHU {f}"), f, Some(Demod::Am))),
    );
    v.push(key("NAVTEX", 4.2095, Some(Demod::Usb)));
    v.extend(
        [3.855, 7.88, 13.8825]
            .iter()
            .map(|&f| key(&format!("FAX {f}"), f, Some(Demod::Usb))),
    );
    v
}

/// The digital keys of the shortwave screen (all USB, the middle of the activity).
pub fn digital_keys() -> Vec<BandKey> {
    let mut v = Vec::new();
    for (name, list) in [
        (
            "FT8",
            &[7.0755, 10.1375, 14.0755, 18.1015, 21.0755, 28.0755][..],
        ),
        ("RTTY", &[3.5815, 7.0415, 14.0815][..]),
        ("PSK", &[7.0715, 14.0715][..]),
        ("SSTV", &[14.2315][..]),
        ("WSPR", &[14.0971][..]),
    ] {
        v.extend(
            list.iter()
                .map(|&f| key(&format!("{name} {f}"), f, Some(Demod::Usb))),
        );
    }
    v
}

/// The FT8 dial frequencies the amateur band keys of the digital stacks tune (Hz).
pub const FT8_DIAL_HZ: [f64; 12] = [
    1.840e6, 3.573e6, 7.074e6, 10.136e6, 14.074e6, 18.100e6, 21.074e6, 24.915e6, 28.074e6,
    50.313e6, 144.174e6, 432.174e6,
];

/// A key lights when the listening frequency is within max(3 % of the key's frequency, 0.15 MHz) of it.
pub fn key_lit(key_hz: f64, listening_hz: f64) -> bool {
    (key_hz - listening_hz).abs() <= (0.03 * key_hz).max(0.15e6)
}

/// The AM filter of the shortwave broadcast keys (Hz).
pub const SW_FILTER_HZ: f64 = 9000.0;
/// The step of the shortwave broadcast keys (the HFCC raster, Hz).
pub const SW_STEP_HZ: f64 = 5000.0;

/// The sideband convention: LSB below 10 MHz, USB from 10 MHz up, and USB on 60 m (5.3 to 5.5 MHz).
pub fn conventional_sideband(hz: f64) -> Demod {
    let mhz = hz / 1e6;
    if (5.3..=5.5).contains(&mhz) || mhz >= 10.0 {
        Demod::Usb
    } else {
        Demod::Lsb
    }
}

/// The width a ham plan key sets (SSB 2.7 kHz, FM 12.5 kHz).
pub fn ham_width(demod: Demod) -> f64 {
    if demod.is_fm() {
        12_500.0
    } else {
        2700.0
    }
}

/// The centre of a Maidenhead locator (2, 4, 6, 8 or 10 characters) as (latitude, longitude) in degrees, by `sdrkit_util_geo`.
/// `None` for a locator that is not valid.
pub fn locator_position(loc: &str) -> Option<(f64, f64)> {
    let c = sdrkit_util_geo::maidenhead::decode(loc.trim())
        .ok()?
        .centre();
    Some((c.lat_deg, c.lon_deg))
}

/// The position of the needle S-meter (0 to 1) for an uncalibrated level in dBFS.
pub fn needle_position(level_dbfs: f64) -> f64 {
    ((level_dbfs + 110.0) / 80.0).clamp(0.0, 1.0)
}

/// Calibrated S units (IARU R.1): S9 is -73 dBm below 30 MHz and -93 dBm from 30 MHz up, 6 dB per S unit. Returns the S unit (1 to
/// 9) and the dB over S9 (0 below it), and the text ("S7", "S9 + 12 dB").
pub fn s_units(dbm: f64, freq_hz: f64) -> (u8, f64, String) {
    let s9 = if freq_hz < 30e6 { -73.0 } else { -93.0 };
    if dbm >= s9 {
        let over = dbm - s9;
        let t = if over >= 0.5 {
            format!("S9 + {over:.0} dB")
        } else {
            "S9".into()
        };
        (9, over, t)
    } else {
        let s = (9.0 + ((dbm - s9) / 6.0).round()).clamp(0.0, 9.0) as u8;
        (s, 0.0, format!("S{s}"))
    }
}

/// What a key does: the frequency that it tunes and the settings that it applies. The channel goes on the dial.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyAction {
    /// The listening frequency (Hz).
    pub freq_hz: f64,
    /// The demodulator.
    pub demod: Demod,
    /// The filter width (Hz).
    pub width_hz: f64,
    /// The tuning step (Hz), when the key sets one.
    pub step_hz: Option<f64>,
    /// The AM detector, when the key sets one.
    pub am_detector: Option<AmDetector>,
}

/// A shortwave broadcast band key that tunes `centre_hz`: AM with a 9 kHz filter and a 5 kHz step, and the synchronous
/// detector when Auto quality is off.
pub fn sw_band_key(centre_hz: f64, am_auto: bool) -> KeyAction {
    KeyAction {
        freq_hz: centre_hz,
        demod: Demod::Am,
        width_hz: SW_FILTER_HZ,
        step_hz: Some(SW_STEP_HZ),
        am_detector: (!am_auto).then_some(AmDetector::Synchronous),
    }
}

/// A key of an amateur voice segment: its tune frequency `tune_hz` with the demodulator `demod` and the width of
/// [`ham_width`].
pub fn ham_key(tune_hz: f64, demod: Demod) -> KeyAction {
    KeyAction {
        freq_hz: tune_hz,
        demod,
        width_hz: ham_width(demod),
        step_hz: None,
        am_detector: None,
    }
}

/// What a decoder key of the shortwave screen needs from the receiver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoderNeed {
    /// A sideband decoder (FT8, RTTY, PSK, SSTV ...).
    Sideband,
    /// The CW (Morse) decoder.
    Cw,
    /// Time signals, identification and NAVTEX keys leave the demodulator alone.
    Nothing,
}

/// The demodulator and width that a decoder key sets at `freq_hz` with the demodulator `current`: a sideband decoder sets USB or
/// LSB (by the convention of the band plan) with 2.7 kHz unless the demodulator is already USB or LSB, and the CW decoder sets CW
/// with 500 Hz unless it is CW, USB or LSB. `None` when the key changes nothing.
pub fn decoder_key(need: DecoderNeed, current: Demod, freq_hz: f64) -> Option<(Demod, f64)> {
    match need {
        DecoderNeed::Sideband if !current.is_ssb() => {
            Some((conventional_sideband(freq_hz), 2700.0))
        }
        DecoderNeed::Cw if !matches!(current, Demod::Cw | Demod::Usb | Demod::Lsb) => {
            Some((Demod::Cw, 500.0))
        }
        _ => None,
    }
}

/// The bar display's scale: -130 to 0 dBFS, ticks every 10 dB from -120 to -10.
pub const BAR_SCALE_DBFS: (f64, f64) = (-130.0, 0.0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sideband_convention() {
        assert_eq!(conventional_sideband(5.357e6), Demod::Usb);
        assert_eq!(conventional_sideband(7.1e6), Demod::Lsb);
        assert_eq!(conventional_sideband(14.2e6), Demod::Usb);
    }

    #[test]
    fn key_actions() {
        let k = sw_band_key(9_650_000.0, false);
        assert_eq!(k.freq_hz, 9_650_000.0);
        assert_eq!(k.am_detector, Some(AmDetector::Synchronous));
        assert!(k.width_hz == 9000.0 && k.step_hz == Some(5000.0));
        assert_eq!(sw_band_key(9_650_000.0, true).am_detector, None);
        let h = ham_key(145.5e6, Demod::Nfm);
        assert_eq!(h.freq_hz, 145.5e6);
        assert_eq!(h.width_hz, 12_500.0);
        assert_eq!(
            decoder_key(DecoderNeed::Sideband, Demod::Am, 7.074e6),
            Some((Demod::Lsb, 2700.0))
        );
        assert_eq!(
            decoder_key(DecoderNeed::Sideband, Demod::Usb, 7.074e6),
            None
        );
        assert_eq!(
            decoder_key(DecoderNeed::Cw, Demod::Nfm, 7.03e6),
            Some((Demod::Cw, 500.0))
        );
        assert_eq!(decoder_key(DecoderNeed::Cw, Demod::Lsb, 7.03e6), None);
        assert_eq!(decoder_key(DecoderNeed::Nothing, Demod::Am, 10e6), None);
    }

    #[test]
    fn steps_keys_and_meters() {
        assert_eq!(step_frequency(145_503_000.0, 12_500.0, 1), 145_512_500.0);
        assert_eq!(step_frequency(1500.0, 1000.0, -1), 1000.0);
        assert_eq!(snap(7_100_049.0, 100.0), 7_100_000.0);
        assert!(key_lit(7.15e6, 7.1e6));
        assert!(!key_lit(14.2e6, 15.0e6));
        assert_eq!(needle_position(-110.0), 0.0);
        assert_eq!(needle_position(-30.0), 1.0);
        assert_eq!(s_units(-73.0, 14e6).2, "S9");
        assert_eq!(s_units(-61.0, 14e6).2, "S9 + 12 dB");
        assert_eq!(s_units(-105.0, 145e6).2, "S7");
        let a = tune_to(145.55e6, 145.5e6, 145.75e6, 2e6, (-6250.0, 6250.0), false);
        assert_eq!(a, TuneAction::MoveChannel(50e3));
        let b = tune_to(150e6, 145.5e6, 145.75e6, 2e6, (-6250.0, 6250.0), false);
        assert_eq!(b, TuneAction::Retune(150e6));
        let c = tune_to(150e6, 145.5e6, 145.75e6, 2e6, (-6250.0, 6250.0), true);
        assert!(
            matches!(c, TuneAction::MoveChannel(o) if (o - (146.65e6 - 6250.0 - 145.5e6)).abs() < 1.0)
        );
        assert_eq!(utility_keys().len(), 12);
        assert_eq!(digital_keys().len(), 13);
    }
}
