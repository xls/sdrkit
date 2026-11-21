//! Frequency pairing: the 200 DME channels (1X to 126Y), the VHF frequencies (VOR and localizer) paired with them, and the glide
//! path frequency paired with each localizer frequency.
//! Standard: ICAO Annex 10 Volume I, 3.5.3.3.4 and table A.

sdrkit_core::choice_enum! {
    /// DME channel mode: X or Y.
    pub enum DmeMode {
        /// 12 µs pulse spacing both ways.
        X = ("x", "X"),
        /// 36 µs interrogation, 30 µs reply.
        Y = ("y", "Y"),
    }
}

impl DmeMode {
    /// Pulse pair spacing of the interrogation, µs.
    pub fn interrogation_spacing_us(self) -> f64 {
        match self {
            DmeMode::X => 12.0,
            DmeMode::Y => 36.0,
        }
    }
    /// Pulse pair spacing of the reply, µs.
    pub fn reply_spacing_us(self) -> f64 {
        match self {
            DmeMode::X => 12.0,
            DmeMode::Y => 30.0,
        }
    }
    /// "X" or "Y".
    pub fn letter(self) -> char {
        match self {
            DmeMode::X => 'X',
            DmeMode::Y => 'Y',
        }
    }
}

/// A DME channel, 1 to 126, X or Y.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DmeChannel {
    /// 1 to 126.
    pub number: u32,
    /// X or Y.
    pub mode: DmeMode,
}

impl DmeChannel {
    /// The channel, if the number is 1 to 126.
    pub fn new(number: u32, mode: DmeMode) -> Option<Self> {
        (1..=126)
            .contains(&number)
            .then_some(DmeChannel { number, mode })
    }
    /// "17X".
    pub fn name(&self) -> String {
        format!("{}{}", self.number, self.mode.letter())
    }
    /// Parses "17X" or "17y".
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let (n, m) = s.split_at(s.len().checked_sub(1)?);
        let mode = match m {
            "X" | "x" => DmeMode::X,
            "Y" | "y" => DmeMode::Y,
            _ => return None,
        };
        DmeChannel::new(n.parse().ok()?, mode)
    }
    /// The aircraft's interrogation frequency, MHz: 1025 + (n - 1).
    pub fn interrogation_mhz(&self) -> f64 {
        1024.0 + self.number as f64
    }
    /// The ground beacon's reply frequency, MHz (63 MHz from the interrogation).
    pub fn reply_mhz(&self) -> f64 {
        let i = self.interrogation_mhz();
        match (self.mode, self.number <= 63) {
            (DmeMode::X, true) | (DmeMode::Y, false) => i - 63.0,
            (DmeMode::X, false) | (DmeMode::Y, true) => i + 63.0,
        }
    }
    /// The paired VHF frequency (VOR, localizer), MHz. `None` for channels 1-16 and 60-69, which have none.
    pub fn vhf_mhz(&self) -> Option<f64> {
        let n = self.number as f64;
        let y = if self.mode == DmeMode::Y { 0.05 } else { 0.0 };
        match self.number {
            17..=59 => Some(round2(108.0 + (n - 17.0) * 0.1 + y)),
            70..=126 => Some(round2(112.3 + (n - 70.0) * 0.1 + y)),
            _ => None,
        }
    }
    /// The channel paired with a VHF frequency (108.00 to 117.95 MHz on the 50 kHz grid).
    pub fn for_vhf(mhz: f64) -> Option<Self> {
        let k = ((mhz - 108.0) / 0.05).round();
        if !(0.0..=199.0).contains(&k) || (mhz - (108.0 + k * 0.05)).abs() > 0.01 {
            return None;
        }
        let k = k as u32;
        let mode = if k % 2 == 1 { DmeMode::Y } else { DmeMode::X };
        let tenth = k / 2; // 0 = 108.0
        let number = if tenth <= 42 {
            17 + tenth
        } else {
            70 + tenth - 43
        };
        DmeChannel::new(number, mode).filter(|c| c.vhf_mhz().is_some())
    }
    /// The channel whose reply (or interrogation, when `reply` is false) is at `mhz` and whose spacing is `mode`'s.
    pub fn for_frequency(mhz: f64, mode: DmeMode, reply: bool) -> Option<Self> {
        (1..=126).find_map(|n| {
            let c = DmeChannel { number: n, mode };
            let f = if reply {
                c.reply_mhz()
            } else {
                c.interrogation_mhz()
            };
            ((f - mhz).abs() < 0.5).then_some(c)
        })
    }
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Localizer frequency (MHz) and its paired glide path frequency (MHz).
pub const ILS_PAIRS: [(f64, f64); 40] = [
    (108.10, 334.70),
    (108.15, 334.55),
    (108.30, 334.10),
    (108.35, 333.95),
    (108.50, 329.90),
    (108.55, 329.75),
    (108.70, 330.50),
    (108.75, 330.35),
    (108.90, 329.30),
    (108.95, 329.15),
    (109.10, 331.40),
    (109.15, 331.25),
    (109.30, 332.00),
    (109.35, 331.85),
    (109.50, 332.60),
    (109.55, 332.45),
    (109.70, 333.20),
    (109.75, 333.05),
    (109.90, 333.80),
    (109.95, 333.65),
    (110.10, 334.40),
    (110.15, 334.25),
    (110.30, 335.00),
    (110.35, 334.85),
    (110.50, 329.60),
    (110.55, 329.45),
    (110.70, 330.20),
    (110.75, 330.05),
    (110.90, 330.80),
    (110.95, 330.65),
    (111.10, 331.70),
    (111.15, 331.55),
    (111.30, 332.30),
    (111.35, 332.15),
    (111.50, 332.90),
    (111.55, 332.75),
    (111.70, 333.50),
    (111.75, 333.35),
    (111.90, 331.10),
    (111.95, 330.95),
];

/// The glide path frequency paired with a localizer frequency.
pub fn glide_path_for(loc_mhz: f64) -> Option<f64> {
    ILS_PAIRS
        .iter()
        .find(|p| (p.0 - loc_mhz).abs() < 0.01)
        .map(|p| p.1)
}

/// The localizer frequency paired with a glide path frequency.
pub fn localizer_for(gp_mhz: f64) -> Option<f64> {
    ILS_PAIRS
        .iter()
        .find(|p| (p.1 - gp_mhz).abs() < 0.01)
        .map(|p| p.0)
}

/// A VHF navigation frequency is a localizer's: 108.10 to 111.95 MHz with an odd tenth.
pub fn is_localizer(mhz: f64) -> bool {
    glide_path_for(mhz).is_some()
}

/// A VHF navigation frequency is a VOR's: 108.00 to 117.95 MHz, not a localizer's, on the 50 kHz grid.
pub fn is_vor(mhz: f64) -> bool {
    (108.0 - 0.01..=117.96).contains(&mhz)
        && ((mhz - 108.0) / 0.05 - ((mhz - 108.0) / 0.05).round()).abs() < 0.2
        && !is_localizer(mhz)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dme_channels_pair_with_vhf() {
        let c = DmeChannel::parse("17X").unwrap();
        assert_eq!(c.vhf_mhz(), Some(108.0));
        assert_eq!(c.interrogation_mhz(), 1041.0);
        assert_eq!(c.reply_mhz(), 978.0);
        assert_eq!(DmeChannel::parse("59Y").unwrap().vhf_mhz(), Some(112.25));
        assert_eq!(DmeChannel::parse("70X").unwrap().vhf_mhz(), Some(112.3));
        assert_eq!(DmeChannel::parse("126Y").unwrap().vhf_mhz(), Some(117.95));
        assert_eq!(DmeChannel::parse("1X").unwrap().reply_mhz(), 962.0);
        assert_eq!(DmeChannel::parse("126X").unwrap().reply_mhz(), 1213.0);
        assert_eq!(DmeChannel::parse("1Y").unwrap().reply_mhz(), 1088.0);
        assert_eq!(DmeChannel::parse("64Y").unwrap().reply_mhz(), 1025.0);
        assert_eq!(DmeChannel::parse("65X").unwrap().vhf_mhz(), None);
        assert_eq!(DmeChannel::for_vhf(113.1).unwrap().name(), "78X");
        assert_eq!(DmeChannel::for_vhf(109.35).unwrap().name(), "30Y");
        for n in [17, 40, 59, 70, 100, 126] {
            for m in [DmeMode::X, DmeMode::Y] {
                let c = DmeChannel::new(n, m).unwrap();
                assert_eq!(DmeChannel::for_vhf(c.vhf_mhz().unwrap()), Some(c));
            }
        }
    }

    #[test]
    fn ils_pairs() {
        assert_eq!(glide_path_for(110.3), Some(335.0));
        assert_eq!(localizer_for(329.15), Some(108.95));
        assert!(is_localizer(109.5));
        assert!(!is_localizer(109.6));
        assert!(is_vor(113.1) && is_vor(108.2) && !is_vor(108.1));
    }
}
