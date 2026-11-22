//! A message that a stack reads from its test signal has the time of the host's clock: the engine makes the signal
//! as it plays.

use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Value, ValueKind, Values};
use sdrkit_host_engine::OwnerRef;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The first value of the time kind of the newest row of table `table` of stack `stack` on its test signal, once
/// the table has a row. `None` if no row came in `wait`, or the row has no time.
fn first_time(stack: &str, table: &str, wait: Duration) -> Option<f64> {
    let sdr = sdrkit_host_catalog::engine_on(Arc::new(FakeTransport::new())).unwrap();
    let schema = sdr.radio_schema("synth", stack).unwrap();
    let radio = sdr
        .open_radio("synth", stack, &Values::defaults(&schema))
        .unwrap();
    let session = sdr.start(stack, radio, None).unwrap();
    let began = Instant::now();
    let mut found = None;
    while began.elapsed() < wait && found.is_none() {
        std::thread::sleep(Duration::from_millis(200));
        found = sdr
            .owner(OwnerRef::Session(session), |o| {
                let r = o.results();
                let t = r.table(r.decl().table_id(table)?)?;
                let (_, row) = t.rows_newest_first().next()?;
                let at = t
                    .columns()
                    .fields()
                    .iter()
                    .position(|c| c.kind == ValueKind::Time)?;
                Some(match row.value(sdrkit_core::record::FieldId(at as u16)) {
                    Some(Value::Time(t)) => Some(*t),
                    _ => None,
                })
            })
            .ok()
            .flatten();
    }
    let _ = sdr.close_radio(radio);
    found.flatten()
}

#[test]
fn an_acars_message_of_the_test_signal_has_a_time() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let t =
        first_time("acars", "messages", Duration::from_secs(30)).expect("a message with a time");
    assert!((t - now).abs() < 120.0, "{t} against {now}");
}
