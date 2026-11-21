//! A change of frequency: each transmitter test, after its test signal, forgets its measurement and its tables.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_components::sim::impaired;
use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, Owner, StackDescriptor, Value};
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter as meter;

/// The stacks, a measurement field of each, the rate and the seconds of signal it needs.
fn stacks() -> [(&'static StackDescriptor, &'static str, f64, f64); 10] {
    [
        (&meter::freq_accuracy::DESCRIPTOR, "offset_hz", 48e3, 6.0),
        (&meter::phase_noise::DESCRIPTOR, "l_1khz", 250e3, 7.0),
        (&meter::mer::DESCRIPTOR, "mer_db", 250e3, 5.0),
        (&meter::ssb_test::DESCRIPTOR, "tone1_hz", 48e3, 6.0),
        (&meter::sinad::DESCRIPTOR, "sinad_db", 48e3, 4.0),
        (&meter::nfm_meter::DESCRIPTOR, "ctcss_hz", 48e3, 4.0),
        (&meter::afsk_tuner::DESCRIPTOR, "mark_hz", 48e3, 4.0),
        (&meter::tx_stability::DESCRIPTOR, "final_hz", 48e3, 12.0),
        (&meter::cw_keying::DESCRIPTOR, "wpm", 48e3, 20.0),
        (
            &meter::fsk_analyser::DESCRIPTOR,
            "symbol_rate_bd",
            250e3,
            5.0,
        ),
    ]
}

fn has(r: &sdrkit_core::Results, key: &str) -> bool {
    r.decl()
        .field_id(key)
        .and_then(|id| r.fields().value(id))
        .is_some_and(|v| *v != Value::None)
}

fn good(r: &sdrkit_core::Results) -> i64 {
    match r
        .decl()
        .field_id("good")
        .and_then(|id| r.fields().value(id))
    {
        Some(Value::I64(n)) => *n,
        _ => -1,
    }
}

#[test]
fn a_change_of_frequency_forgets_the_measurement_and_the_tables() {
    for (d, key, rate, secs) in stacks() {
        let h = Harness::new(d)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, 30.0);
        let values = h.values().unwrap();
        let g = (d.make_generator.unwrap())(&GenConfig::new(rate, values.clone()).with_seed(1))
            .unwrap();
        let mut g = impaired(g, &values).unwrap();
        let mut rx = h.receiver().unwrap();
        rx.configure(&InputInfo::new(rate, 10e6, 10e6).recording())
            .unwrap();
        let mut x = vec![Complex32::default(); (rate / 4.0) as usize];
        let mut out = Output::default();
        for _ in 0..(secs * 4.0) as usize {
            let n = g.generate(&mut x);
            out.clear();
            rx.feed(&x[..n], &mut out);
        }
        let before = Owner::results(rx.as_ref()).clone();
        assert!(
            has(&before, key) && good(&before) > 0,
            "{}: {key} before the change",
            d.id
        );
        rx.retuned(&InputInfo::new(rate, 20e6, 20e6).recording());
        rx.forget();
        out.clear();
        rx.feed(&vec![Complex32::default(); (rate / 2.0) as usize], &mut out);
        let after = Owner::results(rx.as_ref()).clone();
        assert!(!has(&after, key), "{}: {key} kept over the change", d.id);
        assert_eq!(good(&after), 0, "{}: the tally kept over the change", d.id);
        let rows: usize = after
            .decl()
            .tables()
            .iter()
            .filter_map(|t| after.decl().table_id(&t.key).and_then(|id| after.table(id)))
            .map(|t| t.len())
            .sum();
        assert_eq!(rows, 0, "{}: rows kept over the change", d.id);
    }
}

/// Runs `secs` of the test signal of `h` at `rate` with an SNR of 30 dB in its bandwidth, then retunes and feeds zeros:
/// the results before and after.
fn retune_run(
    d: &'static StackDescriptor,
    h: Harness,
    rate: f64,
    secs: f64,
) -> (sdrkit_core::Results, sdrkit_core::Results) {
    let h = h.set(&GEN_NOISE, NoiseKind::Snr).set(&GEN_SNR_DB, 30.0);
    let values = h.values().unwrap();
    let g = (d.make_generator.unwrap())(&GenConfig::new(rate, values.clone()).with_seed(1)).unwrap();
    let mut g = impaired(g, &values).unwrap();
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(rate, 10e6, 10e6).recording()).unwrap();
    let mut x = vec![Complex32::default(); (rate / 100.0) as usize];
    let mut out = Output::default();
    for _ in 0..(secs * 100.0) as usize {
        let n = g.generate(&mut x);
        out.clear();
        rx.feed(&x[..n], &mut out);
    }
    let before = Owner::results(rx.as_ref()).clone();
    rx.retuned(&InputInfo::new(rate, 20e6, 20e6).recording());
    rx.forget();
    out.clear();
    rx.feed(&vec![Complex32::default(); (rate / 2.0) as usize], &mut out);
    (before, Owner::results(rx.as_ref()).clone())
}

fn rows(r: &sdrkit_core::Results) -> usize {
    r.decl()
        .tables()
        .iter()
        .filter_map(|t| r.decl().table_id(&t.key).and_then(|id| r.table(id)))
        .map(|t| t.len())
        .sum()
}

#[test]
fn the_interference_and_health_stacks_forget_and_glitch_keeps_its_log() {
    use meter::{beacon_monitor, fading, input_health, intermod, kurtosis, noise_figure, papr, spur};
    let cases: [(&'static StackDescriptor, Harness, &str, f64, f64); 8] = [
        (&papr::DESCRIPTOR, Harness::new(&papr::DESCRIPTOR), "ccdf_1_db", 1e6, 2.0),
        (&input_health::DESCRIPTOR, Harness::new(&input_health::DESCRIPTOR), "rms_dbfs", 250e3, 2.0),
        (&noise_figure::DESCRIPTOR, Harness::new(&noise_figure::DESCRIPTOR), "y_db", 250e3, 25.0),
        (&kurtosis::DESCRIPTOR, Harness::new(&kurtosis::DESCRIPTOR).set(&kurtosis::gen::GEN_TONES, ""), "clean_pct", 250e3, 9.0),
        (&intermod::DESCRIPTOR, Harness::new(&intermod::DESCRIPTOR), "products", 1e6, 30.0),
        (&spur::DESCRIPTOR, Harness::new(&spur::DESCRIPTOR), "dc_db", 1e6, 30.0),
        (&fading::DESCRIPTOR, Harness::new(&fading::DESCRIPTOR), "s4", 16e3, 40.0),
        (&beacon_monitor::DESCRIPTOR, Harness::new(&beacon_monitor::DESCRIPTOR), "level_db", 250e3, 20.0),
    ];
    for (d, h, key, rate, secs) in cases {
        let id = d.id;
        let (before, after) = retune_run(d, h, rate, secs);
        assert!(has(&before, key) && good(&before) > 0, "{id}: {key} before the change");
        assert!(!has(&after, key), "{id}: {key} kept over the change");
        assert_eq!(good(&after), 0, "{id}: the tally kept over the change");
        // The noise figure table lists the ENR values of the setting; after the change it holds no noise figure.
        let nf = &sdrkit_stack_meter::typed::NOISE_FIGURE_TABLE;
        let listed = nf.rows::<sdrkit_stack_meter::typed::NoiseFigureRow>(&after).map(|(_, r)| r).collect::<Vec<_>>();
        assert!(listed.iter().all(|r| r.nf_db.is_none()), "{id}: a noise figure kept over the change");
        assert_eq!(rows(&after) - listed.len(), 0, "{id}: rows kept over the change");
    }
    let (before, after) = retune_run(&meter::glitch::DESCRIPTOR, Harness::new(&meter::glitch::DESCRIPTOR), 2e6, 2.0);
    let drops = |r: &sdrkit_core::Results| match r.decl().field_id("sample_drops").and_then(|id| r.fields().value(id)) {
        Some(Value::I64(n)) => *n,
        _ => -1,
    };
    assert!(has(&before, "tone_hz") && drops(&before) == 2, "glitch before the change");
    assert!(!has(&after, "tone_hz") && good(&after) == 0, "glitch: the tone kept over the change");
    assert!(drops(&after) == 2 && rows(&after) >= rows(&before) && rows(&before) >= 2, "glitch: the log or the totals lost");
}
