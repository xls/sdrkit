//! What the stacks of single services say about themselves: the status writer.

use super::display::lock_state;
use sdrkit_core::Results;

/// The status sentence of a stack that no off-air signal has confirmed.
pub const EXPERIMENTAL: &str = "experimental: not confirmed on a real signal";

/// Writes the status: the lock state of `state` (0 searching, 1 and 2 tracking, 3 locked), the signal to noise ratio, the
/// level, the carrier offset and the summary.
pub fn set_status(
    r: &mut Results,
    state: u8,
    snr_db: Option<f32>,
    level_dbfs: Option<f32>,
    offset_hz: Option<f64>,
    summary: &str,
) {
    let lock_state = lock_state(state);
    r.update_status(|s| {
        let changed = s.lock_state != lock_state
            || s.snr_db != snr_db
            || s.level_dbfs != level_dbfs
            || s.offset_hz != offset_hz
            || s.summary != summary;
        s.lock_state = lock_state;
        s.snr_db = snr_db;
        s.level_dbfs = level_dbfs;
        s.offset_hz = offset_hz;
        if s.summary != summary {
            s.summary.clear();
            s.summary.push_str(summary);
        }
        changed
    });
}
