//! Satellite systems, satellite names and the physical constants of the signal standards.
//!
//! Standard: IS-GPS-200, Galileo OS SIS ICD, BDS SIS ICD B1I, GLONASS ICD, RTCA DO-229.

use serde::{Deserialize, Serialize};

/// Speed of light, m/s.
pub const C: f64 = 299_792_458.0;
/// GPS L1 (also QZSS, SBAS, Galileo E1) carrier, Hz.
pub const L1_HZ: f64 = 1_575_420_000.0;
/// BeiDou B1I carrier, Hz.
pub const B1I_HZ: f64 = 1_561_098_000.0;
/// GLONASS L1OF channel 0 carrier, Hz.
pub const GLO_L1_HZ: f64 = 1_602_000_000.0;
/// GLONASS L1OF channel spacing, Hz.
pub const GLO_STEP_HZ: f64 = 562_500.0;
/// The C/A (and B1I) code period, s.
pub const CODE_PERIOD_S: f64 = 1e-3;
/// Seconds in a week.
pub const WEEK_S: f64 = 604_800.0;
/// Pi as IS-GPS-200 fixes it for semicircle conversion.
#[allow(clippy::approx_constant)] // the standard fixes this value, not the exact pi
pub const GPS_PI: f64 = 3.1415926535898;
/// Earth rotation rate (WGS-84, IS-GPS-200), rad/s.
pub const OMEGA_E: f64 = 7.2921151467e-5;
/// Earth rotation rate (CGCS2000), rad/s.
pub const OMEGA_E_BDS: f64 = 7.2921150e-5;
/// Earth rotation rate (PZ-90), rad/s.
pub const OMEGA_E_GLO: f64 = 7.292115e-5;
/// Gravitational constant (GPS), m^3/s^2.
pub const MU_GPS: f64 = 3.986005e14;
/// Gravitational constant (Galileo, BeiDou), m^3/s^2.
pub const MU_GAL: f64 = 3.986004418e14;
/// Gravitational constant (GLONASS, PZ-90), m^3/s^2.
pub const MU_GLO: f64 = 398_600.44e9;
/// Relativistic clock constant F (GPS), s/sqrt(m).
pub const F_GPS: f64 = -4.442807633e-10;
/// Relativistic clock constant F (Galileo, BeiDou), s/sqrt(m).
pub const F_GAL: f64 = -4.442807309e-10;
/// GLONASS earth radius, m.
pub const AE_GLO: f64 = 6_378_136.0;
/// GLONASS second zonal harmonic.
pub const J2_GLO: f64 = 1082.6257e-6;
/// BDT week 0 began in this GPS week.
pub const BDT_WEEK0_IN_GPS: i64 = 1356;
/// BDT is this many seconds behind GPS time.
pub const BDT_GPS_S: f64 = 14.0;
/// Leap seconds assumed until the UTC page says otherwise.
pub const DEFAULT_LEAP_S: i32 = 18;

/// A satellite system. The order is the order of the masks' bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum System {
    /// GPS.
    Gps,
    /// QZSS.
    Qzss,
    /// SBAS.
    Sbas,
    /// Galileo.
    Galileo,
    /// BeiDou.
    Beidou,
    /// GLONASS.
    Glonass,
}

impl System {
    /// Every system, in mask order.
    pub const ALL: [System; 6] = [
        System::Gps,
        System::Qzss,
        System::Sbas,
        System::Galileo,
        System::Beidou,
        System::Glonass,
    ];
    /// The bit of this system in a mask.
    pub fn bit(self) -> u8 {
        1 << (self as u8)
    }
    /// The RINEX 3 letter.
    pub fn letter(self) -> char {
        match self {
            System::Gps => 'G',
            System::Qzss => 'J',
            System::Sbas => 'S',
            System::Galileo => 'E',
            System::Beidou => 'C',
            System::Glonass => 'R',
        }
    }
    /// The name shown on screens.
    pub fn name(self) -> &'static str {
        match self {
            System::Gps => "GPS",
            System::Qzss => "QZSS",
            System::Sbas => "SBAS",
            System::Galileo => "Galileo",
            System::Beidou => "BeiDou",
            System::Glonass => "GLONASS",
        }
    }
    /// The settings key of the system in the `systems` set.
    pub fn key(self) -> &'static str {
        match self {
            System::Gps => "gps",
            System::Qzss => "qzss",
            System::Sbas => "sbas",
            System::Galileo => "galileo",
            System::Beidou => "beidou",
            System::Glonass => "glonass",
        }
    }
    /// The carrier frequency (GLONASS channel 0).
    pub fn carrier_hz(self) -> f64 {
        match self {
            System::Beidou => B1I_HZ,
            System::Glonass => GLO_L1_HZ,
            _ => L1_HZ,
        }
    }
    /// The system a PRN number of the L1 C/A family belongs to.
    pub fn of_l1_prn(prn: u16) -> Option<System> {
        match prn {
            1..=32 => Some(System::Gps),
            120..=158 => Some(System::Sbas),
            193..=202 => Some(System::Qzss),
            _ => None,
        }
    }
}

impl sdrkit_core::Choice for System {
    const CHOICES: &'static [sdrkit_core::ChoiceDecl] = &[
        sdrkit_core::ChoiceDecl::new("gps", "GPS"),
        sdrkit_core::ChoiceDecl::new("qzss", "QZSS"),
        sdrkit_core::ChoiceDecl::new("sbas", "SBAS"),
        sdrkit_core::ChoiceDecl::new("galileo", "Galileo"),
        sdrkit_core::ChoiceDecl::new("beidou", "BeiDou"),
        sdrkit_core::ChoiceDecl::new("glonass", "GLONASS"),
    ];
    fn index(self) -> u16 {
        self as u16
    }
    fn from_index(index: u16) -> Option<Self> {
        System::ALL.get(usize::from(index)).copied()
    }
}

/// A satellite: its system and number. For GLONASS before the slot is known, `prn` is 100 + the frequency channel + 7 (a key only),
/// see [`SatId::glo_channel`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SatId {
    /// The system.
    pub system: System,
    /// The PRN or slot number.
    pub prn: u16,
}

impl SatId {
    /// Satellite `prn` of `system`.
    pub fn new(system: System, prn: u16) -> SatId {
        SatId { system, prn }
    }
    /// A GLONASS satellite known only by its frequency channel `k` (−7 to +6).
    pub fn glo_channel(k: i8) -> SatId {
        SatId {
            system: System::Glonass,
            prn: (100 + 7 + k as i16) as u16,
        }
    }
    /// The frequency channel of a GLONASS key made by [`SatId::glo_channel`].
    pub fn glo_k(&self) -> Option<i8> {
        (self.system == System::Glonass && self.prn >= 100).then(|| (self.prn as i16 - 107) as i8)
    }
    /// The RINEX 3 name (see the `Display` form).
    pub fn name(&self) -> String {
        self.to_string()
    }
}

/// The RINEX 3 name: G05, J01 (PRN 193), S27 (PRN 127), E12, C19, R05. A GLONASS satellite known by its channel is "R k−3".
impl std::fmt::Display for SatId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(k) = self.glo_k() {
            return write!(f, "R k{}", SignedK(k));
        }
        let n = match self.system {
            System::Qzss => self.prn.saturating_sub(192),
            System::Sbas => self.prn.saturating_sub(100),
            _ => self.prn,
        };
        write!(f, "{}{:02}", self.system.letter(), n)
    }
}

/// A GLONASS channel number, shown as "−3", "+2" or "0".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignedK(pub i8);

impl std::fmt::Display for SignedK {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0.cmp(&0) {
            std::cmp::Ordering::Less => write!(f, "\u{2212}{}", -i16::from(self.0)),
            std::cmp::Ordering::Greater => write!(f, "+{}", self.0),
            std::cmp::Ordering::Equal => f.write_str("0"),
        }
    }
}

sdrkit_core::choice_enum! {
    /// The SBAS service that a PRN belongs to.
    pub enum SbasService {
        /// European Geostationary Navigation Overlay Service.
        Egnos = ("egnos", "EGNOS"),
        /// Wide Area Augmentation System.
        Waas = ("waas", "WAAS"),
        /// MTSAT Satellite-based Augmentation System.
        Msas = ("msas", "MSAS"),
        /// GPS Aided GEO Augmented Navigation.
        Gagan = ("gagan", "GAGAN"),
        /// System for Differential Corrections and Monitoring.
        Sdcm = ("sdcm", "SDCM"),
        /// BeiDou Satellite-Based Augmentation System.
        Bdsbas = ("bdsbas", "BDSBAS"),
        /// Korea Augmentation Satellite System.
        Kass = ("kass", "KASS"),
    }
}

/// The SBAS service of a PRN, or `None` if no service lists the PRN.
pub fn sbas_service(prn: u16) -> Option<SbasService> {
    match prn {
        123 | 126 | 136 => Some(SbasService::Egnos),
        131 | 133 | 135 | 138 => Some(SbasService::Waas),
        129 | 137 => Some(SbasService::Msas),
        127 | 128 | 132 => Some(SbasService::Gagan),
        125 | 140 | 141 => Some(SbasService::Sdcm),
        130 => Some(SbasService::Bdsbas),
        134 => Some(SbasService::Kass),
        _ => None,
    }
}

/// The carrier of GLONASS frequency channel `k`.
pub fn glo_carrier(k: i8) -> f64 {
    GLO_L1_HZ + k as f64 * GLO_STEP_HZ
}

/// Whether a BeiDou ranging code number belongs to a geostationary satellite (D2 message).
pub fn bds_is_geo(prn: u16) -> bool {
    prn <= 5 || (59..=63).contains(&prn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_rinex() {
        assert_eq!(SatId::new(System::Gps, 5).name(), "G05");
        assert_eq!(SatId::new(System::Qzss, 193).name(), "J01");
        assert_eq!(SatId::new(System::Sbas, 127).name(), "S27");
        assert_eq!(SatId::glo_channel(-3).name(), "R k\u{2212}3");
        assert_eq!(SatId::glo_channel(-3).glo_k(), Some(-3));
        assert_eq!(System::of_l1_prn(127), Some(System::Sbas));
        assert_eq!(System::of_l1_prn(193), Some(System::Qzss));
        assert_eq!(System::of_l1_prn(5), Some(System::Gps));
        assert_eq!(System::of_l1_prn(170), None);
        assert_eq!(sbas_service(131), Some(SbasService::Waas));
    }
}
