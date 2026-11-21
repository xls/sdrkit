//! NR: the recording against its expected results, and test signal round trips.

use crate::common::*;
use sdrkit_core::{Complex32, InputInfo};
use sdrkit_stack_cell::nr::gen::{scene, NrCellCfg, NrGenOptions, NrGenerator};
use sdrkit_stack_cell::NR;
use serde_json::Value as J;

#[test]
fn recording() {
    let Some((rec, exp)) = vector("nr_n28_15khz_3840k") else {
        return;
    };
    let input = InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz).recording();
    // The recording is of band n28, whose sync raster n20 shares: the order of the data file takes n28 first.
    let order = [("band_order", J::String(band_row("nr", "band_order", "europe")))];
    let d = run_mode(&NR, &order, input, &rec.samples, &[]);
    let cells = d["cells"].as_array().unwrap();
    assert_eq!(
        cells.len(),
        1,
        "{}",
        serde_json::to_string_pretty(&d).unwrap()
    );
    let c = &cells[0];
    let w = &exp["cells"][0];
    for k in [
        "pci",
        "nid1",
        "nid2",
        "ssb_scs_khz",
        "gscn",
        "band",
        "frequency_hz",
        "sib1_scheduled",
        "coreset0_rbs",
        "coreset0_symbols",
        "coreset0_offset_rb",
        "lmax",
        "strongest_ssb",
    ] {
        assert_eq!(c[k], w[k], "{k}");
    }
    for k in [
        "scs_common_khz",
        "k_ssb",
        "dmrs_type_a_position",
        "coreset0",
        "search_space0",
        "barred",
        "intra_freq_reselection",
    ] {
        assert_eq!(c["mib"][k], w["mib"][k], "mib.{k}");
    }
    let sfn = c["mib"]["sfn"].as_i64().unwrap();
    assert!((100..=107).contains(&sfn), "{sfn}");
    assert!(
        near(&c["carrier_offset_hz"], 3000.0, 300.0),
        "{}",
        c["carrier_offset_hz"]
    );
    assert!(c["sinr_db"].as_f64().unwrap() >= 3.0);
    let beams: Vec<i64> = c["beams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["ssb"].as_i64().unwrap())
        .collect();
    assert_eq!(beams, vec![0, 1, 2, 3]);
    let r = |i: usize| c["beams"][i]["rsrp_dbfs"].as_f64().unwrap();
    assert!((r(2) - r(1)).abs() <= 3.0 && (r(2) - r(3)).abs() <= 3.0);
    assert!(
        r(2) - r(0) >= 1.0 && r(2) - r(0) <= 5.0,
        "{} {}",
        r(2),
        r(0)
    );
    let d2 = run_mode(
        &NR,
        &order,
        input,
        &rec.samples,
        &random_pieces(13, rec.samples.len()),
    );
    assert_same(&d, &d2);
}

/// The cells found in the test signal of `cells` at `rate` through the impairments of `keys`, with the radio at `centre`.
fn run_gen(
    rate: f64,
    centre: f64,
    cells: Vec<NrCellCfg>,
    opt: NrGenOptions,
    keys: &[(&str, J)],
    seconds: f64,
) -> Vec<J> {
    let x = impaired(NrGenerator::new(rate, cells, opt), keys, seconds);
    let d = run_mode(
        &NR,
        &[],
        InputInfo::new(rate, centre, centre).recording(),
        &x,
        &[],
    );
    d["cells"].as_array().unwrap().clone()
}

/// Noise for `snr_db` in the SS/PBCH block, the carrier moved by `offset_hz` and the sample clock `ppm` off.
fn keys(snr_db: f64, offset_hz: f64, ppm: f64) -> Vec<(&'static str, J)> {
    let mut k = snr(snr_db);
    k.push(("gen_offset_hz", J::from(offset_hz)));
    k.push(("gen_clock_ppm", J::from(ppm)));
    k
}

#[test]
fn eight_beams() {
    let (centre, mut cells) = scene(2);
    cells[0].mib.barred = false;
    cells[0].mib.k_ssb = 4;
    cells[0].gains_db = vec![-6.0, -3.0, 0.0, -1.0, -2.0, -3.0, -4.0, -5.0];
    let cs = run_gen(
        7.68e6,
        centre,
        cells,
        NrGenOptions::default(),
        &keys(10.0, 0.0, 0.0),
        0.08,
    );
    assert_eq!(cs.len(), 1, "{cs:?}");
    assert_eq!(cs[0]["pci"], 17);
    assert_eq!(cs[0]["lmax"], 8);
    assert_eq!(cs[0]["beams"].as_array().unwrap().len(), 8);
    assert_eq!(cs[0]["strongest_ssb"], 2);
}

#[test]
fn two_cells_wideband() {
    let (centre, cells) = scene(0);
    let cs = run_gen(
        20e6,
        centre,
        cells,
        NrGenOptions::default(),
        &keys(0.0, 23e3, 10.0),
        0.08,
    );
    for (pci, sfn0) in [(501, 512), (1007, 33)] {
        let c = cs
            .iter()
            .find(|c| c["pci"] == pci)
            .unwrap_or_else(|| panic!("PCI {pci}: {cs:?}"));
        let sfn = c["mib"]["sfn"].as_i64().unwrap();
        assert!((sfn0..sfn0 + 8).contains(&sfn));
    }
}

#[test]
fn second_half_frame_lmax4() {
    let (centre, mut cells) = scene(1);
    cells[0].period_ms = 5;
    cells[0].second_half = true;
    let cs = run_gen(
        10e6,
        centre,
        cells,
        NrGenOptions::default(),
        &keys(3.0, -9e3, 0.0),
        0.08,
    );
    assert_eq!(cs.len(), 1, "{cs:?}");
    assert_eq!(cs[0]["lmax"], 4);
    assert!(cs[0]["mib"].is_object());
}

#[test]
fn wrong_spacing() {
    // A 15 kHz cell placed where only 30 kHz SSBs are searched (n78): nothing.
    let (_, mut cells) = scene(1);
    cells[0].offset_hz = 0.0;
    let cs = run_gen(7.68e6, 3541.44e6, cells, NrGenOptions::default(), &[], 0.08);
    assert!(cs.is_empty(), "{cs:?}");
}

#[test]
fn weak() {
    let (centre, cells) = scene(1);
    let cs = run_gen(
        3.84e6,
        centre,
        cells,
        NrGenOptions {
            seed: 3,
            ..Default::default()
        },
        &[
            ("gen_noise", J::from("snr")),
            ("gen_snr_db", J::from(-5.0)),
            ("gen_seed", J::from(3)),
        ],
        0.08,
    );
    assert_eq!(cs.len(), 1);
    assert!(cs[0]["mib"].is_object());
}

#[test]
fn noise_and_tone() {
    let mut n = sdrkit_algo_sim::rng::Rng::new(1);
    let x: Vec<Complex32> = (0..(7.68e6 * 0.08) as usize)
        .map(|_| n.complex_normal(2.0 * 0.05 * 0.05))
        .collect();
    let d = run_mode(
        &NR,
        &[],
        InputInfo::new(7.68e6, 3541.44e6, 3541.44e6).recording(),
        &x,
        &[],
    );
    assert!(d["cells"].as_array().unwrap().is_empty());
    let y: Vec<Complex32> = (0..(7.68e6 * 0.08) as usize)
        .map(|i| Complex32::from_polar(0.3, (i as f64 * 0.01) as f32))
        .collect();
    let d = run_mode(
        &NR,
        &[],
        InputInfo::new(7.68e6, 3541.44e6, 3541.44e6).recording(),
        &y,
        &[],
    );
    assert!(d["cells"].as_array().unwrap().is_empty());
}
