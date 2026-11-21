//! Seven-unit words of duplex ARQ teleprinter links on HF: the constant-ratio code of ITU-R F.342-2 (the "TOR 342" code of
//! ITU-R SM.1052, used by single-channel systems after ITU-R F.519), and the word of an identification unit, the five ITA2
//! elements and an odd-parity unit.
//!
//! A word holds unit 1 (sent first) in bit 0, and mark (the stop polarity, "Z") as 1. The ITA2 code of a character is written as
//! in [`crate::ita2`]: element 1 in bit 0, mark as 1.
//!
//! # The F.342 code
//!
//! Every word of F.342-2 Table I has exactly three Z units and four A units: 32 traffic combinations (one per ITA2 combination,
//! the unperforated tape included) and three service signals: signal repetition (RQ), alpha and beta (the idle signal). A
//! word in an inverted place of the repetition cycle has four Z units.
//!
//! # The identification-and-parity word
//!
//! Unit 1 is 0 for a traffic character and 1 for a service signal; units 2 to 6 are ITA2 elements 1 to 5; unit 7 makes the
//! number of 1 units odd. No public document defines this word.
//!
//! ```
//! use sdrkit_util_text::sevenunit::{f342, F342Signal, ident_parity};
//! assert_eq!(f342::word(3), 0x2C); // A = AAZZAZA
//! assert_eq!(f342::signal(0x1A), Some(F342Signal::Beta));
//! assert_eq!(ident_parity::word(3, false), 0x46);
//! assert!(ident_parity::passes(0x46));
//! ```

/// What an F.342 word stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum F342Signal {
    /// A traffic combination: the ITA2 code.
    Traffic(u8),
    /// Signal repetition (RQ), AZZAZAA.
    Repetition,
    /// Signal alpha, AZAZAAZ.
    Alpha,
    /// Signal beta, AZAZZAA: the idle signal.
    Beta,
}

/// The number of Z (1) units of a word.
pub fn weight(w: u8) -> u32 {
    (w & 0x7F).count_ones()
}

/// Whether a seven-unit word has exactly three 1 units (the 35 words of a 3:4 constant-ratio code).
pub fn three_of_seven(w: u8) -> bool {
    w < 0x80 && weight(w) == 3
}

/// The constant-ratio code of ITU-R F.342-2 Table I.
pub mod f342 {
    use super::F342Signal;

    /// The word of each ITA2 code (index = code): F.342-2 Table I, Z = 1, unit 1 in bit 0.
    pub const WORDS: [u8; 32] = [
        0x70, 0x0E, 0x0D, 0x2C, 0x0B, 0x2A, 0x07, 0x26, 0x61, 0x1C, 0x13, 0x62, 0x15, 0x64, 0x19,
        0x68, 0x51, 0x46, 0x23, 0x52, 0x25, 0x54, 0x29, 0x58, 0x31, 0x4C, 0x43, 0x32, 0x45, 0x34,
        0x49, 0x38,
    ];
    /// Signal repetition (RQ).
    pub const REPETITION: u8 = 0x16;
    /// Signal alpha.
    pub const ALPHA: u8 = 0x4A;
    /// Signal beta, the idle signal.
    pub const BETA: u8 = 0x1A;

    /// The word of ITA2 code `code` (its low five bits).
    pub fn word(code: u8) -> u8 {
        WORDS[usize::from(code & 31)]
    }

    /// What word `w` stands for; `None` for a word that is not in the table.
    pub fn signal(w: u8) -> Option<F342Signal> {
        match w {
            REPETITION => Some(F342Signal::Repetition),
            ALPHA => Some(F342Signal::Alpha),
            BETA => Some(F342Signal::Beta),
            _ => WORDS
                .iter()
                .position(|&v| v == w)
                .map(|c| F342Signal::Traffic(c as u8)),
        }
    }
}

/// The word of an identification unit, five ITA2 elements and an odd-parity unit.
pub mod ident_parity {
    /// The word of ITA2 code `code`; `service` sets the identification unit (a service signal such as idle or RQ).
    pub fn word(code: u8, service: bool) -> u8 {
        let w = u8::from(service) | ((code & 31) << 1);
        if w.count_ones().is_multiple_of(2) {
            w | 0x40
        } else {
            w
        }
    }

    /// Whether `w` has an odd number of 1 units.
    pub fn passes(w: u8) -> bool {
        (w & 0x7F).count_ones() % 2 == 1
    }

    /// Whether `w` is a service signal (identification unit 1).
    pub fn is_service(w: u8) -> bool {
        w & 1 == 1
    }

    /// The ITA2 code of `w` (units 2 to 6).
    pub fn code(w: u8) -> u8 {
        (w >> 1) & 31
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_f342_table_has_35_distinct_words_of_weight_three() {
        let mut all: Vec<u8> = f342::WORDS.to_vec();
        all.extend([f342::REPETITION, f342::ALPHA, f342::BETA]);
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 35);
        assert!(all.iter().all(|&w| three_of_seven(w)));
        assert_eq!((0u8..128).filter(|&w| three_of_seven(w)).count(), 35);
        for c in 0..32u8 {
            assert_eq!(f342::signal(f342::word(c)), Some(F342Signal::Traffic(c)));
        }
        // E = AZZZAAA, space = ZZAZAAA, A inverted = ZZAAZAZ.
        assert_eq!(f342::word(1), 0x0E);
        assert_eq!(f342::word(4), 0x0B);
        assert_eq!(!f342::word(3) & 0x7F, 0x53);
    }

    #[test]
    fn identification_and_parity_words() {
        assert_eq!(ident_parity::word(3, false), 0x46);
        assert_eq!(ident_parity::word(1, false), 0x02);
        assert_eq!(ident_parity::word(4, false), 0x08);
        assert_eq!(!ident_parity::word(3, false) & 0x7F, 0x39);
        for c in 0..32 {
            for s in [false, true] {
                let w = ident_parity::word(c, s);
                assert!(ident_parity::passes(w));
                assert_eq!(ident_parity::code(w), c);
                assert_eq!(ident_parity::is_service(w), s);
            }
        }
    }
}
