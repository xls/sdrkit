//! The region is a choice from the list of the data folder; the stack, the channel list and the scan take its data.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use sdrkit_host_tui::api::{Handle, Value};
use sdrkit_host_tui::app::{App, EditFor, Mode, Start};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The data folder of the repository.
fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data")
}

/// Starts DVB-T on its test signal with `start` and ticks until the results of its session are read.
fn session(start: Start) -> App {
    let engine = Handle::engine_in_memory().expect("engine");
    let mut app = App::new(engine, start).expect("app");
    tick_until(&mut app, |a| {
        a.live.as_ref().is_some_and(|l| l.session.results.counter > 0)
    });
    app
}

fn tick_until(app: &mut App, done: impl Fn(&App) -> bool) {
    let t0 = Instant::now();
    loop {
        app.tick();
        if done(app) {
            return;
        }
        assert!(t0.elapsed() < Duration::from_secs(20), "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn press(app: &mut App, code: KeyCode) {
    app.key(KeyEvent::new(code, KeyModifiers::NONE));
}

/// The options of the open list.
fn options(app: &App) -> (EditFor, Vec<String>, usize) {
    match &app.mode {
        Mode::Pick { what, options, sel, .. } => (*what, options.clone(), *sel),
        other => panic!("no list is open: {other:?}"),
    }
}

/// Moves the open list to option `to` and takes it.
fn take(app: &mut App, to: usize) {
    let (_, _, sel) = options(app);
    for _ in to..sel {
        press(app, KeyCode::Up);
    }
    for _ in sel..to {
        press(app, KeyCode::Down);
    }
    press(app, KeyCode::Enter);
}

/// Setting `key` of the running stack, as the text that the application shows.
fn stack_setting(app: &App, key: &str) -> String {
    let l = app.live.as_ref().expect("session");
    let i = l.stack.settings.iter().position(|d| d.key == key).expect(key);
    let mut v = Value::None;
    l.stack.handle.get(i as u32, &mut v).expect("the value");
    sdrkit_host_tui::format::value_text(&v, &l.stack.settings[i])
}

/// Field `key` of the scan service.
fn scan_field(app: &App, key: &str) -> Value {
    let r = &app.live.as_ref().expect("session").scan.as_ref().expect("scan").results;
    let i = r.decl.fields.iter().position(|d| d.key == key).expect(key);
    r.fields[i].clone()
}

#[test]
fn the_region_is_chosen_from_a_list_and_the_scan_visits_its_channels() {
    let mut app = session(Start {
        stack: Some("dvbt".into()),
        data: Some(data()),
        ..Start::default()
    });
    assert!(app.regional.is_none());
    // The list of the regions: code and name, from the data.
    press(&mut app, KeyCode::Char('g'));
    let (what, regions, _) = options(&app);
    assert_eq!(what, EditFor::Region);
    let au = regions.iter().position(|r| r == "AU  Australia").expect("AU");
    assert!(regions.iter().any(|r| r.starts_with("DE  ")));
    take(&mut app, au);
    assert!(!app.status.error, "{}", app.status.text);
    assert_eq!(app.regional.as_ref().map(|r| r.region.country()), Some("AU"));
    // The running stack has the channel width and the channel list of the country.
    assert_eq!(stack_setting(&app, "bandwidth"), "7 MHz");
    let channels = stack_setting(&app, "channels");
    assert!(channels.starts_with("177500000;184500000;191500000"), "{channels}");
    // The list of the channels: label, frequency and width. Enter tunes.
    press(&mut app, KeyCode::Char('c'));
    let (what, channels, _) = options(&app);
    assert_eq!(what, EditFor::Channel);
    let seven = channels.iter().position(|c| c.starts_with("7 ")).expect("channel 7");
    assert!(channels[seven].contains("184.5 MHz") && channels[seven].contains("7 MHz"), "{}", channels[seven]);
    take(&mut app, seven);
    assert!(app.status.text.contains("184.5 MHz"), "{}", app.status.text);
    // The scan visits the 32 channels of the country, the first at 177.5 MHz.
    app.scan();
    assert!(app.status.text.contains("32 channels"), "{}", app.status.text);
    tick_until(&mut app, |a| matches!(scan_field(a, "count"), Value::I64(n) if n > 0));
    assert_eq!(scan_field(&app, "count"), Value::I64(32));
    assert_eq!(scan_field(&app, "freq_hz"), Value::Hz(177.5e6));
    app.scan();
    app.shutdown();
}

#[test]
fn an_unknown_region_is_refused_with_a_note() {
    let app = session(Start {
        stack: Some("dvbt".into()),
        region: Some("XX".into()),
        data: Some(data()),
        ..Start::default()
    });
    assert!(app.regional.is_none());
    assert!(app.status.text.contains("no plans for XX"), "{}", app.status.text);
}

#[test]
fn every_stack_takes_the_settings_of_its_preset() {
    use sdrkit_host_tui::known::Regional;
    let engine = Handle::engine_in_memory().expect("engine");
    let app = App::new(engine, Start::default()).expect("app");
    let mut with_settings = 0;
    for code in ["AU", "US", "DE", "JP"] {
        let r = Regional::open(&data(), code, None).expect("the region");
        for s in &app.stacks {
            if r.preset(&s.id, None, None).expect("the preset").is_none() {
                continue;
            }
            let set = r
                .settings(&app.engine.handle, &s.id, None, None)
                .unwrap_or_else(|e| panic!("{code}: {e}"));
            with_settings += usize::from(!set.is_empty());
        }
    }
    assert!(with_settings > 40, "{with_settings}");
}

#[test]
fn the_chosen_region_is_kept_for_the_next_run() {
    let file = std::env::temp_dir().join(format!("sdrkit-tui-{}/tui.conf", std::process::id()));
    let _ = std::fs::remove_file(&file);
    let start = Start {
        data: Some(data()),
        prefs: Some(file.clone()),
        ..Start::default()
    };
    let mut first = App::new(Handle::engine_in_memory().expect("engine"), start.clone()).expect("app");
    assert!(first.regional.is_none());
    first.set_region("de").expect("the region");
    first.shutdown();
    assert_eq!(std::fs::read_to_string(&file).expect("the file"), "region=DE\n");
    // The next run starts with the region of the file; `--region` replaces it for that run.
    let next = App::new(Handle::engine_in_memory().expect("engine"), start.clone()).expect("app");
    assert_eq!(next.regional.as_ref().map(|r| r.region.country()), Some("DE"));
    let given = Start {
        region: Some("AU".into()),
        ..start
    };
    let other = App::new(Handle::engine_in_memory().expect("engine"), given).expect("app");
    assert_eq!(other.regional.as_ref().map(|r| r.region.country()), Some("AU"));
    let _ = std::fs::remove_dir_all(file.parent().expect("the folder"));
}
