//! Orbcomm: the generator through the standard impairments into the receiver.

use crate::common::{command, command_with, number, row_count, set};
use sdrkit_core::{Choice, InputInfo, LockState, Results, StateBlob, TableDef, Value};
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_satcom::modes::orbcomm::{self, gen, packet};
use serde_json::json;

const RATE: f64 = 250e3;
const CENTRE: f64 = 137.275e6;

fn harness(seconds: f64) -> Harness {
    Harness::new(&orbcomm::DESCRIPTOR)
        .rate(RATE)
        .freq(CENTRE)
        .seconds(seconds)
        .piece(25_000)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::EsN0,
        )
        .set(&sdrkit_algo_sim::chain::GEN_ESN0_DB, 20.0)
}

fn run(h: Harness) -> Run {
    h.run().expect("the run")
}

fn int(r: &Run, key: &str) -> i64 {
    r.field(key).and_then(number).map_or(0, |v| v as i64)
}

fn def(table: &str) -> &'static TableDef {
    match table {
        "satellites" => sdrkit_stack_satcom::typed::ORBCOMM_SATELLITES.def(),
        "decoders" => sdrkit_stack_satcom::typed::ORBCOMM_DECODERS.def(),
        "packets" => sdrkit_stack_satcom::typed::ORBCOMM_PACKETS.def(),
        _ => sdrkit_stack_satcom::typed::ORBCOMM_PACKET_TYPES.def(),
    }
}

/// The cell of row `row` of `table` in the declared column `col`.
fn cell(r: &Results, table: &str, row: &str, col: usize) -> Value {
    let t = r.table(r.decl().table_id(table).unwrap()).unwrap();
    let rec = t.row(row).unwrap_or_else(|| panic!("{table} row {row}"));
    let id = rec.decl().id(def(table).columns[col].key()).unwrap();
    rec.value(id).cloned().unwrap_or(Value::None)
}

fn num(v: Value) -> f64 {
    number(&v).unwrap_or(f64::NAN)
}

fn state(v: Value) -> String {
    match v {
        Value::Choice(i) => orbcomm::DecoderState::from_index(i)
            .map(|s| format!("{s:?}"))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// The decoders table row of the decoder that follows satellite `id`.
fn decoder_of(r: &Results, id: u8) -> Option<String> {
    ["0", "1"]
        .into_iter()
        .find(|d| cell(r, "decoders", d, 7) == Value::I64(i64::from(id)))
        .map(str::to_string)
}

#[test]
fn o1_packet_format() {
    let mut p = packet::build(&packet::Packet::Fill);
    assert!(packet::check(&p));
    p[5] ^= 0x10;
    assert!(!packet::check(&p));
    // The WGS-84 conversion: a point on the equator at 775 km.
    let (la, lo, h) = packet::ecef_to_wgs84(6378.137 + 775.0, 0.0, 0.0);
    assert!(la.abs() < 1e-9 && lo.abs() < 1e-9 && (h - 775.0).abs() < 1e-6);
}

#[test]
fn o2_clean_downlink() {
    let secs = 12.0;
    let r = run(harness(secs));
    let res = &r.results;
    assert_eq!(
        res.status().lock_state,
        LockState::Locked,
        "{}",
        res.status().summary
    );
    let sent = (secs * 50.0) as i64;
    let good = int(&r, "frames_ok");
    assert!(good + 200 >= sent, "{good} of {sent}");
    assert_eq!(int(&r, "frames_bad"), 0);
    let dop = num(cell(res, "satellites", "23", 2));
    assert!((dop - 1200.0).abs() < 15.0, "{dop}");
    let f = num(cell(res, "satellites", "23", 1));
    assert!((f - (137.25e6 + 1200.0)).abs() < 15.0, "{f}");
    let h = num(cell(res, "satellites", "23", 8));
    assert!(h > 750e3 && h < 820e3, "{h}");
    assert!(matches!(
        cell(res, "satellites", "23", 7),
        Value::Position { .. }
    ));
    let incl = num(cell(res, "satellites", "23", 9));
    assert!((incl - 45.0).abs() < 0.01);
    let period = num(cell(res, "satellites", "23", 10));
    assert!(period > 99.0 * 60.0 && period < 102.0 * 60.0, "{period}");
    assert!(res.points().contains_key("orbcomm/23"));
    for k in [
        "sync",
        "message",
        "uplink",
        "downlink",
        "ephemeris",
        "elements",
        "network",
        "fill",
    ] {
        assert!(num(cell(res, "packet_types", k, 1)) > 0.0, "{k}");
    }
    assert!(int(&r, "messages_joined") > 5);
    let messages: Vec<_> = r.events_of("orbcomm.message").collect();
    assert!(
        messages.iter().any(
            |e| gen::MESSAGES.contains(&e.get(&sdrkit_core::publish::keys::TEXT).unwrap_or(""))
        ),
        "{messages:?}"
    );
    assert_eq!(state(cell(res, "decoders", "0", 1)), "FrameLock");
    let c = res.decl().series_id("constellation").unwrap();
    assert!(!res.series(c).unwrap().points.is_empty());
}

#[test]
fn o3_two_downlinks_with_a_ramp() {
    let secs = 14.0;
    let h = harness(secs)
        .set(&sdrkit_stack_satcom::modes::orbcomm::gen::GEN_DOWNLINKS, 2)
        .set(
            &sdrkit_stack_satcom::modes::orbcomm::gen::GEN_RAMP_HZ,
            4000.0,
        )
        .set(&sdrkit_stack_satcom::modes::orbcomm::gen::GEN_RAMP_S, 10.0);
    let mut p = gen::GenParams::new(RATE);
    p.downlinks = 2;
    p.ramp_hz = 4000.0;
    p.ramp_s = 10.0;
    let g = gen::OrbGenerator::new(p);
    let r = run(h);
    let res = &r.results;
    for (k, id) in [(0usize, 23u8), (1, 41)] {
        let heard = num(cell(res, "satellites", &id.to_string(), 5));
        assert!(heard + 200.0 >= secs * 50.0, "{id}: {heard}");
        let d = decoder_of(res, id).unwrap_or_else(|| panic!("decoder of {id}"));
        let got = num(cell(res, "decoders", &d, 4));
        let want = g.doppler(k, secs);
        assert!((got - want).abs() < 60.0, "{id}: {got} vs {want}");
    }
    assert!(int(&r, "frames_bad") < 20);
}

#[test]
fn o4_one_decoder_and_a_clock_error() {
    let r = run(harness(10.0)
        .set(&sdrkit_stack_satcom::modes::orbcomm::gen::GEN_DOWNLINKS, 2)
        .set(&sdrkit_algo_sim::chain::GEN_CLOCK_PPM, 150.0)
        .set(
            &sdrkit_stack_satcom::modes::orbcomm::DECODERS,
            sdrkit_stack_satcom::modes::orbcomm::DecoderCount::One,
        ));
    let res = &r.results;
    assert_eq!(state(cell(res, "decoders", "1", 1)), "Free");
    assert!(int(&r, "frames_ok") > 250);
    assert_eq!(state(cell(res, "decoders", "0", 1)), "FrameLock");
    let ppm = num(cell(res, "decoders", "0", 8));
    assert!((ppm.abs() - 150.0).abs() < 60.0, "{ppm}");
}

#[test]
fn o5_broken_checks() {
    let secs = 10.0;
    let r = run(harness(secs).set(&sdrkit_stack_satcom::modes::orbcomm::gen::GEN_BROKEN, 0.10));
    let (good, bad) = (int(&r, "frames_ok"), int(&r, "frames_bad"));
    let seen = good + bad;
    assert!(seen as f64 > 0.85 * secs * 50.0 - 200.0, "{seen}");
    let share = bad as f64 / seen as f64;
    assert!((share - 0.10).abs() < 0.03, "{share}");
    assert_eq!(r.results.status().lock_state, LockState::Locked);
}

#[test]
fn o6_noise_only() {
    let h = harness(6.0);
    let noise = vec![sdrkit_core::Complex32::new(0.0, 0.0); (6.0 * RATE) as usize];
    let mut x = noise;
    let mut awgn = sdrkit_algo_sim::noise::Awgn::with_power(0.02, 5).unwrap();
    awgn.process(&mut x);
    let r = h.feed(&x).unwrap();
    assert_eq!(int(&r, "frames_ok"), 0);
    assert_eq!(int(&r, "satellites"), 0);
}

#[test]
fn cut_independence() {
    let h = harness(5.0);
    let a = run(h.clone().piece(1_013));
    let b = run(h.piece(100_000));
    assert!(int(&a, "frames_ok") > 100);
    assert_eq!(a.field("frames_ok"), b.field("frames_ok"));
    let packets = |r: &Run| {
        r.results
            .table(r.results.decl().table_id("packets").unwrap())
            .cloned()
    };
    assert_eq!(packets(&a), packets(&b));
}

#[test]
fn every_event_is_declared_and_has_a_frequency() {
    let r = run(harness(8.0));
    assert!(!r.events.is_empty());
    for e in &r.events {
        assert!(e.is(&sdrkit_stack_satcom::typed::ORBCOMM_MESSAGE), "{e:?}");
        assert!(e.freq_hz > 136e6, "{e:?}");
    }
}

#[test]
fn a_channel_change_asks_the_host_to_retune() {
    let mut rx = Harness::new(&orbcomm::DESCRIPTOR)
        .rate(RATE)
        .receiver()
        .unwrap();
    rx.configure(&InputInfo::new(RATE, CENTRE, CENTRE)).unwrap();
    let channel = Value::Choice(orbcomm::Channel::C1374625.index());
    command_with(rx.as_mut(), "preset.apply", &[("channel", channel)]).unwrap();
    let mut out = sdrkit_core::Output::default();
    rx.feed(
        &vec![sdrkit_core::Complex32::new(0.0, 0.0); RATE as usize],
        &mut out,
    );
    assert!(
        out.requests.iter().any(|q| matches!(
            q,
            sdrkit_core::HostRequest::Retune { tuned_hz, .. } if (*tuned_hz - 137.4625e6).abs() < 1.0
        )),
        "{:?}",
        out.requests
    );
    assert!(set(rx.as_mut(), "decoders", json!("3")).is_err());
}

#[test]
fn saved_state_comes_back() {
    let h = harness(6.0);
    let x = h.generate().unwrap();
    let mut a = h.receiver().unwrap();
    a.configure(&InputInfo::new(RATE, CENTRE, CENTRE)).unwrap();
    let mut out = sdrkit_core::Output::default();
    a.feed(&x, &mut out);
    let state = a.save_state().expect("a saved state");
    let mut b = h.receiver().unwrap();
    b.configure(&InputInfo::new(RATE, CENTRE, CENTRE)).unwrap();
    b.restore_state(&state).unwrap();
    b.feed(&x[..25_000], &mut out);
    assert_eq!(
        row_count(a.results(), "satellites"),
        row_count(b.results(), "satellites")
    );
    assert!(b
        .restore_state(&StateBlob::new("argos", 1, Vec::new()))
        .is_err());
    command(b.as_mut(), "clear").unwrap();
}

#[test]
fn frequencies_follow_a_retune() {
    let other = 137.300e6;
    let h = harness(8.0).set(
        &sdrkit_stack_satcom::modes::orbcomm::gen::GEN_CENTRE_HZ,
        other,
    );
    let x = h.generate().unwrap();
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(RATE, CENTRE, CENTRE)).unwrap();
    let mut out = sdrkit_core::Output::default();
    rx.feed(
        &vec![sdrkit_core::Complex32::new(0.0, 0.0); 25_000],
        &mut out,
    );
    rx.retuned(&InputInfo::new(RATE, other, other));
    out.clear();
    rx.feed(&x, &mut out);
    let f = num(cell(rx.results(), "satellites", "23", 1));
    assert!((f - (137.25e6 + 1200.0)).abs() < 15.0, "{f}");
}
