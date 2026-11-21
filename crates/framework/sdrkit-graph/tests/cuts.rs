//! Every standard component produces the same output however the stream is cut into calls.

use sdrkit_core::{Complex32, InputInfo, Output, Receiver};
use sdrkit_graph::*;

fn signal(n: usize) -> Vec<Complex32> {
    // Two tones, some noise, a click, so every component has something to do.
    let mut s = 987654321u64;
    (0..n)
        .map(|i| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let noise = ((s >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 0.02;
            let t = i as f64;
            let a = std::f64::consts::TAU * (0.013 * t + 0.4 * (0.0007 * t).sin());
            let b = std::f64::consts::TAU * -0.21 * t;
            let click = if i % 9000 == 4000 { 3.0 } else { 0.0 };
            Complex32::new(
                (0.3 * a.cos() + 0.1 * b.cos()) as f32 + noise + click,
                (0.3 * a.sin() + 0.1 * b.sin()) as f32 - noise,
            )
        })
        .collect()
}

/// Runs `ty` (fed through `pre` when it needs another input kind) and returns its output stream, as read by a tap.
fn run(ty: &str, pre: &[&str], x: &[Complex32], piece: usize) -> Vec<String> {
    let r = Registry::standard();
    let mut b = GraphBuilder::new();
    let mut prev = INPUT.to_string();
    for (k, p) in pre.iter().chain([&ty]).enumerate() {
        let id = format!("c{k}");
        b = b.add_boxed(&id, r.make(p).unwrap()).connect(&prev, &id);
        prev = id;
    }
    let comp = r.make(ty).unwrap();
    let has_out = !comp.info().outputs.is_empty();
    let mut g = b.build().unwrap();
    g.configure(&InputInfo {
        rate_hz: 250e3,
        centre_hz: 0.0,
        tuned_hz: 0.0,
        live: true,
    })
    .unwrap();
    let mut tap = if has_out {
        Some(g.tap(&prev, 100_000).unwrap())
    } else {
        None
    };
    let mut out = Output::default();
    for c in x.chunks(piece) {
        g.feed(c, &mut out);
    }
    let mut seen: Vec<String> = Vec::new();
    if let Some(t) = &mut tap {
        let mut buf = Buffer::default();
        while t.pop(&mut buf) {
            items(&buf, &mut seen);
            seen.extend(buf.tags().iter().map(|t| format!("{t:?}")));
        }
    }
    seen
}

fn items(b: &Buffer, out: &mut Vec<String>) {
    match &b.data {
        Data::Iq(v) | Data::Symbols(v) => out.extend(
            v.iter()
                .map(|x| format!("{:?}", (x.re.to_bits(), x.im.to_bits()))),
        ),
        Data::Real(v) => out.extend(v.iter().map(|x| format!("{}", x.to_bits()))),
        Data::Audio(a) => out.extend(a.samples.iter().map(|x| format!("{}", x.to_bits()))),
        Data::Spectrum(f) => out.extend(f.values.iter().map(|x| format!("{}", x.to_bits()))),
        d => out.push(format!("{d:?}")),
    }
}

#[test]
fn every_component_ignores_where_the_stream_is_cut() {
    let x = signal(40_000);
    let cases: &[(&str, &[&str])] = &[
        ("dsp.dc_removal", &[]),
        ("dsp.iq_correction", &[]),
        ("dsp.noise_blanker", &[]),
        ("dsp.shift", &[]),
        ("dsp.fir", &[]),
        ("dsp.decimator", &[]),
        ("dsp.resampler", &[]),
        ("dsp.channel", &[]),
        ("dsp.agc", &[]),
        ("dsp.spectrum", &[]),
        ("dsp.fm_demod", &[]),
        ("dsp.am_demod", &[]),
        ("dsp.ssb_demod", &["dsp.channel"]),
        ("dsp.squelch", &[]),
        ("dsp.deemphasis", &["dsp.fm_demod"]),
        ("dsp.biquad", &["dsp.fm_demod"]),
        ("dsp.audio_resampler", &["dsp.fm_demod"]),
        ("dsp.real_to_audio", &["dsp.fm_demod"]),
        ("dsp.deemphasis", &["dsp.fm_demod", "dsp.real_to_audio"]),
        ("dsp.null_sink", &[]),
    ];
    let mut bad = Vec::new();
    for (ty, pre) in cases {
        let whole = run(ty, pre, &x, x.len());
        // Tags are compared separately from items: they sit among the items of their call.
        let (wi, wt): (Vec<_>, Vec<_>) = whole.iter().partition(|s| !s.starts_with("Tag"));
        assert!(
            !wi.is_empty() || *ty == "dsp.null_sink",
            "{ty} gave nothing"
        );
        for piece in [1, 63, 1000] {
            let got = run(ty, pre, &x, piece);
            let (gi, gt): (Vec<_>, Vec<_>) = got.iter().partition(|s| !s.starts_with("Tag"));
            if gi != wi {
                let at = gi.iter().zip(&wi).position(|(a, b)| a != b);
                bad.push(format!(
                    "{ty} after {pre:?} (pieces of {piece}): items differ from {at:?}, {} vs {}",
                    gi.len(),
                    wi.len()
                ));
            }
            if gt != wt {
                bad.push(format!("{ty} (pieces of {piece}): tags {gt:?} vs {wt:?}"));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

/// The peak and the level that a power meter publishes after `x` is fed in pieces of `piece`.
fn meter(x: &[Complex32], piece: usize) -> (Option<f64>, Option<f64>) {
    let mut g = GraphBuilder::new()
        .add("level", components::PowerMeterSink::default())
        .input("level")
        .build()
        .unwrap();
    g.configure(&InputInfo::new(48e3, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    for c in x.chunks(piece) {
        g.process(c, &mut out);
    }
    let r = g.component_results("level").unwrap();
    let peak =
        sdrkit_core::publish::Field::<sdrkit_core::publish::field::F64>::new("peak_dbfs", "", "");
    (
        r.fields().get(&peak),
        r.fields().get(&sdrkit_core::publish::keys::LEVEL_DBFS),
    )
}

#[test]
fn the_power_meter_measures_whole_tenths_of_a_second_whatever_the_cuts() {
    let x = signal(48_000);
    let whole = meter(&x, 48_000);
    assert!(whole.0.is_some());
    for piece in [1000, 4799, 7777] {
        assert_eq!(meter(&x, piece), whole, "pieces of {piece}");
    }
}
