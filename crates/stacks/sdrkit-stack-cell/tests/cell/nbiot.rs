//! NB-IoT: the recording against its expected results, and test signal round trips.

use crate::common::*;
use sdrkit_core::{Complex32, InputInfo};
use sdrkit_stack_cell::nbiot::gen::{scene, NbGenOptions, NbGenerator};
use sdrkit_stack_cell::NBIOT;
use serde_json::Value as J;

fn check_c4(d: &J, exp: &J, split: bool) {
    let cells = d["cells"].as_array().unwrap();
    assert_eq!(
        cells.len(),
        1,
        "{}",
        serde_json::to_string_pretty(d).unwrap()
    );
    let c = &cells[0];
    let w = &exp["cells"][0];
    for k in ["pci", "frequency_hz", "earfcn", "band"] {
        assert_eq!(c[k], w[k], "{k}");
    }
    for (k, v) in w["mib"].as_object().unwrap() {
        assert_eq!(&c["mib"][k], v, "mib.{k}");
    }
    for (k, v) in w["sib1"].as_object().unwrap() {
        assert_eq!(&c["sib1"][k], v, "sib1.{k}");
    }
    assert!(near(&c["snr_db"], 15.0, 3.0), "SNR {}", c["snr_db"]);
    assert!(
        near(&c["total_carrier_offset_hz"], 6500.0, 150.0),
        "{}",
        c["total_carrier_offset_hz"]
    );
    if split {
        assert!(
            near(&c["raster_offset_khz"], 2.5, 1.0),
            "{}",
            c["raster_offset_khz"]
        );
        assert!(
            near(&c["tuning_error_hz"], 4000.0, 200.0),
            "{}",
            c["tuning_error_hz"]
        );
    }
}

#[test]
fn recording() {
    let Some((rec, exp)) = vector("nbiot_standalone_1920k") else {
        return;
    };
    let input = InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz).recording();
    let d = run_mode(&NBIOT, &[], input, &rec.samples, &[]);
    check_c4(&d, &exp, false);
    let pieces = random_pieces(11, rec.samples.len());
    let d2 = run_mode(
        &NBIOT,
        &[("freq_correction_ppm", J::from(4.233))],
        input,
        &rec.samples,
        &pieces,
    );
    check_c4(&d2, &exp, true);
}

/// Scene `sc` at `rate` with noise for `snr_db` in the carrier (seeded by `seed`), `seconds` long.
fn gen(
    rate: f64,
    sc: u32,
    opt: NbGenOptions,
    snr_db: f64,
    seed: u64,
    seconds: f64,
) -> Vec<Complex32> {
    let mut k = snr(snr_db);
    k.push(("gen_seed", J::from(seed)));
    impaired(NbGenerator::new(rate, scene(sc), opt), &k, seconds)
}

fn cells(rate: f64, centre: f64, x: &[Complex32]) -> Vec<J> {
    let d = run_mode(
        &NBIOT,
        &[],
        InputInfo::new(rate, centre, centre).recording(),
        x,
        &[],
    );
    d["cells"].as_array().unwrap().clone()
}

#[test]
fn scenes() {
    let want = [
        (100u64, "standalone", 1u64, 0x1A2Bu64, 8u64),
        (250, "guard band", 2, 0x2B3C, 20),
        (17, "in-band (same PCI)", 2, 0x3C4D, 3),
        (400, "in-band (other PCI)", 1, 0x4D5E, 20),
    ];
    for (sc, w) in want.iter().enumerate() {
        let seed = sc as u64 + 1;
        let opt = NbGenOptions {
            seed,
            ..Default::default()
        };
        let x = gen(1.92e6, sc as u32, opt, 15.0, seed, 1.0);
        let cs = cells(1.92e6, 945e6, &x);
        assert_eq!(cs.len(), 1, "scene {sc}: {cs:?}");
        let c = &cs[0];
        assert_eq!(c["pci"], w.0, "scene {sc}");
        assert_eq!(c["mib"]["operation_mode"], w.1, "scene {sc}");
        assert_eq!(c["mib"]["nrs_ports"], w.2, "scene {sc}");
        assert_eq!(c["sib1"]["tac"], w.3, "scene {sc}: {c}");
        assert_eq!(c["sib1"]["band"], w.4, "scene {sc}");
        let raster = scene(sc as u32).raster_hz;
        assert!(
            near(&c["total_carrier_offset_hz"], raster, 150.0),
            "scene {sc}: {}",
            c["total_carrier_offset_hz"]
        );
    }
}

#[test]
fn offsets_clock_rates() {
    for (rate, off, ppm) in [(1.92e6, 15e3, 20.0), (2e6, -15e3, -20.0), (1e6, 5e3, 0.0)] {
        let mut k = snr(15.0);
        k.push(("gen_offset_hz", J::from(off)));
        k.push(("gen_clock_ppm", J::from(ppm)));
        let x = impaired(
            NbGenerator::new(rate, scene(0), NbGenOptions::default()),
            &k,
            1.0,
        );
        let cs = cells(rate, 945e6, &x);
        assert_eq!(cs.len(), 1, "{rate} {off} {ppm}: {cs:?}");
        assert_eq!(cs[0]["pci"], 100);
        assert!(cs[0]["mib"].is_object(), "{rate} {off} {ppm}");
    }
    // The carrier 100 kHz off the dial: the probe finds it.
    let x = gen(1.92e6, 0, NbGenOptions::default(), 15.0, 1, 1.0);
    let d = run_mode(
        &NBIOT,
        &[],
        InputInfo::new(1.92e6, 945e6, 945.1e6).recording(),
        &x,
        &[],
    );
    assert_eq!(d["cells"][0]["pci"], 100);
}

#[test]
fn weak() {
    let x = gen(
        1.92e6,
        0,
        NbGenOptions {
            seed: 5,
            ..Default::default()
        },
        5.0,
        5,
        1.0,
    );
    let cs = cells(1.92e6, 945e6, &x);
    assert_eq!(cs.len(), 1);
    assert!(cs[0]["mib"].is_object());
    let x = gen(
        1.92e6,
        0,
        NbGenOptions {
            seed: 6,
            ..Default::default()
        },
        8.0,
        6,
        1.0,
    );
    let cs = cells(1.92e6, 945e6, &x);
    assert!(cs[0]["sib1"].is_object());
}

#[test]
fn noise_alone() {
    let mut n = sdrkit_algo_sim::rng::Rng::new(2);
    let x: Vec<Complex32> = (0..1_920_000)
        .map(|_| n.complex_normal(2.0 * 0.05 * 0.05))
        .collect();
    assert!(cells(1.92e6, 945e6, &x).is_empty());
}

/// The per-frame NPBCH phase rotation of later releases: a signal that carries it still decodes to its MIB-NB
/// (and SIB1-NB), 1 and 2 NRS ports.
#[test]
fn npbch_rotation_is_taken_out() {
    for sc in [0u32, 1] {
        let opt = NbGenOptions {
            seed: 9,
            npbch_rotation: true,
            ..Default::default()
        };
        let x = gen(1.92e6, sc, opt, 15.0, 9, 1.0);
        let cs = cells(1.92e6, 945e6, &x);
        assert_eq!(cs.len(), 1, "scene {sc}: {cs:?}");
        assert_eq!(cs[0]["pci"], scene(sc).pci, "scene {sc}");
        assert!(cs[0]["mib"].is_object(), "scene {sc}: {}", cs[0]);
        assert_eq!(cs[0]["mib"]["nrs_ports"], scene(sc).ports, "scene {sc}");
        assert!(cs[0]["sib1"].is_object(), "scene {sc}: {}", cs[0]);
    }
}
