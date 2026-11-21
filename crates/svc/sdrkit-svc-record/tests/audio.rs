//! Audio recording through the service on generated audio: tones separated by silence.

mod support;

use sdrkit_core::{Choice, StorePath, Value};
use sdrkit_graph::stream::SampleType;
use sdrkit_svc_record::{read_recording, AudioContainer, AudioFeed, Container, Feed, Gate, Split};
use support::*;

const RATE: f64 = 8000.0;

/// (seconds, tone on) pieces of stereo audio: a 440 Hz tone at half scale on the left and its opposite on the right, or
/// silence.
fn program(pieces: &[(f64, bool)]) -> Vec<(Vec<f32>, bool)> {
    let mut n = 0u64;
    pieces
        .iter()
        .map(|&(s, on)| {
            let len = (s * RATE).round() as usize;
            let mut v = Vec::with_capacity(len * 2);
            for _ in 0..len {
                n += 1;
                let x = if on {
                    0.5 * (2.0 * std::f32::consts::PI * 440.0 * n as f32 / RATE as f32).sin()
                } else {
                    0.0
                };
                v.extend([x, -x]);
            }
            (v, on)
        })
        .collect()
}

/// Pushes `prog` in pieces of 333 frames, with the squelch open on the tones.
fn play(f: &mut AudioFeed, prog: &[(Vec<f32>, bool)]) {
    for (v, on) in prog {
        for c in v.chunks(666) {
            f.push(c, 2, RATE, *on);
        }
    }
}

fn audio(feed: Feed) -> AudioFeed {
    match feed {
        Feed::Audio(f) => *f,
        f => panic!("not an audio feed: {f:?}"),
    }
}

/// (channels, frames) of a WAV file, after making sure that its header matches its length.
fn wav_info(b: &[u8]) -> (u16, usize) {
    assert_eq!(&b[0..4], b"RIFF");
    let ch = u16::from_le_bytes([b[22], b[23]]);
    let data = u32::from_le_bytes([b[40], b[41], b[42], b[43]]) as usize;
    assert_eq!(data + 44, b.len());
    (ch, data / 2 / ch as usize)
}

const PIECES: [(f64, bool); 7] = [
    (1.0, false),
    (2.0, true),
    (3.0, false),
    (0.3, true), // too short: deleted
    (3.0, false),
    (1.5, true),
    (3.0, false),
];

fn per_transmission() -> Vec<(&'static str, Value)> {
    vec![
        ("prefix", text("rx")),
        ("split", Value::Choice(Split::Transmission.index())),
        ("gate", Value::Choice(Gate::Level.index())),
        ("preroll", Value::Duration(0.25)),
        ("hang", Value::Duration(1.0)),
        ("min_length", Value::Duration(1.0)),
        ("log", Value::Bool(true)),
        ("reserve_bytes", Value::I64(0)),
    ]
}

#[test]
fn one_file_per_transmission_by_level() {
    let mut rig = Rig::new(&per_transmission());
    let (row, feed) = rig.start("audio.start", &[("session", text("s1"))]);
    let mut f = audio(feed);
    f.set_tuning("nfm", 145.5e6);
    play(&mut f, &program(&PIECES));
    rig.call("audio.stop", &[("row", text(&row))]).unwrap();
    drop(f);
    rig.settle();
    let files = rig.files("wav");
    assert_eq!(files.len(), 2, "{files:?}");
    assert_eq!(rig.events_of("record.ended").len(), 2);
    // Pre-roll + tone + hang, within the 10 ms of the level gate.
    for (name, tone) in files.iter().zip([2.0, 1.5]) {
        let (ch, frames) = wav_info(&rig.read(name));
        assert_eq!(ch, 2);
        let s = frames as f64 / RATE;
        assert!((s - (0.25 + tone + 1.0)).abs() < 0.03, "{name} {s}");
        assert!(
            name.starts_with("recordings/rx_nfm_145.5000MHz_20250101-1"),
            "{name}"
        );
    }
    let csv = String::from_utf8(rig.read("recordings/rx.csv")).unwrap();
    assert_eq!(csv.lines().count(), 3);
    assert!(
        csv.lines().nth(1).unwrap().contains(",145500000,nfm,"),
        "{csv}"
    );
    // The recordings table reads the files back.
    let recs = rows(&rig.svc, "recordings");
    assert_eq!(recs.len(), 2);
    assert_eq!(cell(&recs[0].1, "channels"), Value::I64(2));
}

#[test]
fn squelch_gate_mono_and_flac() {
    let mut set = per_transmission();
    set.push(("gate", Value::Choice(Gate::Squelch.index())));
    set.push((
        "audio_container",
        Value::Choice(AudioContainer::Flac.index()),
    ));
    let mut rig = Rig::new(&set);
    let (_, feed) = rig.start("audio.start", &[("session", text("s1"))]);
    let mut f = audio(feed);
    // Only the right channel: the mono mix is half of it.
    let prog: Vec<(Vec<f32>, bool)> = program(&PIECES)
        .into_iter()
        .map(|(v, on)| (v.chunks(2).flat_map(|p| [0.0, p[1]]).collect(), on))
        .collect();
    play(&mut f, &prog);
    drop(f);
    rig.settle();
    let files = rig.files("flac");
    assert_eq!(files.len(), 2, "{files:?}");
    let r = read_recording(&*rig.storage, &StorePath::new(&files[0]).unwrap()).unwrap();
    assert_eq!(r.container, Container::Flac);
    assert_eq!((r.format.rate, r.format.channels), (RATE, 1));
    assert_eq!(r.format.sample_type, Some(SampleType::I16));
    // The squelch moves at the pieces exactly: pre-roll + 2 s + hang.
    assert_eq!(r.frames, (RATE * 3.25) as u64);
}

#[test]
fn whole_stereo_in_parts_and_mono() {
    let mut rig = Rig::new(&[
        ("max_file", Value::Duration(1.0)),
        ("reserve_bytes", Value::I64(0)),
    ]);
    let (_, feed) = rig.start("audio.start", &[("session", text("s1"))]);
    let mut f = audio(feed);
    play(&mut f, &program(&[(1.0, false), (2.5, true)]));
    drop(f);
    rig.settle();
    let files = rig.files("wav");
    assert_eq!(files.len(), 4, "{files:?}");
    let frames: Vec<usize> = files
        .iter()
        .map(|f| {
            let (ch, n) = wav_info(&rig.read(f));
            assert_eq!(ch, 2);
            n
        })
        .collect();
    assert_eq!(frames.iter().sum::<usize>(), 28_000);
    assert!(frames.iter().all(|&n| n <= 8000 + 333));
    // Left and right are kept apart.
    let b = rig.read(files.last().unwrap());
    let l = i16::from_le_bytes([b[44 + 400], b[45 + 400]]);
    let rr = i16::from_le_bytes([b[46 + 400], b[47 + 400]]);
    assert_eq!(l, -rr);

    // Mono mixes the channels: opposite channels give silence. A mono stream stores one channel.
    let mut rig = Rig::new(&[
        ("mono", Value::Bool(true)),
        ("reserve_bytes", Value::I64(0)),
    ]);
    let (_, feed) = rig.start("audio.start", &[("session", text("s1"))]);
    let mut f = audio(feed);
    play(&mut f, &program(&[(0.5, true)]));
    let mono: Vec<f32> = vec![0.25; 4000];
    f.push(&mono, 1, RATE, true);
    drop(f);
    rig.settle();
    let files = rig.files("wav");
    assert_eq!(files.len(), 2, "{files:?}");
    let b = rig.read(&files[0]);
    assert_eq!(wav_info(&b), (1, 4000));
    assert!(b[44..].iter().all(|&x| x == 0));
    let b = rig.read(&files[1]);
    assert_eq!(wav_info(&b), (1, 4000));
    assert_eq!(i16::from_le_bytes([b[44], b[45]]), 8192);
}

#[test]
fn a_transmission_shorter_than_the_minimum_is_deleted_and_channel_iq_goes_beside() {
    let mut set = per_transmission();
    set.push(("gate", Value::Choice(Gate::Squelch.index())));
    set.push(("channel_iq", Value::Bool(true)));
    let mut rig = Rig::new(&set);
    let (_, feed) = rig.start("audio.start", &[("session", text("s1"))]);
    let mut f = audio(feed);
    f.set_tuning("usb", 145.5e6);
    let iq = vec![sdrkit_core::Complex32::new(0.5, -0.5); 480];
    for (v, on) in program(&[
        (0.5, false),
        (0.4, true),
        (2.0, false),
        (2.0, true),
        (2.0, false),
    ]) {
        for c in v.chunks(160) {
            f.push_iq(&iq, 48_000.0);
            f.push(c, 2, RATE, on);
        }
    }
    drop(f);
    rig.settle();
    assert_eq!(rig.files("wav").len(), 1);
    let iq_files = rig.files("cf32");
    assert_eq!(iq_files.len(), 1, "{iq_files:?}");
    assert!(iq_files[0].ends_with("_0.048Msps.cf32"), "{iq_files:?}");
    let r = read_recording(&*rig.storage, &StorePath::new(&iq_files[0]).unwrap()).unwrap();
    assert_eq!((r.format.rate, r.format.centre_hz), (48_000.0, 145.5e6));
    // Pre-roll + 2 s + hang, at 6 IQ samples per audio frame.
    let s = r.frames as f64 / 48_000.0;
    assert!((s - 3.25).abs() < 0.05, "{s}");
}

#[test]
fn refusals() {
    let mut rig = Rig::new(&[]);
    // A folder that is not a store path is refused when it is set.
    let id = sdrkit_core::owner::Owner::schema(&rig.svc)
        .id("folder")
        .unwrap();
    assert!(sdrkit_core::owner::apply(&mut rig.svc, id, &text("../up")).is_err());
    // A reserve larger than the storage holds refuses the start.
    rig.storage.set_limit(Some(1 << 20));
    let id = sdrkit_core::owner::Owner::schema(&rig.svc)
        .id("reserve_bytes")
        .unwrap();
    sdrkit_core::owner::apply(&mut rig.svc, id, &Value::I64(2 << 20)).unwrap();
    assert!(rig.call("audio.start", &[("session", text("s1"))]).is_err());
}

#[test]
fn the_squelch_can_be_ignored() {
    let mut set = per_transmission();
    set.push(("gate", Value::Choice(Gate::Always.index())));
    let mut rig = Rig::new(&set);
    let (_, feed) = rig.start("audio.start", &[("session", text("s1"))]);
    let mut f = audio(feed);
    for (v, _) in program(&[(1.0, false), (1.0, true)]) {
        for c in v.chunks(666) {
            f.push(c, 2, RATE, false);
        }
    }
    drop(f);
    rig.settle();
    let files = rig.files("wav");
    assert_eq!(files.len(), 1);
    assert_eq!(wav_info(&rig.read(&files[0])), (2, 16_000));
}
