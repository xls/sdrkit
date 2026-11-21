//! Text compared without regard to case, char by char, with no allocation.

use std::cmp::Ordering;

/// Compares two texts without regard to case, char by char.
pub fn cmp_text(a: &str, b: &str) -> Ordering {
    a.chars()
        .flat_map(char::to_lowercase)
        .cmp(b.chars().flat_map(char::to_lowercase))
}

/// True if `hay` starts with `needle`, without regard to case.
pub fn starts(hay: &str, needle: &str) -> bool {
    let mut h = hay.chars().flat_map(char::to_lowercase);
    needle
        .chars()
        .flat_map(char::to_lowercase)
        .all(|c| h.next() == Some(c))
}

/// True if `hay` ends with `needle`, without regard to case.
pub fn ends(hay: &str, needle: &str) -> bool {
    let mut h = hay.chars().rev().flat_map(char::to_lowercase);
    needle
        .chars()
        .rev()
        .flat_map(char::to_lowercase)
        .all(|c| h.next() == Some(c))
}

/// True if `hay` holds `needle`, without regard to case.
pub fn contains(hay: &str, needle: &str) -> bool {
    hay.char_indices()
        .map(|(i, _)| &hay[i..])
        .chain(std::iter::once(""))
        .any(|tail| starts(tail, needle))
}
