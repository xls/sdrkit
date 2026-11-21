//! The transmit settings of the general stack and their values as the transmitter reads them.

use crate::settings::Deemphasis;
use crate::types::Demod;
pub use sdrkit_components::voice::SsbMethod;
use sdrkit_core::publish::field;
use sdrkit_core::{Setting, SettingError, SettingId, Value};

/// The section of the transmit settings.
pub const TX: &str = "Transmit";

sdrkit_core::choice_enum! {
    /// The audio band of SSB.
    #[derive(Default)]
    pub enum SsbBand {
        /// 300 to 2700 Hz.
        #[default]
        Fixed = ("fixed", "300 to 2700 Hz"),
        /// From 300 Hz over the width of the receive filter.
        Filter = ("filter", "As the filter"),
    }
}

/// The transmit frequency less the listening frequency.
pub const TX_SPLIT: Setting<field::Hz> = Setting::new("tx_split", "Transmit offset", "Hz", 0.0)
    .range(-1e10, 1e10, 1.0)
    .help("The transmit frequency less the listening frequency: 0 for simplex, the shift of a repeater for duplex.")
    .section(TX);
/// The peak deviation of NFM.
pub const TX_DEVIATION: Setting<field::Hz> =
    Setting::new("tx_deviation", "Deviation (NFM)", "Hz", 5000.0)
        .range(1000.0, 10_000.0, 100.0)
        .common(&[2500.0, 5000.0])
        .help("The peak deviation of NFM speech with its tone: 5000 Hz in 25 kHz channels, 2500 Hz in 12.5 kHz channels.")
        .section(TX);
/// The pre-emphasis of NFM.
pub const TX_PREEMPHASIS: Setting<field::Choice<Deemphasis>> =
    Setting::choice("tx_preemphasis", "Pre-emphasis (NFM)", Deemphasis::Off)
        .help("Lifts the treble of NFM speech. A receiver with the same de-emphasis hears it flat.")
        .section(TX);
/// The CTCSS tone of NFM.
pub const TX_CTCSS: Setting<field::Hz> = Setting::new("tx_ctcss", "CTCSS tone", "Hz", 0.0)
    .range(0.0, 300.0, 0.1)
    .zero_means("No tone")
    .help("A sub-audio tone sent under NFM speech at 12 % of the deviation.")
    .section(TX);
/// The modulation depth limit of AM.
pub const TX_AM_DEPTH: Setting<field::F64> = Setting::new(
    "tx_am_depth",
    "Modulation depth (AM)",
    "%",
    90.0,
)
.range(10.0, 100.0, 1.0)
.help("The largest modulation depth of AM. The carrier and both sidebands stay within full scale.")
.section(TX);
/// The sideband method of SSB.
pub const TX_SSB_METHOD: Setting<field::Choice<SsbMethod>> =
    Setting::choice("tx_ssb_method", "Sideband method (SSB)", SsbMethod::Filter)
        .help("Filter: a band-pass filter passes one sideband. Phasing: a Hilbert transformer cancels the other one.")
        .section(TX);
/// The audio band of SSB.
pub const TX_SSB_BAND: Setting<field::Choice<SsbBand>> =
    Setting::choice("tx_ssb_band", "Audio band (SSB)", SsbBand::Fixed)
        .help("300 to 2700 Hz, or from 300 Hz over the width of the receive filter.")
        .section(TX);
/// The compression of SSB speech.
pub const TX_COMPRESSION: Setting<field::F64> = Setting::new(
    "tx_compression",
    "Compression (SSB)",
    "dB",
    0.0,
)
.range(0.0, 20.0, 1.0)
.zero_means("Off")
.help("Raises the average level of SSB speech by up to this much. The peaks stay at full scale.")
.section(TX);
/// The time-out of push-to-talk.
pub const TX_TIMEOUT: Setting<field::Duration> = Setting::new("tx_timeout", "Time-out", "s", 180.0)
    .range(1.0, 3600.0, 1.0)
    .help("Push-to-talk ends after this time on air.")
    .section(TX);
/// The speed of CW.
pub const TX_CW_WPM: Setting<field::F64> = Setting::new("tx_cw_wpm", "Speed (CW)", "WPM", 20.0)
    .range(5.0, 60.0, 1.0)
    .help("The speed of the characters that send keys.")
    .section(TX);
/// The overall speed of CW (Farnsworth).
pub const TX_CW_FARNSWORTH: Setting<field::F64> =
    Setting::new("tx_cw_farnsworth", "Overall speed (CW)", "WPM", 0.0)
        .range(0.0, 60.0, 1.0)
        .zero_means("The speed of the characters")
        .help("Farnsworth spacing: the characters at the speed, the gaps stretched to this lower overall speed.")
        .section(TX);
/// The time limit of the tune carrier.
pub const TX_TUNE_LIMIT: Setting<field::Duration> =
    Setting::new("tx_tune_limit", "Tune time", "s", 10.0)
        .range(1.0, 120.0, 1.0)
        .help("The tune carrier ends after this time.")
        .section(TX);

/// The values of the transmit settings that the host sets.
#[derive(Clone, Debug, PartialEq)]
pub struct TxSettings {
    /// The transmit offset (Hz).
    pub split: f64,
    /// The NFM deviation (Hz).
    pub deviation: f64,
    /// The NFM pre-emphasis.
    pub preemphasis: Deemphasis,
    /// The CTCSS tone (Hz, 0 for none).
    pub ctcss: f64,
    /// The AM depth limit (%).
    pub am_depth: f64,
    /// The SSB method.
    pub ssb_method: SsbMethod,
    /// The SSB audio band.
    pub ssb_band: SsbBand,
    /// The SSB compression (dB, 0 for off).
    pub compression: f64,
    /// The push-to-talk time-out (s).
    pub timeout: f64,
    /// The CW speed (WPM).
    pub cw_wpm: f64,
    /// The CW overall speed (WPM, 0 for the speed).
    pub cw_farnsworth: f64,
    /// The tune limit (s).
    pub tune_limit: f64,
}

impl Default for TxSettings {
    fn default() -> Self {
        TxSettings {
            split: TX_SPLIT.default,
            deviation: TX_DEVIATION.default,
            preemphasis: TX_PREEMPHASIS.default,
            ctcss: TX_CTCSS.default,
            am_depth: TX_AM_DEPTH.default,
            ssb_method: TX_SSB_METHOD.default,
            ssb_band: TX_SSB_BAND.default,
            compression: TX_COMPRESSION.default,
            timeout: TX_TIMEOUT.default,
            cw_wpm: TX_CW_WPM.default,
            cw_farnsworth: TX_CW_FARNSWORTH.default,
            tune_limit: TX_TUNE_LIMIT.default,
        }
    }
}

/// The key of each transmit setting, in the order of [`TxSettings::values`].
pub const TX_KEYS: [&str; 12] = [
    TX_SPLIT.field.key,
    TX_DEVIATION.field.key,
    TX_PREEMPHASIS.field.key,
    TX_CTCSS.field.key,
    TX_AM_DEPTH.field.key,
    TX_SSB_METHOD.field.key,
    TX_SSB_BAND.field.key,
    TX_COMPRESSION.field.key,
    TX_TIMEOUT.field.key,
    TX_CW_WPM.field.key,
    TX_CW_FARNSWORTH.field.key,
    TX_TUNE_LIMIT.field.key,
];

impl TxSettings {
    /// Applies the value `v` of the setting with key `key` (one of [`TX_KEYS`]); `id` names the setting in an error. `Ok(false)`
    /// for a key that is not a transmit setting.
    ///
    /// # Errors
    ///
    /// The kind errors of [`Setting::read`].
    pub fn apply(&mut self, key: &str, id: SettingId, v: &Value) -> Result<bool, SettingError> {
        match key {
            k if k == TX_SPLIT.field.key => self.split = TX_SPLIT.read(id, v)?,
            k if k == TX_DEVIATION.field.key => self.deviation = TX_DEVIATION.read(id, v)?,
            k if k == TX_PREEMPHASIS.field.key => self.preemphasis = TX_PREEMPHASIS.read(id, v)?,
            k if k == TX_CTCSS.field.key => self.ctcss = TX_CTCSS.read(id, v)?,
            k if k == TX_AM_DEPTH.field.key => self.am_depth = TX_AM_DEPTH.read(id, v)?,
            k if k == TX_SSB_METHOD.field.key => self.ssb_method = TX_SSB_METHOD.read(id, v)?,
            k if k == TX_SSB_BAND.field.key => self.ssb_band = TX_SSB_BAND.read(id, v)?,
            k if k == TX_COMPRESSION.field.key => self.compression = TX_COMPRESSION.read(id, v)?,
            k if k == TX_TIMEOUT.field.key => self.timeout = TX_TIMEOUT.read(id, v)?,
            k if k == TX_CW_WPM.field.key => self.cw_wpm = TX_CW_WPM.read(id, v)?,
            k if k == TX_CW_FARNSWORTH.field.key => {
                self.cw_farnsworth = TX_CW_FARNSWORTH.read(id, v)?
            }
            k if k == TX_TUNE_LIMIT.field.key => self.tune_limit = TX_TUNE_LIMIT.read(id, v)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// The values, in the order of [`TX_KEYS`].
    pub fn values(&self) -> [Value; 12] {
        use sdrkit_core::Choice;
        [
            Value::Hz(self.split),
            Value::Hz(self.deviation),
            Value::Choice(self.preemphasis.index()),
            Value::Hz(self.ctcss),
            Value::F64(self.am_depth),
            Value::Choice(self.ssb_method.index()),
            Value::Choice(self.ssb_band.index()),
            Value::F64(self.compression),
            Value::Duration(self.timeout),
            Value::F64(self.cw_wpm),
            Value::F64(self.cw_farnsworth),
            Value::Duration(self.tune_limit),
        ]
    }
}

/// What the transmitter needs from the receive side: the demodulator, the filter width and the channel offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Channel {
    /// The demodulator, which is the transmit mode.
    pub demod: Demod,
    /// The width of the receive filter (Hz).
    pub width: f64,
    /// The listening frequency less the dial (Hz).
    pub offset: f64,
}

impl Default for Channel {
    fn default() -> Self {
        Channel {
            demod: Demod::Nfm,
            width: Demod::Nfm.default_width(),
            offset: 0.0,
        }
    }
}
