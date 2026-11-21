//! Each `modem.*` synchronisation component in a small graph: a known answer, the same output for any cut of the input, and strict
//! settings.

mod set_util;
use set_util::{result, result_f64, run_iq, run_passes, set, Tb};

use sdrkit_algo_dsp::constellation::by_name as point_set;
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_sim::modulate::{Cpm, Linear};
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_algo_sim::offset::FrequencyOffset;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_components::modem::sync::points::ConstellationName;
use sdrkit_components::modem::sync::{
    BurstDetect, CarrierSyncComponent, Equaliser, FreqEstimate, FreqSearch, IqBurst, MatchedFilter,
    Slicer, SnrMeter, SquelchComponent, SymbolSyncComponent,
};
use sdrkit_components::testing::Source;
use sdrkit_core::{Choice, Complex32, Output, Value};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Component, Format, Graph, Kind, TagKind, Timed};
use std::f32::consts::FRAC_1_SQRT_2 as S;

const RATE: f64 = 48_000.0;

fn qpsk_symbols(n: usize, seed: u64) -> Vec<Complex32> {
    let mut rng = Rng::new(seed);
    (0..n)
        .map(|_| {
            Complex32::new(
                if rng.bit() == 0 { S } else { -S },
                if rng.bit() == 0 { S } else { -S },
            )
        })
        .collect()
}

fn rrc_signal(syms: &[Complex32], sps: f64) -> Vec<Complex32> {
    let mut tx = Linear::rrc(0.35, 16.0, sps).unwrap();
    let mut x = Vec::new();
    tx.push(syms, &mut x);
    tx.flush(&mut x);
    x
}

fn iq(d: Data) -> Vec<Complex32> {
    match d {
        Data::Iq(v) | Data::Symbols(v) => v,
        d => panic!("{d:?}"),
    }
}

fn one(id: &str, c: impl Component + 'static) -> Graph {
    Tb::new().add(id, c).input(id).build().unwrap()
}

/// A graph feeding `data` (of `format`) from a source, `per_pass` at a time, into `c`.
fn fed(format: Format, data: Data, per_pass: usize, c: impl Component + 'static) -> Graph {
    Tb::new()
        .add("src", Source::new(format, data, per_pass))
        .add("c", c)
        .connect("src", "c")
        .build()
        .unwrap()
}

fn constellation(key: &str) -> ConstellationName {
    (0..)
        .map_while(ConstellationName::from_index)
        .find(|c| c.key() == key)
        .unwrap_or_else(|| panic!("no constellation {key}"))
}

/// A squelch of `mode` and `threshold_db`, made through its settings.
fn squelch(mode: &str, threshold_db: f64) -> Result<SquelchComponent, sdrkit_core::SettingError> {
    let mut s = SquelchComponent::default();
    set(&mut s, "mode", &Value::Text(mode.into()))?;
    set(&mut s, "threshold_db", &Value::F64(threshold_db))?;
    Ok(s)
}

fn passes(n: usize, per: usize) -> usize {
    n.div_ceil(per) + 1
}

fn near_point(z: Complex32, name: &str) -> f32 {
    let p = point_set(name).unwrap();
    p.points()
        .iter()
        .map(|q| (z - q).norm())
        .fold(f32::INFINITY, f32::min)
}

fn refuses(c: &mut dyn Component, key: &str, v: Value) {
    assert!(set(c, key, &v).is_err(), "{key} = {v:?} was accepted");
}

// ---------------------------------------------------------------------------------------------------------------------------
// modem.matched_filter

#[test]
fn the_matched_filter_answers_an_impulse_with_its_taps() {
    let h = taps(Shape::Rrc { beta: 0.35 }, 4.0, 8.0, Norm::Energy).unwrap();
    let mut x = vec![Complex32::default(); 200];
    x[0] = Complex32::new(1.0, 0.0);
    for (dec, piece) in [(1usize, 200usize), (1, 7), (2, 1), (2, 64)] {
        let mut g = one("mf", MatchedFilter::rrc(0.35, 4.0, dec).unwrap());
        let y = iq(run_iq(
            &mut g,
            Format::iq(RATE, 0.0, 0.0),
            &x,
            piece,
            "mf.out",
        ));
        assert_eq!(y.len(), 200 / dec);
        assert_eq!(g.format("mf.out").unwrap().rate, RATE / dec as f64);
        for (k, v) in y.iter().enumerate() {
            let want = h.get(k * dec + dec - 1).copied().unwrap_or(0.0);
            assert!(
                (v.re - want).abs() < 1e-6 && v.im == 0.0,
                "decimate {dec}: {k}: {v} {want}"
            );
        }
    }
    let mut m = MatchedFilter::default();
    refuses(&mut m, "pulse", Value::Text("sinc".into()));
    refuses(&mut m, "beta", Value::F64(1.5));
    refuses(&mut m, "decimate", Value::F64(2.5));
    refuses(&mut m, "span", Value::Text("long".into()));
    refuses(&mut m, "colour", Value::F64(1.0));
    assert!(set(&mut m, "pulse", &Value::Text("gaussian".into())).is_ok());
}

// ---------------------------------------------------------------------------------------------------------------------------
// modem.symbol_sync

fn sync_graph(detector: &str, interp: &str, matched_first: bool) -> Graph {
    let mut b = Tb::new();
    let mut ss = SymbolSyncComponent::new(if matched_first { 2.0 } else { 4.0 }).unwrap();
    set(&mut ss, "detector", &Value::Text(detector.into())).unwrap();
    set(&mut ss, "interp", &Value::Text(interp.into())).unwrap();
    if matched_first {
        b = b
            .add("mf", MatchedFilter::rrc(0.35, 4.0, 2).unwrap())
            .input("mf")
            .connect("mf", "ss");
    } else {
        b = b.input("ss");
    }
    b.add("ss", ss).build().unwrap()
}

#[test]
fn every_detector_and_interpolator_locks_on_rrc_qpsk() {
    let syms = qpsk_symbols(3000, 1);
    let mut x = rrc_signal(&syms, 4.0);
    Awgn::with_power(0.02, 3).unwrap().process(&mut x);
    for det in ["gardner", "mm", "early_late", "zero_crossing"] {
        for interp in ["linear", "cubic", "polyphase", "matched"] {
            let mut g = sync_graph(det, interp, interp != "matched");
            let y = iq(run_iq(
                &mut g,
                Format::iq(RATE, 0.0, 0.0),
                &x,
                1000,
                "ss.out",
            ));
            assert!(
                y.len() > 2950 && y.len() < 3050,
                "{det} {interp}: {} symbols",
                y.len()
            );
            let worst = y[2500..2900]
                .iter()
                .map(|z| near_point(*z, "qpsk"))
                .fold(0f32, f32::max);
            assert!(worst < 0.35, "{det} {interp}: worst distance {worst}");
            assert_eq!(
                result(&g, "ss", "locked"),
                Value::Bool(true),
                "{det} {interp}"
            );
            assert_eq!(g.format("ss.out").unwrap().kind, Kind::Symbols);
        }
    }
}

#[test]
fn symbol_sync_returns_errors_per_symbol_two_outputs_per_symbol_and_the_same_output_for_any_cut() {
    let x = rrc_signal(&qpsk_symbols(1000, 2), 4.0);
    let run = |piece: usize, ops: f64| {
        let mut ss = SymbolSyncComponent::new(4.0).unwrap();
        set(&mut ss, "interp", &Value::Text("matched".into())).unwrap();
        set(&mut ss, "outputs_per_symbol", &Value::F64(ops)).unwrap();
        let mut g = one("ss", ss);
        let y = iq(run_iq(
            &mut g,
            Format::iq(RATE, 0.0, 0.0),
            &x,
            piece,
            "ss.out",
        ));
        let Data::Real(e) = run_iq(&mut g, Format::iq(RATE, 0.0, 0.0), &x, piece, "ss.error")
        else {
            panic!()
        };
        assert_eq!(g.format("ss.out").unwrap().rate, RATE / 4.0 * ops);
        assert_eq!(g.format("ss.error").unwrap().rate, RATE / 4.0);
        (y, e)
    };
    let (y, e) = run(4096, 1.0);
    assert_eq!(y.len(), e.len());
    // Locked: the error averages to 0.
    let mean = e[600..].iter().sum::<f32>() / (e.len() - 600) as f32;
    assert!(mean.abs() < 0.02, "mean timing error {mean}");
    for piece in [1, 13, 1000] {
        let (y2, e2) = run(piece, 1.0);
        assert_eq!(y, y2, "piece {piece}");
        assert_eq!(e, e2, "piece {piece}");
    }
    let (y2, e2) = run(500, 2.0);
    assert!(
        y2.len() == 2 * e2.len() || y2.len() == 2 * e2.len() + 1,
        "{} {}",
        y2.len(),
        e2.len()
    );

    let mut s = SymbolSyncComponent::default();
    refuses(&mut s, "detector", Value::Text("costas".into()));
    refuses(&mut s, "interp", Value::Text("sinc".into()));
    refuses(&mut s, "outputs_per_symbol", Value::F64(3.0));
    refuses(&mut s, "sps", Value::F64(1.0));
    refuses(&mut s, "bandwidth", Value::F64(0.6));
    refuses(&mut s, "oqpsk", Value::Text("maybe".into()));
    // A matched filter too long for the bank is refused at configure.
    set(&mut s, "interp", &Value::Text("matched".into())).unwrap();
    set(&mut s, "span", &Value::F64(32.0)).unwrap();
    let g = Tb::new().add("ss", s).input("ss").build();
    assert!(g
        .unwrap()
        .configure_input(Format::iq(RATE, 0.0, 0.0))
        .is_err());
}

// ---------------------------------------------------------------------------------------------------------------------------
// modem.carrier_sync

/// Symbols of `set` (random labels, or the known pilot every `pilot` symbols) turned by 0.7 rad and drifting `f` cycles per
/// symbol, with a little noise.
fn turned(name: &str, n: usize, f: f64, pilot: Option<usize>) -> Vec<Complex32> {
    let p = point_set(name).unwrap();
    let mut rng = Rng::new(5);
    (0..n)
        .map(|k| {
            let d = match pilot {
                Some(sp) if k % sp == 0 => Complex32::new(S, S),
                _ => p.points()[rng.below(p.len() as u64) as usize],
            };
            let ph = 0.7 + std::f64::consts::TAU * f * k as f64;
            d * Complex32::from_polar(1.0, ph as f32) + rng.complex_normal(1e-4)
        })
        .collect()
}

fn carrier(detector: &str, name: &str, order: f64) -> CarrierSyncComponent {
    let mut c = CarrierSyncComponent::default();
    set(&mut c, "detector", &Value::Text(detector.into())).unwrap();
    set(&mut c, "constellation", &Value::Text(name.into())).unwrap();
    set(&mut c, "order", &Value::F64(order)).unwrap();
    c
}

#[test]
fn every_carrier_detector_locks_and_measures_the_offset() {
    let rate = 10_000.0;
    for (det, name, order, pilot) in [
        ("costas", "bpsk", 2.0, None),
        ("costas", "qpsk", 4.0, None),
        ("costas", "psk8", 8.0, None),
        ("dd", "qam16_dvb", 4.0, None),
        ("ml_tanh", "bpsk", 2.0, None),
        ("pilot", "qpsk", 4.0, Some(8usize)),
    ] {
        let x = turned(name, 6000, 0.001, pilot);
        let mut c = carrier(det, name, order);
        if let Some(sp) = pilot {
            set(&mut c, "pilot_spacing", &Value::F64(sp as f64)).unwrap();
            set(&mut c, "bandwidth", &Value::F64(0.05)).unwrap();
        }
        let mut g = fed(
            Format::new(Kind::Symbols, rate),
            Data::Symbols(x.clone()),
            700,
            c,
        );
        let y = iq(run_passes(&mut g, passes(x.len(), 700), "c.out"));
        assert_eq!(y.len(), x.len());
        let worst = y[5000..]
            .iter()
            .map(|z| near_point(*z, name))
            .fold(0f32, f32::max);
        assert!(worst < 0.1, "{det} {name}: {worst}");
        assert_eq!(result(&g, "c", "locked"), Value::Bool(true), "{det} {name}");
        let f = result_f64(&g, "c", "offset_hz");
        assert!((f - 10.0).abs() < 0.5, "{det} {name}: {f} Hz");
        if pilot.is_some() {
            // Pilots fix the absolute phase: they come out on their own point.
            assert!(
                y[5000..]
                    .iter()
                    .step_by(8)
                    .all(|z| (z - Complex32::new(S, S)).norm() < 0.1),
                "{det}"
            );
        }
    }
}

#[test]
fn carrier_sync_returns_the_same_output_for_any_cut_and_refuses_bad_settings() {
    let x = turned("qpsk", 3000, 0.003, None);
    let run = |per: usize| {
        let mut c = CarrierSyncComponent::costas(4, 0.02);
        set(&mut c, "fll", &Value::F64(0.01)).unwrap();
        set(&mut c, "gears", &Value::F64(0.005)).unwrap();
        let mut g = fed(
            Format::new(Kind::Symbols, 1e4),
            Data::Symbols(x.clone()),
            per,
            c,
        );
        iq(run_passes(&mut g, passes(x.len(), per), "c.out"))
    };
    let a = run(3000);
    assert_eq!(a, run(1));
    assert_eq!(a, run(77));
    let mut c = CarrierSyncComponent::default();
    refuses(&mut c, "order", Value::F64(3.0));
    refuses(&mut c, "constellation", Value::Text("qam17".into()));
    refuses(&mut c, "detector", Value::Text("pll".into()));
    refuses(&mut c, "fll", Value::F64(-0.1));
    refuses(&mut c, "pilot_spacing", Value::F64(0.5));
}

// ---------------------------------------------------------------------------------------------------------------------------
// modem.freq_search

fn estimates(g: &Graph, all: &mut Vec<Timed<FreqEstimate>>) {
    if let Some(b) = g.buffer("fs.estimate") {
        if let Data::Items(Some(v)) = &b.data {
            if let Some(v) = v.as_any().downcast_ref::<Vec<Timed<FreqEstimate>>>() {
                all.extend(v.iter().cloned());
            }
        }
    }
}

fn search(
    fs: FreqSearch,
    x: &[Complex32],
    piece: usize,
) -> (Vec<Timed<FreqEstimate>>, Vec<Complex32>) {
    let mut g = one("fs", fs);
    g.configure_input(Format::iq(RATE, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    let (mut est, mut y) = (Vec::new(), Vec::new());
    for p in x.chunks(piece) {
        out.clear();
        g.process(p, &mut out);
        estimates(&g, &mut est);
        if let Data::Iq(v) = &g.buffer("fs.out").unwrap().data {
            y.extend_from_slice(v);
        }
    }
    (est, y)
}

#[test]
fn every_search_method_finds_the_carrier() {
    // QPSK at +1234 Hz: the 4th-power line.
    let mut q = rrc_signal(&qpsk_symbols(4000, 3), 4.0);
    FrequencyOffset::new(RATE, 1234.0, 0.0)
        .unwrap()
        .process(&mut q);
    Awgn::with_power(0.05, 4).unwrap().process(&mut q);
    // A tone at -2100 Hz.
    let mut tone: Vec<Complex32> = (0..16384)
        .map(|n| {
            Complex32::from_polar(
                1.0,
                (-std::f64::consts::TAU * 2100.0 * n as f64 / RATE) as f32,
            )
        })
        .collect();
    Awgn::with_power(0.01, 5).unwrap().process(&mut tone);
    // MSK at 6 kbaud, +700 Hz: lines of x^2 at 2 fc +- Rs / 2.
    let mut msk = Vec::new();
    let mut m = Cpm::msk(8.0).unwrap();
    let mut rng = Rng::new(6);
    let levels: Vec<f32> = (0..2500)
        .map(|_| if rng.bit() == 0 { 1.0 } else { -1.0 })
        .collect();
    m.push(&levels, &mut msk);
    FrequencyOffset::new(RATE, 700.0, 0.0)
        .unwrap()
        .process(&mut msk);

    let cases: [(&str, u32, &[Complex32], f64, f64); 4] = [
        ("power_line", 4, &q, 1234.0, 2.0),
        ("lag", 1, &tone, -2100.0, 2.0),
        ("centroid", 1, &tone, -2100.0, 30.0),
        ("line_pair", 2, &msk, 700.0, 10.0),
    ];
    for (method, mm, x, want, tol) in cases {
        let mut fs = FreqSearch::default();
        set(&mut fs, "method", &Value::Text(method.into())).unwrap();
        set(&mut fs, "m", &Value::F64(mm as f64)).unwrap();
        set(&mut fs, "fft", &Value::F64(4096.0)).unwrap();
        if method == "line_pair" {
            set(&mut fs, "spacing_hz", &Value::F64(6000.0)).unwrap();
        }
        let (est, y) = search(fs, x, 1000);
        assert_eq!(est.len(), x.len() / 4096, "{method}");
        assert_eq!(y.len(), x.len(), "{method}");
        for e in &est {
            assert!(
                (e.value.offset_hz - want).abs() < tol,
                "{method}: {} Hz",
                e.value.offset_hz
            );
            assert_eq!(e.time, e.value.start as f64 / RATE);
        }
        if method == "line_pair" {
            assert!((est[0].value.spacing_hz.unwrap() - 6000.0).abs() < 20.0);
        }
        // From the second block on, the output has the carrier at 0 Hz.
        if method == "lag" {
            let rest = &y[4096 * 2..4096 * 3];
            let f = sdrkit_algo_modem::freq::kay(rest).unwrap() * RATE;
            assert!(f.abs() < 2.0, "shifted tone at {f} Hz");
        }
    }
}

#[test]
fn freq_search_returns_the_same_output_for_any_cut_and_refuses_bad_settings() {
    let mut q = rrc_signal(&qpsk_symbols(3000, 9), 4.0);
    FrequencyOffset::new(RATE, -800.0, 0.0)
        .unwrap()
        .process(&mut q);
    let (e1, y1) = search(FreqSearch::default(), &q, 12_000);
    let (e2, y2) = search(FreqSearch::default(), &q, 333);
    assert_eq!(e1, e2);
    assert_eq!(y1, y2);
    let mut f = FreqSearch::default();
    refuses(&mut f, "fft", Value::F64(1000.0));
    refuses(&mut f, "method", Value::Text("music".into()));
    refuses(&mut f, "m", Value::F64(0.0));
    // A span beyond the unambiguous range for m = 4 is refused at configure.
    set(&mut f, "span_hz", &Value::F64(10_000.0)).unwrap();
    assert!(one("fs", f)
        .configure_input(Format::iq(RATE, 0.0, 0.0))
        .is_err());
}

// ---------------------------------------------------------------------------------------------------------------------------
// modem.slicer

fn slice(s: Slicer, syms: &[Complex32], per: usize) -> (Vec<i8>, Vec<Complex32>, Graph) {
    let mut g = fed(
        Format::new(Kind::Symbols, 1000.0),
        Data::Symbols(syms.to_vec()),
        per,
        s,
    );
    let Data::Llr(l) = run_passes(&mut g, passes(syms.len(), per), "c.out") else {
        panic!()
    };
    let d = iq(run_passes(&mut g, passes(syms.len(), per), "c.decisions"));
    (l, d, g)
}

#[test]
fn the_slicer_returns_true_llrs_and_the_bits_sent() {
    // QPSK on its points at noise variance 0.1: each bit's max-log LLR is |2s|^2 / 0.1 = 20 nats, 80 at scale 4.
    let pts = point_set("qpsk").unwrap();
    let bits: Vec<u8> = (0..64).map(|k| ((k * 7 + k / 3) % 3 == 0) as u8).collect();
    let mut syms = Vec::new();
    pts.map_bits(&bits, &mut syms).unwrap();
    let (l, d, g) = slice(Slicer::new(constellation("qpsk")), &syms, 5);
    assert_eq!(g.format("c.out").unwrap().llr_scale, Some(4.0));
    assert_eq!(g.format("c.out").unwrap().rate, 2000.0);
    assert_eq!(l.len(), 64);
    for (b, v) in bits.iter().zip(&l) {
        assert_eq!(*v, if *b == 0 { 80 } else { -80 });
    }
    assert_eq!(d, syms);

    // 16-QAM (DVB map), exact demapping: the hard decisions are the bits sent.
    let pts = point_set("qam16_dvb").unwrap();
    let bits: Vec<u8> = (0..400).map(|k| ((k * 13 + k / 5) % 7 < 3) as u8).collect();
    let mut syms = Vec::new();
    pts.map_bits(&bits, &mut syms).unwrap();
    let mut rng = Rng::new(8);
    let noisy: Vec<Complex32> = syms.iter().map(|z| z + rng.complex_normal(0.002)).collect();
    let mut s = Slicer::new(constellation("qam16_dvb"));
    set(&mut s, "demap", &Value::Text("exact".into())).unwrap();
    set(&mut s, "noise_var", &Value::F64(0.002)).unwrap();
    let (l, _, _) = slice(s, &noisy, 33);
    let hard: Vec<u8> = l.iter().map(|&v| (v < 0) as u8).collect();
    assert_eq!(hard, bits);
}

#[test]
fn the_slicer_detects_differential_steps_takes_noise_from_an_input_and_erases_bad_symbols() {
    // DBPSK: 1 turns the phase by pi.
    let bits = [0u8, 1, 1, 0, 1, 0, 0, 0, 1];
    let mut z = Complex32::new(0.0, 1.0);
    let mut syms = vec![z];
    for b in bits {
        if b == 1 {
            z = -z;
        }
        syms.push(z);
    }
    let mut s = Slicer::default();
    set(&mut s, "differential", &Value::Text("dbpsk".into())).unwrap();
    let (l, _, g) = slice(s, &syms, 4);
    assert_eq!(g.format("c.out").unwrap().rate, 1000.0);
    let hard: Vec<u8> = l[1..].iter().map(|&v| (v < 0) as u8).collect();
    assert_eq!(hard, bits);

    // Noise variance from a second input: 0.2 halves the LLRs of the setting's 0.1.
    let pts = vec![
        Complex32::new(S, S),
        Complex32::new(-S, S),
        Complex32::new(f32::NAN, 0.0),
    ];
    let mut g = Tb::new()
        .add(
            "src",
            Source::new(Format::new(Kind::Symbols, 1000.0), Data::Symbols(pts), 3),
        )
        .add(
            "nv",
            Source::new(
                Format::new(Kind::Real, 1000.0),
                Data::Real(vec![0.2, 0.2, 0.2]),
                3,
            ),
        )
        .add("c", Slicer::new(constellation("qpsk")))
        .connect("src", "c.in")
        .connect("nv", "c.noise")
        .build()
        .unwrap();
    let Data::Llr(l) = run_passes(&mut g, 2, "c.out") else {
        panic!()
    };
    assert_eq!(l, vec![40, 40, -40, 40, 0, 0]);

    let mut s = Slicer::default();
    refuses(&mut s, "demap", Value::Text("soft".into()));
    refuses(&mut s, "differential", Value::Text("d16psk".into()));
    refuses(&mut s, "constellation", Value::Text("qam3".into()));
    refuses(&mut s, "llr_scale", Value::F64(0.0));
    refuses(&mut s, "noise_var", Value::F64(-1.0));
}

#[test]
fn the_slicer_returns_the_same_output_for_any_cut() {
    let syms = qpsk_symbols(500, 12);
    let mut s = Slicer::default();
    set(&mut s, "differential", &Value::Text("dqpsk".into())).unwrap();
    let (a, da, _) = slice(s, &syms, 500);
    let mut s = Slicer::default();
    set(&mut s, "differential", &Value::Text("dqpsk".into())).unwrap();
    let (b, db, _) = slice(s, &syms, 7);
    assert_eq!(a, b);
    assert_eq!(da, db);
}

// ---------------------------------------------------------------------------------------------------------------------------
// modem.equaliser

fn through_channel(syms: &[Complex32], h: &[Complex32]) -> Vec<Complex32> {
    (0..syms.len())
        .map(|k| {
            (0..h.len())
                .filter(|&j| j <= k)
                .map(|j| syms[k - j] * h[j])
                .sum()
        })
        .collect()
}

#[test]
fn the_equaliser_undoes_a_two_path_channel() {
    let syms = qpsk_symbols(6000, 13);
    let x = through_channel(
        &syms,
        &[Complex32::new(1.0, 0.0), Complex32::new(0.35, 0.2)],
    );
    for (update, mu) in [("nlms", 0.05), ("dd_lms", 0.005), ("cma", 0.005)] {
        let run = |per: usize| {
            let mut e = Equaliser::default();
            set(&mut e, "update", &Value::Text(update.into())).unwrap();
            set(&mut e, "mu", &Value::F64(mu)).unwrap();
            let mut g = fed(
                Format::new(Kind::Symbols, 1000.0),
                Data::Symbols(x.clone()),
                per,
                e,
            );
            let y = iq(run_passes(&mut g, passes(x.len(), per), "c.out"));
            let mse = result_f64(&g, "c", "mse_db");
            (y, mse)
        };
        let (y, mse) = run(1000);
        assert_eq!(y.len(), x.len());
        if update == "cma" {
            // Blind: the modulus is restored (the phase is left to a carrier loop).
            let worst = y[5000..]
                .iter()
                .map(|z| (z.norm() - 1.0).abs())
                .fold(0f32, f32::max);
            assert!(worst < 0.15, "{update}: {worst}");
        } else {
            let worst = y[5000..]
                .iter()
                .map(|z| near_point(*z, "qpsk"))
                .fold(0f32, f32::max);
            assert!(worst < 0.1, "{update}: {worst}");
            assert!(mse < -20.0, "{update}: {mse} dB");
        }
        assert_eq!(y, run(37).0, "{update}");
    }
    let mut e = Equaliser::default();
    refuses(&mut e, "update", Value::Text("rls".into()));
    refuses(&mut e, "sps", Value::F64(4.0));
    refuses(&mut e, "taps", Value::F64(0.0));
    refuses(&mut e, "mu", Value::F64(2.0));
}

// ---------------------------------------------------------------------------------------------------------------------------
// modem.snr

#[test]
fn the_snr_meter_reads_the_snr_mer_and_evm() {
    // QPSK at Es/N0 = 10 dB: noise variance 0.1, EVM sqrt(0.1) = 31.6 %.
    let mut rng = Rng::new(14);
    let x: Vec<Complex32> = qpsk_symbols(20_000, 15)
        .into_iter()
        .map(|z| z + rng.complex_normal(0.1))
        .collect();
    for method in ["m2m4", "dd"] {
        let run = |per: usize| {
            let mut s = SnrMeter::default();
            set(&mut s, "method", &Value::Text(method.into())).unwrap();
            set(&mut s, "interval_s", &Value::F64(1.0)).unwrap();
            let mut g = fed(
                Format::new(Kind::Symbols, 10_000.0),
                Data::Symbols(x.clone()),
                per,
                s,
            );
            run_passes(&mut g, passes(x.len(), per), "src.out");
            [
                result_f64(&g, "c", "snr_db"),
                result_f64(&g, "c", "mer_db"),
                result_f64(&g, "c", "evm_pct"),
            ]
        };
        let r = run(1000);
        let [snr, mer, evm] = r;
        assert!((snr - 10.0).abs() < 0.4, "{method}: snr {snr}");
        assert!((mer - 10.0).abs() < 0.6, "{method}: mer {mer}");
        assert!((evm - 31.6).abs() < 2.0, "{method}: evm {evm}");
        assert_eq!(r, run(29), "{method}");
    }
    let mut s = SnrMeter::default();
    refuses(&mut s, "method", Value::Text("guess".into()));
    refuses(&mut s, "interval_s", Value::F64(0.0));
}

// ---------------------------------------------------------------------------------------------------------------------------
// modem.squelch

fn lock_tags(g: &mut Graph, per: usize, port: &str, n: usize) -> (Data, Vec<(u64, bool)>) {
    g.configure_input(Format::iq(1.0, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    let empty: [Complex32; 0] = [];
    let mut tags = Vec::new();
    let mut all: Option<Data> = None;
    for _ in 0..passes(n, per) {
        out.clear();
        g.process(&empty, &mut out);
        let b = g.buffer(port).unwrap();
        tags.extend(b.tags().iter().filter_map(|t| match t.kind {
            TagKind::Lock { locked } => Some((t.at, locked)),
            _ => None,
        }));
        let d = all.get_or_insert_with(|| g.format(port).unwrap().kind.empty_data());
        sdrkit_components::testing::extend_range(d, &b.data, 0, b.data.len());
    }
    (all.unwrap(), tags)
}

#[test]
fn the_power_and_snr_squelch_open_on_a_signal_and_mark_it() {
    let mut rng = Rng::new(16);
    let mut x: Vec<Complex32> = (0..12_000).map(|_| rng.complex_normal(1e-6)).collect();
    for v in &mut x[4000..8000] {
        *v += Complex32::new(0.1, 0.0);
    }
    for (mode, thr) in [("power", -40.0), ("snr", 20.0)] {
        let run = |per: usize| {
            let s = squelch(mode, thr).unwrap();
            let mut g = fed(Format::iq(RATE, 0.0, 0.0), Data::Iq(x.clone()), per, s);
            lock_tags(&mut g, per, "c.out", x.len())
        };
        let (y, tags) = run(1000);
        let y = iq(y);
        assert_eq!(tags.len(), 2, "{mode}: {tags:?}");
        assert!(
            tags[0].1 && tags[0].0 >= 4000 && tags[0].0 < 4000 + 512,
            "{mode}: {tags:?}"
        );
        assert!(
            !tags[1].1 && tags[1].0 >= 8000 && tags[1].0 < 8000 + 6 * 256,
            "{mode}: {tags:?}"
        );
        assert!(
            y[..4000].iter().all(|z| *z == Complex32::default()),
            "{mode}"
        );
        assert!(y[5000..7900].iter().all(|z| z.re > 0.09), "{mode}");
        assert!(
            y[10_000..].iter().all(|z| *z == Complex32::default()),
            "{mode}"
        );
        let (y2, tags2) = run(77);
        assert_eq!(y, iq(y2), "{mode}");
        assert_eq!(tags, tags2, "{mode}");
    }
}

#[test]
fn the_quieting_squelch_opens_on_a_quiet_discriminator() {
    let rate = 48_000.0;
    let mut rng = Rng::new(17);
    // Discriminator noise with no carrier (white, strong above the voice band), then a carrier: a 1 kHz tone, little noise.
    let mut x: Vec<f32> = (0..24_000).map(|_| 0.5 * rng.normal_f32()).collect();
    x.extend((0..24_000).map(|n| {
        0.3 * (std::f64::consts::TAU * 1000.0 * n as f64 / rate).sin() as f32
            + 0.002 * rng.normal_f32()
    }));
    let s = squelch("quieting", 10.0).unwrap().real();
    let mut g = fed(
        Format::new(Kind::Real, rate),
        Data::Real(x.clone()),
        1000,
        s,
    );
    let (y, tags) = lock_tags(&mut g, 1000, "c.out", x.len());
    let Data::Real(y) = y else { panic!() };
    assert_eq!(tags.len(), 1, "{tags:?}");
    assert!(
        tags[0].1 && tags[0].0 >= 24_000 && tags[0].0 < 24_000 + 1024,
        "{tags:?}"
    );
    assert!(y[..24_000].iter().all(|v| *v == 0.0));
    assert_eq!(&y[30_000..], &x[30_000..]);
    let q = result_f64(&g, "c", "quieting_db");
    assert!(q > 20.0, "{q}");

    // On Iq the quieting mode is refused at configure.
    let mut g = one("sq", squelch("quieting", 10.0).unwrap());
    assert!(g.configure_input(Format::iq(rate, 0.0, 0.0)).is_err());
    let mut s = SquelchComponent::default();
    refuses(&mut s, "mode", Value::Text("tone".into()));
    refuses(&mut s, "block", Value::F64(0.0));
    refuses(&mut s, "noise_hz", Value::F64(100.0));
    assert!(squelch("tone", 0.0).is_err());
}

// ---------------------------------------------------------------------------------------------------------------------------
// modem.burst_detect

fn bursts_of(x: &[Complex32], piece: usize) -> Vec<Timed<IqBurst>> {
    let mut g = one("bd", BurstDetect::default());
    g.configure_input(Format::iq(RATE, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    let mut all = Vec::new();
    for p in x.chunks(piece) {
        out.clear();
        g.process(p, &mut out);
        if let Data::Items(Some(v)) = &g.buffer("bd.out").unwrap().data {
            all.extend(
                v.as_any()
                    .downcast_ref::<Vec<Timed<IqBurst>>>()
                    .unwrap()
                    .iter()
                    .cloned(),
            );
        }
    }
    all
}

#[test]
fn the_burst_detector_hands_on_each_burst_with_its_start_and_frequency() {
    let mut rng = Rng::new(18);
    let mut x: Vec<Complex32> = (0..60_000).map(|_| rng.complex_normal(1e-4)).collect();
    let bursts = [
        (5_000usize, 2_000usize, 1500.0f64),
        (20_000, 800, -3000.0),
        (41_000, 5_000, 250.0),
    ];
    for &(at, len, f) in &bursts {
        for (k, v) in x[at..at + len].iter_mut().enumerate() {
            *v += Complex32::from_polar(0.1, (std::f64::consts::TAU * f * k as f64 / RATE) as f32);
        }
    }
    let found = bursts_of(&x, 4096);
    assert_eq!(found.len(), 3);
    for (b, &(at, len, f)) in found.iter().zip(&bursts) {
        let v = &b.value;
        assert!(
            v.start <= at as u64 && v.start + 200 >= at as u64,
            "start {} for {at}",
            v.start
        );
        assert!(
            v.start + v.samples.len() as u64 >= (at + len) as u64,
            "{} samples",
            v.samples.len()
        );
        assert!((v.freq_hz - f).abs() < 50.0, "{} Hz for {f}", v.freq_hz);
        assert!(v.snr_db > 15.0 && !v.truncated);
        assert_eq!(b.time, v.start as f64 / RATE);
        assert_eq!(v.rate_hz, RATE);
    }
    let again = bursts_of(&x, 333);
    assert_eq!(found.len(), again.len());
    for (a, b) in found.iter().zip(&again) {
        assert_eq!(
            (a.value.start, a.value.samples.as_slice()),
            (b.value.start, b.value.samples.as_slice())
        );
    }
    let mut d = BurstDetect::default();
    refuses(&mut d, "floor", Value::Text("median".into()));
    refuses(&mut d, "pool", Value::F64(0.0));
    refuses(&mut d, "window", Value::F64(1.5));
    set(&mut d, "close_db", &Value::F64(20.0)).unwrap();
    assert!(one("bd", d)
        .configure_input(Format::iq(RATE, 0.0, 0.0))
        .is_err());
}

// ---------------------------------------------------------------------------------------------------------------------------
// The registry

#[test]
fn every_component_is_registered_under_its_own_name() {
    let r = sdrkit_components::registry();
    for n in [
        "modem.matched_filter",
        "modem.symbol_sync",
        "modem.carrier_sync",
        "modem.freq_search",
        "modem.slicer",
        "modem.equaliser",
        "modem.snr",
        "modem.squelch",
        "modem.burst_detect",
    ] {
        let c = r
            .make(n)
            .unwrap_or_else(|_| panic!("{n} is not registered"));
        assert_eq!(c.info().name, n);
        // Every setting the schema lists is one the component takes, with its default.
        let schema = c.info().settings;
        for s in schema.settings() {
            let mut c = r.make(n).unwrap();
            let id = schema.id(&s.field.key).unwrap();
            assert!(
                c.apply(id, &s.default).is_ok(),
                "{n}.{} refuses its default",
                s.field.key
            );
        }
    }
}
