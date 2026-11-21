//! The entry line of the session screen, on an engine with its files in memory and the test signal of a stack.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use sdrkit_host_tui::api::{Error, Handle};
use sdrkit_host_tui::app::{App, Screen, Start, ENTRY_PANE, TABLES_PANE};
use std::time::{Duration, Instant};

/// Starts `stack` on its test signal and ticks until its tables and the results of its session are read.
fn session(stack: &str) -> App {
    let engine = Handle::engine_in_memory().expect("engine");
    let start = Start {
        stack: Some(stack.to_owned()),
        ..Start::default()
    };
    let mut app = App::new(engine, start).expect("app");
    let t0 = Instant::now();
    loop {
        app.tick();
        let live = app.live.as_ref().expect("session");
        if !app.shown_tables().is_empty() && live.session.results.counter > 0 {
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

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}

/// Shows the table with `key` and returns its place among the shown tables.
fn show_table(app: &mut App, key: &str) -> usize {
    let tables = app.shown_tables();
    let mut keys = tables.iter().map(|(r, t)| r.decl.tables[*t].key.as_str());
    let at = keys.position(|k| k == key).expect("table");
    while app.sel.table != at {
        press(app, KeyCode::Char(']'));
    }
    at
}

#[test]
fn a_stack_whose_first_command_takes_a_text_has_an_entry_line() {
    for (stack, command) in [("meshtastic", "mt.send"), ("meshcore", "mc.send")] {
        let mut app = session(stack);
        let cmd = app.entry_command().expect("entry command");
        assert_eq!(cmd.key, command);
        assert_eq!(cmd.args[1].key, "channel");
        assert_eq!(app.panes(), 3);

        // The session opens on the messages.
        let tables = app.shown_tables();
        let (r, t) = tables[app.sel.table];
        assert_eq!(r.decl.tables[t].key, "messages");
        assert!(!app.entry_columns().is_empty());

        // Tab goes from the controls to the line and then to the tables; `i` goes to the line from each pane.
        assert_eq!(app.pane, 0);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.pane, ENTRY_PANE);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.pane, TABLES_PANE);
        press(&mut app, KeyCode::Char('i'));
        assert!(app.on_entry());

        // Every character goes into the line; no key of the screen acts.
        type_text(&mut app, "q 4[]/i");
        assert!(!app.quit);
        assert_eq!(app.screen, Screen::Session);
        assert_eq!(app.live.as_ref().expect("session").entry.text, "q 4[]/i");

        // Esc leaves the line and keeps the text.
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.pane, TABLES_PANE);
        assert_eq!(app.live.as_ref().expect("session").entry.text, "q 4[]/i");
        app.shutdown();
    }
}

#[test]
fn enter_on_a_channel_row_chooses_the_channel_and_a_second_enter_clears_it() {
    let mut app = session("meshtastic");
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Tab);
    show_table(&mut app, "channels");
    let t0 = Instant::now();
    while app.shown_tables()[app.sel.table].0.tables[app.sel.table]
        .rows
        .is_empty()
    {
        assert!(t0.elapsed() < Duration::from_secs(20), "timed out");
        std::thread::sleep(Duration::from_millis(10));
        app.tick();
    }
    let tables = app.shown_tables();
    let (r, t) = tables[app.sel.table];
    let name = r.tables[t].rows[0]
        .key
        .rsplit('/')
        .next()
        .expect("name")
        .to_owned();

    press(&mut app, KeyCode::Enter);
    let entry = &app.live.as_ref().expect("session").entry;
    assert_eq!(entry.chosen("channel"), Some(name.as_str()));
    assert_eq!(app.status.text, format!("channel: {name}"));

    press(&mut app, KeyCode::Enter);
    let entry = &app.live.as_ref().expect("session").entry;
    assert_eq!(entry.chosen("channel"), None);

    // A table without such a column keeps the action that Enter has on its rows.
    show_table(&mut app, "nodes");
    assert!(app.entry_columns().is_empty());
    app.shutdown();
}

#[test]
fn a_text_that_cannot_go_on_air_stays_in_the_line_with_the_reason() {
    let mut app = session("meshtastic");
    let blocked = app
        .entry_blocked()
        .expect("the test signal has no transmitter");
    press(&mut app, KeyCode::Char('i'));

    // An empty line does not run the command.
    press(&mut app, KeyCode::Enter);
    assert!(!app.live.as_ref().expect("session").entry.error.is_empty());

    type_text(&mut app, "hello");
    press(&mut app, KeyCode::Enter);
    let entry = &app.live.as_ref().expect("session").entry;
    assert_eq!(entry.text, "hello");
    assert!(entry.error.ends_with(&blocked), "{}", entry.error);
    assert!(!entry.error.contains("SDRKIT_E"));
    app.shutdown();
}

#[test]
fn a_command_that_ran_clears_the_line_and_a_refused_one_keeps_it_with_the_message() {
    let mut app = session("meshtastic");
    press(&mut app, KeyCode::Char('i'));
    type_text(&mut app, "hello");

    let refusal = Error {
        code: -1,
        message: "the text is longer than a packet".to_owned(),
    };
    app.entry_ran("Send: done".to_owned(), Err(refusal), None);
    let entry = &app.live.as_ref().expect("session").entry;
    assert_eq!(entry.text, "hello");
    assert_eq!(entry.error, "the text is longer than a packet");

    app.entry_ran("Send: done".to_owned(), Ok(()), None);
    let entry = &app.live.as_ref().expect("session").entry;
    assert_eq!((entry.text.as_str(), entry.error.as_str()), ("", ""));
    assert_eq!(app.status.text, "Send: done");
    assert!(!app.status.error);
    assert!(app.on_entry());
    app.shutdown();
}

#[test]
fn a_stack_without_such_a_command_has_no_entry_line() {
    let mut app = session("fm");
    assert!(app.entry_command().is_none());
    assert_eq!(app.panes(), 2);
    assert_eq!(app.sel.table, 0);
    press(&mut app, KeyCode::Char('i'));
    assert_eq!(app.pane, 0);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.pane, TABLES_PANE);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.pane, 0);
    app.shutdown();
}
