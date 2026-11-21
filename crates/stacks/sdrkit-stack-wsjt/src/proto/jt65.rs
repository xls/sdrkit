//! The RS(63,12) code of JT65 over GF(64) in the symbol order JT65 sends, on the shared Reed-Solomon codec, and the 7 x 9
//! interleaver.
//! Standard: J. Taylor, "The JT65 Communications Protocol", QEX September/October 2005.

use sdrkit_algo_fec::rs::{codes::JT65_63_12, Rs};
use sdrkit_util_bits::interleave::{Permutation, ReadOrder};
use std::sync::OnceLock;

/// Symbols in a code word.
pub const N: usize = 63;
/// Data symbols.
pub const K: usize = 12;

fn codec() -> Option<Rs> {
    Rs::new(JT65_63_12).ok()
}

fn interleaver() -> &'static Option<Permutation> {
    static P: OnceLock<Option<Permutation>> = OnceLock::new();
    P.get_or_init(|| Permutation::row_column(9, 7, ReadOrder::Columns).ok())
}

/// The 63 symbols of 12 data symbols (each below 64) in the order JT65 sends them before interleaving: the 51 parity symbols,
/// highest degree last, then the 12 data symbols. All zero when a data symbol is 64 or more.
///
/// The JT65 word is the shared code word of the reversed data, read backwards.
pub fn encode(dat: &[u8; K]) -> [u8; N] {
    let mut out = [0u8; N];
    let Some(rs) = codec() else { return out };
    let mut word = [0u8; N];
    for (w, &d) in word[..K].iter_mut().zip(dat.iter().rev()) {
        *w = d;
    }
    let (data, parity) = word.split_at_mut(K);
    if rs.encode(data, parity).is_err() {
        return out;
    }
    for (o, &w) in out.iter_mut().zip(word.iter().rev()) {
        *o = w;
    }
    out
}

/// Decodes a received word (JT65 order) with `erasures` (JT65 positions known to be unreliable, each once). Returns the data,
/// the corrected code word and the number of symbols changed; `None` when the word cannot be decoded, a symbol is 64 or more,
/// or an erasure is out of range or repeated.
pub fn decode(received: &[u8; N], erasures: &[usize]) -> Option<([u8; K], [u8; N], usize)> {
    let mut rs = codec()?;
    let mut word = [0u8; N];
    for (w, &r) in word.iter_mut().zip(received.iter().rev()) {
        *w = r;
    }
    let mut er = [0u16; N];
    if erasures.len() > N {
        return None;
    }
    for (e, &x) in er.iter_mut().zip(erasures) {
        *e = (N - 1).checked_sub(x)? as u16;
    }
    let r = rs.decode(&mut word, &er[..erasures.len()]).ok()?;
    let mut cw = [0u8; N];
    for (c, &w) in cw.iter_mut().zip(word.iter().rev()) {
        *c = w;
    }
    let mut dat = [0u8; K];
    dat.copy_from_slice(&cw[N - K..]);
    Some((dat, cw, r.corrected as usize))
}

/// The JT65 interleaver: the code word written into 9 rows of 7 and read column by column.
pub fn interleave(cw: &[u8; N]) -> [u8; N] {
    let mut out = [0u8; N];
    if let Some(p) = interleaver() {
        let _ = p.apply(cw, &mut out);
    }
    out
}

/// Undoes [`interleave`].
pub fn deinterleave<T: Copy>(x: &[T; N]) -> [T; N] {
    let mut out = *x;
    if let Some(p) = interleaver() {
        let _ = p.invert(x, &mut out);
    }
    out
}
