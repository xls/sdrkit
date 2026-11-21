//! `modem.ofdm_sync` and `modem.ofdm_demod`: 802.11a-layout symbols built from the plain crate's own layout table (data cells
//! QPSK, pilots from `OfdmLayout::pilots`, IFFT, cyclic prefix) with a carrier offset of 1.3 carriers, found, corrected and
//! equalised back to the exact cells sent; cuts; strict settings.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_modem::ofdm::OfdmLayout;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_components::modem::demod::{OfdmCells, OfdmDemod, OfdmSync};
use sdrkit_components::testing::run_iq;
use sdrkit_core::{Complex32, Value};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{components::NullSink, Applied, Format, Graph, TagKind};

#[path = "modem_demod_util.rs"]
mod util;
use util::run_items;
#[allow(unused_imports)]
use util::set_util::{set, txt, Tb};

const RATE: f64 = 20e6;

/// `symbols` symbols of the 802.11a layout after 300 samples of quiet, at a carrier offset of `offset` carriers, in light noise.
/// Returns the samples and the QPSK cells sent on the data carriers of each symbol.
fn signal(symbols: usize, offset: f64, seed: u64) -> (Vec<Complex32>, Vec<Vec<Complex32>>) {
    let layout = OfdmLayout::ieee80211a();
    let n = layout.nfft();
    let mut rng = Rng::new(seed);
    let mut ifft = Fft::inverse(n);
    let mut x = vec![Complex32::default(); 300];
    let mut sent = Vec::new();
    let s = std::f32::consts::FRAC_1_SQRT_2;
    for l in 0..symbols as u64 {
        let mut cells = vec![Complex32::default(); layout.used()];
        let mut data = Vec::new();
        for &i in layout.data(l) {
            let c = Complex32::new(
                if rng.bit() == 0 { s } else { -s },
                if rng.bit() == 0 { s } else { -s },
            );
            cells[i as usize] = c;
            data.push(c);
        }
        for p in layout.pilots(l) {
            cells[p.index as usize] = p.value;
        }
        let mut bins = vec![Complex32::default(); n];
        for (i, c) in cells.iter().enumerate() {
            bins[layout.bin(i)] = *c;
        }
        ifft.process(&mut bins);
        let scale = 1.0 / (n as f32).sqrt();
        let cp = layout.cp(l);
        x.extend(bins[n - cp..].iter().map(|v| v * scale));
        x.extend(bins.iter().map(|v| v * scale));
        sent.push(data);
    }
    x.extend(vec![Complex32::default(); 400]);
    let f = offset / n as f64;
    for (k, v) in x.iter_mut().enumerate() {
        let ph = std::f64::consts::TAU * f * k as f64;
        *v *= Complex32::new(ph.cos() as f32, ph.sin() as f32);
    }
    Awgn::with_power(1e-4, seed + 1).unwrap().process(&mut x);
    (x, sent)
}

fn graph() -> Graph {
    Tb::new()
        .add("sync", OfdmSync::default())
        .add("demod", OfdmDemod::default())
        .add("sink", NullSink::default())
        .input("sync")
        .connect("sync", "demod")
        .connect("demod", "sink")
        .build()
        .unwrap()
}

fn qpsk(z: Complex32) -> (bool, bool) {
    (z.re > 0.0, z.im > 0.0)
}

#[test]
fn symbols_are_found_corrected_and_equalised_to_the_cells_sent() {
    let layout = OfdmLayout::ieee80211a();
    let (x, sent) = signal(30, 1.3, 1);
    let f = Format::iq(RATE, 0.0, 0.0);
    let mut first: Option<Vec<(u64, Vec<Complex32>)>> = None;
    for piece in [4096usize, 333, 1] {
        let mut g = graph();
        let mut out = run_items::<OfdmCells>(&mut g, f.clone(), &x, piece, "demod.out");
        // The thirty sent, then the grid runs on into the quiet after them until the lock is dropped.
        assert!(out.len() >= 30, "piece {piece}: {}", out.len());
        out.truncate(30);
        assert!(out
            .iter()
            .enumerate()
            .all(|(k, t)| t.value.symbol == k as u64));
        for t in &out {
            let c = &t.value;
            let l = c.symbol;
            assert_eq!(c.cells.len(), layout.used());
            let data: Vec<Complex32> = layout
                .data(l)
                .iter()
                .map(|&i| c.cells[i as usize])
                .collect();
            assert_eq!(data.len(), 48);
            for (k, (z, s)) in data.iter().zip(&sent[l as usize]).enumerate() {
                assert_eq!(qpsk(*z), qpsk(*s), "symbol {l} cell {k}: {z} for {s}");
                assert!((z - s).norm() < 0.2, "symbol {l} cell {k}: {z} for {s}");
            }
            assert!(c.weights.iter().all(|w| w.is_finite()));
            assert!(c.noise > 0.0 && c.noise < 0.01);
        }
        let got: Vec<(u64, Vec<Complex32>)> = out
            .iter()
            .map(|t| (t.value.symbol, t.value.cells.to_vec()))
            .collect();
        match &first {
            None => first = Some(got),
            // Any cut produces the same cells.
            Some(r) => assert_eq!(&got, r, "piece {piece}"),
        }
    }
}

#[test]
fn the_sync_marks_the_frame_start_and_lock_and_cuts_whole_symbols() {
    let (x, _) = signal(12, -0.7, 2);
    let mut g = Tb::new()
        .add("sync", OfdmSync::default())
        .add("sink", NullSink::default())
        .input("sync")
        .connect("sync", "sink")
        .build()
        .unwrap();
    g.configure_input(Format::iq(RATE, 0.0, 0.0)).unwrap();
    let mut out = sdrkit_core::Output::default();
    let mut samples = 0usize;
    let mut tags = Vec::new();
    for p in x.chunks(500) {
        out.clear();
        g.process(p, &mut out);
        let b = g.buffer("sync.out").unwrap();
        samples += b.len();
        tags.extend(b.tags().iter().cloned());
    }
    // Whole symbols: the twelve sent, then the grid runs on through the quiet until the lock is dropped.
    assert_eq!(samples % 80, 0);
    assert!(samples >= 12 * 80, "{samples}");
    assert_eq!(tags[0].at, 0);
    // Still locked: the offset found is -0.7 carriers of 312.5 kHz.
    assert_eq!(
        util::set_util::result(&g, "sync", "locked"),
        Value::Bool(true)
    );
    let off = util::set_util::result_f64(&g, "sync", "offset_hz");
    assert!((off + 0.7 * 312_500.0).abs() < 2_000.0, "{off}");
    assert!(tags.iter().any(|t| t.at == 0
        && matches!(
            t.kind,
            TagKind::FrameStart {
                inverted: false,
                errors: 0
            }
        )));
    assert!(tags
        .iter()
        .any(|t| t.at == 0 && t.kind == TagKind::Lock { locked: true }));
    // After the last symbol the lock is lost once 8 symbol times pass with no prefix.
    let mut more = vec![Complex32::default(); 2000];
    Awgn::with_power(1e-4, 5).unwrap().process(&mut more);
    out.clear();
    g.process(&more, &mut out);
    let b = g.buffer("sync.out").unwrap();
    assert!(b
        .tags()
        .iter()
        .any(|t| t.kind == TagKind::Lock { locked: false }));
    // The output is the input at the same rate.
    assert_eq!(g.format("sync.out").unwrap().rate, RATE);
    assert!(
        matches!(run_iq(&mut g, Format::iq(RATE, 0.0, 0.0), &x, 4096, "sync.out"), Data::Iq(v) if v.len() == samples)
    );
}

#[test]
fn settings_are_strict() {
    let mut s = OfdmSync::default();
    assert_eq!(
        set(&mut s, "layout", &Value::Text("dvbt_2k".into())),
        Ok(Applied::Reformat)
    );
    assert!(set(&mut s, "layout", &Value::Text("dab".into())).is_err());
    assert!(set(&mut s, "guard", &Value::F64(5.0)).is_err());
    assert!(set(&mut s, "guard", &Value::F64(8.0)).is_ok());
    assert!(set(&mut s, "threshold", &Value::F64(0.0)).is_err());
    assert!(set(&mut s, "max_shift", &Value::F64(-1.0)).is_err());
    assert!(set(&mut s, "colour", &Value::Bool(true)).is_err());
    let mut d = OfdmDemod::default();
    assert!(set(&mut d, "estimator", &Value::Text("magic".into())).is_err());
    assert!(set(&mut d, "estimator", &Value::Text("time_freq".into())).is_ok());
    assert!(set(&mut d, "backoff", &Value::F64(1.5)).is_err());
    // A backoff longer than the prefix is refused when the graph configures.
    let mut g = Tb::new()
        .add("demod", OfdmDemod::default())
        .add("sink", NullSink::default())
        .input("demod")
        .connect("demod", "sink")
        .set("demod.backoff", Value::F64(40.0))
        .build()
        .unwrap();
    assert!(g.configure_input(Format::iq(RATE, 0.0, 0.0)).is_err());
}
