//! What an activity scan visits: a scan list (entries of frequency, stack, name, dwell, lockout and priority, kept as CSV), a
//! band search (start, stop, step), or a list made from the memories.

use crate::error::{LineProblem, ScanError, ScanProblem};
use crate::host::Target;
use sdrkit_core::OwnerSetting;
use sdrkit_svc_memory::freq::{self, UnitHint};
use sdrkit_svc_memory::Memory;
use sdrkit_util_text::csv;
use serde::{Deserialize, Serialize};

/// One channel of a scan list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanEntry {
    /// The frequency, Hz.
    pub freq_hz: f64,
    /// The stack id. Empty for the running stack.
    pub stack: String,
    /// The submode of the stack (the demodulator of the general receiver). Empty for its default.
    pub submode: String,
    /// The name shown.
    pub name: String,
    /// How long to listen, seconds. `None` for the dwell of the scan.
    pub dwell_s: Option<f64>,
    /// The scan passes it by.
    pub lockout: bool,
    /// The priority channel (the first one marked is used).
    pub priority: bool,
    /// The settings for this channel, each with its owner.
    pub settings: Vec<OwnerSetting>,
    /// The channel bandwidth, Hz.
    pub bandwidth_hz: Option<f64>,
    /// The memory it came from, for writing hits back.
    pub memory_id: Option<u64>,
    /// The hits counted on it.
    pub hits: u64,
    /// The time of the last hit, seconds since 1970.
    pub last_hit_s: Option<f64>,
    /// The last tone heard on it.
    pub tone: String,
}

impl Default for ScanEntry {
    fn default() -> Self {
        ScanEntry {
            freq_hz: 0.0,
            stack: String::new(),
            submode: String::new(),
            name: String::new(),
            dwell_s: None,
            lockout: false,
            priority: false,
            settings: Vec::new(),
            bandwidth_hz: None,
            memory_id: None,
            hits: 0,
            last_hit_s: None,
            tone: String::new(),
        }
    }
}

impl ScanEntry {
    /// An entry at `freq_hz` in `stack`, named `name`.
    pub fn new(freq_hz: f64, stack: &str, name: &str) -> ScanEntry {
        ScanEntry {
            freq_hz,
            stack: stack.into(),
            name: name.into(),
            ..ScanEntry::default()
        }
    }

    /// The entry of a memory, with its settings and gains.
    pub fn from_memory(m: &Memory) -> ScanEntry {
        ScanEntry {
            freq_hz: m.freq_hz,
            stack: m.stack.clone(),
            submode: m.submode.clone(),
            name: m.label(),
            dwell_s: m.dwell_s,
            lockout: m.lockout,
            priority: m.priority,
            settings: m.settings.iter().chain(&m.gains).cloned().collect(),
            bandwidth_hz: m.bandwidth_hz,
            memory_id: Some(m.id),
            hits: m.hits,
            last_hit_s: m.last_hit_s,
            tone: m.tone.clone(),
        }
    }
}

/// The CSV columns of a scan list. The settings cell is a JSON list of settings with their owners and typed values.
pub const LIST_COLUMNS: [&str; 7] = [
    "frequency_hz",
    "mode",
    "name",
    "dwell_s",
    "lockout",
    "priority",
    "settings",
];

/// A scan list of the user.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanList {
    /// The entries in scan order.
    pub entries: Vec<ScanEntry>,
}

impl ScanList {
    /// The `memories`, in the manual order of the store. Locked-out memories stay in, marked, so that the user sees them.
    /// The scan passes them by.
    pub fn from_memories(mut memories: Vec<Memory>) -> ScanList {
        memories.sort_by_key(|m| m.order);
        ScanList {
            entries: memories.iter().map(ScanEntry::from_memory).collect(),
        }
    }

    /// The list as CSV with `LIST_COLUMNS`.
    pub fn to_csv(&self) -> String {
        let mut out = csv::record(&LIST_COLUMNS, ',');
        out.push('\n');
        for e in &self.entries {
            let settings = if e.settings.is_empty() {
                String::new()
            } else {
                serde_json::to_string(&e.settings).unwrap_or_default()
            };
            let row = [
                e.freq_hz.to_string(),
                e.stack.clone(),
                e.name.clone(),
                e.dwell_s.map(|d| d.to_string()).unwrap_or_default(),
                u8::from(e.lockout).to_string(),
                u8::from(e.priority).to_string(),
                settings,
            ];
            out.push_str(&csv::record(&row, ','));
            out.push('\n');
        }
        out
    }

    /// Reads CSV in the layout of [`ScanList::to_csv`]. The header is optional. A frequency can carry a unit. Returns the list
    /// and the lines that could not be read.
    pub fn from_csv(text: &str) -> (ScanList, Vec<LineProblem>) {
        let d = csv::sniff_delimiter(text);
        let mut list = ScanList::default();
        let mut problems = Vec::new();
        for (line, row) in csv::read(text, d) {
            match entry_of(&row) {
                Ok(e) => list.entries.push(e),
                // The first line can be the header.
                Err(ScanProblem::NotAFrequency) if line == 1 => {}
                Err(kind) => problems.push(LineProblem { line, kind }),
            }
        }
        (list, problems)
    }
}

fn entry_of(row: &[String]) -> Result<ScanEntry, ScanProblem> {
    let cell = |i: usize| row.get(i).map_or("", |s| s.trim());
    let hz = freq::parse_hz(cell(0), UnitHint::Hz).ok_or(ScanProblem::NotAFrequency)?;
    let dwell_s = match cell(3) {
        "" => None,
        t => match t.parse::<f64>() {
            Ok(v) if v > 0.0 => Some(v),
            _ => return Err(ScanProblem::NotADwell),
        },
    };
    let flag = |t: &str| matches!(t.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "y");
    let settings = match cell(6) {
        "" => Vec::new(),
        t => serde_json::from_str(t).map_err(|_| ScanProblem::NotSettings)?,
    };
    Ok(ScanEntry {
        freq_hz: hz,
        stack: cell(1).into(),
        name: cell(2).into(),
        dwell_s,
        lockout: flag(cell(4)),
        priority: flag(cell(5)),
        settings,
        ..ScanEntry::default()
    })
}

/// A band search: every `step_hz` from `start_hz` to `stop_hz` (inclusive, within a hertz), in one stack.
#[derive(Clone, Debug, PartialEq)]
pub struct BandSearch {
    start_hz: f64,
    stop_hz: f64,
    step_hz: f64,
    stack: String,
    /// The settings for every step, each with its owner.
    pub settings: Vec<OwnerSetting>,
    /// The channel bandwidth, Hz.
    pub bandwidth_hz: Option<f64>,
}

/// The most steps a band search can have.
pub const MAX_BAND_STEPS: usize = 10_000_000;

impl BandSearch {
    /// A band search from `start_hz` to `stop_hz` in steps of `step_hz`, in `stack`.
    ///
    /// # Errors
    ///
    /// [`ScanError::BadBand`] if the start is not above 0, the stop is below the start, the step is below 1 Hz, or there are
    /// more than `MAX_BAND_STEPS` steps.
    pub fn new(
        start_hz: f64,
        stop_hz: f64,
        step_hz: f64,
        stack: &str,
    ) -> Result<BandSearch, ScanError> {
        let good = start_hz.is_finite()
            && stop_hz.is_finite()
            && start_hz > 0.0
            && stop_hz >= start_hz
            && step_hz.is_finite()
            && step_hz >= 1.0;
        let b = BandSearch {
            start_hz,
            stop_hz,
            step_hz,
            stack: stack.into(),
            settings: Vec::new(),
            bandwidth_hz: None,
        };
        if !good || b.len() > MAX_BAND_STEPS {
            return Err(ScanError::BadBand);
        }
        Ok(b)
    }

    /// The number of steps.
    pub fn len(&self) -> usize {
        ((self.stop_hz - self.start_hz) / self.step_hz + 1e-6).floor() as usize + 1
    }

    /// Always false: a band has at least its start.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The frequency of step `i`.
    pub fn freq(&self, i: usize) -> f64 {
        self.start_hz + i as f64 * self.step_hz
    }
}

/// The channels of an activity scan: a list or a band.
#[derive(Clone, Debug, PartialEq)]
pub enum Channels {
    /// A list (a scan list, or memories).
    List(Vec<ScanEntry>),
    /// A band search.
    Band(BandSearch),
}

impl Channels {
    /// The number of channels.
    pub fn len(&self) -> usize {
        match self {
            Channels::List(entries) => entries.len(),
            Channels::Band(band) => band.len(),
        }
    }

    /// True if there are no channels.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The frequency of channel `i`.
    pub fn freq(&self, i: usize) -> f64 {
        match self {
            Channels::List(entries) => entries.get(i).map_or(0.0, |e| e.freq_hz),
            Channels::Band(band) => band.freq(i),
        }
    }

    /// The entry of a list. `None` for a step of a band.
    pub fn entry(&self, i: usize) -> Option<&ScanEntry> {
        match self {
            Channels::List(entries) => entries.get(i),
            Channels::Band(_) => None,
        }
    }

    /// The entry of a list, to change.
    pub fn entry_mut(&mut self, i: usize) -> Option<&mut ScanEntry> {
        match self {
            Channels::List(entries) => entries.get_mut(i),
            Channels::Band(_) => None,
        }
    }

    /// The channel `i` to tune.
    pub fn target(&self, i: usize) -> Target<'_> {
        match self {
            Channels::List(entries) => match entries.get(i) {
                Some(e) => Target {
                    freq_hz: e.freq_hz,
                    stack: &e.stack,
                    submode: &e.submode,
                    name: &e.name,
                    settings: &e.settings,
                    bandwidth_hz: e.bandwidth_hz,
                    rate_hz: None,
                },
                None => Target {
                    freq_hz: 0.0,
                    stack: "",
                    submode: "",
                    name: "",
                    settings: &[],
                    bandwidth_hz: None,
                    rate_hz: None,
                },
            },
            Channels::Band(band) => Target {
                freq_hz: band.freq(i),
                stack: &band.stack,
                submode: "",
                name: "",
                settings: &band.settings,
                bandwidth_hz: band.bandwidth_hz,
                rate_hz: None,
            },
        }
    }

    /// The name of channel `i` for a hit: the name of the entry, or the frequency.
    pub fn name(&self, i: usize) -> String {
        match self.entry(i) {
            Some(e) if !e.name.is_empty() => e.name.clone(),
            _ => freq::format(self.freq(i)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::{OwnerKey, Value};
    use sdrkit_svc_memory::{MemoryFilter, MemoryStore};

    #[test]
    fn list_csv_round_trip() {
        let mut a = ScanEntry::new(118.1e6, "general", "Tower, main");
        a.dwell_s = Some(3.0);
        a.priority = true;
        a.settings.push(OwnerSetting {
            owner: OwnerKey::of("stack.general"),
            key: "demod".into(),
            value: Value::Choice(2),
        });
        let mut b = ScanEntry::new(145.5e6, "general", "Calling");
        b.lockout = true;
        let list = ScanList {
            entries: vec![a, b],
        };
        let (back, problems) = ScanList::from_csv(&list.to_csv());
        assert!(problems.is_empty());
        assert_eq!(back, list);
        let (l, problems) =
            ScanList::from_csv("145.5 MHz,general,A\nbad,general,B\n100000000,,C,-1\n");
        assert_eq!(l.entries.len(), 1);
        assert_eq!(l.entries[0].freq_hz, 145.5e6);
        assert_eq!(
            problems,
            vec![
                LineProblem {
                    line: 2,
                    kind: ScanProblem::NotAFrequency
                },
                LineProblem {
                    line: 3,
                    kind: ScanProblem::NotADwell
                }
            ]
        );
    }

    #[test]
    fn band_steps() {
        let b = BandSearch::new(144e6, 146e6, 12.5e3, "general").unwrap();
        assert_eq!(b.len(), 161);
        assert_eq!(b.freq(160), 146e6);
        assert_eq!(
            BandSearch::new(146e6, 144e6, 1e3, ""),
            Err(ScanError::BadBand)
        );
        assert!(BandSearch::new(1e6, 2e9, 1.0, "").is_err());
        assert!(BandSearch::new(1e6, 2e6, 0.0, "").is_err());
        let c = Channels::Band(b);
        assert_eq!(c.name(0), "144.000 MHz");
        assert_eq!(c.target(1).freq_hz, 144.0125e6);
    }

    #[test]
    fn memories_become_a_list_in_manual_order() {
        let mut s = MemoryStore::new();
        let mut a = Memory::new("A", 100e6, "general");
        a.submode = "nfm".into();
        a.gains.push(OwnerSetting {
            owner: OwnerKey::of("radio"),
            key: "lna".into(),
            value: Value::F64(20.0),
        });
        let a = s.insert(a, 0.0).unwrap();
        let mut b = Memory::new("B", 200e6, "general");
        b.scan = false;
        s.insert(b, 0.0).unwrap();
        let c = s.insert(Memory::new("C", 300e6, "fm"), 0.0).unwrap();
        s.move_to(c, 0).unwrap();
        let l = ScanList::from_memories(
            s.filter(&MemoryFilter::InScan)
                .into_iter()
                .cloned()
                .collect(),
        );
        let names: Vec<&str> = l.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["C", "A"]);
        assert_eq!(l.entries[1].memory_id, Some(a));
        assert_eq!(l.entries[1].submode, "nfm");
        assert_eq!(l.entries[1].settings[0].key, "lna");
    }
}
