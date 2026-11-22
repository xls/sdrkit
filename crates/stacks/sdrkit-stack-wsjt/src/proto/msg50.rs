//! The 50-bit message of FST4W: the first 50 bits of the 77-bit message of type i3 = 0, n3 = 6. Type 1 is a standard call,
//! a 4-character locator and a power; type 2 a standard call with a prefix or suffix and a power; type 3 the 22-bit hash
//! of a call and a 6-character locator. Bit numbers below are 0-based within the 50 bits, each field most significant bit
//! first.
//!
//! | Type | Bits | Content |
//! |---|---|---|
//! | 1 | 0 … 27, 28 … 42, 43 … 47, 48 … 49 | call field, locator (as in the 77-bit message), power index, 0 0 |
//! | 2 | 0 … 27, 28 … 43, 44 … 48, 49 | call field of the base call, prefix or suffix number, power index, 1 |
//! | 3 | 0 … 21, 22 … 46, 47 … 49 | 22-bit call hash, 6-character locator, 0 1 0 |

use super::msg77::{
    grid4_text, grid4_value, hash22, is_grid6, is_plausible, std_call_text, std_call_value,
    CallHashes, PackError, MAX22, NTOKENS,
};

/// The 36 characters of a prefix or suffix, value 0 … 35.
const A36: &[u8; 36] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// The prefix or suffix numbers from this one up are suffixes.
const SUFFIX: u32 = 46_656;
/// The locator value of a 4-character locator in type 3: subsquare letters 24 and 24.
const NO_SUBSQUARE: u32 = 24 * 25 + 24;

/// A decoded FST4W message.
#[derive(Clone, Debug, PartialEq)]
pub struct Msg50 {
    /// The text (a type 3 shows `<CALL>`, or `<...>` for a hash not yet heard).
    pub text: String,
    /// The message type, 1 to 3.
    pub kind: u8,
    /// The call; for type 3 the call of the hash when known, else empty.
    pub call: String,
    /// The locator (4 or 6 characters, empty for type 2).
    pub grid: String,
    /// Power, dBm; -1 for type 3, which has no power.
    pub dbm: i32,
}

/// The power index of `dbm`: round(0.3 · dBm), dBm first limited to 0 … 60.
fn power_index(dbm: i32) -> u32 {
    (0.3 * f64::from(dbm.clamp(0, 60))).round() as u32
}

/// The power of an index (0 … 18): round(index · 10 / 3) dBm.
fn index_power(i: u32) -> i32 {
    (f64::from(i) * 10.0 / 3.0).round() as i32
}

/// The value of a character of a prefix or suffix.
fn a36(c: u8) -> Option<u32> {
    A36.iter().position(|&x| x == c).map(|v| v as u32)
}

/// The call field of a standard call.
fn call_field(call: &str) -> Option<u32> {
    std_call_value(call).map(|v| NTOKENS + MAX22 + v)
}

/// The standard call of a call field.
fn field_call(n: u32) -> Option<String> {
    let v = n.checked_sub(NTOKENS + MAX22)?;
    if v >= 37 * 36 * 10 * 27 * 27 * 27 {
        return None;
    }
    let s = std_call_text(v);
    (s.len() >= 3).then_some(s)
}

/// The number of a prefix (1 to 3 characters: its base-36 value).
fn prefix_number(p: &str) -> Option<u32> {
    if !(1..=3).contains(&p.len()) {
        return None;
    }
    p.bytes().try_fold(0u32, |n, c| Some(36 * n + a36(c)?))
}

/// The number of a suffix: one character, two characters, or three characters of which the third is a digit.
fn suffix_number(s: &str) -> Option<u32> {
    let v = match s.as_bytes() {
        [a] => a36(*a)?,
        [a, b] => 36 * a36(*a)? + a36(*b)?,
        [a, b, c] if c.is_ascii_digit() => 360 * a36(*a)? + 10 * a36(*b)? + u32::from(c - b'0'),
        _ => return None,
    };
    Some(SUFFIX + v)
}

/// The text of a prefix or suffix number.
fn affix_text(n: u32) -> Option<(String, bool)> {
    let ch = |v: u32| A36[v as usize] as char;
    if n < SUFFIX {
        let mut v = n;
        let mut p = Vec::new();
        loop {
            p.push(ch(v % 36));
            v /= 36;
            if v == 0 {
                break;
            }
        }
        return Some((p.iter().rev().collect(), false));
    }
    let v = n - SUFFIX;
    let s = match v {
        0..=35 => ch(v).to_string(),
        36..=1295 => format!("{}{}", ch(v / 36), ch(v % 36)),
        1296..=12959 => format!("{}{}{}", ch(v / 360), ch(v / 10 % 36), v % 10),
        _ => return None,
    };
    Some((s, true))
}

/// The 25-bit locator of type 3 from a 4- or 6-character locator.
fn grid25(g: &str) -> Option<u32> {
    let b = g.as_bytes();
    let v4 = grid4_value(g.get(..4)?)?;
    let sub = match b.len() {
        4 => NO_SUBSQUARE,
        6 if is_grid6(g) => 25 * u32::from(b[4] - b'A') + u32::from(b[5] - b'A'),
        _ => return None,
    };
    Some(v4 * 625 + sub)
}

/// The locator of a 25-bit value of type 3.
fn grid25_text(v: u32) -> Option<String> {
    let (v4, sub) = (v / 625, v % 625);
    if v4 >= 32_400 {
        return None;
    }
    let g = grid4_text(v4);
    if sub == NO_SUBSQUARE {
        return Some(g);
    }
    let (s1, s2) = (sub / 25, sub % 25);
    if s1 >= 24 || s2 >= 24 {
        return None;
    }
    Some(format!("{g}{}{}", (b'A' + s1 as u8) as char, (b'A' + s2 as u8) as char))
}

/// Appends the `n` low bits of `v` to `out`, most significant first.
fn put(out: &mut Vec<u8>, v: u32, n: u32) {
    out.extend((0..n).rev().map(|i| (v >> i & 1) as u8));
}

/// The value of `bits`, most significant first.
fn get(bits: &[u8]) -> u32 {
    bits.iter().fold(0, |a, &b| a << 1 | u32::from(b & 1))
}

/// Packs an FST4W message into its 50 bits: `CALL GRID4 DBM` (type 1), `PFX/CALL DBM` or `CALL/SFX DBM` (type 2), or
/// `<CALL> GRID` with a 4- or 6-character locator (type 3, no power). A power is limited to 0 … 60 dBm and sent to the
/// nearest of 0, 3, 7, 10, … 60.
///
/// # Errors
///
/// [`PackError::NotWspr`] when the text has none of these forms.
pub fn pack50(text: &str) -> Result<[u8; 50], PackError> {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_uppercase();
    let w: Vec<&str> = t.split(' ').collect();
    let bad = PackError::NotWspr;
    let mut b = Vec::with_capacity(50);
    match w.as_slice() {
        [call, grid] if call.starts_with('<') && call.ends_with('>') => {
            let c = call.trim_matches(|c| c == '<' || c == '>');
            if !is_plausible(c) {
                return Err(bad);
            }
            put(&mut b, hash22(c), 22);
            put(&mut b, grid25(grid).ok_or(bad)?, 25);
            put(&mut b, 0b010, 3);
        }
        [call, grid, dbm] => {
            let dbm: i32 = dbm.parse().map_err(|_| bad)?;
            put(&mut b, call_field(call).ok_or(bad)?, 28);
            put(&mut b, grid4_value(grid).ok_or(bad)?, 15);
            put(&mut b, power_index(dbm), 5);
            put(&mut b, 0, 2);
        }
        [call, dbm] => {
            let dbm: i32 = dbm.parse().map_err(|_| bad)?;
            let (a, s) = call.split_once('/').ok_or(bad)?;
            let (base, n) = match (std_call_value(s), std_call_value(a)) {
                (Some(_), _) if a.len() <= 3 => (s, prefix_number(a)),
                (_, Some(_)) => (a, suffix_number(s)),
                _ => return Err(bad),
            };
            put(&mut b, call_field(base).ok_or(bad)?, 28);
            put(&mut b, n.ok_or(bad)?, 16);
            put(&mut b, power_index(dbm), 5);
            put(&mut b, 1, 1);
        }
        _ => return Err(bad),
    }
    let mut out = [0u8; 50];
    out.copy_from_slice(&b);
    Ok(out)
}

/// Unpacks 50 bits, naming a type 3 call from the 22-bit hashes of `hashes`. `None` when the bits are not a message.
pub fn unpack50(bits: &[u8], hashes: &CallHashes) -> Option<Msg50> {
    let bits = bits.get(..50)?;
    let msg = |text: String, kind, call, grid, dbm| Msg50 {
        text,
        kind,
        call,
        grid,
        dbm,
    };
    match (bits[47], bits[48], bits[49]) {
        (_, _, 1) => {
            let call = field_call(get(&bits[..28]))?;
            let (affix, suffix) = affix_text(get(&bits[28..44]))?;
            let i = get(&bits[44..49]);
            if i > 18 {
                return None;
            }
            let full = if suffix {
                format!("{call}/{affix}")
            } else {
                format!("{affix}/{call}")
            };
            let dbm = index_power(i);
            Some(msg(format!("{full} {dbm}"), 2, full, String::new(), dbm))
        }
        (_, 0, 0) => {
            let call = field_call(get(&bits[..28]))?;
            let g = get(&bits[28..43]);
            if g >= 32_400 {
                return None;
            }
            let grid = grid4_text(g);
            let i = get(&bits[43..48]);
            if i > 18 {
                return None;
            }
            let dbm = index_power(i);
            Some(msg(format!("{call} {grid} {dbm}"), 1, call, grid, dbm))
        }
        (0, 1, 0) => {
            let grid = grid25_text(get(&bits[22..47]))?;
            let call = hashes.get22(get(&bits[..22])).unwrap_or_default().to_string();
            let shown = if call.is_empty() { "..." } else { &call };
            Some(msg(format!("<{shown}> {grid}"), 3, call, grid, -1))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::codes::tests::hex_bits;

    #[test]
    fn the_printed_type_1_message() {
        let b = pack50("K1ABC FN42 37").unwrap();
        assert_eq!(b.to_vec(), hex_bits("09bde3550ccb0", 50));
        assert_eq!(get(&b[..28]), 10_214_965);
        assert_eq!(get(&b[28..43]), 10_342);
        assert_eq!(get(&b[43..48]), 11);
        let u = unpack50(&b, &CallHashes::default()).unwrap();
        assert_eq!((u.text.as_str(), u.kind, u.dbm), ("K1ABC FN42 37", 1, 37));
    }

    #[test]
    fn the_powers_go_to_the_nearest_index() {
        for (dbm, shown) in [(0, 0), (3, 3), (7, 7), (10, 10), (23, 23), (38, 37), (60, 60), (70, 60), (-5, 0)] {
            let b = pack50(&format!("K1ABC FN42 {dbm}")).unwrap();
            assert_eq!(unpack50(&b, &CallHashes::default()).unwrap().dbm, shown, "{dbm}");
        }
        let mut b = pack50("K1ABC FN42 37").unwrap();
        b[43..48].copy_from_slice(&[1, 0, 0, 1, 1]);
        assert_eq!(unpack50(&b, &CallHashes::default()), None);
    }

    #[test]
    fn the_types_round_trip() {
        let mut h = CallHashes::default();
        h.learn("PJ4/K1ABC");
        for (t, kind) in [
            ("K1ABC FN42 37", 1),
            ("G4XYZ IO91 23", 1),
            ("PJ4/K1ABC 30", 2),
            ("K/W9XYZ 3", 2),
            ("K1ABC/P 10", 2),
            ("K1ABC/12 33", 2),
            ("K1ABC/AB3 0", 2),
            ("<PJ4/K1ABC> FN42AX", 3),
            ("<PJ4/K1ABC> FN42", 3),
        ] {
            let b = pack50(t).unwrap_or_else(|e| panic!("{t}: {e}"));
            let u = unpack50(&b, &h).unwrap_or_else(|| panic!("{t}"));
            assert_eq!((u.text.as_str(), u.kind), (t, kind));
        }
        assert_eq!(get(&pack50("PJ4/K1ABC 30").unwrap()[28..44]), 25 * 1296 + 19 * 36 + 4);
        assert_eq!(get(&pack50("K1ABC/P 10").unwrap()[28..44]), SUFFIX + 25);
        let b = pack50("<K1ABC> FN42AX").unwrap();
        assert_eq!(b[..22].to_vec(), (0..22).rev().map(|i| (hash22("K1ABC") >> i & 1) as u8).collect::<Vec<_>>());
        assert_eq!(b[47..], [0, 1, 0]);
        let u = unpack50(&b, &CallHashes::default()).unwrap();
        assert_eq!((u.text.as_str(), u.dbm), ("<...> FN42AX", -1));
    }

    #[test]
    fn the_wspr_layout_reads_as_another_message() {
        // The 50 bits of the WSPR message differ from the FST4W message of the same text.
        let wspr = crate::proto::wsprmsg::pack50("K1ABC FN42 37").unwrap();
        assert_ne!(wspr, pack50("K1ABC FN42 37").unwrap());
        assert_eq!(wspr, pack50("YXD/YO3AVD 60").unwrap());
    }

    #[test]
    fn other_texts_and_bits_are_refused() {
        for t in ["K1ABC FN42", "K1ABC FN42AX 37", "<K1ABC> FN42 37", "CQ K1ABC FN42", "K1ABC/ABCD 10", "K1ABC FN42 X"] {
            assert!(pack50(t).is_err(), "{t}");
        }
        let mut b = pack50("<K1ABC> FN42AX").unwrap();
        b[47] = 1;
        assert_eq!(unpack50(&b, &CallHashes::default()), None);
    }
}
