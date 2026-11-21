//! The time codes of the stations: encoders that make a minute of symbols from a UTC time, and frame decoders with every check of
//! the codes (fixed bits, parities, ranges, markers).

use crate::calendar::{self as timeutil, civil, Civil};
use sdrkit_util_geo::time::Weekday;

sdrkit_core::choice_enum! {
    /// Why a minute or a burst does not decode.
    #[derive(serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Refusal {
        /// A data second holds a marker or no keying.
        Symbol = ("symbol", "A data second holds another symbol"),
        /// A second was not read.
        NotRead = ("not_read", "A second was not read"),
        /// A marker is missing or in the wrong place.
        MarkerMisplaced = ("marker", "A marker is misplaced"),
        /// A fixed bit is wrong.
        FixedBit = ("fixed_bit", "A fixed bit is wrong"),
        /// The fixed bits are wrong.
        FixedBits = ("fixed_bits", "The fixed bits are wrong"),
        /// Second 59 is keyed.
        Keyed59 = ("keyed_59", "Second 59 is keyed"),
        /// Second 0 is keyed.
        Keyed0 = ("keyed_0", "Second 0 is keyed"),
        /// The two summer-time bits are equal.
        SummerBits = ("summer_bits", "The summer-time bits are equal"),
        /// A parity check failed.
        Parity = ("parity", "Parity error"),
        /// A value is out of its range.
        ValueRange = ("value_range", "A value is out of range"),
        /// The date does not exist.
        DateRange = ("date_range", "The date is out of range"),
        /// The day of the year does not exist.
        DayOfYear = ("day_of_year", "The day of the year is out of range"),
        /// The year is out of range.
        YearRange = ("year_range", "The year is out of range"),
        /// The weekday is out of range.
        WeekdayRange = ("weekday_range", "The weekday is out of range"),
        /// The weekday does not match the date.
        WeekdayMismatch = ("weekday_mismatch", "The weekday does not match the date"),
        /// No minute marker at second 0.
        NoMinuteMarker = ("no_minute_marker", "No minute marker at second 0"),
        /// No minute pulse.
        NoMinutePulse = ("no_minute_pulse", "No minute pulse"),
        /// The DUT1 sign bits are wrong.
        Dut1Sign = ("dut1_sign", "The DUT1 sign bits are wrong"),
        /// DUT1 is doubled on both sides.
        Dut1Doubled = ("dut1_doubled", "DUT1 is doubled on both sides"),
        /// The leap-year bit is wrong.
        LeapYearBit = ("leap_year_bit", "The leap-year bit is wrong"),
        /// The frame index is wrong.
        FrameIndex = ("frame_index", "The frame index is wrong"),
        /// The station is decoded from its data bursts, not from a minute of seconds.
        FromBursts = ("from_bursts", "Decoded from its data bursts"),
        /// No P0 marker.
        NoP0 = ("no_p0", "No P0 marker"),
        /// The two copies of a burst differ.
        CopiesDiffer = ("copies_differ", "The two copies differ"),
        /// The burst is not an A frame.
        NotAFrame = ("not_a_frame", "Not an A frame"),
        /// No S element.
        NoSElement = ("no_s_element", "No S element"),
        /// No minute frame.
        NoMinuteFrame = ("no_minute_frame", "No minute frame"),
    }
}

/// Why a minute or a burst does not decode, with the second where the decoder stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeError {
    /// What failed.
    pub reason: Refusal,
    /// The second of the minute (or the element of a burst) that failed, when one did.
    pub second: Option<usize>,
}

impl DecodeError {
    fn of(reason: Refusal) -> Self {
        DecodeError {
            reason,
            second: None,
        }
    }
    fn at(reason: Refusal, second: usize) -> Self {
        DecodeError {
            reason,
            second: Some(second),
        }
    }
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use sdrkit_core::Choice;
        f.write_str(&Refusal::CHOICES[usize::from(self.reason.index())].label)?;
        match self.second {
            Some(at) => write!(f, " (second {at})"),
            None => Ok(()),
        }
    }
}

impl std::error::Error for DecodeError {}

/// A time-code symbol for one second.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Sym {
    /// 0.
    Zero,
    /// 1.
    One,
    /// 2 (MSF A + 2B, BPC).
    Two,
    /// 3.
    Three,
    /// A marker.
    Marker,
    /// No keying.
    None,
    /// Not read.
    Unread,
    /// RWM: a doubled pulse.
    Double,
}

impl Sym {
    /// The character shown.
    pub fn ch(self) -> char {
        match self {
            Sym::Zero => '0',
            Sym::One => '1',
            Sym::Two => '2',
            Sym::Three => '3',
            Sym::Marker => 'M',
            Sym::None => '-',
            Sym::Unread => '?',
            Sym::Double => 'D',
        }
    }
    /// The value of a data symbol.
    pub fn val(self) -> Option<u32> {
        match self {
            Sym::Zero => Some(0),
            Sym::One => Some(1),
            Sym::Two => Some(2),
            Sym::Three => Some(3),
            _ => None,
        }
    }
    fn of(v: u32) -> Sym {
        match v {
            0 => Sym::Zero,
            1 => Sym::One,
            2 => Sym::Two,
            _ => Sym::Three,
        }
    }
}

sdrkit_core::choice_enum! {
    /// A time station. The key is the setting value of the station.
    pub enum Station {
        /// DCF77 77.5 kHz.
        Dcf77 = ("dcf77", "DCF77"),
        /// MSF 60 kHz.
        Msf = ("msf", "MSF"),
        /// WWVB 60 kHz.
        Wwvb = ("wwvb", "WWVB"),
        /// JJY 40 kHz.
        Jjy40 = ("jjy40", "JJY 40 kHz"),
        /// JJY 60 kHz.
        Jjy60 = ("jjy60", "JJY 60 kHz"),
        /// BPC 68.5 kHz.
        Bpc = ("bpc", "BPC"),
        /// WWV (or WWVH).
        Wwv = ("wwv", "WWV / WWVH"),
        /// CHU.
        Chu = ("chu", "CHU"),
        /// BPM.
        Bpm = ("bpm", "BPM"),
        /// BPM coded minute.
        Bpmc = ("bpmc", "BPM coded"),
        /// HLA.
        Hla = ("hla", "HLA"),
        /// RWM.
        Rwm = ("rwm", "RWM"),
    }
}

/// Fixed facts per station.
pub struct StationInfo {
    /// Setting value.
    pub id: &'static str,
    /// Name.
    pub name: &'static str,
    /// Default frequency, Hz.
    pub freq: f64,
    /// Site.
    pub site: (f64, f64),
    /// Offset of the time carried from UTC, minutes (for the fixed zones).
    pub zone_min: i64,
    /// Low frequency (keyed carrier).
    pub lf: bool,
}

impl Station {
    /// All stations.
    pub const ALL: [Station; 12] = [
        Station::Dcf77,
        Station::Msf,
        Station::Wwvb,
        Station::Jjy40,
        Station::Jjy60,
        Station::Bpc,
        Station::Wwv,
        Station::Chu,
        Station::Bpm,
        Station::Bpmc,
        Station::Hla,
        Station::Rwm,
    ];
    /// Facts.
    pub fn info(self) -> StationInfo {
        let s = |id, name, freq, site, zone_min, lf| StationInfo {
            id,
            name,
            freq,
            site,
            zone_min,
            lf,
        };
        match self {
            Station::Dcf77 => s("dcf77", "DCF77", 77.5e3, (50.016, 9.008), 60, true),
            Station::Msf => s("msf", "MSF", 60e3, (54.911, -3.279), 0, true),
            Station::Wwvb => s("wwvb", "WWVB", 60e3, (40.678, -105.047), 0, true),
            Station::Jjy40 => s("jjy40", "JJY 40 kHz", 40e3, (37.373, 140.849), 540, true),
            Station::Jjy60 => s("jjy60", "JJY 60 kHz", 60e3, (33.465, 130.175), 540, true),
            Station::Bpc => s("bpc", "BPC", 68.5e3, (34.457, 115.837), 480, true),
            Station::Wwv => s("wwv", "WWV", 10e6, (40.678, -105.042), 0, false),
            Station::Chu => s("chu", "CHU", 7.85e6, (45.295, -75.757), 0, false),
            Station::Bpm => s("bpm", "BPM", 5e6, (34.950, 109.543), 0, false),
            Station::Bpmc => s("bpmc", "BPM coded", 5e6, (34.950, 109.543), 0, false),
            Station::Hla => s("hla", "HLA", 5e6, (36.387, 127.366), 540, false),
            Station::Rwm => s("rwm", "RWM", 9.996e6, (55.733, 38.150), 0, false),
        }
    }
    /// From a setting value.
    pub fn from_id(id: &str) -> Option<Station> {
        Station::ALL.iter().copied().find(|s| s.info().id == id)
    }
    /// The station `auto` picks for a dial frequency (`sixty` decides 60 kHz).
    pub fn auto(freq: f64, sixty: Station) -> Option<Station> {
        let k = freq / 1e3;
        let near = |c: f64, w: f64| (k - c).abs() <= w;
        if near(77.5, 1.5) {
            Some(Station::Dcf77)
        } else if near(60.0, 1.5) {
            Some(sixty)
        } else if near(40.0, 1.5) {
            Some(Station::Jjy40)
        } else if near(68.5, 1.5) {
            Some(Station::Bpc)
        } else if [4996.0, 9996.0, 14996.0].iter().any(|&c| near(c, 2.0)) {
            Some(Station::Rwm)
        } else if [2500.0, 5000.0, 10000.0, 15000.0, 20000.0]
            .iter()
            .any(|&c| near(c, 8.0))
        {
            Some(Station::Wwv)
        } else {
            None
        }
    }
    /// Symbols per frame (BPC: 20 s frames read a minute at a time).
    pub fn frame_len(self) -> usize {
        60
    }
}

/// Encode options.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
pub struct EncodeOptions {
    /// Leap second announced; its sign (+1 insert, −1 delete).
    pub leap: i8,
    /// DUT1 in tenths of a second.
    pub dut1: i8,
    /// dUT1 in 0.02 s steps (RWM).
    pub dut1_fine: i8,
    /// Call bit (DCF77).
    pub call: bool,
    /// UT1 flag (BPM).
    pub ut1: bool,
    /// BPMc DUT1 in ms.
    pub dut1_ms: f64,
    /// TAI − UTC.
    pub leap_count: u8,
}

/// A decoded frame.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct TimeFrame {
    /// Year, month, day of the frame's own time zone.
    pub year: i64,
    /// Month.
    pub month: u32,
    /// Day.
    pub day: u32,
    /// Day of year.
    pub yday: u32,
    /// Hour.
    pub hour: u32,
    /// Minute.
    pub minute: u32,
    /// The day of the week (None: not sent).
    pub weekday: Option<Weekday>,
    /// Offset from UTC, minutes.
    pub utc_offset_min: i64,
    /// Summer time (None: not sent).
    pub summer: Option<bool>,
    /// WWVB / WWV DST code.
    pub dst_code: Option<u8>,
    /// Summer-time change announced.
    pub change_announced: bool,
    /// Leap second announced (sign).
    pub leap: i8,
    /// DUT1, s (None: not sent).
    pub dut1: Option<f64>,
    /// dUT1, s.
    pub dut1_fine: Option<f64>,
    /// Call bit.
    pub call: bool,
    /// Leap year flag.
    pub leap_year: Option<bool>,
    /// UT1 flag.
    pub ut1: bool,
    /// UTC of the start of the minute the symbols were received in.
    pub utc: i64,
    /// BPC frame index.
    pub bpc_frame: Option<u8>,
    /// BPMc DUT1 ms and leap count.
    pub dut1_ms: Option<f64>,
    /// Leap count.
    pub leap_count: Option<u8>,
    /// Text.
    pub text: String,
}

/// Bits of a value under BCD weights (each weight a power of two times 1, 10 or 100), most significant first.
fn bcd_bits(v: u32, weights: &[u32]) -> Vec<bool> {
    let h = v / 100;
    let t = (v / 10) % 10;
    let u = v % 10;
    weights
        .iter()
        .map(|&w| {
            if w >= 100 {
                h & (w / 100) != 0
            } else if w >= 10 {
                t & (w / 10) != 0
            } else {
                u & w != 0
            }
        })
        .collect()
}

fn bcd_value(bits: &[bool], weights: &[u32]) -> u32 {
    bits.iter()
        .zip(weights)
        .filter(|(b, _)| **b)
        .map(|(_, w)| *w)
        .sum()
}

fn put(m: &mut [Sym; 60], at: usize, weights: &[u32], v: u32) {
    for (k, b) in bcd_bits(v, weights).into_iter().enumerate() {
        m[at + k] = if b { Sym::One } else { Sym::Zero };
    }
}

fn get(m: &[Sym; 60], at: usize, weights: &[u32]) -> Result<u32, DecodeError> {
    let mut bits = Vec::with_capacity(weights.len());
    for k in 0..weights.len() {
        bits.push(match m[at + k] {
            Sym::One => true,
            Sym::Zero => false,
            _ => return Err(DecodeError::at(Refusal::Symbol, at + k)),
        });
    }
    Ok(bcd_value(&bits, weights))
}

fn bit(m: &[Sym; 60], at: usize) -> Result<bool, DecodeError> {
    match m[at] {
        Sym::One => Ok(true),
        Sym::Zero => Ok(false),
        _ => Err(DecodeError::at(Refusal::Symbol, at)),
    }
}

fn even(m: &[Sym; 60], a: usize, b: usize) -> Result<bool, DecodeError> {
    let mut n = 0;
    for i in a..=b {
        n += bit(m, i)? as u32;
    }
    Ok(n % 2 == 0)
}

const MIN7: [u32; 7] = [1, 2, 4, 8, 10, 20, 40];
const HOUR6: [u32; 6] = [1, 2, 4, 8, 10, 20];

/// The weekday of its ISO 8601 number, Monday 1 to Sunday 7 (a number out of range returns Sunday).
fn weekday_of(iso: u32) -> Weekday {
    use Weekday::*;
    [Monday, Tuesday, Wednesday, Thursday, Friday, Saturday][..]
        .get((iso as usize).wrapping_sub(1))
        .copied()
        .unwrap_or(Sunday)
}

fn check_date(y: i64, mo: u32, d: u32) -> Result<(), DecodeError> {
    if !(1..=12).contains(&mo) || d == 0 || d > timeutil::month_days(y, mo) {
        return Err(DecodeError::of(Refusal::DateRange));
    }
    Ok(())
}

fn text_of(f: &TimeFrame, zone: &str) -> String {
    let local = f.utc + f.utc_offset_min * 60;
    let c = civil(local);
    let u = civil(f.utc);
    if f.utc_offset_min == 0 {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02} UTC",
            u.year, u.month, u.day, u.hour, u.minute
        )
    } else {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02} UTC ({:02}:{:02} {zone})",
            u.year, u.month, u.day, u.hour, u.minute, c.hour, c.minute
        )
    }
}

fn wwvb_dst(utc: i64) -> u8 {
    // 0 standard, 1 DST ends today, 2 begins today, 3 in effect (US rules at 00:00 UTC of the day).
    let day0 = utc - utc.rem_euclid(86400);
    let a = timeutil::us_dst(day0, -7);
    let b = timeutil::us_dst(day0 + 86400 - 1, -7);
    match (a, b) {
        (true, true) => 3,
        (false, true) => 2,
        (true, false) => 1,
        _ => 0,
    }
}

/// Encodes the minute that starts at `utc` (whole minute) for a station.
pub fn encode(st: Station, utc: i64, o: &EncodeOptions) -> [Sym; 60] {
    let mut m = [Sym::Zero; 60];
    match st {
        Station::Dcf77 => {
            // Announces the next minute in CET / CEST.
            let next = utc + 60;
            let summer = timeutil::eu_summer(next);
            let local = next + if summer { 7200 } else { 3600 };
            let c = civil(local);
            m[15] = if o.call { Sym::One } else { Sym::Zero };
            let change = timeutil::eu_summer(next + 3600) != summer;
            m[16] = if change { Sym::One } else { Sym::Zero };
            m[17] = if summer { Sym::One } else { Sym::Zero };
            m[18] = if summer { Sym::Zero } else { Sym::One };
            m[19] = if o.leap != 0 { Sym::One } else { Sym::Zero };
            m[20] = Sym::One;
            put(&mut m, 21, &MIN7, c.minute);
            m[28] = par(&m, 21, 27);
            put(&mut m, 29, &HOUR6, c.hour);
            m[35] = par(&m, 29, 34);
            put(&mut m, 36, &HOUR6, c.day);
            put(&mut m, 42, &[1, 2, 4], c.weekday.iso_number() as u32);
            put(&mut m, 45, &[1, 2, 4, 8, 10], c.month);
            put(
                &mut m,
                50,
                &[1, 2, 4, 8, 10, 20, 40, 80],
                (c.year % 100) as u32,
            );
            m[58] = par(&m, 36, 57);
            m[59] = Sym::None;
        }
        Station::Msf => {
            let next = utc + 60;
            let summer = timeutil::eu_summer(next);
            let local = next + if summer { 3600 } else { 0 };
            let c = civil(local);
            let mut a = [false; 60];
            let mut b = [false; 60];
            let setw = |a: &mut [bool; 60], at: usize, w: &[u32], v: u32| {
                for (k, x) in bcd_bits(v, w).into_iter().enumerate() {
                    a[at + k] = x;
                }
            };
            setw(
                &mut a,
                17,
                &[80, 40, 20, 10, 8, 4, 2, 1],
                (c.year % 100) as u32,
            );
            setw(&mut a, 25, &[10, 8, 4, 2, 1], c.month);
            setw(&mut a, 30, &[20, 10, 8, 4, 2, 1], c.day);
            setw(&mut a, 36, &[4, 2, 1], c.weekday.iso_number() as u32 % 7);
            setw(&mut a, 39, &[20, 10, 8, 4, 2, 1], c.hour);
            setw(&mut a, 45, &[40, 20, 10, 8, 4, 2, 1], c.minute);
            for (i, x) in a.iter_mut().enumerate().take(59).skip(52) {
                *x = i != 52;
            }
            if o.dut1 > 0 {
                for x in b.iter_mut().skip(1).take(o.dut1 as usize) {
                    *x = true;
                }
            } else if o.dut1 < 0 {
                for x in b.iter_mut().skip(9).take((-o.dut1) as usize) {
                    *x = true;
                }
            }
            b[53] = timeutil::eu_summer(next + 3600) != summer;
            let oddp = |a: &[bool; 60], s: usize, e: usize| {
                a[s..=e].iter().filter(|&&x| x).count() % 2 == 0
            };
            b[54] = oddp(&a, 17, 24);
            b[55] = oddp(&a, 25, 35);
            b[56] = oddp(&a, 36, 38);
            b[57] = oddp(&a, 39, 51);
            b[58] = summer;
            for i in 1..60 {
                m[i] = Sym::of(a[i] as u32 + 2 * b[i] as u32);
            }
            m[0] = Sym::Marker;
        }
        Station::Wwvb | Station::Bpm => {
            let c = civil(utc);
            for i in [0, 9, 19, 29, 39, 49, 59] {
                m[i] = Sym::Marker;
            }
            put(&mut m, 1, &[40, 20, 10], c.minute / 10 * 10);
            put(&mut m, 5, &[8, 4, 2, 1], c.minute % 10);
            put(&mut m, 12, &[20, 10], c.hour / 10 * 10);
            put(&mut m, 15, &[8, 4, 2, 1], c.hour % 10);
            put(&mut m, 22, &[200, 100], c.yday / 100 * 100);
            put(&mut m, 25, &[80, 40, 20, 10], c.yday % 100 / 10 * 10);
            put(&mut m, 30, &[8, 4, 2, 1], c.yday % 10);
            let neg = o.dut1 < 0;
            m[36] = if neg { Sym::Zero } else { Sym::One };
            m[37] = if neg { Sym::One } else { Sym::Zero };
            m[38] = if neg { Sym::Zero } else { Sym::One };
            put(&mut m, 40, &[8, 4, 2, 1], o.dut1.unsigned_abs() as u32);
            let y = (c.year % 100) as u32;
            put(&mut m, 45, &[80, 40, 20, 10], y / 10 * 10);
            put(&mut m, 50, &[8, 4, 2, 1], y % 10);
            m[55] = if timeutil::is_leap(c.year) {
                Sym::One
            } else {
                Sym::Zero
            };
            m[56] = if o.leap != 0 { Sym::One } else { Sym::Zero };
            if st == Station::Wwvb {
                let d = wwvb_dst(utc);
                m[57] = if d & 2 != 0 { Sym::One } else { Sym::Zero };
                m[58] = if d & 1 != 0 { Sym::One } else { Sym::Zero };
            } else {
                m[54] = if o.ut1 { Sym::One } else { Sym::Zero };
            }
        }
        Station::Jjy40 | Station::Jjy60 => {
            let c = civil(utc + 9 * 3600);
            for i in [0, 9, 19, 29, 39, 49, 59] {
                m[i] = Sym::Marker;
            }
            put(&mut m, 1, &[40, 20, 10], c.minute / 10 * 10);
            put(&mut m, 5, &[8, 4, 2, 1], c.minute % 10);
            put(&mut m, 12, &[20, 10], c.hour / 10 * 10);
            put(&mut m, 15, &[8, 4, 2, 1], c.hour % 10);
            put(&mut m, 22, &[200, 100], c.yday / 100 * 100);
            put(&mut m, 25, &[80, 40, 20, 10], c.yday % 100 / 10 * 10);
            put(&mut m, 30, &[8, 4, 2, 1], c.yday % 10);
            m[36] = par(&m, 12, 18);
            m[37] = par(&m, 1, 8);
            if c.minute != 15 && c.minute != 45 {
                put(
                    &mut m,
                    41,
                    &[80, 40, 20, 10, 8, 4, 2, 1],
                    (c.year % 100) as u32,
                );
                put(&mut m, 50, &[4, 2, 1], c.weekday.iso_number() as u32 % 7);
                if o.leap != 0 {
                    m[53] = Sym::One;
                    m[54] = if o.leap > 0 { Sym::One } else { Sym::Zero };
                }
            }
        }
        Station::Wwv | Station::Hla => {
            let c = civil(utc + if st == Station::Hla { 9 * 3600 } else { 0 });
            m[0] = Sym::None;
            for i in [9, 19, 29, 39, 49, 59] {
                m[i] = Sym::Marker;
            }
            put(&mut m, 10, &[1, 2, 4, 8], c.minute % 10);
            put(&mut m, 15, &[1, 2, 4], c.minute / 10);
            put(&mut m, 20, &[1, 2, 4, 8], c.hour % 10);
            put(&mut m, 25, &[1, 2], c.hour / 10);
            put(&mut m, 30, &[1, 2, 4, 8], c.yday % 10);
            put(&mut m, 35, &[1, 2, 4, 8], c.yday / 10 % 10);
            put(&mut m, 40, &[1, 2], c.yday / 100);
            if st == Station::Wwv {
                let d = wwvb_dst(utc);
                m[2] = if d & 2 != 0 { Sym::One } else { Sym::Zero };
                m[55] = if d & 1 != 0 { Sym::One } else { Sym::Zero };
                m[3] = if o.leap != 0 { Sym::One } else { Sym::Zero };
                let y = (c.year % 100) as u32;
                put(&mut m, 4, &[1, 2, 4, 8], y % 10);
                put(&mut m, 51, &[1, 2, 4, 8], y / 10);
                m[50] = if o.dut1 >= 0 { Sym::One } else { Sym::Zero };
                put(&mut m, 56, &[1, 2, 4], o.dut1.unsigned_abs() as u32);
            }
        }
        Station::Bpc => {
            let local = utc + 8 * 3600;
            for f in 0..3usize {
                let t = local + 20 * f as i64;
                let c = civil(t);
                let base = 20 * f;
                m[base] = Sym::None; // P0: no drop
                let mut v = [0u32; 20];
                v[1] = f as u32;
                v[2] = 0;
                let h12 = c.hour % 12;
                v[3] = h12 >> 2;
                v[4] = h12 & 3;
                v[5] = c.minute >> 4;
                v[6] = (c.minute >> 2) & 3;
                v[7] = c.minute & 3;
                let wd = c.weekday.iso_number() as u32;
                v[8] = wd >> 2;
                v[9] = wd & 3;
                let p1: u32 = v[1..10].iter().map(|x| x.count_ones()).sum();
                v[10] = ((c.hour >= 12) as u32) << 1 | (p1 % 2);
                v[11] = c.day >> 4;
                v[12] = (c.day >> 2) & 3;
                v[13] = c.day & 3;
                v[14] = c.month >> 2;
                v[15] = c.month & 3;
                let y = (c.year % 100) as u32;
                v[16] = (y >> 4) & 3;
                v[17] = (y >> 2) & 3;
                v[18] = y & 3;
                let p2: u32 = v[11..19].iter().map(|x| x.count_ones()).sum();
                v[19] = ((y >> 6) & 1) << 1 | (p2 % 2);
                for k in 1..20 {
                    m[base + k] = Sym::of(v[k]);
                }
            }
        }
        Station::Rwm => {
            // Minutes 10–19: a 100 ms pulse each second, doubled for DUT1 and dUT1.
            m = [Sym::One; 60];
            m[0] = Sym::Marker;
            if o.dut1 > 0 {
                for x in m.iter_mut().skip(1).take(o.dut1 as usize) {
                    *x = Sym::Double;
                }
            } else if o.dut1 < 0 {
                for x in m.iter_mut().skip(9).take((-o.dut1) as usize) {
                    *x = Sym::Double;
                }
            }
            if o.dut1_fine > 0 {
                for x in m.iter_mut().skip(21).take(o.dut1_fine as usize) {
                    *x = Sym::Double;
                }
            } else if o.dut1_fine < 0 {
                for x in m.iter_mut().skip(31).take((-o.dut1_fine) as usize) {
                    *x = Sym::Double;
                }
            }
        }
        Station::Chu | Station::Bpmc => {
            // Seconds are plain ticks; the time travels in the data bursts (see `chu_frames`, `bpmc_second`).
            m = [Sym::Zero; 60];
            m[0] = Sym::Marker;
        }
    }
    m
}

fn par(m: &[Sym; 60], a: usize, b: usize) -> Sym {
    let n = m[a..=b].iter().filter(|s| **s == Sym::One).count();
    if n % 2 == 1 {
        Sym::One
    } else {
        Sym::Zero
    }
}

fn markers(m: &[Sym; 60], at: &[usize]) -> Result<(), DecodeError> {
    for (i, s) in m.iter().enumerate() {
        let want = at.contains(&i);
        if want != (*s == Sym::Marker) {
            return Err(DecodeError::at(Refusal::MarkerMisplaced, i));
        }
    }
    Ok(())
}

fn zero(m: &[Sym; 60], at: &[usize]) -> Result<(), DecodeError> {
    for &i in at {
        if m[i] != Sym::Zero {
            return Err(DecodeError::at(Refusal::FixedBit, i));
        }
    }
    Ok(())
}

/// Decodes a minute of symbols (second 0 first) for a station. `year_hint` serves the stations that send no year.
pub fn decode(st: Station, m: &[Sym; 60], year_hint: i64) -> Result<TimeFrame, DecodeError> {
    let mut f = TimeFrame::default();
    match st {
        Station::Dcf77 => {
            if m[59] != Sym::None {
                return Err(DecodeError::of(Refusal::Keyed59));
            }
            if bit(m, 0)? || !bit(m, 20)? {
                return Err(DecodeError::of(Refusal::FixedBits));
            }
            let (z1, z2) = (bit(m, 17)?, bit(m, 18)?);
            if z1 == z2 {
                return Err(DecodeError::of(Refusal::SummerBits));
            }
            if !even(m, 21, 28)? || !even(m, 29, 35)? || !even(m, 36, 58)? {
                return Err(DecodeError::of(Refusal::Parity));
            }
            let mi = get(m, 21, &MIN7)?;
            let h = get(m, 29, &HOUR6)?;
            let d = get(m, 36, &HOUR6)?;
            let wd = get(m, 42, &[1, 2, 4])?;
            let mo = get(m, 45, &[1, 2, 4, 8, 10])?;
            let y = 2000 + get(m, 50, &[1, 2, 4, 8, 10, 20, 40, 80])? as i64;
            if mi > 59 || h > 23 || !(1..=7).contains(&wd) {
                return Err(DecodeError::of(Refusal::ValueRange));
            }
            check_date(y, mo, d)?;
            let off = if z1 { 120 } else { 60 };
            let announced = timeutil::unix(y, mo, d, h, mi, 0)
                .ok_or(DecodeError::of(Refusal::DateRange))?
                - off * 60;
            f.utc = announced - 60;
            let c = civil(f.utc + off * 60);
            (f.year, f.month, f.day, f.hour, f.minute, f.yday) =
                (c.year, c.month, c.day, c.hour, c.minute, c.yday);
            f.weekday = Some(c.weekday);
            f.utc_offset_min = off;
            f.summer = Some(z1);
            f.change_announced = bit(m, 16)?;
            f.leap = bit(m, 19)? as i8;
            f.call = bit(m, 15)?;
            f.text = text_of(&f, if z1 { "CEST" } else { "CET" });
        }
        Station::Msf => {
            if m[0] != Sym::Marker {
                return Err(DecodeError::of(Refusal::NoMinuteMarker));
            }
            let mut a = [false; 60];
            let mut b = [false; 60];
            for i in 1..60 {
                let v = m[i].val().ok_or(DecodeError::at(Refusal::NotRead, i))?;
                a[i] = v & 1 == 1;
                b[i] = v & 2 == 2;
            }
            let pat = [false, true, true, true, true, true, true, false];
            if a[52..60] != pat {
                return Err(DecodeError::of(Refusal::FixedBits));
            }
            let odd = |s: usize, e: usize, p: usize| {
                (a[s..=e].iter().filter(|&&x| x).count() + b[p] as usize) % 2 == 1
            };
            if !odd(17, 24, 54) || !odd(25, 35, 55) || !odd(36, 38, 56) || !odd(39, 51, 57) {
                return Err(DecodeError::of(Refusal::Parity));
            }
            let v = |s: usize, w: &[u32]| bcd_value(&a[s..s + w.len()], w);
            let y = 2000 + v(17, &[80, 40, 20, 10, 8, 4, 2, 1]) as i64;
            let mo = v(25, &[10, 8, 4, 2, 1]);
            let d = v(30, &[20, 10, 8, 4, 2, 1]);
            let wd = v(36, &[4, 2, 1]);
            let h = v(39, &[20, 10, 8, 4, 2, 1]);
            let mi = v(45, &[40, 20, 10, 8, 4, 2, 1]);
            if mi > 59 || h > 23 || wd > 6 {
                return Err(DecodeError::of(Refusal::ValueRange));
            }
            check_date(y, mo, d)?;
            let summer = b[58];
            let off = if summer { 60 } else { 0 };
            f.utc = timeutil::unix(y, mo, d, h, mi, 0)
                .ok_or(DecodeError::of(Refusal::DateRange))?
                - off * 60
                - 60;
            let c = civil(f.utc + off * 60);
            (f.year, f.month, f.day, f.hour, f.minute, f.yday) =
                (c.year, c.month, c.day, c.hour, c.minute, c.yday);
            f.weekday = Some(c.weekday);
            f.utc_offset_min = off;
            f.summer = Some(summer);
            f.change_announced = b[53];
            let pos = b[1..9].iter().filter(|&&x| x).count() as f64;
            let neg = b[9..17].iter().filter(|&&x| x).count() as f64;
            f.dut1 = Some((pos - neg) / 10.0);
            f.text = text_of(&f, if summer { "BST" } else { "GMT" });
        }
        Station::Wwvb | Station::Bpm => {
            markers(m, &[0, 9, 19, 29, 39, 49, 59])?;
            let mut fixed = vec![4, 10, 11, 14, 20, 21, 24, 34, 35, 44];
            if st == Station::Wwvb {
                fixed.push(54);
            }
            zero(m, &fixed)?;
            let mi = get(m, 1, &[40, 20, 10])? + get(m, 5, &[8, 4, 2, 1])?;
            let h = get(m, 12, &[20, 10])? + get(m, 15, &[8, 4, 2, 1])?;
            let d = get(m, 22, &[200, 100])?
                + get(m, 25, &[80, 40, 20, 10])?
                + get(m, 30, &[8, 4, 2, 1])?;
            let s = (bit(m, 36)?, bit(m, 37)?, bit(m, 38)?);
            let sign = match s {
                (true, false, true) => 1.0,
                (false, true, false) => -1.0,
                _ => return Err(DecodeError::of(Refusal::Dut1Sign)),
            };
            let mag = get(m, 40, &[8, 4, 2, 1])?;
            let y = 2000 + (get(m, 45, &[80, 40, 20, 10])? + get(m, 50, &[8, 4, 2, 1])?) as i64;
            let ly = bit(m, 55)?;
            if mi > 59 || h > 23 || mag > 9 {
                return Err(DecodeError::of(Refusal::ValueRange));
            }
            if ly != timeutil::is_leap(y) {
                return Err(DecodeError::of(Refusal::LeapYearBit));
            }
            let (mo, dd) = timeutil::from_yday(y, d).ok_or(DecodeError::of(Refusal::DayOfYear))?;
            f.utc =
                timeutil::unix(y, mo, dd, h, mi, 0).ok_or(DecodeError::of(Refusal::DateRange))?;
            (f.year, f.month, f.day, f.hour, f.minute, f.yday) = (y, mo, dd, h, mi, d);
            f.dut1 = Some(sign * mag as f64 / 10.0);
            f.leap_year = Some(ly);
            f.leap = bit(m, 56)? as i8;
            if st == Station::Wwvb {
                f.dst_code = Some(2 * bit(m, 57)? as u8 + bit(m, 58)? as u8);
            } else {
                f.ut1 = bit(m, 54)?;
            }
            f.text = text_of(&f, "UTC");
        }
        Station::Jjy40 | Station::Jjy60 => {
            markers(m, &[0, 9, 19, 29, 39, 49, 59])?;
            zero(m, &[4, 10, 11, 14, 20, 21, 24, 34, 35, 38, 40])?;
            let mi = get(m, 1, &[40, 20, 10])? + get(m, 5, &[8, 4, 2, 1])?;
            let h = get(m, 12, &[20, 10])? + get(m, 15, &[8, 4, 2, 1])?;
            let d = get(m, 22, &[200, 100])?
                + get(m, 25, &[80, 40, 20, 10])?
                + get(m, 30, &[8, 4, 2, 1])?;
            if bit(m, 36)? != (par(m, 12, 18) == Sym::One)
                || bit(m, 37)? != (par(m, 1, 8) == Sym::One)
            {
                return Err(DecodeError::of(Refusal::Parity));
            }
            if mi > 59 || h > 23 {
                return Err(DecodeError::of(Refusal::ValueRange));
            }
            let (y, wd) = if mi == 15 || mi == 45 {
                (year_hint, None)
            } else {
                let y = 2000 + get(m, 41, &[80, 40, 20, 10, 8, 4, 2, 1])? as i64;
                let wd = get(m, 50, &[4, 2, 1])?;
                if wd > 6 {
                    return Err(DecodeError::of(Refusal::WeekdayRange));
                }
                f.leap = match (bit(m, 53)?, bit(m, 54)?) {
                    (true, true) => 1,
                    (true, false) => -1,
                    _ => 0,
                };
                (y, Some(weekday_of(if wd == 0 { 7 } else { wd })))
            };
            let (mo, dd) = timeutil::from_yday(y, d).ok_or(DecodeError::of(Refusal::DayOfYear))?;
            f.utc = timeutil::unix(y, mo, dd, h, mi, 0)
                .ok_or(DecodeError::of(Refusal::DateRange))?
                - 9 * 3600;
            (f.year, f.month, f.day, f.hour, f.minute, f.yday) = (y, mo, dd, h, mi, d);
            f.weekday = wd;
            if let Some(w) = wd {
                if civil(f.utc + 9 * 3600).weekday != w {
                    return Err(DecodeError::of(Refusal::WeekdayMismatch));
                }
            }
            f.utc_offset_min = 540;
            f.text = text_of(&f, "JST");
        }
        Station::Wwv | Station::Hla => {
            if m[0] != Sym::None {
                return Err(DecodeError::of(Refusal::Keyed0));
            }
            for (i, s) in m.iter().enumerate().skip(1) {
                if [9, 19, 29, 39, 49, 59].contains(&i) != (*s == Sym::Marker) {
                    return Err(DecodeError::at(Refusal::MarkerMisplaced, i));
                }
            }
            let hla = st == Station::Hla;
            zero(
                m,
                &[1, 8, 14, 18, 24, 27, 28, 34, 42, 43, 44, 45, 46, 47, 48],
            )?;
            if hla {
                zero(m, &[2, 3, 4, 5, 6, 7, 50, 51, 52, 53, 54, 55, 56, 57, 58])?;
            }
            let mi = get(m, 10, &[1, 2, 4, 8])? + 10 * get(m, 15, &[1, 2, 4])?;
            let h = get(m, 20, &[1, 2, 4, 8])? + 10 * get(m, 25, &[1, 2])?;
            let du = get(m, 30, &[1, 2, 4, 8])?;
            let dt = get(m, 35, &[1, 2, 4, 8])?;
            let d = du + 10 * dt + 100 * get(m, 40, &[1, 2])?;
            if mi > 59 || h > 23 || du > 9 || dt > 9 {
                return Err(DecodeError::of(Refusal::ValueRange));
            }
            let y = if hla {
                year_hint
            } else {
                let yu = get(m, 4, &[1, 2, 4, 8])?;
                let yt = get(m, 51, &[1, 2, 4, 8])?;
                if yu > 9 || yt > 9 {
                    return Err(DecodeError::of(Refusal::YearRange));
                }
                2000 + (yu + 10 * yt) as i64
            };
            let (mo, dd) = timeutil::from_yday(y, d).ok_or(DecodeError::of(Refusal::DayOfYear))?;
            let off = if hla { 540 } else { 0 };
            f.utc = timeutil::unix(y, mo, dd, h, mi, 0)
                .ok_or(DecodeError::of(Refusal::DateRange))?
                - off * 60;
            (f.year, f.month, f.day, f.hour, f.minute, f.yday) = (y, mo, dd, h, mi, d);
            f.utc_offset_min = off;
            if !hla {
                f.dst_code = Some(2 * bit(m, 2)? as u8 + bit(m, 55)? as u8);
                f.leap = bit(m, 3)? as i8;
                let sign = if bit(m, 50)? { 1.0 } else { -1.0 };
                f.dut1 = Some(sign * get(m, 56, &[1, 2, 4])? as f64 / 10.0);
                f.leap_year = Some(timeutil::is_leap(y));
            }
            f.text = text_of(&f, if hla { "KST" } else { "UTC" });
        }
        Station::Bpc => {
            // Decode the first 20 s frame and check the other two agree.
            let part = |k: usize| -> Result<TimeFrame, DecodeError> {
                let fr = decode_bpc(&m[20 * k..20 * k + 20])?;
                if fr.bpc_frame != Some(k as u8) {
                    return Err(DecodeError::of(Refusal::FrameIndex));
                }
                Ok(fr)
            };
            f = part(0)?;
            part(1)?;
            part(2)?;
        }
        Station::Rwm => {
            if m[0] != Sym::Marker {
                return Err(DecodeError::of(Refusal::NoMinutePulse));
            }
            let d =
                |a: usize, b: usize| m[a..=b].iter().filter(|s| **s == Sym::Double).count() as i32;
            let (p, n) = (d(1, 8), d(9, 16));
            let (pf, nf) = (d(21, 28), d(31, 38));
            if (p > 0 && n > 0) || (pf > 0 && nf > 0) {
                return Err(DecodeError::of(Refusal::Dut1Doubled));
            }
            f.dut1 = Some((p - n) as f64 / 10.0);
            f.dut1_fine = Some((pf - nf) as f64 * 0.02);
            f.text = format!(
                "DUT1 {:+.1} s, dUT1 {:+.2} s",
                (p - n) as f64 / 10.0,
                (pf - nf) as f64 * 0.02
            );
        }
        Station::Chu | Station::Bpmc => return Err(DecodeError::of(Refusal::FromBursts)),
    }
    Ok(f)
}

/// Decodes one BPC 20 s frame (P0 first).
pub fn decode_bpc(s: &[Sym]) -> Result<TimeFrame, DecodeError> {
    if s[0] != Sym::None {
        return Err(DecodeError::of(Refusal::NoP0));
    }
    let mut v = [0u32; 20];
    for k in 1..20 {
        v[k] = s[k].val().ok_or(DecodeError::at(Refusal::NotRead, k))?;
    }
    if v[1] > 2 {
        return Err(DecodeError::of(Refusal::FrameIndex));
    }
    let p1: u32 = v[1..10].iter().map(|x| x.count_ones()).sum::<u32>() + (v[10] & 1);
    let p2: u32 = v[11..19].iter().map(|x| x.count_ones()).sum::<u32>() + (v[19] & 1);
    if !p1.is_multiple_of(2) || !p2.is_multiple_of(2) {
        return Err(DecodeError::of(Refusal::Parity));
    }
    let h = (v[3] << 2 | v[4]) + 12 * (v[10] >> 1);
    let mi = v[5] << 4 | v[6] << 2 | v[7];
    let wd = v[8] << 2 | v[9];
    let d = v[11] << 4 | v[12] << 2 | v[13];
    let mo = v[14] << 2 | v[15];
    let y = 2000 + (((v[19] >> 1) << 6) | v[16] << 4 | v[17] << 2 | v[18]) as i64;
    if h > 23 || mi > 59 || !(1..=7).contains(&wd) {
        return Err(DecodeError::of(Refusal::ValueRange));
    }
    check_date(y, mo, d)?;
    let local = timeutil::unix(y, mo, d, h, mi, 0).ok_or(DecodeError::of(Refusal::DateRange))?
        + 20 * v[1] as i64;
    if civil(local).weekday.iso_number() as u32 != wd {
        return Err(DecodeError::of(Refusal::WeekdayMismatch));
    }
    let mut f = TimeFrame {
        utc: local - 8 * 3600 - 20 * v[1] as i64,
        bpc_frame: Some(v[1] as u8),
        weekday: Some(weekday_of(wd)),
        utc_offset_min: 480,
        ..Default::default()
    };
    let c: Civil = civil(f.utc + 480 * 60);
    (f.year, f.month, f.day, f.hour, f.minute, f.yday) =
        (c.year, c.month, c.day, c.hour, c.minute, c.yday);
    f.text = text_of(&f, "CST");
    Ok(f)
}

/// The CHU A frame for a UTC second (seconds 32–39): ten bytes, the BCD digits "6 DDD HH MM SS" twice.
pub fn chu_a_frame(utc: i64) -> [u8; 10] {
    let c = civil(utc);
    let digits = [
        6,
        c.yday / 100,
        c.yday / 10 % 10,
        c.yday % 10,
        c.hour / 10,
        c.hour % 10,
        c.minute / 10,
        c.minute % 10,
        c.second / 10,
        c.second % 10,
    ];
    let mut b = [0u8; 10];
    for i in 0..5 {
        b[i] = (digits[2 * i] as u8) | ((digits[2 * i + 1] as u8) << 4);
        b[i + 5] = b[i];
    }
    b
}

/// Decodes a CHU A frame: (day of year, hour, minute, second); the two copies must agree.
pub fn chu_decode_a(b: &[u8; 10]) -> Result<(u32, u32, u32, u32), DecodeError> {
    if b[..5] != b[5..] {
        return Err(DecodeError::of(Refusal::CopiesDiffer));
    }
    let d: Vec<u32> = (0..10)
        .map(|i| ((b[i / 2] >> (4 * (i % 2))) & 0xF) as u32)
        .collect();
    if d[0] != 6 || d.iter().any(|&x| x > 9) {
        return Err(DecodeError::of(Refusal::NotAFrame));
    }
    let (yd, h, mi, s) = (
        100 * d[1] + 10 * d[2] + d[3],
        10 * d[4] + d[5],
        10 * d[6] + d[7],
        10 * d[8] + d[9],
    );
    if yd == 0 || yd > 366 || h > 23 || mi > 59 || s > 60 {
        return Err(DecodeError::of(Refusal::ValueRange));
    }
    Ok((yd, h, mi, s))
}

/// The CHU B frame (second 31): five data bytes then their complement.
pub fn chu_b_frame(dut1: i8, year: i64) -> [u8; 10] {
    let mut b = [0u8; 10];
    b[0] = (if dut1 < 0 { 0xA } else { 0x0 }) | ((dut1.unsigned_abs() & 0xF) << 4);
    b[1] = (year % 100 / 10 * 16 + year % 10) as u8;
    b[2] = (year / 100 % 100 / 10 * 16 + year / 100 % 10) as u8;
    for i in 0..5 {
        b[5 + i] = !b[i];
    }
    b
}

/// Checks the shape of a CHU B frame.
pub fn chu_check_b(b: &[u8; 10]) -> bool {
    (0..5).all(|i| b[5 + i] == !b[i])
}

/// The BPMc second frame elements (52 of 10 ms; true = a 1 kHz burst): S, empty, second, minute, hour, day, month, year (LSB first,
/// odd parity after each), DUT1 ms (sign bit: UT1 behind UTC).
pub fn bpmc_second(utc: i64, dut1_ms: f64) -> Vec<bool> {
    let c = civil(utc);
    let mut e = vec![true, false];
    let field = |v: u32, n: usize, e: &mut Vec<bool>| {
        let mut ones = 0;
        for k in 0..n {
            let b = (v >> k) & 1 == 1;
            ones += b as u32;
            e.push(b);
        }
        e.push(ones % 2 == 0);
    };
    field(c.second, 6, &mut e);
    field(c.minute, 6, &mut e);
    field(c.hour, 5, &mut e);
    field(c.day, 5, &mut e);
    field(c.month, 4, &mut e);
    field((c.year % 100) as u32, 7, &mut e);
    let ms = dut1_ms.abs().floor() as u32;
    e.push(dut1_ms < 0.0);
    for k in 0..6 {
        e.push((ms >> k) & 1 == 1);
    }
    e.resize(52, false);
    e
}

/// Decodes BPMc second elements: (UTC of the second for the century 2000, DUT1 ms).
pub fn bpmc_decode_second(e: &[bool]) -> Result<(i64, f64), DecodeError> {
    if e.len() < 52 || !e[0] || e[1] {
        return Err(DecodeError::of(Refusal::NoSElement));
    }
    let mut p = 2;
    let mut field = |n: usize| -> Result<u32, DecodeError> {
        let mut v = 0;
        let mut ones = 0;
        for k in 0..n {
            if e[p + k] {
                v |= 1 << k;
                ones += 1;
            }
        }
        let par = e[p + n];
        p += n + 1;
        if (ones + par as u32) % 2 != 1 {
            return Err(DecodeError::of(Refusal::Parity));
        }
        Ok(v)
    };
    let s = field(6)?;
    let mi = field(6)?;
    let h = field(5)?;
    let d = field(5)?;
    let mo = field(4)?;
    let y = 2000 + field(7)? as i64;
    if s > 59 || mi > 59 || h > 23 {
        return Err(DecodeError::of(Refusal::ValueRange));
    }
    check_date(y, mo, d)?;
    let neg = e[p];
    let mut ms = 0;
    for k in 0..6 {
        if e[p + 1 + k] {
            ms |= 1 << k;
        }
    }
    Ok((
        timeutil::unix(y, mo, d, h, mi, s).ok_or(DecodeError::of(Refusal::DateRange))?,
        if neg { -(ms as f64) } else { ms as f64 },
    ))
}

/// The BPMc minute frame elements: 30 ones, two zeros, the 0.1 ms DUT1 digit (4 bits), the leap count (7 bits), the leap flag,
/// each group with an odd parity bit.
pub fn bpmc_minute(dut1_ms: f64, leap_count: u8, leap: bool) -> Vec<bool> {
    let mut e = vec![true; 30];
    e.extend([false, false]);
    let tenth = ((dut1_ms.abs() * 10.0).round() as u32) % 10;
    let field = |v: u32, n: usize, e: &mut Vec<bool>| {
        let mut ones = 0;
        for k in 0..n {
            let b = (v >> k) & 1 == 1;
            ones += b as u32;
            e.push(b);
        }
        e.push(ones % 2 == 0);
    };
    field(tenth, 4, &mut e);
    field(leap_count as u32, 7, &mut e);
    field(leap as u32, 1, &mut e);
    e.resize(52, false);
    e
}

/// Decodes the BPMc minute frame: (tenth of a ms digit, leap count, leap flag).
pub fn bpmc_decode_minute(e: &[bool]) -> Result<(u32, u8, bool), DecodeError> {
    if e.len() < 52 || !e[..30].iter().all(|&x| x) || e[30] || e[31] {
        return Err(DecodeError::of(Refusal::NoMinuteFrame));
    }
    let mut p = 32;
    let mut field = |n: usize| -> Result<u32, DecodeError> {
        let mut v = 0;
        let mut ones = 0;
        for k in 0..n {
            if e[p + k] {
                v |= 1 << k;
                ones += 1;
            }
        }
        let par = e[p + n];
        p += n + 1;
        if (ones + par as u32) % 2 != 1 {
            return Err(DecodeError::of(Refusal::Parity));
        }
        Ok(v)
    };
    let t = field(4)?;
    let l = field(7)?;
    let f = field(1)?;
    Ok((t, l as u8, f == 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_station_over_many_minutes() {
        let o = EncodeOptions {
            dut1: -2,
            ..Default::default()
        };
        let starts = [
            timeutil::unix(2026, 10, 12, 14, 2, 0).unwrap(),
            timeutil::unix(2024, 2, 29, 23, 58, 0).unwrap(),
            timeutil::unix(2025, 12, 31, 23, 50, 0).unwrap(),
            timeutil::unix(2026, 3, 29, 0, 55, 0).unwrap(),
            timeutil::unix(2026, 10, 25, 0, 50, 0).unwrap(),
            timeutil::unix(2026, 3, 8, 9, 0, 0).unwrap(),
        ];
        for st in [
            Station::Dcf77,
            Station::Msf,
            Station::Wwvb,
            Station::Jjy40,
            Station::Jjy60,
            Station::Bpc,
            Station::Wwv,
            Station::Bpm,
            Station::Hla,
        ] {
            for &s0 in &starts {
                for k in 0..20 {
                    let utc = s0 + 60 * k;
                    let m = encode(st, utc, &o);
                    let f = decode(st, &m, civil(utc + 9 * 3600).year)
                        .unwrap_or_else(|e| panic!("{st:?} {}: {e}", timeutil::iso(utc)));
                    assert_eq!(f.utc, utc, "{st:?} {}", timeutil::iso(utc));
                    // A flipped data bit of a parity group or a wrong marker rejects (or at least changes) the frame.
                    let mut bad = m;
                    let i = match st {
                        Station::Dcf77 => 22,
                        Station::Msf => 0,
                        Station::Bpc => 0,
                        _ => 9,
                    };
                    bad[i] = if bad[i] == Sym::Marker || bad[i] == Sym::One {
                        Sym::Zero
                    } else {
                        Sym::One
                    };
                    assert!(
                        decode(st, &bad, civil(utc).year).map(|f| f.utc) != Ok(utc),
                        "{st:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn fields() {
        let utc = timeutil::unix(2026, 10, 12, 14, 2, 0).unwrap();
        let o = EncodeOptions {
            dut1: -2,
            ..Default::default()
        };
        let f = decode(Station::Dcf77, &encode(Station::Dcf77, utc, &o), 2026).unwrap();
        assert_eq!(
            (f.summer, f.utc_offset_min, f.weekday, f.month, f.day),
            (Some(true), 120, Some(Weekday::Monday), 10, 12)
        );
        let f = decode(Station::Msf, &encode(Station::Msf, utc, &o), 2026).unwrap();
        assert_eq!((f.summer, f.dut1), (Some(true), Some(-0.2)));
        let f = decode(Station::Wwvb, &encode(Station::Wwvb, utc, &o), 2026).unwrap();
        assert_eq!(
            (f.dst_code, f.dut1, f.yday, f.leap_year),
            (Some(3), Some(-0.2), 285, Some(false))
        );
        let f = decode(Station::Jjy40, &encode(Station::Jjy40, utc, &o), 2026).unwrap();
        assert_eq!(
            (f.utc_offset_min, f.hour, f.weekday),
            (540, 23, Some(Weekday::Monday))
        );
        let f = decode(Station::Bpc, &encode(Station::Bpc, utc, &o), 2026).unwrap();
        assert_eq!(
            (f.utc_offset_min, f.hour, f.weekday, f.month, f.day),
            (480, 22, Some(Weekday::Monday), 10, 12)
        );
        let f = decode(Station::Hla, &encode(Station::Hla, utc, &o), 2026).unwrap();
        assert_eq!((f.utc_offset_min, f.hour, f.yday), (540, 23, 285));
        let f = decode(Station::Wwv, &encode(Station::Wwv, utc, &o), 2026).unwrap();
        assert_eq!((f.dst_code, f.dut1, f.yday), (Some(3), Some(-0.2), 285));
        let f = decode(
            Station::Bpm,
            &encode(Station::Bpm, utc, &EncodeOptions { ut1: true, ..o }),
            2026,
        )
        .unwrap();
        assert!(f.ut1 && f.hour == 14 && f.yday == 285);
        // BPC 20:25 13 Oct 2026 local; a frame index 3 is refused.
        let b = encode(
            Station::Bpc,
            timeutil::unix(2026, 10, 13, 12, 25, 0).unwrap(),
            &o,
        );
        let f = decode_bpc(&b[0..20]).unwrap();
        assert_eq!(
            (f.hour, f.minute, f.day, f.month, f.year),
            (20, 25, 13, 10, 2026)
        );
        let mut b3 = b;
        b3[1] = Sym::Three;
        assert!(decode_bpc(&b3[0..20]).is_err());
        let mut bp = b;
        bp[5] = if bp[5] == Sym::One {
            Sym::Zero
        } else {
            Sym::One
        };
        assert!(decode_bpc(&bp[0..20]).is_err());
    }

    #[test]
    fn rwm_doubled_pulses() {
        for d in -8..=8 {
            for fine in -4..=4 {
                let m = encode(
                    Station::Rwm,
                    0,
                    &EncodeOptions {
                        dut1: d,
                        dut1_fine: fine,
                        ..Default::default()
                    },
                );
                let f = decode(Station::Rwm, &m, 2026).unwrap();
                assert_eq!(f.dut1, Some(d as f64 / 10.0));
                assert!((f.dut1_fine.unwrap() - fine as f64 * 0.02).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn chu_and_bpmc() {
        let utc = timeutil::unix(2026, 10, 12, 14, 3, 35).unwrap();
        assert_eq!(chu_decode_a(&chu_a_frame(utc)), Ok((285, 14, 3, 35)));
        let mut b = chu_a_frame(utc);
        b[7] ^= 1;
        assert!(chu_decode_a(&b).is_err());
        assert!(chu_check_b(&chu_b_frame(-2, 2026)));
        let e = bpmc_second(timeutil::unix(2026, 10, 12, 7, 38, 12).unwrap(), -35.7);
        assert_eq!(
            bpmc_decode_second(&e),
            Ok((timeutil::unix(2026, 10, 12, 7, 38, 12).unwrap(), -35.0))
        );
        let mut bad = e.clone();
        bad[5] = !bad[5];
        assert!(bpmc_decode_second(&bad).is_err());
        let m = bpmc_minute(-35.7, 37, false);
        assert_eq!(bpmc_decode_minute(&m), Ok((7, 37, false)));
    }
}
