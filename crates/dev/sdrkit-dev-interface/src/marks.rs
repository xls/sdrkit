//! The marks of a table and of a stack as text, for the registry record, the comments of the written files and the
//! docs pages: the column that names a row, the default order, the columns of a place, the kind of content and the
//! media.

use sdrkit_core::interface::StackShape;
use sdrkit_core::{Choice, SettingRole, SortOrder, TableDef, TableMarks, TableRole};

/// The marks of table `t` as attributes of its registry line, each with a space before it: `label=`, `sort=` (the
/// column, `:`, the order), `position=`, `latitude=`, `longitude=`, `altitude=`, `course=`, `speed=`, `role=`,
/// `sender=`, `addressee=`, `message=` (the column of the text) and `time=`. Empty for a table without marks.
pub fn table_attributes(t: &TableDef) -> String {
    let m = TableMarks::of(t.key, t.columns, &t.marks);
    let key = |c: Option<u16>| c.map(|c| t.columns[usize::from(c)].key());
    let mut out = String::new();
    let mut put = |name: &str, value: Option<String>| {
        if let Some(v) = value {
            out.push_str(&format!(" {name}={v}"));
        }
    };
    put("label", key(m.label).map(str::to_owned));
    put(
        "sort",
        m.sort.map(|(c, order)| {
            let order = match order {
                SortOrder::Ascending => "ascending",
                SortOrder::Descending => "descending",
            };
            format!("{}:{order}", t.columns[usize::from(c)].key())
        }),
    );
    for (name, c) in [
        ("position", m.position),
        ("latitude", m.latitude),
        ("longitude", m.longitude),
        ("altitude", m.altitude),
        ("course", m.course),
        ("speed", m.speed),
    ] {
        put(name, key(c).map(str::to_owned));
    }
    put("role", m.role.map(|r| r.key().to_owned()));
    for (name, c) in [
        ("sender", m.sender),
        ("addressee", m.addressee),
        ("message", m.text),
        ("time", m.time),
    ] {
        put(name, key(c).map(str::to_owned));
    }
    out
}

/// The marks of table `t` for people, one sentence each. Empty for a table without marks.
pub fn table_sentences(t: &TableDef) -> Vec<String> {
    let m = TableMarks::of(t.key, t.columns, &t.marks);
    let key = |c: u16| t.columns[usize::from(c)].key();
    let mut out = Vec::new();
    if let Some(r) = m.role {
        out.push(
            match r {
                TableRole::Messages => "The rows are the messages of the stack.",
                TableRole::Calls => "The rows are the calls of the stack.",
                TableRole::Nodes => "The rows are the stations or nodes of the stack.",
                TableRole::Groups => "The rows are the groups of the stack.",
            }
            .to_owned(),
        );
    }
    if let Some(c) = m.label {
        out.push(format!("The column `{}` names a row.", key(c)));
    }
    if let Some((c, order)) = m.sort {
        let first = match order {
            SortOrder::Ascending => "lowest",
            SortOrder::Descending => "highest",
        };
        out.push(format!(
            "Default order: the column `{}`, {first} first.",
            key(c)
        ));
    }
    match (m.position, m.latitude, m.longitude) {
        (Some(p), _, _) => out.push(format!("Place of a row: the column `{}`.", key(p))),
        (None, Some(la), Some(lo)) => out.push(format!(
            "Place of a row: the columns `{}` and `{}`.",
            key(la),
            key(lo)
        )),
        _ => {}
    }
    let more: Vec<String> = [
        ("Height", m.altitude),
        ("Course", m.course),
        ("Speed", m.speed),
    ]
    .into_iter()
    .filter_map(|(name, c)| Some(format!("{name}: `{}`.", key(c?))))
    .collect();
    if !more.is_empty() {
        out.push(more.join(" "));
    }
    let parts: Vec<String> = [
        ("Sender", m.sender),
        ("Addressee", m.addressee),
        ("Text", m.text),
        ("Time", m.time),
    ]
    .into_iter()
    .filter_map(|(name, c)| Some(format!("{name}: `{}`.", key(c?))))
    .collect();
    if !parts.is_empty() {
        out.push(parts.join(" "));
    }
    out
}

/// The role of setting `d` for people, in one sentence. `None` for a setting without a role.
pub fn setting_sentence(d: &sdrkit_core::SettingDecl) -> Option<String> {
    let what = match d.role? {
        SettingRole::PictureSource => "chooses whose picture the stack shows",
        SettingRole::PictureView => "chooses the view of the picture",
    };
    Some(format!("The setting `{}` {what}.", d.field.key))
}

/// The kind of content and the media of `stack` as attributes of its registry line `<stack>.content`, each with a
/// space before it. `None` for a stack without a kind of content.
pub fn stack_attributes(stack: &StackShape) -> Option<String> {
    let kind = stack.content?;
    let media = stack.media();
    let mut out = format!(" kind={}", kind.key());
    if let Some(a) = media.audio {
        out.push_str(&format!(" audio={}", a.key()));
    }
    for (name, on) in [
        ("pictures", media.pictures),
        ("video", media.video),
        ("text", media.text),
    ] {
        if on {
            out.push_str(&format!(" {name}=yes"));
        }
    }
    Some(out)
}

/// The kind of content and the media of `stack` for people, in one line. `None` for a stack without a kind of
/// content.
pub fn stack_sentence(stack: &StackShape) -> Option<String> {
    let kind = stack.content?;
    let media = stack.media();
    let mut parts: Vec<String> = Vec::new();
    if let Some(a) = media.audio {
        parts.push(format!("sound ({})", a.label().to_lowercase()));
    }
    for (name, on) in [
        ("pictures", media.pictures),
        ("video", media.video),
        ("text", media.text),
    ] {
        if on {
            parts.push(name.to_owned());
        }
    }
    let media = if parts.is_empty() {
        "none".to_owned()
    } else {
        parts.join(", ")
    };
    Some(format!(
        "Content kind: {} (`{}`). Media: {media}.",
        kind.label(),
        kind.key()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{compare, parse, render, verdict, Verdict};
    use sdrkit_core::interface::{Family, Fields, Table};
    use sdrkit_core::{AudioKind, ContentKind};

    /// A ship.
    #[derive(Fields)]
    struct ShipRow<'a> {
        /// The name.
        #[field(label = "Name")]
        name: &'a str,
        /// The level.
        #[field(label = "Level", unit = "dB")]
        level_db: f64,
        /// The place.
        #[field(label = "Position")]
        position: Option<(f64, f64)>,
        /// The speed.
        #[field(label = "Speed", unit = "kt", role = Speed)]
        speed_kt: Option<f64>,
    }

    static PLAIN: Table<ShipRow<'static>> = Table::new("ships", "Ships", 10);
    static MARKED: Table<ShipRow<'static>> = Table::new("ships", "Ships", 10)
        .label(&ShipRow::NAME)
        .sorted(&ShipRow::LEVEL_DB, SortOrder::Descending);

    #[test]
    fn the_marks_of_a_table_are_attributes_and_sentences() {
        assert_eq!(
            table_attributes(MARKED.def()),
            " label=name sort=level_db:descending position=position speed=speed_kt"
        );
        assert_eq!(
            table_sentences(MARKED.def()),
            [
                "The column `name` names a row.",
                "Default order: the column `level_db`, highest first.",
                "Place of a row: the column `position`.",
                "Speed: `speed_kt`.",
            ]
        );
    }

    #[test]
    fn the_kind_and_the_media_of_a_stack_are_one_registry_line() {
        let family = Family::new("demo", 1, 1)
            .added(1, 1, &["ais.content"])
            .content(ContentKind::Tracking, &["ais"])
            .audio(AudioKind::Monitor, &["ais"])
            .text(&["ais"])
            .stack(StackShape::new("ais").table(&MARKED));
        let stack = family.stack_shape("ais").unwrap();
        assert_eq!(
            stack_sentence(stack).unwrap(),
            "Content kind: Position tracking (`tracking`). Media: sound (monitor), text."
        );
        let text = render(&family);
        assert!(
            text.contains("\nais.content kind=tracking audio=monitor text=yes since=1.1\n"),
            "{text}"
        );
        assert!(stack_attributes(&StackShape::new("none")).is_none());
    }

    #[test]
    fn a_mark_is_a_compatible_change_that_needs_a_higher_version() {
        let old = Family::new("demo", 1, 0).stack(StackShape::new("ais").table(&PLAIN));
        let new = |minor| {
            Family::new("demo", 1, minor)
                .added(1, 1, &["ais.content"])
                .content(ContentKind::Tracking, &["ais"])
                .stack(StackShape::new("ais").table(&MARKED))
        };
        let old = parse(&render(&old)).unwrap();
        let same_version = parse(&render(&new(0))).unwrap();
        let raised = parse(&render(&new(1))).unwrap();
        let changes = compare(&old, &raised);
        assert!(!changes.is_empty() && changes.iter().all(|c| !c.breaking), "{changes:?}");
        assert!(changes.iter().any(|c| c.path == "ais.table.ships" && c.what.starts_with("label")));
        assert!(matches!(verdict(&old, &raised), Ok(Verdict::Compatible(_))));
        assert!(verdict(&old, &same_version).is_err());
        // Another label later is compatible too.
        let mut other = raised.clone();
        for (path, attributes) in &mut other.items {
            if path == "ais.table.ships" {
                attributes.insert("label".into(), "level_db".into());
            }
        }
        assert!(compare(&raised, &other).iter().all(|c| !c.breaking));
    }
}
