//! Scan tables: what the channel scans of a stack found (frequency, channel, name, lock, SNR, level, first and last seen, how
//! often), merged across scans, exported as CSV, and turned into memories.

use crate::hits::HitLog;
use crate::lock::EmptyReason;
use sdrkit_core::{OwnerSetting, Value};
use sdrkit_svc_memory::Memory;
use sdrkit_util_text::csv;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// One channel found (or checked).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanRow {
    /// The frequency, Hz.
    pub freq_hz: f64,
    /// The stack id.
    pub stack: String,
    /// The plan's channel name, empty for none.
    pub channel: String,
    /// The station, ensemble or multiplex name.
    pub name: String,
    /// The stack locked here.
    pub locked: bool,
    /// The best SNR, dB.
    pub snr_db: Option<f32>,
    /// The level, dBFS.
    pub level_dbfs: Option<f32>,
    /// First found, seconds since 1970.
    pub first_seen_s: f64,
    /// Last found, seconds since 1970.
    pub last_seen_s: f64,
    /// How many scans (or stops) found it.
    pub seen: u32,
    /// Why it was empty. `None` for a station.
    pub empty: Option<EmptyReason>,
    /// The settings used for it (the width of a TV channel), so that a memory recalls it the same way.
    pub settings: Vec<OwnerSetting>,
    /// The result fields that the lock profile of the stack keeps (PI, services, ships, tone), by key, typed.
    pub values: Vec<(String, Value)>,
}

impl ScanRow {
    /// The value of kept field `key`.
    pub fn value(&self, key: &str) -> Option<&Value> {
        self.values.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

/// The key of an empty reason, as files write it.
fn empty_key(e: EmptyReason) -> String {
    let i = usize::from(sdrkit_core::Choice::index(e));
    <EmptyReason as sdrkit_core::Choice>::CHOICES
        .get(i)
        .map(|c| c.key.to_string())
        .unwrap_or_default()
}

/// Puts the values of `from` into `into`, replacing those of the same key.
fn merge_values<T: PartialEq>(into: &mut Vec<(T, Value)>, from: Vec<(T, Value)>) {
    for (k, v) in from {
        match into.iter_mut().find(|(o, _)| *o == k) {
            Some(slot) => slot.1 = v,
            None => into.push((k, v)),
        }
    }
}

/// A scan's results.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanTable {
    /// The rows, lowest frequency first.
    pub rows: Vec<ScanRow>,
}

/// Two rows closer than this in the same stack are the same channel, Hz.
const SAME_HZ: f64 = 1.0;

impl ScanTable {
    /// Adds a row, or merges it into the row of the same stack and frequency. The name, lock, SNR, level and reason change if
    /// the new row has them, the kept values and settings merge, `last_seen_s` moves on, `seen` adds up, and `first_seen_s`
    /// stays.
    pub fn upsert(&mut self, row: ScanRow) {
        if let Some(r) = self
            .rows
            .iter_mut()
            .find(|r| r.stack == row.stack && (r.freq_hz - row.freq_hz).abs() < SAME_HZ)
        {
            if !row.name.is_empty() {
                r.name = row.name;
            }
            if !row.channel.is_empty() {
                r.channel = row.channel;
            }
            r.locked = row.locked;
            if row.snr_db.is_some() {
                r.snr_db = row.snr_db;
            }
            if row.level_dbfs.is_some() {
                r.level_dbfs = row.level_dbfs;
            }
            r.empty = row.empty;
            r.last_seen_s = r.last_seen_s.max(row.last_seen_s);
            if row.first_seen_s > 0.0
                && (r.first_seen_s <= 0.0 || row.first_seen_s < r.first_seen_s)
            {
                r.first_seen_s = row.first_seen_s;
            }
            r.seen += row.seen.max(1);
            for st in row.settings {
                match r
                    .settings
                    .iter_mut()
                    .find(|o| o.owner == st.owner && o.key == st.key)
                {
                    Some(o) => o.value = st.value,
                    None => r.settings.push(st),
                }
            }
            merge_values(&mut r.values, row.values);
            return;
        }
        let mut row = row;
        row.seen = row.seen.max(1);
        let at = self.rows.partition_point(|r| {
            r.freq_hz < row.freq_hz || (r.freq_hz == row.freq_hz && r.stack < row.stack)
        });
        self.rows.insert(at, row);
    }

    /// Merges another table into this one.
    pub fn merge(&mut self, other: ScanTable) {
        for r in other.rows {
            self.upsert(r);
        }
    }

    /// The rows where the stack locked.
    pub fn locked(&self) -> impl Iterator<Item = &ScanRow> {
        self.rows.iter().filter(|r| r.locked)
    }

    /// A table of an activity scan's stops: one row per channel, seen once per hit.
    pub fn from_hits(hits: &HitLog) -> ScanTable {
        let mut t = ScanTable::default();
        for h in hits.hits() {
            let mut values = Vec::new();
            if !h.tone.is_empty() {
                values.push(("tone".to_string(), Value::Text(h.tone.clone())));
            }
            t.upsert(ScanRow {
                freq_hz: h.freq_hz,
                stack: h.stack.clone(),
                name: h.name.clone(),
                locked: true,
                snr_db: h.peak_snr_db,
                first_seen_s: h.start_s,
                last_seen_s: h.end_s,
                seen: 1,
                values,
                ..ScanRow::default()
            });
        }
        t
    }

    fn value_columns(&self) -> Vec<String> {
        let set: BTreeSet<&String> = self
            .rows
            .iter()
            .flat_map(|r| r.values.iter().map(|(k, _)| k))
            .collect();
        set.into_iter().cloned().collect()
    }

    /// The table as CSV: `frequency_hz,mode,channel,name,locked,snr_db,level_dbfs,first_seen_s,last_seen_s,seen,note`, then
    /// one column for each key of the kept values.
    pub fn to_csv(&self) -> String {
        let extra = self.value_columns();
        let mut head: Vec<String> = [
            "frequency_hz",
            "mode",
            "channel",
            "name",
            "locked",
            "snr_db",
            "level_dbfs",
            "first_seen_s",
            "last_seen_s",
            "seen",
            "note",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        head.extend(extra.iter().cloned());
        let mut out = csv::record(&head, ',');
        out.push('\n');
        let opt = |v: Option<f32>| v.map(|v| format!("{v:.1}")).unwrap_or_default();
        for r in &self.rows {
            let mut row = vec![
                r.freq_hz.to_string(),
                r.stack.clone(),
                r.channel.clone(),
                r.name.clone(),
                u8::from(r.locked).to_string(),
                opt(r.snr_db),
                opt(r.level_dbfs),
                format!("{:.0}", r.first_seen_s),
                format!("{:.0}", r.last_seen_s),
                r.seen.to_string(),
                r.empty.map(empty_key).unwrap_or_default(),
            ];
            row.extend(extra.iter().map(|k| {
                r.value(k)
                    .map(|v| v.display(&[]).to_string())
                    .unwrap_or_default()
            }));
            out.push_str(&csv::record(&row, ','));
            out.push('\n');
        }
        out
    }

    /// The locked rows (or all with `all`) as memories, named by the station (or the channel), in the scanned group `group`.
    pub fn to_memories(&self, group: &str, all: bool) -> Vec<Memory> {
        self.rows
            .iter()
            .filter(|r| all || r.locked)
            .map(|r| {
                let name = if !r.name.is_empty() {
                    r.name.clone()
                } else {
                    r.channel.clone()
                };
                let mut m = Memory::new(&name, r.freq_hz, &r.stack);
                m.settings = r.settings.clone();
                m.group = group.into();
                if !r.channel.is_empty() && r.channel != name {
                    m.notes = format!("Channel {}", r.channel);
                }
                if let Some(Value::Text(t)) = r.value("tone") {
                    m.tone = t.clone();
                }
                m
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hits::Hit;
    use sdrkit_core::OwnerKey;

    fn row(hz: f64, name: &str, locked: bool, at: f64) -> ScanRow {
        ScanRow {
            freq_hz: hz,
            stack: "dab".into(),
            channel: "12C".into(),
            name: name.into(),
            locked,
            snr_db: Some(12.0),
            first_seen_s: at,
            last_seen_s: at,
            ..ScanRow::default()
        }
    }

    #[test]
    fn merge_keeps_first_seen_and_counts() {
        let mut t = ScanTable::default();
        t.upsert(row(227.36e6, "Mux A", true, 100.0));
        t.upsert(row(225.648e6, "", false, 100.0));
        let mut again = row(227.36e6, "", true, 200.0);
        again.values.push(("services".into(), Value::I64(12)));
        t.upsert(again);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0].freq_hz, 225.648e6, "sorted by frequency");
        let r = &t.rows[1];
        assert_eq!(
            (r.name.as_str(), r.seen, r.first_seen_s, r.last_seen_s),
            ("Mux A", 2, 100.0, 200.0)
        );
        assert_eq!(t.locked().count(), 1);
        let csv = t.to_csv();
        assert!(csv.lines().next().unwrap().ends_with(",note,services"));
        assert!(csv.contains(",12\n"));
    }

    #[test]
    fn saved_form_round_trip() {
        let mut t = ScanTable::default();
        let mut r = row(227.36e6, "Mux A", true, 100.0);
        r.empty = Some(EmptyReason::NoLock);
        r.values.push(("snr_db".into(), Value::F64(14.0)));
        t.upsert(r);
        let back: ScanTable = serde_json::from_slice(&serde_json::to_vec(&t).unwrap()).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn rows_become_memories() {
        let mut t = ScanTable::default();
        let mut r = row(227.36e6, "Mux A", true, 1.0);
        r.settings.push(OwnerSetting {
            owner: OwnerKey::of("stack.dab"),
            key: "width".into(),
            value: Value::Hz(1_536_000.0),
        });
        t.upsert(r);
        t.upsert(row(225.648e6, "", false, 1.0));
        let m = t.to_memories("DAB scan", false);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].name, "Mux A");
        assert_eq!(m[0].group, "DAB scan");
        assert_eq!(m[0].settings[0].value, Value::Hz(1_536_000.0));
        assert_eq!(m[0].notes, "Channel 12C");
        assert_eq!(t.to_memories("x", true)[0].name, "12C");
    }

    #[test]
    fn from_hits() {
        let mut log = HitLog::default();
        for at in [10.0, 30.0] {
            log.push(Hit {
                start_s: at,
                end_s: at + 5.0,
                freq_hz: 145.5e6,
                stack: "general".into(),
                name: "Calling".into(),
                tone: "CTCSS 88.5 Hz".into(),
                peak_snr_db: Some(20.0),
                priority: false,
                memory_id: None,
                index: 0,
                pass: 0,
                text: String::new(),
            });
        }
        let t = ScanTable::from_hits(&log);
        assert_eq!(t.rows.len(), 1);
        assert_eq!(
            (
                t.rows[0].seen,
                t.rows[0].first_seen_s,
                t.rows[0].last_seen_s
            ),
            (2, 10.0, 35.0)
        );
        assert_eq!(t.to_memories("hits", false)[0].tone, "CTCSS 88.5 Hz");
    }
}
