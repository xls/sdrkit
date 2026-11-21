//! Graph behaviour: wiring, formats, tags, settings, descriptions, taps, typed streams, and whole receivers through the Receiver
//! interface.

use sdrkit_core::publish::{field, keys, Field, Published};
use sdrkit_core::Owner;
use sdrkit_core::{
    Choice, Complex32, ConfigError, EventDef, InputInfo, Output, Receiver, Record, ResultsDecl,
    SettingError, TableDef, Value,
};
use sdrkit_graph::components::*;
use sdrkit_graph::*;
use std::f64::consts::TAU;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------------------------------------------------------
// Helpers

fn info(rate: f64) -> InputInfo {
    InputInfo {
        rate_hz: rate,
        centre_hz: 100e6,
        tuned_hz: 100e6,
        live: true,
    }
}

/// An AM signal: carrier at `offset_hz`, 50 % modulated by a 1 kHz tone.
fn am_signal(n: usize, rate: f64, offset_hz: f64) -> Vec<Complex32> {
    (0..n)
        .map(|i| {
            let t = i as f64 / rate;
            let e = 0.2 * (1.0 + 0.5 * (TAU * 1000.0 * t).cos());
            let p = TAU * offset_hz * t;
            Complex32::new((e * p.cos()) as f32, (e * p.sin()) as f32)
        })
        .collect()
}

/// A narrow FM signal: 2.5 kHz deviation by a 1 kHz tone, at `offset_hz`.
fn nfm_signal(n: usize, rate: f64, offset_hz: f64) -> Vec<Complex32> {
    (0..n)
        .map(|i| {
            let t = i as f64 / rate;
            let p = TAU * offset_hz * t + 2.5 * (TAU * 1000.0 * t).sin();
            Complex32::new((0.3 * p.cos()) as f32, (0.3 * p.sin()) as f32)
        })
        .collect()
}

/// Sets the setting `key` (an alias or `"<component>.<key>"`) of a graph.
fn set(g: &mut Graph, key: &str, v: Value) -> Result<(), SettingError> {
    let t = g.target(key)?;
    g.apply_to(t, &v)
}

fn feed(g: &mut dyn Receiver, x: &[Complex32], piece: usize) -> Output {
    let mut out = Output::default();
    for c in x.chunks(piece.max(1)) {
        g.feed(c, &mut out);
    }
    out
}

/// Feeds in pieces of varying sizes (a fixed pseudo-random sequence from 1 to `max`).
fn feed_ragged(g: &mut dyn Receiver, x: &[Complex32], max: usize) -> Output {
    let mut out = Output::default();
    let (mut i, mut s) = (0, 12345u64);
    while i < x.len() {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let n = (1 + (s as usize) % max).min(x.len() - i);
        g.feed(&x[i..i + n], &mut out);
        i += n;
    }
    out
}

fn amp(x: &[f32], f_cycles: f64) -> f64 {
    let (mut re, mut im) = (0.0, 0.0);
    for (i, v) in x.iter().enumerate() {
        let p = TAU * f_cycles * i as f64;
        re += *v as f64 * p.cos();
        im += *v as f64 * p.sin();
    }
    2.0 * (re * re + im * im).sqrt() / x.len() as f64
}

fn zero_crossings(x: &[f32]) -> usize {
    let mean = x.iter().sum::<f32>() / x.len() as f32;
    x.windows(2)
        .filter(|w| (w[0] - mean) < 0.0 && (w[1] - mean) >= 0.0)
        .count()
}

fn am_graph() -> Graph {
    GraphBuilder::new()
        .add(
            "chan",
            Channel::new(10e3, 24e3).unwrap().with_offset(30e3).unwrap(),
        )
        .add("demod", AmDemod::default())
        .add("deemph", DeemphasisComponent::new(0.0))
        .add("audio", AudioResampler::new(48e3).unwrap())
        .input("chan")
        .connect("chan", "demod")
        .connect("demod", "deemph")
        .connect("deemph", "audio")
        .output("audio")
        .alias("width", "chan.filter")
        .alias("detector", "demod.detector")
        .fix("deemph.deemphasis_us", Value::F64(0.0))
        .status_from("demod")
        .build()
        .unwrap()
}

const AM_JSON: &str = r#"{
  "components": [
    { "id": "chan", "type": "dsp.channel", "role": "processor", "settings": { "normal": 10000, "rate": 24000, "offset": 30000 } },
    { "id": "demod", "type": "dsp.am_demod" },
    { "id": "deemph", "type": "dsp.deemphasis" },
    { "id": "audio", "type": "dsp.audio_resampler", "settings": { "rate": 48000 } }
  ],
  "edges": [["input", "chan.in"], ["chan.out", "demod.in"], ["demod.out", "deemph.in"], ["deemph.out", "audio.in"]],
  "outputs": ["audio.out"],
  "aliases": { "width": "chan.filter", "detector": "demod.detector" },
  "fixed": { "deemph.deemphasis_us": 0 },
  "status": "demod"
}"#;

fn nfm_graph() -> Graph {
    GraphBuilder::new()
        .add(
            "chan",
            Channel::new(12.5e3, 24e3)
                .unwrap()
                .with_offset(-40e3)
                .unwrap(),
        )
        .add("demod", FmDemod::new(5e3).unwrap())
        .add("deemph", DeemphasisComponent::new(0.0))
        .add("audio", AudioResampler::new(48e3).unwrap())
        .input("chan")
        .connect("chan", "demod")
        .connect("demod", "deemph")
        .connect("deemph", "audio")
        .output("audio")
        .build()
        .unwrap()
}

/// Passes `Iq` through and tags chosen absolute positions with `Discontinuity { lost: position }`.
struct Tagger(Vec<u64>);
impl Component for Tagger {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("tagger", vec![Port::iq("in")], vec![Port::iq("out")])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        io.pass_through::<Complex32>(0, 0);
        let out = io.output(0);
        let (a, b) = (out.first(), out.end());
        for &p in &self.0 {
            if p >= a && p < b {
                out.push_tag(p, TagKind::Discontinuity { lost: p });
            }
        }
    }
}

/// Records the address of its input buffer and the tags it saw.
#[derive(Default, Clone)]
struct Seen {
    ptrs: Vec<(usize, usize)>,
    tags: Vec<Tag>,
    items: usize,
}
struct Probe(Arc<Mutex<Seen>>);
impl Component for Probe {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("probe", vec![Port::any("in")])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let b = io.input(0);
        let mut s = self.0.lock().unwrap();
        if let Data::Iq(v) = &b.data {
            s.ptrs.push((v.as_ptr() as usize, v.len()));
        }
        s.items += b.len();
        s.tags.extend(b.tags().iter().cloned());
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Tags

fn tag_positions(piece: usize) -> Vec<u64> {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let mut g = GraphBuilder::new()
        .add("tag", Tagger(vec![0, 3, 1000, 1001, 4003, 47_990, 47_999]))
        .add("dec", Decimate::new(4).unwrap())
        .add("res", Resample::new(8e3).unwrap())
        .add("probe", Probe(seen.clone()))
        .input("tag")
        .connect("tag", "dec")
        .connect("dec", "res")
        .connect("res", "probe")
        .build()
        .unwrap();
    g.configure(&info(48e3)).unwrap();
    feed(&mut g, &vec![Complex32::new(0.1, 0.0); 48_000], piece);
    let s = seen.lock().unwrap();
    assert_eq!(s.items, 8000);
    s.tags.iter().map(|t| t.at).collect()
}

#[test]
fn tags_move_through_decimation_and_resampling() {
    // 48 kHz -> 12 kHz (by 4) -> 8 kHz (by 2/3): position p lands on ceil(ceil(p / 4) * 2 / 3).
    let expected: Vec<u64> = [0u64, 3, 1000, 1001, 4003, 47_990]
        .iter()
        .map(|p| (p.div_ceil(4) * 2).div_ceil(3))
        .collect();
    assert_eq!(tag_positions(48_000), expected);
    for piece in [1, 7, 333, 4096] {
        assert_eq!(tag_positions(piece), expected, "cut into pieces of {piece}");
    }
}

#[test]
fn a_retune_reaches_the_components_as_a_tag() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let mut g = GraphBuilder::new()
        .add("dec", Decimate::new(2).unwrap())
        .add("probe", Probe(seen.clone()))
        .input("dec")
        .connect("dec", "probe")
        .build()
        .unwrap();
    g.configure(&info(48e3)).unwrap();
    feed(&mut g, &[Complex32::default(); 1000], 1000);
    g.retuned(&InputInfo {
        rate_hz: 48e3,
        centre_hz: 101e6,
        tuned_hz: 101e6,
        live: true,
    });
    feed(&mut g, &[Complex32::default(); 1000], 1000);
    let s = seen.lock().unwrap();
    assert_eq!(
        s.tags,
        vec![Tag {
            at: 500,
            kind: TagKind::Retuned {
                centre_hz: 101e6,
                tuned_hz: 101e6
            }
        }]
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// Formats

#[test]
fn formats_are_negotiated_and_renegotiated_on_a_rate_change() {
    let mut g = GraphBuilder::new()
        .add("dec", Decimate::new(4).unwrap())
        .add("res", Resample::new(8e3).unwrap())
        .add("sink", NullSink::default())
        .input("dec")
        .connect("dec", "res")
        .connect("res", "sink")
        .build()
        .unwrap();
    g.configure(&info(48e3)).unwrap();
    assert_eq!(g.format("dec").unwrap().rate, 12e3);
    assert_eq!(g.format("res.out").unwrap().rate, 8e3);
    assert_eq!(g.format("dec").unwrap().centre_hz, 100e6);

    g.configure(&info(96e3)).unwrap();
    assert_eq!(g.format("dec").unwrap().rate, 24e3);
    assert_eq!(g.format("res").unwrap().rate, 8e3);

    // A setting that changes a rate reformats from that component on.
    set(&mut g, "dec.factor", Value::I64(2)).unwrap();
    assert_eq!(g.format("dec").unwrap().rate, 48e3);
    set(&mut g, "res.rate", Value::Hz(16e3)).unwrap();
    assert_eq!(g.format("res").unwrap().rate, 16e3);
    let out = feed(&mut g, &vec![Complex32::default(); 96_000], 9600);
    assert!(out.audio().is_empty());
}

#[test]
fn a_rate_too_low_is_refused() {
    let mut g = am_graph();
    let e = g.configure(&info(8e3)).unwrap_err();
    assert_eq!(
        e,
        ConfigError::RateTooLow {
            need: 12.5e3,
            got: 8e3
        }
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// Whole receivers through the Receiver interface

#[test]
fn an_am_graph_decodes_a_tone_at_the_right_frequency() {
    let rate = 240e3;
    let mut g = am_graph();
    let rx: &mut dyn Receiver = &mut g;
    rx.configure(&info(rate)).unwrap();
    let x = am_signal(240_000 * 2, rate, 30e3);
    let out = feed(rx, &x, 4800);
    assert_eq!(out.audio_format(), Some((48e3, 1)), "the audio stays mono");
    let a = &out.audio();
    assert!((a.len() as i64 - 96_000).abs() < 100, "{} frames", a.len());
    let tail = &a[48_000..];
    let crossings = zero_crossings(tail);
    assert!(
        (995..=1005).contains(&crossings),
        "{crossings} cycles in a second"
    );
    let at = amp(tail, 1000.0 / 48e3);
    assert!((at - 0.5).abs() < 0.05, "1 kHz at {at}");
    assert!(amp(tail, 1500.0 / 48e3) < 0.01);
}

#[test]
fn an_nfm_graph_decodes_a_tone_at_the_right_frequency() {
    let rate = 240e3;
    let mut g = nfm_graph();
    Receiver::configure(
        &mut g,
        &InputInfo {
            rate_hz: rate,
            centre_hz: 100e6,
            tuned_hz: 100e6,
            live: true,
        },
    )
    .unwrap();
    let x = nfm_signal(480_000, rate, -40e3);
    let mut out = Output::default();
    for c in x.chunks(2400) {
        Receiver::feed(&mut g, c, &mut out);
    }
    let a = &out.audio();
    let tail = &a[48_000..];
    let crossings = zero_crossings(tail);
    assert!(
        (995..=1005).contains(&crossings),
        "{crossings} cycles in a second"
    );
    let at = amp(tail, 1000.0 / 48e3);
    assert!(
        (at - 0.5).abs() < 0.05,
        "1 kHz at {at} (2.5 kHz of a 5 kHz deviation)"
    );
}

#[test]
fn any_cut_of_the_stream_returns_the_same_output() {
    let x = am_signal(60_000, 240e3, 30e3);
    let run = |piece: usize| {
        let mut g = am_graph();
        g.configure(&info(240e3)).unwrap();
        if piece == 0 {
            feed_ragged(&mut g, &x, 5000).audio().to_vec()
        } else {
            feed(&mut g, &x, piece).audio().to_vec()
        }
    };
    let whole = run(60_000);
    assert!(!whole.is_empty());
    for piece in [1, 13, 4096, 0] {
        assert_eq!(run(piece), whole, "pieces of {piece}");
    }

    // A long chain of the other components too.
    let x = nfm_signal(50_000, 250e3, 20e3);
    let chain = |piece: usize| {
        let mut g = GraphBuilder::new()
            .add("dc", DcRemoval::default())
            .add("iq", IqCorrection::default())
            .add("nb", NoiseBlanker::default())
            .add("shift", Shift::new(-20e3).unwrap())
            .add("fir", FirFilter::lowpass(30e3, 5e3).unwrap())
            .add("dec", Decimate::new(5).unwrap())
            .add("agc", AgcComponent::default())
            .add("ssb", SsbDemod::new(Sideband::Usb))
            .add("sq", Squelch::level(-60.0).unwrap())
            .add(
                "bq",
                BiquadComponent::new(BiquadShape::Highpass, 100.0, 0.7).unwrap(),
            )
            .add("res", AudioResampler::new(44.1e3).unwrap())
            .add("spec", Spectrum::new(SpectrumSize::S256, 50.0).unwrap())
            .add("sink", NullSink::default())
            .input("dc")
            .connect("dc", "iq")
            .connect("iq", "nb")
            .connect("nb", "shift")
            .connect("shift", "fir")
            .connect("fir", "dec")
            .connect("dec", "agc")
            .connect("agc", "ssb")
            .connect("ssb", "sq")
            .connect("sq", "bq")
            .connect("bq", "res")
            .connect("dec", "spec")
            .connect("spec", "sink")
            .output("res")
            .build()
            .unwrap();
        g.configure(&info(250e3)).unwrap();
        let spec = g.tap("spec", 60_000).unwrap();
        let audio = if piece == 0 {
            feed_ragged(&mut g, &x, 3000).audio().to_vec()
        } else {
            feed(&mut g, &x, piece).audio().to_vec()
        };
        let mut frames = Vec::new();
        let mut spec = spec;
        let mut b = Buffer::default();
        while spec.pop(&mut b) {
            if let Data::Spectrum(f) = &b.data {
                frames.extend_from_slice(&f.values);
            }
        }
        (audio, frames)
    };
    let whole = chain(50_000);
    assert!(!whole.0.is_empty() && !whole.1.is_empty());
    for piece in [1, 999, 0] {
        let got = chain(piece);
        assert!(
            got.0 == whole.0,
            "chain audio, pieces of {piece}: first difference at {:?}",
            got.0.iter().zip(&whole.0).position(|(a, b)| a != b)
        );
        assert!(
            got.1 == whole.1,
            "chain spectrum, pieces of {piece}: first difference at {:?}",
            got.1.iter().zip(&whole.1).position(|(a, b)| a != b)
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Descriptions

#[test]
fn a_graph_from_json_matches_the_one_built_in_code_and_round_trips() {
    let registry = Registry::standard();
    let mut a = am_graph();
    let mut b = Graph::from_json(AM_JSON, &registry).unwrap();
    let x = am_signal(48_000, 240e3, 30e3);
    a.configure(&info(240e3)).unwrap();
    b.configure(&info(240e3)).unwrap();
    let (oa, ob) = (feed(&mut a, &x, 2400), feed(&mut b, &x, 2400));
    assert!(!oa.audio().is_empty());
    assert_eq!(oa.audio(), ob.audio());

    // The code-built graph describes itself; that description builds a graph with the same output and the same description.
    let d = a.describe();
    let json = d.to_json();
    let mut c = Graph::from_json(&json, &registry).unwrap();
    assert_eq!(c.describe(), d);
    c.configure(&info(240e3)).unwrap();
    let mut fresh = am_graph();
    fresh.configure(&info(240e3)).unwrap();
    assert_eq!(
        feed(&mut c, &x, 1000).audio().to_vec(),
        feed(&mut fresh, &x, 1000).audio().to_vec()
    );
}

#[test]
fn descriptions_check_types_and_roles() {
    let r = Registry::standard();
    let bad_type = r#"{ "components": [ { "id": "x", "type": "teleporter" } ] }"#;
    assert_eq!(
        Graph::from_json(bad_type, &r).unwrap_err(),
        GraphError::Description {
            at: DescPosition::Component { index: 0 },
            error: Box::new(GraphError::UnknownType("teleporter".into())),
        }
    );
    let bad_role = r#"{ "components": [ { "id": "x", "type": "dsp.null_sink", "role": "generator" } ], "edges": [["input", "x"]] }"#;
    let e = Graph::from_json(bad_role, &r).unwrap_err();
    assert!(e.to_string().contains("is a Sink"), "{e}");
    assert!(matches!(
        Graph::from_json("{", &r),
        Err(GraphError::Syntax { .. })
    ));
}

// ---------------------------------------------------------------------------------------------------------------------------
// Fan-out and taps

#[test]
fn a_fan_out_shares_the_producers_buffer() {
    let seen: Vec<_> = (0..3)
        .map(|_| Arc::new(Mutex::new(Seen::default())))
        .collect();
    let mut g = GraphBuilder::new()
        .add("dec", Decimate::new(2).unwrap())
        .add("a", Probe(seen[0].clone()))
        .add("b", Probe(seen[1].clone()))
        .add("c", Probe(seen[2].clone()))
        .input("dec")
        .connect("dec", "a")
        .connect("dec", "b")
        .connect("dec.out", "c.in")
        .build()
        .unwrap();
    g.configure(&info(48e3)).unwrap();
    feed(&mut g, &vec![Complex32::new(0.5, 0.0); 10_000], 1000);
    let p: Vec<Vec<(usize, usize)>> = seen
        .iter()
        .map(|s| s.lock().unwrap().ptrs.clone())
        .collect();
    assert_eq!(p[0].len(), 10);
    assert_eq!(p[0], p[1]);
    assert_eq!(p[0], p[2]);
    // And the producer's buffer is reused from call to call (no new allocation once grown).
    assert!(p[0][2..].iter().all(|q| q.0 == p[0][2].0));
}

#[test]
fn taps_receive_data_and_never_wait() {
    let mut g = am_graph();
    g.configure(&info(240e3)).unwrap();
    let x = am_signal(240_000, 240e3, 30e3);
    let mut reference = am_graph();
    reference.configure(&info(240e3)).unwrap();
    let expected = feed(&mut reference, &x, 2400).audio().to_vec();

    // A reader on another thread that reads slowly: the graph never waits, and drops what it cannot hold.
    let tap = g.tap("chan.out", 4).unwrap();
    // 240 kHz / 9: at least 24 kHz and 1.25 times the widest filter (20 kHz).
    assert_eq!(tap.format().rate, 240e3 / 9.0);
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let finished = done.clone();
    let reader = std::thread::spawn(move || {
        let mut tap = tap;
        let (mut items, mut b, mut last_end) = (0u64, Buffer::default(), 0);
        loop {
            let over = finished.load(std::sync::atomic::Ordering::Acquire);
            if tap.pop(&mut b) {
                assert!(b.first() >= last_end, "chunks arrive in order");
                last_end = b.end();
                items += b.len() as u64;
                std::thread::sleep(std::time::Duration::from_micros(200));
            } else if over {
                break;
            } else {
                std::thread::yield_now();
            }
        }
        (items, tap.dropped_items())
    });
    let t0 = std::time::Instant::now();
    let out = feed(&mut g, &x, 2400);
    assert!(t0.elapsed() < std::time::Duration::from_secs(5));
    assert_eq!(out.audio(), expected, "a tap does not change the output");
    done.store(true, std::sync::atomic::Ordering::Release);
    let (items, dropped) = reader.join().unwrap();
    assert_eq!(
        items + dropped,
        240_000 / 9,
        "{items} read, {dropped} dropped"
    );

    // A full tap that nobody reads only drops.
    let mut g = am_graph();
    g.configure(&info(240e3)).unwrap();
    let idle = g.tap("demod", 2).unwrap();
    let out = feed(&mut g, &x, 2400);
    assert_eq!(out.audio(), expected);
    assert_eq!(idle.waiting(), 2);
    assert_eq!(idle.dropped_chunks(), 98);
    // Dropping the reader removes the tap without disturbing the graph.
    assert_eq!(g.taps(), 1);
    drop(idle);
    assert_eq!(g.taps(), 0);
}

#[test]
fn adding_and_removing_taps_while_running_leaves_the_output_alone() {
    let x = am_signal(96_000, 240e3, 30e3);
    let mut reference = am_graph();
    reference.configure(&info(240e3)).unwrap();
    let expected = feed(&mut reference, &x, 2400).audio().to_vec();
    let mut g = am_graph();
    g.configure(&info(240e3)).unwrap();
    let mut out = Output::default();
    let mut taps = Vec::new();
    for (k, c) in x.chunks(2400).enumerate() {
        if k % 5 == 0 {
            taps.push(
                g.tap(if k % 2 == 0 { "input" } else { "audio" }, 3)
                    .unwrap(),
            );
        }
        if k % 7 == 0 && !taps.is_empty() {
            taps.remove(0);
        }
        g.feed(c, &mut out);
    }
    assert_eq!(out.audio(), expected);
    let mut b = Buffer::default();
    assert!(taps.last_mut().unwrap().pop(&mut b));
}

// ---------------------------------------------------------------------------------------------------------------------------
// Settings and results

#[test]
fn settings_by_component_key_and_by_alias() {
    let mut g = am_graph();
    g.configure(&info(240e3)).unwrap();
    let choice = |f: ChannelFilter| Value::Choice(f.index());
    set(&mut g, "chan.filter", choice(ChannelFilter::Wide)).unwrap();
    set(&mut g, "width", choice(ChannelFilter::Narrow)).unwrap();
    assert_eq!(
        g.describe().components[0].settings["filter"],
        serde_json::json!("narrow")
    );
    set(
        &mut g,
        "detector",
        Value::Choice(AmDetector::Synchronous.index()),
    )
    .unwrap();
    assert_eq!(
        g.describe().components[1].settings["detector"],
        serde_json::json!("synchronous")
    );
    // Strict: a value that is not a choice is refused with the reason, never replaced.
    let width = g.target("width").unwrap().id();
    assert_eq!(
        set(&mut g, "width", Value::Choice(99)),
        Err(SettingError::NotAChoice {
            id: width,
            index: 99
        })
    );
    let time = g.target("deemph.deemphasis_us").unwrap().id();
    assert_eq!(
        set(&mut g, "deemph.deemphasis_us", Value::F64(50.0)),
        Err(SettingError::Fixed { id: time }),
        "a fixed setting is refused"
    );
    assert_eq!(
        set(&mut g, "chan.colour", Value::F64(1.0)),
        Err(SettingError::Unknown {
            key: "chan.colour".into()
        })
    );
    assert!(matches!(
        set(&mut g, "nobody.width", Value::F64(1.0)),
        Err(SettingError::Unknown { .. })
    ));
    // A number out of the schema's range never reaches the component: it is refused, not clamped.
    let normal = g.target("chan.normal").unwrap().id();
    assert_eq!(
        set(&mut g, "chan.normal", Value::Hz(-5.0)),
        Err(SettingError::OutOfRange {
            id: normal,
            min: 1.0,
            max: 1e9,
            got: -5.0
        })
    );
    let schema = g.schema();
    let keys: Vec<&str> = schema
        .settings()
        .iter()
        .map(|s| s.field.key.as_ref())
        .collect();
    assert_eq!(keys, ["detector", "width"]);
    let width = schema.id("width").unwrap();
    assert_eq!(schema.setting(width).unwrap().field.label, "Filter");
}

/// The keys of the fields that hold a value, in order.
fn held_keys(r: &sdrkit_core::Results) -> Vec<String> {
    let mut keys: Vec<String> = r
        .fields()
        .iter()
        .filter(|(_, v)| **v != Value::None)
        .map(|(d, _)| d.key.to_string())
        .collect();
    keys.sort();
    keys
}

#[test]
fn results_are_merged_and_status_comes_from_the_status_component() {
    let mut g = GraphBuilder::new()
        .add("meter", PowerMeterSink::default())
        .add("chan", Channel::new(10e3, 24e3).unwrap())
        .add("demod", AmDemod::new(AmDetector::Synchronous))
        .add("meter2", PowerMeterSink::default())
        .input("meter")
        .input("chan")
        .connect("chan", "demod")
        .connect("chan", "meter2")
        .add("sink", NullSink::default())
        .connect("demod", "sink")
        .status_from("demod")
        .build()
        .unwrap();
    g.configure(&info(240e3)).unwrap();
    feed(&mut g, &am_signal(120_000, 240e3, 0.0), 4800);
    let r = Owner::results(&g);
    let held = |key: &str| {
        r.decl()
            .field_id(key)
            .and_then(|id| r.fields().value(id))
            .is_some_and(|v| *v != Value::None)
    };
    assert!(held("meter.level_dbfs"));
    assert!(held("meter2.level_dbfs"));
    assert!(
        held("carrier_offset_hz"),
        "the status component's fields keep their names: {:?}",
        held_keys(r)
    );
    assert!(r.status().offset_hz.is_some());
    assert!(
        r.status().level_dbfs.is_none(),
        "status comes from demod only"
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// Validation

#[test]
fn validation_names_what_is_wrong() {
    // Real into an `Iq` input: the analytic signal joins them.
    let mut g = GraphBuilder::new()
        .add("demod", FmDemod::default())
        .add("chan", Channel::default())
        .input("demod")
        .connect("demod", "chan")
        .build()
        .unwrap();
    let report = g.validate(Format::iq(240e3, 0.0, 0.0)).unwrap();
    assert_eq!(report.adapters.len(), 1);
    assert_eq!(report.adapters[0].kind, "dsp.analytic");
    assert_eq!(report.adapters[0].consumers, ["chan.in"]);

    let e = GraphBuilder::new()
        .add("demod", FmDemod::default())
        .build()
        .unwrap_err();
    assert_eq!(e, GraphError::Unconnected("demod.in".into()));
    assert_eq!(e.to_string(), "input demod.in is not connected");

    let e = GraphBuilder::new()
        .add("a", Decimate::new(2).unwrap())
        .add("b", Decimate::new(2).unwrap())
        .add("sink", NullSink::default())
        .connect("a", "b")
        .connect("b", "a")
        .input("sink")
        .build()
        .unwrap_err();
    assert_eq!(e, GraphError::Cycle(vec!["a".into(), "b".into()]));
    assert_eq!(e.to_string(), "the graph has a cycle through a, b");

    let e = GraphBuilder::new()
        .add("a", Decimate::new(2).unwrap())
        .input("a.input")
        .build()
        .unwrap_err();
    assert!(
        e.to_string()
            .contains("has no input \"input\" (its inputs: in)"),
        "{e}"
    );
    let e = GraphBuilder::new()
        .add("a", NullSink::default())
        .input("a")
        .input("a")
        .build()
        .unwrap_err();
    assert!(
        matches!(&e, GraphError::Connect { to, error, .. }
            if to == "a" && matches!(**error, ConnectError::PortTaken { .. })),
        "{e}"
    );
    let e = GraphBuilder::new()
        .add("a", NullSink::default())
        .add("a", NullSink::default())
        .build()
        .unwrap_err();
    assert_eq!(e, GraphError::DuplicateId("a".into()));
    let e = GraphBuilder::new()
        .add("a", Decimate::new(2).unwrap())
        .input("a")
        .output("a")
        .build()
        .unwrap_err();
    assert!(
        matches!(e, GraphError::BadOutput { .. }),
        "an iq output cannot leave the graph: {e}"
    );
    let e = GraphBuilder::new()
        .add("a", NullSink::default())
        .input("a")
        .connect("a", "a")
        .build()
        .unwrap_err();
    assert!(e.to_string().contains("has no output"), "{e}");
}

/// A sink that claims an output.
struct BadSink;
impl Component for BadSink {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo {
            outputs: vec![Port::real("out")],
            ..ComponentInfo::sink("bad", vec![Port::iq("in")])
        }
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, _: &mut Io) {}
}

#[test]
fn roles_are_checked() {
    let e = GraphBuilder::new().add("x", BadSink).build().unwrap_err();
    assert_eq!(
        e.to_string(),
        "component \"x\" is a Sink but it has outputs"
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// Spectrum

#[test]
fn the_spectrum_component_shows_a_tone_in_its_bin() {
    let rate = 256e3;
    let mut g = GraphBuilder::new()
        .add("spec", Spectrum::new(SpectrumSize::S1024, 30.0).unwrap())
        .add("sink", NullSink::default())
        .input("spec")
        .connect("spec", "sink")
        .build()
        .unwrap();
    g.configure(&info(rate)).unwrap();
    let f = g.format("spec").unwrap().clone();
    assert_eq!(
        (f.kind, f.frame_len, f.rate, f.bandwidth_hz),
        (Kind::Spectrum, 1024, 30.0, rate)
    );
    let tap = g.tap("spec", 100).unwrap();
    // 25 kHz is bin 100 above the centre (250 Hz per bin).
    let x: Vec<Complex32> = (0..256_000)
        .map(|i| Complex32::from_polar(1.0, (TAU * (25e3 * i as f64 / rate).fract()) as f32))
        .collect();
    feed(&mut g, &x, 10_000);
    let mut tap = tap;
    let (mut frames, mut b) = (0, Buffer::default());
    while tap.pop(&mut b) {
        let Data::Spectrum(fr) = &b.data else {
            panic!("spectrum frames")
        };
        for k in 0..fr.len() {
            let frame = fr.frame(k);
            let (peak, db) =
                frame
                    .iter()
                    .enumerate()
                    .fold((0, f32::MIN), |m, (i, &v)| if v > m.1 { (i, v) } else { m });
            assert_eq!(peak, 512 + 100);
            assert!(db.abs() < 0.5, "tone reads {db} dBFS");
            assert!(frame[200] < -90.0);
            frames += 1;
        }
    }
    assert!((29..=31).contains(&frames), "{frames} frames in a second");
}

// ---------------------------------------------------------------------------------------------------------------------------
// Typed streams

#[derive(Clone, Debug, PartialEq)]
struct VideoPacket {
    pts: i64,
    bytes: Vec<u8>,
}
impl Item for VideoPacket {
    const KIND: &'static str = "video.h264";
}

#[derive(Clone, Debug, PartialEq)]
struct Subtitle {
    text: String,
}
impl Item for Subtitle {
    const KIND: &'static str = "dvb.subtitle";
}

#[derive(Clone, Debug, PartialEq)]
struct EpgEvent {
    title: String,
    start: i64,
}
impl Item for EpgEvent {
    const KIND: &'static str = "epg.event";
    fn event_def() -> Option<&'static EventDef> {
        Some(&EPG_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        r.set(&TITLE, self.title.as_str());
        r.set(&START, self.start);
    }
}

const TITLE: Field<field::Text> = Field::new("title", "Title", "");
const START: Field<field::I64> = Field::new("start", "Start", "");
static EPG_EVENT: EventDef = EventDef::new("epg.event", "Programme", &[&TITLE, &START]);

/// Another type under the same kind name as `Subtitle`.
#[derive(Clone, Debug)]
struct FakeSubtitle;
impl Item for FakeSubtitle {
    const KIND: &'static str = "dvb.subtitle";
}

/// Makes one item of each type for every 1000 input samples, stamped with the time of the sample it came from.
struct Demux {
    rate: f64,
}
impl Component for Demux {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "demux",
            vec![Port::iq("in")],
            vec![
                Port::typed::<VideoPacket>("video"),
                Port::typed::<Subtitle>("subs"),
                Port::typed::<EpgEvent>("guide"),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        Ok(vec![
            Format::new(Kind::typed::<VideoPacket>(), 0.0),
            Format::new(Kind::typed::<Subtitle>(), 0.0),
            Format::new(Kind::typed::<EpgEvent>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let b = io.input(0);
        let (first, end) = (b.first(), b.end());
        for at in (first.div_ceil(1000) * 1000..end).step_by(1000) {
            let time = at as f64 / self.rate;
            let k = (at / 1000) as i64;
            io.items_out::<VideoPacket>(0).push(Timed {
                time,
                value: VideoPacket {
                    pts: k * 3600,
                    bytes: vec![k as u8; 4],
                },
            });
            io.items_out::<Subtitle>(1).push(Timed {
                time,
                value: Subtitle {
                    text: format!("line {k}"),
                },
            });
            io.items_out::<EpgEvent>(2).push(Timed {
                time,
                value: EpgEvent {
                    title: format!("show {k}"),
                    start: k,
                },
            });
        }
    }
}

/// A sink for one typed stream, keeping what it gets.
struct Keep<T: Item>(Arc<Mutex<Vec<Timed<T>>>>);
impl<T: Item> Component for Keep<T> {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("keep", vec![Port::typed::<T>("in")])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        self.0.lock().unwrap().extend_from_slice(io.items::<T>(0));
    }
}

#[test]
fn a_component_with_three_typed_outputs_feeds_three_consumers() {
    let video = Arc::new(Mutex::new(Vec::new()));
    let subs = Arc::new(Mutex::new(Vec::new()));
    let guide = Arc::new(Mutex::new(Vec::new()));
    let mut g = GraphBuilder::new()
        .add("demux", Demux { rate: 0.0 })
        .add("v", Keep::<VideoPacket>(video.clone()))
        .add("s", Keep::<Subtitle>(subs.clone()))
        .add("e", Keep::<EpgEvent>(guide.clone()))
        .input("demux")
        .connect("demux.video", "v")
        .connect("demux.subs", "s")
        .connect("demux.guide", "e")
        .output("demux.guide")
        .build()
        .unwrap();
    g.configure(&info(10e3)).unwrap();
    let tap = g.tap("demux.subs", 100).unwrap();
    let out = feed(&mut g, &vec![Complex32::default(); 10_000], 777);

    let v = video.lock().unwrap();
    assert_eq!(v.len(), 10);
    assert_eq!(
        v[3],
        Timed {
            time: 0.3,
            value: VideoPacket {
                pts: 3 * 3600,
                bytes: vec![3; 4]
            }
        }
    );
    assert_eq!(
        subs.lock().unwrap()[9].value,
        Subtitle {
            text: "line 9".into()
        }
    );
    assert_eq!(guide.lock().unwrap()[5].time, 0.5);
    // The typed output leaves the graph as JSON events, only there.
    assert_eq!(out.events.len(), 10);
    assert!(out.events[2].is(&EPG_EVENT));
    assert_eq!(out.events[2].time, 0.2);
    assert_eq!(out.events[2].get(&TITLE), Some("show 2"));
    // Taps copy typed items as well.
    let mut tap = tap;
    let (mut n, mut b) = (0, Buffer::default());
    while tap.pop(&mut b) {
        n += b.len();
    }
    assert_eq!(n, 10);
}

#[test]
fn mismatched_typed_streams_are_rejected() {
    let keep = Arc::new(Mutex::new(Vec::new()));
    let e = GraphBuilder::new()
        .add("demux", Demux { rate: 0.0 })
        .add("s", Keep::<Subtitle>(keep.clone()))
        .add("x", NullSink::default())
        .add("y", NullSink::default())
        .input("demux")
        .connect("demux.video", "s")
        .connect("demux.subs", "x")
        .connect("demux.guide", "y")
        .build()
        .unwrap_err();
    assert!(
        matches!(&e, GraphError::Connect { from, to, error }
            if from == "demux.video" && to == "s"
                && matches!(**error, ConnectError::ItemTypeMismatch { .. })),
        "{e}"
    );

    // The same kind name with another Rust type is rejected too.
    let fake = Arc::new(Mutex::new(Vec::<Timed<FakeSubtitle>>::new()));
    let e = GraphBuilder::new()
        .add("demux", Demux { rate: 0.0 })
        .add("s", Keep::<FakeSubtitle>(fake))
        .add("x", NullSink::default())
        .add("y", NullSink::default())
        .input("demux")
        .connect("demux.subs", "s")
        .connect("demux.video", "x")
        .connect("demux.guide", "y")
        .build()
        .unwrap_err();
    assert!(
        matches!(&e, GraphError::Connect { error, .. }
            if matches!(**error, ConnectError::ItemTypeMismatch { .. })),
        "{e}"
    );
}

/// Scales its input by a gain that a later component feeds back at each tick.
#[derive(Default)]
struct Scaled {
    gain: f32,
}
#[derive(Clone, Copy, Debug)]
struct Gain(f32);
impl Item for Gain {
    const KIND: &'static str = "test.gain";
}
impl Component for Scaled {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "scaled",
            vec![Port::iq("in"), Port::typed::<Gain>("gain")],
            vec![Port::real("out")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.gain = 1.0;
        Ok(vec![inputs[0].with_kind(Kind::Real)])
    }
    fn process(&mut self, io: &mut Io) {
        // What the meter decided at the end of the last period acts from the first sample of this one.
        if let Some(g) = io.items::<Gain>(1).last() {
            self.gain = g.value.0;
        }
        let g = self.gain;
        let x = io.iq(0);
        io.real_out(0).extend(x.iter().map(|v| v.re * g));
    }
}
/// At each tick, the gain that brings the period's peak to 1.
#[derive(Default)]
struct Leveller {
    peak: f32,
}
impl Component for Leveller {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "leveller",
            vec![Port::real("in")],
            vec![Port::typed::<Gain>("gain")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Gain>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        self.peak = io.real(0).iter().fold(self.peak, |p, v| p.max(v.abs()));
        if io.tick() {
            let g = if self.peak > 0.0 {
                1.0 / self.peak
            } else {
                1.0
            };
            self.peak = 0.0;
            io.items_out::<Gain>(0).push(Timed {
                time: 0.0,
                value: Gain(g),
            });
        }
    }
}

#[test]
fn a_feedback_loop_acts_at_the_ticks_whatever_the_cuts() {
    let build = || {
        let mut g = GraphBuilder::new()
            .add("scaled", Scaled::default())
            .add("level", Leveller::default())
            .add("sink", sdrkit_graph::components::NullSink::default())
            .input("scaled.in")
            .connect("scaled", "level")
            .connect("scaled", "sink")
            .feedback("level", "scaled.gain")
            .clock(0.01)
            .build()
            .expect("a feedback edge makes no cycle");
        g.configure(&info(48e3)).unwrap();
        g
    };
    let rate = 48e3f64;
    assert_eq!(build().clock_samples(), (0.01 * rate).round() as u64);
    let x: Vec<Complex32> = (0..(0.05 * rate) as usize)
        .map(|i| Complex32::new(0.25 * (i as f32 * 0.1).sin(), 0.0))
        .collect();
    let run = |piece: usize| {
        let mut g = build();
        let mut tap = g.tap("scaled", 10_000).unwrap();
        for c in x.chunks(piece) {
            g.feed(c, &mut Output::default());
        }
        let (mut all, mut b) = (Vec::new(), Buffer::default());
        while tap.pop(&mut b) {
            if let Data::Real(v) = &b.data {
                all.extend_from_slice(v);
            }
        }
        all
    };
    let whole = run(x.len());
    assert_eq!(run(333), whole);
    assert_eq!(run(1), whole);
    // The first period at gain 1, the next ones levelled to a peak of 1.
    let p = (0.01 * rate).round() as usize;
    let peak = |s: &[f32]| s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!((peak(&whole[..p]) - 0.25).abs() < 1e-3);
    assert!((peak(&whole[p..2 * p]) - 1.0).abs() < 1e-3);
    // A feedback edge is described, and builds again.
    let d = build().describe();
    assert_eq!(
        d.feedback,
        vec![("level.gain".to_string(), "scaled.gain".to_string())]
    );
    assert_eq!(d.clock_s, Some(0.01));
}

// ---------------------------------------------------------------------------------------------------------------------------
// The rule for result keys

const SUMMARY_TEXT: Field<field::Text> = Field::new("summary_text", "Summary", "");
const QUEUE: Field<field::Text> = Field::new("queue", "Queue", "");
const NODES: Field<field::I64> = Field::new("nodes", "Nodes", "");
const TEXTS: Field<field::I64> = Field::new("texts", "Texts", "");
static HEARD: TableDef = TableDef {
    key: "heard",
    label: "Heard",
    columns: &[],
    cap: 4,
};

/// A sink whose results the test writes through the graph's owner access.
struct Publisher(Published);
impl Component for Publisher {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("test.publisher", vec![Port::iq("in")])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, _: &mut Io) {}
    fn results(&self) -> Option<&sdrkit_core::Results> {
        Some(self.0.results())
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

fn publishers(decl: Option<&ResultsDecl>) -> Graph {
    let b = GraphBuilder::new()
        .add(
            "status",
            Publisher(Published::new(&[&keys::SNR_DB, &SUMMARY_TEXT])),
        )
        .add(
            "mt",
            Publisher(
                Published::new(&[&NODES, &keys::SNR_DB, &TEXTS, &QUEUE]).with_tables(&[&HEARD]),
            ),
        )
        .add("mc", Publisher(Published::new(&[&TEXTS, &QUEUE])))
        .input("status")
        .input("mt")
        .input("mc")
        .status_from("status");
    match decl {
        Some(d) => b.declare(d),
        None => b,
    }
    .build()
    .unwrap()
}

fn publish(g: &mut Graph, id: &str, f: impl FnOnce(&mut Published)) {
    f(&mut g.component_as_mut::<Publisher>(id).unwrap().0);
}

#[test]
fn declared_keys_keep_their_names_from_any_component_and_the_rest_are_prefixed() {
    let decl = ResultsDecl::new()
        .with_fields(&[&keys::SNR_DB, &NODES, &TEXTS])
        .with_table(&HEARD);
    let fill = |g: &mut Graph| {
        publish(g, "status", |r| {
            r.set(&keys::SNR_DB, 12.5);
            r.set(&SUMMARY_TEXT, "status only");
        });
        publish(g, "mt", |r| {
            r.set(&NODES, 4);
            r.set(&keys::SNR_DB, -3.0); // declared, but the status component sets it
            r.set(&TEXTS, 9);
            r.set(&QUEUE, "2"); // not declared
        });
        publish(g, "mc", |r| {
            r.set(&TEXTS, 1); // declared, but mt runs first and sets it
            r.set(&QUEUE, "5");
        });
        g.refresh_results();
    };

    // Without a declaration every key of a component other than the status one is prefixed.
    let mut g = publishers(None);
    g.configure(&info(48e3)).unwrap();
    fill(&mut g);
    assert_eq!(
        held_keys(Owner::results(&g)),
        [
            "mc.queue",
            "mc.texts",
            "mt.nodes",
            "mt.queue",
            "mt.snr_db",
            "mt.texts",
            "snr_db",
            "summary_text"
        ]
    );
    assert!(Owner::results(&g).decl().table_id("mt.heard").is_some());

    // With the declaration, the declared keys of every component keep their own names.
    let mut g = publishers(Some(&decl));
    g.configure(&info(48e3)).unwrap();
    fill(&mut g);
    let r = Owner::results(&g);
    assert_eq!(
        held_keys(r),
        [
            "mc.queue",
            "mt.queue",
            "nodes",
            "snr_db",
            "summary_text",
            "texts"
        ]
    );
    assert_eq!(r.fields().get(&keys::SNR_DB), Some(12.5));
    assert_eq!(r.fields().get(&TEXTS), Some(9));
    assert_eq!(r.fields().get(&NODES), Some(4));
    assert!(r.decl().table_id("heard").is_some());

    // A declared key that its component stops publishing leaves the merged results.
    publish(&mut g, "mt", |r| r.none(&NODES));
    g.refresh_results();
    assert_eq!(Owner::results(&g).fields().get(&NODES), None);

    // A declaration given later merges again under the new rule.
    let mut g = publishers(None);
    g.configure(&info(48e3)).unwrap();
    fill(&mut g);
    g.declare(&decl);
    assert_eq!(
        held_keys(Owner::results(&g)),
        [
            "mc.queue",
            "mt.queue",
            "nodes",
            "snr_db",
            "summary_text",
            "texts"
        ]
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// Interleaved audio

/// Makes one audio frame of `channels` channels per input sample, and tags frame 100.
struct AudioMaker(u16);
impl Component for AudioMaker {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "audio_maker",
            vec![Port::iq("in")],
            vec![Port::audio("out")],
        )
        .manual_tags()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::audio(inputs[0].rate, self.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(0);
        let out = io.audio_out(0);
        for k in 0..n {
            let frame = [k as f32 * 1e-4; 2];
            out.push_frame(&frame[..usize::from(self.0)]);
        }
        let b = io.output(0);
        if (b.first()..b.end()).contains(&100) {
            b.push_tag(100, TagKind::Discontinuity { lost: 0 });
        }
    }
}

/// Records the address of its audio input buffer, its channel count and the tags it saw.
struct AudioProbe(Probed);
impl Component for AudioProbe {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("audio_probe", vec![Port::audio("in")])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let a = io.audio(0);
        let mut s = self.0.lock().unwrap();
        s.0.push(a.samples.as_ptr() as usize);
        s.1 = a.channels;
        s.2.extend(io.input(0).tags().iter().cloned());
    }
}

type Probed = Arc<Mutex<(Vec<usize>, u16, Vec<Tag>)>>;

fn run_audio(channels: u16) -> (Probed, Probed) {
    let (p, q) = (Probed::default(), Probed::default());
    let mut g = GraphBuilder::new()
        .add("make", AudioMaker(channels))
        .add(
            "eq",
            BiquadComponent::new(BiquadShape::Lowpass, 3000.0, 0.7).unwrap(),
        )
        .add("p", AudioProbe(p.clone()))
        .add("q", AudioProbe(q.clone()))
        .input("make")
        .connect("make", "eq")
        .connect("eq", "p")
        .connect("eq", "q")
        .build()
        .unwrap();
    g.configure(&info(48e3)).unwrap();
    feed(&mut g, &[Complex32::default(); 1000], 250);
    (p, q)
}

#[test]
fn mono_and_stereo_audio_pass_by_reference_and_tags_count_frames() {
    let mut at = Vec::new();
    for channels in [1, 2] {
        let (p, q) = run_audio(channels);
        let (p, q) = (p.lock().unwrap(), q.lock().unwrap());
        assert_eq!((p.1, q.1), (channels, channels));
        assert_eq!(p.0, q.0, "both consumers read the same buffer");
        assert_eq!(p.2.len(), 1);
        at.push(p.2[0].at);
    }
    assert_eq!(at, [100, 100], "the tag sits on the same frame");
}
