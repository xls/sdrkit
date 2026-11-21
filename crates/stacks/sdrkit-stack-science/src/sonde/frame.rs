//! The decoded radiosonde frame (`sonde.frame`) and what every frame decoder implements.

use sdrkit_graph::Item;
use sdrkit_util_bits::sync::SyncWord;
use sdrkit_util_geo::time::LeapSeconds;

sdrkit_core::choice_enum! {
    /// The sonde families.
    #[derive(PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
    pub enum SondeType {
        /// Vaisala RS41.
        Rs41 = ("rs41", "RS41"),
        /// Vaisala RS92.
        Rs92 = ("rs92", "RS92"),
        /// Graw DFM-06/09/17.
        Dfm = ("dfm", "DFM"),
        /// Meteomodem M10.
        M10 = ("m10", "M10"),
        /// Meteomodem M20.
        M20 = ("m20", "M20"),
        /// Lockheed Martin LMS6-1680.
        Lms6 = ("lms6", "LMS6"),
    }
}

impl SondeType {
    /// The name shown.
    pub fn name(self) -> &'static str {
        match self {
            SondeType::Rs41 => "RS41",
            SondeType::Rs92 => "RS92",
            SondeType::Dfm => "DFM",
            SondeType::M10 => "M10",
            SondeType::M20 => "M20",
            SondeType::Lms6 => "LMS6",
        }
    }
    /// The symbol rate on air.
    pub fn symbol_rate(self) -> f64 {
        match self {
            SondeType::Rs41 | SondeType::Rs92 => 4800.0,
            SondeType::Dfm => 2500.0,
            SondeType::M10 => 9615.0,
            SondeType::M20 => 9600.0,
            SondeType::Lms6 => 9616.0,
        }
    }
}

/// One checked radiosonde frame with its decoded fields. Fields a frame does not carry are `None`.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct SondeFrame {
    /// Signal time of the end of the frame, s.
    pub time: f64,
    /// The family.
    pub kind: Option<SondeType>,
    /// "RS41-SG", "DFM-17" …
    pub subtype: String,
    /// The serial, when known.
    pub serial: Option<String>,
    /// The frame passed its checks.
    pub ok: bool,
    /// Frame counter.
    pub frame: Option<i64>,
    /// UTC from the sonde, Unix seconds.
    pub utc: Option<f64>,
    /// Latitude, degrees.
    pub lat: Option<f64>,
    /// Longitude, degrees.
    pub lon: Option<f64>,
    /// Altitude, m.
    pub alt: Option<f64>,
    /// Vertical speed, m/s.
    pub vel_v: Option<f64>,
    /// Horizontal speed, m/s.
    pub vel_h: Option<f64>,
    /// Heading, degrees.
    pub heading: Option<f64>,
    /// Satellites.
    pub sats: Option<u32>,
    /// GPS time of week, s (sondes that send no date).
    pub gps_tow: Option<f64>,
    /// Temperature, °C.
    pub temp_c: Option<f64>,
    /// Relative humidity, %.
    pub humidity: Option<f64>,
    /// Pressure, hPa.
    pub pressure_hpa: Option<f64>,
    /// Battery, V.
    pub battery_v: Option<f64>,
    /// Burst/kill timer, s.
    pub kill_timer_s: Option<f64>,
    /// Calibration subframes held and needed.
    pub cal: Option<(u32, u32)>,
    /// The frequency the sonde announces, Hz.
    pub announced_hz: Option<f64>,
    /// Byte or codeword corrections made.
    pub corrections: u32,
    /// The receiving channel.
    pub channel: usize,
    /// Offset of the channel from the centre, Hz.
    pub offset_hz: f64,
    /// The channel's carrier level, dB in about 10 kHz.
    pub snr_db: f64,
}

impl Item for SondeFrame {
    const KIND: &'static str = "sonde.frame";
}

/// A frame decoder: fed hard symbols (one bool per symbol, `true` for the higher frequency) with their signal time.
pub trait FrameDecoder: Send {
    /// The family.
    fn kind(&self) -> SondeType;
    /// One symbol.
    fn push(&mut self, sym: bool, t: f64, out: &mut Vec<SondeFrame>);
    /// Forgets half-received frames (`hard`: also the slower state such as configuration cycles).
    fn reset(&mut self, hard: bool);
    /// The leap-second table that turns GPS time into UTC. The default ignores it (the family sends UTC or no time).
    fn set_leap_seconds(&mut self, leap: &LeapSeconds) {
        let _ = leap;
    }
}

/// The sync matcher of a pattern (newest bit in bit 0, as [`pattern_str`] returns it) with at most `max_err` differing bits,
/// in both polarities.
pub fn sync_word((pat, len): (u64, u32), max_err: u32) -> SyncWord {
    SyncWord::new(pat as u128, len, max_err)
        .expect("a sync pattern of 1 to 64 bits")
        .with_inverted(true)
}

/// Bits (LSB first per byte) of `bytes` as a pattern, newest (last sent) in bit 0.
pub fn pattern_lsb_first(bytes: &[u8]) -> (u64, u32) {
    let mut p = 0u64;
    let mut n = 0;
    for &b in bytes {
        for k in 0..8 {
            p = (p << 1) | ((b >> k) & 1) as u64;
            n += 1;
        }
    }
    (p, n)
}

/// Bits of a string of '0' and '1' as a pattern, newest in bit 0.
pub fn pattern_str(s: &str) -> (u64, u32) {
    let mut p = 0u64;
    let mut n = 0;
    for c in s.bytes() {
        p = (p << 1) | (c == b'1') as u64;
        n += 1;
    }
    (p, n)
}

/// Heading in degrees 0–360 from east and north speeds.
pub fn heading(e: f64, n: f64) -> f64 {
    (e.atan2(n).to_degrees() + 360.0) % 360.0
}

/// Steinhart–Hart: temperature in °C from a resistance and the coefficients p0…p3.
pub fn steinhart_c(r: f64, p: [f64; 4]) -> f64 {
    let l = r.ln();
    1.0 / (p[0] + p[1] * l + p[2] * l * l + p[3] * l * l * l) - 273.15
}

/// The resistance that corresponds to `t_c` under Steinhart–Hart (Newton on ln R).
pub fn steinhart_inverse(t_c: f64, p: [f64; 4]) -> f64 {
    let target = 1.0 / (t_c + 273.15);
    let mut l = 9.0f64;
    for _ in 0..60 {
        let f = p[0] + p[1] * l + p[2] * l * l + p[3] * l * l * l - target;
        let d = p[1] + 2.0 * p[2] * l + 3.0 * p[3] * l * l;
        l -= f / d;
    }
    l.exp()
}
