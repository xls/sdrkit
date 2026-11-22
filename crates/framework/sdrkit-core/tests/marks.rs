//! The marks that a front end reads: the label, the order and the place of the rows of a table, and the kind of content
//! and the media of a stack.

use sdrkit_core::interface::{Family, Fields, StackShape, Table};
use sdrkit_core::publish::keys;
use sdrkit_core::{AudioKind, ContentKind, RowMarks, SortOrder, TableMarks};

/// A row of the test table.
#[derive(Fields)]
struct CraftRow<'a> {
    /// The name.
    #[field(label = "Name")]
    name: &'a str,
    /// The level.
    #[field(label = "Level", unit = "dB")]
    level_db: f64,
    /// The first position.
    #[field(label = "Reported")]
    reported: Option<(f64, f64)>,
    /// The position to show.
    #[field(label = "Position", role = Position)]
    position: Option<(f64, f64)>,
    /// The height.
    #[field(label = "Altitude", unit = "ft", role = Altitude)]
    alt_ft: Option<f64>,
    /// The speed, with the role of the standard key.
    #[field(shared = keys::SPEED_KT)]
    speed_kt: Option<f64>,
}

/// A row with two numbers for its place.
#[derive(Fields)]
struct StrikeRow {
    /// Degrees north.
    #[field(label = "Latitude", unit = "°", role = Latitude)]
    lat: f64,
    /// Degrees east.
    #[field(label = "Longitude", unit = "°", role = Longitude)]
    lon: f64,
}

static CRAFT: Table<CraftRow<'static>> = Table::new("craft", "Craft", 10)
    .label(&CraftRow::NAME)
    .sorted(&CraftRow::LEVEL_DB, SortOrder::Descending);
static PLAIN: Table<CraftRow<'static>> = Table::new("plain", "Plain", 10);
static STRIKES: Table<StrikeRow> = Table::new("strikes", "Strikes", 10);

#[test]
fn a_table_states_its_label_and_order_and_its_place_follows_from_the_columns() {
    let m = CRAFT.def().decl().marks;
    assert_eq!(m.label, Some(0));
    assert_eq!(m.sort, Some((1, SortOrder::Descending)));
    // The column with the role, not the first column of the position kind.
    assert_eq!(m.position, Some(3));
    assert_eq!((m.altitude, m.course, m.speed), (Some(4), None, Some(5)));
    assert_eq!((m.latitude, m.longitude), (None, None));
}

#[test]
fn a_table_without_statements_has_its_first_position_column_and_no_label() {
    let m = PLAIN.def().decl().marks;
    assert_eq!((m.label, m.sort), (None, None));
    assert_eq!(m.position, Some(3));
    let m = STRIKES.def().decl().marks;
    assert_eq!((m.position, m.latitude, m.longitude), (None, Some(0), Some(1)));
    assert!(!m.is_empty());
}

#[test]
#[should_panic(expected = "the table craft has no column missing")]
fn a_mark_of_a_column_that_the_table_does_not_have_is_refused() {
    let rows = RowMarks {
        label: "missing",
        ..RowMarks::NONE
    };
    let _ = TableMarks::of("craft", CRAFT.def().columns, &rows);
}

#[test]
fn a_family_states_the_kind_and_the_media_of_its_stacks_before_or_after_it_adds_them() {
    let family = Family::new("demo", 1, 1)
        .content(ContentKind::Tracking, &["first"])
        .stack(StackShape::new("first").table(&CRAFT))
        .stack(StackShape::new("second").content(ContentKind::Voice))
        .audio(AudioKind::Voice, &["second"])
        .text(&["first"]);
    let first = family.stack_shape("first").unwrap();
    assert_eq!(first.content, Some(ContentKind::Tracking));
    assert!(first.media().text && first.media().audio.is_none());
    let second = family.stack_shape("second").unwrap();
    assert_eq!(second.content, Some(ContentKind::Voice));
    assert_eq!(second.media().audio, Some(AudioKind::Voice));
    assert!(!second.media().pictures && !second.media().video);
}

const PNG: sdrkit_core::ContentType = sdrkit_core::ContentType::Media(sdrkit_core::media::Codec::Png);

/// A picture of a station.
#[derive(Fields)]
struct LogoRow {
    /// The picture.
    #[field(label = "Logo", content = PNG)]
    logo: std::sync::Arc<[u8]>,
}

static LOGOS: Table<LogoRow> = Table::new("logos", "Logos", 4);

#[test]
fn a_picture_column_gives_the_stack_pictures() {
    assert!(StackShape::new("a").table(&LOGOS).media().pictures);
    assert!(!StackShape::new("a").table(&STRIKES).media().pictures);
    assert!(StackShape::new("a").pictures().media().pictures);
}

/// A row of a message table.
#[derive(Fields)]
struct ChatRow<'a> {
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    time_s: f64,
    /// Who sent it.
    #[field(label = "From", role = Sender)]
    from: &'a str,
    /// To whom.
    #[field(label = "To", role = Addressee)]
    to: Option<&'a str>,
    /// The text, with a role next to the standard key.
    #[field(shared = keys::TEXT, role = Text)]
    text: &'a str,
    /// When it was received.
    #[field(label = "Received")]
    received: Option<sdrkit_core::interface::Time>,
    /// When it was sent.
    #[field(label = "Sent")]
    sent: Option<sdrkit_core::interface::Time>,
}

/// A row whose only time is the signal time.
#[derive(Fields)]
struct PageRow<'a> {
    /// The signal time.
    #[field(label = "Signal time", unit = "s", role = Time)]
    time_s: f64,
    /// The pager.
    #[field(label = "Address", role = Addressee)]
    address: &'a str,
}

static CHAT: Table<ChatRow<'static>> =
    Table::new("messages", "Messages", 10).role(sdrkit_core::TableRole::Messages);
static CHAT_PLAIN: Table<ChatRow<'static>> = Table::new("lines", "Lines", 10);
static PAGES: Table<PageRow<'static>> =
    Table::new("pages", "Pages", 10).role(sdrkit_core::TableRole::Messages);

#[test]
fn a_table_states_what_its_rows_are_and_the_columns_of_a_message() {
    let m = CHAT.def().decl().marks;
    assert_eq!(m.role, Some(sdrkit_core::TableRole::Messages));
    assert_eq!((m.sender, m.addressee, m.text), (Some(1), Some(2), Some(3)));
    // Without a column of the role, the first column of the time kind.
    assert_eq!(m.time, Some(4));
    // A table without a role has the columns that state their part, and no time.
    let plain = CHAT_PLAIN.def().decl().marks;
    assert_eq!(plain.role, None);
    assert_eq!(
        (plain.sender, plain.text, plain.time),
        (Some(1), Some(3), None)
    );
    // A column of seconds of signal that states the role is the time.
    let pages = PAGES.def().decl().marks;
    assert_eq!(
        (pages.time, pages.addressee, pages.sender),
        (Some(0), Some(1), None)
    );
    // A table of another kind has none of these marks.
    let craft = CRAFT.def().decl().marks;
    assert_eq!((craft.role, craft.sender, craft.time), (None, None, None));
}

#[test]
fn a_setting_states_what_it_chooses() {
    use sdrkit_core::publish::field;
    use sdrkit_core::settings::{DeclareSetting, Setting};
    use sdrkit_core::SettingRole;
    const PRODUCT: Setting<field::I64> =
        Setting::new("product", "Product", "", 0).role(SettingRole::PictureSource);
    const GAIN: Setting<field::F64> = Setting::new("gain", "Gain", "dB", 0.0);
    assert_eq!(PRODUCT.decl().role, Some(SettingRole::PictureSource));
    assert_eq!(GAIN.decl().role, None);
    let view = sdrkit_core::settings::CatalogueSetting {
        key: "view",
        label: "View",
        help: "",
        choices: || vec![sdrkit_core::ChoiceDecl::new("ch1", "Channel 1")],
        default: "ch1",
        advanced: false,
        role: Some(SettingRole::PictureView),
    };
    assert_eq!(view.decl().role, Some(SettingRole::PictureView));
    // The role is in the declaration that a host reads as JSON, and absent for a setting without one.
    let json = serde_json::to_string(&view.decl()).unwrap();
    assert!(json.contains("\"role\":\"picture_view\""), "{json}");
    assert!(!serde_json::to_string(&GAIN.decl())
        .unwrap()
        .contains("role"));
}
