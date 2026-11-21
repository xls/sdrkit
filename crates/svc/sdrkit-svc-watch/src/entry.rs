//! Watches and how they match observations: a thing by its identity, any field, a frequency, a range of frequencies, or a
//! word. Identities and values match without regard to case, spaces and dashes, and a trailing `*` matches a prefix.
//! Private content is never read.

use std::fmt::Write as _;

use sdrkit_svc_events::text::contains;
use sdrkit_svc_events::{FamilySet, FieldRef, KindSet, Kinds, NameError, Observation};
use serde::{Deserialize, Serialize};

use crate::WatchError;

sdrkit_core::choice_enum! {
    /// What a watch looks for.
    pub enum WatchWhat {
        /// The thing an event is about (its standard `source` field).
        Source = ("source", "Identity"),
        /// A field that the watch names.
        Field = ("field", "Field"),
        /// A frequency, with a tolerance.
        Frequency = ("frequency", "Frequency"),
        /// A range of frequencies.
        Range = ("range", "Frequency range"),
        /// A word or a phrase in the text of an event.
        Word = ("word", "Word"),
    }
}

/// A watch as the user set it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct EntryText {
    pub label: String,
    pub what: String,
    pub families: String,
    pub kinds: String,
    pub field: String,
    pub value: String,
    pub hz: f64,
    pub tolerance_hz: f64,
    pub to_hz: f64,
    pub urgent: bool,
    pub push: bool,
    pub enabled: bool,
}

/// A watch, resolved against the event kinds.
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub key: String,
    pub text: EntryText,
    what: WatchWhat,
    families: FamilySet,
    kinds: Option<KindSet>,
    field: Option<FieldRef>,
}

/// The characters of `s` that count: upper case, without spaces and dashes.
fn norm(s: &str) -> impl Iterator<Item = char> + '_ {
    s.trim()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .flat_map(char::to_uppercase)
}

/// True if `value` matches `pattern`: equal without regard to case, spaces and dashes, or a prefix if `pattern` ends in
/// `*`.
pub(crate) fn like(pattern: &str, value: &str) -> bool {
    let p = pattern.trim();
    match p.strip_suffix('*') {
        Some(prefix) => {
            let mut v = norm(value);
            norm(prefix).all(|c| v.next() == Some(c))
        }
        None => norm(p).eq(norm(value)),
    }
}

fn name_err(arg: &'static str) -> impl Fn(NameError) -> WatchError {
    move |error| WatchError::Name { arg, error }
}

impl Entry {
    /// The watch `key` of `text` with `what`.
    ///
    /// # Errors
    ///
    /// [`WatchError::Name`] with the position if a kind, a family or the field does not resolve.
    pub fn resolve(
        key: String,
        text: EntryText,
        what: WatchWhat,
        k: &Kinds,
    ) -> Result<Entry, WatchError> {
        let kinds = k.kind_list(&text.kinds, 0).map_err(name_err("kinds"))?;
        let families = Kinds::family_list(&text.families, 0).map_err(name_err("families"))?;
        let field = match what {
            WatchWhat::Field => Some(
                k.field(text.field.trim(), 0, kinds.as_ref())
                    .map_err(name_err("field"))?,
            ),
            _ => None,
        };
        Ok(Entry {
            key,
            text,
            what,
            families,
            kinds,
            field,
        })
    }

    /// True if the watch matches `o`. On a match, what matched is written into `what` (cleared first).
    pub fn matches(&self, o: &Observation<'_>, what: &mut String) -> bool {
        if !self.text.enabled
            || !self.families.admits(o.family)
            || self.kinds.as_ref().is_some_and(|k| !k.contains(o.kind))
        {
            return false;
        }
        let f = o.event.freq_hz;
        let t = &self.text;
        let hit = match self.what {
            WatchWhat::Source => o.identity.filter(|i| like(&t.value, i)),
            WatchWhat::Field => {
                let Some(r) = self.field.as_ref().and_then(|r| o.read(r)) else {
                    return false;
                };
                what.clear();
                let _ = write!(what, "{r}");
                return like(&t.value, what);
            }
            WatchWhat::Frequency => {
                if f > 0.0 && (f - t.hz).abs() <= t.tolerance_hz.max(0.5) {
                    what.clear();
                    let _ = write!(what, "{:.6} MHz", f / 1e6);
                    return true;
                }
                return false;
            }
            WatchWhat::Range => {
                if f > 0.0 && f >= t.hz.min(t.to_hz) && f <= t.hz.max(t.to_hz) {
                    what.clear();
                    let _ = write!(what, "{:.6} MHz", f / 1e6);
                    return true;
                }
                return false;
            }
            WatchWhat::Word => {
                let word = t.value.trim();
                (!o.private && !word.is_empty() && o.words().any(|w| contains(w, word)))
                    .then_some(word)
            }
        };
        match hit {
            Some(h) => {
                what.clear();
                what.push_str(h);
                true
            }
            None => false,
        }
    }
}
