//! Meteosat HRDCP: the message layer, the receiver on its test signal, two platforms 3 kHz apart, impairments, noise, the
//! channel plan file, saved state and settings.

use crate::common::{cell, cell_num, command, field_text, rows, set, values};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{
    Complex32, Event, Generator, InputInfo, LockState, Output, Receiver, StackConfig,
};
use sdrkit_stack_satcom::modes::hrdcp::{self, gen, ChannelPlan};
use serde_json::json;

const DIAL: f64 = 402.3e6;

/// The scene `platforms` at `rate` for `seconds`, with noise at `ebn0_db` and the carrier moved by `offset_hz`.
fn signal(
    platforms: Vec<gen::Platform>,
    rate: f64,
    seconds: f64,
    ebn0_db: Option<f64>,
    extra: &[CaseSetting],
) -> Vec<Complex32> {
    let g = gen::HrdcpGenerator::new(rate, platforms);
    let mut set = vec![CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SEED, 3)];
    if let Some(s) = ebn0_db {
        set.push(CaseSetting::new(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::EbN0,
        ));
        set.push(CaseSetting::new(&sdrkit_algo_sim::chain::GEN_EBN0_DB, s));
    }
    set.extend_from_slice(extra);
    let imp = values(&hrdcp::DESCRIPTOR, &set);
    let mut g = sdrkit_components::sim::impaired(Box::new(g), &imp).unwrap();
    let mut x = vec![Complex32::default(); (seconds * rate) as usize];
    g.generate(&mut x);
    x
}

fn receiver(rate: f64, extra: &[CaseSetting]) -> Box<dyn Receiver> {
    let v = values(&hrdcp::DESCRIPTOR, extra);
    let mut r = (hrdcp::DESCRIPTOR.make_receiver)(&StackConfig::new(v)).unwrap();
    r.configure(&InputInfo::new(rate, DIAL, DIAL).recording())
        .unwrap();
    r
}

fn feed(r: &mut dyn Receiver, x: &[Complex32], rate: f64) -> Vec<Event> {
    let mut out = Output::default();
    let mut ev = Vec::new();
    for c in x.chunks((rate * 0.05) as usize) {
        out.clear();
        r.feed(c, &mut out);
        ev.append(&mut out.events);
    }
    ev
}

fn int(r: &sdrkit_core::Results, key: &str) -> i64 {
    field_text(r, key).parse().unwrap_or(-1)
}

#[test]
fn two_platforms_three_khz_apart() {
    let rate = 250e3;
    let mut a = gen::Platform::a();
    a.offset_hz = -5.0e3;
    let b = gen::Platform {
        offset_hz: -2.0e3,
        address21: 0x5800A,
        sequence: 300,
        alert: true,
        health: 0x0F3,
        data: (0..1000).map(|i| ((131 * i + 7) % 256) as u8).collect(),
        start_s: 0.7,
        period_s: 60.0,
        amplitude: 1.0,
    };
    a.period_s = 60.0;
    let x = signal(vec![a.clone(), b.clone()], rate, 15.0, Some(12.0), &[]);
    let mut r = receiver(rate, &[]);
    let ev = feed(r.as_mut(), &x, rate);
    let res = r.results();
    let reports = rows(res, "reports");
    assert_eq!(int(res, "frames_bad"), 0, "{:?}", res.log_lines());
    assert_eq!(reports.len(), 2, "{:?}", res.log_lines());
    let ra = reports
        .iter()
        .find(|(_, c)| cell(c, "sequence") == "1")
        .unwrap()
        .1;
    assert_eq!(cell(ra, "address"), "162096C4");
    assert_eq!(cell(ra, "type"), "self-timed");
    assert_eq!(cell(ra, "health"), "0101010101");
    assert_eq!(cell(ra, "format"), "ASCII");
    assert_eq!(cell(ra, "crc_ok"), "true");
    assert!(cell(ra, "text").contains("SYNOP 06590"));
    let f = cell_num(ra, "freq_hz").unwrap();
    assert!((f - (DIAL - 5.0e3)).abs() < 5.0, "{f}");
    let rb = reports
        .iter()
        .find(|(_, c)| cell(c, "sequence") == "300")
        .unwrap()
        .1;
    assert_eq!(cell(rb, "frames"), "2");
    assert_eq!(cell(rb, "format"), "binary");
    assert_eq!(cell(rb, "type"), "alert");
    assert_eq!(res.status().lock_state, LockState::Locked);
    assert_eq!(
        ev.iter()
            .filter(|e| e.is(&sdrkit_stack_satcom::typed::HRDCP_REPORT))
            .count(),
        2
    );
    assert!(field_text(res, "note").contains("not confirmed by a public document"));
}

#[test]
fn the_test_signal_decodes_at_its_rate_and_noise_gives_nothing() {
    let rate = hrdcp::DESCRIPTOR.tuning.rate_hz;
    let x = signal(vec![gen::Platform::a()], rate, 8.5, None, &[]);
    let mut r = receiver(rate, &[]);
    feed(r.as_mut(), &x, rate);
    assert_eq!(
        int(r.results(), "frames_ok"),
        1,
        "{:?}",
        r.results().log_lines()
    );
    // Noise alone.
    let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
    let n: Vec<Complex32> = (0..(4.0 * 250e3) as usize)
        .map(|_| Complex32::new(rng.normal() as f32, rng.normal() as f32) * 0.1)
        .collect();
    let mut r = receiver(250e3, &[]);
    feed(r.as_mut(), &n, 250e3);
    assert_eq!(
        (
            int(r.results(), "frames_ok"),
            int(r.results(), "frames_bad")
        ),
        (0, 0)
    );
    assert_ne!(r.results().status().lock_state, LockState::Locked);
}

#[test]
fn impairments_at_96_ksps() {
    let rate = 96e3;
    let mut b = gen::Platform::b();
    b.offset_hz = 3.0e3;
    b.start_s = 0.3;
    b.data = (0..1000).map(|i| ((131 * i + 7) % 256) as u8).collect();
    let imp = [
        CaseSetting::new(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, 125.0),
        CaseSetting::new(&sdrkit_algo_sim::chain::GEN_DRIFT_HZ_S, 1.0),
    ];
    let x = signal(vec![b], rate, 14.0, Some(8.0), &imp);
    let mut r = receiver(rate, &[]);
    feed(r.as_mut(), &x, rate);
    assert_eq!(
        int(r.results(), "frames_ok"),
        1,
        "{:?}",
        r.results().log_lines()
    );
}

#[test]
fn the_plan_file_gives_channel_numbers() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/satcom/meteosat_dcp_plan.json");
    let text = std::fs::read_to_string(path).expect("the plan file");
    let file =
        sdrkit_data_plans::data::parse("meteosat_dcp_plan.json", &text).expect("a plan file");
    assert_eq!(file.groups.len(), 1);
    assert_eq!(file.groups[0].bands.len(), 2);
    let p = ChannelPlan::parse(&text).unwrap();
    assert_eq!(p.channel_of(402.2950e6), Some(174));
    assert_eq!(p.channel_of(402.0355e6), Some(1));
    assert_eq!(p.channel_of(402.0025e6), Some(268));
    assert_eq!(p.channel_of(402.0025e6 + 21.0 * 1.5e3), Some(289));
    assert_eq!(p.channel_of(401.0e6), None);
}

#[test]
fn settings_state_and_clear() {
    let rate = 250e3;
    let x = signal(vec![gen::Platform::a()], rate, 8.5, None, &[]);
    let mut r = receiver(rate, &[]);
    feed(r.as_mut(), &x, rate);
    assert_eq!(rows(r.results(), "reports").len(), 1);
    let state = r.save_state().unwrap();
    let mut z = receiver(rate, &[]);
    z.restore_state(&state).unwrap();
    feed(z.as_mut(), &x[..1000], rate);
    assert_eq!(rows(z.results(), "reports").len(), 1);
    command(r.as_mut(), "clear").unwrap();
    feed(r.as_mut(), &x[..1000], rate);
    assert!(rows(r.results(), "reports").is_empty());
    assert!(set(r.as_mut(), "threshold_db", json!(5.0)).is_err());
    assert!(set(r.as_mut(), "channel_plan", json!("../outside.json")).is_err());
    assert!(set(r.as_mut(), "channel_plan", json!("plans/meteosat.json")).is_ok());
    let v = values(&hrdcp::DESCRIPTOR, &[]);
    let mut low = (hrdcp::DESCRIPTOR.make_receiver)(&StackConfig::new(v)).unwrap();
    assert!(low.configure(&InputInfo::new(40e3, DIAL, DIAL)).is_err());
}

/// The generator of the descriptor, at its rate with the default settings, gives platform A's message within 8 s.
#[test]
fn the_default_test_signal_locks_within_eight_seconds() {
    let rate = hrdcp::DESCRIPTOR.tuning.rate_hz;
    let v = values(&hrdcp::DESCRIPTOR, &[]);
    let g =
        (hrdcp::DESCRIPTOR.make_generator.unwrap())(&sdrkit_core::GenConfig::new(rate, v.clone()))
            .unwrap();
    let mut g = sdrkit_components::sim::impaired(g, &v).unwrap();
    let mut x = vec![Complex32::default(); (8.0 * rate) as usize];
    g.generate(&mut x);
    let mut r = receiver(rate, &[]);
    feed(r.as_mut(), &x, rate);
    assert_eq!(
        r.results().status().lock_state,
        LockState::Locked,
        "{:?}",
        r.results().log_lines()
    );
}
