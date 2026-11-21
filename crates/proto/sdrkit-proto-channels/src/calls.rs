//! The call bookkeeping: a call is a channel that opened, from its first open block to its last, and each channel has its
//! statistics.

use crate::items::ChanBlock;
use crate::{AUDIO_RATE, BLOCK};
use sdrkit_util_objects::{Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

/// One call.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Call {
    /// Its number since the start.
    pub serial: u64,
    /// The channel key.
    pub key: i64,
    /// The channel label.
    pub label: String,
    /// The channel number of the plan, 0 for none.
    pub number: u32,
    /// The frequency, Hz.
    pub freq_hz: f64,
    /// When it started.
    pub at: Stamp,
    /// How long it lasted, s.
    pub duration_s: f64,
    /// The highest block level, dBFS.
    pub peak_dbfs: f64,
    /// The mean block level, dBFS.
    pub mean_dbfs: f64,
    /// The channel is still open.
    pub open: bool,
    /// The demodulator in use at its end.
    pub demod: String,
    /// The last tone or code heard during the call.
    pub tone: String,
    /// The file that holds its recording.
    pub file: String,
    #[serde(skip)]
    last_active: f64,
    #[serde(skip)]
    level_sum: f64,
    #[serde(skip)]
    level_n: u64,
}

impl Call {
    /// The signal time of its last open sample, s.
    pub fn last_active(&self) -> f64 {
        self.last_active
    }
}

impl Shift for Call {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
        self.last_active += by_s;
    }
}

/// The statistics of one channel.
#[derive(Default, Clone, Debug)]
pub struct ChannelStats {
    /// Calls so far.
    pub calls: u64,
    /// The time of the ended calls, s.
    pub total: f64,
    /// The signal time of the last open block, s.
    pub last: Option<f64>,
    /// The highest open block level, dBFS.
    pub peak: Option<f64>,
    /// The level of the last block, dBFS.
    pub level: Option<f64>,
    /// The squelch is open.
    pub open: bool,
}

/// What one block did to the calls.
#[derive(Debug, Default)]
pub struct CallStep {
    /// A call started with this block.
    pub started: Option<Call>,
    /// The call of the channel ended with this block.
    pub ended: Option<Call>,
}

/// The open calls and the statistics of every channel heard.
#[derive(Debug)]
pub struct CallBook {
    /// A call ends when its channel has been closed this long, s.
    pub hang_s: f64,
    open: BTreeMap<i64, Call>,
    rows: BTreeMap<i64, ChannelStats>,
}

impl Default for CallBook {
    fn default() -> Self {
        CallBook {
            hang_s: 1.0,
            open: BTreeMap::new(),
            rows: BTreeMap::new(),
        }
    }
}

impl CallBook {
    /// Takes block `b` of signal time `time`. A call that starts gets the next number of `serial`, the stamp `at` and what
    /// `describe` sets (label and number).
    pub fn block(
        &mut self,
        b: &ChanBlock,
        time: f64,
        serial: &mut u64,
        at: impl FnOnce() -> Stamp,
        describe: impl FnOnce(&mut Call),
    ) -> CallStep {
        let k = b.key;
        let mut step = CallStep::default();
        let row = self.rows.entry(k).or_default();
        row.level = Some(b.level_dbfs);
        row.open = b.open;
        if b.open {
            row.last = Some(time);
            row.peak = Some(row.peak.map_or(b.level_dbfs, |p: f64| p.max(b.level_dbfs)));
            if let Entry::Vacant(e) = self.open.entry(k) {
                *serial += 1;
                let mut c = Call {
                    serial: *serial,
                    key: k,
                    freq_hz: b.freq_hz,
                    at: at(),
                    open: true,
                    last_active: time,
                    peak_dbfs: b.level_dbfs,
                    ..Default::default()
                };
                describe(&mut c);
                row.calls += 1;
                step.started = Some(c.clone());
                e.insert(c);
            }
            if let Some(c) = self.open.get_mut(&k) {
                c.last_active = time + BLOCK as f64 / AUDIO_RATE;
                c.peak_dbfs = c.peak_dbfs.max(b.level_dbfs);
                c.level_sum += b.level_dbfs;
                c.level_n += 1;
                c.duration_s = c.last_active - c.at.t;
            }
        }
        if let Some(c) = self.open.get(&k) {
            if !b.open && time - c.last_active >= self.hang_s {
                step.ended = self.end(k);
            }
        }
        step
    }

    /// Ends the open call of channel `key` and returns it.
    pub fn end(&mut self, key: i64) -> Option<Call> {
        let mut c = self.open.remove(&key)?;
        c.open = false;
        c.duration_s = (c.last_active - c.at.t).max(0.0);
        c.mean_dbfs = if c.level_n > 0 {
            c.level_sum / c.level_n as f64
        } else {
            c.peak_dbfs
        };
        self.rows.entry(key).or_default().total += c.duration_s;
        Some(c)
    }

    /// Ends every open call, in key order.
    pub fn end_all(&mut self) -> Vec<Call> {
        let ks: Vec<i64> = self.open.keys().copied().collect();
        ks.into_iter().filter_map(|k| self.end(k)).collect()
    }

    /// Forgets the open calls and the statistics.
    pub fn clear(&mut self) {
        self.open.clear();
        self.rows.clear();
    }

    /// Forgets the statistics and the open call of channel `key`.
    pub fn forget(&mut self, key: i64) {
        self.open.remove(&key);
        self.rows.remove(&key);
    }

    /// The statistics of channel `key`.
    pub fn stats(&self, key: i64) -> Option<&ChannelStats> {
        self.rows.get(&key)
    }

    /// The statistics of every channel heard, in key order.
    pub fn all_stats(&self) -> impl Iterator<Item = (i64, &ChannelStats)> {
        self.rows.iter().map(|(k, s)| (*k, s))
    }

    /// The open call of channel `key`.
    pub fn open_call(&self, key: i64) -> Option<&Call> {
        self.open.get(&key)
    }

    /// The open call of channel `key`, to set its demodulator, tone or file.
    pub fn open_call_mut(&mut self, key: i64) -> Option<&mut Call> {
        self.open.get_mut(&key)
    }

    /// A squelch is open on some channel.
    pub fn any_open(&self) -> bool {
        self.rows.values().any(|r| r.open)
    }
}
