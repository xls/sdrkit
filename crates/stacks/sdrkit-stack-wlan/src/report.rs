//! Parts of the tables and reports of the stacks of this crate: the privacy note, address and hex text, the wall clock
//! of a live source and histories.

use sdrkit_core::publish::{Published, TableDef};
use sdrkit_core::Clock;

/// The privacy note of the BLE and Wi-Fi monitors.
pub const PRIVACY_NOTE: &str = "Receive only: the packets devices broadcast openly, as a scanner app sees them. Nothing is transmitted, connected to, paired with, joined or decrypted.";

/// The wall clock in seconds since 1970 when the samples are live, else `None` (a recording has no wall time of its own).
pub fn wall(live: bool, clock: &Clock) -> Option<f64> {
    live.then(|| clock.now())
}

/// Six bytes as upper-case hex pairs joined by colons, in the order given ("C0:FF:EE:12:34:56").
pub fn mac_text(b: &[u8]) -> String {
    let mut s = String::with_capacity(17);
    for (i, x) in b.iter().take(6).enumerate() {
        if i > 0 {
            s.push(':');
        }
        sdrkit_core::base64::hex_into(std::slice::from_ref(x), &mut s);
    }
    s
}

/// Bytes as upper-case hex without separators.
pub fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(2 * b.len());
    sdrkit_core::base64::hex_into(b, &mut s);
    s
}

/// Pushes `v` onto a history of at most `cap` values, oldest first.
pub fn push_history<T>(h: &mut Vec<T>, v: T, cap: usize) {
    if h.len() >= cap {
        h.remove(0);
    }
    h.push(v);
}

/// Removes the rows of table `def` other than the `written` newest. Call it after each row that must stay was written with
/// [`Published::row_mut`], which makes a row the newest. It allocates only when there is a row to remove.
pub fn remove_older_rows(p: &mut Published, def: &TableDef, written: usize) {
    let Some(id) = p.results().decl().table_id(def.key) else {
        return;
    };
    let Some(t) = p.results().table(id) else {
        return;
    };
    if t.len() <= written {
        return;
    }
    let stale: Vec<String> = t
        .rows_newest_first()
        .skip(written)
        .map(|(k, _)| k.to_owned())
        .collect();
    for k in &stale {
        p.remove_row(def, k);
    }
}
