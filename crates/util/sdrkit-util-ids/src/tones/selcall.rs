//! Sequential tone sets ("selcall", five-tone and six-tone calling): the tone of each digit 0 to 9, the repeat tone sent in place
//! of a digit equal to the one before, the group tone where one is standard, and the usual tone length.
//!
//! Sources, two published manufacturer tables, which agree on everything kept here except where noted:
//! - Midian Electronics, "Tone Signaling Charts", "Five, six, seven tone sequential single frequency codes & timings" (all sets).
//! - WAVECOM decoder documentation (W-CODE / W74PC online help), one page per system: CCIR, ZVEI-1, ZVEI-2, ZVEI-3, DZVEI, PZVEI,
//!   EEA, EIA, PCCIR, NATEL, EURO, MODAT.
//!
//! Where they differ: WAVECOM lists ZVEI2 with the ZVEI3 digit tones (2200 Hz for 0) and a 2400 Hz repeat; Midian lists ZVEI2 with
//! the ZVEI1 digits and a 970 Hz repeat, which is the set radios call ZVEI2, and is kept. EURO digit 3 is 767.4 Hz (Midian) against 764.4
//! Hz (WAVECOM); the EURO tones fall in a geometric series (ratio 0.9218), which 767.4 fits. MODAT digit 9 is 1987.5 Hz (Midian)
//! against 1937.5 Hz (WAVECOM); the MODAT tones are 150 Hz apart, which 1987.5 fits. The ZVEI3 group tone is 885 Hz (Midian) or
//! 886 Hz (WAVECOM); 885 Hz is kept. The tones A to F other than the repeat and
//! group tones are not kept: Midian notes that makers changed them "for competitive or system needs", and the two sources
//! disagree on them. NATEL's group tone is left out for the same reason (1995 Hz against 1633 Hz).
//!
//! ZVEI and the CCIR/EEA sets are industry and CCIR-era specifications that are not freely available; the sets follow the two
//! manufacturer tables above.
//!
//! ```
//! use sdrkit_util_ids::tones::selcall;
//!
//! let zvei1 = selcall::by_name("ZVEI1").unwrap();
//! assert_eq!(zvei1.digits[1], 1060.0);
//! assert_eq!(zvei1.repeat_hz, 2600.0);
//! assert_eq!(zvei1.tone_ms, 70.0);
//! ```

/// One tone set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneSet {
    /// The short name ("ZVEI1", "CCIR1").
    pub name: &'static str,
    /// Digits 0 to 9, Hz.
    pub digits: [f32; 10],
    /// The repeat tone (digit E), Hz.
    pub repeat_hz: f32,
    /// The group tone (digit A), Hz, where both sources agree on one.
    pub group_hz: Option<f32>,
    /// The usual tone length, ms.
    pub tone_ms: f32,
}

const ZVEI: [f32; 10] = [
    2400.0, 1060.0, 1160.0, 1270.0, 1400.0, 1530.0, 1670.0, 1830.0, 2000.0, 2200.0,
];
const DZVEI: [f32; 10] = [
    2200.0, 970.0, 1060.0, 1160.0, 1270.0, 1400.0, 1530.0, 1670.0, 1830.0, 2000.0,
];
const CCIR: [f32; 10] = [
    1981.0, 1124.0, 1197.0, 1275.0, 1358.0, 1446.0, 1540.0, 1640.0, 1747.0, 1860.0,
];

const fn s(
    name: &'static str,
    digits: [f32; 10],
    repeat_hz: f32,
    group_hz: f32,
    tone_ms: f32,
) -> ToneSet {
    ToneSet {
        name,
        digits,
        repeat_hz,
        group_hz: if group_hz > 0.0 { Some(group_hz) } else { None },
        tone_ms,
    }
}

/// Every tone set.
pub static TONE_SETS: [ToneSet; 14] = [
    s("ZVEI1", ZVEI, 2600.0, 2800.0, 70.0),
    s("ZVEI2", ZVEI, 970.0, 0.0, 70.0),
    s("ZVEI3", DZVEI, 2400.0, 885.0, 70.0),
    s("PZVEI", ZVEI, 2600.0, 970.0, 70.0),
    s("DZVEI", DZVEI, 2400.0, 825.0, 70.0),
    s("PDZVEI", DZVEI, 2400.0, 0.0, 70.0),
    s("CCIR1", CCIR, 2110.0, 2400.0, 100.0),
    s("CCIR2", CCIR, 2110.0, 2400.0, 70.0),
    s("PCCIR", CCIR, 2110.0, 1050.0, 100.0),
    s("EEA", CCIR, 2110.0, 1055.0, 40.0),
    s(
        "EIA",
        [
            600.0, 741.0, 882.0, 1023.0, 1164.0, 1305.0, 1446.0, 1587.0, 1728.0, 1869.0,
        ],
        459.0,
        2151.0,
        33.0,
    ),
    s(
        "EURO",
        [
            979.8, 903.1, 832.5, 767.4, 707.4, 652.0, 601.0, 554.0, 510.7, 470.8,
        ],
        1062.9,
        0.0,
        100.0,
    ),
    s(
        "NATEL",
        [
            1633.0, 631.0, 697.0, 770.0, 852.0, 941.0, 1040.0, 1209.0, 1336.0, 1477.0,
        ],
        1805.0,
        0.0,
        70.0,
    ),
    s(
        "MODAT",
        [
            637.5, 787.5, 937.5, 1087.5, 1237.5, 1387.5, 1537.5, 1687.5, 1837.5, 1987.5,
        ],
        487.5,
        0.0,
        40.0,
    ),
];

/// A tone set by name, either case; "CCIR" is CCIR1 and "ZVEI" is ZVEI1.
pub fn by_name(name: &str) -> Option<&'static ToneSet> {
    let n = name.trim();
    let n = if n.eq_ignore_ascii_case("CCIR") {
        "CCIR1"
    } else if n.eq_ignore_ascii_case("ZVEI") {
        "ZVEI1"
    } else {
        n
    };
    TONE_SETS.iter().find(|t| t.name.eq_ignore_ascii_case(n))
}

impl ToneSet {
    /// The tone of a digit ('0' to '9'; 'E' or 'R' is the repeat tone, 'A' or 'G' the group tone).
    pub fn tone(&self, digit: char) -> Option<f32> {
        match digit.to_ascii_uppercase() {
            d @ '0'..='9' => Some(self.digits[d as usize - '0' as usize]),
            'E' | 'R' => Some(self.repeat_hz),
            'A' | 'G' => self.group_hz,
            _ => None,
        }
    }

    /// The digit a tone stands for within `tolerance_hz` ('E' for the repeat tone, 'A' for the group tone).
    pub fn digit(&self, hz: f32, tolerance_hz: f32) -> Option<char> {
        let near = |t: f32| (t - hz).abs() <= tolerance_hz;
        if let Some(i) = self.digits.iter().position(|&t| near(t)) {
            return char::from_digit(i as u32, 10);
        }
        if near(self.repeat_hz) {
            return Some('E');
        }
        self.group_hz.filter(|&g| near(g)).map(|_| 'A')
    }

    /// The tones of a call: each digit, and the repeat tone in place of a digit equal to the one before (so "22222" is sent as
    /// 2 E 2 E 2). Writes into `out` and returns how many tones were written; `None` when a character is not a digit or `out` is
    /// too short. Does not allocate.
    pub fn encode(&self, digits: &str, out: &mut [f32]) -> Option<usize> {
        let mut prev: Option<char> = None;
        let mut n = 0;
        for d in digits.chars() {
            let tone = if prev == Some(d) {
                prev = None;
                self.repeat_hz
            } else {
                prev = Some(d);
                self.tone(d).filter(|_| d.is_ascii_digit())?
            };
            *out.get_mut(n)? = tone;
            n += 1;
        }
        Some(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_digit_tones() {
        let z1 = by_name("zvei1").unwrap();
        assert_eq!(z1.digits, ZVEI);
        assert_eq!(by_name("ZVEI").unwrap().name, "ZVEI1");
        assert_eq!(by_name("ZVEI3").unwrap().digits[0], 2200.0);
        assert_eq!(by_name("ZVEI2").unwrap().repeat_hz, 970.0);
        assert_eq!(by_name("DZVEI").unwrap().group_hz, Some(825.0));
        assert_eq!(by_name("CCIR").unwrap().tone_ms, 100.0);
        assert_eq!(by_name("CCIR2").unwrap().tone_ms, 70.0);
        assert_eq!(by_name("EEA").unwrap().group_hz, Some(1055.0));
        assert_eq!(by_name("PCCIR").unwrap().group_hz, Some(1050.0));
        assert_eq!(by_name("EIA").unwrap().repeat_hz, 459.0);
        assert_eq!(by_name("nope"), None);
    }

    #[test]
    fn series_checks_the_disputed_tones() {
        let euro = by_name("EURO").unwrap();
        for w in euro.digits.windows(2) {
            assert!((w[1] / w[0] - 0.9218).abs() < 0.0005, "{w:?}");
        }
        assert!((euro.digits[0] / euro.repeat_hz - 0.9218).abs() < 0.0005);
        let modat = by_name("MODAT").unwrap();
        for w in modat.digits.windows(2) {
            assert_eq!(w[1] - w[0], 150.0);
        }
    }

    #[test]
    fn repeat_tone_encoding() {
        let z1 = by_name("ZVEI1").unwrap();
        let mut out = [0.0; 8];
        let n = z1.encode("22222", &mut out).unwrap();
        assert_eq!(&out[..n], [1160.0, 2600.0, 1160.0, 2600.0, 1160.0]);
        let n = z1.encode("12345", &mut out).unwrap();
        assert_eq!(&out[..n], [1060.0, 1160.0, 1270.0, 1400.0, 1530.0]);
        assert_eq!(z1.encode("12x", &mut out), None);
        assert_eq!(z1.encode("123456789", &mut out), None, "output too short");
        assert_eq!(z1.digit(1162.0, 10.0), Some('2'));
        assert_eq!(z1.digit(2600.0, 10.0), Some('E'));
        assert_eq!(z1.digit(2800.0, 10.0), Some('A'));
        assert_eq!(z1.digit(500.0, 10.0), None);
    }
}
