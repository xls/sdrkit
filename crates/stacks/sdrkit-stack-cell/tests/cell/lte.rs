//! LTE: the recording against its expected results, and test signal round trips.

use crate::common::*;
use sdrkit_core::{Complex32, Generator, InputInfo, Owner};
use sdrkit_stack_cell::lte::gen::{scene, LteCell, LteGenOptions, LteGenerator};
use sdrkit_stack_cell::lte::search::NON_SI_PDSCH_DECODES;
use sdrkit_stack_cell::CELL;
use serde_json::Value as J;
use std::sync::atomic::Ordering;

fn check_fields(got: &J, want: &J, path: &str) {
    match want {
        J::Object(m) => {
            for (k, v) in m {
                check_fields(&got[k], v, &format!("{path}.{k}"));
            }
        }
        J::Number(n) => {
            let g = got.as_f64().unwrap_or(f64::NAN);
            assert!(
                (g - n.as_f64().unwrap()).abs() < 1e-6,
                "{path}: got {got}, want {want}"
            );
        }
        _ => assert_eq!(got, want, "{path}"),
    }
}

#[test]
fn recording() {
    let Some((rec, exp)) = vector("lte_5mhz_7680k") else {
        return;
    };
    let input = InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz).recording();
    let d = run_mode(&CELL, &[], input, &rec.samples, &[]);
    let cells = d["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 1);
    let c = &cells[0];
    let w = &exp["cells"][0];
    for k in [
        "pci",
        "duplex",
        "cyclic_prefix",
        "frequency_hz",
        "earfcn",
        "band",
        "band_confirmed",
    ] {
        assert_eq!(c[k], w[k], "{k}");
    }
    assert!(
        near(&c["carrier_offset_hz"], 2300.0, 100.0),
        "{}",
        c["carrier_offset_hz"]
    );
    assert!(near(&c["clock_ppm"], 0.0, 1.0));
    for k in [
        "n_rb",
        "bandwidth_mhz",
        "ports",
        "phich_duration",
        "phich_ng_index",
    ] {
        assert_eq!(c["mib"][k], w["mib"][k], "mib.{k}");
    }
    let sfn = c["mib"]["sfn_first_whole_frame"].as_i64().unwrap();
    assert!((60..=61).contains(&sfn), "{sfn}");
    for k in ["sib1", "sib2", "sib3"] {
        check_fields(&c[k], &w[k], k);
    }
    assert_eq!(c["sib4_intra_neighbours"], w["sib4_intra_neighbours"]);
    assert_eq!(c["sib5_inter_frequencies"], w["sib5_inter_frequencies"]);
    assert_eq!(c["sibs_seen"], w["sibs_seen"]);
    assert!(c["sinr_db"].as_f64().unwrap() >= 8.0, "{}", c["sinr_db"]);
    let rsrq = c["rsrq_db"].as_f64().unwrap();
    assert!((-14.0..=-7.0).contains(&rsrq), "{rsrq}");
    assert_eq!(c["si_crc_failures"], 0);
    assert_eq!(NON_SI_PDSCH_DECODES.load(Ordering::Relaxed), 0);
    // The same table whatever the cuts of the input.
    let d2 = run_mode(
        &CELL,
        &[],
        input,
        &rec.samples,
        &random_pieces(17, rec.samples.len()),
    );
    assert_same(&d, &d2);
}

/// The test signal of `cell` at `rate` through the standard impairments of `keys`, `seconds` long.
fn gen(
    rate: f64,
    cell: LteCell,
    opt: LteGenOptions,
    keys: &[(&str, J)],
    seconds: f64,
) -> Vec<Complex32> {
    impaired(LteGenerator::new(rate, cell, opt), keys, seconds)
}

/// The options of most tests: frames from SFN 60.
fn sfn60() -> LteGenOptions {
    LteGenOptions {
        sfn0: 60,
        ..Default::default()
    }
}

/// The cells found in `x`, most recently seen first.
fn cells(rate: f64, centre: f64, x: &[Complex32]) -> Vec<J> {
    let d = run_mode(
        &CELL,
        &[],
        InputInfo::new(rate, centre, centre).recording(),
        x,
        &[],
    );
    d["cells"].as_array().unwrap().clone()
}

#[test]
fn scenes() {
    for sc in 0..4u32 {
        let c = scene(sc, None);
        let rate = 15e3 * sdrkit_stack_cell::lte::phy::fft_size(c.n_rb) as f64;
        let centre = sdrkit_stack_cell::bands::earfcn_to_mhz(c.earfcn) * 1e6;
        let x = gen(rate, c.clone(), sfn60(), &snr(25.0), 0.8);
        let cs = cells(rate, centre, &x);
        assert_eq!(cs.len(), 1, "scene {sc}: {cs:?}");
        let r = &cs[0];
        assert_eq!(r["pci"], c.pci, "scene {sc}");
        assert_eq!(r["mib"]["n_rb"], c.n_rb, "scene {sc}");
        assert_eq!(r["mib"]["ports"], c.ports, "scene {sc}");
        assert_eq!(r["earfcn"], c.earfcn, "scene {sc}");
        assert_eq!(r["band_confirmed"], true, "scene {sc}: {r}");
        assert_eq!(r["sib1"]["enb"], c.enb, "scene {sc}");
        assert_eq!(
            r["sibs_seen"],
            serde_json::json!([1, 2, 3, 4, 5, 6, 7, 8, 24]),
            "scene {sc}"
        );
        // SIB6, SIB7, SIB8 and SIB24: three UMTS carriers, six GSM ARFCNs, a CDMA2000 carrier, two NR carriers.
        let o = r["other_carriers"].as_array().unwrap();
        let of = |s: &str| o.iter().filter(|x| x["system"] == s).count();
        assert_eq!((of("umts_fdd"), of("umts_tdd"), of("gsm"), of("cdma2000_1x"), of("nr")), (2, 1, 6, 1, 2), "scene {sc}: {o:?}");
        let n78 = o.iter().find(|x| x["channel"] == 643_334).unwrap();
        assert_eq!((n78["band"].as_str(), n78["ssb_scs_khz"].as_u64(), n78["priority"].as_u64()), (Some("n78"), Some(30), Some(6)));
        assert_eq!(n78["q_rx_lev_min_dbm"], -128);
        let gsm: Vec<_> = o.iter().filter(|x| x["system"] == "gsm").map(|x| x["channel"].as_u64().unwrap()).collect();
        assert_eq!(gsm, [10, 14, 60, 600, 601, 608], "scene {sc}");
        let cdma = o.iter().find(|x| x["system"] == "cdma2000_1x").unwrap();
        assert_eq!((cdma["channel"].as_u64(), cdma["band"].as_str()), (Some(283), Some("bc0")));
        assert_eq!(cdma["cells"], serde_json::json!([12, 108, 300]));
    }
}

#[test]
fn noise_and_tone() {
    let mut n = sdrkit_algo_sim::rng::Rng::new(3);
    let x: Vec<Complex32> = (0..(7.68e6 * 0.2) as usize)
        .map(|_| n.complex_normal(2.0 * 0.05 * 0.05))
        .collect();
    assert!(cells(7.68e6, 1815e6, &x).is_empty());
    let y: Vec<Complex32> = (0..(7.68e6 * 0.2) as usize)
        .map(|i| Complex32::from_polar(0.3, (i as f64 * 0.013) as f32))
        .collect();
    assert!(cells(7.68e6, 1815e6, &y).is_empty());
}

fn s1() -> LteCell {
    scene(1, None)
}

fn one(rate: f64, centre: f64, x: &[Complex32]) -> J {
    let cs = cells(rate, centre, x);
    assert_eq!(cs.len(), 1, "{cs:?}");
    cs[0].clone()
}

/// `keys` with the carrier moved by `hz`.
fn moved(mut keys: Vec<(&'static str, J)>, hz: f64) -> Vec<(&'static str, J)> {
    keys.push(("gen_offset_hz", J::from(hz)));
    keys
}

#[test]
fn impairments() {
    let all = serde_json::json!([1, 2, 3, 4, 5, 6, 7, 8, 24]);
    // +20 kHz with the centre known.
    let x = gen(7.68e6, s1(), sfn60(), &moved(snr(25.0), 20e3), 0.45);
    let c = one(7.68e6, 1815e6, &x);
    assert_eq!(c["sibs_seen"], all);
    // -50 kHz without the centre.
    let x = gen(7.68e6, s1(), sfn60(), &moved(snr(25.0), -50e3), 0.45);
    let d = run_mode(
        &CELL,
        &[],
        InputInfo::new(7.68e6, 0.0, 0.0).recording(),
        &x,
        &[],
    );
    assert_eq!(d["cells"][0]["pci"], 301, "{d}");
    assert_eq!(d["cells"][0]["sib1"]["enb"], 7001);
    // Sample clock +-20 ppm.
    for ppm in [20.0, -20.0] {
        let mut keys = snr(25.0);
        keys.push(("gen_clock_ppm", J::from(ppm)));
        let x = gen(7.68e6, s1(), sfn60(), &keys, 0.45);
        let c = one(7.68e6, 1815e6, &x);
        assert_eq!(c["sibs_seen"], all, "{ppm} ppm");
    }
    // A start inside a frame.
    let skip = LteGenOptions {
        skip: 12_345,
        ..sfn60()
    };
    let x = gen(7.68e6, s1(), skip, &snr(25.0), 0.45);
    assert_eq!(one(7.68e6, 1815e6, &x)["sibs_seen"], all);
    // Multipath.
    let mut mp = s1();
    mp.multipath = vec![(77, -6.0), (153, -9.0)];
    let x = gen(7.68e6, mp, sfn60(), &snr(25.0), 0.45);
    let c = one(7.68e6, 1815e6, &x);
    assert!(c["sib1"].is_object(), "multipath: {c}");
    // The radio's own rates: 1.92 Msps decodes the MIB only, the others everything.
    let x = gen(1.92e6, s1(), sfn60(), &snr(25.0), 0.25);
    let c = one(1.92e6, 1815e6, &x);
    assert_eq!(c["mib"]["n_rb"], 25);
    assert!(c["sib1"].is_null());
    for rate in [10e6, 15.36e6] {
        let x = gen(rate, s1(), sfn60(), &snr(25.0), 0.45);
        let c = one(rate, 1815e6, &x);
        assert_eq!(c["sibs_seen"], all, "rate {rate}");
    }
}

#[test]
fn threshold() {
    let mut ok = 0;
    for seed in 1..=4u64 {
        let opt = LteGenOptions { seed, ..sfn60() };
        let mut keys = snr(0.0);
        keys.push(("gen_seed", J::from(seed)));
        let x = gen(7.68e6, s1(), opt, &keys, 0.2);
        let cs = cells(7.68e6, 1815e6, &x);
        assert!(cs.iter().all(|c| c["pci"] == 301), "{cs:?}");
        if cs
            .first()
            .is_some_and(|c| c["mib"].is_object() && c["sib1"].is_object())
        {
            ok += 1;
        }
    }
    assert!(ok >= 3, "{ok} of 4");
    let x = gen(7.68e6, s1(), sfn60(), &snr(-12.0), 0.2);
    let cs = cells(7.68e6, 1815e6, &x);
    assert!(cs.iter().all(|c| c["pci"] == 301), "{cs:?}");
}

/// The sum of `parts` with noise of power `noise` (each part at unit power before its gain).
fn sum(parts: &[Vec<Complex32>], noise: f64, seed: u64) -> Vec<Complex32> {
    let mut n = sdrkit_algo_sim::rng::Rng::new(seed);
    let len = parts.iter().map(Vec::len).min().unwrap_or(0);
    (0..len)
        .map(|i| parts.iter().map(|p| p[i]).sum::<Complex32>() + n.complex_normal(noise))
        .collect()
}

#[test]
fn two_cells_on_one_carrier() {
    let a = gen(7.68e6, s1(), sfn60(), &[], 0.45);
    let mut c2 = scene(1, Some(302));
    c2.gain_db = -6.0;
    let b = gen(7.68e6, c2, LteGenOptions { seed: 2, ..sfn60() }, &[], 0.45);
    // 33 dB under the stronger cell in the whole band (the old test's 0.004 noise against RMS 0.2).
    let cs = cells(7.68e6, 1815e6, &sum(&[a, b], 2.0 * 0.02 * 0.02, 9));
    for pci in [301, 302] {
        let c = cs
            .iter()
            .find(|c| c["pci"] == pci)
            .unwrap_or_else(|| panic!("PCI {pci}: {cs:?}"));
        assert!(c["mib"].is_object(), "PCI {pci}");
        assert!(c["sib1"].is_object(), "PCI {pci}: {c}");
    }
}

#[test]
fn paging_is_never_decoded() {
    let before = NON_SI_PDSCH_DECODES.load(Ordering::Relaxed);
    let x = gen(7.68e6, s1(), sfn60(), &snr(30.0), 0.45);
    let c = one(7.68e6, 1815e6, &x);
    assert_eq!(c["si_crc_failures"], 0);
    assert_eq!(NON_SI_PDSCH_DECODES.load(Ordering::Relaxed), before);
}

/// Three sectors of one site on one 5 MHz carrier at the same timing and SFN: the strongest, one 3 dB
/// under with another N_ID2, one 6 dB under sharing the strongest's N_ID2. Every cell must be read to its SIB1.
fn sectors(seconds: f64) -> Vec<Complex32> {
    let parts: Vec<Vec<Complex32>> = [(301u16, 0.0, 1u64), (302, -3.0, 2), (304, -6.0, 3)]
        .iter()
        .map(|&(pci, gain, seed)| {
            let mut c = scene(1, Some(pci));
            c.gain_db = gain;
            c.sector = (pci - 300) as u32;
            let opt = LteGenOptions {
                seed,
                load: false,
                ..sfn60()
            };
            gen(7.68e6, c, opt, &[], seconds)
        })
        .collect();
    sum(&parts, 2.0 * 0.015 * 0.015, 21)
}

#[test]
fn synchronised_sectors() {
    let x = sectors(0.45);
    let cs = cells(7.68e6, 1815e6, &x);
    for pci in [301, 302, 304] {
        let c = cs
            .iter()
            .find(|c| c["pci"] == pci)
            .unwrap_or_else(|| panic!("PCI {pci}: {cs:#?}"));
        assert!(c["mib"].is_object(), "PCI {pci}: {c}");
        assert!(c["sib1"].is_object(), "PCI {pci}: {c}");
        assert_eq!(c["sib1"]["sector"], pci - 300, "PCI {pci}");
    }
    assert!(
        cs.iter()
            .all(|c| [301, 302, 304].contains(&c["pci"].as_u64().unwrap())),
        "nothing invented: {cs:#?}"
    );
}

/// The band search of the stack as a survey: three carriers in bands 20, 8 and 3 seen through a 7.68 Msps radio. Tuned where
/// no carrier is and told to search, the stack asks the host to hop over the raster of the bands of `search_bands` and lists
/// every cell once with its band confirmed; after one pass the command `stay` asks to tune to the strongest cell. The test
/// plays the host.
#[test]
fn band_search() {
    use sdrkit_core::{HostRequest, Output, Receiver};
    let rate = 7.68e6;
    let mut b20 = scene(0, Some(124));
    b20.n_rb = 25;
    let carriers = [b20, scene(2, None), scene(1, None)];
    let freq = |c: &LteCell| sdrkit_stack_cell::bands::earfcn_to_mhz(c.earfcn) * 1e6;
    let mut rx = stack_graph(
        &CELL,
        &[
            ("band_search", J::from("survey")),
            ("search_bands", J::from("20 8 3")),
            ("search_dwell_s", J::from(0.3)),
        ],
    );
    let mut centre = 1000e6;
    rx.configure(&InputInfo::new(rate, centre, centre).recording()).unwrap();
    // A recording cannot be retuned, so the search does not start by itself: the host asks for it.
    command(&mut rx, "search");
    let mut out = Output::default();
    let mut noise = sdrkit_algo_sim::rng::Rng::new(31);
    let piece = (0.05 * rate) as usize;
    // The hop the stack asked for: its centres, the dwell, and the step the host is at.
    let mut hop: Option<(Vec<f64>, f64, usize)> = None;
    let mut stayed: Option<f64> = None;
    let mut visited = 0usize;
    let mut fed_s = 0.0;
    let mut step_s = 0.0;
    let steps = sdrkit_stack_cell::scan::lte_steps(20, rate).len()
        + sdrkit_stack_cell::scan::lte_steps(8, rate).len()
        + sdrkit_stack_cell::scan::lte_steps(3, rate).len();
    while stayed.is_none() {
        assert!(fed_s < 60.0, "no stay after {fed_s} s, {visited} steps");
        // One pass of the survey (the first step, the others, and the move back to the first): then the stay.
        if visited == steps + 1 {
            stay(&mut rx, "");
        }
        // The signal at this centre: every carrier whole inside the sample band, at its offset, plus noise.
        let mut gens: Vec<Box<dyn Generator>> = carriers
            .iter()
            .filter(|c| (freq(c) - centre).abs() + c.n_rb as f64 * 90e3 + 30e3 < rate / 2.0)
            .enumerate()
            .map(|(i, c)| {
                let opt = LteGenOptions {
                    seed: 3 + i as u64,
                    ..sfn60()
                };
                let v = gen_values(&[("gen_offset_hz", J::from(freq(c) - centre))]);
                sdrkit_components::sim::impaired(Box::new(LteGenerator::new(rate, c.clone(), opt)), &v)
                    .expect("the offset")
            })
            .collect();
        let mut buf = vec![Complex32::new(0.0, 0.0); piece];
        let mut acc = vec![Complex32::new(0.0, 0.0); piece];
        loop {
            acc.iter_mut().for_each(|v| *v = noise.complex_normal(2.0 * 0.015 * 0.015));
            for g in gens.iter_mut() {
                g.generate(&mut buf);
                for (a, b) in acc.iter_mut().zip(&buf) {
                    *a += b;
                }
            }
            rx.feed(&acc, &mut out);
            fed_s += 0.05;
            step_s += 0.05;
            let mut next = None;
            for r in out.requests.drain(..) {
                match r {
                    HostRequest::Hop { channels, dwell_s, .. } => {
                        assert!(dwell_s >= 0.3);
                        next = Some(channels[0]);
                        hop = Some((channels, dwell_s, 0));
                    }
                    HostRequest::Retune { tuned_hz, .. } => {
                        hop = None;
                        stayed = Some(tuned_hz);
                        next = Some(tuned_hz);
                    }
                    other => panic!("unexpected request {other:?}"),
                }
            }
            out.clear();
            if next.is_none() {
                if let Some((channels, dwell_s, at)) = hop.as_mut() {
                    if step_s >= *dwell_s {
                        *at = (*at + 1) % channels.len();
                        next = Some(channels[*at]);
                    }
                }
            }
            if let Some(n) = next {
                centre = n;
                step_s = 0.0;
                visited += 1;
                rx.retuned(&InputInfo::new(rate, centre, centre).recording());
                break;
            }
            assert!(fed_s < 60.0, "stuck at {centre}");
        }
    }
    assert_eq!(visited, steps + 2, "one pass, then the stay");
    let s = state(&rx);
    let d = view(&rx, s);
    let cs = d["cells"].as_array().unwrap();
    for c in &carriers {
        let found: Vec<&J> = cs.iter().filter(|x| x["pci"] == c.pci).collect();
        assert_eq!(found.len(), 1, "PCI {} once: {cs:#?}", c.pci);
        assert_eq!(found[0]["earfcn"], c.earfcn, "PCI {}", c.pci);
        assert_eq!(found[0]["band"], c.band, "PCI {}", c.pci);
        assert_eq!(found[0]["band_confirmed"], true, "PCI {}: {}", c.pci, found[0]);
    }
    assert_eq!(cs.len(), 3, "nothing invented: {cs:#?}");
    assert!(carriers.iter().any(|c| (freq(c) - stayed.unwrap()).abs() < 1.0), "stayed on a cell: {stayed:?}");
}

/// The typed rows of the carriers of other systems, the CDMA2000 neighbours and the survey, on the stack's own test signal.
#[test]
fn other_systems_and_the_survey_are_published() {
    use sdrkit_stack_cell::lte::{NeighbourList, OtherSystem};
    use sdrkit_stack_cell::scan::Generation;
    use sdrkit_stack_cell::typed::*;
    let c = scene(0, None);
    let centre = sdrkit_stack_cell::bands::earfcn_to_mhz(c.earfcn) * 1e6;
    let rate = 15e3 * sdrkit_stack_cell::lte::phy::fft_size(c.n_rb) as f64;
    let x = gen(rate, c.clone(), sfn60(), &snr(25.0), 0.8);
    let mut rx = stack_graph(&CELL, &[]);
    run_graph(&mut rx, InputInfo::new(rate, centre, centre).recording(), &x, &[]);
    let r = Owner::results(&rx);
    let others: Vec<_> = CELL_OTHER_CARRIERS.rows::<CellOtherCarrierRow>(r).map(|x| x.1).collect();
    assert_eq!(others.len(), 12, "{others:?}");
    let nr = others.iter().find(|o| o.system == OtherSystem::Nr && o.channel == 643_334).unwrap();
    assert_eq!((nr.band, nr.priority), (Some("n78"), Some(6)));
    assert!(nr.frequency.is_some_and(|f| (f.0 - 3650.01e6).abs() < 1.0), "{nr:?}");
    assert_eq!(nr.ssb_scs.map(|h| h.0), Some(30e3));
    let gsm = others.iter().find(|o| o.system == OtherSystem::Gsm && o.channel == 600).unwrap();
    assert_eq!(gsm.band, Some("dcs1800"));
    let cdma = others.iter().find(|o| o.system == OtherSystem::Cdma2000OneX).unwrap();
    assert_eq!((cdma.channel, cdma.band, cdma.cells), (283, Some("bc0"), 3));
    let mut pn: Vec<i64> = CELL_NEIGHBOURS
        .rows::<CellNeighbourRow>(r)
        .filter(|x| x.1.list == NeighbourList::Cdma2000)
        .map(|x| {
            assert_eq!((x.1.earfcn, x.1.cdma2000_arfcn), (None, Some(283)));
            x.1.pci
        })
        .collect();
    pn.sort_unstable();
    assert_eq!(pn, [12, 108, 300]);
    let survey: Vec<_> = CELL_SURVEY.rows::<SurveyRow>(r).map(|x| x.1).collect();
    assert_eq!(survey.len(), 1);
    let s = &survey[0];
    assert_eq!((s.generation, s.pci, s.channel), (Generation::Fourth, i64::from(c.pci), Some(i64::from(c.earfcn))));
    assert_eq!((s.cell_identity, s.area_code), (Some(i64::from(c.enb * 256 + c.sector)), Some(i64::from(c.tac))));
    let cells: Vec<_> = CELL_CELLS.rows::<CellRow>(r).map(|x| x.1).collect();
    assert_eq!((cells[0].generation, cells[0].name), (Generation::Fourth, s.name));
}
