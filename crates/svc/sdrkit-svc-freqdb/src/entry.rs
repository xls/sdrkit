//! One entry of the frequency database: its category, where it applies, and the frequencies it covers.

use sdrkit_core::{Choice, OwnerSetting};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

sdrkit_core::choice_enum! {
    /// What an entry is.
    #[derive(PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Category {
        /// Amateur calling channels and digital-mode frequencies.
        Amateur = ("amateur", "Amateur"),
        /// Propagation beacons.
        Beacon = ("beacon", "Beacon"),
        /// Satellite downlinks.
        Satellite = ("satellite", "Satellite"),
        /// Time and standard-frequency stations.
        Time = ("time", "Time"),
        /// Aeronautical frequencies.
        Aviation = ("aviation", "Aviation"),
        /// Maritime frequencies.
        Marine = ("marine", "Marine"),
        /// Licence-free personal radio channels.
        Personal = ("personal", "Personal"),
        /// Paging.
        Paging = ("paging", "Paging"),
        /// ISM and short-range devices.
        Ism = ("ism", "ISM"),
        /// Mesh and LoRa networks.
        Mesh = ("mesh", "Mesh"),
        /// Navigation (GNSS, radio beacons).
        Navigation = ("navigation", "Navigation"),
        /// Distress and emergency.
        Emergency = ("emergency", "Emergency"),
        /// Weather broadcasts and pictures.
        Weather = ("weather", "Weather"),
        /// Broadcast bands.
        Broadcast = ("broadcast", "Broadcast"),
        /// Radio astronomy and science.
        Science = ("science", "Science"),
        /// Anything else.
        Other = ("other", "Other"),
    }
}

impl Category {
    /// The key that files use (`"amateur"`).
    pub fn id(self) -> &'static str {
        match &Self::CHOICES[usize::from(self.index())].key {
            Cow::Borrowed(k) => k,
            Cow::Owned(_) => "",
        }
    }

    /// The category with key `id`, in any case. `None` if no category has it.
    pub fn from_id(id: &str) -> Option<Category> {
        let id = id.trim();
        Self::CHOICES
            .iter()
            .position(|c| c.key.eq_ignore_ascii_case(id))
            .and_then(|i| Category::from_index(u16::try_from(i).ok()?))
    }
}

/// One frequency.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Stable id: `b:` built-in, `p:` from a plan, `u:` the user's.
    pub id: String,
    /// Name.
    pub name: String,
    /// The frequency to tune, Hz (the lower edge of a range).
    pub hz: f64,
    /// The upper edge, Hz, when the entry is a range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_hz: Option<f64>,
    /// The bandwidth, Hz.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bandwidth_hz: Option<f64>,
    /// The id of the stack that decodes it ("general", "fm", "adsb", "ais"). Empty for none.
    #[serde(default)]
    pub stack: String,
    /// The submode of the stack (the demodulator of the general receiver: "usb", "nfm", "am", "cw").
    #[serde(default)]
    pub submode: String,
    /// What it is.
    pub category: Category,
    /// Free tags ("ft8", "calling", "distress").
    #[serde(default)]
    pub tags: Vec<String>,
    /// Where it applies, as tags that the host chooses; empty means everywhere.
    #[serde(default)]
    pub scope: Vec<String>,
    /// Notes.
    #[serde(default)]
    pub notes: String,
    /// Where it comes from (a regulation, a standard, a published list).
    #[serde(default)]
    pub source: String,
    /// Settings to apply when it is tuned, each with its owner.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub settings: Vec<OwnerSetting>,
}

impl Entry {
    /// An entry with no scope, tags or notes.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        hz: f64,
        category: Category,
    ) -> Entry {
        Entry {
            id: id.into(),
            name: name.into(),
            hz,
            end_hz: None,
            bandwidth_hz: None,
            stack: String::new(),
            submode: String::new(),
            category,
            tags: Vec::new(),
            scope: Vec::new(),
            notes: String::new(),
            source: String::new(),
            settings: Vec::new(),
        }
    }

    /// It is the user's.
    pub fn is_user(&self) -> bool {
        self.id.starts_with("u:")
    }

    /// The upper edge (the frequency itself for a point).
    pub fn upper_hz(&self) -> f64 {
        self.end_hz.unwrap_or(self.hz)
    }

    /// The centre: the middle of a range, the frequency of a point.
    pub fn centre_hz(&self) -> f64 {
        (self.hz + self.upper_hz()) / 2.0
    }

    /// Whether `hz` is on the entry: inside a range, or within `tolerance_hz` (at least half the bandwidth) of a point.
    pub fn covers(&self, hz: f64, tolerance_hz: f64) -> bool {
        match self.end_hz {
            Some(end) => hz >= self.hz - tolerance_hz && hz <= end + tolerance_hz,
            None => {
                let t = tolerance_hz.max(self.bandwidth_hz.unwrap_or(0.0) / 2.0);
                (hz - self.hz).abs() <= t
            }
        }
    }

    /// Whether it applies in one of the tags `scope`: always when it has no scope of its own or `scope` is empty, else
    /// when one of its tags is in `scope` (letter case is ignored).
    pub fn applies(&self, scope: &[String]) -> bool {
        self.scope.is_empty()
            || scope.is_empty()
            || self
                .scope
                .iter()
                .any(|t| scope.iter().any(|s| s.eq_ignore_ascii_case(t)))
    }

    /// The words a query matches: of the name, notes, tags, category, stack and submode, lower case.
    pub(crate) fn words(&self) -> Vec<String> {
        format!(
            "{} {} {} {} {} {}",
            self.name,
            self.notes,
            self.tags.join(" "),
            self.category.id(),
            self.stack,
            self.submode,
        )
        .to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '.' || c == '-' || c == '_'))
        .filter(|w| !w.is_empty())
        .map(String::from)
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_and_coverage() {
        let mut e = Entry::new("b:x", "2 m FM calling", 146.52e6, Category::Amateur);
        e.scope = vec!["US".into()];
        e.stack = "general".into();
        e.submode = "nfm".into();
        e.bandwidth_hz = Some(12_500.0);
        let scope = |t: &str| -> Vec<String> { t.split(' ').map(String::from).collect() };
        assert!(e.applies(&scope("us itu2")));
        assert!(!e.applies(&scope("CA itu2")));
        assert!(e.applies(&[]));
        assert!(!e.applies(&scope("itu1")));
        assert!(e.covers(146.525e6, 0.0));
        assert!(!e.covers(146.54e6, 0.0));
        let mut r = Entry::new("b:r", "Range", 100.0, Category::Other);
        r.end_hz = Some(200.0);
        assert!(r.covers(150.0, 0.0) && !r.covers(250.0, 10.0));
        assert_eq!(r.centre_hz(), 150.0);
        assert_eq!(Category::from_id("Marine"), Some(Category::Marine));
        assert_eq!(Category::from_id("zzz"), None);
        assert_eq!(Category::Ism.id(), "ism");
    }
}
