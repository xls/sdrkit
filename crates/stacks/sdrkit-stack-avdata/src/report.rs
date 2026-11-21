//! What the result tables of the stacks share: rows of things heard with their signal time and wall-clock time, the common
//! columns, and short text forms for tables.

use sdrkit_core::{Clock, LockState, Results};
use sdrkit_util_objects::Shift;
use serde::{Deserialize, Serialize};

/// A 24-bit address, shown as six hex digits: the row key of a table of things known by their address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Address(pub u32);

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:06X}", self.0)
    }
}

/// `a` as six upper-case hex digits, written into `buf` (only the low 24 bits).
pub fn hex6(a: u32, buf: &mut [u8; 6]) -> &str {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for (i, b) in buf.iter_mut().enumerate() {
        *b = HEX[((a >> (20 - 4 * i)) & 0xF) as usize];
    }
    std::str::from_utf8(buf).unwrap_or("")
}

/// Something heard at a moment: the signal time, and the wall-clock time when the samples are live.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Heard<T> {
    /// Signal time, seconds.
    pub at: f64,
    /// Wall-clock time (seconds since 1970, UTC), when known.
    pub wall: Option<f64>,
    /// What was heard.
    pub item: T,
}

impl<T> Shift for Heard<T> {
    fn shift_times(&mut self, by_s: f64) {
        self.at += by_s;
    }
}

/// Writes the status of `r`: the lock state `level` (0 searching, 1 signal, 2 decoding), the SNR, the level, the offset and the
/// summary. Nothing moves when nothing changed.
pub fn status(
    r: &mut Results,
    level: u8,
    snr_db: Option<f32>,
    level_dbfs: Option<f32>,
    offset_hz: Option<f64>,
    summary: &str,
) {
    let lock_state = LockState::from_level(level);
    r.update_status(|s| {
        let changed = s.lock_state != lock_state
            || s.snr_db != snr_db
            || s.level_dbfs != level_dbfs
            || s.offset_hz != offset_hz
            || s.summary != summary;
        if changed {
            s.lock_state = lock_state;
            s.snr_db = snr_db;
            s.level_dbfs = level_dbfs;
            s.offset_hz = offset_hz;
            summary.clone_into(&mut s.summary);
        }
        changed
    });
}

/// The wall-clock time now of `clock` when `live`, else `None` (a recording has no wall clock).
pub fn wall(live: bool, clock: &Clock) -> Option<f64> {
    live.then(|| clock.now())
}

/// `Some(s)` when `s` is not empty.
pub fn text(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

/// `Some(v)` when `v` is a finite number.
pub fn finite(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite())
}

/// The text on one line: line breaks become spaces, at most `n` characters, then "...".
pub fn one_line(text: &str, n: usize) -> String {
    let mut s = String::with_capacity(text.len().min(n + 3));
    for (i, c) in text.chars().enumerate() {
        if i == n {
            s.push_str("...");
            break;
        }
        s.push(if c == '\n' || c == '\r' { ' ' } else { c });
    }
    s
}

/// Hex of bytes, upper case, no spaces.
pub fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    sdrkit_core::base64::hex_into(b, &mut s);
    s
}

/// "hh:mm:ss" of seconds of the day.
pub fn hms(sec_of_day: f64) -> String {
    let s = sec_of_day.rem_euclid(86_400.0) as u64;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// Reads a channel list: numbers separated by spaces, commas or semicolons, each in units of `unit_hz` (1e6 for MHz). Returns
/// the frequencies in Hz, rounded to the hertz.
///
/// # Errors
///
/// The byte position of the first word that is not a number, or whose frequency is outside `band_hz`.
pub fn freq_list(text: &str, unit_hz: f64, band_hz: (f64, f64)) -> Result<Vec<f64>, usize> {
    let mut v = Vec::new();
    let mut at = 0;
    for t in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let start = at;
        at += t.len() + 1;
        if t.is_empty() {
            continue;
        }
        match t.parse::<f64>() {
            Ok(x) if (band_hz.0..=band_hz.1).contains(&(x * unit_hz).round()) => {
                v.push((x * unit_hz).round());
            }
            _ => return Err(start),
        }
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(hms(40_000.0), "11:06:40");
        assert_eq!(one_line("A\nB", 80), "A B");
        assert_eq!(one_line("ABCDEF", 3), "ABC...");
        let band = (118e6, 137e6);
        assert_eq!(
            freq_list("131.525 131.725,131.825", 1e6, band),
            Ok(vec![131.525e6, 131.725e6, 131.825e6])
        );
        assert_eq!(freq_list("131.525 x", 1e6, band), Err(8));
        assert_eq!(freq_list("131.525 150", 1e6, band), Err(8));
        assert_eq!(hex(&[0x0A, 0xFF]), "0AFF");
    }
}
