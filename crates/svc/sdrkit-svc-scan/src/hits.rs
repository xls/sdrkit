//! The hit log: every stop of an activity scan (time, frequency, stack, name, length, tone, best SNR), bounded, as CSV, and
//! counted on the memories that the channels came from.

use sdrkit_core::HostRequest;
use sdrkit_util_text::csv;
use std::collections::VecDeque;

/// One stop.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// When activity started, seconds since 1970.
    pub start_s: f64,
    /// When the channel was left (or activity last seen, for a hit that ended by hang), seconds since 1970.
    pub end_s: f64,
    /// The frequency, Hz. For a span or survey find, the tuned frequency plus the carrier offset in the stack's status.
    pub freq_hz: f64,
    /// The stack id.
    pub stack: String,
    /// The channel's name.
    pub name: String,
    /// The tone heard while open, empty for none.
    pub tone: String,
    /// The best SNR seen, dB.
    pub peak_snr_db: Option<f32>,
    /// The channel was the priority channel.
    pub priority: bool,
    /// The memory the channel came from.
    pub memory_id: Option<u64>,
    /// The channel's index in the scan.
    pub index: usize,
    /// The pass of the scan in which it was found, from 0.
    pub pass: u32,
    /// A text that the host attached, such as the text that the stack decoded.
    pub text: String,
}

impl Hit {
    /// How long it lasted, seconds.
    pub fn duration_s(&self) -> f64 {
        (self.end_s - self.start_s).max(0.0)
    }
}

/// The most hits a log keeps by default.
pub const DEFAULT_HIT_CAP: usize = 1000;

/// The hits of a scan, oldest first, at most `cap` (the oldest go).
#[derive(Clone, Debug, PartialEq)]
pub struct HitLog {
    hits: VecDeque<Hit>,
    cap: usize,
    pushed: u64,
    revision: u64,
}

impl Default for HitLog {
    fn default() -> Self {
        HitLog::new(DEFAULT_HIT_CAP)
    }
}

impl HitLog {
    /// An empty log keeping at most `cap` hits.
    pub fn new(cap: usize) -> HitLog {
        HitLog {
            hits: VecDeque::new(),
            cap: cap.max(1),
            pushed: 0,
            revision: 0,
        }
    }

    /// Adds a hit.
    pub fn push(&mut self, h: Hit) {
        self.pushed += 1;
        self.revision += 1;
        self.hits.push_back(h);
        while self.hits.len() > self.cap {
            self.hits.pop_front();
        }
    }

    /// The hits, oldest first.
    pub fn hits(&self) -> impl DoubleEndedIterator<Item = &Hit> + ExactSizeIterator {
        self.hits.iter()
    }

    /// The number of hits pushed since the log was made, also those that the cap dropped.
    pub fn pushed(&self) -> u64 {
        self.pushed
    }

    /// A count that grows with every hit added or changed.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The hit with sequence number `seq` (0 for the first pushed), to change it, if the cap has not dropped it.
    pub fn get_mut(&mut self, seq: u64) -> Option<&mut Hit> {
        let first = self.pushed - self.hits.len() as u64;
        let i = usize::try_from(seq.checked_sub(first)?).ok()?;
        self.revision += 1;
        self.hits.get_mut(i)
    }

    /// The number of hits.
    pub fn len(&self) -> usize {
        self.hits.len()
    }

    /// True if there are no hits.
    pub fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }

    /// Forgets every hit.
    pub fn clear(&mut self) {
        self.hits.clear();
    }

    /// The hits as CSV: `start_s,end_s,duration_s,frequency_hz,mode,name,tone,peak_snr_db,priority`.
    pub fn to_csv(&self) -> String {
        let mut out = String::from(
            "start_s,end_s,duration_s,frequency_hz,mode,name,tone,peak_snr_db,priority\n",
        );
        for h in &self.hits {
            let row = [
                format!("{:.3}", h.start_s),
                format!("{:.3}", h.end_s),
                format!("{:.3}", h.duration_s()),
                h.freq_hz.to_string(),
                h.stack.clone(),
                h.name.clone(),
                h.tone.clone(),
                h.peak_snr_db.map(|s| format!("{s:.1}")).unwrap_or_default(),
                u8::from(h.priority).to_string(),
            ];
            out.push_str(&csv::record(&row, ','));
            out.push('\n');
        }
        out
    }

    /// The requests that count the hits on the memories they came from: one [`HostRequest::MemoryHit`] for each hit of a
    /// memory, with its start time and its tone.
    pub fn memory_hits(&self) -> impl Iterator<Item = HostRequest> + '_ {
        self.hits.iter().filter_map(|h| {
            Some(HostRequest::MemoryHit {
                memory: h.memory_id?.to_string(),
                at_s: h.start_s,
                tone: h.tone.clone(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(at: f64, id: Option<u64>) -> Hit {
        Hit {
            start_s: at,
            end_s: at + 4.0,
            freq_hz: 145.5e6,
            stack: "general".into(),
            name: "Calling, 2 m".into(),
            tone: "CTCSS 88.5 Hz".into(),
            peak_snr_db: Some(21.0),
            priority: false,
            memory_id: id,
            index: 0,
            pass: 0,
            text: String::new(),
        }
    }

    #[test]
    fn capped_csv_and_memory_hits() {
        let mut log = HitLog::new(2);
        log.push(hit(1.0, Some(7)));
        log.push(hit(2.0, Some(7)));
        log.push(hit(3.0, None));
        assert_eq!(log.len(), 2);
        assert_eq!(log.hits().next().unwrap().start_s, 2.0);
        let csv = log.to_csv();
        assert!(csv.contains("\"Calling, 2 m\""));
        assert!(csv.contains(",4.000,"));
        let hits: Vec<HostRequest> = log.memory_hits().collect();
        assert_eq!(
            hits,
            [HostRequest::MemoryHit {
                memory: "7".into(),
                at_s: 2.0,
                tone: "CTCSS 88.5 Hz".into(),
            }]
        );
    }
}
