//! any_stack: runs any stack by its id and prints its result fields and tables from their declarations. It knows no
//! stack: names, units and options come from the results at run time.
//!
//! `cargo run -p sdrkit-examples --example any_stack -- <stack> [source] [seconds] [rows]`
//!
//! - `stack`: a stack id, such as `fm`, `adsb` or `meshtastic`.
//! - `source`: `synth:<stack>` (the default) is the test signal of the stack. A radio is an address such as
//!   `rtl_tcp:192.168.1.20:1234`.
//! - `seconds`: how long to run before the results are printed, 5 by default.
//! - `rows`: the most rows to print of each table, 5 by default.

use sdrkit_core::{FieldDecl, Record, Value};
use sdrkit_host_engine::OwnerRef;
use std::error::Error;
use std::time::Duration;

/// `value` as text, with the unit or the option label of its declaration.
fn show(decl: &FieldDecl, value: &Value) -> String {
    let unit = if decl.unit.is_empty() {
        String::new()
    } else {
        format!(" {}", decl.unit)
    };
    match value {
        Value::None => "-".into(),
        Value::Bool(b) => if *b { "yes" } else { "no" }.into(),
        Value::I64(n) => format!("{n}{unit}"),
        Value::F64(x) | Value::Hz(x) | Value::Time(x) | Value::Duration(x) => format!("{x}{unit}"),
        Value::Text(t) => t.clone(),
        Value::Position { lat, lon } => format!("{lat:.5} {lon:.5}"),
        Value::Choice(i) => decl
            .choices
            .get(usize::from(*i))
            .map_or_else(|| i.to_string(), |c| c.label.to_string()),
        Value::Flags(_) => value.display(&[]).with_flags(&decl.flags).to_string(),
        Value::Bytes(b) => format!("{} bytes", b.len()),
    }
}

/// The cells of `record` that hold a value, as `key=value`.
fn cells(record: &Record) -> String {
    record
        .decl()
        .fields()
        .iter()
        .enumerate()
        .filter_map(|(i, decl)| {
            let value = record.value(sdrkit_core::FieldId(i as u16))?;
            (*value != Value::None).then(|| format!("{}={}", decl.key, show(decl, value)))
        })
        .collect::<Vec<_>>()
        .join("  ")
}

fn main() -> Result<(), Box<dyn Error>> {
    let arg = |n: usize| std::env::args().nth(n);
    let stack = arg(1).ok_or("usage: any_stack <stack> [source] [seconds] [rows]")?;
    let source = arg(2).unwrap_or_else(|| format!("synth:{stack}"));
    let seconds: f64 = arg(3).map_or(Ok(5.0), |s| s.parse())?;
    let most: usize = arg(4).map_or(Ok(5), |s| s.parse())?;

    let sdr = sdrkit_host_catalog::engine()?;
    let radio = sdr.open(&source)?;
    let session = sdr.start(&stack, radio, None)?;
    // Audio that no one reads is dropped, so a stack with sound needs no reader here.
    std::thread::sleep(Duration::from_secs_f64(seconds));

    let report = sdr.owner(OwnerRef::Stack(session), |o| {
        let results = o.results();
        let mut out = vec![format!("{stack} on {source}: {}", cells(results.fields()))];
        for (i, table) in results.decl().tables().iter().enumerate() {
            let Some(rows) = results.table(sdrkit_core::TableId(i as u16)) else {
                continue;
            };
            out.push(format!("table {}: {} rows", table.key, rows.len()));
            for (key, row) in rows.rows_newest_first().take(most) {
                out.push(format!("   {key}  {}", cells(row)));
            }
        }
        out
    })?;
    for line in report {
        println!("{line}");
    }
    sdr.stop_session(session)?;
    Ok(())
}
