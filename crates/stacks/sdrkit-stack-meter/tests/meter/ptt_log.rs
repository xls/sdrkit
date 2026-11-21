//! The PTT logger: the scripted session at two rates and three carrier offsets; a stuck key; a sweep of the noise; noise
//! alone; silence at the start.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_DRIFT_HZ_S, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ, GEN_SNR_DB,
};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::ptt_log::gen::{parse_spans, DEFAULT_KEYED, GEN_FADES, GEN_KEYED};
use sdrkit_stack_meter::ptt_log::DESCRIPTOR;
use sdrkit_stack_meter::typed::{PttKind, PttLogFields, PttTransmissionEvent, PTT_TRANSMISSION};

const CENTRE: f64 = 146.52e6;

fn at_cnr(cnr: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, cnr)
}

fn rows(r: &Streamed) -> Vec<PttTransmissionEvent> {
    r.events
        .iter()
        .filter_map(|e| PTT_TRANSMISSION.read::<PttTransmissionEvent>(e))
        .collect()
}

fn session(rate: f64, offset: f64, drift: f64) {
    let h = at_cnr(24.0)
        .set(&GEN_OFFSET_HZ, offset)
        .set(&GEN_DRIFT_HZ_S, drift);
    let r = stream(&DESCRIPTOR, &h.seed(1), rate, CENTRE, 330.0, 1, |_, _| {});
    let at = format!("{rate} sps, {offset} Hz, {drift} Hz/s");
    let sent = parse_spans(DEFAULT_KEYED).unwrap();
    let got: Vec<PttTransmissionEvent> = rows(&r)
        .into_iter()
        .filter(|e| e.kind != PttKind::StuckKey)
        .collect();
    assert_eq!(got.len(), 15, "{at}: {got:?}");
    let mut worst = (0.0f64, 0.0f64);
    for (&(a, b), e) in sent.iter().zip(&got) {
        let (ds, dl) = ((e.start_s.0 - a).abs(), (e.length_s.0 - (b - a)).abs());
        worst = (worst.0.max(ds), worst.1.max(dl));
        assert!(ds <= 0.1 && dl <= 0.15, "{at}: sent {a}..{b}, got {e:?}");
        assert_eq!(e.kind == PttKind::Kerchunk, b - a < 0.5, "{at}: {e:?}");
    }
    let f = PttLogFields::read(r.results.fields()).unwrap();
    let keyed: f64 = sent.iter().map(|&(a, b)| b - a).sum();
    let duty = f.duty_pct.unwrap();
    assert_eq!((f.transmissions, f.kerchunks), (15, 2), "{at}");
    assert!(
        (duty - 100.0 * keyed / 325.0).abs() <= 1.0,
        "{at}: duty {duty}"
    );
    println!(
        "{at}: starts within {:.3} s, lengths within {:.3} s, duty {duty:.2} %",
        worst.0, worst.1
    );
}

#[test]
fn the_session_at_48_ksps() {
    session(48e3, 0.0, 0.0);
    session(48e3, 2300.0, 0.0);
    session(48e3, -4100.0, 3.0);
}

#[test]
fn the_session_at_96_ksps() {
    session(96e3, 0.0, 0.0);
    session(96e3, 2300.0, 0.0);
    session(96e3, -4100.0, 3.0);
}

#[test]
fn a_key_down_of_250_s_is_one_stuck_key_row_and_one_transmission() {
    let h = at_cnr(24.0).set(&GEN_KEYED, "20, 270").set(&GEN_FADES, "");
    let r = stream(&DESCRIPTOR, &h.seed(2), 48e3, CENTRE, 300.0, 2, |_, _| {});
    let rows = rows(&r);
    assert_eq!(
        rows.iter().filter(|e| e.kind == PttKind::StuckKey).count(),
        1,
        "{rows:?}"
    );
    let tx: Vec<_> = rows
        .iter()
        .filter(|e| e.kind == PttKind::Transmission)
        .collect();
    assert_eq!(tx.len(), 1, "{rows:?}");
    assert!((tx[0].length_s.0 - 250.0).abs() <= 0.2, "{tx:?}");
}

#[test]
fn ten_overs_are_found_without_an_extra_from_8_db() {
    let overs: String = (0..10)
        .map(|k| format!("{}, {}; ", 15 + 12 * k, 19 + 12 * k))
        .collect();
    for cnr in [20.0, 14.0, 10.0, 8.0, 6.0, 4.0] {
        let h = at_cnr(cnr).set(&GEN_KEYED, &overs).set(&GEN_FADES, "");
        let r = stream(&DESCRIPTOR, &h.seed(3), 48e3, CENTRE, 140.0, 3, |_, _| {});
        let rows = rows(&r);
        let hits = (0..10)
            .filter(|k| {
                rows.iter()
                    .any(|e| (e.start_s.0 - (15 + 12 * k) as f64).abs() <= 0.3)
            })
            .count();
        println!("{cnr} dB: {hits} of 10, {} rows", rows.len());
        if cnr >= 8.0 {
            assert_eq!((hits, rows.len()), (10, 10), "{cnr} dB: {rows:?}");
        }
    }
}

#[test]
fn noise_alone_for_30_minutes_gives_no_transmission() {
    let h = Harness::new(&DESCRIPTOR)
        .set(&GEN_KEYED, "")
        .set(&GEN_FADES, "")
        .set(&GEN_NOISE, NoiseKind::Floor)
        .set(&GEN_NOISE_FLOOR_DBFS, -30.0);
    let r = stream(&DESCRIPTOR, &h.seed(4), 48e3, CENTRE, 1800.0, 4, |_, _| {});
    assert!(rows(&r).is_empty(), "{:?}", rows(&r));
}

#[test]
fn silence_at_the_start_does_not_set_the_floor() {
    for zero_s in [0.5, 2.0, 4.0] {
        let h = at_cnr(20.0).set(&GEN_OFFSET_HZ, 120.0);
        let r = stream(&DESCRIPTOR, &h.seed(3), 48e3, CENTRE, 25.0, 3, |t0, x| {
            for (i, z) in x.iter_mut().enumerate() {
                if t0 + i as f64 / 48e3 < zero_s {
                    *z = Default::default();
                }
            }
        });
        let f = PttLogFields::read(r.results.fields()).unwrap();
        assert_eq!(
            (f.transmissions, f.keyed),
            (2, false),
            "{zero_s} s of zeros: {f:?}"
        );
    }
}
