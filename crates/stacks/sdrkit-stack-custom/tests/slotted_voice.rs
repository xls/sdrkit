//! `slotted_voice` frame version 3 and the beacon through the transmitter and the receiver (Part C section 6).

mod common;

use common::{power, shift, Noise};
use sdrkit_codec_custom::voice3::{self, V3Rate, Variant};
use sdrkit_core::Complex32;
use sdrkit_stack_custom::slotted_voice::beacon::Beacon;
use sdrkit_stack_custom::slotted_voice::test_speech;
use sdrkit_stack_custom::slotted_voice::v3::{self, Event, Receiver, Transmitter, P_DATA};

const ALL: [V3Rate; 4] = [V3Rate::R12, V3Rate::R16, V3Rate::R24, V3Rate::R32];

fn beacon() -> Beacon {
    Beacon {
        callsign: "TEST1".into(),
        lat: 54.687,
        lon: 25.279,
        power_dbm: 10.0,
        link: 0,
        grid: "KO24ab".into(),
        text: b"TEST1 test beacon".to_vec(),
    }
}

/// The 48 kHz signal of `seconds` of test speech.
fn signal(rate: V3Rate, call: &str, seconds: f64, b: Option<Beacon>) -> (Vec<Complex32>, Vec<Vec<u8>>) {
    let pcm = test_speech((seconds * 16_000.0) as usize, 3);
    let mut tx = Transmitter::new(rate, call, b);
    let mut out = Vec::new();
    for c in pcm.chunks(1234) {
        tx.push(c, &mut out);
    }
    tx.finish(&mut out);
    // The codec frames the transmitter sent.
    let v = Variant::V3(rate);
    let mut enc = voice3::Encoder::new(v);
    let mut frames = Vec::new();
    let mut f = vec![0u8; v.frame_bytes()];
    for c in pcm.as_chunks::<{ voice3::FRAME_SAMPLES }>().0 {
        enc.encode(c, &mut f).unwrap();
        frames.push(f.clone());
    }
    (out, frames)
}

struct Run {
    rx: Receiver,
    frames: Vec<Option<Vec<u8>>>,
    beacons: Vec<(bool, Beacon)>,
}

fn receive(x: &[Complex32], chunk: usize) -> Run {
    let mut rx = Receiver::new();
    let mut ev = Vec::new();
    for c in x.chunks(chunk) {
        rx.process(c, &mut ev);
    }
    let mut frames = Vec::new();
    let mut beacons = Vec::new();
    for e in ev {
        match e {
            Event::Speech(_, f) => frames.push(f),
            Event::Beacon(h) => beacons.push((h.full, h.beacon)),
        }
    }
    Run { rx, frames, beacons }
}

/// Noise variance for a per-sample SNR over the data power.
fn noise_for(snr_db: f64) -> f64 {
    0.16 * P_DATA / 10f64.powf(snr_db / 10.0)
}

/// The per-sample SNR of an Eb/N0 at information length `k`.
fn snr_of_ebn0(ebn0: f64, k: f64) -> f64 {
    ebn0 + 10.0 * (k / 480.0).log10() + 10.0 * 4f64.log10() - 10.0 * 6f64.log10()
}

/// The received codec frames follow the sent ones without a gap.
fn frames_match(got: &[Option<Vec<u8>>], sent: &[Vec<u8>]) -> usize {
    let good: Vec<&Vec<u8>> = got.iter().flatten().collect();
    assert!(!good.is_empty());
    let at = sent.iter().position(|f| f == good[0]).expect("a received frame was sent");
    let mut n = 0;
    for (g, s) in good.iter().zip(&sent[at..]) {
        assert_eq!(*g, s);
        n += 1;
    }
    n
}

#[test]
fn clean_loopback_at_each_bit_rate() {
    for rate in ALL {
        let (x, sent) = signal(rate, "TEST1", 4.0, None);
        let r = receive(&x, 1000);
        assert_eq!(r.rx.report.words_bad, 0, "{rate:?}");
        assert_eq!(r.rx.report.rate, Some(rate));
        let n = frames_match(&r.frames, &sent);
        assert!(n >= 80, "{rate:?}: {n}");
    }
}

#[test]
fn decode_at_eb_n0_10_db() {
    let (mut x, _) = signal(V3Rate::R16, "TEST1", 6.0, None);
    Noise::new(1).add(&mut x, noise_for(snr_of_ebn0(10.0, 320.0)));
    let r = receive(&x, 1000);
    let (g, b) = (r.rx.report.words_good, r.rx.report.words_bad);
    assert!(g + b >= 150, "{g} {b}");
    assert!((b as f64) < 0.05 * (g + b) as f64, "{g} {b}");
}

#[test]
fn carrier_offsets_of_200_hz() {
    for hz in [-200.0, 200.0] {
        let (mut x, _) = signal(V3Rate::R16, "TEST1", 5.0, None);
        Noise::new(2).add(&mut x, noise_for(snr_of_ebn0(12.0, 320.0)));
        shift(&mut x, hz, 48_000.0);
        let r = receive(&x, 1000);
        let (g, b) = (r.rx.report.words_good, r.rx.report.words_bad);
        assert!(g > 50 && (b as f64) < 0.05 * (g + b) as f64, "{hz}: {g} {b}");
        assert!((r.rx.report.carrier_hz - hz).abs() < 30.0, "{hz}: {}", r.rx.report.carrier_hz);
    }
}

#[test]
fn start_in_the_middle() {
    let (mut x, _) = signal(V3Rate::R16, "TEST1", 6.0, None);
    Noise::new(3).add(&mut x, noise_for(snr_of_ebn0(12.0, 320.0)));
    let r = receive(&x[100_003..], 1000);
    let (g, b) = (r.rx.report.words_good, r.rx.report.words_bad);
    assert!(g >= 60 && (b as f64) < 0.05 * (g + b) as f64, "{g} {b}");
    assert!(r.frames.len() * 320 >= 16_000);
}

#[test]
fn the_sweep_spells_the_station() {
    let (x, _) = signal(V3Rate::R16, "OH2XYZ/MM", 26.0, None);
    let r = receive(&x, 4000);
    assert_eq!(r.rx.report.sweep_id.as_deref(), Some("OH2XYZ/MM"));
    let s = r.rx.report.sweep_db.unwrap();
    assert!((-12.5..-7.5).contains(&s), "{s}");
}

#[test]
fn no_lock_on_noise() {
    let mut x = vec![Complex32::new(0.0, 0.0); 48_000 * 3];
    Noise::new(9).add(&mut x, 0.01);
    let r = receive(&x, 1000);
    assert_eq!(r.rx.report.words_good, 0);
}

#[test]
fn beacons_in_the_voice_stream() {
    let (x, _) = signal(V3Rate::R16, "TEST1", 8.5, Some(beacon()));
    let r = receive(&x, 1000);
    // The full frame at 0 s comes before the lock; the headers at 2, 4 and 6 s follow (and 8 s when it is whole).
    let headers: Vec<&(bool, Beacon)> = r.beacons.iter().filter(|b| !b.0).collect();
    assert!(headers.len() >= 3, "{:?}", r.beacons);
    let b = &headers[0].1;
    assert_eq!(b.callsign, "TEST1");
    assert_eq!(b.grid, "KO24ab");
    assert!((b.lat - 54.687).abs() < 0.01 && (b.lon - 25.279).abs() < 0.01 && (b.power_dbm - 10.0).abs() < 0.6);
    // The beacon is 10 dB below the voice.
    let mut quiet = Transmitter::new(V3Rate::R16, "TEST1", None);
    let mut y = Vec::new();
    quiet.push(&test_speech(16_000, 3), &mut y);
    assert!(power(&x[..y.len()]) > power(&y));
}

/// The full beacon frames at 0 s and 10 s: the receiver locks on the frames already in its first buffer, so it reads the one
/// at the start of the stream too.
#[test]
fn the_full_beacon_frame_after_ten_seconds() {
    let (x, _) = signal(V3Rate::R12, "TEST1", 15.0, Some(beacon()));
    let r = receive(&x, 2000);
    let full: Vec<&(bool, Beacon)> = r.beacons.iter().filter(|b| b.0).collect();
    assert_eq!(full.len(), 2, "{:?}", r.beacons);
    assert!(full.iter().all(|b| b.1.text == beacon().text));
}

#[test]
fn frames_block_and_words_round_trip() {
    let bits: Vec<u8> = (0..320).map(|i| ((i * 7) % 3 == 0) as u8).collect();
    let w = v3::words(&bits, V3Rate::R16);
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].len(), 480);
    let w32 = v3::words(&[bits.clone(), bits.clone()].concat(), V3Rate::R32);
    assert_eq!(w32.len(), 2);
}

/// Section 6.2, time to lock and audio: 12 kbit/s, per-sample SNR 10 and 40 dB, carrier 0, +300, -1000 and +1000 Hz, the
/// stream from sample 0 or 7777: the lock and the first audio within 1.3 s of stream time.
#[test]
fn time_to_lock_and_audio() {
    let (x, _) = signal(V3Rate::R12, "TEST1", 3.0, Some(beacon()));
    for snr in [10.0, 40.0] {
        for hz in [0.0, 300.0, -1000.0, 1000.0] {
            for start in [0usize, 7777] {
                let mut y = x[start..].to_vec();
                Noise::new(start as u64 + 1).add(&mut y, noise_for(snr));
                shift(&mut y, hz, 48_000.0);
                let mut rx = Receiver::new();
                let mut ev = Vec::new();
                let (mut lock, mut audio, mut n) = (None, None, 0usize);
                for c in y.chunks(960) {
                    rx.process(c, &mut ev);
                    n += c.len();
                    if lock.is_none() && rx.report.locked {
                        lock = Some(n);
                    }
                    if audio.is_none() && ev.iter().any(|e| matches!(e, Event::Speech(..))) {
                        audio = Some(n);
                    }
                }
                let limit = (1.3 * 48_000.0) as usize;
                assert!(lock.is_some_and(|t| t <= limit) && audio.is_some_and(|t| t <= limit), "{snr} dB, {hz} Hz, from {start}: lock {lock:?}, audio {audio:?}");
            }
        }
    }
}

/// The identity the receiver reports after `y`.
fn sweep_id(y: &[Complex32]) -> Option<String> {
    let mut rx = Receiver::new();
    let mut ev = Vec::new();
    for c in y.chunks(48_000) {
        rx.process(c, &mut ev);
        ev.clear();
    }
    rx.finish();
    rx.report.sweep_id
}

/// Section 6.3, scanner: 40 s of LY2ABC/P at 16 kbit/s at per-sample SNR 60, 0, -10, -18 and -20 dB with carriers 0, +-100
/// and +150 Hz: the identity is read, by the in-lock reader or, for signals too weak to lock to, by the scanner. At -20 dB
/// with a random start and a carrier of +100 or -80 Hz, at least 9 of 10 seeds. Noise alone, and plain up sweeps (a callsign
/// the sweep cannot send) at 10 to -20 dB, give no identity.
#[test]
fn sweep_scanner_reads_weak_signals() {
    let (x, _) = signal(V3Rate::R16, "LY2ABC/P", 40.0, None);
    for (snr, hz) in [(60.0, 0.0), (0.0, 100.0), (-10.0, -100.0), (-18.0, 150.0), (-20.0, 0.0), (-20.0, -100.0)] {
        let mut y = x.clone();
        Noise::new(snr as u64 + 100).add(&mut y, noise_for(snr));
        shift(&mut y, hz, 48_000.0);
        assert_eq!(sweep_id(&y).as_deref(), Some("LY2ABC/P"), "{snr} dB, {hz} Hz");
    }
    let mut read = 0;
    for seed in 0..10u64 {
        let start = (seed * 1_234_567 % 1_000_000) as usize;
        let mut y = x[start..].to_vec();
        y.extend_from_slice(&x[..start]);
        Noise::new(seed + 200).add(&mut y, noise_for(-20.0));
        shift(&mut y, if seed % 2 == 0 { 100.0 } else { -80.0 }, 48_000.0);
        read += usize::from(sweep_id(&y).as_deref() == Some("LY2ABC/P"));
    }
    assert!(read >= 9, "{read} of 10");
    let mut n = vec![Complex32::new(0.0, 0.0); 40 * 48_000];
    Noise::new(300).add(&mut n, 1.0);
    assert_eq!(sweep_id(&n), None);
    let (up, _) = signal(V3Rate::R16, "NOT/SENDABLE/CALL", 40.0, None);
    for snr in [10.0, 0.0, -10.0, -20.0] {
        let mut y = up.clone();
        Noise::new(snr as u64 + 400).add(&mut y, noise_for(snr));
        assert_eq!(sweep_id(&y), None, "{snr} dB");
    }
}

/// The recordings of section 9.5 in both sample forms, through the stack's receiver graph at 48 ksps: version 3 at 12 and 16
/// kbit/s (8000 symbols a second) and at 24 and 32 kbit/s (16000 symbols a second), beacon on. Every word decodes, at the
/// symbol rate and bit rate of the file, and the beacon names the station.
#[test]
fn recorded_chain() {
    use sdrkit_core::publish::keys;
    use sdrkit_core::{InputInfo, Output, StackConfig, Values};
    use sdrkit_stack_custom::typed::SlottedVoiceFields;
    let Some(root) = common::recordings() else { return };
    let dir = common::folder_with(&root, "_voice1_say_12kbps_cf32.sigmf-data");
    let d = &sdrkit_stack_custom::slotted_voice::SLOTTED_VOICE;
    for (stem, kbps, sym) in [("_voice1_say_12kbps", 12.0, 8000.0), ("_voice1_say", 16.0, 8000.0), ("_voice1_say_24kbps", 24.0, 16_000.0), ("_voice1_say_32kbps", 32.0, 16_000.0)] {
        for (name, rate, y) in common::both_forms(&dir, stem) {
            assert_eq!(rate, 48e3, "{name}");
            let mut rx = (d.make_receiver)(&StackConfig::new(Values::defaults(&(d.settings)()))).unwrap();
            rx.configure(&InputInfo::new(rate, 0.0, 0.0).recording()).unwrap();
            let mut out = Output::default();
            for c in y.chunks(48_000) {
                rx.feed(c, &mut out);
                out.clear();
            }
            rx.finish(&mut out);
            let r = rx.results().clone();
            let f = r.fields();
            let (ok, bad) = (f.get(&keys::FRAMES_OK).unwrap_or(0), f.get(&keys::FRAMES_BAD).unwrap_or(0));
            assert_eq!(f.get(&keys::SYMBOL_RATE_HZ), Some(sym), "{name}");
            assert_eq!(f.at(&SlottedVoiceFields::VOICE_KBPS), Some(kbps), "{name}");
            assert!(ok >= 2000 && bad == 0, "{name}: {ok} good, {bad} bad");
            assert_eq!(f.get(&keys::CALLSIGN), Some("LY2ABC"), "{name}");
        }
    }
}

#[test]
fn in_lock_identity_at_15_db_and_150_hz() {
    for hz in [150.0, -150.0] {
        let (mut x, _) = signal(V3Rate::R16, "LY2ABC/P", 26.0, None);
        Noise::new(11).add(&mut x, noise_for(15.0));
        shift(&mut x, hz, 48_000.0);
        let r = receive(&x, 4000);
        assert_eq!(r.rx.report.sweep_id.as_deref(), Some("LY2ABC/P"), "{hz}");
    }
}

#[test]
fn beacon_frames_at_eb_n0_6_db() {
    use sdrkit_stack_custom::slotted_voice::beacon::{frame_bits, slice, BeaconReceiver};
    let mut b = beacon();
    b.text = b"TEST1 beacon text at 6 dB".to_vec();
    let (mut full, mut head) = (0, 0);
    for seed in 0..5u64 {
        let mut x = vec![Complex32::new(0.0, 0.0); 4000];
        x.extend(slice(&frame_bits(&b, true).unwrap(), 48_000.0));
        x.extend(vec![Complex32::new(0.0, 0.0); 3000]);
        x.extend(slice(&frame_bits(&b, false).unwrap(), 48_000.0));
        x.extend(vec![Complex32::new(0.0, 0.0); 6000]);
        // Eb/N0 per information bit at rate 1/4 and 48 samples a symbol; the frames have unit power.
        let var = 48.0 / (10f64.powf(0.6) / 4.0);
        Noise::new(100 + seed).add(&mut x, var);
        let mut rx = BeaconReceiver::new();
        let mut heard = Vec::new();
        for c in x.chunks(2000) {
            rx.process(c, &mut heard);
        }
        assert!(heard.len() <= 2, "{seed}: {heard:?}");
        full += heard.iter().filter(|h| h.full && h.beacon.text == b.text).count();
        head += heard.iter().filter(|h| !h.full).count();
    }
    assert!(full >= 4 && head >= 4, "{full} {head}");
}

/// Section 9.3: 1 s of a 440 Hz tone through the early-format transmitter at 48 kHz without the beacon: 63072 samples, the
/// listed values within 0.004 after the output gain, the same output for any input chunk size, and at 24 kHz about half the
/// length with the same mean power within 6 %.
#[test]
fn transmitter_vector_of_the_early_format() {
    use sdrkit_stack_custom::slotted_voice::v3::EarlyTransmitter;
    let pcm: Vec<i16> = (0..16_000).map(|i| (8000.0 * (std::f64::consts::TAU * 440.0 * f64::from(i) / 16_000.0).sin()).trunc() as i16).collect();
    let run = |chunk: usize| {
        let mut tx = EarlyTransmitter::new();
        let mut out = Vec::new();
        for c in pcm.chunks(chunk) {
            tx.push(c, &mut out);
        }
        tx.finish(&mut out);
        out
    };
    let y = run(1000);
    assert_eq!(y.len(), 63_072);
    let want: [(usize, f32, f32); 14] = [
        (0, 0.0998296, -0.0751763),
        (1, 0.0299193, -0.1243485),
        (2, -0.0515766, -0.1229105),
        (3, -0.1103579, -0.0720987),
        (4, -0.1218235, 0.0067612),
        (5, -0.0814515, 0.0812970),
        (6, -0.0006252, 0.1154341),
        (7, 0.0747291, 0.1076366),
        (1000, 0.4140123, -0.1916594),
        (5000, 0.1599347, -0.0480500),
        (20_000, -0.3340012, -0.1974967),
        (40_000, -0.4698866, -0.0584272),
        (62_000, -0.3787077, 0.1090496),
        (63_071, -0.0520771, -0.1229842),
    ];
    for (i, re, im) in want {
        let e = (y[i] - Complex32::new(re, im) * 0.4).norm();
        assert!(e < 0.004, "sample {i}: {:?}, error {e}", y[i]);
    }
    assert_eq!(run(333), y);
    let mut half = Vec::new();
    sdrkit_algo_dsp::resample::Resampler::new(48_000.0, 24_000.0).unwrap().process(&y, &mut half);
    assert!((63_072 / 2 - 40..=63_072 / 2 + 60).contains(&half.len()), "{}", half.len());
    let r = power(&half) / power(&y);
    assert!((0.94..=1.06).contains(&r), "{r}");
}

/// Section 6.2, radio chain: each bit rate sent through a transmit converter at 2 MHz, received at 2.4 or 2.048 Msps with a
/// clock error of up to 5 ppm and a carrier of up to 7 kHz, as 8-bit samples, through the stack's receiver graph: locked
/// within 12 s and never lost after, more than 300 good frames, fewer than 1 bad in 20, the symbol rate and the bit rate
/// read, the first beacon within 14 s, and the beacon text received.
#[test]
fn radio_chain() {
    use sdrkit_algo_dsp::resample::Resampler;
    use sdrkit_core::publish::keys;
    use sdrkit_core::{InputInfo, Output, StackConfig, Values};
    use sdrkit_stack_custom::typed::SlottedVoiceFields;
    let d = &sdrkit_stack_custom::slotted_voice::SLOTTED_VOICE;
    for (rate, rx_rate, ppm, hz, kbps, sym) in [
        (V3Rate::R12, 2.4e6, 5.0, 7000.0, 12.0, 8000.0),
        (V3Rate::R16, 2.048e6, -5.0, -7000.0, 16.0, 8000.0),
        (V3Rate::R24, 2.4e6, 3.0, -3000.0, 24.0, 16_000.0),
        (V3Rate::R32, 2.048e6, -2.0, 0.0, 32.0, 16_000.0),
    ] {
        let case = format!("{kbps} kbit/s, {rx_rate} sps, {ppm} ppm, {hz} Hz");
        let (x, _) = signal(rate, "TEST1", 16.0, Some(beacon()));
        let fs = if sym > 8000.0 { 96_000.0 } else { 48_000.0 };
        let mut dac = Vec::new();
        Resampler::new(fs, 2e6).unwrap().process(&x, &mut dac);
        let mut y = Vec::new();
        Resampler::new(2e6, rx_rate * (1.0 + ppm * 1e-6)).unwrap().process(&dac, &mut y);
        drop(dac);
        shift(&mut y, hz, rx_rate);
        let peak = y.iter().map(|v| v.re.abs().max(v.im.abs())).fold(0f32, f32::max);
        Noise::new(kbps as u64).add(&mut y, f64::from(peak * peak) * 1e-4);
        let q = |v: f32| f32::from((v / peak * 0.8 * 127.0).round().clamp(-128.0, 127.0) as i8) / 127.0;
        let mut rx = (d.make_receiver)(&StackConfig::new(Values::defaults(&(d.settings)()))).unwrap();
        rx.configure(&InputInfo::new(rx_rate, 0.0, 0.0).recording()).unwrap();
        let mut out = Output::default();
        let (mut lock_at, mut lost, mut beacon_at) = (None, false, None);
        let chunk = (rx_rate * 0.25) as usize;
        for (i, c) in y.chunks(chunk).enumerate() {
            let c: Vec<Complex32> = c.iter().map(|v| Complex32::new(q(v.re), q(v.im))).collect();
            rx.feed(&c, &mut out);
            out.clear();
            let t = (i + 1) as f64 * 0.25;
            let f = rx.results().fields().clone();
            match (lock_at, f.get(&keys::LOCKED) == Some(true)) {
                (None, true) => lock_at = Some(t),
                (Some(_), false) => lost = true,
                _ => {}
            }
            if beacon_at.is_none() && f.get(&keys::CALLSIGN) == Some("TEST1") {
                beacon_at = Some(t);
            }
        }
        rx.finish(&mut out);
        let r = rx.results().clone();
        let f = r.fields();
        let (ok, bad) = (f.get(&keys::FRAMES_OK).unwrap_or(0), f.get(&keys::FRAMES_BAD).unwrap_or(0));
        assert!(lock_at.is_some_and(|t| t <= 12.0) && !lost, "{case}: lock at {lock_at:?}, lost {lost}");
        assert!(ok > 300 && bad * 20 < ok, "{case}: {ok} good, {bad} bad");
        assert_eq!((f.get(&keys::SYMBOL_RATE_HZ), f.at(&SlottedVoiceFields::VOICE_KBPS)), (Some(sym), Some(kbps)), "{case}");
        assert!(beacon_at.is_some_and(|t| t <= 14.0), "{case}: beacon at {beacon_at:?}");
        let table = r.decl().table_id("stations").and_then(|id| r.table(id)).expect("stations");
        let text = table.row("TEST1").and_then(|row| row.get(&keys::TEXT)).unwrap_or_default().to_string();
        assert_eq!(text, "TEST1 test beacon", "{case}");
    }
}
