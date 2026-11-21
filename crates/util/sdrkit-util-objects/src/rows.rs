//! Lookup of an object by the row key that its table shows.

use crate::publish::KeyText;
use crate::table::ObjectTable;
use std::fmt::Display;
use std::hash::Hash;

/// Returns the key of the object of `objects` whose row key (the text of its key) is `row`, or `None` if no object has that
/// row key.
pub fn key_of_row<K, R>(objects: &ObjectTable<K, R>, row: &str) -> Option<K>
where
    K: Hash + Eq + Clone + Display,
{
    let mut text = KeyText::new();
    objects
        .iter()
        .find(|r| text.set("", r.key) == row)
        .map(|r| r.key.clone())
}

/// Removes the object of `objects` whose row key is `row`. Returns false if no object has that row key.
pub fn remove_row<K, R>(objects: &mut ObjectTable<K, R>, row: &str) -> bool
where
    K: Hash + Eq + Clone + Display,
{
    key_of_row(objects, row).is_some_and(|k| objects.remove(&k).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_key_finds_and_removes_its_object() {
        let mut t: ObjectTable<u32, &str> = ObjectTable::new(8);
        let at = crate::Stamp { t: 0.0, wall: None };
        t.upsert(&0x4CA2D6, at, || "a").unwrap();
        t.upsert(&7, at, || "b").unwrap();
        assert_eq!(key_of_row(&t, "5022422"), Some(0x4CA2D6));
        assert_eq!(key_of_row(&t, "8"), None);
        assert!(remove_row(&mut t, "7"));
        assert!(!remove_row(&mut t, "7"));
        assert_eq!(t.len(), 1);
    }
}
