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
    // The recording was made without the phase of up-conversion.
    let order = [
        ("band_order", J::String(band_row("nr", "band_order", "europe"))),
        ("symbol_phase", J::Bool(false)),
    ];
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

/// The cell of `pci` in a list of cells.
fn cell_of(cs: &[J], pci: i64) -> &J {
    cs.iter()
        .find(|c| c["pci"] == pci)
        .unwrap_or_else(|| panic!("PCI {pci}: {cs:?}"))
}

#[test]
fn sib1_of_the_default_scene() {
    // The default scene at the default rate: the SIB1 of the cell whose CORESET#0 lies in the capture is read; the other
    // cell says what its CORESET#0 needs.
    let (centre, cells) = scene(0);
    let cs = run_gen(20e6, centre, cells, NrGenOptions::default(), &keys(10.0, 0.0, 0.0), 0.08);
    let c = cell_of(&cs, 1007);
    assert_eq!(c["sib1_step"], "decoded", "{c}");
    let s = &c["sib1"];
    assert_eq!(s["parts"], 4);
    let n = &s["networks"][0];
    assert_eq!(n["plmn"], "001-01");
    assert_eq!(n["tac"], 0x0A01);
    assert_eq!(n["nci"], (0x1234u64 << 12) | 1007);
    assert_eq!((n["gnb"].as_u64(), n["cell"].as_u64()), (Some(0x1234), Some(1007)));
    assert_eq!(s["bands"], serde_json::json!([78]));
    assert_eq!((s["carrier_rb"].as_u64(), s["bwp_rb"].as_u64()), (Some(106), Some(106)));
    assert_eq!(s["ssb_period_ms"], 20);
    assert_eq!(s["sibs"].as_array().unwrap().len(), 2);
    assert_eq!(s["sibs"][1]["sib"], 3);
    let other = cell_of(&cs, 501);
    assert_eq!(other["sib1_step"], "too_narrow", "{other}");
    // 48 blocks of 30 kHz from 13.74 MHz below the centre.
    assert!(near(&other["sib1_rate_needed_hz"], 27.48e6, 0.1e6), "{}", other["sib1_rate_needed_hz"]);
}

/// Whether the SIB1 of the n28 scene is read at `rate` through the impairments of `keys`, and the cell.
fn n28_sib1(rate: f64, edit: impl Fn(&mut NrCellCfg), keys: &[(&str, J)], seed: u64) -> J {
    let (centre, mut cells) = scene(1);
    edit(&mut cells[0]);
    let opt = NrGenOptions { seed, ..Default::default() };
    let cs = run_gen(rate, centre, cells, opt, keys, 0.08);
    assert_eq!(cs.len(), 1, "{cs:?}");
    cs[0].clone()
}

#[test]
fn sib1_with_offsets_and_noise() {
    // 15 kHz, CORESET#0 of 3 symbols, DM-RS position 3: carrier 700 Hz off, clock 2 ppm off, 3 dB in the SSB band.
    let c = n28_sib1(10e6, |_| {}, &keys(3.0, 700.0, 2.0), 1);
    assert_eq!(c["sib1_step"], "decoded", "{c}");
    assert_eq!(c["sib1"]["networks"][0]["nci"], (0x1234u64 << 12) | 222);
    assert_eq!(c["sib1"]["bands"], serde_json::json!([28]));
    assert_eq!(c["sib1"]["carrier_rb"], 52);
}

#[test]
fn sib1_other_allocations() {
    // Interleaved blocks, mapping type B of 7 symbols (row 8), a part of the width, aggregation level 4, one version.
    let c = n28_sib1(
        10e6,
        |c| {
            let s = c.sib1.as_mut().unwrap();
            s.interleaved = true;
            s.tdra = 7;
            s.blocks = (2, 21);
            s.level = 4;
            s.rv_cycle = vec![0];
        },
        &keys(10.0, 0.0, 0.0),
        2,
    );
    assert_eq!(c["sib1_step"], "decoded", "{c}");
    // Mapping type A of the least length (row 5), versions 3 then 1.
    let c = n28_sib1(
        10e6,
        |c| {
            let s = c.sib1.as_mut().unwrap();
            s.tdra = 3;
            s.rv_cycle = vec![3, 1];
        },
        &keys(10.0, 0.0, 0.0),
        3,
    );
    assert_eq!(c["sib1_step"], "decoded", "{c}");
    // A SIB1 without the length of the gNB identity: the cell identity is whole, not split.
    let c = n28_sib1(
        10e6,
        |c| c.sib1.as_mut().unwrap().msg.networks[0].gnb_id_length = None,
        &keys(10.0, 0.0, 0.0),
        4,
    );
    assert_eq!(c["sib1_step"], "decoded", "{c}");
    assert!(c["sib1"]["networks"][0]["gnb"].is_null() && c["sib1"]["networks"][0]["nci"].is_u64());
}

#[test]
fn sib1_states_what_stops_it() {
    // 3.84 Msps holds the SS/PBCH block, not CORESET#0 (1.89 MHz below to 2.43 MHz above the block centre).
    let c = n28_sib1(3.84e6, |_| {}, &keys(10.0, 0.0, 0.0), 1);
    assert_eq!(c["sib1_step"], "too_narrow", "{c}");
    assert!(near(&c["sib1_rate_needed_hz"], 4.86e6, 0.05e6), "{}", c["sib1_rate_needed_hz"]);
    assert!(c["sib1"].is_null());
    // A cell that sends no SIB1 in its occasions: no control channel found.
    let c = n28_sib1(10e6, |c| c.sib1 = None, &keys(10.0, 0.0, 0.0), 1);
    assert_eq!(c["sib1_step"], "no_control", "{c}");
    // A MIB that schedules none.
    let (centre, cells) = scene(2);
    let cs = run_gen(7.68e6, centre, cells, NrGenOptions::default(), &keys(10.0, 0.0, 0.0), 0.08);
    assert_eq!(cs[0]["sib1_step"], "not_scheduled");
}

/// A capture too narrow for CORESET#0 says so: the quiet reason and the status name the remedy first, then the cell, the
/// width of CORESET#0 and the rate that reads its SIB1.
#[test]
fn a_capture_too_narrow_for_sib1_says_so() {
    use sdrkit_core::{Output, Owner, Receiver};
    let (centre, cells) = scene(1);
    let rate = 3.84e6;
    let x = impaired(NrGenerator::new(rate, cells, NrGenOptions::default()), &keys(10.0, 0.0, 0.0), 0.3);
    let mut rx = stack_graph(&NR, &[("band_search", J::from("off"))]);
    rx.configure(&InputInfo::new(rate, centre, centre).recording()).unwrap();
    rx.feed(&x, &mut Output::default());
    rx.refresh_results();
    let r = Owner::results(&rx);
    let q = text(r, "quiet_reason").unwrap_or_default();
    let remedy = "raise the rate to 5 Msps to read this cell's operator: ";
    assert!(q.starts_with(&format!("{remedy}PCI 222 sends its SIB1 (the operator) in a CORESET#0 of 24 resource blocks (4.32 MHz)")), "{q}");
    assert!(q.contains("4.86 Msps at this centre"), "{q}");
    assert!(r.status().summary.contains(&q), "{}", r.status().summary);
}

/// The SNR in the SSB band at which SIB1 is read: prints the decodes of 4 seeds per level. Run with `--ignored --nocapture`.
#[test]
#[ignore = "a measurement"]
fn sib1_sensitivity() {
    for snr_db in [4.0, 2.0, 0.0, -1.0, -2.0, -3.0, -4.0, -6.0] {
        let ok = (1..=4u64)
            .filter(|&seed| {
                let mut k = keys(snr_db, 300.0, 1.0);
                k.push(("gen_seed", J::from(seed)));
                n28_sib1(10e6, |_| {}, &k, seed)["sib1_step"] == "decoded"
            })
            .count();
        println!("SNR {snr_db:+.0} dB: SIB1 read in {ok} of 4");
    }
}

#[test]
fn sib1_at_the_stated_sensitivity() {
    for seed in 1..=3u64 {
        let mut k = keys(0.0, 300.0, 1.0);
        k.push(("gen_seed", J::from(seed)));
        let c = n28_sib1(10e6, |_| {}, &k, seed);
        assert_eq!(c["sib1_step"], "decoded", "seed {seed}: {c}");
    }
}

#[test]
fn sib1_is_read_once_and_then_at_a_low_duty() {
    use sdrkit_stack_cell::decoder::Capture;
    use sdrkit_stack_cell::nr::search::{NrSearcher, SIB1_AGAIN_S};
    let (centre, cells) = scene(0);
    let x = impaired(NrGenerator::new(20e6, cells, NrGenOptions::default()), &keys(10.0, 0.0, 0.0), 0.08);
    let mut cap = Capture::new(x, 20e6, centre, 0.08);
    let mut s = NrSearcher::default();
    let ms = |t: std::time::Instant| t.elapsed().as_secs_f64() * 1e3;
    let t = std::time::Instant::now();
    let first = s.search(&cap);
    let cold = ms(t);
    let read = |o: &sdrkit_stack_cell::nr::search::NrSearchOut| {
        o.cells.iter().filter(|c| c.sib1.as_ref().is_some_and(|r| r.payload.is_some())).count()
    };
    assert_eq!(read(&first), 1);
    // The same capture a moment later: the SIB1 of every cell was read or tried, so none is tried.
    cap.end_time = 0.16;
    let t = std::time::Instant::now();
    let second = s.search(&cap);
    let plain = ms(t);
    assert!(second.cells.iter().all(|c| c.sib1.is_none()));
    // After the wait it is read again, with the decoders of the first read.
    cap.end_time = 0.16 + SIB1_AGAIN_S;
    let t = std::time::Instant::now();
    let third = s.search(&cap);
    let warm = ms(t);
    assert_eq!(read(&third), 1);
    // A cell forgotten is tried at once.
    cap.end_time += 0.08;
    s.forget_sib1(1007, ((centre + 4.32e6) / 30e3).round() as i64);
    assert_eq!(read(&s.search(&cap)), 1);
    println!("an 80 ms capture at 20 Msps: search {plain:.0} ms, with the first SIB1 read {cold:.0} ms, with a later read {warm:.0} ms");
}

#[test]
fn sib1_rows_are_published() {
    use sdrkit_core::Owner;
    use sdrkit_stack_cell::typed::{NrDetailRow, NrNetworkRow, NrSibRow, NR_DETAILS, NR_NETWORKS, NR_SIBS};
    let (centre, cells) = scene(0);
    let x = impaired(NrGenerator::new(20e6, cells, NrGenOptions::default()), &keys(10.0, 0.0, 0.0), 0.08);
    let mut rx = stack_graph(&NR, &[]);
    run_graph(&mut rx, InputInfo::new(20e6, centre, centre).recording(), &x, &[]);
    let r = Owner::results(&rx);
    let nets: Vec<_> = NR_NETWORKS.rows::<NrNetworkRow>(r).collect();
    assert_eq!(nets.len(), 1);
    let (key, n) = &nets[0];
    assert!(key.starts_with("1007/30@"), "{key}");
    assert_eq!((n.plmn, n.tac, n.nci, n.gnb, n.cell_id), ("001-01", Some(0x0A01), (0x1234 << 12) | 1007, Some(0x1234), Some(1007)));
    assert_eq!(NR_SIBS.rows::<NrSibRow>(r).count(), 2);
    // The survey: one row per cell, the decoded one with its network, area and identity.
    let survey: Vec<_> = sdrkit_stack_cell::typed::NR_SURVEY.rows::<sdrkit_stack_cell::typed::SurveyRow>(r).collect();
    assert_eq!(survey.len(), 2);
    let (_, s) = survey.iter().find(|(k, _)| k.starts_with("1007/")).unwrap();
    assert_eq!((s.name, s.operator, s.band, s.area_code), ("Test network 5G n77 PCI 1007", Some("Test network"), Some("n77"), Some(0x0A01)));
    assert_eq!(s.cell_identity, Some((0x1234 << 12) | 1007));
    let details: Vec<_> = NR_DETAILS.rows::<NrDetailRow>(r).collect();
    assert_eq!(details.len(), 2);
    for (_, d) in &details {
        if d.cell.starts_with("1007/") {
            assert_eq!(d.sib1, sdrkit_stack_cell::nr::sib1::Sib1Step::Decoded);
            assert_eq!((d.sib1_parts, d.bands, d.bwp_rbs, d.carrier_rbs), (Some(4), Some("78"), Some(106), Some(106)));
            assert_eq!(d.coreset0_width.map(|h| h.0), Some(8.64e6));
        } else {
            assert_eq!(d.sib1, sdrkit_stack_cell::nr::sib1::Sib1Step::TooNarrow);
            assert_eq!(d.coreset0_width.map(|h| h.0), Some(17.28e6));
            assert!(d.rate_needed.is_some_and(|h| (h.0 - 27.48e6).abs() < 0.1e6));
        }
    }
}

#[test]
fn sib1_of_a_case_b_cell() {
    // n5 with 30 kHz blocks, block 0 the strongest: the blocks lie at symbols {4, 8, 16, 20} (Case B, not 2 and 22 for
    // blocks 0 and 3), which sets the frame timing of the
    // occasions. The cell is found, its band named, and its SIB1 read with 300 Hz and 1 ppm off.
    let (centre, cells) = scene(3);
    let cs = run_gen(20e6, centre, cells, NrGenOptions::default(), &keys(6.0, 300.0, 1.0), 0.08);
    let c = cell_of(&cs, 333);
    assert_eq!((c["band"].as_u64(), c["ssb_scs_khz"].as_u64()), (Some(5), Some(30)), "{c}");
    assert_eq!(c["sib1_step"], "decoded", "{c}");
    assert_eq!(c["sib1"]["bands"], serde_json::json!([5]));
    assert!(c["sib1_occasions"].as_u64().is_some_and(|n| n >= 1), "{c}");
    // The same blocks taken as Case C: the timing of the occasions is wrong and no control channel is found.
    let (centre, mut cells) = scene(3);
    cells[0].pattern = sdrkit_stack_cell::nr::phy::SsbPattern::AC;
    let cs = run_gen(20e6, centre, cells, NrGenOptions::default(), &keys(6.0, 0.0, 0.0), 0.08);
    assert_ne!(cell_of(&cs, 333)["sib1_step"], "decoded");
}

#[test]
fn a_try_without_control_channels_says_what_it_searched() {
    let c = n28_sib1(10e6, |c| c.sib1 = None, &keys(10.0, 0.0, 0.0), 1);
    assert_eq!(c["sib1_step"], "no_control", "{c}");
    assert!(c["sib1_occasions"].as_u64().is_some_and(|n| n >= 2), "{c}");
    assert_eq!(c["sib1_pdcch"], 0, "{c}");
}

/// The live recording of a busy n5 carrier at 12 Msps: no capture shows a block other than the one cell, whose MIB
/// decodes, and the stack lists that cell alone, with its operator. Skipped when the recording is not on this machine.
#[test]
#[ignore]
fn a_busy_live_carrier_lists_only_its_cell() {
    use sdrkit_stack_cell::decoder::Capture;
    let Some(rec) = crate::search::live_recording("sdrkit_nr_884.450MHz_12Msps_20261127-161307") else { return };
    let (rate, hz) = (rec.rate_hz, rec.centre_hz);
    let n = (0.08 * rate) as usize;
    let mut s = sdrkit_stack_cell::nr::search::NrSearcher::default();
    let captures = rec.samples.len() / n;
    let mut decoded = 0;
    for k in 0..captures {
        let cap = Capture::new(rec.samples[k * n..(k + 1) * n].to_vec(), rate, hz, ((k + 1) * n) as f64 / rate);
        let out = s.search(&cap);
        let found: Vec<(u16, bool)> = out.cells.iter().map(|f| (f.pci, f.mib.is_some())).collect();
        assert!(found.iter().all(|f| f.0 == 897), "capture {k}: {found:?}");
        decoded += usize::from(found.contains(&(897, true)));
    }
    assert!(10 * decoded >= 9 * captures, "{decoded} of {captures}");
    let mut rx = stack_graph(&NR, &[("band_search", J::from("off"))]);
    run_graph(&mut rx, InputInfo::new(rate, hz, hz).recording(), &rec.samples[..rec.samples.len() / 2], &[]);
    let cs = cells(&state(&rx));
    assert_eq!(cs.len(), 1, "{cs:#?}");
    assert_eq!((&cs[0]["pci"], &cs[0]["sib1"]["networks"][0]["plmn"]), (&J::from(897), &J::from("505-01")), "{}", cs[0]);
}

/// Adds to `x` (at `rate`) one OFDM symbol of 15 kHz subcarriers with a normal prefix whose useful part starts at input
/// sample `useful` (any fraction): `values` on the subcarriers `first`, `first + 1`, ... (from the radio centre).
fn add_symbol(x: &mut [Complex32], rate: f64, useful: f64, first: i64, values: &[Complex32]) {
    let n = (rate / 15e3).round() as usize;
    let cp = 144.0 / 2048.0 * n as f64;
    let base = useful.floor();
    let mut f = vec![Complex32::default(); n];
    for (j, v) in values.iter().enumerate() {
        let k = first + j as i64;
        // The fraction of a sample by which the symbol starts after `base`, as a phase of each subcarrier.
        let a = -std::f64::consts::TAU * k as f64 * (useful - base) / n as f64;
        f[k.rem_euclid(n as i64) as usize] = *v * Complex32::from_polar(1.0, a as f32);
    }
    sdrkit_algo_dsp::fft::Fft::inverse(n).process(&mut f);
    let s = 1.0 / (n as f32).sqrt();
    for i in (useful - cp).ceil() as i64..(useful + n as f64).ceil() as i64 {
        if let Some(v) = usize::try_from(i).ok().and_then(|i| x.get_mut(i)) {
            *v += f[(i - base as i64).rem_euclid(n as i64) as usize] * s;
        }
    }
}

/// A random 16-QAM value of mean power `p`, whose real part has the sign of `sign` when given.
fn qam16(r: &mut sdrkit_algo_sim::rng::Rng, p: f32, sign: Option<f32>) -> Complex32 {
    let level = |r: &mut sdrkit_algo_sim::rng::Rng| [-3.0f32, -1.0, 1.0, 3.0][(r.next_u32() % 4) as usize];
    let re = level(r);
    let re = sign.map_or(re, |s| re.abs() * s.signum());
    Complex32::new(re, level(r)) * (p / 10.0).sqrt()
}

/// A random QPSK value of power `p`.
fn qpsk(r: &mut sdrkit_algo_sim::rng::Rng, p: f32) -> Complex32 {
    let b = |r: &mut sdrkit_algo_sim::rng::Rng| if r.next_u32() & 1 == 0 { 1.0f32 } else { -1.0 };
    Complex32::new(b(r), b(r)) * (p / 2.0).sqrt()
}

/// One real n28 cell at 12 Msps beside traffic of a busy carrier: bursts of QPSK and 16-QAM OFDM data at random times and
/// places over the band, and a burst of broadcast data sent again every 20 ms whose 16-QAM values follow in their signs the
/// PSS and the SSS of PCI 301 on two of its symbols (traffic that passes the sync search), more than a block's width from
/// the real block. Only the real cell is listed, with its MIB, and no capture shows the other as a block.
#[test]
fn traffic_bursts_beside_one_block_list_one_cell() {
    use sdrkit_stack_cell::decoder::Capture;
    use sdrkit_stack_cell::nr::phy::{pss, sss};
    let rate = 12e6;
    let (ssb_hz, mut cells) = scene(1);
    let centre = ssb_hz - 3e6;
    cells[0].offset_hz = 3e6;
    let seconds = 0.5;
    let mut x = impaired(NrGenerator::new(rate, cells, NrGenOptions::default()), &snr(15.0), seconds);
    let p = 4.0 * x.iter().map(|v| v.norm_sqr()).sum::<f32>() / x.len() as f32;
    let mut r = sdrkit_algo_sim::rng::Rng::new(29);
    let sym = 856.25;
    // The broadcast burst: 14 symbols on the raster point nearest 2.4 MHz under the centre, its sync-like symbols 0 and 2.
    let probe = Capture::new(vec![Complex32::default(); 1000], rate, centre, 0.0);
    let (cands, _) = sdrkit_stack_cell::nr::search::candidates(&probe, &[]);
    let fake = cands
        .iter()
        .filter(|c| c.scs == 15)
        .min_by(|a, b| (a.offset_hz + 2.4e6).abs().total_cmp(&(b.offset_hz + 2.4e6).abs()))
        .expect("a raster point under the centre")
        .offset_hz;
    assert!((fake - 3e6).abs() > 240.0 * 15e3, "{fake}");
    let (nid1, nid2) = (100u16, 1u16);
    let (ps, ss) = (pss(nid2), sss(nid1, nid2));
    // The PSS lies on subcarriers -64 to 62 of the block centre, which is a whole number of subcarriers off a bin here.
    let at = (fake / 15e3).round() as i64;
    let first = at - 150;
    let broadcast: Vec<Vec<Complex32>> = (0..14)
        .map(|l| {
            (0..300)
                .map(|j| {
                    let i = j - 86;
                    let seq = match l {
                        0 => Some(&ps),
                        2 => Some(&ss),
                        _ => None,
                    };
                    let sign = seq.and_then(|s| (0..127).contains(&i).then(|| s[i as usize]));
                    qam16(&mut r, p, sign)
                })
                .collect()
        })
        .collect();
    let period = 0.02 * rate;
    let mut t = 0.0073 * rate;
    while t + 14.0 * sym < x.len() as f64 {
        for (l, v) in broadcast.iter().enumerate() {
            add_symbol(&mut x, rate, t + l as f64 * sym + 56.25, first, v);
        }
        t += period;
    }
    // Bursts of traffic: 2 to 14 symbols of QPSK or 16-QAM on 100 to 600 subcarriers, in about a third of the time.
    let mut t = 0.0;
    while t < x.len() as f64 {
        let n = 2 + (r.next_u32() % 13) as usize;
        let width = 100 + (r.next_u32() % 501) as i64;
        let first = -360 + (r.next_u32() % (720 - width as u32)) as i64;
        let qam = r.next_u32() & 1 == 0;
        for l in 0..n {
            let v: Vec<Complex32> = (0..width).map(|_| if qam { qam16(&mut r, p, None) } else { qpsk(&mut r, p) }).collect();
            add_symbol(&mut x, rate, t + l as f64 * sym + 56.25, first, &v);
        }
        t += (3 * n) as f64 * sym * (0.5 + r.uniform());
    }
    // The bursts pass the sync search where the gate of the DMRS is not.
    let mut s = sdrkit_stack_cell::nr::search::NrSearcher::default();
    let n = (0.08 * rate) as usize;
    for k in 0..x.len() / n {
        let cap = Capture::new(x[k * n..(k + 1) * n].to_vec(), rate, centre, ((k + 1) * n) as f64 / rate);
        let out = s.search(&cap);
        let others: Vec<(u16, bool)> = out.cells.iter().filter(|f| f.pci != 222).map(|f| (f.pci, f.mib.is_some())).collect();
        assert!(others.is_empty(), "capture {k}: {others:?}");
        assert!(out.cells.iter().any(|f| f.pci == 222 && f.mib.is_some()), "capture {k}");
    }
    let d = run_mode(&NR, &[], InputInfo::new(rate, centre, centre).recording(), &x, &[]);
    let cs = d["cells"].as_array().unwrap();
    assert_eq!(cs.len(), 1, "{cs:#?}");
    assert_eq!(cs[0]["pci"], 222);
    assert!(cs[0]["mib"].is_object(), "{}", cs[0]);
}

/// A real block whose PBCH does not decode (its data random, so the CRC fails) at 3 dB SNR: the PSS, the SSS and the
/// PBCH DMRS of PCI 301 every 20 ms, the radio clock 3 ppm off. The cell is listed by its sync signals after three
/// captures, and no longer listed once it has been gone for more than [`SYNC_ONLY_KEPT_S`].
#[test]
fn a_block_without_its_mib_is_listed_after_three_captures_and_dropped_when_gone() {
    use sdrkit_stack_cell::nr::phy::{dmrs, dmrs_res, pbch_res, pss, sss, useful_start};
    use sdrkit_core::Receiver;
    use sdrkit_stack_cell::nr::SYNC_ONLY_KEPT_S;
    let rate = 3.84e6;
    let (centre, _) = scene(1);
    let pci = 301u16;
    let (ps, ss) = (pss(pci % 3), sss(pci / 3, pci % 3));
    let r = dmrs(pci, 0);
    let mut grid = [[Complex32::default(); 240]; 4];
    for i in 0..127 {
        grid[0][56 + i] = Complex32::new(ps[i], 0.0);
        grid[2][56 + i] = Complex32::new(ss[i], 0.0);
    }
    for (m, &(k, l)) in dmrs_res(pci).iter().enumerate() {
        grid[l][k] = r[m];
    }
    let mut rng = sdrkit_algo_sim::rng::Rng::new(41);
    for &(k, l) in &pbch_res(pci) {
        grid[l][k] = qpsk(&mut rng, 1.0);
    }
    let noise = |rng: &mut sdrkit_algo_sim::rng::Rng, n: usize| -> Vec<Complex32> { (0..n).map(|_| rng.complex_normal(0.5)).collect() };
    let seconds = 0.5;
    let mut x = noise(&mut rng, (seconds * rate) as usize);
    let period = 0.02 * rate * (1.0 + 3e-6);
    let mut t = 0.0031 * rate;
    while t + 0.005 * rate < x.len() as f64 {
        for (l, row) in grid.iter().enumerate() {
            add_symbol(&mut x, rate, t + useful_start(15, 2 + l) as f64, -120, row);
        }
        t += period;
    }
    let mut rx = stack_graph(&NR, &[]);
    rx.configure(&InputInfo::new(rate, centre, centre).recording()).unwrap();
    let mut out = sdrkit_core::Output::default();
    let n = (0.08 * rate) as usize;
    let mut listed_at = None;
    for (k, piece) in x.chunks(n).enumerate() {
        rx.feed(piece, &mut out);
        out.clear();
        if listed_at.is_none() && cells(&state(&rx)).iter().any(|c| c["pci"] == pci) {
            listed_at = Some(k);
        }
    }
    assert_eq!(listed_at, Some(2), "listed after the capture of that index");
    let cs = cells(&state(&rx));
    assert_eq!(cs.len(), 1, "{cs:#?}");
    assert!(cs[0]["mib"].is_null(), "{}", cs[0]);
    // The cell goes off the air.
    let quiet = noise(&mut rng, n);
    let mut gone_s = 0.0;
    while gone_s < SYNC_ONLY_KEPT_S + 1.0 {
        rx.feed(&quiet, &mut out);
        out.clear();
        gone_s += 0.08;
    }
    let cs = cells(&state(&rx));
    assert!(cs.is_empty(), "{cs:#?}");
}
