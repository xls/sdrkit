//! One memory channel: where to tune, in which stack, with which settings, and how the user organised it.

use crate::error::{MemoryError, Problem};
use sdrkit_core::{OwnerKey, OwnerSetting, Value};
use serde::{Deserialize, Serialize};

/// A stored channel. Every field has a default, and unknown fields in a file are ignored, so files written by newer or older versions
/// load.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Memory {
    /// Unique in its store, given by the store; 0 before it is stored.
    pub id: u64,
    /// The name shown (the name of a station if its stack returned one).
    pub name: String,
    /// The frequency, Hz.
    pub freq_hz: f64,
    /// The stack id (`"fm"`, `"general"`, `"dab"`). Empty keeps the current stack.
    pub stack: String,
    /// The demodulator of the general receiver (`"nfm"`). Empty for none.
    pub submode: String,
    /// The sample rate to use, Hz; `None` keeps the current one.
    pub rate_hz: Option<f64>,
    /// The channel bandwidth, Hz. `None` keeps the bandwidth of the stack.
    pub bandwidth_hz: Option<f64>,
    /// The gains of the radio, each a setting of the radio's owner.
    pub gains: Vec<OwnerSetting>,
    /// The settings of the stack, each with its owner.
    pub settings: Vec<OwnerSetting>,
    /// The group (folder) it belongs to; empty for none.
    pub group: String,
    /// Free tags.
    pub tags: Vec<String>,
    /// Free notes.
    pub notes: String,
    /// A colour as `#rrggbb`; empty for none.
    pub color: String,
    /// Starred memories sort on top.
    pub starred: bool,
    /// Included in a memory scan ("include in scan").
    pub scan: bool,
    /// Locked out: a scan passes it by.
    pub lockout: bool,
    /// The scan's priority channel.
    pub priority: bool,
    /// How long a scan listens here, seconds; `None` for the scan's own dwell.
    pub dwell_s: Option<f64>,
    /// The position in the user's manual order.
    pub order: u64,
    /// When it was stored, seconds since 1970.
    pub created_s: f64,
    /// When it was last recalled, seconds since 1970.
    pub last_used_s: Option<f64>,
    /// How many times a scan stopped here.
    pub hits: u64,
    /// When a scan last stopped here, seconds since 1970.
    pub last_hit_s: Option<f64>,
    /// The CTCSS or DCS tone heard there (`"CTCSS 88.5 Hz"`, `"DCS 023"`), empty for none.
    pub tone: String,
}

impl Default for Memory {
    fn default() -> Self {
        Memory {
            id: 0,
            name: String::new(),
            freq_hz: 0.0,
            stack: String::new(),
            submode: String::new(),
            rate_hz: None,
            bandwidth_hz: None,
            gains: Vec::new(),
            settings: Vec::new(),
            group: String::new(),
            tags: Vec::new(),
            notes: String::new(),
            color: String::new(),
            starred: false,
            scan: true,
            lockout: false,
            priority: false,
            dwell_s: None,
            order: 0,
            created_s: 0.0,
            last_used_s: None,
            hits: 0,
            last_hit_s: None,
            tone: String::new(),
        }
    }
}

impl Memory {
    /// A memory of a frequency in a stack, with a name.
    pub fn new(name: &str, freq_hz: f64, stack: &str) -> Memory {
        Memory {
            name: name.to_string(),
            freq_hz,
            stack: stack.to_string(),
            ..Memory::default()
        }
    }

    /// The same memory with setting `key` of `owner` set to `value`.
    pub fn with_setting(mut self, owner: OwnerKey, key: &str, value: Value) -> Memory {
        put_setting(&mut self.settings, owner, key, value);
        self
    }

    /// The name to show: the name, or the frequency when it has none.
    pub fn label(&self) -> String {
        if self.name.trim().is_empty() {
            crate::freq::format(self.freq_hz)
        } else {
            self.name.clone()
        }
    }

    /// Makes sure that the memory can be stored: a frequency above zero and below 1 THz, a colour of the form `#rrggbb` or
    /// none, and a rate, bandwidth and dwell time above zero when given.
    ///
    /// # Errors
    ///
    /// [`MemoryError::Invalid`] with the problem.
    pub fn validate(&self) -> Result<(), MemoryError> {
        if !(self.freq_hz.is_finite() && self.freq_hz > 0.0 && self.freq_hz < 1e12) {
            return Err(MemoryError::Invalid(Problem::FrequencyOutOfRange));
        }
        if !self.color.is_empty() && !is_color(&self.color) {
            return Err(MemoryError::Invalid(Problem::NotAColour));
        }
        for v in [self.rate_hz, self.bandwidth_hz, self.dwell_s]
            .into_iter()
            .flatten()
        {
            if !(v.is_finite() && v > 0.0) {
                return Err(MemoryError::Invalid(Problem::NotPositive));
            }
        }
        Ok(())
    }

    /// True if the memory is the same channel as `freq_hz` in `stack`: the stack matches in any case, and the frequencies are
    /// within `tolerance_hz`.
    pub fn same_channel(&self, freq_hz: f64, stack: &str, tolerance_hz: f64) -> bool {
        self.stack.eq_ignore_ascii_case(stack) && (self.freq_hz - freq_hz).abs() <= tolerance_hz
    }
}

/// Sets setting `key` of `owner` in `list` to `value`, in place if it is there.
pub(crate) fn put_setting(list: &mut Vec<OwnerSetting>, owner: OwnerKey, key: &str, value: Value) {
    match list.iter_mut().find(|s| s.owner == owner && s.key == key) {
        Some(s) => s.value = value,
        None => list.push(OwnerSetting {
            owner,
            key: key.to_owned(),
            value,
        }),
    }
}

/// Removes setting `key` of `owner` from `list`. Returns true if it was there.
pub(crate) fn remove_setting(list: &mut Vec<OwnerSetting>, owner: &OwnerKey, key: &str) -> bool {
    let n = list.len();
    list.retain(|s| !(s.owner == *owner && s.key == key));
    list.len() != n
}

/// True if `s` is a colour of the form `#rrggbb`.
pub(crate) fn is_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_and_missing_fields_load() {
        let m: Memory =
            serde_json::from_str(r#"{"name":"Tower","freq_hz":118.1e6,"future_field":[1,2]}"#)
                .unwrap();
        assert_eq!(m.name, "Tower");
        assert!(m.scan, "include in scan defaults on");
        assert_eq!(m.stack, "");
    }

    #[test]
    fn validation() {
        assert!(Memory::new("a", 100e6, "fm").validate().is_ok());
        assert!(Memory::new("a", 0.0, "fm").validate().is_err());
        let mut m = Memory::new("a", 100e6, "fm");
        m.color = "red".into();
        assert!(m.validate().is_err());
        m.color = "#ff0000".into();
        m.dwell_s = Some(0.0);
        assert!(m.validate().is_err());
    }

    #[test]
    fn label_falls_back_to_the_frequency() {
        assert_eq!(Memory::new("", 145.5e6, "general").label(), "145.500 MHz");
    }
}
