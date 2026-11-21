//! Extra channels, the components in a described graph, the listening chain as a component, and the whistle, hum and status
//! line of the AM processor.

use crate::common::*;
use sdrkit_core::{Complex32, InputInfo, Output, Owner, Receiver, SettingSet, Value};
use sdrkit_graph::{components::FmDemod, components::RealToAudio, GraphBuilder, Registry};
use sdrkit_stack_general::amproc::{AmConfig, AmDetector, AmProcessor};
use sdrkit_stack_general::listen::ListeningComponent;
use sdrkit_stack_general::typed::{ChannelRow, CHANNELS};
use std::f64::consts::TAU;

const RATE: f64 = 2e6;

#[test]
fn extra_channels_decode_their_own_station() {
    // The main channel on an NFM station at the dial, an extra channel 100 kHz up on another (AM).
    let mut rx = receiver(
        RATE,
        -250e3,
        &[
            ("demod", "nfm"),
            ("extra1", "1"),
            ("extra1_offset", "100000"),
            ("extra1_demod", "am"),
        ],
    );
    let mut a = generator(RATE, "nfm", &[("gen_snr_db", "40")]);
    let mut b = generator(
        RATE,
        "am",
        &[("gen_offset_hz", "100000"), ("gen_tone", "700")],
    );
    let mut buf = vec![Complex32::default(); 20_000];
    let mut tmp = vec![Complex32::default(); 20_000];
    let mut out = Output::default();
    let mut main_audio = Vec::new();
    for _ in 0..100 {
        a.generate(&mut buf);
        b.generate(&mut tmp);
        for (x, y) in buf.iter_mut().zip(&tmp) {
            *x += *y;
        }
        out.clear();
        rx.feed(&buf, &mut out);
        main_audio.extend(first_channel(&out));
    }
    let t = &main_audio[main_audio.len() - 24_000..];
    assert!(tone(t, 1000.0) > 0.2, "the main channel is heard");
    let r = Owner::results(&rx);
    let table = r.table(r.decl().table_id(CHANNELS.key).unwrap()).unwrap();
    let row = table.row("1").expect("a row for extra channel 1");
    assert_eq!(
        row.at(&ChannelRow::FREQ_HZ),
        Some(145.6e6),
        "the frequency of the extra channel"
    );
    assert_eq!(
        row.at(&ChannelRow::DEMOD),
        Some(sdrkit_stack_general::Demod::Am)
    );
    assert_eq!(
        row.at(&ChannelRow::SQUELCH_OPEN),
        Some(true),
        "its squelch is open"
    );
    // Hear the extra channel instead.
    set(&mut rx, "listen_channel", "1").unwrap();
    let mut extra_audio = Vec::new();
    for _ in 0..60 {
        a.generate(&mut buf);
        b.generate(&mut tmp);
        for (x, y) in buf.iter_mut().zip(&tmp) {
            *x += *y;
        }
        out.clear();
        rx.feed(&buf, &mut out);
        extra_audio.extend(first_channel(&out));
    }
    let t = &extra_audio[extra_audio.len() - 24_000..];
    assert!(
        tone(t, 700.0) > 0.2,
        "the extra channel is heard: {}",
        tone(t, 700.0)
    );
    set(&mut rx, "extra1", "0").unwrap();
    rx.feed(&buf, &mut out);
    let r = Owner::results(&rx);
    assert!(r
        .table(r.decl().table_id(CHANNELS.key).unwrap())
        .is_none_or(|t| t.is_empty()));
}

#[test]
fn the_components_build_from_a_description() {
    let mut r = Registry::new();
    sdrkit_components::dsp::register(&mut r);
    sdrkit_stack_general::register(&mut r);
    let g = sdrkit_stack_general::receiver::build_graph().unwrap();
    let json = serde_json::to_string(&g.describe()).unwrap();
    let mut again = sdrkit_graph::Graph::from_json(&json, &r).expect("the description builds");
    again
        .configure(&InputInfo::new(RATE, 145.75e6, 145.5e6))
        .unwrap();
    let mut gen = generator(RATE, "nfm", &[("gen_snr_db", "30")]);
    let mut buf = vec![Complex32::default(); 200_000];
    let mut out = Output::default();
    let mut audio = Vec::new();
    for _ in 0..5 {
        gen.generate(&mut buf);
        out.clear();
        again.feed(&buf, &mut out);
        audio.extend(first_channel(&out));
    }
    assert!(tone(&audio[audio.len() - 24_000..], 1000.0) > 0.2);
}

#[test]
fn the_listening_chain_runs_as_a_component() {
    // input (an FM station at 0 Hz, 240 kHz) -> fm_demod -> real_to_audio -> general.listening (the chain on, a 3 kHz low-pass).
    let mut lc = ListeningComponent::default();
    use sdrkit_graph::Component;
    use sdrkit_stack_general::listen::ListenSetting;
    lc.apply(ListenSetting::ChainOn.id(), &Value::Bool(true))
        .unwrap();
    lc.apply(ListenSetting::Lowpass.id(), &Value::Bool(true))
        .unwrap();
    lc.apply(ListenSetting::LowpassHz.id(), &Value::Hz(2000.0))
        .unwrap();
    let mut g = GraphBuilder::new()
        .add("fm", FmDemod::default())
        .add("mono", RealToAudio::default())
        .add("listen", lc)
        .input("fm")
        .connect("fm", "mono")
        .connect("mono", "listen")
        .output("listen")
        .build()
        .unwrap();
    g.configure(&InputInfo::new(48e3, 0.0, 0.0)).unwrap();
    let x: Vec<Complex32> = (0..48_000)
        .map(|i| {
            let t = i as f64 / 48e3;
            let ph = -(3000.0 / 1000.0) * (TAU * 1000.0 * t).cos()
                - (3000.0 / 6000.0) * (TAU * 6000.0 * t).cos();
            Complex32::new(ph.cos() as f32, ph.sin() as f32)
        })
        .collect();
    let mut out = Output::default();
    g.feed(&x, &mut out);
    let l: Vec<f32> = first_channel(&out).collect();
    let t = &l[l.len() - 24_000..];
    let (a1, a6) = (tone(t, 1000.0), tone(t, 6000.0));
    // 12 dB/octave: about 19 dB down an octave and a half above the corner.
    assert!(a1 > 6.0 * a6, "the low-pass works in the graph: {a1} {a6}");
    assert_eq!(
        g.component_results("listen")
            .unwrap()
            .fields()
            .at(&sdrkit_stack_general::typed::ListeningFields::ACTIVE),
        Some(true)
    );
}

fn am_with(extra: impl Fn(f64) -> f64, seconds: f64) -> AmProcessor {
    let mut p = AmProcessor::new(AmConfig {
        auto: true,
        detector: AmDetector::Auto,
        ..Default::default()
    });
    let mut out = Vec::new();
    let mut rng = sdrkit_algo_sim::rng::Rng::new(7);
    let mut noise = move || (rng.uniform() - 0.5) * 0.002;
    for i in 0..(seconds * 48e3) as usize {
        let t = i as f64 / 48e3;
        let e = 0.2
            * (1.0
                + 0.4 * (TAU * 700.0 * t).sin()
                + 0.3 * (TAU * 1900.0 * t).sin()
                + 0.2 * (TAU * 3100.0 * t).sin());
        let (wr, wi) = (extra(t), 0.0);
        let x = Complex32::new((e + wr + noise()) as f32, (wi + noise()) as f32);
        p.process(&[x], &mut out);
    }
    p
}

#[test]
fn a_whistle_on_one_side_is_notched() {
    // A heterodyne 2.5 kHz above the carrier only (one side): a complex tone added to the input.
    let mut p = AmProcessor::new(AmConfig {
        auto: true,
        detector: AmDetector::Auto,
        ..Default::default()
    });
    let mut out = Vec::new();
    for i in 0..(6.0 * 48e3) as usize {
        let t = i as f64 / 48e3;
        let e = 0.2 * (1.0 + 0.4 * (TAU * 700.0 * t).sin() + 0.3 * (TAU * 1900.0 * t).sin());
        let w = 0.01 * num_complex::Complex64::from_polar(1.0, TAU * 2500.0 * t);
        p.process(&[Complex32::new((e + w.re) as f32, w.im as f32)], &mut out);
    }
    let st = p.status().clone();
    assert!(st.locked, "{}", st.line);
    assert!(
        st.whistles_hz.iter().any(|&w| (w - 2500.0).abs() < 30.0),
        "{:?} {}",
        st.whistles_hz,
        st.line
    );
    assert!(
        st.line.contains("whistle at 2.50 kHz removed"),
        "{}",
        st.line
    );
}

#[test]
fn mains_hum_is_notched() {
    let p = &mut am_with(
        |t| {
            0.2 * (0.02 * (TAU * 100.0 * t).sin()
                + 0.02 * (TAU * 150.0 * t).sin()
                + 0.02 * (TAU * 200.0 * t).sin()
                + 0.02 * (TAU * 50.0 * t).sin())
        },
        5.0,
    );
    let st = p.status().clone();
    assert_eq!(st.hum_hz, 50, "{}", st.line);
    assert!(st.line.contains("50 Hz hum removed"));
}

#[test]
fn the_status_line_tells_what_happens() {
    let p = &mut am_with(|_| 0.0, 3.0);
    let st = p.status().clone();
    assert!(st.active && st.carrier_found && st.locked);
    assert!(
        st.line.starts_with("Locked to the carrier ("),
        "{}",
        st.line
    );
    assert!(st.line.contains("; audio to "), "{}", st.line);
    assert!((st.carrier_dbfs + 14.0).abs() < 1.0, "{}", st.carrier_dbfs);
    let mut quiet = AmProcessor::new(AmConfig {
        auto: true,
        detector: AmDetector::Auto,
        ..Default::default()
    });
    let mut out = Vec::new();
    quiet.process(&vec![Complex32::default(); 4800], &mut out);
    assert_eq!(out.len(), 4800, "as many samples out as in");
    assert!(quiet.status().line.starts_with("Looking for the carrier"));
}

#[test]
fn enhance_adds_treble_above_the_edge() {
    let run = |enhance: bool| {
        let mut p = AmProcessor::new(AmConfig {
            auto: true,
            detector: AmDetector::Auto,
            enhance,
            bandwidth: 4000.0,
            ..Default::default()
        });
        let mut out = Vec::new();
        for i in 0..(4.0 * 48e3) as usize {
            let t = i as f64 / 48e3;
            let e = 0.2 * (1.0 + 0.4 * (TAU * 2900.0 * t).sin() + 0.3 * (TAU * 700.0 * t).sin());
            p.process(&[Complex32::new(e as f32, 0.0)], &mut out);
        }
        let tail = &out[out.len() - 48_000..];
        // The rectified top octave makes harmonics above the 4 kHz edge: the second of 2.9 kHz.
        (tone(tail, 5800.0), p.status().line.clone())
    };
    let (plain, _) = run(false);
    let (enh, line) = run(true);
    assert!(enh > 2.0 * plain, "{enh} against {plain}");
    assert!(line.contains("enhanced (synthetic treble)"));
}

/// The audio of the AM processor with its leveller for a fixed station whose programme falls by 20 dB and rises again, so
/// that the leveller works up, down and holds.
#[test]
fn the_am_processor_keeps_its_digest() {
    let mut p = AmProcessor::new(AmConfig {
        auto: true,
        detector: AmDetector::Auto,
        ..Default::default()
    });
    let mut rng = sdrkit_algo_sim::rng::Rng::new(7);
    let mut out = Vec::new();
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for i in 0..(8.0 * 48e3) as usize {
        let t = i as f64 / 48e3;
        let m = if (3.0..5.0).contains(&t) { 0.05 } else { 0.5 };
        let e = 0.2 * (1.0 + m * (0.6 * (TAU * 700.0 * t).sin() + 0.4 * (TAU * 1900.0 * t).sin()));
        let n = (rng.uniform() - 0.5) * 0.002;
        out.clear();
        p.process(&[Complex32::new((e + n) as f32, n as f32)], &mut out);
        for y in &out {
            for b in y.to_bits().to_le_bytes() {
                h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    eprintln!("AM processor digest {h:#018x}");
    assert_eq!(h, 0xa30d1a892d273f30);
}
