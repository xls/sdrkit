//! DTMF (dual-tone multi-frequency) signalling: each key is one tone of a low group (row) and one of a high group (column),
//! ITU-T Q.23 (Technical features of push-button telephone sets) and Q.24.
//!
//! ```
//! use sdrkit_util_ids::tones::dtmf;
//!
//! assert_eq!(dtmf::tones('5'), Some((770.0, 1336.0)));
//! assert_eq!(dtmf::key(941.0, 1477.0), Some('#'));
//! ```

/// The low-group (row) frequencies, Hz.
pub const ROWS: [f32; 4] = [697.0, 770.0, 852.0, 941.0];
/// The high-group (column) frequencies, Hz.
pub const COLUMNS: [f32; 4] = [1209.0, 1336.0, 1477.0, 1633.0];
/// The keys by row and column.
pub const KEYS: [[char; 4]; 4] = [
    ['1', '2', '3', 'A'],
    ['4', '5', '6', 'B'],
    ['7', '8', '9', 'C'],
    ['*', '0', '#', 'D'],
];

/// The (row, column) indexes of a key (either case for A to D).
pub fn position(key: char) -> Option<(usize, usize)> {
    let k = key.to_ascii_uppercase();
    KEYS.iter()
        .enumerate()
        .find_map(|(r, row)| row.iter().position(|&c| c == k).map(|c| (r, c)))
}

/// The (row, column) frequencies of a key, Hz.
pub fn tones(key: char) -> Option<(f32, f32)> {
    position(key).map(|(r, c)| (ROWS[r], COLUMNS[c]))
}

/// The key of an exact (row, column) frequency pair.
pub fn key(row_hz: f32, column_hz: f32) -> Option<char> {
    let r = ROWS.iter().position(|&f| f == row_hz)?;
    let c = COLUMNS.iter().position(|&f| f == column_hz)?;
    Some(KEYS[r][c])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q23_matrix() {
        assert_eq!(tones('1'), Some((697.0, 1209.0)));
        assert_eq!(tones('0'), Some((941.0, 1336.0)));
        assert_eq!(tones('*'), Some((941.0, 1209.0)));
        assert_eq!(tones('d'), Some((941.0, 1633.0)));
        assert_eq!(tones('E'), None);
        for row in KEYS {
            for k in row {
                let (a, b) = tones(k).unwrap();
                assert_eq!(key(a, b), Some(k));
            }
        }
        assert_eq!(key(700.0, 1209.0), None);
    }
}
