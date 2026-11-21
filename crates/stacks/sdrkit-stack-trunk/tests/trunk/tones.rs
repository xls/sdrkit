//! The tone decoders: the recorded vector, the demonstration through FM, the gate, sideband input and any cut of
//! the input.

use crate::common::*;
use num_complex::Complex32;
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_core::Generator;
use sdrkit_core::Results;
use sdrkit_stack_trunk::tones::gen::{demo, Part, Scene, TonesGenerator};
use sdrkit_stack_trunk::typed::TONES_HEARD as EVENTS;
use sdrkit_stack_trunk::TONES;
use sdrkit_util_ids::tones::dcs::{decode, Dcs};

/// (kind, text, detail, start, length) of every row.
fn events(r: &Results) -> Vec<(String, String, String, f64, f64)> {
    rows(r, &EVENTS)
        .into_iter()
        .map(|e| {
            let kind = match e["kind"].as_str() {
                "ctcss" => "CTCSS",
                "dcs" => "DCS",
                "dtmf" => "DTMF",
                "selcall" => "Selcall",
                "selcal" => "SELCAL",
                "two_tone" => "2-tone",
                k => k,
            };
            (
                kind.to_string(),
                e["text"].clone(),
                e["detail"].clone(),
                e["start_s"].parse().unwrap(),
                e["length"].parse().unwrap(),
            )
        })
        .collect()
}

fn kinds_texts(r: &Results) -> Vec<(String, String)> {
    events(r).into_iter().map(|e| (e.0, e.1)).collect()
}

/// Feeds audio at `rate` (1.0 is 5 kHz of deviation) with 3 s of silence after it.
fn run_audio(
    audio: &[f32],
    rate: f64,
    settings: &[(&str, &str)],
) -> (Box<dyn sdrkit_core::Receiver>, Fed) {
    let mut s = vec![("audio_input", "true")];
    s.extend_from_slice(settings);
    let mut rx = make(&TONES, &s);
    configure(rx.as_mut(), rate, 0.0, 0.0);
    let mut x = audio_iq(audio);
    x.extend(std::iter::repeat_n(
        Complex32::new(0.0, 0.0),
        (rate * 3.0) as usize,
    ));
    let fed = feed(rx.as_mut(), &x, 4096);
    (rx, fed)
}

#[test]
fn recorded_vector_matches_expected() {
    let Some(rec) = sdrkit_dev_test::read_recording("trunk/tones_audio_16k") else {
        return;
    };
    let exp = sdrkit_dev_test::sigmf::read_expected("trunk").unwrap();
    let e = &exp["recordings"]["tones_audio_16k"];
    let mut rx = make(&TONES, &[("audio_input", "true")]);
    configure(rx.as_mut(), rec.rate_hz, 0.0, 0.0);
    let mut fed = feed(rx.as_mut(), &rec.samples, 4096);
    fed.events
        .extend(feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 48000], 4096).events);
    let got = events(results(rx.as_ref()));
    let want = e["events"].as_array().unwrap();
    assert_eq!(got.len(), want.len(), "{got:#?}");
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g.0, w["kind"].as_str().unwrap(), "{g:?}");
        if g.0 == "2-tone" {
            let p: Vec<f64> =
                g.1.trim_end_matches(" Hz")
                    .split(" + ")
                    .map(|x| x.parse().unwrap())
                    .collect();
            assert!(
                (p[0] - 682.3).abs() <= 4.0 && (p[1] - 1000.0).abs() <= 4.0,
                "{}",
                g.1
            );
        } else {
            assert_eq!(g.1, w["text"].as_str().unwrap(), "{g:?}");
            assert_eq!(g.2, w["detail"].as_str().unwrap(), "{g:?}");
        }
        assert!(
            (g.3 - w["t"].as_f64().unwrap()).abs() <= 0.3,
            "{g:?} vs {w}"
        );
        let tol = if g.0 == "CTCSS" || g.0 == "DCS" {
            0.6
        } else {
            0.3
        };
        assert!(
            (g.4 - w["dur_s"].as_f64().unwrap()).abs() <= tol,
            "{g:?} vs {w}"
        );
    }
    check_events(
        &kinds(&sdrkit_stack_trunk::tones::declaration()),
        &fed.events,
        false,
    );
}

#[test]
fn demonstration_through_fm_at_1_msps() {
    let audio = demo().audio(8000.0).unwrap();
    let mut g = TonesGenerator::looping(audio, 1e6);
    let mut rx = make(&TONES, &[]);
    configure(rx.as_mut(), 1e6, 145.5e6, 0.0);
    let mut x = vec![Complex32::new(0.0, 0.0); 100_000];
    let mut fed = Fed::default();
    for k in 0..340 {
        g.generate(&mut x);
        add_noise(&mut x, 0.02, k);
        let f = feed(rx.as_mut(), &x, 100_000);
        fed.events.extend(f.events);
    }
    let r = results(rx.as_ref());
    let k = kinds_texts(r);
    for want in [
        ("CTCSS", "123.0 Hz"),
        ("DCS", "D023N"),
        ("DTMF", "159#"),
        ("Selcall", "12345"),
        ("SELCAL", "AM-CP"),
    ] {
        assert!(
            k.contains(&(want.0.to_string(), want.1.to_string())),
            "{want:?}: {k:?}"
        );
    }
    assert!(k.iter().any(|e| e.0 == "2-tone"), "{k:?}");
    assert_eq!(num(r, "audio_input"), 0.0);
    assert_eq!(num(r, "input_rate_hz"), 1e6);
    check_events(
        &kinds(&sdrkit_stack_trunk::tones::declaration()),
        &fed.events,
        true,
    );
}

#[test]
fn gate_opens_on_the_chosen_tone_only() {
    for (tone, open) in [(123.0, true), (88.5, false)] {
        let sc = Scene::new(8.0)
            .with(Part::Tone(tone, 0.0, 8.0, 0.12))
            .with(Part::Tone(1000.0, 0.0, 8.0, 0.3));
        let (rx, fed) = run_audio(
            &sc.audio(8000.0).unwrap(),
            8000.0,
            &[
                ("gate", "chosen"),
                ("gate_kind", "ctcss"),
                ("gate_tone", "123.0"),
                ("listen", "true"),
            ],
        );
        let tail = &fed.audio[fed.audio.len() - 30000..fed.audio.len() - 26000];
        let rms = sdrkit_dev_test::measure::rms(tail);
        if open {
            assert!(rms > 0.02, "{rms}");
        } else {
            assert!(rms < 0.003, "{rms}");
        }
        let _ = rx;
    }
}

/// Two tones of a SELCAL code as single-sideband IQ: `sign` +1 for USB, -1 for LSB, `err` Hz added to every tone.
fn ssb_selcal(code: &str, rate: f64, sign: f64, err: f64) -> Vec<Complex32> {
    let t = sdrkit_util_ids::tones::selcal::Code::parse(code)
        .unwrap()
        .tones();
    let n = (4.0 * rate) as usize;
    (0..n)
        .map(|i| {
            let s = i as f64 / rate - 0.5;
            let mut z = Complex32::new(0.0, 0.0);
            for (a, b, at) in [(t[0], t[1], 0.0), (t[2], t[3], 1.2)] {
                if s >= at && s < at + 1.0 {
                    for f in [a as f64, b as f64] {
                        let ph = std::f64::consts::TAU * sign * (f + err) * i as f64 / rate;
                        z += Complex32::new(0.05 * ph.cos() as f32, 0.05 * ph.sin() as f32);
                    }
                }
            }
            z
        })
        .collect()
}

#[test]
fn selcal_on_usb_off_tune_and_not_on_the_wrong_sideband() {
    let rate = 500e3;
    let mut x = ssb_selcal("AM-CP", rate, 1.0, 15.0);
    add_noise(&mut x, 0.002, 5);
    let mut rx = make(&TONES, &[("demod", "usb")]);
    configure(rx.as_mut(), rate, 8.891e6, 0.0);
    feed(rx.as_mut(), &x, 8192);
    let k = kinds_texts(results(rx.as_ref()));
    assert!(k.contains(&("SELCAL".into(), "AM-CP".into())), "{k:?}");
    let mut x = ssb_selcal("AM-CP", rate, -1.0, 0.0);
    add_noise(&mut x, 0.002, 4);
    let mut rx = make(&TONES, &[("demod", "usb")]);
    configure(rx.as_mut(), rate, 8.891e6, 0.0);
    feed(rx.as_mut(), &x, 8192);
    assert!(kinds_texts(results(rx.as_ref()))
        .iter()
        .all(|e| e.0 != "SELCAL"));
}

#[test]
fn automatic_demodulation() {
    use sdrkit_stack_trunk::tones::front::Demod;
    assert_eq!(Demod::Auto.resolve(8.879e6), Demod::Usb);
    assert_eq!(Demod::Auto.resolve(145.5e6), Demod::Fm);
    assert_eq!(Demod::Auto.resolve(0.0), Demod::Fm);
}

/// The tone set named `name`.
fn zvei(name: &str) -> &'static sdrkit_util_ids::tones::selcall::ToneSet {
    sdrkit_util_ids::tones::selcall::by_name(name).unwrap()
}

#[test]
fn zvei2_and_zvei3_are_two_sets() {
    // A ZVEI3 call: digit 1 is 970 Hz and the repeat tone is 2400 Hz (the third tone repeats the second).
    let sc = Scene::new(3.0).with(Part::Selcall(zvei("ZVEI3"), "12234".into(), 0.5, None, 0.5));
    let (_rx, fed) = run_audio(&sc.audio(8000.0).unwrap(), 8000.0, &[]);
    let calls: Vec<_> = fed
        .events
        .iter()
        .filter(|e| e.kind() == "tones.selcall")
        .collect();
    assert_eq!(calls.len(), 1, "{:?}", fed.events);
    let c = calls[0];
    assert_eq!(field(c, "digits"), "12234");
    let set = field(c, "set");
    let also = field(c, "also_fits");
    let fits: Vec<&str> = std::iter::once(set.as_str())
        .chain(also.split(", "))
        .collect();
    assert!(fits.contains(&"ZVEI3"), "{fits:?}");
    assert!(!fits.contains(&"ZVEI2"), "{fits:?}");
    // And a ZVEI2 call reads as ZVEI2, with its own digits.
    let sc = Scene::new(3.0).with(Part::Selcall(zvei("ZVEI2"), "12234".into(), 0.5, None, 0.5));
    let (_rx, fed) = run_audio(&sc.audio(8000.0).unwrap(), 8000.0, &[]);
    let c = fed
        .events
        .iter()
        .find(|e| e.kind() == "tones.selcall")
        .unwrap();
    assert_eq!(field(c, "digits"), "12234");
    let set = field(c, "set");
    let also = field(c, "also_fits");
    assert!(
        set == "ZVEI2" || also.split(", ").any(|s| s == "ZVEI2"),
        "{set} {also}"
    );
}

/// DCS audio of `word` repeated, with the bit `wrong[k % 3]` of repetition `k` changed (`None`: no error), 0.15 of full scale.
fn dcs_audio(word: u32, wrong: [Option<u32>; 3], seconds: f64) -> Vec<f32> {
    let rate = 8000.0;
    let n = (seconds * rate) as usize;
    let mut lp = sdrkit_algo_dsp::iir::OnePole::new(rate, 250.0);
    (0..n)
        .map(|i| {
            let k = (i as f64 * 134.4 / rate) as u64;
            let rep = (k / 23) as usize;
            let bit = (k % 23) as u32;
            let mut w = word;
            if let Some(b) = wrong[rep % 3] {
                w ^= 1 << b;
            }
            let x = if (w >> bit) & 1 == 1 { 0.15 } else { -0.15 };
            lp.process_sample(x)
        })
        .collect()
}

#[test]
fn dcs_errors_are_repaired_by_the_vote_not_the_golay_code() {
    // A wrong bit in one of every three repetitions: the vote of the repeats repairs it.
    let word = Dcs::from_octal("023").unwrap().word();
    let mut audio = dcs_audio(word, [Some(3), None, Some(17)], 5.0);
    add_real_noise(&mut audio, 0.002, 7);
    let (rx, _) = run_audio(&audio, 8000.0, &[]);
    let k = kinds_texts(results(rx.as_ref()));
    assert!(k.contains(&("DCS".into(), "D023N".into())), "{k:?}");
    // The same wrong bit in every repetition is not a DCS word: no Golay correction turns it into a code.
    let audio = dcs_audio(word, [Some(17); 3], 5.0);
    let (rx, _) = run_audio(&audio, 8000.0, &[]);
    assert!(kinds_texts(results(rx.as_ref()))
        .iter()
        .all(|e| e.0 != "DCS"));
    // Noise alone never reads as a code.
    let mut noise = vec![0.0f32; 8000 * 20];
    add_real_noise(&mut noise, 0.1, 9);
    let (rx, _) = run_audio(&noise, 8000.0, &[]);
    assert!(kinds_texts(results(rx.as_ref())).is_empty());
}

#[test]
fn any_cut_returns_the_same_events() {
    let audio = demo().audio(8000.0).unwrap();
    let x = audio_iq(&audio[..8000 * 26]);
    let run = |chunk: usize| {
        let mut rx = make(&TONES, &[("audio_input", "true")]);
        configure(rx.as_mut(), 8000.0, 0.0, 0.0);
        feed(rx.as_mut(), &x, chunk);
        events(results(rx.as_ref()))
    };
    assert_eq!(run(4096), run(997));
}

/// The word of a transmitter that put the three fixed bits as the value 1 (bits 1 0 0 after the nine code bits, not 0 0 1) and
/// took the middle octal digit as the low three bits of the written code divided by ten.
fn word_with_both_faults(written: u16) -> u32 {
    let value = (written / 100) * 64 + ((written / 10) & 7) * 8 + written % 10;
    let data = u32::from(value) | (1 << 9);
    let parity = (SmallCode::golay_23_12().encode(u128::from(data)) & 0x7FF) as u32;
    (parity << 12) | data
}

#[test]
fn dcs_words_built_with_the_two_faults_read_as_other_codes() {
    // The shared Golay code gives the word of `Dcs::word` when the fixed bits are right.
    let golay = SmallCode::golay_23_12();
    for &v in &sdrkit_util_ids::tones::dcs::STANDARD {
        let data = u32::from(v) | (0b100 << 9);
        let parity = (golay.encode(u128::from(data)) & 0x7FF) as u32;
        assert_eq!((parity << 12) | data, Dcs::new(v).unwrap().word());
    }
    for (sent, read) in [
        ("023", "D411N"),
        ("025", "D412N"),
        ("065", "D432N"),
        ("226", "D053N"),
        ("703", "D627N"),
    ] {
        let written: u16 = sent.parse().unwrap();
        let wrong = decode(word_with_both_faults(written)).unwrap();
        assert_eq!(wrong.name(), read, "D{sent}N sent with both faults");
        let right = decode(Dcs::from_octal(sent).unwrap().word()).unwrap();
        assert_eq!(right.name(), format!("D{sent}N"));
    }
}
