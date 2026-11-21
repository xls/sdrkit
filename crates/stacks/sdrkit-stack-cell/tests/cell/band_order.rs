//! The band order setting: each stack with the values of two rows of `data/cell/band_order.csv`, at a frequency that the
//! rasters of two bands hold.

use crate::common::*;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{InputInfo, StackDescriptor};
use sdrkit_dev_test::Harness;
use sdrkit_stack_cell::lte::gen::GEN_SFN0;
use sdrkit_stack_cell::{CELL, NBIOT, NR};
use serde_json::Value as J;

/// The cells that `desc` finds in its test signal at the radio centre `centre_hz`, with `extra` receiver settings.
fn cells_at(
    desc: &'static StackDescriptor,
    seconds: f64,
    gen: &[CaseSetting],
    centre_hz: f64,
    extra: &[(&str, J)],
) -> Vec<J> {
    let x = Harness::new(desc)
        .seconds(seconds)
        .seed(5)
        .with_settings(gen)
        .generate()
        .expect("the test signal");
    let mut rx = stack_graph(desc, extra);
    let input = InputInfo::new(desc.tuning.rate_hz, centre_hz, centre_hz).recording();
    cells(&run_graph(&mut rx, input, &x, &[262_144]))
}

/// The receiver setting `band_order` with the values of the row `name` of `stack`.
fn order(stack: &str, name: &str) -> (&'static str, J) {
    ("band_order", J::String(band_row(stack, "band_order", name)))
}

#[test]
fn lte_takes_the_band_order_of_two_rows() {
    // 760 MHz is in band 28 and in band 14. Without SIB1 the band is the first one of the order that holds it.
    let gen = [CaseSetting::new(&GEN_SFN0, 4)];
    for (name, band, earfcn) in [("europe", 28, 9230), ("band_number", 14, 5300)] {
        let cs = cells_at(
            &CELL,
            0.45,
            &gen,
            760e6,
            &[order("cell", name), ("system_information", J::Bool(false))],
        );
        assert!(!cs.is_empty(), "{name}");
        for c in &cs {
            assert_eq!((c["band"].as_u64(), c["earfcn"].as_i64()), (Some(band), Some(earfcn)), "{name}");
        }
    }
}

#[test]
fn nbiot_takes_the_band_order_of_two_rows() {
    let gen = [
        CaseSetting::new(&GEN_NOISE, NoiseKind::Snr),
        CaseSetting::new(&GEN_SNR_DB, 20.0),
    ];
    for (name, earfcn) in [("europe", 9230), ("band_number", 5300)] {
        let cs = cells_at(&NBIOT, 1.05, &gen, 760e6, &[order("nbiot", name)]);
        assert!(!cs.is_empty(), "{name}");
        assert!(cs.iter().all(|c| c["earfcn"].as_i64() == Some(earfcn)), "{name}: {cs:?}");
    }
}

#[test]
fn nr_takes_the_band_order_of_two_rows() {
    // The sync raster of n78 lies inside the one of n77.
    for (name, band) in [("europe", 78), ("band_number", 77)] {
        let cs = cells_at(&NR, 0.09, &[], start_hz(&NR), &[order("nr", name)]);
        let placed: Vec<&J> = cs.iter().filter(|c| c["gscn"].as_i64() >= Some(0)).collect();
        assert!(!placed.is_empty(), "{name}");
        assert!(placed.iter().all(|c| c["band"].as_u64() == Some(band)), "{name}: {placed:?}");
    }
}
