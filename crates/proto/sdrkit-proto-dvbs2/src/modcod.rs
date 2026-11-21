//! DVB-S2 MODCODs: modulation, code rate, frame sizes, BCH and LDPC parameters and the ideal Es/N0.
//!
//! Standard: ETSI EN 302 307-1 tables 12 and 13.

sdrkit_core::choice_enum! {
    /// The modulation of a DVB-S2 MODCOD.
    pub enum Modulation {
        /// QPSK.
        Qpsk = ("QPSK", "QPSK"),
        /// 8PSK.
        Psk8 = ("8PSK", "8PSK"),
        /// 16APSK.
        Apsk16 = ("16APSK", "16APSK"),
        /// 32APSK.
        Apsk32 = ("32APSK", "32APSK"),
    }
}

sdrkit_core::choice_enum! {
    /// A code rate of DVB-S or DVB-S2.
    pub enum CodeRate {
        /// 1/4.
        R14 = ("1/4", "1/4"),
        /// 1/3.
        R13 = ("1/3", "1/3"),
        /// 2/5.
        R25 = ("2/5", "2/5"),
        /// 1/2.
        R12 = ("1/2", "1/2"),
        /// 3/5.
        R35 = ("3/5", "3/5"),
        /// 2/3.
        R23 = ("2/3", "2/3"),
        /// 3/4.
        R34 = ("3/4", "3/4"),
        /// 4/5.
        R45 = ("4/5", "4/5"),
        /// 5/6.
        R56 = ("5/6", "5/6"),
        /// 7/8.
        R78 = ("7/8", "7/8"),
        /// 8/9.
        R89 = ("8/9", "8/9"),
        /// 9/10.
        R910 = ("9/10", "9/10"),
    }
}

impl CodeRate {
    /// The numerator and the denominator.
    pub fn fraction(self) -> (u16, u16) {
        match self {
            CodeRate::R14 => (1, 4),
            CodeRate::R13 => (1, 3),
            CodeRate::R25 => (2, 5),
            CodeRate::R12 => (1, 2),
            CodeRate::R35 => (3, 5),
            CodeRate::R23 => (2, 3),
            CodeRate::R34 => (3, 4),
            CodeRate::R45 => (4, 5),
            CodeRate::R56 => (5, 6),
            CodeRate::R78 => (7, 8),
            CodeRate::R89 => (8, 9),
            CodeRate::R910 => (9, 10),
        }
    }

    /// The rate of fraction `f`, if it is one of the list.
    pub fn of(f: (u16, u16)) -> Option<CodeRate> {
        use sdrkit_core::Choice;
        (0..Self::CHOICES.len() as u16)
            .filter_map(Self::from_index)
            .find(|r| r.fraction() == f)
    }

    /// The DVB-S rate of the same value, if DVB-S has it.
    pub fn punctured(self) -> Option<crate::inner::PunctureRate> {
        use crate::inner::PunctureRate as P;
        match self {
            CodeRate::R12 => Some(P::R12),
            CodeRate::R23 => Some(P::R23),
            CodeRate::R34 => Some(P::R34),
            CodeRate::R56 => Some(P::R56),
            CodeRate::R78 => Some(P::R78),
            _ => None,
        }
    }

    /// The rate of the DVB-S rate `r`.
    pub fn of_punctured(r: crate::inner::PunctureRate) -> CodeRate {
        use crate::inner::PunctureRate as P;
        match r {
            P::R12 => CodeRate::R12,
            P::R23 => CodeRate::R23,
            P::R34 => CodeRate::R34,
            P::R56 => CodeRate::R56,
            P::R78 => CodeRate::R78,
        }
    }
}

impl Modulation {
    /// Bits per symbol.
    pub fn bits(self) -> usize {
        match self {
            Modulation::Qpsk => 2,
            Modulation::Psk8 => 3,
            Modulation::Apsk16 => 4,
            Modulation::Apsk32 => 5,
        }
    }
    /// "8PSK".
    pub fn name(self) -> &'static str {
        match self {
            Modulation::Qpsk => "QPSK",
            Modulation::Psk8 => "8PSK",
            Modulation::Apsk16 => "16APSK",
            Modulation::Apsk32 => "32APSK",
        }
    }
}

/// A DVB-S2 MODCOD (1 to 28).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModCod {
    /// 1 to 28.
    pub id: u8,
    /// Modulation.
    pub modulation: Modulation,
    /// Code rate numerator and denominator.
    pub rate: (u16, u16),
}

const TABLE: [(Modulation, u16, u16, f64); 28] = [
    (Modulation::Qpsk, 1, 4, -2.35),
    (Modulation::Qpsk, 1, 3, -1.24),
    (Modulation::Qpsk, 2, 5, -0.30),
    (Modulation::Qpsk, 1, 2, 1.00),
    (Modulation::Qpsk, 3, 5, 2.23),
    (Modulation::Qpsk, 2, 3, 3.10),
    (Modulation::Qpsk, 3, 4, 4.03),
    (Modulation::Qpsk, 4, 5, 4.68),
    (Modulation::Qpsk, 5, 6, 5.18),
    (Modulation::Qpsk, 8, 9, 6.20),
    (Modulation::Qpsk, 9, 10, 6.42),
    (Modulation::Psk8, 3, 5, 5.50),
    (Modulation::Psk8, 2, 3, 6.62),
    (Modulation::Psk8, 3, 4, 7.91),
    (Modulation::Psk8, 5, 6, 9.35),
    (Modulation::Psk8, 8, 9, 10.69),
    (Modulation::Psk8, 9, 10, 10.98),
    (Modulation::Apsk16, 2, 3, 8.97),
    (Modulation::Apsk16, 3, 4, 10.21),
    (Modulation::Apsk16, 4, 5, 11.03),
    (Modulation::Apsk16, 5, 6, 11.61),
    (Modulation::Apsk16, 8, 9, 12.89),
    (Modulation::Apsk16, 9, 10, 13.13),
    (Modulation::Apsk32, 3, 4, 12.73),
    (Modulation::Apsk32, 4, 5, 13.64),
    (Modulation::Apsk32, 5, 6, 14.28),
    (Modulation::Apsk32, 8, 9, 15.69),
    (Modulation::Apsk32, 9, 10, 16.05),
];

impl ModCod {
    /// MODCOD `id` (1 to 28).
    pub fn new(id: u8) -> Option<ModCod> {
        let (m, a, b, _) = *TABLE.get((id as usize).checked_sub(1)?)?;
        Some(ModCod {
            id,
            modulation: m,
            rate: (a, b),
        })
    }
    /// Every MODCOD.
    pub fn all() -> impl Iterator<Item = ModCod> {
        (1..=28).filter_map(ModCod::new)
    }
    /// The MODCOD of a modulation and a code rate, if DVB-S2 has it.
    pub fn find(modulation: Modulation, rate: CodeRate) -> Option<ModCod> {
        ModCod::all().find(|m| m.modulation == modulation && m.rate == rate.fraction())
    }
    /// The code rate.
    pub fn code_rate(&self) -> Option<CodeRate> {
        CodeRate::of(self.rate)
    }
    /// "3/5".
    pub fn rate_name(&self) -> String {
        format!("{}/{}", self.rate.0, self.rate.1)
    }
    /// "QPSK 3/5".
    pub fn name(&self) -> String {
        format!("{} {}", self.modulation.name(), self.rate_name())
    }
    /// The ideal Es/N0 of EN 302 307-1 table 13 (normal frames; short frames +0.25 dB).
    pub fn qef_esn0(&self, short: bool) -> f64 {
        TABLE[self.id as usize - 1].3 + if short { 0.25 } else { 0.0 }
    }
    /// The code rate as a number.
    pub fn rate_value(&self) -> f64 {
        self.rate.0 as f64 / self.rate.1 as f64
    }
    /// Whether the short frame exists (no 9/10).
    pub fn has_short(&self) -> bool {
        self.rate != (9, 10)
    }
    /// The frame's code parameters.
    pub fn code(&self, short: bool) -> Option<Code> {
        Code::of(self.rate, short)
    }
    /// Slots (90 symbols) of the frame's data.
    pub fn slots(&self, short: bool) -> usize {
        let n = if short { 16200 } else { 64800 };
        n / self.modulation.bits() / 90
    }
    /// The PLFRAME length in symbols: header, slots, and a pilot block after every 16 slots but the last.
    pub fn frame_symbols(&self, short: bool, pilots: bool) -> usize {
        let s = self.slots(short);
        90 * (s + 1) + if pilots { 36 * ((s - 1) / 16) } else { 0 }
    }
}

/// The FEC parameters of one code (EN 302 307-1 tables 5a and 5b).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Code {
    /// Code rate.
    pub rate: (u16, u16),
    /// Short frame.
    pub short: bool,
    /// BCH message bits.
    pub kbch: usize,
    /// LDPC message bits (= BCH code word).
    pub kldpc: usize,
    /// LDPC code word.
    pub nldpc: usize,
    /// BCH correction capability.
    pub t: usize,
    /// q of the LDPC parity address rule.
    pub q: usize,
}

impl Code {
    /// The code of `rate` in a normal (64800) or short (16200) frame.
    pub fn of(rate: (u16, u16), short: bool) -> Option<Code> {
        let (kbch, kldpc, t, q) = if !short {
            match rate {
                (1, 4) => (16008, 16200, 12, 135),
                (1, 3) => (21408, 21600, 12, 120),
                (2, 5) => (25728, 25920, 12, 108),
                (1, 2) => (32208, 32400, 12, 90),
                (3, 5) => (38688, 38880, 12, 72),
                (2, 3) => (43040, 43200, 10, 60),
                (3, 4) => (48408, 48600, 12, 45),
                (4, 5) => (51648, 51840, 12, 36),
                (5, 6) => (53840, 54000, 10, 30),
                (8, 9) => (57472, 57600, 8, 20),
                (9, 10) => (58192, 58320, 8, 18),
                _ => return None,
            }
        } else {
            match rate {
                (1, 4) => (3072, 3240, 12, 36),
                (1, 3) => (5232, 5400, 12, 30),
                (2, 5) => (6312, 6480, 12, 27),
                (1, 2) => (7032, 7200, 12, 25),
                (3, 5) => (9552, 9720, 12, 18),
                (2, 3) => (10632, 10800, 12, 15),
                (3, 4) => (11712, 11880, 12, 12),
                (4, 5) => (12432, 12600, 12, 10),
                (5, 6) => (13152, 13320, 12, 8),
                (8, 9) => (14232, 14400, 12, 5),
                _ => return None,
            }
        };
        Some(Code {
            rate,
            short,
            kbch,
            kldpc,
            nldpc: if short { 16200 } else { 64800 },
            t,
            q,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modcods_and_frame_lengths() {
        let m = ModCod::new(5).unwrap();
        assert_eq!(m.name(), "QPSK 3/5");
        assert_eq!(m.frame_symbols(false, false), 32490);
        assert_eq!(m.frame_symbols(false, true), 33282);
        assert_eq!(
            ModCod::new(13).unwrap().frame_symbols(true, true),
            90 * 61 + 36 * 3
        );
        assert_eq!(
            ModCod::find(Modulation::Psk8, CodeRate::R35).unwrap().id,
            12
        );
        assert_eq!(ModCod::new(28).unwrap().name(), "32APSK 9/10");
        for m in ModCod::all() {
            let c = m.code(false).unwrap();
            assert_eq!(
                c.kldpc,
                c.nldpc * m.rate.0 as usize / m.rate.1 as usize,
                "{}",
                m.name()
            );
            assert_eq!(c.kldpc - c.kbch, 16 * c.t);
            assert_eq!(c.q * 360, c.nldpc - c.kldpc);
            if m.has_short() {
                let c = m.code(true).unwrap();
                assert_eq!(c.kldpc - c.kbch, 14 * c.t);
                assert_eq!(c.q * 360, c.nldpc - c.kldpc);
            }
        }
        assert!(!ModCod::new(11).unwrap().has_short());
    }
}
