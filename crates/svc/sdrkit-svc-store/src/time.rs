//! Dates and times as text for files and exports.

use sdrkit_util_geo::time::{iso8601, split_f64, Civil};

/// The date and time of `unix_s`, `offset_min` minutes ahead of UTC. `None` if `unix_s` is out of range.
pub(crate) fn civil(unix_s: f64, offset_min: i32) -> Option<Civil> {
    let (secs, _) = split_f64(unix_s).ok()?;
    Civil::from_unix_local(secs, 0, offset_min).ok()
}

/// `YYYY-MM-DD` of `unix_s`, `offset_min` minutes ahead of UTC. `None` if `unix_s` is out of range.
pub(crate) fn date(unix_s: f64, offset_min: i32) -> Option<String> {
    civil(unix_s, offset_min).map(|c| format!("{:04}-{:02}-{:02}", c.year, c.month, c.day))
}

/// `unix_s` as ISO 8601 UTC with milliseconds (`2026-11-08T10:42:07.250Z`). Empty if `unix_s` is out of range.
pub(crate) fn iso(unix_s: f64) -> String {
    iso8601(unix_s, 3)
        .map(|t| t.as_str().to_owned())
        .unwrap_or_default()
}
