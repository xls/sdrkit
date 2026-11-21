//! Helpers of the test program: a run of a receiver with a tap on one of its streams, the strips of a run, and the settings of
//! the standard impairments.

use sdrkit_core::command::CommandError;
use sdrkit_core::{
    owner, Complex32, Event, HostRequest, InputInfo, Output, Owner, Receiver, Record, Results,
    SettingError, Value, ValueKind,
};
use sdrkit_dev_test::Harness;
use sdrkit_graph::{Buffer, Data, Graph, Item, Kind, Timed};

/// The results of a run.
pub struct Ran {
    /// The results at the end.
    pub results: Results,
    /// The items of the tapped stream.
    pub items: Vec<Box<dyn std::any::Any>>,
    /// The events.
    pub events: Vec<Event>,
    /// The host requests.
    pub requests: Vec<HostRequest>,
    /// The receiver, to look at or feed again.
    pub rx: Box<dyn Receiver>,
}

impl Ran {
    /// A field's value.
    pub fn value(&self, key: &str) -> Option<&Value> {
        let id = self.results.decl().field_id(key)?;
        self.results.fields().value(id)
    }
    /// A typed field's value as a number (a whole number, a number or yes and no).
    pub fn num(&self, key: &str) -> f64 {
        match self.value(key) {
            Some(Value::F64(v) | Value::Hz(v) | Value::Duration(v) | Value::Time(v)) => *v,
            Some(Value::I64(v)) => *v as f64,
            Some(Value::Bool(b)) => f64::from(u8::from(*b)),
            _ => f64::NAN,
        }
    }
    /// A field's text: a text as it is, a choice by its key.
    pub fn text(&self, key: &str) -> &str {
        let Some(id) = self.results.decl().field_id(key) else {
            return "";
        };
        match self.results.fields().value(id) {
            Some(Value::Text(t)) => t,
            Some(Value::Choice(i)) => self
                .results
                .decl()
                .fields()
                .field(id)
                .and_then(|d| d.choices.get(usize::from(*i)))
                .map_or("", |c| c.key.as_ref()),
            _ => "",
        }
    }
    /// The tapped items of type `T`.
    pub fn of<T: Item>(&self) -> Vec<T> {
        self.items
            .iter()
            .filter_map(|b| b.downcast_ref::<T>().cloned())
            .collect()
    }
}

/// The standard impairments for a test: the level of the old generators (-13 dBFS), white noise at `esn0` dB (none when
/// `None`), a carrier offset and a clock error.
pub fn impaired(h: Harness, esn0: Option<f64>, offset_hz: f64, clock_ppm: f64) -> Harness {
    let h = h
        .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -13.0)
        .set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, offset_hz)
        .set(&sdrkit_algo_sim::chain::GEN_CLOCK_PPM, clock_ppm);
    match esn0 {
        Some(db) => h
            .set(
                &sdrkit_algo_sim::chain::GEN_NOISE,
                sdrkit_algo_sim::chain::NoiseKind::EsN0,
            )
            .set(&sdrkit_algo_sim::chain::GEN_ESN0_DB, db),
        None => h,
    }
}

/// Feeds `x` (at `rate`, centred on `centre`, the receiver tuned to `tuned`) in pieces of `piece` samples to the receiver of `h`,
/// with a tap on `tap` (`"<component>.<port>"`) of items of type `T`.
pub fn run<T: Item>(
    h: &Harness,
    x: &[Complex32],
    rate: f64,
    centre: f64,
    tuned: f64,
    piece: usize,
    tap: Option<&str>,
) -> Ran {
    let mut rx = h.receiver().expect("the receiver");
    rx.configure(&InputInfo::new(rate, centre, tuned).recording())
        .expect("the receiver takes the rate");
    feed::<T>(rx, x, piece, tap)
}

/// Feeds `x` to a receiver that is already configured. See [`run`].
pub fn feed<T: Item>(
    mut rx: Box<dyn Receiver>,
    x: &[Complex32],
    piece: usize,
    tap: Option<&str>,
) -> Ran {
    let mut reader = tap.map(|t| {
        rx.as_any_mut()
            .and_then(|a| a.downcast_mut::<Graph>())
            .expect("a graph")
            .tap(t, 1 << 16)
            .expect("the tapped port")
    });
    let mut buf = Buffer::new(&Kind::typed::<T>());
    let mut items: Vec<Box<dyn std::any::Any>> = Vec::new();
    let mut events = Vec::new();
    let mut requests = Vec::new();
    let mut out = Output::default();
    for p in x.chunks(piece.max(1)) {
        out.clear();
        rx.feed(p, &mut out);
        events.append(&mut out.events);
        requests.append(&mut out.requests);
        if let Some(r) = reader.as_mut() {
            while r.pop(&mut buf) {
                if let Data::Items(Some(v)) = &buf.data {
                    if let Some(v) = v.as_any().downcast_ref::<Vec<Timed<T>>>() {
                        items.extend(
                            v.iter()
                                .map(|t| Box::new(t.value.clone()) as Box<dyn std::any::Any>),
                        );
                    }
                }
            }
        }
    }
    let src = Owner::results(&*rx);
    let mut results = Results::new(std::sync::Arc::clone(src.decl()));
    results.copy_from(src);
    Ran {
        results,
        items,
        events,
        requests,
        rx,
    }
}

/// A file of the satimg recordings (an expected picture). `None` when it is missing.
pub fn vector_file(name: &str) -> Option<std::path::PathBuf> {
    sdrkit_dev_test::recording(&format!("satimg/{name}"))
}

/// Reads an 8-bit binary PGM: (width, height, samples).
pub fn pgm(path: &std::path::Path) -> (usize, usize, Vec<u8>) {
    let b = std::fs::read(path).expect("pgm");
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b[i] == b'#' {
            while b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let s = i;
        while !b[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&b[s..i]).to_string());
    }
    i += 1;
    let w: usize = fields[1].parse().unwrap();
    let h: usize = fields[2].parse().unwrap();
    (w, h, b[i..i + w * h].to_vec())
}

/// The value of setting `key` of `rx` from its text: a choice by its key, a flag as "true" or "false", a number, else text.
pub fn setting_value(rx: &dyn Receiver, key: &str, text: &str) -> Option<Value> {
    let schema = rx.schema();
    let id = schema.id(key).ok()?;
    let decl = schema.setting(id)?;
    Some(match decl.field.kind {
        ValueKind::Choice => Value::Choice(decl.field.choice_index(text).unwrap_or(u16::MAX)),
        ValueKind::Bool => Value::Bool(text == "true"),
        ValueKind::F64 => Value::F64(text.parse().ok()?),
        ValueKind::Hz => Value::Hz(text.parse().ok()?),
        ValueKind::I64 => Value::I64(text.parse().ok()?),
        _ => Value::Text(text.into()),
    })
}

/// Sets setting `key` of `rx` from its text through the checked path. An unknown key or a text that is no value of the setting
/// is refused.
pub fn set(rx: &mut dyn Receiver, key: &str, text: &str) -> Result<(), SettingError> {
    let id = rx.schema().id(key)?;
    let v = setting_value(rx, key, text).ok_or(SettingError::Malformed { id, at: None })?;
    owner::apply(rx, id, &v)
}

/// Runs command `key` of `rx` with these text arguments.
pub fn invoke(
    rx: &mut dyn Receiver,
    key: &str,
    args: &[(&str, &str)],
) -> Result<Option<Record>, CommandError> {
    let id = rx.commands().id(key)?;
    let def = &rx.commands().defs()[id.index()];
    let mut given = def.new_args();
    for (k, t) in args {
        let a = def.args.id(k).expect("an argument of the command");
        given
            .set_value(sdrkit_core::FieldId(a.0), &Value::Text((*t).into()))
            .expect("a text argument");
    }
    owner::invoke(rx, id, &given)
}

/// The fields of `r` by key, without the keys of `skip`.
pub fn fields_without(r: &Results, skip: &[&str]) -> Vec<(String, Value)> {
    r.fields()
        .iter()
        .filter(|(d, _)| !skip.contains(&d.key.as_ref()))
        .map(|(d, v)| (d.key.to_string(), v.clone()))
        .collect()
}

/// The rows of every table of `r`: (table key, row key, row).
pub fn table_rows(r: &Results) -> Vec<(String, String, Record)> {
    let mut v = Vec::new();
    for (i, t) in r.decl().tables().iter().enumerate() {
        if let Some(table) = r.table(sdrkit_core::TableId(i as u16)) {
            for (k, row) in table.rows_newest_first() {
                v.push((t.key.to_string(), k.to_string(), row.clone()));
            }
        }
    }
    v
}

/// Makes sure that the results of a run carry every field of `decl` and that each event the run emitted is a kind of `decl`.
pub fn assert_declared(r: &Ran, decl: &sdrkit_core::ResultsDecl) {
    for f in decl.fields().fields() {
        assert!(
            r.results.decl().field_id(&f.key).is_some(),
            "field {} is not published",
            f.key
        );
    }
    for e in &r.events {
        assert!(
            decl.events().iter().any(|d| d.kind == e.kind()),
            "event {} is not declared",
            e.kind()
        );
    }
}

/// The whole-number cell `column` of row `row` of table `table` of `r`.
pub fn cell_i64(
    r: &Results,
    table: &str,
    row: &str,
    column: &sdrkit_core::interface::Column<sdrkit_core::publish::field::I64>,
) -> Option<i64> {
    let id = r.decl().table_id(table)?;
    r.table(id)?.row(row)?.at(column)
}
