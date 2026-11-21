//! The classic chain for a linear modulation, end to end: an RRC QPSK signal from `sdrkit-algo-sim` with a carrier offset, a clock
//! error and noise, through `modem.matched_filter`, `modem.symbol_sync`, `modem.carrier_sync` and `modem.slicer`, returns the
//! transmitted bits exactly once the loops have locked. The QPSK phase ambiguity the carrier loop leaves is removed by differential
//! coding (DQPSK steps on QPSK points, detected by the slicer's `differential`).

mod set_util;
use set_util::{gset, result, result_f64, txt, Tb};

use sdrkit_algo_modem::differential::Differential;
use sdrkit_algo_sim::clock::ClockError;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_algo_sim::offset::FrequencyOffset;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_components::modem::sync::points::ConstellationName;
use sdrkit_components::modem::sync::{
    CarrierSyncComponent, MatchedFilter, Slicer, SnrMeter, SymbolSyncComponent,
};
use sdrkit_components::testing::run_iq;
use sdrkit_core::{Complex32, Output, Receiver, Value};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Format, Graph, TagKind};

const RATE: f64 = 48_000.0;
const SPS: f64 = 4.0;
const OFFSET_HZ: f64 = 150.0;
const CLOCK_PPM: f64 = 400.0;
const SYMBOLS: usize = 20_000;

/// The transmitted bits (two per symbol after the first, which only sets the phase) and the impaired signal.
fn signal(seed: u64) -> (Vec<u8>, Vec<Complex32>) {
    let mut rng = Rng::new(seed);
    let steps = Differential::dqpsk();
    let pts = steps.steps().points().to_vec();
    let mut bits = vec![0u8; 2 * (SYMBOLS - 1)];
    rng.fill_bits(&mut bits);
    let mut z = Complex32::from_polar(1.0, std::f32::consts::FRAC_PI_4);
    let mut syms = vec![z];
    for b in bits.chunks(2) {
        z *= pts[((b[0] << 1) | b[1]) as usize];
        syms.push(z);
    }
    let mut tx = Linear::rrc(0.35, 16.0, SPS).unwrap();
    let mut x = Vec::new();
    tx.push(&syms, &mut x);
    tx.flush(&mut x);
    let mut y = Vec::new();
    ClockError::new(CLOCK_PPM).unwrap().process(&x, &mut y);
    FrequencyOffset::new(RATE, OFFSET_HZ, 0.0)
        .unwrap()
        .process(&mut y);
    // Es/N0 = SPS / noise power: 17 dB.
    Awgn::with_power(SPS / 50.0, seed + 1)
        .unwrap()
        .process(&mut y);
    (bits, y)
}

fn chain() -> Graph {
    Tb::new()
        .add("mf", MatchedFilter::rrc(0.35, SPS, 2).unwrap())
        .add("ss", SymbolSyncComponent::new(SPS / 2.0).unwrap())
        .add("cs", CarrierSyncComponent::costas(4, 0.02))
        .add("sl", Slicer::new(ConstellationName::Qpsk))
        .add("snr", SnrMeter::default())
        .input("mf")
        .connect("mf", "ss")
        .connect("ss.out", "cs")
        .connect("cs", "sl")
        .connect("cs", "snr")
        .set("cs.fll", Value::F64(0.005))
        .set("ss.bandwidth", Value::F64(0.002))
        .set("sl.differential", txt("dqpsk"))
        .finish("sl.out")
}

fn hard(llr: &[i8]) -> Vec<u8> {
    llr.iter().map(|&l| (l < 0) as u8).collect()
}

/// The shift `s` (even, in bits) with `rx[i] == tx[i - s]` over a window well after lock.
fn alignment(rx: &[u8], tx: &[u8], from: usize, len: usize) -> Option<isize> {
    (-40isize..=40).step_by(2).find(|&s| {
        (from..from + len).all(|i| {
            let j = i as isize - s;
            j >= 0 && (j as usize) < tx.len() && rx[i] == tx[j as usize]
        })
    })
}

#[test]
fn the_chain_recovers_every_bit_after_lock() {
    let (bits, x) = signal(7);
    let mut g = chain();
    let Data::Llr(llr) = run_iq(&mut g, Format::iq(RATE, 0.0, 0.0), &x, 4096, "sl.out") else {
        panic!()
    };
    assert_eq!(g.format("sl.out").unwrap().llr_scale, Some(4.0));
    let rx = hard(&llr);
    // Lock within the first 1500 symbols; check every bit from there to the last few symbols.
    let lock = 2 * 1500;
    let s = alignment(&rx, &bits, 2 * 3000, 400).expect("the bits line up after lock");
    let end = (bits.len() as isize + s).min(rx.len() as isize) as usize - 2 * 20;
    let errors = (lock..end)
        .filter(|&i| rx[i] != bits[(i as isize - s) as usize])
        .count();
    assert_eq!(errors, 0, "bit errors after lock (shift {s})");
    assert!(end - lock > 2 * 6000, "checked {} bits", end - lock);

    let ppm = result_f64(&g, "ss", "clock_ppm");
    // The integrator holds the clock error, under the loop's own noise (a narrow loop keeps that small).
    assert!((ppm - CLOCK_PPM).abs() < 60.0, "clock {ppm} ppm");
    assert_eq!(result(&g, "ss", "locked"), Value::Bool(true));
    let f = result_f64(&g, "cs", "offset_hz");
    assert!((f - OFFSET_HZ).abs() < 5.0, "offset {f} Hz");
    assert_eq!(result(&g, "cs", "locked"), Value::Bool(true));
    let snr = result_f64(&g, "snr", "snr_db");
    assert!(snr > 13.0 && snr < 19.0, "snr {snr} dB");
}

#[test]
fn any_cut_of_the_input_returns_the_same_soft_bits() {
    let (_, x) = signal(11);
    let x = &x[..40_000];
    let run = |piece: usize| {
        let mut g = chain();
        match run_iq(&mut g, Format::iq(RATE, 0.0, 0.0), x, piece, "sl.out") {
            Data::Llr(v) => v,
            _ => panic!(),
        }
    };
    let a = run(4096);
    assert!(a.len() > 19_000, "{}", a.len());
    assert_eq!(a, run(1));
    assert_eq!(a, run(333));
    assert_eq!(a, run(40_000));
}

/// Runs `x` in pieces and gathers the tags of `port`.
fn tags(g: &mut Graph, x: &[Complex32], port: &str) -> Vec<(u64, TagKind)> {
    g.configure_input(Format::iq(RATE, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    let mut v = Vec::new();
    for (k, p) in x.chunks(4096).enumerate() {
        if k == 6 {
            Receiver::retuned(g, &sdrkit_core::InputInfo::new(RATE, 1000.0, 1000.0));
        }
        out.clear();
        g.process(p, &mut out);
        v.extend(
            g.buffer(port)
                .unwrap()
                .tags()
                .iter()
                .map(|t| (t.at, t.kind.clone())),
        );
    }
    v
}

#[test]
fn the_synchronisers_tag_lock_and_reset_on_a_retune() {
    let (_, x) = signal(3);
    let mut g = chain();
    let ss = tags(&mut g, &x[..60_000], "ss.out");
    let cs_tags = {
        let mut g = chain();
        tags(&mut g, &x[..60_000], "cs.out")
    };
    for (name, t) in [("symbol sync", &ss), ("carrier sync", &cs_tags)] {
        let lock: Vec<_> = t
            .iter()
            .filter(|(_, k)| matches!(k, TagKind::Lock { .. }))
            .collect();
        // Locked during the first 6 pieces, unlocked by the retune, locked again after it.
        assert!(lock.len() >= 3, "{name}: {lock:?}");
        assert_eq!(lock[0].1, TagKind::Lock { locked: true }, "{name}");
        let retune = t
            .iter()
            .find(|(_, k)| matches!(k, TagKind::Retuned { .. }))
            .expect("the retune is passed on");
        let unlock = lock
            .iter()
            .find(|(at, k)| *k == TagKind::Lock { locked: false } && *at >= retune.0);
        assert!(
            unlock.is_some_and(|u| u.0 == retune.0),
            "{name}: unlocked at the retune: {t:?}"
        );
        assert!(
            lock.last().unwrap().1 == TagKind::Lock { locked: true },
            "{name}: locked again"
        );
        // The retune lands on the symbol made after input sample 6 x 4096, one per SPS samples.
        let expect = 6.0 * 4096.0 / SPS;
        assert!(
            (retune.0 as f64 - expect).abs() < 20.0,
            "{name}: retune at {}",
            retune.0
        );
    }
    assert!(gset(&mut g, "ss.detector", &Value::Text("nope".into())).is_err());
}
