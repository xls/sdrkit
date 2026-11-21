//! A stack that starts from the region data, and the table of the known stations of the region.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use sdrkit_host_tui::api::{Handle, Value};
use sdrkit_host_tui::app::{App, Start, TABLES_PANE};
use sdrkit_host_tui::known::KNOWN_STATIONS;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The data folder of the tests of `sdrkit-presets`.
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../helpers/sdrkit-presets/tests/fixture")
}

/// Starts DVB-T on its test signal with `start` and ticks until the results of its session are read.
fn session(start: Start) -> App {
    let engine = Handle::engine_in_memory().expect("engine");
    let mut app = App::new(engine, start).expect("app");
    let t0 = Instant::now();
    loop {
        app.tick();
        if app
            .live
            .as_ref()
            .is_some_and(|l| l.session.results.counter > 0)
        {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(20), "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    app
}

fn press(app: &mut App, code: KeyCode) {
    app.key(KeyEvent::new(code, KeyModifiers::NONE));
}

/// The frequency that the shown session is tuned to.
fn tuned(app: &App) -> f64 {
    let l = app.live.as_ref().expect("session");
    let id = l.tuned.expect("the setting of the frequency");
    let mut v = Value::None;
    l.session.handle.get(id, &mut v).expect("the frequency");
    match v {
        Value::Hz(hz) => hz,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_stack_starts_from_the_region_data_and_enter_tunes_to_a_known_station() {
    let mut app = session(Start {
        stack: Some("dvbt".into()),
        region: Some("AU".into()),
        city: Some("Sydney".into()),
        data: Some(fixture()),
        ..Start::default()
    });
    assert!(app.status.text.contains("226.5 MHz"), "{}", app.status.text);
    assert_eq!(tuned(&app), 226.5e6);
    let l = app.live.as_ref().expect("session");
    let bandwidth = l
        .stack
        .settings
        .iter()
        .position(|d| d.key == "bandwidth")
        .expect("bandwidth");
    let seven = l.stack.settings[bandwidth]
        .options
        .iter()
        .position(|o| o.key == "7")
        .expect("7 MHz");
    assert_eq!(l.stack.values[bandwidth], Value::Enum(seven as u32));

    // The table is the last one of the session screen, with the stations of the city first.
    let tables = app.shown_tables();
    let (r, t) = *tables.last().expect("tables");
    assert_eq!(r.decl.tables[t].key, KNOWN_STATIONS);
    let rows = &r.tables[t].rows;
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0].cells[0], Value::String("Sydney".into()));
    assert_eq!(rows[4].cells[0], Value::String("Melbourne".into()));
    let last = tables.len() - 1;

    // Enter on a row tunes the session to its frequency.
    app.pane = TABLES_PANE;
    while app.sel.table != last {
        press(&mut app, KeyCode::Char(']'));
    }
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    assert_eq!(tuned(&app), 191.5e6);
    app.shutdown();
}

#[test]
fn without_a_data_folder_the_stack_starts_as_without_region_data() {
    let nowhere = fixture().join("nowhere");
    let mut app = session(Start {
        stack: Some("dvbt".into()),
        region: Some("AU".into()),
        data: Some(nowhere.clone()),
        ..Start::default()
    });
    let want = format!("no region data loaded: {}", nowhere.display());
    assert!(app.status.text.contains(&want), "{}", app.status.text);
    assert!(app.live.as_ref().is_some_and(|l| l.known.is_none()));
    app.shutdown();
}
