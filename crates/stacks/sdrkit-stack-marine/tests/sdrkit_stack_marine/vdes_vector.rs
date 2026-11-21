//! The VDES vector.

use crate::common::*;
use sdrkit_core::publish::keys;
use sdrkit_stack_marine::vdes::{Modulation, VdesAsmRow, VdesBurstRow};
use sdrkit_stack_marine::VDES;

#[test]
fn the_vdes_vector_decodes_as_listed() {
    let Some(rec) = vector("vdes_asm_vdeter_400k") else {
        return;
    };
    let exp = expected("vdes_asm_vdeter_400k")["expect"].clone();
    let mut rx = receiver(&VDES, None, &[]);
    configure(rx.as_mut(), rec.rate_hz, rec.centre_hz, rec.centre_hz);
    let ev = run(rx.as_mut(), &rec.samples, 8192);
    let r = rx.results();
    check_events(
        &sdrkit_stack_marine::typed::shape("vdes").results_decl(),
        &ev,
    );
    let asm = rows(r, "asm");
    let bursts = rows(r, "bursts");
    for a in &asm {
        println!("ASM {a:?}");
    }
    for b in &bursts {
        println!("TER {b:?}");
    }
    assert_eq!(asm.len(), 2);
    for (w, g) in exp["asm"].as_array().unwrap().iter().zip(&asm) {
        assert_eq!(g.get(&keys::CHANNEL), w["channel"].as_str());
        assert_eq!(g.at(&VdesAsmRow::BITS), w["bits"].as_i64());
        assert_eq!(g.at(&VdesAsmRow::HEX), w["hex"].as_str());
        if w["ais_type"] == 1 {
            assert_eq!(g.at(&VdesAsmRow::MSG_TYPE), Some(1));
            let mmsi = format!("{:09}", w["ais_mmsi"].as_u64().unwrap());
            assert_eq!(g.at(&VdesAsmRow::MMSI), Some(mmsi.as_str()));
            let (lat, lon) = g.get(&keys::POSITION).expect("a position");
            assert!((lat - 52.5).abs() < 1e-5 && (lon - 5.25).abs() < 1e-5);
            assert_eq!(g.get(&keys::SPEED_KT), Some(12.5));
            assert_eq!(g.get(&keys::COURSE_DEG), Some(80.0));
        } else {
            assert_eq!(g.at(&VdesAsmRow::MSG_TYPE), None);
            assert_eq!(g.at(&VdesAsmRow::MMSI), None);
        }
    }
    assert_eq!(r.points().len(), 1);
    let want = exp["vde_bursts"].as_array().unwrap();
    assert_eq!(bursts.len(), want.len());
    for (w, g) in want.iter().zip(&bursts) {
        let centre = g.at(&VdesBurstRow::CENTRE_HZ).unwrap();
        assert!(
            (centre - w["centre_mhz"].as_f64().unwrap() * 1e6).abs() < 1.0,
            "{g:?}"
        );
        assert_eq!(
            g.at(&VdesBurstRow::BANDWIDTH_HZ),
            Some(w["bw_khz"].as_f64().unwrap() * 1e3)
        );
        let m = g.at(&VdesBurstRow::MODULATION).unwrap();
        assert_eq!(m.name(), w["mod"].as_str().unwrap());
        assert_ne!(m, Modulation::Unknown);
        let d = g.at(&VdesBurstRow::DURATION).unwrap();
        assert!((d - 0.030).abs() <= 0.006, "{g:?}");
        let off = g.get(&keys::OFFSET_HZ).unwrap();
        assert!(off.abs() <= 900.0, "{g:?}");
    }
}
