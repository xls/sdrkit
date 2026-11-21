//! What the tests share: settings and commands by key, typed fields and tables of results, and default configurations.

use sdrkit_core::command::CommandError;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::results::Table;
use sdrkit_core::{
    Event, GenConfig, Record, Results, SettingError, StackConfig, StackDescriptor, Value, Values,
};

/// Sets the setting `key` of `o` to `v`, validated against the schema of `o`.
pub fn set<O: Owner + ?Sized>(o: &mut O, key: &str, v: Value) -> Result<(), SettingError> {
    let id = o.schema().id(key)?;
    owner::apply(o, id, &v)
}

/// Runs the command `key` of `o` with the arguments `args`.
pub fn run<O: Owner + ?Sized>(
    o: &mut O,
    key: &str,
    args: &[(&str, Value)],
) -> Result<Option<Record>, CommandError> {
    let id = o.commands().id(key)?;
    let mut given = o.commands().defs()[id.index()].new_args();
    for (k, v) in args {
        let a = given.decl().id(k).expect("an argument of the command");
        given
            .set_value(a, v)
            .expect("a value of the argument's kind");
    }
    owner::invoke(o, id, &given)
}

/// Runs the command `clear` of `o`.
pub fn clear<O: Owner + ?Sized>(o: &mut O) {
    run(o, "clear", &[]).expect("clear runs");
}

/// The table `key` of `r`.
///
/// # Panics
///
/// If `r` declares no such table.
pub fn table<'a>(r: &'a Results, key: &str) -> &'a Table {
    let id = r
        .decl()
        .table_id(key)
        .unwrap_or_else(|| panic!("no table {key}"));
    r.table(id).expect("a declared table")
}

/// The rows of every table of `r`.
pub fn rows(r: &Results) -> usize {
    (0..r.decl().tables().len())
        .filter_map(|i| r.table(sdrkit_core::TableId(i as u16)))
        .map(Table::len)
        .sum()
}

/// The configuration of a receiver of stack `m` at its defaults.
pub fn stack_defaults(m: &StackDescriptor) -> StackConfig {
    StackConfig::new(Values::defaults(&(m.settings)()))
}

/// The configuration of a generator of stack `m` at `rate_hz`, with the stack's settings and those of the framework at their
/// defaults.
pub fn gen_defaults(m: &StackDescriptor, rate_hz: f64) -> GenConfig {
    let schema = sdrkit_components::sim::with_generator_settings(&(m.settings)())
        .expect("the generator settings join the stack settings");
    GenConfig::new(rate_hz, Values::defaults(&schema))
}

/// A configuration of a receiver of stack `m` with the settings `set` (key and value) over the defaults.
pub fn stack_with(m: &StackDescriptor, set: &[(&str, Value)]) -> StackConfig {
    let schema = (m.settings)();
    let mut v = Values::defaults(&schema);
    for (k, x) in set {
        let id = schema.id(k).expect("a setting of the stack");
        v.set(id, x).expect("a value the stack takes");
    }
    StackConfig::new(v)
}

/// The number in the field `key` of `r`, 0 when it has none.
pub fn number(r: &Results, key: &str) -> f64 {
    let f = r.fields();
    match f.decl().id(key).and_then(|id| f.value(id)) {
        Some(Value::F64(x) | Value::Hz(x) | Value::Duration(x)) => *x,
        Some(Value::I64(x)) => *x as f64,
        _ => 0.0,
    }
}

/// Whether the schema of component `c` refuses `v` for its setting `key`.
pub fn refused<C: sdrkit_graph::Component>(c: &C, key: &str, v: Value) -> bool {
    let s = c.info().settings;
    s.id(key).map_or(true, |id| s.check(id, &v).is_err())
}

/// Whether `events` are all kinds that `decl` declares.
pub fn declared(decl: &sdrkit_core::ResultsDecl, events: &[Event]) -> bool {
    events
        .iter()
        .all(|e| decl.events().iter().any(|d| d.kind == e.kind()))
}

/// Whether the field `key` of `r` holds a value.
pub fn has(r: &Results, key: &str) -> bool {
    let f = r.fields();
    f.decl()
        .id(key)
        .and_then(|id| f.value(id))
        .is_some_and(|v| !v.is_none())
}

/// The points of the series `key` of `r`, 0 when it has none.
pub fn points(r: &Results, key: &str) -> usize {
    r.decl()
        .series_id(key)
        .and_then(|id| r.series(id))
        .map_or(0, |s| s.points.len())
}

/// Feeds `x` to `r` in pieces and ends the stream.
pub fn feed_all(r: &mut dyn sdrkit_core::Receiver, x: &[sdrkit_core::Complex32]) {
    let mut out = sdrkit_core::Output::default();
    for c in x.chunks(65536) {
        r.feed(c, &mut out);
        out.clear();
    }
    r.finish(&mut out);
}

/// The repository's data folder.
pub fn data_folder() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data")
}

/// The Z-Wave regions of the data folder.
pub fn zwave_regions() -> Vec<sdrkit_presets::ism::ZwaveRegion> {
    sdrkit_presets::ism::read_zwave_regions(
        &data_folder().join(sdrkit_presets::ism::ZWAVE_REGIONS_FILE),
    )
    .expect("the Z-Wave region file")
}

/// The Z-Wave region `id` of the data folder.
pub fn zwave_region(id: &str) -> sdrkit_presets::ism::ZwaveRegion {
    zwave_regions()
        .into_iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("no Z-Wave region {id}"))
}

/// The Wi-SUN band plan `id` of the data folder.
pub fn wisun_plan(id: &str) -> sdrkit_presets::ism::WisunPlan {
    let plans = sdrkit_presets::ism::read_wisun_bands(
        &data_folder().join(sdrkit_presets::ism::WISUN_BANDS_FILE),
    )
    .expect("the Wi-SUN band plan file");
    plans
        .into_iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("no Wi-SUN plan {id}"))
}

/// The ranges of the Wi-SUN plan of a tuned frequency and the Wi-SUN band plans of the data folder.
pub fn wisun_plans_of_frequency() -> (
    Vec<sdrkit_presets::ism::WisunPlanRange>,
    Vec<sdrkit_presets::ism::WisunPlan>,
) {
    let ranges = sdrkit_presets::ism::read_wisun_plan_ranges(
        &data_folder().join(sdrkit_presets::ism::WISUN_PLAN_OF_FREQUENCY_FILE),
    )
    .expect("the file of the Wi-SUN plan of a frequency");
    let plans = sdrkit_presets::ism::read_wisun_bands(
        &data_folder().join(sdrkit_presets::ism::WISUN_BANDS_FILE),
    )
    .expect("the Wi-SUN band plan file");
    (ranges, plans)
}
