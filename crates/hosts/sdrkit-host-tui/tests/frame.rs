//! The time of one frame of the application (the tick that reads the engine, and the drawing) while a stack runs on its
//! test signal, on the screen of the session and on the load screen.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use sdrkit_host_tui::api::Handle;
use sdrkit_host_tui::app::{App, Screen, Start};
use sdrkit_host_tui::ui;
use std::time::{Duration, Instant};

/// The mean and the longest time of `frames` frames of `stack` on `screen` at 15 frames per second, in ms.
fn frame_times(stack: &str, screen: Screen, frames: usize) -> (f64, f64) {
    let engine = Handle::engine_in_memory().expect("engine");
    let start = Start {
        stack: Some(stack.to_owned()),
        ..Start::default()
    };
    let mut app = App::new(engine, start).expect("app");
    let mut terminal = Terminal::new(TestBackend::new(160, 50)).expect("terminal");
    let t0 = Instant::now();
    while app.live.as_ref().is_none_or(|l| l.session.results.counter == 0) {
        app.tick();
        assert!(t0.elapsed() < Duration::from_secs(20), "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    // The user reaches a screen by its number.
    let number = Screen::ALL.iter().position(|s| *s == screen).expect("a screen of the bar") as u8;
    app.key(KeyEvent::from(KeyCode::Char(char::from(b'1' + number))));
    assert_eq!(app.screen, screen);
    let (mut sum, mut worst) = (0.0f64, 0.0f64);
    for _ in 0..frames {
        let t = Instant::now();
        app.tick();
        terminal.draw(|f| ui::draw(f, &app)).expect("draw");
        let dt = t.elapsed().as_secs_f64() * 1e3;
        sum += dt;
        worst = worst.max(dt);
        std::thread::sleep(Duration::from_millis(66));
    }
    if screen == Screen::Load {
        // The load screen shows the cores, the tables, and the queue of the session as the first connection.
        let text: String = terminal.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        for part in ["needs about", "Components by share", "Stages", "Connections", "queue"] {
            assert!(text.contains(part), "{stack}: the load screen has no \"{part}\": {text}");
        }
    }
    app.shutdown();
    (sum / frames as f64, worst)
}

#[test]
fn a_frame_takes_less_than_5_ms() {
    for (stack, screen) in [
        ("fm", Screen::Session),
        ("dvbt", Screen::Session),
        ("fm", Screen::Load),
        ("dvbt", Screen::Load),
    ] {
        let (mean, worst) = frame_times(stack, screen, 60);
        eprintln!("{stack} on {screen:?}: frame mean {mean:.2} ms, worst {worst:.2} ms");
        assert!(mean < 5.0, "{stack} on {screen:?}: mean {mean:.2} ms");
        // The longest frame is a figure of the optimised build: cargo test --release -p sdrkit-host-tui --test frame
        if !cfg!(debug_assertions) {
            assert!(worst < 5.0, "{stack} on {screen:?}: {worst:.2} ms");
        }
    }
}

#[test]
fn a_thread_opens_a_radio_through_an_engine_handle_of_its_own() {
    let engine = Handle::engine_in_memory().expect("engine");
    let own = engine.key().and_then(|k| engine.find(&k)).expect("a second engine handle");
    let radio = std::thread::spawn(move || own.radio_open("synth:fm", &[]))
        .join()
        .expect("thread")
        .expect("radio");
    let key = radio.key().expect("key");
    assert!(engine.find(&key).is_ok(), "the engine lists {key}");
}

#[test]
fn a_stack_starts_on_a_recording_that_a_thread_opens() {
    let folder = std::env::temp_dir().join(format!("sdrkit-tui-frame-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("folder");
    // One second of IQ at 1 Msps as a WAV file of two 16-bit channels.
    let (rate, frames) = (1_000_000u32, 1_000_000u32);
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend((36 + frames * 4).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(rate.to_le_bytes());
    bytes.extend((rate * 4).to_le_bytes());
    bytes.extend(4u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend((frames * 4).to_le_bytes());
    bytes.extend((0..frames * 2).flat_map(|i| ((i % 251) as i16 * 40).to_le_bytes()));
    std::fs::write(folder.join("noise.wav"), bytes).expect("recording");
    let engine = Handle::engine_in(&folder).expect("engine");
    let start = Start {
        source: Some("file:noise.wav".to_owned()),
        stack: Some("fm".to_owned()),
        ..Start::default()
    };
    let mut app = App::new(engine, start).expect("app");
    let t0 = Instant::now();
    let mut longest = 0.0f64;
    while app.live.as_ref().is_none_or(|l| l.session.results.counter == 0) {
        let t = Instant::now();
        app.tick();
        longest = longest.max(t.elapsed().as_secs_f64() * 1e3);
        assert!(t0.elapsed() < Duration::from_secs(20), "no session: {}", app.status.text);
        std::thread::sleep(Duration::from_millis(10));
    }
    eprintln!("the longest tick while the recording opened and the stack started: {longest:.1} ms");
    assert_eq!(app.radios.len(), 1, "the radio is opened once");
    app.shutdown();
    let _ = std::fs::remove_dir_all(&folder);
}
