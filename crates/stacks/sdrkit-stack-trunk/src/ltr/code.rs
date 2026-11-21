//! The LTR word: 40 bits, most significant first: a 9-bit sync 101011000, AREA (1), GO-TO (5), HOME (5), ID (8), FREE (5) and
//! 7 check bits.
//!
//! Standard: US patent 6,529,486 B1.

use sdrkit_algo_fec::small::SmallCode;
use std::sync::OnceLock;

/// The sync pattern (158 hex), 9 bits.
pub const SYNC: u64 = 0x158;

/// The check values of the 24 information bits, in transmission order (AREA, GO-TO 4..0, HOME 4..0, ID 7..0, FREE 4..0).
pub const CHECK_VALUES: [u8; 24] = [
    0x38, 0x1C, 0x0E, 0x46, 0x23, 0x51, 0x68, 0x75, 0x7A, 0x3D, 0x1F, 0x4F, 0x26, 0x52, 0x29, 0x15,
    0x0B, 0x45, 0x62, 0x31, 0x19, 0x0D, 0x07, 0x43,
];

/// The fields of a word.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LtrFields {
    /// AREA bit.
    pub area: u8,
    /// GO-TO repeater (or a special code).
    pub goto: u8,
    /// HOME repeater.
    pub home: u8,
    /// ID (talkgroup within the home repeater, 240 to 255 special).
    pub id: u8,
    /// FREE repeater (or a special code).
    pub free: u8,
}

/// The 24 information bits of the fields, first sent most significant.
pub fn info_bits(f: &LtrFields) -> u32 {
    ((f.area as u32 & 1) << 23)
        | ((f.goto as u32 & 31) << 18)
        | ((f.home as u32 & 31) << 13)
        | ((f.id as u32) << 5)
        | (f.free as u32 & 31)
}

/// The LTR word code as a systematic (31,24) code: the check bits of each information bit are its value in [`CHECK_VALUES`].
pub fn code() -> &'static SmallCode {
    static C: OnceLock<SmallCode> = OnceLock::new();
    C.get_or_init(|| {
        let rows: Vec<u32> = CHECK_VALUES.iter().map(|&v| v as u32).collect();
        SmallCode::from_parity_rows(31, 24, &rows).expect("24 rows of 7 check bits")
    })
}

/// The check of 24 information bits: the XOR of the values of the bits that are 1 (US 6,529,486 B1, the check table).
pub fn check_of(info: u32) -> u8 {
    (code().encode(info as u128) & 0x7F) as u8
}

/// The 40-bit word of the fields (in the low 40 bits).
pub fn encode(f: &LtrFields) -> u64 {
    let info = info_bits(f);
    (SYNC << 31) | ((info as u64) << 7) | check_of(info) as u64
}

/// The fields of a 40-bit word.
pub fn fields(w: u64) -> LtrFields {
    let info = ((w >> 7) & 0xFF_FFFF) as u32;
    LtrFields {
        area: ((info >> 23) & 1) as u8,
        goto: ((info >> 18) & 31) as u8,
        home: ((info >> 13) & 31) as u8,
        id: ((info >> 5) & 0xFF) as u8,
        free: (info & 31) as u8,
    }
}

/// The check bits as received.
pub fn check_bits(w: u64) -> u8 {
    (w & 0x7F) as u8
}

/// Whether the check is right: the computed value, or (for a word found with the inverted sync, sent by a mobile) its complement.
pub fn check_ok(w: u64, inverted_sync: bool) -> bool {
    let c = check_of(((w >> 7) & 0xFF_FFFF) as u32);
    let got = check_bits(w);
    got == c || (inverted_sync && got == c ^ 0x7F)
}

/// Whether the fields are plausible: HOME 1 to 20, GO-TO 0 to 20 or 29 to 31, FREE 0 to 20 or 31.
pub fn plausible(f: &LtrFields) -> bool {
    (1..=20).contains(&f.home)
        && (f.goto <= 20 || (29..=31).contains(&f.goto))
        && (f.free <= 20 || f.free == 31)
}

/// "{AREA}-{HOME, 2 digits}-{ID, 3 digits}".
pub fn talkgroup(f: &LtrFields) -> String {
    format!("{}-{:02}-{:03}", f.area, f.home, f.id)
}

sdrkit_core::choice_enum! {
    /// What the ID of a word says.
    pub enum WordKind {
        /// A talkgroup: a call.
        Call = ("call", "Call"),
        /// The repeater is idle.
        Idle = ("idle", "Idle"),
        /// The repeater sends its callsign.
        CallsignId = ("callsign_id", "Callsign ID"),
        /// The repeater is busy.
        Busy = ("busy", "Busy"),
        /// A token.
        Token = ("token", "Token"),
    }
}

/// The kind of an ID: 255 idle, 254 callsign ID, 253 busy, 240 to 247 token, otherwise a call.
pub fn kind(id: u8) -> WordKind {
    match id {
        255 => WordKind::Idle,
        254 => WordKind::CallsignId,
        253 => WordKind::Busy,
        240..=247 => WordKind::Token,
        _ => WordKind::Call,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(area: u8, goto: u8, home: u8, id: u8, free: u8) -> LtrFields {
        LtrFields {
            area,
            goto,
            home,
            id,
            free,
        }
    }

    #[test]
    fn l1_six_words() {
        let cases = [
            (f(0, 3, 3, 17, 5), 0x55),
            (f(0, 5, 1, 101, 7), 0x5C),
            (f(1, 12, 12, 254, 2), 0x31),
            (f(0, 0, 8, 255, 9), 0x04),
            (f(0, 7, 2, 33, 20), 0x05),
            (f(0, 19, 19, 200, 1), 0x1B),
        ];
        for (w, c) in cases {
            let e = encode(&w);
            assert!(e < 1 << 40);
            assert_eq!(e >> 31, SYNC);
            assert_eq!(fields(e), w);
            assert_eq!(check_bits(e), c, "{w:?}");
            assert!(plausible(&w));
            assert!(check_ok(e, false));
        }
        assert_eq!(talkgroup(&f(0, 3, 3, 17, 5)), "0-03-017");
    }

    #[test]
    fn l2_kinds() {
        assert_eq!(kind(255), WordKind::Idle);
        assert_eq!(kind(254), WordKind::CallsignId);
        assert_eq!(kind(17), WordKind::Call);
    }

    #[test]
    fn l3_check_values() {
        let mut all: Vec<u8> = CHECK_VALUES.to_vec();
        all.extend((0..7).map(|i| 1u8 << i));
        for (i, a) in all.iter().enumerate() {
            assert_ne!(*a, 0);
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
        let w = encode(&f(0, 3, 3, 17, 5));
        for b in 0..31 {
            assert!(!check_ok(w ^ (1 << b), false), "bit {b}");
        }
        let comp = w ^ 0x7F;
        assert!(!check_ok(comp, false));
        assert!(check_ok(comp, true));
    }

    #[test]
    fn l4_bad_sync_and_home() {
        let w = encode(&f(0, 3, 3, 17, 5)) ^ (1 << 35);
        assert_ne!(w >> 31, SYNC);
        assert!(!plausible(&f(0, 3, 25, 17, 5)));
    }
}
