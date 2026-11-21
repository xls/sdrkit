//! CW: the skimmer on three carriers, noise alone, the three listen choices, the recording, every span at the session's rate
//! and offset, 100 carriers, the filter widths at their speed limits and their sensitivity, the cost, the decoder of one
//! carrier, and the timing decoder on any input.

use crate::common::*;
use sdrkit_core::{Complex32, InputInfo, Output, SettingSet};
use sdrkit_dev_test::Run;
use sdrkit_stack_hfkbd as hf;

// Columns of the signals table.
const OFFSET: &str = "offset_hz";
const WPM: &str = "wpm";
const FARNSWORTH: &str = "farnsworth";
const CHARS: &str = "characters";
const TEXT: &str = "text";

fn cw(seconds: f64) -> sdrkit_dev_test::Harness {
    harness(&hf::CW).rate(24000.0).piece(2400).seconds(seconds)
}

/// SNR of the key-down carriers in 500 Hz.
fn snr_500(h: sdrkit_dev_test::Harness, db: f64) -> sdrkit_dev_test::Harness {
    h.set(&GEN_NOISE, NoiseKind::Snr).set(&GEN_SNR_DB, db)
}

fn found(r: &Run, f: f64, want: &str, wpm: f64) -> bool {
    found_in(&rows(r, "signals"), f, want, wpm)
}

fn found_in(rows: &[sdrkit_core::Record], f: f64, want: &str, wpm: f64) -> bool {
    rows.iter().any(|s| {
        (cell_num(s, OFFSET) - f).abs() < 30.0
            && cell_text(s, TEXT).contains(want)
            && (cell_num(s, WPM) - wpm).abs() < 0.12 * wpm
    })
}

/// [`found_in`] with the text compared without its word spaces: the shared timing decoder misses some word gaps once a long
/// pause is among the gaps it learned from, and these tests check the characters.
fn heard_in(rows: &[sdrkit_core::Record], f: f64, want: &str, wpm: f64) -> bool {
    let want = want.replace(' ', "");
    rows.iter().any(|s| {
        (cell_num(s, OFFSET) - f).abs() < 30.0
            && cell_text(s, TEXT).replace(' ', "").contains(&want)
            && (cell_num(s, WPM) - wpm).abs() < 0.12 * wpm
    })
}

/// The signals as lines of offset, speed and text, for a failure message.
fn show(rows: &[sdrkit_core::Record]) -> String {
    rows.iter()
        .map(|s| {
            format!(
                "{:+.0} Hz {:.1} WPM {:?}\n",
                cell_num(s, OFFSET),
                cell_num(s, WPM),
                cell_text(s, TEXT)
            )
        })
        .collect()
}

/// The default test signal: each of the three carriers is decoded, and they give three rows and no more, with noise and
/// without, over several seeds.
#[test]
fn cw_three_signals() {
    for (seed, noise) in [(1, true), (2, true), (3, true), (1, false)] {
        let h = cw(60.0).seed(seed);
        let r = run(&if noise { snr_500(h, 20.0) } else { h });
        let sigs = rows(&r, "signals");
        for (f, want, wpm) in [
            (400.0, "CQ DE K0TST", 20.0),
            (-1500.0, "TEST DE N0XYZ 599", 28.0),
            (2500.0, "VVV DE K0TST", 15.0),
        ] {
            assert!(
                found(&r, f, want, wpm),
                "{f}, seed {seed}:\n{}",
                show(&sigs)
            );
        }
        assert_eq!(
            sigs.len(),
            3,
            "seed {seed}, noise {noise}:\n{}",
            show(&sigs)
        );
        assert_eq!(num(&r, "signals"), 3.0);
        assert!(
            (num(&r, "now_s") - 60.0).abs() < 0.2,
            "{}",
            num(&r, "now_s")
        );
    }
}

/// The points of series `key` of the results.
fn series(r: &Run, key: &str) -> Vec<[f32; 2]> {
    let res = &r.results;
    res.decl()
        .series_id(key)
        .and_then(|id| res.series(id))
        .map(|s| s.points.clone())
        .unwrap_or_default()
}

/// The spectrum series covers the span in bins of about 12 Hz and shows each carrier above the noise; the key series of the
/// signal listened to (the strongest, without a focus) has its key changes of the last 2 s.
#[test]
fn cw_spectrum_and_key_series() {
    let r = run(&snr_500(cw(20.0), 20.0));
    let sp = series(&r, "spectrum");
    assert!(sp.len() > 1000, "{}", sp.len());
    let step = sp[1][0] - sp[0][0];
    assert!((10.0..14.0).contains(&step), "{step}");
    assert!(sp[0][0] < -6900.0 && sp[sp.len() - 1][0] > 6900.0);
    let mut levels: Vec<f32> = sp.iter().map(|p| p[1]).collect();
    levels.sort_by(f32::total_cmp);
    let median = levels[levels.len() / 2];
    for f in [400.0f32, -1500.0, 2500.0] {
        let at = sp
            .iter()
            .filter(|p| (p[0] - f).abs() < 20.0)
            .map(|p| p[1])
            .fold(f32::MIN, f32::max);
        assert!(at > median + 15.0, "{f}: {at} over {median}");
    }
    let key = series(&r, "key");
    let now = num(&r, "now_s");
    assert!(key.len() >= 4, "{key:?}");
    assert!(key.iter().any(|p| p[1] == 1.0) && key.iter().any(|p| p[1] == 0.0));
    assert!(key.iter().all(|p| (now - f64::from(p[0])) <= 2.01));
    // The series starts 2 s ago with the state then.
    assert!((now - 2.0 - f64::from(key[0][0])).abs() < 0.01, "{key:?}");
    // The signal listened to is the strongest row: the carrier at 400 Hz, which has the most power.
    let listened = num(&r, "listened");
    let row = rows(&r, "signals")
        .into_iter()
        .find(|s| cell_num(s, "id") == listened)
        .unwrap_or_else(|| panic!("no row {listened}"));
    assert!(cell_num(&row, "level_dbfs") > -200.0);
}

/// Right after the focus moves to another signal, the key series covers the last 2 s: its state 2 s ago, then its changes,
/// then its state now.
#[test]
fn cw_key_series_is_whole_after_a_focus_change() {
    let v = values(Span::Khz14, FilterWidth::Hz100, hf::cw::Listen::Off);
    let rate = 24e3;
    let specs = [
        hf::cw::CwSignalSpec::new(400.0, 20.0, "CQ DE K0TST K "),
        hf::cw::CwSignalSpec::new(-1500.0, 25.0, "TEST DE N0XYZ K "),
    ];
    let mut x = vec![Complex32::default(); (12.0 * rate) as usize];
    hf::decoder::Modulator::fill(&mut hf::cw::CwModulator::new(rate, &specs, 1), &mut x);
    sdrkit_algo_sim::noise::Awgn::with_power(n0_for(20.0) * rate, 2)
        .unwrap()
        .process(&mut x);
    let mut rx = (hf::CW.make_receiver)(&sdrkit_core::StackConfig::new(v)).unwrap();
    rx.configure(&InputInfo::new(rate, 14.07e6, 14.07e6).recording())
        .unwrap();
    let id = rx.schema().id("focus_hz").unwrap();
    let mut out = Output::default();
    for (k, c) in x.chunks(2400).enumerate() {
        if k == 100 {
            rx.apply(id, &sdrkit_core::Value::Hz(-1500.0)).unwrap();
        }
        rx.feed(c, &mut out);
        out.clear();
        if k >= 100 {
            let res = sdrkit_core::Owner::results(rx.as_ref());
            let key = res
                .decl()
                .series_id("key")
                .and_then(|i| res.series(i))
                .map(|s| s.points.clone())
                .unwrap_or_default();
            let now = field_num(res, "now_s");
            assert!(key.len() >= 2, "{k}: {key:?}");
            assert!(
                (now - 2.0 - f64::from(key[0][0])).abs() < 0.01,
                "{k}: {key:?}"
            );
            assert!(
                (now - f64::from(key[key.len() - 1][0])).abs() < 0.01,
                "{k}: {key:?}"
            );
            assert_eq!(field_num(res, "listened"), field_num(res, "focus"));
        }
    }
}

/// A focus where no signal is starts a picked signal, which shows in the table; once the focus moves away it is no longer
/// picked and leaves when quiet.
#[test]
fn cw_picked_signal_leaves_after_the_focus_moves() {
    let v = values(Span::Khz14, FilterWidth::Hz100, hf::cw::Listen::Off);
    let rate = 24e3;
    let mut x = vec![Complex32::default(); (45.0 * rate) as usize];
    hf::decoder::Modulator::fill(
        &mut hf::cw::CwModulator::new(
            rate,
            &[hf::cw::CwSignalSpec::new(400.0, 20.0, "CQ DE K0TST K ")],
            1,
        ),
        &mut x,
    );
    sdrkit_algo_sim::noise::Awgn::with_power(n0_for(20.0) * rate, 2)
        .unwrap()
        .process(&mut x);
    let mut rx = (hf::CW.make_receiver)(&sdrkit_core::StackConfig::new(v)).unwrap();
    rx.configure(&InputInfo::new(rate, 14.07e6, 14.07e6).recording())
        .unwrap();
    let id = rx.schema().id("focus_hz").unwrap();
    let mut out = Output::default();
    let picked = |rx: &dyn sdrkit_core::Receiver| {
        rows_of(rx, "signals")
            .into_iter()
            .filter(|r| (cell_num(r, OFFSET) + 3000.0).abs() < 30.0)
            .collect::<Vec<_>>()
    };
    for (k, c) in x.chunks(2400).enumerate() {
        if k == 10 {
            rx.apply(id, &sdrkit_core::Value::Hz(-3000.0)).unwrap();
        }
        if k == 30 {
            let p = picked(rx.as_ref());
            assert_eq!(p.len(), 1, "{}", show(&p));
            assert_eq!(cell_flag(&p[0], "manual"), Some(true));
            rx.apply(id, &sdrkit_core::Value::Hz(400.0)).unwrap();
        }
        rx.feed(c, &mut out);
        out.clear();
    }
    let rows = rows_of(rx.as_ref(), "signals");
    assert!(picked(rx.as_ref()).is_empty(), "{}", show(&rows));
    let f = rows
        .iter()
        .find(|r| (cell_num(r, OFFSET) - 400.0).abs() < 30.0)
        .unwrap();
    assert_eq!(cell_flag(f, "manual"), Some(true));
}

#[test]
fn cw_noise_only() {
    let x = {
        let mut x = vec![Complex32::default(); 24000 * 30];
        sdrkit_algo_sim::noise::Awgn::with_power(24000.0 / 2500.0, 3)
            .unwrap()
            .process(&mut x);
        x
    };
    let r = feed(&cw(30.0), &x);
    let chars: f64 = rows(&r, "signals").iter().map(|s| cell_num(s, CHARS)).sum();
    assert!(chars <= 2.0, "{chars}");
}

#[test]
fn cw_sidetone() {
    let h = cw(8.0)
        .set(&hf::decoder::GEN_MESSAGE, "CQ CQ DE K0TST K ")
        .set(&hf::cw::GEN_CARRIER_HZ, 1200.0)
        .set(&hf::cw::LISTEN, hf::cw::Listen::Tone)
        .set(&hf::cw::FOCUS_HZ, 1200.0);
    let r = run(&h);
    let rms = (r.audio.iter().map(|v| v * v).sum::<f32>() / r.audio.len().max(1) as f32).sqrt();
    assert!(rms > 0.02, "{rms}");
    let f = num(&r, "focus");
    let s = rows(&r, "signals")
        .into_iter()
        .find(|s| cell_num(s, "id") == f)
        .unwrap();
    assert!((cell_num(&s, OFFSET) - 1200.0).abs() < 30.0);
    let quiet = run(&cw(8.0)
        .set(&hf::decoder::GEN_MESSAGE, "CQ CQ DE K0TST K ")
        .set(&hf::cw::GEN_CARRIER_HZ, 1200.0));
    assert!(quiet.audio.iter().all(|&v| v == 0.0));
}

#[test]
fn cw_recording() {
    let name = "hfkbd_cw_12k";
    let e = expected(name);
    for piece in [800, 4093] {
        let r = recording(
            &harness(&hf::CW).piece(piece).set(&hf::cw::THRESHOLD, 10.0),
            name,
        );
        let sigs = rows(&r, "signals");
        for want in e["expect"]["signals"].as_array().unwrap() {
            let ok = sigs.iter().any(|s| {
                in_range(cell_num(s, OFFSET), &want["offset_hz"])
                    && cell_text(s, TEXT).contains(want["contains"].as_str().unwrap())
                    && in_range(cell_num(s, WPM), &want["wpm"])
                    && want["farnsworth"]
                        .as_bool()
                        .is_none_or(|f| cell_flag(s, FARNSWORTH) == Some(f))
            });
            assert!(ok, "{want} in\n{}", show(&sigs));
        }
    }
}

/// The decoder of one carrier reads hand keying at several speeds from the test signal of the family.
#[test]
fn morse_decoder_reads_one_carrier() {
    use sdrkit_graph::{DecoderComponent, GraphBuilder};
    for (wpm, jitter) in [(12.0, 0.0), (25.0, 0.06), (40.0, 0.0)] {
        let h = harness(&hf::CW)
            .rate(8000.0)
            .set(&hf::decoder::GEN_MESSAGE, "CQ DE K0TST K0TST K ")
            .set(&hf::cw::GEN_CARRIER_HZ, 600.0)
            .set(&hf::cw::GEN_WPM, wpm)
            .set(&hf::cw::GEN_JITTER, jitter)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, 15.0)
            .seconds(25.0);
        let x = h.generate().unwrap();
        let mut dec = hf::cw::MorseDecoder::new();
        sdrkit_graph::Decoder::apply(
            &mut dec,
            hf::cw::MorseSetting::OffsetHz.id(),
            &sdrkit_core::Value::Hz(600.0),
        )
        .unwrap();
        let mut g = GraphBuilder::new()
            .add("morse", DecoderComponent::new(dec))
            .input("morse")
            .output("morse.events")
            .status_from("morse")
            .clock(0.1)
            .build()
            .unwrap();
        sdrkit_core::Receiver::configure(&mut g, &InputInfo::new(8000.0, 0.0, 0.0).recording())
            .unwrap();
        let mut out = Output::default();
        for c in x.chunks(800) {
            sdrkit_core::Receiver::feed(&mut g, c, &mut out);
        }
        let res = sdrkit_core::Owner::results(&g);
        let text = field_text(res, "text");
        assert!(text.contains("DE K0TST K0TST K"), "{wpm}: {text:?}");
        let got = field_num(res, "wpm");
        assert!((got - wpm).abs() < 0.12 * wpm, "{got} vs {wpm}");
    }
}

/// The tune offset of the family: the radio is this far above the dial.
const TUNE_OFFSET: f64 = 20e3;

/// The CW stack with `span`, `width` and the listen choice `listen`, at its defaults otherwise.
fn values(
    span: hf::front::Span,
    width: FilterWidth,
    listen: hf::cw::Listen,
) -> sdrkit_core::Values {
    sdrkit_dev_test::Harness::new(&hf::CW)
        .set(&hf::front::SPAN, span)
        .set(&hf::cw::FILTER, width)
        .set(&hf::cw::LISTEN, listen)
        .values()
        .unwrap()
}

/// What a session gives the stack: the carriers `specs` (offsets from the dial) at the rate that the settings ask for, with
/// the radio 20 kHz above the dial, and white noise of `n0` per Hz. Returns the rows, the audio and the input rate.
fn session(
    v: &sdrkit_core::Values,
    specs: &[hf::cw::CwSignalSpec],
    n0: f64,
    seconds: f64,
    seed: u64,
) -> (Vec<sdrkit_core::Record>, Vec<f32>, f64) {
    use hf::decoder::Modulator;
    let rate = hf::CW.rate_hz(v);
    let moved: Vec<_> = specs
        .iter()
        .map(|s| {
            let mut s = s.clone();
            s.offset_hz -= TUNE_OFFSET;
            s
        })
        .collect();
    let mut x = vec![Complex32::default(); (seconds * rate) as usize];
    hf::cw::CwModulator::new(rate, &moved, seed).fill(&mut x);
    sdrkit_algo_sim::noise::Awgn::with_power(n0 * rate, seed + 1)
        .unwrap()
        .process(&mut x);
    let (rx, audio) = feed_at(v, &x, rate, TUNE_OFFSET);
    (rows_of(rx.as_ref(), "signals"), audio, rate)
}

/// Feeds `x` at `rate` to a new CW receiver with values `v`, the dial at 14.07 MHz and the centre `offset` above it.
fn feed_at(
    v: &sdrkit_core::Values,
    x: &[Complex32],
    rate: f64,
    offset: f64,
) -> (Box<dyn sdrkit_core::Receiver>, Vec<f32>) {
    let dial = 14.07e6;
    let mut rx = (hf::CW.make_receiver)(&sdrkit_core::StackConfig::new(v.clone())).unwrap();
    rx.configure(&InputInfo::new(rate, dial + offset, dial).recording())
        .unwrap();
    let mut out = Output::default();
    let mut audio = Vec::new();
    for c in x.chunks((rate / 10.0) as usize) {
        rx.feed(c, &mut out);
        audio.extend_from_slice(out.audio());
        out.clear();
    }
    (rx, audio)
}

/// The rows of table `key` of a receiver.
fn rows_of(rx: &dyn sdrkit_core::Receiver, key: &str) -> Vec<sdrkit_core::Record> {
    let res = sdrkit_core::Owner::results(rx);
    let Some(t) = res.decl().table_id(key).and_then(|id| res.table(id)) else {
        return Vec::new();
    };
    t.rows_newest_first().map(|(_, row)| row.clone()).collect()
}

/// White noise per Hz that puts a carrier of amplitude 1 at `snr_db` in 500 Hz.
fn n0_for(snr_db: f64) -> f64 {
    10f64.powf(-snr_db / 10.0) / 500.0
}

use hf::cw::receiver::FilterWidth;
use hf::front::Span;

/// Each span, at the rate it asks for and with the radio 20 kHz above the dial, decodes a carrier near each edge.
#[test]
fn cw_each_span_decodes_both_edges() {
    for span in [Span::Khz14, Span::Khz48, Span::Khz96, Span::Khz192] {
        let v = values(span, FilterWidth::Hz100, hf::cw::Listen::Off);
        let edge = 0.97 * span.width_hz() / 2.0;
        let specs = [
            hf::cw::CwSignalSpec::new(-edge, 22.0, "CQ DE K1ABC K1ABC K "),
            hf::cw::CwSignalSpec::new(edge, 26.0, "TEST DE W2XYZ W2XYZ K "),
        ];
        let (rows, _, rate) = session(&v, &specs, n0_for(20.0), 26.0, 1);
        for (s, want) in specs.iter().zip(["DE K1ABC K1ABC", "DE W2XYZ W2XYZ"]) {
            assert!(
                heard_in(&rows, s.offset_hz, want, s.wpm),
                "{} kHz span at {rate} Hz, {:+.0} Hz:\n{}",
                span.width_hz() / 1e3,
                s.offset_hz,
                show(&rows)
            );
        }
        assert_eq!(rows.len(), 2, "{}", show(&rows));
        // The carriers are at full scale while the key is down.
        for r in &rows {
            assert!(
                cell_num(r, "level_dbfs").abs() < 2.0,
                "{}",
                cell_num(r, "level_dbfs")
            );
        }
    }
}

/// The default test signal at the rate that the stack asks for (2 Msps) with the radio 20 kHz above the dial gives its
/// three rows and no more.
#[test]
fn cw_default_signal_at_the_session_rate() {
    let v = values(Span::Khz14, FilterWidth::Hz100, hf::cw::Listen::Off);
    let specs = hf::cw::demo_signals();
    let (rows, _, rate) = session(&v, &specs, n0_for(20.0), 40.0, 1);
    assert_eq!(rate, 2e6);
    for s in &specs {
        assert!(
            rows.iter()
                .any(|r| (cell_num(r, OFFSET) - s.offset_hz).abs() < 30.0),
            "{:+.0}:\n{}",
            s.offset_hz,
            show(&rows)
        );
    }
    assert_eq!(rows.len(), 3, "{}", show(&rows));
}

/// A focus outside ±7 kHz given with the settings is kept until the span is known: it picks the carrier there.
#[test]
fn cw_focus_beyond_7_khz_from_the_settings() {
    let mut v = values(Span::Khz192, FilterWidth::Hz100, hf::cw::Listen::Off);
    let schema = (hf::CW.settings)();
    v.set(
        schema.id("focus_hz").unwrap(),
        &sdrkit_core::Value::Hz(50e3),
    )
    .unwrap();
    let specs = [hf::cw::CwSignalSpec::new(
        50e3,
        22.0,
        "CQ DE K1ABC K1ABC K ",
    )];
    let (rows, _, _) = session(&v, &specs, n0_for(20.0), 6.0, 1);
    let r = rows
        .iter()
        .find(|r| (cell_num(r, OFFSET) - 50e3).abs() < 30.0)
        .unwrap_or_else(|| panic!("{}", show(&rows)));
    assert_eq!(cell_flag(r, "manual"), Some(true));
    assert_eq!(rows.len(), 1, "{}", show(&rows));
}

/// The span changed on a running stack: the stack reads the new band at once.
#[test]
fn cw_span_changes_while_running() {
    use hf::decoder::Modulator;
    let v = values(Span::Khz14, FilterWidth::Hz100, hf::cw::Listen::Off);
    let rate = 128e3;
    let specs: Vec<_> = [
        (-23280.0, "CQ DE K1ABC K1ABC K "),
        (23280.0, "TEST DE W2XYZ W2XYZ K "),
    ]
    .into_iter()
    .map(|(f, t)| hf::cw::CwSignalSpec::new(f - TUNE_OFFSET, 24.0, t))
    .collect();
    let mut x = vec![Complex32::default(); (24.0 * rate) as usize];
    hf::cw::CwModulator::new(rate, &specs, 1).fill(&mut x);
    sdrkit_algo_sim::noise::Awgn::with_power(n0_for(20.0) * rate, 2)
        .unwrap()
        .process(&mut x);
    let dial = 14.07e6;
    let mut rx = (hf::CW.make_receiver)(&sdrkit_core::StackConfig::new(v)).unwrap();
    rx.configure(&InputInfo::new(rate, dial + TUNE_OFFSET, dial).recording())
        .unwrap();
    let id = rx.schema().id("span").unwrap();
    let mut out = Output::default();
    for (k, c) in x.chunks(12800).enumerate() {
        if k == 10 {
            assert!(rows_of(rx.as_ref(), "signals").is_empty());
            rx.apply(id, &sdrkit_core::Value::Choice(Span::Khz48.index()))
                .unwrap();
        }
        rx.feed(c, &mut out);
        out.clear();
    }
    let res = sdrkit_core::Owner::results(rx.as_ref());
    assert_eq!(field_num(res, "span_hz"), 48e3);
    let rows = rows_of(rx.as_ref(), "signals");
    for (f, call) in [(-23280.0, "K1ABC"), (23280.0, "W2XYZ")] {
        assert!(heard_in(&rows, f, call, 24.0), "{f}:\n{}", show(&rows));
    }
    assert_eq!(rows.len(), 2, "{}", show(&rows));
}

/// 100 carriers over a span of 192 kHz, from 22 to 32 WPM and over 10 dB of level, each at 15 dB or more in 500 Hz: every one
/// is decoded at its offset and speed.
#[test]
fn cw_hundred_carriers_at_192_khz() {
    let v = values(Span::Khz192, FilterWidth::Hz100, hf::cw::Listen::Off);
    let specs = hf::cw::spread_signals(100, 96e3, 22.0, 32.0, 10.0);
    let weakest = specs.iter().map(|s| s.amp).fold(1.0, f64::min);
    // The weakest carrier at 15 dB in 500 Hz.
    let (rows, _, _) = session(&v, &specs, n0_for(15.0) * weakest * weakest, 26.0, 3);
    let missed: Vec<_> = specs
        .iter()
        .enumerate()
        .filter(|(i, s)| !heard_in(&rows, s.offset_hz, &hf::cw::spread_call(*i), s.wpm))
        .map(|(i, s)| {
            let near: Vec<_> = rows
                .iter()
                .filter(|r| (cell_num(r, OFFSET) - s.offset_hz).abs() < 30.0)
                .cloned()
                .collect();
            format!(
                "{i} {} {:.1} WPM amp {:.2}: {}",
                hf::cw::spread_call(i),
                s.wpm,
                s.amp,
                show(&near)
            )
        })
        .collect();
    let stray: Vec<_> = rows
        .iter()
        .filter(|r| {
            !specs
                .iter()
                .any(|s| (cell_num(r, OFFSET) - s.offset_hz).abs() < 30.0)
        })
        .cloned()
        .collect();
    assert!(
        missed.is_empty() && rows.len() == 100,
        "missed {missed:?}; {} rows, not at a carrier:\n{}",
        rows.len(),
        show(&stray)
    );
}

/// The test signal of 100 carriers over the span of the setting is decoded too.
#[test]
fn cw_test_signal_of_many_carriers() {
    let h = harness(&hf::CW)
        .rate(128e3)
        .piece(12800)
        .seconds(20.0)
        .set(&hf::front::SPAN, Span::Khz96)
        .set(&hf::cw::GEN_SIGNALS, 40)
        .set(&hf::cw::GEN_WPM, 18.0)
        .set(&hf::cw::GEN_WPM_MAX, 30.0)
        .set(&hf::cw::GEN_LEVEL_SPREAD_DB, 6.0);
    let r = run(&snr_500(h, 40.0));
    let sigs = rows(&r, "signals");
    let want = hf::cw::spread_signals(40, 48e3, 18.0, 30.0, 6.0);
    for (i, s) in want.iter().enumerate() {
        assert!(
            heard_in(&sigs, s.offset_hz, &hf::cw::spread_call(i), s.wpm),
            "{i}:\n{}",
            show(&sigs)
        );
    }
    assert_eq!(num(&r, "signals"), 40.0);
    assert_eq!(num(&r, "span_hz"), 96e3);
}

/// One carrier at 400 Hz sending `text` at `wpm` through filter `width`, at `snr_db` in 500 Hz, at 24 kHz. Returns the rows.
fn one_carrier(width: FilterWidth, wpm: f64, snr_db: f64, seed: u64) -> Vec<sdrkit_core::Record> {
    let v = values(Span::Khz14, width, hf::cw::Listen::Off);
    let spec = hf::cw::CwSignalSpec::new(400.0, wpm, "CQ DE K0TST K0TST K ");
    // Two passes of the text, which is about 230 dots, with the pause after it.
    let seconds = 3.0 + 2.0 * (230.0 * 1.2 / wpm + 2.0);
    let rate = 24e3;
    let mut x = vec![Complex32::default(); (seconds * rate) as usize];
    hf::decoder::Modulator::fill(&mut hf::cw::CwModulator::new(rate, &[spec], seed), &mut x);
    sdrkit_algo_sim::noise::Awgn::with_power(n0_for(snr_db) * rate, seed + 1)
        .unwrap()
        .process(&mut x);
    let (rx, _) = feed_at(&v, &x, rate, 0.0);
    rows_of(rx.as_ref(), "signals")
}

/// Each filter width decodes at 12 WPM and at its stated speed limit (50 Hz to 20 WPM, 100 Hz to 40 WPM, 250 and 500 Hz to
/// 60 WPM), with the word spaces where they were sent.
#[test]
fn cw_filter_widths_at_their_speed_limits() {
    for (width, top) in [
        (FilterWidth::Hz50, 20.0),
        (FilterWidth::Hz100, 40.0),
        (FilterWidth::Hz250, 60.0),
        (FilterWidth::Hz500, 60.0),
    ] {
        for (wpm, seed) in [(12.0, 1), (top, 1), (top, 2), (top, 3)] {
            let rows = one_carrier(width, wpm, 20.0, seed);
            assert!(
                found_in(&rows, 400.0, "DE K0TST K0TST K", wpm),
                "{width:?} at {wpm} WPM, seed {seed}:\n{}",
                show(&rows)
            );
        }
    }
}

/// The lowest SNR in 500 Hz, in whole dB from `top` down, at which filter `width` decodes `wpm` with three seeds out of three.
fn threshold_db(width: FilterWidth, wpm: f64, top: i32) -> i32 {
    let mut last = top + 1;
    for snr in (-12..=top).rev() {
        let ok = (1..=3).all(|seed| {
            heard_in(
                &one_carrier(width, wpm, snr as f64, seed),
                400.0,
                "DE K0TST K0TST",
                wpm,
            )
        });
        if !ok {
            break;
        }
        last = snr;
    }
    last
}

/// The sensitivity of the filter widths: 50 Hz decodes 12 WPM at a lower SNR than 100 Hz. Prints the thresholds at 15 WPM.
#[test]
fn cw_narrow_filter_is_more_sensitive() {
    let at12: Vec<i32> = [FilterWidth::Hz50, FilterWidth::Hz100]
        .into_iter()
        .map(|w| threshold_db(w, 12.0, 20))
        .collect();
    let at15: Vec<i32> = [FilterWidth::Hz50, FilterWidth::Hz100, FilterWidth::Hz500]
        .into_iter()
        .map(|w| threshold_db(w, 15.0, 20))
        .collect();
    println!(
        "cw sensitivity (dB in 500 Hz, 3 seeds of 3): 12 WPM 50 Hz {} 100 Hz {}; 15 WPM 50 Hz {} 100 Hz {} 500 Hz {}",
        at12[0], at12[1], at15[0], at15[1], at15[2]
    );
    assert!(at12[0] < at12[1], "{at12:?}");
    assert!(at15[0] <= at15[1] && at15[1] < at15[2], "{at15:?}");
}

/// The power of `x` at `f` Hz (8 kHz audio), by a Goertzel sum over the whole slice.
fn tone_power(x: &[f32], f: f64) -> f64 {
    let w = std::f64::consts::TAU * f / 8000.0;
    let (mut re, mut im) = (0.0, 0.0);
    for (n, &v) in x.iter().enumerate() {
        re += v as f64 * (w * n as f64).cos();
        im -= v as f64 * (w * n as f64).sin();
    }
    (re * re + im * im) / (x.len() as f64).powi(2)
}

/// The least RMS of the 20 ms pieces of `x`.
fn quietest(x: &[f32]) -> f64 {
    x.chunks(160)
        .map(|c| (c.iter().map(|v| (v * v) as f64).sum::<f64>() / c.len() as f64).sqrt())
        .fold(f64::MAX, f64::min)
}

/// The three listen choices: silence, a clean tone at the pitch that is silent between elements, and the real signal at the
/// pitch with its noise between elements.
#[test]
fn cw_listen_choices_give_audio_at_the_pitch() {
    let spec = hf::cw::CwSignalSpec::new(1200.0, 18.0, "CQ CQ DE K0TST K0TST K ");
    let audio = |listen| {
        let mut v = values(Span::Khz14, FilterWidth::Hz100, listen);
        let schema = (hf::CW.settings)();
        v.set(
            schema.id("focus_hz").unwrap(),
            &sdrkit_core::Value::Hz(1200.0),
        )
        .unwrap();
        v.set(schema.id("pitch").unwrap(), &sdrkit_core::Value::Hz(650.0))
            .unwrap();
        let (_, audio, rate) = session(&v, std::slice::from_ref(&spec), n0_for(20.0), 12.0, 5);
        // The audio keeps the pace of the input, less the delay of the bank.
        let want = 12.0 * 8000.0;
        assert!(
            (audio.len() as f64) <= want && audio.len() as f64 > want - 0.25 * 8000.0,
            "{listen:?} at {rate}: {} samples",
            audio.len()
        );
        audio
    };
    assert!(audio(hf::cw::Listen::Off).iter().all(|&v| v == 0.0));
    for listen in [hf::cw::Listen::Tone, hf::cw::Listen::Signal] {
        let a = audio(listen);
        let tail = &a[4 * 8000..];
        let at = tone_power(tail, 650.0);
        for other in [450.0, 850.0, 1200.0] {
            assert!(
                at > 100.0 * tone_power(tail, other),
                "{listen:?}: {other} Hz"
            );
        }
        let rms = (tail.iter().map(|v| (v * v) as f64).sum::<f64>() / tail.len() as f64).sqrt();
        assert!(rms > 0.05, "{listen:?}: {rms}");
        let quiet = quietest(tail);
        match listen {
            hf::cw::Listen::Tone => assert!(quiet < 1e-4, "tone between elements: {quiet}"),
            _ => assert!(quiet > 3e-3, "signal between elements: {quiet}"),
        }
    }
    // Without a focus the audio follows the strongest signal in the table.
    let v = values(Span::Khz14, FilterWidth::Hz100, hf::cw::Listen::Tone);
    let (_, a, _) = session(&v, std::slice::from_ref(&spec), n0_for(20.0), 12.0, 5);
    let tail = &a[6 * 8000..];
    assert!(tone_power(tail, 700.0) > 100.0 * tone_power(tail, 900.0));
}

/// The power density of `x` (8 kHz audio) at `f` Hz: the mean of the Goertzel powers of its 0.5 s pieces.
fn density(x: &[f32], f: f64) -> f64 {
    let pieces: Vec<&[f32]> = x.as_chunks::<4000>().0.iter().map(|c| &c[..]).collect();
    pieces.iter().map(|p| tone_power(p, f)).sum::<f64>() / pieces.len() as f64
}

/// What the stack says it plays after each 0.1 s block: the choice `playing` (its label), `listened_hz` and `listened`.
struct Played {
    playing: Vec<String>,
    hz: Vec<f64>,
    listened: Vec<f64>,
}

/// Feeds `x` at `rate` to a new CW receiver with values `v`, as [`feed_at`] does with the radio 20 kHz above the dial, and
/// reads what it plays after each block. Returns that and the audio.
fn feed_watch(v: &sdrkit_core::Values, x: &[Complex32], rate: f64) -> (Played, Vec<f32>) {
    let dial = 14.07e6;
    let mut rx = (hf::CW.make_receiver)(&sdrkit_core::StackConfig::new(v.clone())).unwrap();
    rx.configure(&InputInfo::new(rate, dial + TUNE_OFFSET, dial).recording())
        .unwrap();
    let mut out = Output::default();
    let mut audio = Vec::new();
    let mut p = Played {
        playing: Vec::new(),
        hz: Vec::new(),
        listened: Vec::new(),
    };
    for c in x.chunks((rate / 10.0) as usize) {
        rx.feed(c, &mut out);
        audio.extend_from_slice(out.audio());
        out.clear();
        let res = sdrkit_core::Owner::results(rx.as_ref());
        p.playing.push(field_text(res, "playing"));
        p.hz.push(field_num(res, "listened_hz"));
        p.listened.push(field_num(res, "listened"));
    }
    (p, audio)
}

/// Noise of `n0` per Hz and a steady carrier of amplitude `a` at `carrier` Hz from the dial, for `seconds`, at the rate of
/// the CW stack with values `v`, the radio 20 kHz above the dial. Returns the samples and the rate.
fn dial_band(
    v: &sdrkit_core::Values,
    n0: f64,
    carrier: f64,
    a: f32,
    seconds: f64,
) -> (Vec<Complex32>, f64) {
    let rate = hf::CW.rate_hz(v);
    let w = std::f64::consts::TAU * (carrier - TUNE_OFFSET) / rate;
    let mut x: Vec<Complex32> = (0..(seconds * rate) as usize)
        .map(|n| Complex32::from_polar(a, (w * n as f64).rem_euclid(std::f64::consts::TAU) as f32))
        .collect();
    sdrkit_algo_sim::noise::Awgn::with_power(n0 * rate, 9)
        .unwrap()
        .process(&mut x);
    (x, rate)
}

/// With no signal in the table, the real signal plays the dial through the filter: a steady carrier at the dial is a beat
/// note at the pitch, the noise around it is as wide as the filter, and the results say that the dial plays. The rebuilt
/// tone plays nothing.
#[test]
fn cw_signal_plays_the_dial_without_a_signal() {
    for width in [FilterWidth::Hz100, FilterWidth::Hz500] {
        let mut v = values(Span::Khz14, width, hf::cw::Listen::Signal);
        let schema = (hf::CW.settings)();
        v.set(schema.id("pitch").unwrap(), &sdrkit_core::Value::Hz(650.0))
            .unwrap();
        // A carrier 10 dB over the noise in 500 Hz, unkeyed, at the dial.
        let (x, rate) = dial_band(&v, n0_for(10.0), 0.0, 1.0, 16.0);
        let (p, audio) = feed_watch(&v, &x, rate);
        for (i, s) in p.playing.iter().enumerate().skip(5) {
            assert_eq!(s, "Dial, no signal", "{width:?} block {i}");
            assert!((p.hz[i] - 14.07e6).abs() < 1e-3, "{width:?}: {}", p.hz[i]);
            assert!(p.listened[i].is_nan(), "{width:?}: {}", p.listened[i]);
        }
        let tail = &audio[2 * 8000..];
        let rms = (tail.iter().map(|v| (v * v) as f64).sum::<f64>() / tail.len() as f64).sqrt();
        assert!(rms > 0.02 && rms < 0.3, "{width:?}: rms {rms}");
        let at = |f: f64| 10.0 * density(tail, f).log10();
        let (pitch, inside, outside) = (
            at(650.0),
            at(650.0 + 0.3 * width.width_hz()),
            at(650.0 + 1.5 * width.width_hz()),
        );
        let below = at(650.0 - 0.3 * width.width_hz());
        assert!(
            pitch > inside + 20.0,
            "{width:?}: beat {pitch:.1} dB, noise {inside:.1} dB"
        );
        assert!(
            (inside - below).abs() < 3.0,
            "{width:?}: {inside:.1} and {below:.1} dB"
        );
        assert!(
            inside > outside + 15.0,
            "{width:?}: in {inside:.1} dB, out {outside:.1} dB"
        );
    }
    let v = values(Span::Khz14, FilterWidth::Hz100, hf::cw::Listen::Tone);
    let (x, rate) = dial_band(&v, n0_for(10.0), 0.0, 1.0, 6.0);
    let (p, audio) = feed_watch(&v, &x, rate);
    assert!(audio.iter().all(|&v| v == 0.0));
    assert!(p.playing.iter().all(|s| s == "Nothing"), "{:?}", p.playing);
    assert!(p.hz.iter().all(|h| h.is_nan()));
}

/// The real signal plays the dial, moves to a station once its row appears, and back to the dial once the station is gone,
/// with no step louder than the audio around it at either change.
#[test]
fn cw_signal_moves_between_the_dial_and_a_station() {
    use hf::decoder::Modulator;
    let v = values(Span::Khz14, FilterWidth::Hz100, hf::cw::Listen::Signal);
    let (mut x, rate) = dial_band(&v, n0_for(20.0), 0.0, 0.0, 50.0);
    // A station 1500 Hz above the dial for the first 14 s, then 36 s of noise alone.
    let spec = hf::cw::CwSignalSpec::new(1500.0 - TUNE_OFFSET, 20.0, "CQ CQ DE K0TST K0TST K ");
    let mut s = vec![Complex32::default(); (14.0 * rate) as usize];
    hf::cw::CwModulator::new(rate, &[spec], 3).fill(&mut s);
    for (a, b) in x.iter_mut().zip(&s) {
        *a += b;
    }
    let (p, audio) = feed_watch(&v, &x, rate);
    let station = 14.07e6 + 1500.0;
    let on = p
        .playing
        .iter()
        .position(|s| s == "Signal")
        .unwrap_or_else(|| panic!("never the station: {:?}", p.playing));
    let off = on
        + p.playing[on..]
            .iter()
            .position(|s| s != "Signal")
            .unwrap_or_else(|| panic!("never back to the dial"));
    assert!(p.playing[..on]
        .iter()
        .skip(2)
        .all(|s| s == "Dial, no signal"));
    assert!(p.playing[off..].iter().all(|s| s == "Dial, no signal"));
    assert!(
        p.hz[on..off].iter().all(|h| (h - station).abs() < 30.0),
        "{:?}",
        &p.hz[on..off]
    );
    assert!(p.listened[on..off].iter().all(|l| !l.is_nan()));
    assert!(p.hz[off..].iter().all(|h| (h - 14.07e6).abs() < 1e-3));
    // The station's row came while it sent, and left about 30 s after it stopped.
    assert!(on < 140, "station at block {on}");
    assert!((420..=480).contains(&off), "dial again at block {off}");
    // The audio of the station is at the pitch.
    let a = |block: usize| block * 800;
    let station_audio = &audio[a(on + 10)..a(140)];
    assert!(tone_power(station_audio, 700.0) > 30.0 * tone_power(station_audio, 900.0));
    // No step at a change is louder than the audio of the 2 s before it and the 2 s after the fade.
    let peak = |x: &[f32]| x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    for change in [on, off] {
        // The audio of a block comes out about 0.2 s late.
        let around = peak(&audio[a(change - 23)..a(change - 3)])
            .max(peak(&audio[a(change + 5)..a(change + 25)]));
        let at = peak(&audio[a(change - 3)..a(change + 5)]);
        assert!(at <= 1.2 * around, "block {change}: {at} against {around}");
    }
}

/// The processor share of the skimmer: `carriers` keyed carriers over 0.95 of the span at the rate that the span asks for (or
/// `rate` when given), fed in 0.1 s pieces. Returns the share of one core and the signals followed at the end.
fn share(span: Span, rate: Option<f64>, carriers: usize) -> (f64, usize) {
    use hf::decoder::Modulator;
    let v = values(span, FilterWidth::Hz100, hf::cw::Listen::Off);
    let rate = rate.unwrap_or_else(|| hf::CW.rate_hz(&v));
    let offset = if rate == 24e3 { 0.0 } else { TUNE_OFFSET };
    let specs: Vec<_> = hf::cw::spread_signals(carriers, span.width_hz() / 2.0, 18.0, 30.0, 0.0)
        .into_iter()
        .map(|mut s| {
            s.offset_hz -= offset;
            s
        })
        .collect();
    let seconds = 10.0;
    let mut x = vec![Complex32::default(); (seconds * rate) as usize];
    hf::cw::CwModulator::new(rate, &specs, 1).fill(&mut x);
    // Each carrier at 25 dB in 500 Hz: the carriers are at 1 / carriers of full scale.
    let a = 1.0 / carriers as f64;
    sdrkit_algo_sim::noise::Awgn::with_power(n0_for(25.0) * a * a * rate, 3)
        .unwrap()
        .process(&mut x);
    let dial = 14.07e6;
    let mut rx = (hf::CW.make_receiver)(&sdrkit_core::StackConfig::new(v)).unwrap();
    rx.configure(&InputInfo::new(rate, dial + offset, dial).recording())
        .unwrap();
    let mut out = Output::default();
    let t = std::time::Instant::now();
    for c in x.chunks((rate / 10.0) as usize) {
        rx.feed(c, &mut out);
        out.clear();
    }
    let share = t.elapsed().as_secs_f64() / seconds;
    let signals = rows_of(rx.as_ref(), "signals")
        .iter()
        .filter(|s| cell_flag(s, "gone") == Some(false))
        .count();
    (share, signals)
}

/// The cost of the skimmer in the build of the test (not a release figure). Run alone:
/// `cargo test -p sdrkit-stack-hfkbd --test sdrkit_stack_hfkbd cw_cost -- --ignored --nocapture`.
#[test]
#[ignore = "timing: run alone, it prints the processor share"]
fn cw_cost() {
    for (span, rate, carriers) in [
        (Span::Khz14, Some(24e3), 3),
        (Span::Khz14, Some(24e3), 24),
        (Span::Khz14, None, 3),
        (Span::Khz48, None, 3),
        (Span::Khz96, None, 3),
        (Span::Khz192, None, 3),
        (Span::Khz192, None, 100),
    ] {
        let (s, n) = share(span, rate, carriers);
        println!(
            "cw cost: span {:.0} kHz, input {:.0} kHz, {carriers} carriers: {:.2} % of a core, {n} followed",
            span.width_hz() / 1e3,
            rate.unwrap_or_else(|| hf::CW.rate_hz(&values(span, FilterWidth::Hz100, hf::cw::Listen::Off))) / 1e3,
            s * 100.0
        );
    }
}

#[test]
fn cw_timing_decoder_has_no_recursion() {
    // Key lengths of every size on a thread with a 256 kB stack.
    let h = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let cfg = sdrkit_util_text::morse::DecoderConfig::following(20.0);
            let mut d = sdrkit_util_text::morse::Decoder::new(cfg).unwrap();
            let mut r = sdrkit_algo_sim::rng::Rng::new(1);
            for _ in 0..20000 {
                let ms = r.below(2000) as f64 + 1.0;
                d.push(r.bit() == 1, ms / 1000.0, &mut |_| {});
            }
        })
        .unwrap();
    h.join().unwrap();
}

/// The test signal of five carriers spread over `span` as the test-signal radio plays it with the stack's settings: at the
/// rate of the span (48 kHz for 14 kHz), the dial 20 kHz below the centre, through filter `width`, rounded to 8 bits like
/// the converter of the radio. `noise` is the SNR of all carriers together in 500 Hz (none: no noise but that of the
/// rounding). With `noise_only` the carriers are not sent and the noise stays. Returns the rows after `seconds`.
fn played(
    span: Span,
    width: FilterWidth,
    noise: Option<f64>,
    noise_only: bool,
    seconds: f64,
    seed: u64,
) -> Vec<sdrkit_core::Record> {
    let v = values(span, width, hf::cw::Listen::Off);
    let rate = match span {
        Span::Khz14 => 48e3,
        _ => hf::CW.rate_hz(&v),
    };
    let mut h = harness(&hf::CW)
        .rate(rate)
        .seconds(seconds)
        .seed(seed)
        .set(&hf::front::SPAN, span)
        .set(&hf::cw::FILTER, width)
        .set(&hf::cw::GEN_SIGNALS, 5)
        .set(&hf::cw::GEN_NOISE_ONLY, noise_only)
        .set(&GEN_OFFSET_HZ, -TUNE_OFFSET);
    if let Some(snr) = noise {
        h = snr_500(h, snr);
    }
    let mut x = h.generate().unwrap();
    // The 8-bit converter of the test-signal radio: 128 steps to a full scale of 4.
    let step = 4.0f32 / 128.0;
    let q = |v: f32| (v / step).clamp(-128.0, 127.0).round() * step;
    for c in &mut x {
        *c = Complex32::new(q(c.re), q(c.im));
    }
    let (rx, _) = feed_at(&v, &x, rate, TUNE_OFFSET);
    rows_of(rx.as_ref(), "signals")
}

/// Whether every word of the text of row `r` but the first and the last (which can be cut) is a word that a carrier of
/// `sent` sends, or several of them run together: no stray space splits a word. (The timing decoder can miss the word space
/// after the first word when a pause came before it; that is not checked here.)
fn words_of_the_text(r: &sdrkit_core::Record, sent: &[hf::cw::CwSignalSpec]) -> bool {
    let text = cell_text(r, TEXT);
    let words: Vec<&str> = text.split_whitespace().collect();
    let known: Vec<&str> = sent
        .iter()
        .flat_map(|s| s.text.split_whitespace())
        .collect();
    // Whether `w` is made of known words, one after another.
    fn made_of(w: &str, known: &[&str]) -> bool {
        w.is_empty()
            || known
                .iter()
                .any(|k| w.strip_prefix(k).is_some_and(|rest| made_of(rest, known)))
    }
    let n = words.len();
    words
        .iter()
        .take(n.saturating_sub(1))
        .skip(1)
        .all(|w| made_of(w, &known))
}

/// Over 120 s at `span`, with every filter width: five carriers give five rows, each with its call sign, without noise (the
/// mixing products of the rounding are there; then no word is split) and with noise; noise alone gives no row. Each filter
/// width runs on its own thread.
fn rows_equal_the_carriers(span: Span) {
    let widths = [
        FilterWidth::Hz50,
        FilterWidth::Hz100,
        FilterWidth::Hz250,
        FilterWidth::Hz500,
    ];
    // Each carrier at 20 dB in 500 Hz: all five together 7 dB more.
    let cases = [
        (None, false, 1),
        (Some(27.0), false, 2),
        (Some(27.0), false, 3),
        (Some(27.0), true, 1),
        (Some(27.0), true, 2),
        (Some(20.0), true, 3),
    ];
    let failures: Vec<String> = std::thread::scope(|s| {
        let runs: Vec<_> = widths
            .iter()
            .map(|&w| {
                s.spawn(move || {
                    let mut bad = Vec::new();
                    for (noise, only, seed) in cases {
                        let rows = played(span, w, noise, only, 120.0, seed);
                        let want = hf::cw::spread_signals(5, span.width_hz() / 2.0, 20.0, 0.0, 0.0);
                        let ok = if only {
                            rows.is_empty()
                        } else {
                            rows.len() == 5
                                && want.iter().enumerate().all(|(i, c)| {
                                    heard_in(&rows, c.offset_hz, &hf::cw::spread_call(i), c.wpm)
                                })
                                && (noise.is_some() || rows.iter().all(|r| words_of_the_text(r, &want)))
                        };
                        if !ok {
                            bad.push(format!(
                                "{w:?}, noise {noise:?}, noise only {only}, seed {seed}: {} rows\n{}",
                                rows.len(),
                                show(&rows)
                            ));
                        }
                    }
                    bad
                })
            })
            .collect();
        runs.into_iter().flat_map(|r| r.join().unwrap()).collect()
    });
    assert!(
        failures.is_empty(),
        "{:.0} kHz span:\n{}",
        span.width_hz() / 1e3,
        failures.join("\n")
    );
}

#[test]
fn cw_rows_equal_the_carriers_at_14_khz() {
    rows_equal_the_carriers(Span::Khz14);
}

#[test]
fn cw_rows_equal_the_carriers_at_48_khz() {
    rows_equal_the_carriers(Span::Khz48);
}

#[test]
fn cw_rows_equal_the_carriers_at_96_khz() {
    rows_equal_the_carriers(Span::Khz96);
}

#[test]
fn cw_rows_equal_the_carriers_at_192_khz() {
    rows_equal_the_carriers(Span::Khz192);
}

/// A steady carrier in the span, like the centre of a direct-conversion radio, beside two keyed carriers: it is followed for
/// at most 5 s once, is not started again while it stays, and leaves no row; the keyed carriers give their rows.
#[test]
fn cw_steady_carrier_is_dropped_and_not_tried_again() {
    use hf::decoder::Modulator;
    use sdrkit_graph::{Decoder, Format, Outbox};
    let rate = 64e3;
    let specs = [
        hf::cw::CwSignalSpec::new(-10e3, 20.0, "CQ DE K1ABC K1ABC K "),
        hf::cw::CwSignalSpec::new(5e3, 24.0, "TEST DE W2XYZ W2XYZ K "),
    ];
    let mut x = vec![Complex32::default(); (60.0 * rate) as usize];
    hf::cw::CwModulator::new(rate, &specs, 1).fill(&mut x);
    let spike = sdrkit_algo_dsp::nco::Nco::new(rate, 20e3);
    let mut spike = spike;
    for v in &mut x {
        *v += spike.next_sample() * 0.3;
    }
    sdrkit_algo_sim::noise::Awgn::with_power(n0_for(20.0) * rate, 2)
        .unwrap()
        .process(&mut x);
    let mut dec = hf::cw::CwDecoder::new();
    let input = Format {
        bandwidth_hz: 48e3,
        ..Format::iq(rate, 14.07e6, 14.07e6)
    };
    let kinds: Vec<_> = dec
        .configure(&input)
        .unwrap()
        .into_iter()
        .map(|f| f.kind)
        .collect();
    let mut out = Outbox::new(&kinds);
    let (mut followed, mut starts, mut was) = (0.0, 0, false);
    for c in x.chunks((rate / 10.0) as usize) {
        dec.process(c, 0, &mut out);
        let now = dec
            .channels()
            .iter()
            .any(|c| (c.freq_hz() - 20e3).abs() < 30.0);
        followed += 0.1 * f64::from(u8::from(now));
        starts += usize::from(now && !was);
        was = now;
    }
    assert!(
        starts <= 1 && followed <= 7.0,
        "{starts} starts, {followed:.1} s followed"
    );
    let mut res = dec.results();
    dec.report(&mut res);
    let res = res.results();
    let id = res.decl().table_id("signals").unwrap();
    let rows: Vec<_> = res
        .table(id)
        .unwrap()
        .rows_newest_first()
        .map(|(_, r)| r.clone())
        .collect();
    for (s, call) in specs.iter().zip(["K1ABC", "W2XYZ"]) {
        assert!(heard_in(&rows, s.offset_hz, call, s.wpm), "{}", show(&rows));
    }
    assert_eq!(rows.len(), 2, "{}", show(&rows));
}

/// The level and noise settings of the test-signal radio act on the CW test signal: `gen_level_dbfs` is the power of all
/// five carriers with their keys down, so each is 7 dB below it; `gen_snr_db` with `gen_noise` at SNR sets the noise.
#[test]
fn cw_test_signal_takes_the_level_and_the_noise() {
    let h = |snr: f64| {
        harness(&hf::CW)
            .rate(24e3)
            .piece(2400)
            .seconds(30.0)
            .set(&hf::cw::GEN_SIGNALS, 5)
            .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -20.0)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, snr)
    };
    let r = run(&h(27.0));
    let sigs = rows(&r, "signals");
    assert_eq!(sigs.len(), 5, "{}", show(&sigs));
    let each = -20.0 - 10.0 * 5f64.log10();
    for s in &sigs {
        let l = cell_num(s, "level_dbfs");
        // The level of a row reads up to 2 dB low (the edges of the marks), as in cw_each_span_decodes_both_edges.
        assert!((l - each).abs() < 2.0, "{l} for {each}:\n{}", show(&sigs));
    }
    // At -10 dB for the five together (-17 dB each in 500 Hz) no carrier is decoded.
    assert!(rows(&run(&h(-10.0)), "signals").is_empty());
}

/// `seconds` of the carriers `specs` with white noise at 20 dB in 500 Hz for a carrier of amplitude 1, at 24 kHz.
fn carriers(specs: &[hf::cw::CwSignalSpec], seconds: f64, seed: u64) -> Vec<Complex32> {
    let rate = 24e3;
    let mut x = vec![Complex32::default(); (seconds * rate) as usize];
    hf::decoder::Modulator::fill(&mut hf::cw::CwModulator::new(rate, specs, seed), &mut x);
    sdrkit_algo_sim::noise::Awgn::with_power(n0_for(20.0) * rate, seed + 1)
        .unwrap()
        .process(&mut x);
    x
}

/// Feeds `x` to `rx` in 0.1 s pieces and returns the time after which a row that is not gone is at `f` Hz with `call`
/// in its text, s (infinite if none).
fn first_row(rx: &mut dyn sdrkit_core::Receiver, x: &[Complex32], f: f64, call: &str) -> f64 {
    let mut out = Output::default();
    for (k, c) in x.chunks(2400).enumerate() {
        rx.feed(c, &mut out);
        out.clear();
        let found = rows_of(rx, "signals").iter().any(|r| {
            cell_flag(r, "gone") == Some(false)
                && (cell_num(r, OFFSET) - f).abs() < 30.0
                && cell_text(r, TEXT).contains(call)
        });
        if found {
            return 0.1 * (k + 1) as f64;
        }
    }
    f64::INFINITY
}

/// After four changes of frequency, each with carriers or noise of its own and with or without the call that forgets the
/// results, a carrier on the last frequency is listed as fast as in a new session.
#[test]
fn cw_lists_a_carrier_as_fast_after_four_retunes() {
    let v = values(Span::Khz14, FilterWidth::Hz100, hf::cw::Listen::Off);
    let make = || {
        let mut rx = (hf::CW.make_receiver)(&sdrkit_core::StackConfig::new(v.clone())).unwrap();
        rx.configure(&InputInfo::new(24e3, 14.0e6, 14.0e6).recording())
            .unwrap();
        rx
    };
    let wanted = carriers(
        &[hf::cw::CwSignalSpec::new(400.0, 20.0, "CQ DE K0TST K ")],
        30.0,
        7,
    );
    let fresh = first_row(make().as_mut(), &wanted, 400.0, "K0TST");
    assert!(fresh < 10.0, "a new session: {fresh} s");
    let before = [
        carriers(
            &[hf::cw::CwSignalSpec::new(400.0, 25.0, "TEST DE N0XYZ K ")],
            10.0,
            1,
        ),
        carriers(&[], 10.0, 2),
        carriers(
            &[
                hf::cw::CwSignalSpec::new(-1500.0, 18.0, "CQ DE W1AW K "),
                hf::cw::CwSignalSpec::new(2500.0, 28.0, "CQ DE G0ABC K "),
            ],
            10.0,
            3,
        ),
        carriers(&[], 4.0, 4),
    ];
    for forget in [true, false] {
        let mut rx = make();
        let mut out = Output::default();
        for (i, x) in before.iter().enumerate() {
            for c in x.chunks(2400) {
                rx.feed(c, &mut out);
                out.clear();
            }
            let f = 14.1e6 + 0.1e6 * i as f64;
            rx.retuned(&InputInfo::new(24e3, f, f).recording());
            if forget {
                rx.forget();
            }
        }
        let after = first_row(rx.as_mut(), &wanted, 400.0, "K0TST");
        // The row is a new one: the text of the first frequency stays in its own row, marked gone.
        let rows = rows_of(rx.as_ref(), "signals");
        assert!(
            rows.iter()
                .all(|r| !(cell_text(r, TEXT).contains("N0XYZ")
                    && cell_text(r, TEXT).contains("K0TST"))),
            "forget {forget}:\n{}",
            show(&rows)
        );
        assert!(
            after <= fresh + 0.5,
            "forget {forget}: {after} s after four changes, {fresh} s in a new session\n{}",
            show(&rows_of(rx.as_ref(), "signals"))
        );
    }
}
