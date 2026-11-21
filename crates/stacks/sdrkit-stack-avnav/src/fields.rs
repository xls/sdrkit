//! What several navigation-aid stacks write: the identification with the table of words heard, the carrier, and the
//! status line.

use crate::carrier::CarrierInfo;
use crate::morse::IdentTracker;
use crate::typed::{CarrierFields, IdentFields, IdentRow, IDENTS};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::Published;
use sdrkit_core::LockState;

/// Writes the identification `ident`, the last word and the speed `wpm` of `tracker`, and the table of words heard. The
/// rows of words that the tracker has dropped are removed.
pub fn put_idents(
    p: &mut Published,
    tracker: &IdentTracker,
    ident: Option<&str>,
    wpm: Option<f32>,
) {
    p.put(&IdentFields {
        ident,
        last_ident: tracker.last(),
        ident_wpm: wpm.map(f64::from),
    });
    for (w, n) in tracker.words() {
        IDENTS.put(
            p,
            w,
            &IdentRow {
                ident: w,
                heard: i64::from(n),
            },
        );
    }
    let table = p
        .results()
        .decl()
        .table_id(IDENTS.key)
        .and_then(|id| p.results().table(id));
    if table.map_or(0, |t| t.len()) > tracker.words().count() {
        let gone: Vec<String> = table
            .into_iter()
            .flat_map(|t| t.rows_newest_first())
            .map(|(k, _)| k)
            .filter(|k| !tracker.words().any(|(w, _)| w == *k))
            .map(str::to_owned)
            .collect();
        for k in gone {
            p.remove_row(&IDENTS, &k);
        }
    }
}

/// The carrier fields of the measurement `c` of a station tuned at `tuned_hz`. With no measurement yet, the numbers
/// have no value.
pub fn carrier_fields(c: Option<&CarrierInfo>, tuned_hz: f64) -> CarrierFields {
    CarrierFields {
        carrier: c.is_some_and(|c| c.present),
        carrier_hz: c.map(|c| Hertz(tuned_hz + c.offset_hz)),
        cn0_dbhz: c.map(|c| f64::from(c.cn0_dbhz)),
        offset_hz: c.map(|c| Hertz(c.offset_hz)),
        level_dbfs: c.map(|c| f64::from(c.level_dbfs)),
    }
}

/// Writes the status line: the lock state, the carrier's offset, level and SNR in a channel `width_hz` wide (when a carrier is
/// measured), and `summary`. The status changes only when a part of it changes.
pub fn put_carrier_status(
    p: &mut Published,
    lock_state: LockState,
    carrier: Option<&CarrierInfo>,
    width_hz: f64,
    summary: &str,
) {
    put_status(
        p,
        lock_state,
        carrier.map(|c| c.cn0_dbhz - 10.0 * (width_hz as f32).log10()),
        carrier.map(|c| c.level_dbfs),
        carrier.map(|c| c.offset_hz),
        summary,
    );
}

/// Writes the status line. The status changes only when a part of it changes.
pub fn put_status(
    p: &mut Published,
    lock_state: LockState,
    snr_db: Option<f32>,
    level_dbfs: Option<f32>,
    offset_hz: Option<f64>,
    summary: &str,
) {
    p.results_mut().update_status(|s| {
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
            s.summary.clear();
            s.summary.push_str(summary);
        }
        changed
    });
}
