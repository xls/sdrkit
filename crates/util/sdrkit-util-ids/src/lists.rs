//! Watch and ignore lists: a short list of ids a person types into one settings field ("1234 2000-2099 232*"), checked for every
//! message heard. Items are separated by spaces, commas, semicolons or line breaks; an item is an id, a range `a-b` of a numeric
//! kind (either order), or a prefix ending in `*` matched against the written form of the id (`232*` holds every MMSI of MID 232,
//! `4CA*` every Irish ICAO address). Bad items are refused and listed, never guessed at.
//!
//! ```
//! use sdrkit_util_ids::lists::IdList;
//! use sdrkit_util_ids::names::IdKind;
//!
//! let (watch, refused) = IdList::parse(IdKind::Decimal, "1234, 2000-2099 77- x");
//! assert_eq!(refused, ["77-", "x"]);
//! assert!(watch.contains_number(2042));
//! assert!(watch.contains("1234"));
//! assert!(!watch.contains("1235"));
//! ```

use crate::names::IdKind;

/// One item of a list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// One id, in its written form.
    One(String),
    /// A range of numeric ids, both ends included.
    Range(u64, u64),
    /// Every id whose written form starts with this text.
    Prefix(String),
}

/// A watch or ignore list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdList {
    kind: IdKind,
    items: Vec<Item>,
}

fn number(kind: IdKind, written: &str) -> Option<u64> {
    match kind {
        IdKind::Decimal | IdKind::Mmsi => written.parse().ok(),
        IdKind::Icao => u64::from_str_radix(written, 16).ok(),
        IdKind::Selcal | IdKind::Text => None,
    }
}

impl IdList {
    /// An empty list.
    pub fn new(kind: IdKind) -> IdList {
        IdList {
            kind,
            items: Vec::new(),
        }
    }

    /// Reads a list. `""` and `"none"` give an empty list. Returns the list and the items refused, as written.
    pub fn parse(kind: IdKind, text: &str) -> (IdList, Vec<String>) {
        let mut list = IdList::new(kind);
        let mut refused = Vec::new();
        let t = text.trim();
        if t.is_empty() || t.eq_ignore_ascii_case("none") {
            return (list, refused);
        }
        let sep: &[char] = if kind == IdKind::Text {
            &[',', ';', '\n']
        } else {
            &[' ', ',', ';', '\n', '\t', '\r']
        };
        for raw in t.split(sep).map(str::trim).filter(|x| !x.is_empty()) {
            match list.item(raw) {
                Some(i) => list.items.push(i),
                None => refused.push(raw.to_string()),
            }
        }
        (list, refused)
    }

    fn item(&self, raw: &str) -> Option<Item> {
        let kind = self.kind;
        if let Some(p) = raw.strip_suffix('*') {
            let p = p.trim();
            let ok = match kind {
                IdKind::Decimal | IdKind::Mmsi => {
                    !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())
                }
                IdKind::Icao => {
                    !p.is_empty() && p.len() <= 6 && p.bytes().all(|b| b.is_ascii_hexdigit())
                }
                IdKind::Selcal | IdKind::Text => !p.is_empty(),
            };
            return ok.then(|| Item::Prefix(p.to_uppercase()));
        }
        if matches!(kind, IdKind::Decimal | IdKind::Icao) {
            if let Some((a, b)) = raw.split_once('-') {
                let a = number(kind, &kind.normalise(a)?)?;
                let b = number(kind, &kind.normalise(b)?)?;
                return Some(Item::Range(a.min(b), a.max(b)));
            }
        }
        kind.normalise(raw).map(Item::One)
    }

    /// The kind of id.
    pub fn kind(&self) -> IdKind {
        self.kind
    }

    /// The items.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Whether an id (as heard, in any form [`IdKind::normalise`] reads) is on the list.
    pub fn contains(&self, id: &str) -> bool {
        match self.kind.normalise(id) {
            Some(w) => self.contains_written(&w),
            None => false,
        }
    }

    /// Whether a numeric id is on the list. Does not allocate unless the list has prefix items.
    pub fn contains_number(&self, id: u64) -> bool {
        let mut written: Option<String> = None;
        self.items.iter().any(|it| match it {
            Item::Range(a, b) => (*a..=*b).contains(&id),
            Item::One(w) => number(self.kind, w) == Some(id),
            Item::Prefix(p) => {
                let w = written.get_or_insert_with(|| match self.kind {
                    IdKind::Mmsi => format!("{id:09}"),
                    IdKind::Icao => format!("{id:06X}"),
                    _ => id.to_string(),
                });
                w.starts_with(p.as_str())
            }
        })
    }

    fn contains_written(&self, w: &str) -> bool {
        let n = number(self.kind, w);
        self.items.iter().any(|it| match it {
            Item::One(x) => x == w,
            Item::Range(a, b) => n.is_some_and(|n| (*a..=*b).contains(&n)),
            Item::Prefix(p) => w.starts_with(p.as_str()),
        })
    }

    /// The list written back as text, one item per space.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for (i, it) in self.items.iter().enumerate() {
            if i > 0 {
                out.push(if self.kind == IdKind::Text { ',' } else { ' ' });
            }
            match it {
                Item::One(w) => out.push_str(w),
                Item::Prefix(p) => {
                    out.push_str(p);
                    out.push('*');
                }
                Item::Range(a, b) => match self.kind {
                    IdKind::Icao => out.push_str(&format!("{a:06X}-{b:06X}")),
                    _ => out.push_str(&format!("{a}-{b}")),
                },
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capcode_lists() {
        let (w, bad) = IdList::parse(
            IdKind::Decimal,
            "1234 99 2000-2099 2995-2990 77- -5 99999999999999999999999",
        );
        assert_eq!(
            bad,
            ["77-", "-5", "99999999999999999999999"],
            "bad items are refused, not clamped"
        );
        assert!(w.contains_number(99) && w.contains_number(2992) && w.contains_number(2000));
        assert!(!w.contains_number(2100));
        assert!(IdList::parse(IdKind::Decimal, "none").0.is_empty());
        assert!(IdList::parse(IdKind::Decimal, "").0.is_empty());
        let (r, _) = IdList::parse(IdKind::Decimal, &w.to_text());
        assert_eq!(r, w);
    }

    #[test]
    fn mmsi_prefix_and_icao_ranges() {
        let (w, bad) = IdList::parse(IdKind::Mmsi, "232*; 2320064, 9701*");
        assert!(bad.is_empty());
        assert!(w.contains("232123456"));
        assert!(w.contains_number(2_320_064));
        assert!(w.contains("002320064"));
        assert!(w.contains_number(970_123_456));
        assert!(!w.contains("244123456"));
        let (a, _) = IdList::parse(IdKind::Icao, "4CA000-4CAFFF A835AF 3C*");
        assert!(a.contains("4ca123") && a.contains("a835af") && a.contains("3C1234"));
        assert!(!a.contains("4CB000"));
        assert!(a.contains_number(0x3C_0001));
        assert_eq!(a.to_text(), "4CA000-4CAFFF A835AF 3C*");
    }

    #[test]
    fn selcal_and_text_lists() {
        let (s, bad) = IdList::parse(IdKind::Selcal, "AM-CP bq-dr XX-YY");
        assert_eq!(bad, ["XX-YY"]);
        assert!(s.contains("AMCP") && s.contains("BQ-DR"));
        let (t, _) = IdList::parse(IdKind::Text, "Coast Guard, PAN PAN");
        assert!(t.contains("coast   guard"));
    }
}
