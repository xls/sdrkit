//! `modem.sync_search`: the CCSDS attached sync marker 0x1ACFFC1D found in soft bits and in soft symbols, as written and
//! inverted (the bits after an inverted match come out inverted back), with allowed errors; behind `modem.fsk_demod` on a 2-FSK
//! signal from `sdrkit-algo-sim`; cuts; strict settings.

use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_components::modem::demod::{FskDemod, SyncSearch};
use sdrkit_components::testing::Source;
use sdrkit_core::{Complex32, Output, Value};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{components::NullSink, Applied, Format, Graph, Kind, Tag, TagKind};

#[path = "modem_demod_util.rs"]
mod util;
#[allow(unused_imports)]
use util::set_util::{set, txt, Tb};
use util::{hard, random_bits};

const ASM: u32 = 0x1ACF_FC1D;

fn asm_bits() -> Vec<u8> {
    (0..32).map(|k| ((ASM >> (31 - k)) & 1) as u8).collect()
}

/// 1000 bits: the marker at 200 (with bits 3 and 17 wrong), the inverted marker at 600 with every bit after it inverted too.
fn frame() -> (Vec<u8>, Vec<u8>) {
    let truth = random_bits(1000, 31);
    let mut sent = truth.clone();
    let w = asm_bits();
    let mut wrong = w.clone();
    wrong[3] ^= 1;
    wrong[17] ^= 1;
    sent[200..232].copy_from_slice(&wrong);
    let mut t = truth;
    t[200..232].copy_from_slice(&w);
    t[600..632].copy_from_slice(&w);
    for (s, &b) in sent[600..].iter_mut().zip(&t[600..]) {
        *s = b ^ 1;
    }
    // `t` holds what downstream should see: every bit after the inverted marker inverted back (the marker itself as sent).
    t[600..632].iter_mut().for_each(|b| *b ^= 1);
    (sent, t)
}

/// Runs a source graph for `passes` and gathers output `port` (`Llr` or `Symbols`) with its tags.
fn run(g: &mut Graph, passes: usize, port: &str) -> (Data, Vec<Tag>) {
    g.configure_input(Format::iq(1.0, 0.0, 0.0)).unwrap();
    let mut data: Option<Data> = None;
    let mut tags = Vec::new();
    let mut out = Output::default();
    for _ in 0..passes {
        out.clear();
        g.process(&[], &mut out);
        let b = g.buffer(port).unwrap();
        let d = data.get_or_insert_with(|| g.format(port).unwrap().kind.empty_data());
        sdrkit_components::testing::extend_range(d, &b.data, 0, b.data.len());
        tags.extend(b.tags().iter().cloned());
    }
    (data.unwrap(), tags)
}

fn frame_starts(tags: &[Tag]) -> Vec<(u64, bool, u32)> {
    tags.iter()
        .filter_map(|t| match t.kind {
            TagKind::FrameStart { inverted, errors } => Some((t.at, inverted, errors)),
            _ => None,
        })
        .collect()
}

fn search(src: Source, set: &[(&str, Value)]) -> Graph {
    search_on(src, SyncSearch::default(), set)
}

fn search_on(src: Source, sync: SyncSearch, set: &[(&str, Value)]) -> Graph {
    let mut b = Tb::new()
        .add("src", src)
        .add("sync", sync)
        .add("sink", NullSink::default())
        .connect("src", "sync")
        .connect("sync", "sink")
        .set("sync.word", txt("0x1ACFFC1D"));
    for (k, v) in set {
        b = b.set(&format!("sync.{k}"), v.clone());
    }
    b.build().unwrap()
}

#[test]
fn the_marker_is_found_in_soft_bits_and_inverted_bits_are_turned_back() {
    let (sent, truth) = frame();
    let mut rng = Rng::new(5);
    let llr: Vec<i8> = sent
        .iter()
        .map(|&b| {
            ((1.0 - 2.0 * b as f32) * 40.0 + 8.0 * rng.normal_f32()).clamp(-127.0, 127.0) as i8
        })
        .collect();
    let set = [
        ("max_errors", Value::F64(2.0)),
        ("polarity", Value::Text("both".into())),
    ];
    let mut first = None;
    for per_pass in [1000usize, 7, 1] {
        let src = Source::new(
            Format::llr(1200.0, Some(4.0)),
            Data::Llr(llr.clone()),
            per_pass,
        );
        let mut g = search(src, &set);
        let (d, tags) = run(&mut g, 1000 / per_pass + 2, "sync.out");
        assert_eq!(frame_starts(&tags), [(232, false, 2), (632, true, 0)]);
        let got = hard(&d);
        assert_eq!(&got[232..], &truth[232..], "per pass {per_pass}");
        assert_eq!(g.format("sync.out").unwrap().llr_scale, Some(4.0));
        match &first {
            None => first = Some(got),
            Some(f) => assert_eq!(&got, f),
        }
    }
    // As written only: the inverted marker is not a match, and nothing is inverted.
    let src = Source::new(Format::llr(1200.0, None), Data::Llr(llr.clone()), 100);
    let mut g = search(src, &[("max_errors", Value::F64(2.0))]);
    let (d, tags) = run(&mut g, 12, "sync.out");
    assert_eq!(frame_starts(&tags), [(232, false, 2)]);
    assert_eq!(hard(&d), sent);
    // With no errors allowed, the first marker is missed too.
    let src = Source::new(Format::llr(1200.0, None), Data::Llr(llr), 100);
    let mut g = search(src, &[("polarity", Value::Text("both".into()))]);
    assert_eq!(
        frame_starts(&run(&mut g, 12, "sync.out").1),
        [(632, true, 0)]
    );
}

#[test]
fn the_marker_is_found_in_soft_symbols() {
    let (sent, truth) = frame();
    let mut rng = Rng::new(6);
    // BPSK at gain 0.7 with an offset of 0.2 and noise.
    let sym: Vec<Complex32> = sent
        .iter()
        .map(|&b| {
            Complex32::new(
                (1.0 - 2.0 * b as f32) * 0.7 + 0.2 + 0.1 * rng.normal_f32(),
                0.1 * rng.normal_f32(),
            )
        })
        .collect();
    let set = [
        ("max_errors", Value::F64(2.0)),
        ("polarity", Value::Text("both".into())),
        ("threshold", Value::F64(0.7)),
    ];
    let mut first = None;
    for per_pass in [1000usize, 13, 1] {
        let src = Source::new(
            Format::new(Kind::Symbols, 1200.0),
            Data::Symbols(sym.clone()),
            per_pass,
        );
        let mut g = search_on(src, SyncSearch::on_symbols(), &set);
        let (d, tags) = run(&mut g, 1000 / per_pass + 2, "sync.out");
        assert_eq!(frame_starts(&tags), [(232, false, 2), (632, true, 0)]);
        let Data::Symbols(z) = &d else { panic!() };
        // Every symbol but the last four (held until a later one arrives), inverted back after the inverted marker.
        assert_eq!(z.len(), 996);
        let got: Vec<u8> = z.iter().map(|v| (v.re < 0.2) as u8).collect();
        assert_eq!(&got[232..], &truth[232..996], "per pass {per_pass}");
        assert_eq!(&got[..200], &sent[..200]);
        match &first {
            None => first = Some(z.clone()),
            Some(f) => assert_eq!(z, f),
        }
    }
}

#[test]
fn behind_the_fsk_demodulator_the_marker_starts_the_frame() {
    // 2-FSK made by sdrkit-algo-sim, demodulated to soft bits, the marker found, and the payload after it read back exactly.
    let mut bits = random_bits(1200, 32);
    bits[400..432].copy_from_slice(&asm_bits());
    let mut m = Cpm::cpfsk(1.0, 10.0).unwrap();
    let a: Vec<f32> = bits.iter().map(|&b| 2.0 * b as f32 - 1.0).collect();
    let mut x = Vec::new();
    m.push(&a, &mut x);
    m.flush(&mut x);
    let mut g = Tb::new()
        .add("fsk", FskDemod::default())
        .add("sync", SyncSearch::default())
        .add("sink", NullSink::default())
        .input("fsk")
        .connect("fsk.out", "sync")
        .connect("sync", "sink")
        .set("fsk.shift_hz", Value::F64(1200.0))
        .set("fsk.baud", Value::F64(1200.0))
        .set("sync.word", txt("0001 1010 1100 1111 1111 1100 0001 1101"))
        .build()
        .unwrap();
    g.configure_input(Format::iq(12_000.0, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    let mut all = Vec::new();
    let mut starts = Vec::new();
    for p in x.chunks(2000) {
        out.clear();
        g.process(p, &mut out);
        let b = g.buffer("sync.out").unwrap();
        starts.extend(frame_starts(b.tags()));
        all.extend(hard(&b.data));
    }
    assert_eq!(starts.len(), 1, "{starts:?}");
    let (at, inverted, errors) = starts[0];
    assert!(!inverted && errors == 0);
    let at = at as usize;
    assert_eq!(&all[at..at + 600], &bits[432..1032]);
}

#[test]
fn settings_are_strict() {
    let mut s = SyncSearch::default();
    assert_eq!(
        set(&mut s, "word", &Value::Text("0x1ACFFC1D".into())),
        Ok(Applied::Reformat)
    );
    assert!(set(&mut s, "word", &Value::Text("0x1ACG".into())).is_err());
    assert!(set(&mut s, "word", &Value::Text("1012".into())).is_err());
    assert!(set(&mut s, "max_errors", &Value::F64(-1.0)).is_err());
    assert!(set(&mut s, "polarity", &Value::Text("upside_down".into())).is_err());
    assert!(set(&mut s, "threshold", &Value::F64(1.5)).is_err());
    assert!(set(&mut s, "colour", &Value::Bool(true)).is_err());
    // No word, too many errors for the word, or hard bits, are refused when the graph configures.
    let llr = || Source::new(Format::llr(1.0, None), Data::Llr(vec![0; 4]), 4);
    let mut g = Tb::new()
        .add("src", llr())
        .add("sync", SyncSearch::default())
        .add("sink", NullSink::default())
        .connect("src", "sync")
        .connect("sync", "sink")
        .build()
        .unwrap();
    assert!(g.configure_input(Format::iq(1.0, 0.0, 0.0)).is_err());
    let mut g = search(llr(), &[("max_errors", Value::F64(16.0))]);
    assert!(g.configure_input(Format::iq(1.0, 0.0, 0.0)).is_err());
    let mut g = search(llr(), &[]);
    assert!(g.configure_input(Format::iq(1.0, 0.0, 0.0)).is_ok());
    let hard_src = Source::new(Format::hard_bits(1.0), Data::HardBits(vec![0; 4]), 4);
    assert!(Tb::new()
        .add("src", hard_src)
        .add("sync", SyncSearch::default())
        .add("sink", NullSink::default())
        .connect("src", "sync")
        .connect("sync", "sink")
        .build()
        .is_err());
}
