//! The text rows of the screens, built from fixed results without a terminal.

use sdrkit_host_tui::api::{Decl, Opt, ResultsDecl, TableDecl, Type, Value};
use sdrkit_host_tui::format;
use sdrkit_host_tui::results::{Results, Row, Rows};
use sdrkit_host_tui::views;

fn column(key: &str, label: &str, ty: Type) -> Decl {
    Decl {
        key: key.into(),
        label: label.into(),
        ty,
        ..Decl::default()
    }
}

fn row(key: &str, cells: Vec<Value>) -> Row {
    Row {
        key: key.into(),
        cells,
    }
}

fn s(x: &str) -> Value {
    Value::String(x.into())
}

/// The engine's stacks and signal types tables with two stacks.
fn engine() -> Results {
    let mut signal = column("signal", "Signal type", Type::Enum);
    signal.options = vec![
        Opt {
            key: "fm".into(),
            label: "FM broadcast".into(),
        },
        Opt {
            key: "rds".into(),
            label: "RDS".into(),
        },
        Opt {
            key: "dab".into(),
            label: "DAB".into(),
        },
    ];
    let mut r = Results::default();
    r.decl = ResultsDecl {
        fields: vec![column("cpu", "Processor", Type::F64)],
        tables: vec![
            TableDecl {
                key: "stacks".into(),
                label: "Stacks".into(),
                columns: vec![
                    column("name", "Name", Type::String),
                    column("group", "Group", Type::String),
                    column("blurb", "About", Type::String),
                    column("default_hz", "Frequency", Type::Hz),
                    column("rate_hz", "Sample rate", Type::Hz),
                ],
                cap: 10,
            },
            TableDecl {
                key: "stack_signals".into(),
                label: "Signal types".into(),
                columns: vec![column("stack", "Stack", Type::String), signal],
                cap: 10,
            },
        ],
    };
    r.fields = vec![Value::F64(0.25)];
    r.tables = vec![
        Rows {
            rows: vec![
                row(
                    "fm",
                    vec![
                        s("FM"),
                        s("broadcast"),
                        s("FM radio"),
                        Value::Hz(104.9e6),
                        Value::Hz(2.4e6),
                    ],
                ),
                row(
                    "dab",
                    vec![
                        s("DAB"),
                        s("broadcast"),
                        s("DAB radio"),
                        Value::Hz(206.352e6),
                        Value::Hz(2.048e6),
                    ],
                ),
            ],
        },
        Rows {
            rows: vec![
                row("dab/dab", vec![s("dab"), Value::Enum(2)]),
                row("fm/rds", vec![s("fm"), Value::Enum(1)]),
                row("fm/fm", vec![s("fm"), Value::Enum(0)]),
            ],
        },
    ];
    r
}

#[test]
fn stacks_join_their_signal_types() {
    let rows = views::stacks(&engine());
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, "dab");
    assert_eq!(rows[1].id, "fm");
    assert_eq!(rows[1].signals, "FM broadcast, RDS");
    assert_eq!(rows[1].default_hz, 104.9e6);
    assert_eq!(rows[0].rate_hz, 2.048e6);
}

#[test]
fn table_text_has_the_key_and_the_cells_by_unit() {
    let (header, rows) = views::table(&engine(), 0, 1, 5);
    assert_eq!(
        header,
        ["key", "Name", "Group", "About", "Frequency", "Sample rate"]
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0],
        [
            "dab",
            "DAB",
            "broadcast",
            "DAB radio",
            "206.352 MHz",
            "2.048 MHz"
        ]
    );
    let fields = views::fields(&engine());
    assert_eq!(fields, [("Processor".to_owned(), "0.25".to_owned())]);
}

#[test]
fn typed_text_reads_back() {
    let hz = column("f", "Frequency", Type::Hz);
    assert_eq!(format::parse("104.9M", &hz), Ok(Value::Hz(104.9e6)));
    assert_eq!(format::parse("100k", &hz), Ok(Value::Hz(100e3)));
    let mut e = column("region", "Region", Type::Enum);
    e.options = vec![Opt {
        key: "au".into(),
        label: "Australia".into(),
    }];
    assert_eq!(format::parse("au", &e), Ok(Value::Enum(0)));
    assert_eq!(format::parse("australia", &e), Ok(Value::Enum(0)));
    let b = column("k", "Key", Type::Bytes);
    assert_eq!(format::parse("01 ff", &b), Ok(Value::Bytes(vec![1, 255])));
    assert!(format::parse("é1", &b).is_err());
    let t = column("t", "Time", Type::Time);
    assert_eq!(
        format::value_text(&Value::Time(86_400.0 * 365.0), &t),
        "1971-01-01 00:00:00"
    );
}

#[test]
fn a_new_row_goes_to_the_front() {
    let mut r = Rows::default();
    assert_eq!(r.upsert("a", 2), 0);
    assert_eq!(r.upsert("b", 2), 0);
    assert_eq!(r.upsert("a", 2), 1);
    assert_eq!(r.rows[0].key, "b");
}
