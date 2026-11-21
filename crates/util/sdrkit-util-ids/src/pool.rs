//! The string pool the generated tables use: one `&'static str` holding every distinct string of a table, and the end offset of
//! each, so a table of thousands of names costs one relocation instead of one per name.

/// A pool of strings stored end to end.
pub(crate) struct Pool {
    /// Every string, end to end.
    pub text: &'static str,
    /// The end offset of string `i` in `text`; string `i` starts where string `i - 1` ends.
    pub ends: &'static [u32],
}

impl Pool {
    /// String `i`, or "" for an index past the end (the build checks every index, so this does not happen).
    pub fn get(&'static self, i: u16) -> &'static str {
        let i = i as usize;
        let end = match self.ends.get(i) {
            Some(&e) => e as usize,
            None => return "",
        };
        let start = if i == 0 { 0 } else { self.ends[i - 1] as usize };
        self.text.get(start..end).unwrap_or("")
    }
}

include!(concat!(env!("OUT_DIR"), "/tables.rs"));

/// Binary search of a sorted key array.
pub(crate) fn find<K: Ord>(keys: &[K], key: &K) -> Option<usize> {
    keys.binary_search(key).ok()
}
