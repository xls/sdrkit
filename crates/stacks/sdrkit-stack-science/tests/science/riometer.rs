//! The riometer: sidereal time, the quiet-day curve over days of minute powers with absorption and interference, the curve
//! as text, the power estimate on IQ with bursts and a carrier, and the receiver with and without the station position.

use sci::riometer::gen::{GenParams, RiometerGenerator};
use sci::riometer::meter::{Minute, PowerMeter};
use sci::riometer::qdc::{self, Qdc};
use sci::riometer::{Riometer, DESCRIPTOR, QDC_SETTING};
use sci::typed::{
    RiometerAbsorptionEvent, RiometerFields, RiometerQdcArgs, RiometerQdcReply, RIOMETER_ABSORPTION,
};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, Generator, InputInfo, Output, Receiver, Value};
use sdrkit_dev_test::Harness;
use sdrkit_stack_science as sci;
use std::f64::consts::TAU;

/// 2000-01-01 12:00:00 UT.
const J2000: f64 = 946_728_000.0;
/// 2026-01-01 00:00:00 UTC.
const START: f64 = 1_767_225_600.0;

#[test]
fn local_sidereal_time_at_j2000_a_sidereal_day_later_and_15_degrees_east() {
    assert!((qdc::lst_hours(J2000, 0.0) - 18.6974).abs() <= 0.001);
    assert!((qdc::lst_hours(J2000 + 86_164.090_5, 0.0) - 18.6974).abs() <= 0.01);
    assert!((qdc::lst_hours(J2000, 15.0) - 19.6974).abs() <= 0.001);
    assert_eq!(qdc::bin_of(23.999_999), 287);
    assert_eq!(qdc::bin_of(0.0), 0);
}

/// The quiet sky: 1 + 0.25 cos(2 pi (LST - 17.7) / 24).
fn sky(lst: f64) -> f64 {
    1.0 + 0.25 * (TAU * (lst - 17.7) / 24.0).cos()
}

const LON: f64 = 21.0;

fn minute(i: u64, power: f64, interfered: bool) -> Minute {
    Minute {
        index: i,
        power,
        interfered,
        interfered_seconds: 0,
    }
}

#[test]
fn seven_days_read_an_absorption_of_0_8_db_on_day_7() {
    let mut rio = Riometer::new(0.3);
    let mut rng = Rng::new(4);
    let event = (6 * 1440 + 600)..(6 * 1440 + 720);
    let (mut read, mut flagged, mut false_minutes) = (None, false, 0);
    for i in 0..7 * 1440u64 {
        let t = i as f64 * 60.0 + 30.0;
        let mut p = sky(qdc::lst_hours(START + t, LON)) * (1.0 + 0.004 * rng.normal());
        if event.contains(&i) {
            p *= 10f64.powf(-0.08);
        }
        let o = rio.push(minute(i, p, false), t, START + t, Some(LON));
        if i == event.start + 60 {
            read = Some(o.absorption_db);
            flagged = rio.event_on();
        }
        if i >= 2 * 1440 && !event.contains(&i) && o.absorption_db >= 0.3 {
            false_minutes += 1;
        }
    }
    assert!(rio.qdc().filled() >= 280, "{}", rio.qdc().filled());
    let a = read.unwrap();
    assert!((a - 0.8).abs() <= 0.15, "{a}");
    assert!(flagged);
    assert!(false_minutes <= 3, "{false_minutes}");
}

/// Five days with every 97th minute three times stronger and flagged and every 131st minute 1.4 times stronger and not
/// flagged: every bin of the curve is within 0.8 dB of the sky at the middle of the bin, also the bins whose first minute was
/// one of the stronger ones.
#[test]
fn the_curve_follows_the_sky_through_interference() {
    let mut rio = Riometer::new(0.3);
    let mut rng = Rng::new(9);
    let mut first_high = 0;
    for i in 0..5 * 1440u64 {
        let t = i as f64 * 60.0 + 30.0;
        let mut p = sky(qdc::lst_hours(START + t, LON)) * (1.0 + 0.004 * rng.normal());
        let mut flag = false;
        if i % 97 == 0 {
            p *= 3.0;
            flag = true;
        } else if i % 131 == 0 {
            p *= 1.4;
            if rio.qdc().at(qdc::lst_hours(START + t, LON)).is_none() {
                first_high += 1;
            }
        }
        rio.push(minute(i, p, flag), t, START + t, Some(LON));
    }
    assert!(first_high >= 1, "no stronger minute came first in its bin");
    for (k, &q) in rio.qdc().bins().iter().enumerate() {
        assert!(q > 0.0, "bin {k} empty");
        let e = 10.0 * (q / sky((k as f64 + 0.5) * 24.0 / 288.0)).log10();
        assert!(e.abs() <= 0.8, "bin {k}: {e:.2} dB");
    }
}

/// An empty bin refuses a first minute that is a jump against the bins beside it, takes one that is a jump against only one of
/// them, and takes any minute when no bin beside it holds a value.
#[test]
fn an_empty_bin_does_not_take_a_jump_as_its_first_value() {
    let h = |k: usize| (k as f64 + 0.5) / 12.0;
    let mut q = Qdc::default();
    q.update(h(10), 1.4, false);
    assert_eq!(q.at(h(10)), Some(1.4));
    q.update(h(9), 1.0, false);
    q.update(h(8), 1.2, false);
    assert_eq!(q.at(h(8)), None);
    q.update(h(8), 1.1, false);
    assert_eq!(q.at(h(8)), Some(1.1));
    // Bin 11 has bin 10 (1.4) and the empty bin 12 beside it.
    q.update(h(11), 1.7, false);
    assert_eq!(q.at(h(11)), None);
    q.update(h(12), 1.0, false);
    q.update(h(11), 1.12, false);
    assert_eq!(q.at(h(11)), Some(1.12));
    // The curve is a circle: bin 287 is beside bin 0.
    q.update(h(0), 1.0, false);
    q.update(h(287), 1.3, false);
    assert_eq!(q.at(h(287)), None);
}

#[test]
fn the_curve_as_text_reads_back_and_bad_texts_are_refused() {
    let mut q = Qdc::default();
    // From the top bin down, so that no first value is a jump against the bin beside it.
    for k in (0..288).rev() {
        q.update((k as f64 + 0.5) / 12.0, 0.001 * (k + 1) as f64, false);
    }
    let text = q.export();
    assert!(text.starts_with("qdc1 0.001 0.002 "), "{}", &text[..40]);
    assert_eq!(text.split(' ').count(), 289);
    let back = Qdc::import(&text).unwrap();
    for (a, b) in back.bins().iter().zip(q.bins()) {
        assert!((a / b - 1.0).abs() < 5e-4);
    }
    assert!(Qdc::import("qdc1 1 2 3").is_err());
    assert!(Qdc::import("junk").is_err());
    assert!(Qdc::import(&text.replace("qdc1", "qdc2")).is_err());
    assert!(Qdc::import(&format!("{text} 1")).is_err());
    assert!(Qdc::import(&text.replacen(" 0.001 ", " -1 ", 1)).is_err());
}

/// 130 s at 48 ksps: noise of power 1, 0.5 dB lower from 70 s, 12 % burst chunks, a carrier of amplitude 3 at 5 kHz.
fn iq_130_s() -> Vec<Complex32> {
    let p = GenParams {
        amplitude: std::f64::consts::FRAC_1_SQRT_2,
        burst_share: 0.12,
        half_step_db: 0.0,
        period_s: 600.0,
        step: Some((70.0, -0.5)),
        carrier: Some((5000.0, 3.0)),
    };
    let mut g = RiometerGenerator::new(48e3, p, 3);
    let mut x = vec![Complex32::default(); (48e3 * 130.0) as usize];
    g.generate(&mut x);
    x
}

#[test]
fn iq_with_bursts_and_a_carrier_reads_two_minutes_in_pieces_of_any_size() {
    let x = iq_130_s();
    let mut runs = Vec::new();
    for piece in [4800usize, 65_536] {
        let mut m = PowerMeter::new(48e3);
        let mut minutes = Vec::new();
        for c in x.chunks(piece) {
            m.process(c);
            minutes.extend(m.take_minutes());
        }
        runs.push(minutes);
    }
    assert_eq!(runs[0].len(), 2);
    assert_eq!(runs[0], runs[1]);
    let db: Vec<f64> = runs[0].iter().map(|m| 10.0 * m.power.log10()).collect();
    assert!(db[0].abs() <= 0.25, "{db:?}");
    assert!((db[1] + 0.5).abs() <= 0.2, "{db:?}");
}

fn fed(h: Harness, x: &[Complex32]) -> Box<dyn Receiver> {
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(48e3, 30e6, 30e6).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(16_384) {
        out.clear();
        rx.feed(c, &mut out);
    }
    rx
}

fn command(rx: &mut dyn Receiver, key: &str, text: Option<&str>) -> Result<Option<sdrkit_core::Record>, sdrkit_core::CommandError> {
    let id = rx.commands().id(key).unwrap();
    let mut args = rx.commands().def(id).unwrap().new_args();
    if let Some(t) = text {
        sdrkit_core::interface::Arguments::write(&RiometerQdcArgs { text: t }, &mut args);
    }
    sdrkit_core::owner::invoke(rx, id, &args)
}

#[test]
fn without_a_position_the_receiver_measures_power_only() {
    let x = iq_130_s();
    let rx = fed(Harness::new(&DESCRIPTOR).rate(48e3).set(&sci::mscatter::START_UTC, START), &x);
    let r = rx.results();
    assert_eq!(r.fields().at(&RiometerFields::MINUTES), Some(2));
    assert!(r.fields().at(&RiometerFields::POWER_DB).unwrap().abs() < 1.0);
    assert_eq!(r.fields().at(&RiometerFields::CURVE_BINS), Some(0));
    assert!(r.status().summary.contains("position is missing"), "{}", r.status().summary);
}

#[test]
fn a_saved_curve_gives_absorption_and_events_through_the_receiver() {
    // A curve 1 dB over the power of the test signal everywhere: absorption about 1 dB from the first minute.
    let level = 10f64.powf(0.1) * 2.0 * 0.1 * 0.1;
    let curve = format!("qdc1{}", format!(" {level:.6}").repeat(288));
    let station = sdrkit_core::Station {
        position: Some(sdrkit_util_geo::Position::new(60.0, LON)),
        ..sdrkit_core::Station::default()
    };
    let h = Harness::new(&DESCRIPTOR)
        .rate(48e3)
        .station(station)
        .set(&QDC_SETTING, curve.as_str())
        .set(&sci::mscatter::START_UTC, START);
    let mut g = RiometerGenerator::new(48e3, GenParams::standard(0.03), 1);
    let mut x = vec![Complex32::default(); (48e3 * 125.0) as usize];
    g.generate(&mut x);
    let r = h.feed(&x).unwrap();
    let a = r.results.fields().at(&RiometerFields::ABSORPTION_DB).unwrap();
    assert!((a - 1.0).abs() < 0.3, "{a}");
    assert_eq!(r.results.fields().at(&RiometerFields::CURVE_READY), Some(true));
    let ev: Vec<_> = r.events.iter().filter(|e| e.is(&RIOMETER_ABSORPTION)).collect();
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert_eq!(ev[0].at(&RiometerAbsorptionEvent::PHASE), Some("start"));
    // A bad curve is refused.
    let mut rx = h.receiver().unwrap();
    let id = rx.schema().id(QDC_SETTING.key()).unwrap();
    assert!(sdrkit_core::owner::apply(rx.as_mut(), id, &Value::Text("qdc1 1 2 3".into())).is_err());
}

#[test]
fn export_and_import_commands_carry_the_curve() {
    let x = iq_130_s();
    let station = sdrkit_core::Station {
        position: Some(sdrkit_util_geo::Position::new(60.0, LON)),
        ..sdrkit_core::Station::default()
    };
    let mut rx = fed(
        Harness::new(&DESCRIPTOR).rate(48e3).station(station).set(&sci::mscatter::START_UTC, START),
        &x,
    );
    // Two minutes fill one or two bins of 5 sidereal minutes.
    let n = rx.results().fields().at(&RiometerFields::CURVE_BINS).unwrap();
    assert!((1..=2).contains(&n), "{n}");
    let reply = command(rx.as_mut(), "export_qdc", None).unwrap().unwrap();
    let text = reply.at(&RiometerQdcReply::QDC).unwrap().to_string();
    assert_eq!(Qdc::import(&text).unwrap().filled() as i64, n);
    assert!(command(rx.as_mut(), "import_qdc", Some("junk")).is_err());
    let full = format!("qdc1{}", " 0.5".repeat(288));
    command(rx.as_mut(), "import_qdc", Some(&full)).unwrap();
    // The curve shows at the next report; clear keeps it.
    command(rx.as_mut(), "clear", None).unwrap();
    let reply = command(rx.as_mut(), "export_qdc", None).unwrap().unwrap();
    assert_eq!(Qdc::import(reply.at(&RiometerQdcReply::QDC).unwrap()).unwrap().filled(), 288);
}
