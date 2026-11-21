//! The sound of the application from the test signal of a stack to an output that counts: the application runs as on
//! a screen, and the output takes the place of the device.

use sdrkit_host_tui::api::{self, AudioInfo, Handle};
use sdrkit_host_tui::app::{App, Start};
use sdrkit_host_tui::sound::{Fill, Meter, Output, SoundError, Source, Speaker};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The tests of this file run one after the other: each needs its stack to run in real time.
static ALONE: Mutex<()> = Mutex::new(());

/// What the outputs received.
#[derive(Default)]
struct Tally {
    /// Every audio sample, with the number of its output.
    samples: Vec<(usize, f32)>,
    /// The time and the audio frames of each buffer that held audio.
    buffers: Vec<(Instant, usize)>,
    /// The rate and the channel count of each output that was opened.
    opened: Vec<(f64, u16)>,
}

/// An output that behaves as a device: it takes 120 ms to open, asks for a buffer every 10 ms on a thread of its own,
/// and stops at the buffer after it was dropped.
#[derive(Default)]
struct Counting(Arc<Mutex<Tally>>, bool);

/// Stops the thread of a [`Counting`] output when dropped.
struct Running(Arc<AtomicBool>);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

impl Output for Counting {
    fn open(&self, rate_hz: f64, channels: u16, mut fill: Fill) -> Result<Box<dyn std::any::Any>, SoundError> {
        std::thread::sleep(Duration::from_millis(120));
        let tally = Arc::clone(&self.0);
        // The second field: the output keeps every sample.
        let keep = self.1;
        tally.lock().unwrap().opened.push((rate_hz, channels));
        let on = Arc::new(AtomicBool::new(true));
        let running = Running(Arc::clone(&on));
        std::thread::spawn(move || {
            let period = Duration::from_millis(10);
            let mut buffer = vec![0.0f32; (rate_hz / 100.0) as usize * usize::from(channels)];
            let mut next = Instant::now();
            while on.load(Ordering::Relaxed) {
                let n = fill(&mut buffer);
                if n > 0 {
                    let frames = n / usize::from(channels);
                    let mut t = tally.lock().unwrap();
                    t.buffers.push((Instant::now(), frames));
                    if keep {
                        let output = t.opened.len();
                        t.samples.extend(buffer[..n].iter().map(|x| (output, *x)));
                    }
                }
                next += period;
                std::thread::sleep(next.saturating_duration_since(Instant::now()));
            }
        });
        Ok(Box::new(running))
    }
}

/// The samples that the shown session of `app` has worked on.
fn samples(app: &App) -> f64 {
    let r = &app.live.as_ref().expect("a session").session.results;
    let i = r.decl.fields.iter().position(|d| d.key == "session.samples");
    match i.and_then(|i| r.fields.get(i)) {
        Some(api::Value::I64(n)) => *n as f64,
        _ => 0.0,
    }
}

/// Runs `stack` on its test signal in the application for `time` after the first audio reached an output. Returns the
/// frames that the outputs received, the frames that the rate of the last output makes in the signal that the session
/// worked on in that time (the test signal of a build without optimisation can be slower than real time), the
/// longest time without audio in ms, and the number of outputs opened.
fn play(stack: &str, time: Duration) -> (usize, f64, f64, usize) {
    let engine = Handle::engine_in_memory().expect("engine");
    let start = Start {
        stack: Some(stack.to_owned()),
        ..Start::default()
    };
    let mut app = App::new(engine, start).expect("app");
    let tally = Arc::new(Mutex::new(Tally::default()));
    app.output = Arc::new(Counting(Arc::clone(&tally), false));
    let t0 = Instant::now();
    let first = loop {
        app.tick();
        if let Some(b) = tally.lock().unwrap().buffers.first() {
            break b.0;
        }
        assert!(t0.elapsed() < Duration::from_secs(30), "{stack}: no audio: {}", app.status.text);
        std::thread::sleep(Duration::from_millis(66));
    };
    let at_first = samples(&app);
    while first.elapsed() < time {
        app.tick();
        std::thread::sleep(Duration::from_millis(66));
    }
    let end = Instant::now();
    app.tick();
    let radio = &app.radios[0].handle;
    let mut rate = api::Value::None;
    radio.get(radio.setting_find("rate").expect("rate"), &mut rate).expect("rate");
    let api::Value::Hz(input_hz) = rate else {
        panic!("the rate of the radio is {rate:?}");
    };
    let signal_s = ((samples(&app) - at_first) / input_hz).min((end - first).as_secs_f64());
    let t = tally.lock().unwrap();
    let frames: usize = t.buffers.iter().map(|b| b.1).sum();
    let rate = t.opened.last().expect("an output").0;
    let mut gap = Duration::ZERO;
    for pair in t.buffers.windows(2) {
        gap = gap.max(pair[1].0 - pair[0].0);
    }
    gap = gap.max(end - t.buffers.last().expect("a buffer").0);
    let outputs = t.opened.len();
    drop(t);
    let r = &app.live.as_ref().expect("a session").session.results;
    let lost = r.decl.fields.iter().position(|d| d.key == "session.audio_lost");
    assert_eq!(lost.and_then(|i| r.fields.get(i)), Some(&api::Value::I64(0)), "{stack}: audio lost");
    eprintln!("{stack}: {signal_s:.2} s of signal in {:.2} s", (end - first).as_secs_f64());
    app.shutdown();
    (frames, rate * signal_s, gap.as_secs_f64() * 1e3, outputs)
}

/// Asserts that the output of `stack` received at least 95 % of 10 s of audio with no gap over 100 ms.
fn plays_without_a_gap(stack: &str) {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    let (frames, expected, gap_ms, outputs) = play(stack, Duration::from_secs(10));
    eprintln!("{stack}: {frames} of {expected:.0} frames, longest gap {gap_ms:.0} ms, {outputs} outputs");
    assert!(frames as f64 >= 0.95 * expected, "{stack}: {frames} of {expected:.0} frames");
    assert!(gap_ms <= 100.0, "{stack}: a gap of {gap_ms:.0} ms");
}

#[test]
fn fm_plays_for_10_s_without_a_gap() {
    plays_without_a_gap("fm");
}

#[test]
fn dab_plays_for_10_s_without_a_gap() {
    plays_without_a_gap("dab");
}

/// Audio that starts at 24 kHz mono and goes on at 48 kHz stereo, as a decoder that learns the format of its stream.
/// Sample n has the value n.
struct Scripted {
    next: usize,
}

/// The samples of the first format, and of both.
const FIRST: usize = 12_000;
const BOTH: usize = FIRST + 96_000;

impl Source for Scripted {
    fn audio_read(&mut self, out: &mut [f32]) -> api::Result<AudioInfo> {
        let (end, rate_hz, channels) = if self.next < FIRST {
            (FIRST, 24e3, 1)
        } else {
            (BOTH, 48e3, 2)
        };
        let n = (end - self.next).min(out.len());
        let n = n - n % usize::from(channels);
        for (i, o) in out[..n].iter_mut().enumerate() {
            *o = (self.next + i) as f32;
        }
        self.next += n;
        Ok(AudioInfo {
            frames: n / usize::from(channels),
            rate_hz,
            channels,
        })
    }
}

#[test]
fn a_change_of_the_audio_format_loses_no_sample() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    let tally = Arc::new(Mutex::new(Tally::default()));
    let output = Arc::new(Counting(Arc::clone(&tally), true));
    let mut speaker = Speaker::start(output, Scripted { next: 0 }, Arc::new(Meter::default()));
    let t0 = Instant::now();
    while tally.lock().unwrap().samples.len() < BOTH && t0.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(speaker.failed().is_none());
    assert_eq!(speaker.format(), Some((48e3, 2)));
    drop(speaker);
    let t = tally.lock().unwrap();
    assert_eq!(t.opened, [(24e3, 1), (48e3, 2)], "one output for each format");
    assert_eq!(t.samples.len(), BOTH, "samples played");
    for (n, (output, x)) in t.samples.iter().enumerate() {
        assert_eq!(*x, n as f32, "sample {n} is in its place");
        assert_eq!(*output, if n < FIRST { 1 } else { 2 }, "sample {n} plays in its format");
    }
}
