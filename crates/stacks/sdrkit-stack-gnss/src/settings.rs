//! The settings of the `gnss` and `sbas` stacks and of their receiver component, and the test signal's options.

use crate::core::CoreConfig;
use crate::gen::GenOptions;
use crate::system::System;
use sdrkit_core::publish::field;
use sdrkit_core::{Setting, Values};

sdrkit_core::choice_enum! {
    /// The band that the radio is tuned to.
    pub enum Tune {
        /// GPS L1 and Galileo E1 at 1575.42 MHz.
        L1 = ("l1", "L1 / E1 1575.42 MHz"),
        /// BeiDou B1I at 1561.098 MHz.
        B1i = ("b1i", "BeiDou B1I 1561.098 MHz"),
        /// GLONASS L1 at 1602 MHz.
        Glo = ("glo", "GLONASS L1 1602 MHz"),
        /// Every system at 1582 MHz.
        All = ("all", "All systems 1582 MHz"),
        /// A frequency and rate that the user sets.
        Custom = ("custom", "Custom"),
    }
}

impl Tune {
    /// The centre frequency and the sample rate of the tune, in Hz.
    pub fn centre_and_rate(self) -> (f64, f64) {
        match self {
            Tune::L1 | Tune::Custom => (1575.42e6, 4e6),
            Tune::B1i => (1561.098e6, 4e6),
            Tune::Glo => (1602e6, 10e6),
            Tune::All => (1582e6, 50e6),
        }
    }

    /// The tune whose centre is within 1 kHz of `centre_hz`, else [`Tune::Custom`].
    pub fn of_centre(centre_hz: f64) -> Tune {
        [Tune::L1, Tune::B1i, Tune::Glo, Tune::All]
            .into_iter()
            .find(|t| (t.centre_and_rate().0 - centre_hz).abs() <= 1e3)
            .unwrap_or(Tune::Custom)
    }
}

const SEARCH_HELP: &str = "The first frequency window while nothing is found.";

/// The band that the radio is tuned to. The host tunes the radio, so no component reads it.
pub const TUNE: Setting<field::Choice<Tune>> = Setting::choice("tune", "Tune", Tune::L1)
    .help("Which band the radio is tuned to. A tune that needs another sample rate restarts the radio.")
    .section("Signals")
    .restart();

macro_rules! system_setting {
    ($name:ident, $key:literal, $label:literal) => {
        #[doc = concat!("Search and track the ", $label, " satellites.")]
        pub const $name: Setting<field::Bool> = Setting::new($key, $label, "", true)
            .help("Search and track this system's satellites.")
            .section("Signals");
    };
}
system_setting!(SYS_GPS, "sys_gps", "GPS");
system_setting!(SYS_QZSS, "sys_qzss", "QZSS");
system_setting!(SYS_SBAS, "sys_sbas", "SBAS");
system_setting!(SYS_GALILEO, "sys_galileo", "Galileo");
system_setting!(SYS_BEIDOU, "sys_beidou", "BeiDou");
system_setting!(SYS_GLONASS, "sys_glonass", "GLONASS");

/// Satellites lower than this are tracked but left out of the position.
pub const ELEVATION_MASK: Setting<field::F64> =
    Setting::new("elevation_mask", "Elevation mask", "°", 5.0)
        .range(0.0, 30.0, 1.0)
        .help("Satellites lower than this are tracked but left out of the position.")
        .section("Position");
/// The half width of the first frequency window of the search.
pub const SEARCH_HALF_WIDTH: Setting<field::Hz> =
    Setting::new("search_half_width", "Search half width", "Hz", 10_000.0)
        .range(1000.0, 200_000.0, 500.0)
        .help(SEARCH_HELP)
        .section("Search");
/// The remembered frequency error of the radio at 1575.42 MHz.
pub const FREQUENCY_HINT: Setting<field::Hz> =
    Setting::new("frequency_hint", "Radio frequency error", "Hz", 0.0)
        .range(-200_000.0, 200_000.0, 1.0)
        .help("The remembered frequency error of this radio, as seen at 1575.42 MHz.")
        .section("Search");
/// Whether the search starts at the remembered frequency error.
pub const FREQUENCY_HINT_VALID: Setting<field::Bool> = Setting::new(
    "frequency_hint_valid",
    "Use the remembered error",
    "",
    false,
)
.section("Search");
/// Search work per millisecond of signal.
pub const ACQUISITION_EFFORT: Setting<field::F64> =
    Setting::new("acquisition_effort", "Acquisition effort", "FFT/ms", 10.0)
        .range(1.0, 100.0, 1.0)
        .help("Search work per millisecond of signal.")
        .section("Search");
/// The GPS week that resolves the broadcast weeks. 0 takes it from the computer's clock.
pub const KNOWN_WEEK: Setting<field::I64> = Setting::new("known_week", "Known week", "", 0)
    .range(0.0, 9999.0, 1.0)
    .help("The GPS week used to resolve broadcast weeks. 0 takes it from the computer's clock.")
    .section("Time");
/// Compare the computer's clock with GPS time.
pub const CLOCK_COMPARE: Setting<field::Bool> =
    Setting::new("clock_compare", "Compare the computer clock", "", false)
        .help("On for a live radio: the computer's clock against GPS time.")
        .section("Time");
/// The file of the Galileo E1-B memory codes.
pub const GALILEO_CODES: Setting<field::Text> =
    Setting::new("galileo_codes", "Galileo E1-B code table", "", "")
        .help("A text file with the E1-B memory codes of the Galileo OS SIS ICD, Annex C (one hexadecimal code a line).")
        .section("Signals");

macro_rules! gen_system_setting {
    ($name:ident, $key:literal, $label:literal) => {
        #[doc = concat!("The test signal holds the ", $label, " satellites.")]
        pub const $name: Setting<field::Bool> =
            Setting::new($key, $label, "", true).section("Test signal");
    };
}
gen_system_setting!(GEN_GPS, "gen_gps", "GPS");
gen_system_setting!(GEN_QZSS, "gen_qzss", "QZSS");
gen_system_setting!(GEN_SBAS, "gen_sbas", "SBAS");
gen_system_setting!(GEN_GALILEO, "gen_galileo", "Galileo");
gen_system_setting!(GEN_BEIDOU, "gen_beidou", "BeiDou");
gen_system_setting!(GEN_GLONASS, "gen_glonass", "GLONASS");

/// The number of satellites of the test signal (0: all).
pub const GEN_SATS: Setting<field::I64> = Setting::new("gen_sats", "Satellites (0: all)", "", 0)
    .range(0.0, 12.0, 1.0)
    .section("Test signal")
    .advanced();
/// The test signal starts cold: no almanac and no time.
pub const GEN_COLD: Setting<field::Bool> = Setting::new("gen_cold", "Cold start", "", false)
    .section("Test signal")
    .advanced();

/// The system settings, in the order of [`System::ALL`].
const SYSTEMS: [(System, Setting<field::Bool>); 6] = [
    (System::Gps, SYS_GPS),
    (System::Qzss, SYS_QZSS),
    (System::Sbas, SYS_SBAS),
    (System::Galileo, SYS_GALILEO),
    (System::Beidou, SYS_BEIDOU),
    (System::Glonass, SYS_GLONASS),
];

/// The test signal's system settings, in the order of [`System::ALL`].
const GEN_SYSTEMS: [(System, Setting<field::Bool>); 6] = [
    (System::Gps, GEN_GPS),
    (System::Qzss, GEN_QZSS),
    (System::Sbas, GEN_SBAS),
    (System::Galileo, GEN_GALILEO),
    (System::Beidou, GEN_BEIDOU),
    (System::Glonass, GEN_GLONASS),
];

sdrkit_core::settings! {
    /// The settings of the `gnss` stack.
    pub enum GnssSetting {
        /// Tune.
        Tune = TUNE,
        /// GPS.
        SysGps = SYS_GPS,
        /// QZSS.
        SysQzss = SYS_QZSS,
        /// SBAS.
        SysSbas = SYS_SBAS,
        /// Galileo.
        SysGalileo = SYS_GALILEO,
        /// BeiDou.
        SysBeidou = SYS_BEIDOU,
        /// GLONASS.
        SysGlonass = SYS_GLONASS,
        /// Elevation mask.
        ElevationMask = ELEVATION_MASK,
        /// Search half width.
        SearchHalfWidth = SEARCH_HALF_WIDTH,
        /// Radio frequency error.
        FrequencyHint = FREQUENCY_HINT,
        /// Use the remembered error.
        FrequencyHintValid = FREQUENCY_HINT_VALID,
        /// Acquisition effort.
        AcquisitionEffort = ACQUISITION_EFFORT,
        /// Known week.
        KnownWeek = KNOWN_WEEK,
        /// Compare the computer clock.
        ClockCompare = CLOCK_COMPARE,
        /// Galileo E1-B code table.
        GalileoCodes = GALILEO_CODES,
        /// Test signal: GPS.
        GenGps = GEN_GPS,
        /// Test signal: QZSS.
        GenQzss = GEN_QZSS,
        /// Test signal: SBAS.
        GenSbas = GEN_SBAS,
        /// Test signal: Galileo.
        GenGalileo = GEN_GALILEO,
        /// Test signal: BeiDou.
        GenBeidou = GEN_BEIDOU,
        /// Test signal: GLONASS.
        GenGlonass = GEN_GLONASS,
        /// Test signal: satellites.
        GenSats = GEN_SATS,
        /// Test signal: cold start.
        GenCold = GEN_COLD,
    }
}

sdrkit_core::settings! {
    /// The settings of the `sbas` stack.
    pub enum SbasSetting {
        /// Search half width.
        SearchHalfWidth = SEARCH_HALF_WIDTH,
        /// Radio frequency error.
        FrequencyHint = FREQUENCY_HINT,
        /// Use the remembered error.
        FrequencyHintValid = FREQUENCY_HINT_VALID,
        /// Acquisition effort.
        AcquisitionEffort = ACQUISITION_EFFORT,
    }
}

sdrkit_core::settings! {
    /// The settings of the `gnss.receiver` component.
    pub enum ReceiverSetting {
        /// GPS.
        SysGps = SYS_GPS,
        /// QZSS.
        SysQzss = SYS_QZSS,
        /// SBAS.
        SysSbas = SYS_SBAS,
        /// Galileo.
        SysGalileo = SYS_GALILEO,
        /// BeiDou.
        SysBeidou = SYS_BEIDOU,
        /// GLONASS.
        SysGlonass = SYS_GLONASS,
        /// Elevation mask.
        ElevationMask = ELEVATION_MASK,
        /// Search half width.
        SearchHalfWidth = SEARCH_HALF_WIDTH,
        /// Radio frequency error.
        FrequencyHint = FREQUENCY_HINT,
        /// Use the remembered error.
        FrequencyHintValid = FREQUENCY_HINT_VALID,
        /// Acquisition effort.
        AcquisitionEffort = ACQUISITION_EFFORT,
        /// Known week.
        KnownWeek = KNOWN_WEEK,
        /// Compare the computer clock.
        ClockCompare = CLOCK_COMPARE,
        /// Galileo E1-B code table.
        GalileoCodes = GALILEO_CODES,
    }
}

sdrkit_core::settings! {
    /// The settings of the `gnss.sbas_receiver` component.
    pub enum SbasReceiverSetting {
        /// Search half width.
        SearchHalfWidth = SEARCH_HALF_WIDTH,
        /// Radio frequency error.
        FrequencyHint = FREQUENCY_HINT,
        /// Use the remembered error.
        FrequencyHintValid = FREQUENCY_HINT_VALID,
        /// Acquisition effort.
        AcquisitionEffort = ACQUISITION_EFFORT,
    }
}

impl SbasReceiverSetting {
    /// The same setting of the `gnss.receiver` component.
    pub fn receiver(self) -> ReceiverSetting {
        match self {
            SbasReceiverSetting::SearchHalfWidth => ReceiverSetting::SearchHalfWidth,
            SbasReceiverSetting::FrequencyHint => ReceiverSetting::FrequencyHint,
            SbasReceiverSetting::FrequencyHintValid => ReceiverSetting::FrequencyHintValid,
            SbasReceiverSetting::AcquisitionEffort => ReceiverSetting::AcquisitionEffort,
        }
    }
}

/// The GPS week from the computer's clock.
fn week_from_clock() -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    crate::geo::week_from_clock(now, &sdrkit_util_geo::time::LeapSeconds::BUILTIN)
}

/// The settings of the receiver as plain values. The receiver component keeps them and makes its [`CoreConfig`] from them.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceiverValues {
    /// The systems to search and track, one bit per system of [`System::ALL`].
    pub systems: u8,
    /// Elevation mask, degrees.
    pub elevation_mask: f64,
    /// Search half width, Hz.
    pub search_half_width: f64,
    /// The remembered frequency error, Hz.
    pub frequency_hint: f64,
    /// Whether the search starts at the remembered error.
    pub frequency_hint_valid: bool,
    /// Search work per millisecond.
    pub effort: f64,
    /// The known GPS week (0: from the computer's clock).
    pub known_week: i64,
    /// Whether the computer's clock is compared with GPS time.
    pub clock_compare: bool,
    /// The file of the Galileo codes (empty: none).
    pub galileo_codes: String,
}

impl ReceiverValues {
    /// The defaults of the `gnss` receiver, or with `sbas` of the `sbas` receiver (SBAS alone).
    pub fn defaults(sbas: bool) -> Self {
        ReceiverValues {
            systems: if sbas {
                System::Sbas.bit()
            } else {
                System::ALL.iter().fold(0, |m, s| m | s.bit())
            },
            elevation_mask: ELEVATION_MASK.default,
            search_half_width: SEARCH_HALF_WIDTH.default,
            frequency_hint: FREQUENCY_HINT.default,
            frequency_hint_valid: FREQUENCY_HINT_VALID.default,
            effort: ACQUISITION_EFFORT.default,
            known_week: KNOWN_WEEK.default,
            clock_compare: CLOCK_COMPARE.default,
            galileo_codes: String::new(),
        }
    }

    /// The system setting of `s`.
    pub fn system_setting(s: System) -> ReceiverSetting {
        match s {
            System::Gps => ReceiverSetting::SysGps,
            System::Qzss => ReceiverSetting::SysQzss,
            System::Sbas => ReceiverSetting::SysSbas,
            System::Galileo => ReceiverSetting::SysGalileo,
            System::Beidou => ReceiverSetting::SysBeidou,
            System::Glonass => ReceiverSetting::SysGlonass,
        }
    }

    /// The receiver configuration of these values. With `sbas`, SBAS alone with the widening search plan. The Galileo table is
    /// `galileo`, read from [`ReceiverValues::galileo_codes`] by the caller.
    pub fn core_config(&self, sbas: bool, galileo: Option<crate::codes::E1bTable>) -> CoreConfig {
        let mut c = CoreConfig {
            systems: self.systems,
            elevation_mask: self.elevation_mask,
            search_half_width: self.search_half_width,
            freq_hint: self.frequency_hint_valid.then_some(self.frequency_hint),
            effort: self.effort,
            clock_compare: self.clock_compare,
            galileo,
            sbas_mode: sbas,
            ..CoreConfig::default()
        };
        if sbas {
            c.systems = System::Sbas.bit();
            c.week_ref = 2400;
        } else {
            c.week_ref = if self.known_week > 0 {
                self.known_week
            } else {
                week_from_clock()
            };
        }
        c
    }
}

/// The generator options of the settings `v` at `rate`. With `sbas`, the three SBAS satellites with message content. Else the
/// systems and satellites that the `gen_*` settings of `gnss` choose.
pub fn gen_options(
    v: &Values,
    rate: f64,
    seed: u64,
    sbas: bool,
    centre: Option<f64>,
) -> GenOptions {
    let mut o = GenOptions {
        rate_hz: rate,
        seed,
        ..Default::default()
    };
    if let Some(c) = centre {
        o.centre_hz = c;
    }
    if sbas {
        o.systems = System::Sbas.bit();
        o.sbas_structured = true;
    } else {
        o.systems = GEN_SYSTEMS
            .iter()
            .filter(|(_, s)| v.get(s))
            .fold(0, |m, (sys, _)| m | sys.bit());
        let n = v.get(&GEN_SATS);
        o.max_sats = usize::try_from(n).ok().filter(|&n| n > 0);
        o.cold = v.get(&GEN_COLD);
    }
    o
}

/// The receiver values of the `gnss` stack settings `v`.
pub fn receiver_values(v: &Values) -> ReceiverValues {
    ReceiverValues {
        systems: SYSTEMS
            .iter()
            .filter(|(_, s)| v.get(s))
            .fold(0, |m, (sys, _)| m | sys.bit()),
        elevation_mask: v.get(&ELEVATION_MASK),
        search_half_width: v.get(&SEARCH_HALF_WIDTH),
        frequency_hint: v.get(&FREQUENCY_HINT),
        frequency_hint_valid: v.get(&FREQUENCY_HINT_VALID),
        effort: v.get(&ACQUISITION_EFFORT),
        known_week: v.get(&KNOWN_WEEK),
        clock_compare: v.get(&CLOCK_COMPARE),
        galileo_codes: v.get(&GALILEO_CODES).to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::SettingSet;

    #[test]
    fn defaults_make_a_configuration_and_tunes_are_recognised() {
        let v = receiver_values(&Values::defaults(&GnssSetting::schema()));
        let c = v.core_config(false, None);
        assert_eq!(c.systems, 0x3F);
        assert_eq!(c.elevation_mask, 5.0);
        assert_eq!(c.effort, 10.0);
        assert!(c.freq_hint.is_none());
        let s = ReceiverValues::defaults(true).core_config(true, None);
        assert_eq!(s.systems, System::Sbas.bit());
        assert_eq!(Tune::of_centre(1561.0985e6), Tune::B1i);
        assert_eq!(Tune::of_centre(1590.4e6), Tune::Custom);
        assert!(SbasSetting::schema().id("tune").is_err());
    }
}
