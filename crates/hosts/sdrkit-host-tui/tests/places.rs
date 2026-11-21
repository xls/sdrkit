//! What the application remembers: the row of each table, the table of the session and of each service, the pane of each
//! screen and the line of each object. On an engine with its files in memory, the test
//! signal of the DVB-T stack and the data folder of the helper's tests.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use sdrkit_host_tui::api::Handle;
use sdrkit_host_tui::app::{App, Screen, Start, TABLES_PANE};
use std::time::{Duration, Instant};

fn session(stack: &str) -> App {
    let engine = Handle::engine_in_memory().expect("engine");
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../helpers/sdrkit-presets/tests/fixture"
    );
    let start = Start {
        stack: Some(stack.to_owned()),
        region: Some("AU".to_owned()),
        city: Some("Sydney".to_owned()),
        data: Some(fixture.into()),
        ..Start::default()
    };
    let mut app = App::new(engine, start).expect("app");
    let t0 = Instant::now();
    loop {
        app.tick();
        let live = app.live.as_ref().expect("session");
        if live.session.results.counter > 0 {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(20), "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    app.tick();
    app
}

fn press(app: &mut App, code: KeyCode) {
    app.key(KeyEvent::new(code, KeyModifiers::NONE));
}

/// The key of the shown table and the key of its selected row (empty for a table without rows).
fn place(app: &App) -> (String, String) {
    let tables = app.shown_tables();
    let (r, t) = tables[app.sel.table];
    let row = r.tables[t].rows.get(app.sel.row);
    (
        r.decl.tables[t].key.clone(),
        row.map(|x| x.key.clone()).unwrap_or_default(),
    )
}

fn show_table(app: &mut App, key: &str) {
    for _ in 0..app.shown_tables().len() {
        if place(app).0 == key {
            return;
        }
        press(app, KeyCode::Char(']'));
    }
    panic!("no table {key}");
}

/// The table of the known stations of the country: its rows come from the data folder and do not change.
const STATIONS: &str = "known_stations";

#[test]
fn a_table_returns_to_the_row_it_was_left_on() {
    let mut app = session("dvbt");
    assert_eq!(app.screen, Screen::Session);
    app.pane = TABLES_PANE;
    show_table(&mut app, STATIONS);
    let rows = |app: &App| {
        let tables = app.shown_tables();
        let (r, t) = tables[app.sel.table];
        r.tables[t].rows.len()
    };
    assert!(rows(&app) >= 3, "{} rows", rows(&app));

    // A row chosen in one table is the row of that table after other tables were shown.
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    let left = place(&app);
    assert_eq!(left.0, STATIONS);
    press(&mut app, KeyCode::Char(']'));
    press(&mut app, KeyCode::Char(']'));
    assert_ne!(place(&app).0, STATIONS);
    press(&mut app, KeyCode::Char('['));
    press(&mut app, KeyCode::Char('['));
    assert_eq!(place(&app), left);

    // The other screens and back: the same table, the same row, the same pane.
    for screen in ['7', '4', '6', '3'] {
        press(&mut app, KeyCode::Char(screen));
    }
    assert_eq!(app.screen, Screen::Session);
    assert_eq!(app.pane, TABLES_PANE);
    assert_eq!(place(&app), left);
}

#[test]
fn each_object_keeps_its_line_of_the_settings() {
    let mut app = session("dvbt");
    press(&mut app, KeyCode::Char('4'));
    press(&mut app, KeyCode::Right);
    for _ in 0..3 {
        press(&mut app, KeyCode::Down);
    }
    let (object, line) = (app.target, app.sel.setting);
    assert_eq!(line, 3);
    // Another object, a line there, then the first object again.
    press(&mut app, KeyCode::Left);
    press(&mut app, KeyCode::Down);
    assert_ne!(app.target, object);
    press(&mut app, KeyCode::Right);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Left);
    press(&mut app, KeyCode::Up);
    assert_eq!((app.target, app.sel.setting), (object, 3));
    press(&mut app, KeyCode::Right);
    // Another screen and back.
    press(&mut app, KeyCode::Char('3'));
    press(&mut app, KeyCode::Char('4'));
    assert_eq!((app.target, app.sel.setting, app.pane), (object, 3, 1));
}
