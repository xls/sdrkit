//! `dect`: the R-CRC, the RFPI, the generator round trip, wrong CRCs, noise and rates, and the scan request.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::owner::Owner;
use sdrkit_core::{
    Complex32, Generator, HostRequest, InputInfo, Output, Receiver, StackConfig, Value,
};
use sdrkit_dev_test::Harness;
use sdrkit_graph::StackGraph;
use sdrkit_stack_ism::dect::{self, Bases, GenParams, TestBase, DECT};

const CARRIER: f64 = 1897.344e6;

fn feed(h: &Harness, rate: f64, x: &[Complex32]) -> StackGraph {
    let mut r = dect::receiver(&StackConfig::new(h.values().unwrap())).unwrap();
    r.configure(&InputInfo::new(rate, CARRIER, CARRIER).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(65536) {
        r.feed(c, &mut out);
        out.clear();
    }
    r.finish(&mut out);
    r
}

fn signal(h: &Harness, rate: f64, p: GenParams, seconds: f64) -> Vec<Complex32> {
    let g: Box<dyn Generator> = Box::new(dect::generator(rate, p));
    let mut g = sdrkit_components::sim::impaired(g, &h.values().unwrap()).unwrap();
    let mut x = vec![Complex32::default(); (rate * seconds) as usize];
    g.generate(&mut x);
    x
}

fn store(r: &StackGraph) -> &dect::Store {
    r.graph().component_as::<Bases>("bases").unwrap().store()
}

fn field(r: &StackGraph, k: &str) -> f64 {
    crate::util::number(r.results(), k)
}

fn noisy(snr_db: f64) -> Harness {
    Harness::new(&DECT)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, snr_db)
        .seed(2)
}

/// The R-CRC is CRC-16/DECT-R, check 0x007E (EN 300 175-3 clause 6.2.5).
#[test]
fn the_r_crc_has_its_final_xor() {
    assert_eq!(dect::r_crc(b"123456789"), 0x007E);
}

/// Only the R-CRC itself passes: the R-CRC with its last bit inverted or complemented is refused.
#[test]
fn only_the_r_crc_itself_passes() {
    for (ta, tail) in [(3, 0x0100A5C3D1u64), (3, 0x0100F31A22), (4, 0x0123456789)] {
        let a = dect::afield(ta, true, 0, tail, true);
        assert!(dect::rcrc_ok(&a));
        let c = u16::from_be_bytes([a[6], a[7]]);
        for wrong in [c ^ 1, !c, c ^ 0x0F0F] {
            let mut b = a;
            b[6..].copy_from_slice(&wrong.to_be_bytes());
            assert!(!dect::rcrc_ok(&b), "{wrong:04X}");
        }
        for bit in 0..64 {
            let mut b = a;
            b[bit / 8] ^= 0x80 >> (bit % 8);
            assert!(!dect::rcrc_ok(&b), "bit {bit}");
        }
    }
}

#[test]
fn rfpi_split_and_qt() {
    assert_eq!(
        dect::rfpi_split(0x0100A5C3D1),
        (false, 0, Some((4106, 47226, 1)))
    );
    assert_eq!(
        dect::rfpi_split(0x0100F31A22),
        (false, 0, Some((4111, 25412, 2)))
    );
    assert_eq!(
        dect::qt_fields(0x0123456789)[0].1,
        "static system information"
    );
    assert_eq!(dect::carrier_of(CARRIER), Some(("Europe", 0)));
    assert_eq!(
        dect::carrier_of(CARRIER - 9.0 * 1.728e6),
        Some(("Europe", 9))
    );
}

#[test]
fn round_trip() {
    for off in [-50e3, 50e3] {
        let h = noisy(22.0).set(&GEN_OFFSET_HZ, off).rate(5e6).seconds(0.1);
        let r = feed(&h, 5e6, &h.generate().unwrap());
        let s = store(&r);
        assert_eq!(s.bases.len(), 2, "{:?}", r.results().fields());
        let a = s.bases.get(&0x0100A5C3D1).unwrap();
        let b = s.bases.get(&0x0100F31A22).unwrap();
        assert_eq!(a.slots, 1);
        assert_eq!(b.slots, 1 << 5);
        assert!(a.qt > 0 && b.qt > 0);
        assert_eq!(a.last_qt, Some(0x0123456789));
        assert_eq!(a.emc, Some(4106));
        let bursts = field(&r, "base_bursts") + field(&r, "handset_bursts");
        assert!(field(&r, "handset_bursts") > 0.0);
        assert!(
            field(&r, "frames_bad") <= 0.01 * bursts + 1.0,
            "{:?}",
            r.results().fields()
        );
    }
}

#[test]
fn wrong_crc_never_creates_a_base() {
    let h = noisy(30.0);
    let p = GenParams {
        bases: vec![TestBase {
            rfpi: 0x0100A5C3D1,
            slot: 2,
            level_db: 0.0,
            good_crc: false,
            handset: false,
        }],
        seed: 3,
    };
    let r = feed(&h, 4.608e6, &signal(&h, 4.608e6, p, 0.1));
    assert!(store(&r).bases.is_empty());
    assert!(field(&r, "base_bursts") > 0.0, "bursts are still counted");
}

#[test]
fn noise_and_rates() {
    let h = noisy(10.0)
        .set(&dect::GEN_OPTION, dect::GenBases::None)
        .rate(5e6)
        .seconds(2.0);
    let r = feed(&h, 5e6, &h.generate().unwrap());
    assert!(store(&r).bases.is_empty());
    let mut r = dect::receiver(&StackConfig::new(h.values().unwrap())).unwrap();
    assert!(r.configure(&InputInfo::new(4.608e6, 1.9e9, 1.9e9)).is_ok());
    assert!(r.configure(&InputInfo::new(4e6, 1.9e9, 1.9e9)).is_err());
    assert_eq!(
        r.results().status().summary,
        "DECT cordless phones needs a sample rate of at least 4.608 Msps"
    );
}

#[test]
fn the_scan_asks_the_host_to_hop_over_the_band() {
    let mut r = dect::receiver(&StackConfig::new(Harness::new(&DECT).values().unwrap())).unwrap();
    r.configure(&InputInfo::new(4.608e6, CARRIER, CARRIER).recording())
        .unwrap();
    let mut out = Output::default();
    r.feed(&vec![Complex32::default(); 100_000], &mut out);
    assert!(out.requests.is_empty());
    crate::util::set(&mut r, "scan", Value::Bool(true)).unwrap();
    out.clear();
    r.feed(&vec![Complex32::default(); 100_000], &mut out);
    match out.requests.as_slice() {
        [HostRequest::Hop {
            channels, dwell_s, ..
        }] => {
            assert_eq!(channels.len(), 10);
            assert!(channels.windows(2).all(|w| w[0] < w[1]));
            assert_eq!(*channels.last().unwrap(), CARRIER);
            assert_eq!(*dwell_s, 5.0);
        }
        other => panic!("{other:?}"),
    }
    crate::util::set(&mut r, "scan", Value::Bool(false)).unwrap();
    out.clear();
    r.feed(&vec![Complex32::default(); 100_000], &mut out);
    assert!(matches!(
        out.requests.as_slice(),
        [HostRequest::Retune { tuned_hz, .. }] if *tuned_hz == CARRIER
    ));
}

/// A value of the wrong kind or out of range is refused.
#[test]
fn a_bad_setting_value_is_refused() {
    assert!(crate::util::refused(
        &Bases::default(),
        "band",
        Value::Text("moon".into())
    ));
    let mut r = dect::receiver(&StackConfig::new(Harness::new(&DECT).values().unwrap())).unwrap();
    assert!(crate::util::set(&mut r, "dwell", Value::Duration(0.0)).is_err());
}

/// The events of new base stations carry the frequency of the carrier.
#[test]
fn every_event_is_declared_and_has_its_frequency() {
    let run = Harness::new(&DECT).rate(5e6).seconds(0.1).run().unwrap();
    let decl = sdrkit_stack_ism::typed::shape("dect").results_decl();
    assert_eq!(run.events.len(), 2, "{:?}", run.events);
    assert!(crate::util::declared(&decl, &run.events));
    for e in &run.events {
        assert!(
            (e.freq_hz - CARRIER).abs() < 50e3,
            "{} at {} Hz",
            e.kind(),
            e.freq_hz
        );
    }
}

/// The demodulator benchmark runs (`ism.dect_demod`; the pending candidates are a queue, not a `Vec` with `remove(0)`).
#[test]
fn the_demodulator_benchmark_runs() {
    let b = dect::benchmark();
    assert_eq!(b.name, "ism.dect_demod");
    assert!((b.run)(0.01).items_per_second > 0.0);
}

/// A change of frequency keeps the base stations, the bursts and their counts, and drops the SNR of the present signal.
#[test]
fn a_change_of_frequency_keeps_the_bases_and_drops_the_snr() {
    let h = Harness::new(&DECT).rate(5e6).seconds(0.1);
    let mut r = feed(&h, 5e6, &h.generate().unwrap());
    let bases = crate::util::table(r.results(), "bases").len();
    let bursts = field(&r, "base_bursts");
    assert!(bases >= 1 && bursts > 0.0);
    assert!(r.results().status().snr_db.is_some());
    r.forget();
    assert_eq!(crate::util::table(r.results(), "bases").len(), bases);
    assert!(!crate::util::table(r.results(), "bursts").is_empty());
    assert_eq!(field(&r, "base_bursts"), bursts);
    assert_eq!(r.results().status().snr_db, None);
}
