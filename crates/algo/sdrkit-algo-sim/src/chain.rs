//! The standard generator settings, and the impairment chain built from them.
//!
//! Every stack's generator makes its clean signal; [`Impaired`] runs it through a [`Chain`] configured by the `gen_*` settings of
//! [`GenSetting`], which the framework adds to every stack that has a generator. Each setting has one meaning everywhere.
//!
//! # Order of the stages
//!
//! The chain follows the signal from the transmitter to the receiver's ADC:
//!
//! 1. **level**: the clean signal is scaled to `gen_level_dbfs` (from the power the generator states in [`SignalInfo`]);
//! 2. **spectral inversion** (`gen_inverted`), before the offset, so the signal stays at `gen_offset_hz`;
//! 3. **fading** (`gen_fading`), a tapped delay line whose mean power gain is 1;
//! 4. **carrier offset and drift** (`gen_offset_hz`, `gen_drift_hz_s`);
//! 5. **phase noise** of the receiver's oscillator;
//! 6. **sample-clock error** (`gen_clock_ppm`), which changes the number of samples, and **bit-edge jitter** (`gen_bit_jitter`),
//!    which moves each bit edge of the signal by its own amount;
//! 7. **hum** (amplitude ripple);
//! 8. **noise**: AWGN at the level set against the signal's mean power after step 1 (white at the receiver's sample rate, so it is
//!    added after the clock stage);
//! 9. **impulse noise**;
//! 10. **IQ imbalance**, then **DC offset** (the receiver's mixer);
//! 11. the **ADC quantiser** (`gen_adc_bits`), which also clips at full scale.
//!
//! Each random stage draws from its own stream of `gen_seed`.

use crate::clock::ClockError;
use crate::fading::profiles::{EchoPath, ProfileName, ProfileParams};
use crate::fading::{Profile, Tdl};
use crate::front::{self, Hum, ImpulseNoise, IqImbalance, Quantiser};
use crate::noise::{Awgn, NoiseLevel};
use crate::offset::FrequencyOffset;
use crate::phase_noise::PhaseNoise;
use crate::rng::Rng;
use crate::{Param, SimError};
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Setting, SettingSet, Values};

/// Samples per internal piece (the chain works in pieces of this size, so its buffers are fixed).
const PIECE: usize = 1024;

/// Stream numbers of the random stages (so each draws independently from the one seed).
const STREAM_FADING: u64 = 1;
const STREAM_PHASE: u64 = 2;
const STREAM_NOISE: u64 = 3;
const STREAM_IMPULSE: u64 = 4;
const STREAM_JITTER: u64 = 5;

/// What the chain needs to know about the clean signal: its sample rate, its mean power while it is on, and, for noise stated
/// against them, its bandwidth, symbol rate and information bit rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SignalInfo {
    /// Samples per second.
    pub rate_hz: f64,
    /// Mean power of the clean signal while it transmits (1 for the unit-power signals sdrkit's generators make).
    pub power: f64,
    /// The occupied (or channel) bandwidth an SNR is measured in, Hz.
    pub bandwidth_hz: Option<f64>,
    /// Symbols per second, for Es/N0.
    pub symbol_rate_hz: Option<f64>,
    /// Information bits per second, for Eb/N0.
    pub bit_rate_hz: Option<f64>,
}

impl SignalInfo {
    /// A unit-power signal at `rate_hz`, nothing else stated.
    pub fn new(rate_hz: f64) -> Self {
        SignalInfo {
            rate_hz,
            power: 1.0,
            bandwidth_hz: None,
            symbol_rate_hz: None,
            bit_rate_hz: None,
        }
    }
    /// With this mean power.
    pub fn with_power(mut self, power: f64) -> Self {
        self.power = power;
        self
    }
    /// With this bandwidth, Hz.
    pub fn with_bandwidth(mut self, hz: f64) -> Self {
        self.bandwidth_hz = Some(hz);
        self
    }
    /// With this symbol rate, Hz.
    pub fn with_symbol_rate(mut self, hz: f64) -> Self {
        self.symbol_rate_hz = Some(hz);
        self
    }
    /// With this information bit rate, bits per second.
    pub fn with_bit_rate(mut self, hz: f64) -> Self {
        self.bit_rate_hz = Some(hz);
        self
    }
}

/// Phase noise settings: `dbc_hz` at `offset_hz` on the `1/f^2` slope, with an optional white floor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseNoiseSpec {
    /// The level, dBc/Hz.
    pub dbc_hz: f64,
    /// Where it is measured, Hz from the carrier.
    pub offset_hz: f64,
    /// The white floor, dBc/Hz.
    pub floor_dbc_hz: Option<f64>,
}

/// Hum settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HumSpec {
    /// The ripple frequency, Hz.
    pub freq_hz: f64,
    /// The amplitude-modulation depth, 0 to 1.
    pub depth: f64,
}

/// Impulse noise settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImpulseSpec {
    /// Mean impulses per second.
    pub rate_hz: f64,
    /// Length of each, seconds.
    pub width_s: f64,
    /// Power while it lasts, dBFS.
    pub level_dbfs: f64,
}

/// Everything a [`Chain`] does. The default does nothing: unit gain at 0 dBFS for a unit-power signal, no noise, no channel.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainConfig {
    /// The seed every random stage draws from.
    pub seed: u64,
    /// The signal's mean power after scaling, dBFS.
    pub level_dbfs: f64,
    /// Invert the spectrum.
    pub inverted: bool,
    /// The channel.
    pub fading: Profile,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// Carrier drift, Hz per second.
    pub drift_hz_s: f64,
    /// Phase noise.
    pub phase_noise: Option<PhaseNoiseSpec>,
    /// Sample-clock error, ppm (positive: the receiver's clock is fast).
    pub clock_ppm: f64,
    /// Bit-edge jitter: the most an edge moves, a fraction of the bit (0 to 0.25; the bit rate comes from [`SignalInfo`]).
    pub bit_jitter: f64,
    /// Hum.
    pub hum: Option<HumSpec>,
    /// Additive white Gaussian noise.
    pub noise: NoiseLevel,
    /// Impulse noise.
    pub impulse: Option<ImpulseSpec>,
    /// The Q branch's gain error, dB.
    pub iq_gain_db: f64,
    /// The Q branch's phase error, degrees.
    pub iq_phase_deg: f64,
    /// DC offset, full-scale units.
    pub dc: Complex32,
    /// ADC bits (`None`: no quantiser, no clipping).
    pub adc_bits: Option<u32>,
}

impl Default for ChainConfig {
    fn default() -> Self {
        ChainConfig {
            seed: 1,
            level_dbfs: 0.0,
            inverted: false,
            fading: Profile::None,
            offset_hz: 0.0,
            drift_hz_s: 0.0,
            phase_noise: None,
            clock_ppm: 0.0,
            bit_jitter: 0.0,
            hum: None,
            noise: NoiseLevel::Off,
            impulse: None,
            iq_gain_db: 0.0,
            iq_phase_deg: 0.0,
            dc: Complex32::new(0.0, 0.0),
            adc_bits: None,
        }
    }
}

sdrkit_core::choice_enum! {
    /// How the setting `gen_noise` states the noise level.
    pub enum NoiseKind {
        /// No noise.
        Off = ("off", "Off"),
        /// `gen_snr_db` in the signal's bandwidth.
        Snr = ("snr", "SNR in the signal's bandwidth"),
        /// `gen_cn0_dbhz`.
        Cn0 = ("cn0", "C/N0"),
        /// `gen_esn0_db`.
        EsN0 = ("esn0", "Es/N0"),
        /// `gen_ebn0_db`.
        EbN0 = ("ebn0", "Eb/N0"),
        /// `gen_noise_floor_dbfs`.
        Floor = ("floor", "Fixed floor"),
    }
}

const SECTION: &str = "Test signal";

/// Declares a number setting of the section "Test signal".
macro_rules! number {
    ($(#[$m:meta])* $name:ident: $kind:ty = ($key:literal, $label:literal, $unit:literal, $default:expr, $min:expr, $max:expr, $step:expr, $help:literal)) => {
        $(#[$m])*
        pub const $name: Setting<$kind> = Setting::new($key, $label, $unit, $default)
            .range($min, $max, $step)
            .section(SECTION)
            .help($help);
    };
}

number!(
    /// The seed of every random part.
    GEN_SEED: field::I64 = ("gen_seed", "Seed", "", 1, 0.0, 9_007_199_254_740_991.0, 1.0, "The same seed gives the same signal.")
);
number!(
    /// The signal's mean power, dBFS.
    GEN_LEVEL_DBFS: field::F64 = ("gen_level_dbfs", "Level", "dBFS", 0.0, -150.0, 20.0, 0.0, "The signal's mean power; 0 dBFS is a mean power of 1.")
);
/// How the noise level is given.
pub const GEN_NOISE: Setting<field::Choice<NoiseKind>> =
    Setting::choice("gen_noise", "Noise", NoiseKind::Off)
        .section(SECTION)
        .help("How the noise level is given.");
number!(
    /// The SNR in the signal's bandwidth, dB.
    GEN_SNR_DB: field::F64 = ("gen_snr_db", "SNR", "dB", 20.0, -50.0, 100.0, 0.0, "Signal to noise in the signal's bandwidth.")
);
number!(
    /// The carrier to noise density, dB-Hz.
    GEN_CN0_DBHZ: field::F64 = ("gen_cn0_dbhz", "C/N0", "dB-Hz", 60.0, -20.0, 160.0, 0.0, "Carrier to noise density.")
);
number!(
    /// The energy per symbol to noise density, dB.
    GEN_ESN0_DB: field::F64 = ("gen_esn0_db", "Es/N0", "dB", 10.0, -30.0, 100.0, 0.0, "Energy per symbol to noise density.")
);
number!(
    /// The energy per information bit to noise density, dB.
    GEN_EBN0_DB: field::F64 = ("gen_ebn0_db", "Eb/N0", "dB", 10.0, -30.0, 100.0, 0.0, "Energy per information bit to noise density.")
);
number!(
    /// The noise power over the whole sample rate, dBFS.
    GEN_NOISE_FLOOR_DBFS: field::F64 = ("gen_noise_floor_dbfs", "Noise floor", "dBFS", -60.0, -200.0, 20.0, 0.0, "Noise power over the whole sample rate.")
);
number!(
    /// The carrier frequency offset, Hz.
    GEN_OFFSET_HZ: field::F64 = ("gen_offset_hz", "Offset", "Hz", 0.0, -1e9, 1e9, 0.0, "Carrier frequency offset.")
);
number!(
    /// The change of the offset per second, Hz/s.
    GEN_DRIFT_HZ_S: field::F64 = ("gen_drift_hz_s", "Drift", "Hz/s", 0.0, -1e7, 1e7, 0.0, "Change of the offset per second.")
);
number!(
    /// The sample-clock error, ppm.
    GEN_CLOCK_PPM: field::F64 = ("gen_clock_ppm", "Clock error", "ppm", 0.0, -10_000.0, 10_000.0, 0.0, "The receiver's sample clock runs this fast.")
);
number!(
    /// The bit-edge jitter: the most an edge moves, a fraction of the bit.
    GEN_BIT_JITTER: field::F64 = ("gen_bit_jitter", "Bit-edge jitter", "bit", 0.0, 0.0, 0.25, 0.0, "Each bit edge moves by a random amount up to this share of a bit.")
);
/// The channel profile.
pub const GEN_FADING: Setting<field::Choice<ProfileName>> =
    Setting::choice("gen_fading", "Channel", ProfileName::None)
        .section(SECTION)
        .help("A fading or multipath channel from the standards.");
number!(
    /// The maximum Doppler of moving and flat fading channels, Hz.
    GEN_DOPPLER_HZ: field::F64 = ("gen_doppler_hz", "Doppler", "Hz", 10.0, 0.001, 100_000.0, 0.0, "Maximum Doppler of moving and flat fading channels.")
);
number!(
    /// The Rice factor, dB.
    GEN_RICIAN_K_DB: field::F64 = ("gen_rician_k_db", "Rice factor", "dB", 6.0, -30.0, 40.0, 0.0, "Direct path over scattered power.")
);
number!(
    /// The delay of the steady echo of the profile `echo`, seconds.
    GEN_ECHO_DELAY_S: field::F64 = ("gen_echo_delay_s", "Echo delay", "s", 1e-3, 0.0, 1.0, 0.0, "Delay of the steady echo.")
);
number!(
    /// The level of the steady echo relative to the direct path, dB.
    GEN_ECHO_DB: field::F64 = ("gen_echo_db", "Echo level", "dB", -6.0, -80.0, 20.0, 0.0, "Power of the echo relative to the direct path.")
);
number!(
    /// The carrier phase of the steady echo, degrees.
    GEN_ECHO_PHASE_DEG: field::F64 = ("gen_echo_phase_deg", "Echo phase", "°", 0.0, -180.0, 180.0, 0.0, "Carrier phase of the echo.")
);
/// The steady echoes of the profile `echoes`, as [`EchoPath::parse_list`] reads them.
pub const GEN_ECHOES: Setting<field::Text> = Setting::new("gen_echoes", "Echoes", "", "")
    .section(SECTION)
    .help(
        "The steady echoes of the channel \"echoes\": entries separated by \";\", each the delay in seconds \
         (negative for a pre-echo), the level in dB and the phase in degrees, for example \"2e-6 -8 0; -1e-6 -10 90\".",
    );
/// Oscillator phase noise on or off.
pub const GEN_PHASE_NOISE: Setting<field::Bool> =
    Setting::new("gen_phase_noise", "Phase noise", "", false).section(SECTION);
number!(
    /// The phase noise level on the `1/f^2` slope, dBc/Hz.
    GEN_PHASE_NOISE_DBC_HZ: field::F64 = ("gen_phase_noise_dbc_hz", "Phase noise level", "dBc/Hz", -80.0, -200.0, 0.0, 0.0, "On the 1/f^2 slope.")
);
number!(
    /// The offset where the phase noise level is measured, Hz.
    GEN_PHASE_NOISE_OFFSET_HZ: field::F64 = ("gen_phase_noise_offset_hz", "Phase noise offset", "Hz", 10e3, 1.0, 1e8, 0.0, "Where the level is measured.")
);
/// Spectral inversion on or off.
pub const GEN_INVERTED: Setting<field::Bool> =
    Setting::new("gen_inverted", "Inverted spectrum", "", false).section(SECTION);
number!(
    /// The mains ripple frequency, Hz.
    GEN_HUM_HZ: field::F64 = ("gen_hum_hz", "Hum frequency", "Hz", 50.0, 1.0, 1000.0, 0.0, "Mains ripple frequency.")
);
number!(
    /// The hum depth (0 for no hum).
    GEN_HUM_DEPTH: field::F64 = ("gen_hum_depth", "Hum depth", "", 0.0, 0.0, 1.0, 0.0, "Amplitude ripple; 0 for none.")
);
number!(
    /// The mean impulses per second (0 for no impulse noise).
    GEN_IMPULSE_RATE_HZ: field::F64 = ("gen_impulse_rate_hz", "Impulses", "Hz", 0.0, 0.0, 1e6, 0.0, "Mean impulses per second; 0 for none.")
);
number!(
    /// The length of each impulse, seconds.
    GEN_IMPULSE_WIDTH_S: field::F64 = ("gen_impulse_width_s", "Impulse length", "s", 1e-4, 1e-9, 1.0, 0.0, "Length of each impulse.")
);
number!(
    /// The power of an impulse, dBFS.
    GEN_IMPULSE_DBFS: field::F64 = ("gen_impulse_dbfs", "Impulse level", "dBFS", 0.0, -150.0, 40.0, 0.0, "Power of an impulse.")
);
number!(
    /// The gain of Q relative to I, dB.
    GEN_IQ_GAIN_DB: field::F64 = ("gen_iq_gain_db", "IQ gain error", "dB", 0.0, -6.0, 6.0, 0.0, "Gain of Q relative to I.")
);
number!(
    /// The phase of Q relative to I, degrees.
    GEN_IQ_PHASE_DEG: field::F64 = ("gen_iq_phase_deg", "IQ phase error", "°", 0.0, -45.0, 45.0, 0.0, "Phase of Q relative to I.")
);
number!(
    /// The DC offset added to I, full-scale units.
    GEN_DC_I: field::F64 = ("gen_dc_i", "DC offset I", "", 0.0, -1.0, 1.0, 0.0, "Added to I, full-scale units.")
);
number!(
    /// The DC offset added to Q, full-scale units.
    GEN_DC_Q: field::F64 = ("gen_dc_q", "DC offset Q", "", 0.0, -1.0, 1.0, 0.0, "Added to Q, full-scale units.")
);
number!(
    /// The ADC bits (0 for an ideal receiver with no quantiser).
    GEN_ADC_BITS: field::I64 = ("gen_adc_bits", "ADC bits", "", 0, 0.0, 24.0, 1.0, "Quantise and clip; 0 for an ideal receiver.")
);

sdrkit_core::settings! {
    /// The standard generator settings, section "Test signal". Every key starts with `gen_`. A stack that has a generator adds
    /// [`GenSetting::SETTINGS`](sdrkit_core::SettingSet::SETTINGS) to its own settings.
    pub enum GenSetting {
        /// [`GEN_SEED`].
        Seed = GEN_SEED,
        /// [`GEN_LEVEL_DBFS`].
        LevelDbfs = GEN_LEVEL_DBFS,
        /// [`GEN_NOISE`].
        Noise = GEN_NOISE,
        /// [`GEN_SNR_DB`].
        SnrDb = GEN_SNR_DB,
        /// [`GEN_CN0_DBHZ`].
        Cn0DbHz = GEN_CN0_DBHZ,
        /// [`GEN_ESN0_DB`].
        EsN0Db = GEN_ESN0_DB,
        /// [`GEN_EBN0_DB`].
        EbN0Db = GEN_EBN0_DB,
        /// [`GEN_NOISE_FLOOR_DBFS`].
        NoiseFloorDbfs = GEN_NOISE_FLOOR_DBFS,
        /// [`GEN_OFFSET_HZ`].
        OffsetHz = GEN_OFFSET_HZ,
        /// [`GEN_DRIFT_HZ_S`].
        DriftHzS = GEN_DRIFT_HZ_S,
        /// [`GEN_CLOCK_PPM`].
        ClockPpm = GEN_CLOCK_PPM,
        /// [`GEN_BIT_JITTER`].
        BitJitter = GEN_BIT_JITTER,
        /// [`GEN_FADING`].
        Fading = GEN_FADING,
        /// [`GEN_DOPPLER_HZ`].
        DopplerHz = GEN_DOPPLER_HZ,
        /// [`GEN_RICIAN_K_DB`].
        RicianKDb = GEN_RICIAN_K_DB,
        /// [`GEN_ECHO_DELAY_S`].
        EchoDelayS = GEN_ECHO_DELAY_S,
        /// [`GEN_ECHO_DB`].
        EchoDb = GEN_ECHO_DB,
        /// [`GEN_ECHO_PHASE_DEG`].
        EchoPhaseDeg = GEN_ECHO_PHASE_DEG,
        /// [`GEN_ECHOES`].
        Echoes = GEN_ECHOES,
        /// [`GEN_PHASE_NOISE`].
        PhaseNoise = GEN_PHASE_NOISE,
        /// [`GEN_PHASE_NOISE_DBC_HZ`].
        PhaseNoiseDbcHz = GEN_PHASE_NOISE_DBC_HZ,
        /// [`GEN_PHASE_NOISE_OFFSET_HZ`].
        PhaseNoiseOffsetHz = GEN_PHASE_NOISE_OFFSET_HZ,
        /// [`GEN_INVERTED`].
        Inverted = GEN_INVERTED,
        /// [`GEN_HUM_HZ`].
        HumHz = GEN_HUM_HZ,
        /// [`GEN_HUM_DEPTH`].
        HumDepth = GEN_HUM_DEPTH,
        /// [`GEN_IMPULSE_RATE_HZ`].
        ImpulseRateHz = GEN_IMPULSE_RATE_HZ,
        /// [`GEN_IMPULSE_WIDTH_S`].
        ImpulseWidthS = GEN_IMPULSE_WIDTH_S,
        /// [`GEN_IMPULSE_DBFS`].
        ImpulseDbfs = GEN_IMPULSE_DBFS,
        /// [`GEN_IQ_GAIN_DB`].
        IqGainDb = GEN_IQ_GAIN_DB,
        /// [`GEN_IQ_PHASE_DEG`].
        IqPhaseDeg = GEN_IQ_PHASE_DEG,
        /// [`GEN_DC_I`].
        DcI = GEN_DC_I,
        /// [`GEN_DC_Q`].
        DcQ = GEN_DC_Q,
        /// [`GEN_ADC_BITS`].
        AdcBits = GEN_ADC_BITS,
    }
}

/// Whether `key` is one of the standard generator settings.
pub fn is_generator_key(key: &str) -> bool {
    GenSetting::schema().id(key).is_ok()
}

impl ChainConfig {
    /// The configuration that the generator settings in `values` describe, for a signal described by `sig`. The schema of
    /// `values` must declare the settings of [`GenSetting`]. Other settings are ignored.
    ///
    /// # Errors
    ///
    /// [`SimError::SignalLacks`] for a noise level that the signal cannot support (an Es/N0 for a signal with no symbol rate,
    /// for example), [`SimError::EchoText`] for an echo list that cannot be read, and the errors of [`Profile::echoes`].
    pub fn from_values(values: &Values, sig: &SignalInfo) -> Result<ChainConfig, SimError> {
        let params = ProfileParams {
            max_doppler_hz: values.get(&GEN_DOPPLER_HZ),
            k_db: values.get(&GEN_RICIAN_K_DB),
            los_hz: 0.0,
            echo_delay_s: values.get(&GEN_ECHO_DELAY_S),
            echo_db: values.get(&GEN_ECHO_DB),
            echo_phase_deg: values.get(&GEN_ECHO_PHASE_DEG),
        };
        let need = |what: Option<f64>, param: Param| -> Result<f64, SimError> {
            what.map_or_else(|| param.signal_lacks(), Ok)
        };
        let noise = match values.get(&GEN_NOISE) {
            NoiseKind::Off => NoiseLevel::Off,
            NoiseKind::Snr => NoiseLevel::SnrDb {
                snr_db: values.get(&GEN_SNR_DB),
                bandwidth_hz: need(sig.bandwidth_hz, Param::Bandwidth)?,
            },
            NoiseKind::Cn0 => NoiseLevel::Cn0DbHz(values.get(&GEN_CN0_DBHZ)),
            NoiseKind::EsN0 => NoiseLevel::EsN0Db {
                esn0_db: values.get(&GEN_ESN0_DB),
                symbol_rate_hz: need(sig.symbol_rate_hz, Param::SymbolRate)?,
            },
            NoiseKind::EbN0 => NoiseLevel::EbN0Db {
                ebn0_db: values.get(&GEN_EBN0_DB),
                bit_rate_hz: need(sig.bit_rate_hz, Param::BitRate)?,
            },
            NoiseKind::Floor => NoiseLevel::FloorDbfs(values.get(&GEN_NOISE_FLOOR_DBFS)),
        };
        let fading = match values.get(&GEN_FADING) {
            ProfileName::Echoes => {
                Profile::echoes(&EchoPath::parse_list(values.get(&GEN_ECHOES))?)?
            }
            name => Profile::from_name(name, &params)?,
        };
        let hum_depth = values.get(&GEN_HUM_DEPTH);
        let imp_rate = values.get(&GEN_IMPULSE_RATE_HZ);
        let bits = values.get(&GEN_ADC_BITS) as u32;
        Ok(ChainConfig {
            seed: values.get(&GEN_SEED) as u64,
            level_dbfs: values.get(&GEN_LEVEL_DBFS),
            inverted: values.get(&GEN_INVERTED),
            fading,
            offset_hz: values.get(&GEN_OFFSET_HZ),
            drift_hz_s: values.get(&GEN_DRIFT_HZ_S),
            phase_noise: values.get(&GEN_PHASE_NOISE).then_some(PhaseNoiseSpec {
                dbc_hz: values.get(&GEN_PHASE_NOISE_DBC_HZ),
                offset_hz: values.get(&GEN_PHASE_NOISE_OFFSET_HZ),
                floor_dbc_hz: None,
            }),
            clock_ppm: values.get(&GEN_CLOCK_PPM),
            bit_jitter: values.get(&GEN_BIT_JITTER),
            hum: (hum_depth > 0.0).then_some(HumSpec {
                freq_hz: values.get(&GEN_HUM_HZ),
                depth: hum_depth,
            }),
            noise,
            impulse: (imp_rate > 0.0).then_some(ImpulseSpec {
                rate_hz: imp_rate,
                width_s: values.get(&GEN_IMPULSE_WIDTH_S),
                level_dbfs: values.get(&GEN_IMPULSE_DBFS),
            }),
            iq_gain_db: values.get(&GEN_IQ_GAIN_DB),
            iq_phase_deg: values.get(&GEN_IQ_PHASE_DEG),
            dc: Complex32::new(values.get(&GEN_DC_I) as f32, values.get(&GEN_DC_Q) as f32),
            adc_bits: (bits > 0).then_some(bits),
        })
    }
}

/// The impairment chain (see the module documentation for the order of the stages). Feed it pieces of any size.
#[derive(Clone, Debug)]
pub struct Chain {
    gain: f32,
    inverted: bool,
    fading: Option<Tdl>,
    offset: Option<FrequencyOffset>,
    phase: Option<PhaseNoise>,
    clock: Option<ClockError>,
    hum: Option<Hum>,
    noise: Option<Awgn>,
    impulse: Option<ImpulseNoise>,
    iq: Option<IqImbalance>,
    dc: Complex32,
    adc: Option<Quantiser>,
    piece: Vec<Complex32>,
    clipped: u64,
}

impl Chain {
    /// The chain `cfg` describes for the signal `sig`. Errors for any parameter a stage refuses.
    pub fn new(cfg: &ChainConfig, sig: &SignalInfo) -> Result<Chain, SimError> {
        let rate = sig.rate_hz;
        if !(rate > 0.0 && rate.is_finite()) {
            return Param::Rate.positive(rate);
        }
        if !(sig.power > 0.0 && sig.power.is_finite()) {
            return Param::SignalPower.positive(sig.power);
        }
        if !cfg.level_dbfs.is_finite() {
            return Param::Level.not_finite();
        }
        let seed = cfg.seed;
        let level = 10f64.powf(cfg.level_dbfs / 10.0);
        let stream = |k: u64| Rng::stream(seed, k).next_u64();
        let fading = match cfg.fading {
            Profile::None => None,
            ref p => Some(Tdl::from_profile(p, rate, stream(STREAM_FADING))?),
        };
        let offset = if cfg.offset_hz != 0.0 || cfg.drift_hz_s != 0.0 {
            Some(FrequencyOffset::new(rate, cfg.offset_hz, cfg.drift_hz_s)?)
        } else {
            None
        };
        let phase = match cfg.phase_noise {
            Some(p) => Some(PhaseNoise::from_dbc_hz(
                rate,
                p.dbc_hz,
                p.offset_hz,
                p.floor_dbc_hz,
                stream(STREAM_PHASE),
            )?),
            None => None,
        };
        if !(0.0..=0.25).contains(&cfg.bit_jitter) {
            return Param::JitterPeak.range(0.0, 0.25, cfg.bit_jitter);
        }
        let clock = if cfg.clock_ppm != 0.0 || cfg.bit_jitter > 0.0 {
            let c = ClockError::new(cfg.clock_ppm)?;
            if cfg.bit_jitter > 0.0 {
                let Some(bit_rate) = sig.bit_rate_hz.or(sig.symbol_rate_hz).filter(|r| *r > 0.0)
                else {
                    return Param::BitRate.signal_lacks();
                };
                let period = rate * (1.0 + cfg.clock_ppm * 1e-6) / bit_rate;
                Some(c.with_jitter(period, cfg.bit_jitter * period, stream(STREAM_JITTER))?)
            } else {
                Some(c)
            }
        } else {
            None
        };
        let hum = match cfg.hum {
            Some(h) => Some(Hum::new(rate, h.freq_hz, h.depth)?),
            None => None,
        };
        let noise = match cfg.noise {
            NoiseLevel::Off => None,
            n => Some(Awgn::complex(n, level, rate, stream(STREAM_NOISE))?),
        };
        let impulse = match cfg.impulse {
            Some(i) => Some(ImpulseNoise::new(
                rate,
                i.rate_hz,
                i.width_s,
                i.level_dbfs,
                stream(STREAM_IMPULSE),
            )?),
            None => None,
        };
        let iq = if cfg.iq_gain_db != 0.0 || cfg.iq_phase_deg != 0.0 {
            Some(IqImbalance::new(cfg.iq_gain_db, cfg.iq_phase_deg)?)
        } else {
            None
        };
        if !(cfg.dc.re.is_finite() && cfg.dc.im.is_finite()) {
            return Param::DcOffset.not_finite();
        }
        let adc = match cfg.adc_bits {
            Some(b) => Some(Quantiser::new(b)?),
            None => None,
        };
        Ok(Chain {
            gain: (level / sig.power).sqrt() as f32,
            inverted: cfg.inverted,
            fading,
            offset,
            phase,
            clock,
            hum,
            noise,
            impulse,
            iq,
            dc: cfg.dc,
            adc,
            piece: vec![Complex32::default(); PIECE],
            clipped: 0,
        })
    }

    /// The fixed delay the chain adds, in samples (the fading line's latency plus the clock interpolator's).
    pub fn latency(&self) -> f64 {
        self.fading.as_ref().map_or(0, |f| f.latency()) as f64
            + self.clock.as_ref().map_or(0.0, |c| c.delay())
    }

    /// The noise power added over the whole sample rate (0 without noise).
    pub fn noise_power(&self) -> f64 {
        self.noise.as_ref().map_or(0.0, |n| n.power())
    }

    /// How many I or Q components the ADC has clipped so far.
    pub fn clipped(&self) -> u64 {
        self.clipped
    }

    /// How many outputs the next `n` inputs give (more or fewer than `n` only with a clock error).
    pub fn outputs_for(&self, n: usize) -> usize {
        self.clock.as_ref().map_or(n, |c| c.outputs_for(n))
    }

    /// Runs `input` through the chain and appends the result to `out`. Nothing is allocated once `out` has room for
    /// [`outputs_for`](Self::outputs_for) more samples.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<Complex32>) {
        for piece in input.chunks(PIECE) {
            let buf = &mut self.piece[..piece.len()];
            buf.copy_from_slice(piece);
            if self.gain != 1.0 {
                front::scale(buf, self.gain);
            }
            if self.inverted {
                front::invert(buf);
            }
            if let Some(f) = &mut self.fading {
                f.process(buf);
            }
            if let Some(o) = &mut self.offset {
                o.process(buf);
            }
            if let Some(p) = &mut self.phase {
                p.process(buf);
            }
            let start = out.len();
            match &mut self.clock {
                Some(c) => c.process(buf, out),
                None => out.extend_from_slice(buf),
            }
            let y = &mut out[start..];
            if let Some(h) = &mut self.hum {
                h.process(y);
            }
            if let Some(n) = &mut self.noise {
                n.process(y);
            }
            if let Some(i) = &mut self.impulse {
                i.process(y);
            }
            if let Some(iq) = &self.iq {
                iq.process(y);
            }
            if self.dc != Complex32::new(0.0, 0.0) {
                front::add_dc(y, self.dc);
            }
            if let Some(q) = &self.adc {
                self.clipped += q.process(y) as u64;
            }
        }
    }
}

/// A generator run through a [`Chain`]: itself a [`Generator`], so an engine, a test or a scene uses an impaired generator as it
/// would the clean one.
///
/// ```
/// use sdrkit_algo_sim::chain::{ChainConfig, Impaired, SignalInfo};
/// use sdrkit_algo_sim::noise::NoiseLevel;
/// use sdrkit_core::{Complex32, Generator};
///
/// struct Carrier;
/// impl Generator for Carrier {
///     fn rate_hz(&self) -> f64 { 48_000.0 }
///     fn generate(&mut self, out: &mut [Complex32]) -> usize {
///         out.fill(Complex32::new(1.0, 0.0));
///         out.len()
///     }
/// }
/// let cfg = ChainConfig { noise: NoiseLevel::Cn0DbHz(60.0), clock_ppm: 50.0, ..ChainConfig::default() };
/// let mut g = Impaired::new(Carrier, &cfg, SignalInfo::new(48_000.0)).unwrap();
/// let mut out = vec![Complex32::default(); 4800];
/// assert_eq!(g.generate(&mut out), 4800);
/// ```
pub struct Impaired<G: Generator> {
    gen: G,
    chain: Chain,
    src: Vec<Complex32>,
    pending: Vec<Complex32>,
    read: usize,
    ended: bool,
}

impl<G: Generator> Impaired<G> {
    /// `gen` through the chain `cfg` describes; `sig` describes the clean signal (its rate should be `gen.rate_hz()`).
    pub fn new(gen: G, cfg: &ChainConfig, sig: SignalInfo) -> Result<Self, SimError> {
        if (sig.rate_hz - gen.rate_hz()).abs() > 1e-9 * sig.rate_hz.abs() {
            return Param::StreamRate.conflict(Param::Rate);
        }
        let chain = Chain::new(cfg, &sig)?;
        Ok(Impaired {
            gen,
            chain,
            src: vec![Complex32::default(); PIECE],
            pending: Vec::with_capacity(4 * PIECE),
            read: 0,
            ended: false,
        })
    }

    /// The chain.
    pub fn chain(&self) -> &Chain {
        &self.chain
    }

    /// The clean generator.
    pub fn inner(&self) -> &G {
        &self.gen
    }
}

impl<G: Generator> Generator for Impaired<G> {
    fn rate_hz(&self) -> f64 {
        self.gen.rate_hz()
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut done = 0;
        while done < out.len() {
            if self.read == self.pending.len() {
                self.pending.clear();
                self.read = 0;
                if self.ended {
                    break;
                }
                let k = self.gen.generate(&mut self.src);
                if k < self.src.len() {
                    self.ended = true;
                }
                self.chain.process(&self.src[..k], &mut self.pending);
                continue;
            }
            let k = (self.pending.len() - self.read).min(out.len() - done);
            out[done..done + k].copy_from_slice(&self.pending[self.read..self.read + k]);
            self.read += k;
            done += k;
        }
        done
    }
}

#[cfg(test)]
mod tests;
