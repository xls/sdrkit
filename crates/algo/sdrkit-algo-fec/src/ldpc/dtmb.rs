//! The DTMB LDPC codes as quasi-cyclic layers of 127 checks, built from a parity-check matrix in quasi-cyclic form.
//!
//! Standard: GB 20600-2006, the LDPC code of the frame body. The matrices are in [`super::tables`].

use super::{Edge, InfoAt, LdpcCode, NO_MASK};
use crate::error::{Param, Result};

/// The circulant size.
const Z: usize = 127;
/// Column groups of 127 bits: 7493 bits in all.
const GROUPS: usize = 59;

impl LdpcCode {
    /// Bits deleted from the front of each 7493-bit DTMB code word, so that 7488 bits are sent. A receiver puts LLR 0 at code
    /// word positions 0 to 4 and the 7488 received values after them. A transmitter drops those five bits.
    pub const DTMB_DELETED: usize = 5;

    /// A DTMB LDPC code (GB 20600) from its parity-check matrix in quasi-cyclic form: `rows[i]` lists `(column group, shift)`
    /// for block row `i`, with 59 column groups of 127 bits (`n` = 7493). Each pair is a 127 x 127 circulant whose row `l`
    /// has its one at column `(l + shift) mod 127`. A circulant of weight 2 or more is given as several pairs with the same
    /// column group. 35, 23 or 11 block rows give rate 0.4, 0.6 or 0.8 (`k` = 3048, 4572 or 6096), and `info_at` says
    /// whether the information bits are the first or the last `k` of the code word.
    ///
    /// The code word holds all 7493 bits; see [`LdpcCode::DTMB_DELETED`] for the bits that are not sent.
    /// [`LdpcCode::encode`] is not available; encode with [`LdpcEncoder`](super::LdpcEncoder). An error when the number
    /// of block rows is not 35, 23 or 11, a column group is 59 or more, a shift is 127 or more, a pair repeats, or a block
    /// row is empty.
    pub fn dtmb(rows: &[&[(u16, u16)]], info_at: InfoAt) -> Result<Self> {
        let rate = match rows.len() {
            35 => "0.4",
            23 => "0.6",
            11 => "0.8",
            _ => return Param::Rows.not_allowed(rows.len()),
        };
        let mut layers = Vec::with_capacity(rows.len());
        for r in rows {
            let mut layer: Vec<Edge> = Vec::with_capacity(r.len());
            for &(c, s) in r.iter() {
                Param::Column.check(c, 0, GROUPS as i64 - 1)?;
                Param::Shift.check(s, 0, Z as i64 - 1)?;
                if layer.iter().any(|e| e.col == c as u32 && e.shift == s) {
                    return Param::Table.degenerate();
                }
                layer.push(Edge {
                    col: c as u32,
                    shift: s,
                    mask: NO_MASK,
                });
            }
            layers.push(layer);
        }
        let n = GROUPS * Z;
        let k = (GROUPS - rows.len()) * Z;
        Self::assemble(
            format!("dtmb {rate}"),
            n,
            k,
            Z,
            GROUPS,
            layers,
            Vec::new(),
            info_at,
        )
    }
}
