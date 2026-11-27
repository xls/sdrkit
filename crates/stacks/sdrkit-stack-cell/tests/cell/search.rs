//! The status of `cell` and `nr` when nothing shows, and the band search of `nr`.

use crate::common::*;
use sdrkit_core::{Complex32, HostRequest, InputInfo, Output, Owner, Receiver};
use sdrkit_stack_cell::nr::gen::{scene, NrGenOptions, NrGenerator};
use sdrkit_stack_cell::{CELL, NR};
use serde_json::Value as J;

/// Complex noise of standard deviation `sigma` a part, `seconds` long at `rate`.
fn noise(rate: f64, seconds: f64, sigma: f64, seed: u64) -> Vec<Complex32> {
    let mut r = sdrkit_algo_sim::rng::Rng::new(seed);
    (0..(rate * seconds) as usize).map(|_| r.complex_normal(2.0 * sigma * sigma)).collect()
}

/// The status summary of a fresh stack of `desc` (band search off) over `x` at `rate`, tuned to `hz`.
fn summary(desc: &sdrkit_core::StackDescriptor, rate: f64, hz: f64, x: &[Complex32]) -> String {
    let mut rx = stack_graph(desc, &[("band_search", J::from("off"))]);
    run_graph(&mut rx, InputInfo::new(rate, hz, hz).recording(), x, &[]);
    Owner::results(&rx).status().summary.clone()
}

#[test]
fn lte_on_noise_says_there_is_no_carrier() {
    let s = summary(&CELL, 2.56e6, 778e6, &noise(2.56e6, 0.6, 0.05, 1));
    assert!(s.contains("no LTE carrier on this frequency"), "{s}");
}

#[test]
fn a_signal_in_the_lowest_steps_of_the_converter_is_too_weak() {
    // An 8-bit radio whose samples move by one or two steps of 1/128 only.
    let x: Vec<Complex32> = noise(2.56e6, 0.6, 0.006, 2)
        .into_iter()
        .map(|v| Complex32::new((v.re * 128.0).round() / 128.0, (v.im * 128.0).round() / 128.0))
        .collect();
    let mut rx = stack_graph(&CELL, &[("band_search", J::from("off"))]);
    run_graph(&mut rx, InputInfo::new(2.56e6, 778e6, 778e6).recording(), &x, &[]);
    let r = Owner::results(&rx);
    assert!(r.status().summary.contains("too weak at the radio"), "{}", r.status().summary);
    let id = r.decl().field_id("quiet_reason").expect("the field");
    let why = match r.fields().value(id) {
        Some(sdrkit_core::Value::Text(t)) => t.clone(),
        other => panic!("{other:?}"),
    };
    assert!(why.contains("too weak at the radio"), "{why}");
    let s = summary(&NR, 7.68e6, 3550e6, &noise(7.68e6, 0.2, 0.05, 3));
    assert!(!s.contains("too weak at the radio"), "{s}");
}

#[test]
fn nr_says_when_the_capture_is_too_narrow_for_its_blocks() {
    let s = summary(&NR, 4e6, 3550e6, &noise(4e6, 0.3, 0.05, 4));
    assert!(s.contains("too narrow for a 30 kHz SSB"), "{s}");
    let s = summary(&NR, 20e6, 3550e6, &noise(20e6, 0.1, 0.05, 5));
    assert!(s.contains("no SSB on the"), "{s}");
    assert!(!s.contains("too narrow"), "{s}");
}

/// Tuned at the edge of n28 with nothing there and told to search, `nr` hops over the sync raster of n28, finds the cell of scene 1 and stays
/// on it after the pass. The test plays the host.
#[test]
fn nr_band_search_finds_and_stays() {
    let rate = 7.68e6;
    let (cell_hz, cells) = scene(1);
    let mut rx = stack_graph(&NR, &[("search_dwell_s", J::from(0.1))]);
    let mut centre = 760.0e6;
    rx.configure(&InputInfo::new(rate, centre, centre).recording()).unwrap();
    command(&mut rx, "search");
    let mut out = Output::default();
    let piece = (0.02 * rate) as usize;
    let mut hop: Option<(Vec<f64>, f64, usize)> = None;
    let mut stayed = None;
    let (mut fed, mut step) = (0.0, 0.0);
    let gen_at = |c: f64| {
        let v = gen_values(&[("gen_offset_hz", J::from(cell_hz - c))]);
        sdrkit_components::sim::impaired(Box::new(NrGenerator::new(rate, cells.clone(), NrGenOptions::default())), &v)
            .expect("the offset")
    };
    let mut g = gen_at(centre);
    let mut r = sdrkit_algo_sim::rng::Rng::new(9);
    let mut buf = vec![Complex32::default(); piece];
    while stayed.is_none() {
        assert!(fed < 40.0, "no stay after {fed} s");
        g.generate(&mut buf);
        let near = (cell_hz - centre).abs() < rate / 2.0 - 2.2e6;
        for v in buf.iter_mut() {
            if !near {
                *v = Complex32::default();
            }
            *v += r.complex_normal(2.0 * 0.01 * 0.01);
        }
        rx.feed(&buf, &mut out);
        fed += 0.02;
        step += 0.02;
        let mut next = None;
        for q in out.requests.drain(..) {
            match q {
                HostRequest::Hop { channels, dwell_s, .. } => {
                    next = Some(channels[0]);
                    hop = Some((channels, dwell_s, 0));
                }
                // A step with a sync signal is searched longer: the radio goes back there.
                HostRequest::Retune { tuned_hz, reason } => {
                    hop = None;
                    if reason.contains("stays on") {
                        stayed = Some(tuned_hz);
                    }
                    next = Some(tuned_hz);
                }
                other => panic!("unexpected request {other:?}"),
            }
        }
        out.clear();
        if next.is_none() {
            if let Some((ch, dwell, at)) = hop.as_mut() {
                if step >= *dwell {
                    *at = (*at + 1) % ch.len();
                    next = Some(ch[*at]);
                }
            }
        }
        if let Some(n) = next {
            centre = n;
            step = 0.0;
            g = gen_at(centre);
            rx.retuned(&InputInfo::new(rate, centre, centre).recording());
        }
    }
    let pcis: Vec<i64> = cells_of(&rx);
    assert_eq!(pcis, [222]);
    // At the cell the status says where the search stayed.
    let x = impaired(NrGenerator::new(rate, cells.clone(), NrGenOptions::default()), &snr(15.0), 0.2);
    rx.feed(&x, &mut out);
    let s = Owner::results(&rx).status().summary.clone();
    assert!(s.starts_with("Staying on PCI 222"), "{s}");
    assert!((stayed.unwrap() - cell_hz).abs() < 10e3, "{stayed:?} for {cell_hz}");
}

/// A known carrier of the setting `search_first` is the first step of the search, at its own centre; in auto the search
/// stays on its cell there and visits no other step.
#[test]
fn a_known_carrier_is_searched_first() {
    let (cell_hz, _) = scene(1);
    // 1900 MHz lies outside n28: left out.
    let first = format!("{} 1900", cell_hz / 1e6);
    let mut rx = stack_graph(&NR, &[("search_bands", J::from("28")), ("search_first", J::from(first.as_str()))]);
    let centre = 760e6;
    rx.configure(&InputInfo::new(12e6, centre, centre).recording()).unwrap();
    command(&mut rx, "search");
    let mut out = Output::default();
    rx.feed(&noise(12e6, 0.01, 0.01, 2), &mut out);
    let hop = out.requests.iter().find_map(|q| match q {
        HostRequest::Hop { channels, .. } => Some(channels.clone()),
        _ => None,
    });
    let hop = hop.expect("a hop");
    assert_eq!(hop[0], cell_hz, "{hop:?}");
    assert_eq!(hop.len(), sdrkit_stack_cell::scan::nr_steps(28, 12e6, &[], &[]).len() + 1);
    // The radio moves there: the cell is found and the search stays on it.
    rx.retuned(&InputInfo::new(12e6, cell_hz, cell_hz).recording());
    out.clear();
    rx.feed(&n28_from(cell_hz, 0.3), &mut out);
    assert_eq!(cells_of(&rx), [222]);
    assert!(out.requests.iter().any(|q| matches!(q, HostRequest::Retune { tuned_hz, .. } if *tuned_hz == cell_hz)), "{:?}", out.requests);
    // The search is over: no step and no reason why no cell shows.
    rx.refresh_results();
    let r = Owner::results(&rx);
    assert_eq!((value(r, "search_step"), value(r, "search_steps")), (None, None));
    assert_eq!(text(r, "quiet_reason"), None);
}

/// The n28 cell of scene 1 as a radio centred at `centre` sees it at 12 Msps, `seconds` long at 15 dB SNR; only noise
/// when the SSB is not whole in the capture.
fn n28_from(centre: f64, seconds: f64) -> Vec<Complex32> {
    let rate = 12e6;
    let (cell_hz, cells) = scene(1);
    let mut keys = snr(15.0);
    keys.push(("gen_offset_hz", J::from(cell_hz - centre)));
    let mut x = impaired(NrGenerator::new(rate, cells, NrGenOptions::default()), &keys, seconds);
    if (cell_hz - centre).abs() > rate / 2.0 - 2.2e6 {
        x = noise(rate, seconds, 0.01, 5);
    }
    x
}

/// The frequencies of the rows of the cells table.
fn cell_freqs(rx: &sdrkit_graph::StackGraph) -> Vec<f64> {
    cells(&state(rx)).iter().filter_map(|c| c["frequency_hz"].as_f64()).collect()
}

/// One SSB seen from two radio centres that both hold it is one cell at its own frequency.
#[test]
fn nr_one_ssb_seen_from_two_centres_is_one_cell() {
    let (cell_hz, _) = scene(1);
    let mut rx = stack_graph(&NR, &[("band_search", J::from("off"))]);
    let (a, b) = (cell_hz - 3e6, cell_hz + 3e6);
    rx.configure(&InputInfo::new(12e6, a, a).recording()).unwrap();
    rx.feed(&n28_from(a, 0.4), &mut Output::default());
    rx.retuned(&InputInfo::new(12e6, b, b).recording());
    rx.feed(&n28_from(b, 0.4), &mut Output::default());
    rx.finish(&mut Output::default());
    assert_eq!(cells_of(&rx), [222]);
    let f = cell_freqs(&rx);
    assert!((f[0] - cell_hz).abs() < 10e3, "{f:?} for {cell_hz}");
}

/// After a move the engine passes only samples of the new centre: the cell of the old centre, outside the new capture, stays
/// one cell at its own frequency.
#[test]
fn nr_a_move_away_from_a_cell_gives_no_second_cell() {
    let (cell_hz, _) = scene(1);
    let mut rx = stack_graph(&NR, &[("band_search", J::from("off"))]);
    // Two neighbouring steps of a band search at 12 Msps: the cell is 1.48 MHz above the first and outside the second.
    let a = cell_hz - 1.48e6;
    let b = a + 7.2e6;
    rx.configure(&InputInfo::new(12e6, a, a).recording()).unwrap();
    rx.feed(&n28_from(a, 0.5), &mut Output::default());
    rx.retuned(&InputInfo::new(12e6, b, b).recording());
    rx.feed(&n28_from(b, 0.4), &mut Output::default());
    rx.finish(&mut Output::default());
    assert_eq!(cells_of(&rx), [222], "{:?}", cell_freqs(&rx));
    let f = cell_freqs(&rx);
    assert!((f[0] - cell_hz).abs() < 10e3, "{f:?} for {cell_hz}");
}

/// The same for LTE: a 1.4 MHz carrier 2 MHz above the old centre and 1 MHz below the new one, with only samples of the
/// new centre after the move, is one cell at its own frequency.
#[test]
fn lte_a_move_gives_no_second_cell() {
    let rate = 12e6;
    let c = sdrkit_stack_cell::lte::gen::scene(2, None);
    let cell_hz = sdrkit_stack_cell::bands::earfcn_to_mhz(c.earfcn) * 1e6;
    let (a, b) = (cell_hz - 2e6, cell_hz + 1e6);
    let from = |centre: f64, seconds: f64| {
        let mut keys = snr(20.0);
        keys.push(("gen_offset_hz", J::from(cell_hz - centre)));
        impaired(sdrkit_stack_cell::lte::gen::LteGenerator::new(rate, c.clone(), Default::default()), &keys, seconds)
    };
    let mut rx = stack_graph(&CELL, &[("band_search", J::from("off"))]);
    rx.configure(&InputInfo::new(rate, a, a).recording()).unwrap();
    rx.feed(&from(a, 0.6), &mut Output::default());
    assert_eq!(cells_of(&rx), [7], "the cell at the old centre");
    rx.retuned(&InputInfo::new(rate, b, b).recording());
    rx.feed(&from(b, 0.6), &mut Output::default());
    rx.finish(&mut Output::default());
    assert_eq!(cells_of(&rx), [7], "{:?}", cell_freqs(&rx));
    let f = cell_freqs(&rx);
    assert!((f[0] - cell_hz).abs() < 10e3, "{f:?} for {cell_hz}");
}

fn cells_of(rx: &sdrkit_graph::StackGraph) -> Vec<i64> {
    cells(&state(rx)).iter().filter_map(|c| c["pci"].as_i64()).collect()
}

/// On live samples a stack whose tuned frequency shows no sync signal in its first look starts the band search by itself: a
/// hop over the steps of the band of the tuned frequency, well within a second of signal. A recording never starts one.
#[test]
fn a_quiet_live_frequency_starts_the_search() {
    for live in [true, false] {
        let rate = 2.56e6;
        let mut rx = stack_graph(&CELL, &[]);
        let input = InputInfo::new(rate, 778e6, 778e6);
        rx.configure(&if live { input } else { input.recording() }).unwrap();
        let mut out = Output::default();
        let mut hops = Vec::new();
        let mut fed_s = 0.0;
        let mut first_hop_s = None;
        for p in noise(rate, 2.5, 0.05, 7).chunks(65536) {
            // Live samples arrive at about their rate; the helper thread searches as they come.
            if live {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            rx.feed(p, &mut out);
            fed_s += p.len() as f64 / rate;
            if first_hop_s.is_none() && out.requests.iter().any(|h| matches!(h, HostRequest::Hop { .. })) {
                first_hop_s = Some(fed_s);
            }
            hops.append(&mut out.requests);
            out.clear();
        }
        rx.finish(&mut out);
        hops.append(&mut out.requests);
        let mut steps = sdrkit_stack_cell::scan::lte_steps(28, rate, &[], &[]);
        // The tuned frequency was searched already.
        steps.retain(|c| (c - 778e6).abs() > 1.0);
        if live {
            assert!(
                hops.iter().any(|h| matches!(h, HostRequest::Hop { channels, .. } if *channels == steps)),
                "{hops:?}"
            );
            assert!(first_hop_s.is_some_and(|t| t < 0.8), "{first_hop_s:?}");
        } else {
            assert!(hops.is_empty(), "{hops:?}");
        }
    }
}

/// On live samples in auto, a cell found at the tuned frequency keeps the stack there: no band search starts, and the status
/// names the cell and the setting that visits all bands.
#[test]
fn auto_stays_on_a_live_cell_and_says_so() {
    let rate = 7.68e6;
    let (cell_hz, scene_cells) = scene(1);
    let x = impaired(NrGenerator::new(rate, scene_cells, NrGenOptions::default()), &snr(15.0), 1.0);
    let mut rx = stack_graph(&NR, &[]);
    rx.configure(&InputInfo::new(rate, cell_hz, cell_hz)).unwrap();
    let mut out = Output::default();
    for p in x.chunks(65536) {
        std::thread::sleep(std::time::Duration::from_millis(8));
        rx.feed(p, &mut out);
        assert!(!out.requests.iter().any(|q| matches!(q, HostRequest::Hop { .. })), "{:?}", out.requests);
        out.clear();
    }
    rx.finish(&mut out);
    let r = Owner::results(&rx);
    let s = &r.status().summary;
    assert!(s.starts_with("Staying on PCI 222 at 795.85 MHz, the cell found (band_search survey visits all bands)"), "{s}");
    assert_eq!(value(r, "search_step"), None);
    assert_eq!(text(r, "quiet_reason"), None);
}

/// A cell decoded far below full scale (-53 dBFS, as a 12-bit radio at its automatic gain gives): the level is no reason
/// why nothing shows, so the status and the quiet reason say nothing of it.
#[test]
fn a_low_level_with_a_cell_decoded_gives_no_reason() {
    let rate = 7.68e6;
    let (cell_hz, scene_cells) = scene(1);
    let mut x = impaired(NrGenerator::new(rate, scene_cells, NrGenOptions::default()), &snr(15.0), 0.6);
    let p = x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
    let k = (10f64.powf(-5.3) / p).sqrt() as f32;
    x.iter_mut().for_each(|v| *v *= k);
    for desc in [&NR, &CELL] {
        let mut rx = stack_graph(desc, &[("band_search", J::from("off"))]);
        run_graph(&mut rx, InputInfo::new(rate, cell_hz, cell_hz).recording(), &x, &[]);
        let r = Owner::results(&rx);
        let s = &r.status().summary;
        assert!(!s.contains("weak at the radio") && !s.contains("dBFS"), "{s}");
        if desc.id == "nr" {
            assert_eq!(cells_of(&rx), [222]);
            assert_eq!(text(r, "quiet_reason"), None);
        }
    }
}

/// A reason why no cell shows belongs to the frequency and the capture it was found on: a move clears it, and a cell listed
/// after the move leaves none.
#[test]
fn a_quiet_reason_clears_on_a_move_and_on_a_cell() {
    let (cell_hz, _) = scene(1);
    let away = cell_hz - 20e6;
    let mut rx = stack_graph(&NR, &[("band_search", J::from("off"))]);
    rx.configure(&InputInfo::new(12e6, away, away).recording()).unwrap();
    let mut out = Output::default();
    rx.feed(&noise(12e6, 0.2, 0.01, 3), &mut out);
    assert!(text(Owner::results(&rx), "quiet_reason").is_some_and(|q| q.starts_with("no SSB")));
    rx.retuned(&InputInfo::new(12e6, cell_hz, cell_hz).recording());
    rx.feed(&noise(12e6, 0.01, 0.01, 4), &mut out);
    rx.refresh_results();
    assert_eq!(text(Owner::results(&rx), "quiet_reason"), None, "the reason of the old frequency");
    rx.feed(&n28_from(cell_hz, 0.3), &mut out);
    rx.refresh_results();
    assert_eq!(cells_of(&rx), [222]);
    let r = Owner::results(&rx);
    assert_eq!(text(r, "quiet_reason"), None);
    assert!(!r.status().summary.contains("no SSB"), "{}", r.status().summary);
}

/// A start in Australia without a setting of its own: the host sets the country's band order, its known carriers and the
/// ranges of its bands. The band search of a quiet live frequency visits the other known carriers first, then steps through every
/// band of that order, in that order, and leaves out n78 below 3400 MHz.
#[test]
fn a_default_start_in_australia_searches_the_bands_of_the_country() {
    use sdrkit_core::SettingId;
    use sdrkit_stack_cell::decoder::{FrequencyList, RangeList};
    let data = sdrkit_presets::Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap();
    let au = sdrkit_presets::Region::new("AU").unwrap();
    let rate = 12e6;
    type Steps = fn(u16, f64, &[(f64, f64)], &[f64]) -> Vec<f64>;
    for (desc, want, steps) in [
        (&CELL, &[28u16, 5, 26, 8, 3, 1, 7, 40][..], sdrkit_stack_cell::scan::lte_steps as Steps),
        (&NR, &[5, 28, 78, 1, 3, 7, 8, 40][..], sdrkit_stack_cell::scan::nr_steps),
    ] {
        let p = data.preset(desc.id, &au, &sdrkit_presets::Options::default()).unwrap();
        let extra: Vec<(&str, J)> = p
            .settings
            .iter()
            .map(|(k, v)| match v {
                sdrkit_core::Value::Text(t) => (k.as_str(), J::from(t.as_str())),
                other => panic!("{k}: {other:?}"),
            })
            .collect();
        let text = |key: &str| extra.iter().find(|(k, _)| *k == key).map(|(_, v)| v.as_str().unwrap_or_default().to_owned());
        let order = text("band_order").expect("a band order");
        let mut first = FrequencyList::read(SettingId(0), &text("search_first").expect("known carriers")).unwrap().hz;
        // The tuned frequency was searched already.
        first.retain(|f| (f - p.frequency_hz).abs() > 1.0);
        let ranges = RangeList::read(SettingId(0), &text("search_ranges").expect("band ranges")).unwrap().hz;
        let mut rx = stack_graph(desc, &extra);
        rx.configure(&InputInfo::new(rate, p.frequency_hz, p.frequency_hz)).unwrap();
        let mut out = Output::default();
        let mut hop = None;
        for i in 0..1000 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            rx.feed(&noise(rate, 0.01, 0.05, i), &mut out);
            hop = hop.or(out.requests.iter().find_map(|q| match q {
                HostRequest::Hop { channels, .. } => Some(channels.clone()),
                _ => None,
            }));
            out.clear();
            if hop.is_some() {
                break;
            }
        }
        let hop = hop.unwrap_or_else(|| panic!("{}: no band search ({order:?})", desc.id));
        // The known carriers first, then every step of each band of the order, without what an earlier band covers.
        assert_eq!(hop[..first.len()], first[..], "{}: the known carriers first", desc.id);
        let mut taken: Vec<f64> = Vec::new();
        for b in want {
            let own = steps(*b, rate, &ranges, &taken);
            let missing: Vec<f64> = own.iter().copied().filter(|c| !hop.iter().any(|h| (h - c).abs() < 1.0)).collect();
            assert!(missing.is_empty(), "{}: band {b} misses {missing:?}", desc.id);
            taken.extend(own);
        }
        assert_eq!(hop[first.len()], steps(want[0], rate, &ranges, &[])[0], "{}: the first band first", desc.id);
        assert!(!hop.iter().any(|h| (3300e6..3400e6).contains(h)), "{}: a step in 3300 to 3400 MHz", desc.id);
    }
}

/// Plays the host of `rx` at `rate` for up to `seconds` of noise from `centre`: follows each hop with its dwell and each
/// retune when `follow`, and then stops after the first whole pass. Returns the requests seen and the frequency where the
/// radio ended.
fn host_on_noise(
    rx: &mut sdrkit_graph::StackGraph,
    rate: f64,
    mut centre: f64,
    seconds: f64,
    follow: bool,
) -> (Vec<HostRequest>, f64) {
    let mut out = Output::default();
    let piece = (0.05 * rate) as usize;
    let mut r = sdrkit_algo_sim::rng::Rng::new(11);
    let mut buf = vec![Complex32::default(); piece];
    let mut hop: Option<(Vec<f64>, f64, usize)> = None;
    let mut seen = Vec::new();
    let (mut fed, mut step) = (0.0, 0.0);
    while fed < seconds && !(follow && value(Owner::results(&*rx), "search_passes").unwrap_or(0.0) >= 1.0) {
        buf.iter_mut().for_each(|v| *v = r.complex_normal(2.0 * 0.05 * 0.05));
        rx.feed(&buf, &mut out);
        fed += 0.05;
        step += 0.05;
        let mut next = None;
        for q in out.requests.drain(..) {
            match &q {
                HostRequest::Hop { channels, dwell_s, .. } => {
                    next = Some(channels[0]);
                    hop = Some((channels.clone(), *dwell_s, 0));
                }
                HostRequest::Retune { tuned_hz, .. } => {
                    hop = None;
                    next = Some(*tuned_hz);
                }
                _ => {}
            }
            seen.push(q);
        }
        out.clear();
        if !follow {
            continue;
        }
        if next.is_none() {
            if let Some((ch, dwell, at)) = hop.as_mut() {
                if step >= *dwell {
                    *at = (*at + 1) % ch.len();
                    next = Some(ch[*at]);
                }
            }
        }
        if let Some(n) = next {
            centre = n;
            step = 0.0;
            rx.retuned(&InputInfo::new(rate, centre, centre).recording());
        }
    }
    (seen, centre)
}

/// A cell in the tables from before the search is not one to stay on: a pass that finds nothing goes on searching.
#[test]
fn a_pass_without_a_cell_does_not_stay_on_an_old_one() {
    let rate = 7.68e6;
    let c = sdrkit_stack_cell::lte::gen::scene(0, None);
    let hz = sdrkit_stack_cell::bands::earfcn_to_mhz(c.earfcn) * 1e6;
    let g = sdrkit_stack_cell::lte::gen::LteGenerator::new(rate, c, Default::default());
    let x = impaired(g, &snr(25.0), 0.3);
    let mut rx = stack_graph(&CELL, &[("search_bands", J::from("20")), ("search_dwell_s", J::from(0.2))]);
    rx.configure(&InputInfo::new(rate, hz, hz).recording()).unwrap();
    rx.feed(&x, &mut Output::default());
    assert_eq!(cells_of(&rx), [123], "the old cell");
    rx.retuned(&InputInfo::new(rate, 1000e6, 1000e6).recording());
    // The retune reaches the search with the next samples; a search asked for before them would end at it.
    rx.feed(&noise(rate, 0.05, 0.05, 12), &mut Output::default());
    command(&mut rx, "search");
    let steps = sdrkit_stack_cell::scan::lte_steps(20, rate, &[], &[]).len();
    let (seen, _) = host_on_noise(&mut rx, rate, 1000e6, 12.0 * steps as f64, true);
    assert!(seen.iter().any(|q| matches!(q, HostRequest::Hop { .. })), "{seen:?}");
    assert!(!seen.iter().any(|q| matches!(q, HostRequest::Retune { .. })), "no stay: {seen:?}");
    let r = sdrkit_core::Owner::results(&rx);
    assert!(
        value(r, "search_passes").unwrap_or(0.0) >= 1.0,
        "{} {:?} {seen:?}",
        r.status().summary,
        value(r, "search_passes")
    );
}

/// A host that does not move the radio ends the search after a few dwells, and the status says why.
#[test]
fn a_search_the_radio_does_not_follow_ends() {
    let rate = 7.68e6;
    let mut rx = stack_graph(&CELL, &[("search_bands", J::from("20")), ("search_dwell_s", J::from(0.2))]);
    rx.configure(&InputInfo::new(rate, 800e6, 800e6).recording()).unwrap();
    command(&mut rx, "search");
    let (seen, _) = host_on_noise(&mut rx, rate, 800e6, 4.0, false);
    assert!(seen.iter().any(|q| matches!(q, HostRequest::Hop { .. })), "{seen:?}");
    let s = Owner::results(&rx).status().summary.clone();
    assert!(s.contains("did not move"), "{s}");
    assert!(!s.contains("Searching"), "{s}");
}

/// The text of the results field `key` of `rx`.
fn field_text(rx: &sdrkit_graph::StackGraph, key: &str) -> Option<String> {
    let r = Owner::results(rx);
    let id = r.decl().field_id(key)?;
    match r.fields().value(id) {
        Some(sdrkit_core::Value::Text(t)) => Some(t.clone()),
        _ => None,
    }
}

/// A 20 MHz carrier at 7.68 Msps: the cell is listed with what the centre gives (PCI, bandwidth from the MIB), and the
/// status and `quiet_reason` say the operator needs about 18 Msps. On a live radio in auto the stack asks for 20 Msps at
/// the carrier centre, once.
#[test]
fn a_carrier_wider_than_the_capture_says_what_it_needs() {
    let rate = 7.68e6;
    let c = sdrkit_stack_cell::lte::gen::scene(3, None);
    let hz = sdrkit_stack_cell::bands::earfcn_to_mhz(c.earfcn) * 1e6;
    let pci = i64::from(c.pci);
    let g = sdrkit_stack_cell::lte::gen::LteGenerator::new(rate, c, Default::default());
    let x = impaired(g, &snr(25.0), 0.6);
    let mut rx = stack_graph(&CELL, &[("band_search", J::from("off"))]);
    run_graph(&mut rx, InputInfo::new(rate, hz, hz).recording(), &x, &[]);
    assert_eq!(cells_of(&rx), [pci]);
    let s = Owner::results(&rx).status().summary.clone();
    assert!(s.contains("20 MHz carrier") && s.contains("about 18 Msps"), "{s}");
    let why = field_text(&rx, "quiet_reason").unwrap_or_default();
    assert!(why.contains("about 18 Msps"), "{why}");
    // Live, in auto: one request for the rate.
    let mut rx = stack_graph(&CELL, &[]);
    rx.configure(&InputInfo::new(rate, hz, hz)).unwrap();
    let mut out = Output::default();
    let mut asked = Vec::new();
    for p in x.chunks(65536) {
        std::thread::sleep(std::time::Duration::from_millis(20));
        rx.feed(p, &mut out);
        asked.append(&mut out.requests);
        out.clear();
    }
    rx.finish(&mut out);
    asked.append(&mut out.requests);
    let tunes: Vec<&HostRequest> = asked.iter().filter(|q| matches!(q, HostRequest::Tune { .. })).collect();
    assert_eq!(tunes.len(), 1, "{asked:?}");
    match tunes[0] {
        HostRequest::Tune { rate_hz, freq_hz, .. } => {
            assert_eq!(*rate_hz, Some(20e6));
            assert!((freq_hz.unwrap() - hz).abs() < 50e3);
        }
        _ => unreachable!(),
    }
}

/// Noise, and noise with two steady tones (spurs of a radio): no LTE or NR cell is ever listed. 50 captures of LTE and 50 of
/// NR: the false listings counted are 0.
#[test]
fn noise_and_spurs_list_no_cell() {
    for (desc, rate, hz, seconds) in [(&CELL, 7.68e6, 2140e6, 10.0), (&NR, 7.68e6, 2140e6, 4.0)] {
        for tones in [false, true] {
            let mut x = noise(rate, seconds, 0.05, 21);
            if tones {
                for (i, v) in x.iter_mut().enumerate() {
                    let t = i as f64 / rate;
                    for f in [1.2e6, -2.3e6] {
                        let ph = std::f64::consts::TAU * f * t;
                        *v += Complex32::new(0.2 * ph.cos() as f32, 0.2 * ph.sin() as f32);
                    }
                }
            }
            let mut rx = stack_graph(desc, &[("band_search", J::from("off"))]);
            run_graph(&mut rx, InputInfo::new(rate, hz, hz).recording(), &x, &[]);
            assert!(cells_of(&rx).is_empty(), "{} tones {tones}: {:?}", desc.id, cells_of(&rx));
        }
    }
}

/// The recording of a live n5 cell (PCI 897 at 884.45 MHz, 7.68 Msps): only that cell is listed, without the "sync only"
/// cells that its SSB gave at the neighbouring raster points. Skipped when the recording is not on this machine.
#[test]
fn live_n5_recording_lists_one_cell() {
    let Some(rec) = live_recording("sdrkit_nr_884.720MHz_7.68Msps_20261126-132757") else { return };
    let mut rx = stack_graph(&NR, &[("band_search", J::from("off"))]);
    run_graph(&mut rx, InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz).recording(), &rec.samples, &[]);
    assert_eq!(cells_of(&rx), [897]);
}

/// The recording of a live 10 MHz band 28 carrier (PCI 367, 505-02, 11.52 Msps): the cell, its operator, band 28 with the
/// Australian band order. Skipped when the recording is not on this machine.
#[test]
fn live_band28_recording_reads_the_operator() {
    let Some(rec) = live_recording("sdrkit_cell_763.000MHz_11.52Msps_20261126-132305") else { return };
    let mut rx = stack_graph(&CELL, &[("band_search", J::from("off")), ("band_order", J::from("28 5 26 8 3 1 7 40"))]);
    run_graph(&mut rx, InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz).recording(), &rec.samples, &[]);
    let cs = cells(&state(&rx));
    assert_eq!(cs.len(), 1, "{cs:#?}");
    assert_eq!(cs[0]["pci"], 367);
    assert_eq!(cs[0]["band"], 28);
    assert_eq!(cs[0]["sib1"]["plmns"][0], "505-02", "{}", cs[0]);
}

/// A band search over a live recording: the recording's centre is the first step (a known carrier), the other steps are
/// noise. The host follows each hop and retune and plays the recording on from where it was while the radio is on its
/// centre. Returns the stack after the first pass.
fn search_over(desc: &sdrkit_core::StackDescriptor, rec: &sdrkit_dev_test::sigmf::Recording, settings: &[(&str, J)]) -> sdrkit_graph::StackGraph {
    survey_over(desc, &rec.samples, rec.rate_hz, rec.centre_hz, settings, 1).0
}

/// [`search_over`] of `samples` at `rate` on `centre` for `passes` passes. Returns the stack and the radio centres of the
/// moves, in order.
fn survey_over(
    desc: &sdrkit_core::StackDescriptor,
    samples: &[Complex32],
    rate: f64,
    on: f64,
    settings: &[(&str, J)],
    passes: u32,
) -> (sdrkit_graph::StackGraph, Vec<f64>) {
    host_over(desc, samples, rate, on, &[&[("band_search", J::from("survey"))], settings].concat(), false, |r| {
        value(r, "search_passes").unwrap_or(0.0) >= f64::from(passes)
    })
}

/// [`survey_over`] with the band search of `settings` (else auto), until `done` holds for the results. When `near`, a
/// centre within a quarter of the rate of `on` hears the samples too, moved by the difference.
fn host_over(
    desc: &sdrkit_core::StackDescriptor,
    samples: &[Complex32],
    rate: f64,
    on: f64,
    settings: &[(&str, J)],
    near: bool,
    mut done: impl FnMut(&sdrkit_core::Results) -> bool,
) -> (sdrkit_graph::StackGraph, Vec<f64>) {
    let first = format!("{}", on / 1e6);
    let mut keys: Vec<(&str, J)> = vec![("search_first", J::from(first.as_str()))];
    keys.extend(settings.iter().cloned());
    let mut rx = stack_graph(desc, &keys);
    let mut centre = on;
    let mut moves = Vec::new();
    rx.configure(&InputInfo::new(rate, centre, centre).recording()).unwrap();
    command(&mut rx, "search");
    let piece = (0.01 * rate) as usize;
    let mut r = sdrkit_algo_sim::rng::Rng::new(13);
    let (mut out, mut at, mut step) = (Output::default(), 0usize, 0.0);
    let mut hop: Option<(Vec<f64>, f64, usize)> = None;
    while !done(Owner::results(&rx)) {
        let heard = (centre - on).abs() < 1.0 || (near && (centre - on).abs() < rate / 4.0);
        let x: Vec<Complex32> = if heard && at + piece <= samples.len() {
            at += piece;
            let w = std::f64::consts::TAU * (on - centre) / rate;
            (at - piece..at).map(|i| samples[i] * Complex32::from_polar(1.0, (w * i as f64) as f32)).collect()
        } else {
            (0..piece).map(|_| r.complex_normal(2.0 * 0.01 * 0.01)).collect()
        };
        rx.feed(&x, &mut out);
        step += 0.01;
        let mut next = None;
        for q in out.requests.drain(..) {
            match q {
                HostRequest::Hop { channels, dwell_s, .. } => {
                    next = Some(channels[0]);
                    hop = Some((channels, dwell_s, 0));
                }
                HostRequest::Retune { tuned_hz, .. } => {
                    hop = None;
                    next = Some(tuned_hz);
                }
                _ => {}
            }
        }
        out.clear();
        if next.is_none() {
            if let Some((ch, dwell, i)) = hop.as_mut() {
                if step >= *dwell {
                    *i = (*i + 1) % ch.len();
                    next = Some(ch[*i]);
                }
            }
        }
        if let Some(n) = next {
            centre = n;
            step = 0.0;
            moves.push(n);
            rx.retuned(&InputInfo::new(rate, centre, centre).recording());
        }
    }
    (rx, moves)
}

/// A 20 MHz LTE carrier at 12 Msps, the known carrier of a survey of its band: its operator is out of reach at that rate,
/// so the step is not searched longer, and the second pass visits no carrier more than the first.
#[test]
fn a_carrier_wider_than_the_capture_is_not_held_or_visited_again() {
    let rate = 12e6;
    let c = sdrkit_stack_cell::lte::gen::scene(3, None);
    let hz = sdrkit_stack_cell::bands::earfcn_to_mhz(c.earfcn) * 1e6;
    let x = impaired(sdrkit_stack_cell::lte::gen::LteGenerator::new(rate, c, Default::default()), &snr(25.0), 1.0);
    let (rx, moves) = survey_over(&CELL, &x, rate, hz, &[("search_bands", J::from("1"))], 2);
    let steps = sdrkit_stack_cell::scan::lte_steps(1, rate, &[], &[]).len();
    assert_eq!(cells_of(&rx).len(), 1);
    // Each step once a pass, the known carrier and the steps of band 1, and no move back for a longer search.
    let visits = moves.iter().filter(|m| (**m - hz).abs() < 1.0).count();
    assert!(visits <= 2, "{visits} visits of the carrier in {} moves", moves.len());
    assert!(moves.len() <= 2 * (steps + 1) + 1, "{} moves for {steps} steps", moves.len());
    let r = Owner::results(&rx);
    assert!(!r.status().summary.contains("carriers added"), "{}", r.status().summary);
    // A step of noise in the second pass while the cell is listed: nothing to explain.
    assert!(text(r, "quiet_reason").is_none_or(|q| q.contains("20 MHz carrier")), "{:?}", text(r, "quiet_reason"));
}

/// The live recordings searched as the first step of a band search, a first look and a longer stay: the same cells and
/// operators as the whole recordings give. Skipped when the recordings are not on this machine.
#[test]
#[ignore]
fn live_recordings_through_a_band_search_step() {
    if let Some(rec) = live_recording("sdrkit_nr_884.720MHz_7.68Msps_20261126-132757") {
        let rx = search_over(&NR, &rec, &[("search_bands", J::from("5"))]);
        assert_eq!(cells_of(&rx), [897]);
        // Its CORESET#0 of 48 resource blocks (8.64 MHz) is wider than 7.68 Msps: the SIB1 cannot be read there.
        let c = &cells(&state(&rx))[0];
        assert_eq!(c["sib1_step"], "too_narrow", "{c}");
        assert_eq!(c["coreset0_rbs"], 48, "{c}");
    }
    if let Some(rec) = live_recording("sdrkit_cell_763.000MHz_11.52Msps_20261126-132305") {
        let rx = search_over(&CELL, &rec, &[("search_bands", J::from("28")), ("band_order", J::from("28 5 26 8 3 1 7 40"))]);
        let cs = cells(&state(&rx));
        assert_eq!(cs.len(), 1, "{cs:#?}");
        assert_eq!(cs[0]["pci"], 367);
        assert_eq!(cs[0]["sib1"]["plmns"][0], "505-02", "{}", cs[0]);
    }
}

/// The live n5 recording at 7.68 Msps, the known carrier of a search of n5 in auto: its CORESET#0 does not fit the capture, so
/// the cell does not end the search; the pass finds no cell to read and ends on it, and the status says so. Skipped when
/// the recording is not on this machine.
#[test]
#[ignore]
fn live_n5_auto_searches_on_and_ends_on_the_cell() {
    let Some(rec) = live_recording("sdrkit_nr_884.720MHz_7.68Msps_20261126-132757") else { return };
    let mut seen = Vec::new();
    let mut fed = 0;
    let (rx, moves) = host_over(&NR, &rec.samples, rec.rate_hz, rec.centre_hz, &[("search_bands", J::from("5"))], true, |r| {
        fed += 1;
        let s = &r.status().summary;
        if seen.last() != Some(s) {
            seen.push(s.clone());
        }
        (s.contains("the strongest cell found") && text(r, "quiet_reason").is_some()) || fed > 6000
    });
    let r = Owner::results(&rx);
    eprintln!("{} moves; {:#?}", moves.len(), seen);
    assert_eq!(cells_of(&rx), [897]);
    assert!(seen.iter().any(|s| s.contains("PCI 897") && s.contains("searching on for a cell that can be read")), "{seen:#?}");
    let s = &r.status().summary;
    assert!(s.starts_with("Staying on PCI 897") && s.contains("no cell of the pass can be read at 7.68 Msps"), "{s}");
    let why = text(r, "quiet_reason").unwrap_or_default();
    assert!(why.starts_with("raise the rate to "), "{why}");
}

/// Each first look of a band search step at ten places in the live recordings shows the cell's sync signal, so that the
/// step is searched longer. Skipped when the recordings are not on this machine.
#[test]
#[ignore]
fn live_first_looks_show_the_cells() {
    use sdrkit_stack_cell::decoder::Capture;
    for (name, nr, look_s) in [
        ("sdrkit_nr_884.720MHz_7.68Msps_20261126-132757", true, 0.08),
        ("sdrkit_cell_763.000MHz_11.52Msps_20261126-132305", false, 0.05),
    ] {
        let Some(rec) = live_recording(name) else { continue };
        let n = (look_s * rec.rate_hz) as usize;
        let mut missed = Vec::new();
        for k in 0..10 {
            let at = k * (rec.samples.len() - n) / 10;
            let cap = Capture::new(rec.samples[at..at + n].to_vec(), rec.rate_hz, rec.centre_hz, (at + n) as f64 / rec.rate_hz);
            let seen = if nr {
                let mut s = sdrkit_stack_cell::nr::search::NrSearcher::default();
                let o = s.search(&cap);
                !o.cells.is_empty() || o.best_pss_db >= s.min_pss_db
            } else {
                let mut s = sdrkit_stack_cell::lte::search::LteSearcher::default();
                let o = s.search(&cap);
                !o.cells.is_empty() || o.best_pss_db >= s.min_pss_db
            };
            if !seen {
                missed.push(at);
            }
        }
        assert!(missed.is_empty(), "{name}: no sync in the looks at {missed:?}");
    }
}

/// A live survey pass over noise, the host following each hop and retune: from the stack's first report on, no status has
/// an empty summary, also right after each move of the radio.
#[test]
fn a_live_survey_never_shows_an_empty_status() {
    let rate = 7.68e6;
    for (desc, band, range) in [(&CELL, "20", "791-821"), (&NR, "28", "758-783")] {
        let mut rx = stack_graph(
            desc,
            &[("band_search", J::from("survey")), ("search_bands", J::from(band)), ("search_ranges", J::from(range))],
        );
        let mut centre = 700e6;
        rx.configure(&InputInfo::new(rate, centre, centre)).unwrap();
        let piece = (0.02 * rate) as usize;
        let mut r = sdrkit_algo_sim::rng::Rng::new(17);
        let mut buf = vec![Complex32::default(); piece];
        let (mut out, mut hop, mut step, mut fed) = (Output::default(), None::<(Vec<f64>, f64, usize)>, 0.0, 0.0);
        let (mut lines, mut empty, mut moves) = (0usize, Vec::new(), 0usize);
        let start = std::time::Instant::now();
        while value(Owner::results(&rx), "search_passes").unwrap_or(0.0) < 1.0 {
            assert!(fed < 120.0, "{}: no pass after {fed} s", desc.id);
            buf.iter_mut().for_each(|v| *v = r.complex_normal(2.0 * 0.02 * 0.02));
            // At the pace of the signal, as a radio gives it.
            if let Some(w) = (start + std::time::Duration::from_secs_f64(fed)).checked_duration_since(std::time::Instant::now()) {
                std::thread::sleep(w);
            }
            rx.feed(&buf, &mut out);
            fed += 0.02;
            step += 0.02;
            // Before the first report of the stack the status is the framework's own (off, no text).
            let s = Owner::results(&rx).status();
            lines += 1;
            if s.summary.is_empty() && s.lock_state != sdrkit_core::LockState::Off {
                empty.push((fed, moves));
            }
            let mut next = None;
            for q in out.requests.drain(..) {
                match q {
                    HostRequest::Hop { channels, dwell_s, .. } => {
                        next = Some(channels[0]);
                        hop = Some((channels, dwell_s, 0));
                    }
                    HostRequest::Retune { tuned_hz, .. } => {
                        hop = None;
                        next = Some(tuned_hz);
                    }
                    _ => {}
                }
            }
            out.clear();
            if next.is_none() {
                if let Some((ch, dwell, at)) = hop.as_mut() {
                    if step >= *dwell {
                        *at = (*at + 1) % ch.len();
                        next = Some(ch[*at]);
                    }
                }
            }
            if let Some(n) = next {
                centre = n;
                step = 0.0;
                moves += 1;
                rx.retuned(&InputInfo::new(rate, centre, centre));
            }
        }
        eprintln!("{}: {lines} status lines over {moves} moves in {fed:.1} s, {} empty", desc.id, empty.len());
        assert!(empty.is_empty(), "{}: {} of {lines} status lines empty, at (s, move) {empty:?}", desc.id, empty.len());
    }
}

/// What a live run in auto with a host that gives no other rate showed.
struct AutoRun {
    /// The radio centre at the end, Hz.
    centre: f64,
    /// The status texts seen, each once, in order.
    statuses: Vec<String>,
    /// The requests for another rate.
    tunes: usize,
}

/// Plays a host in auto, live at `rate`, from `start` at the pace of the signal until `done` holds for the results and the
/// radio centre, at most `seconds`: carrier `wide` (a 20 MHz LTE carrier) on its centre, `narrow` (a 5 MHz one) on its
/// centre when given, noise elsewhere. The host follows hops and retunes, reports no move to the frequency the radio is on,
/// and gives no other rate: when `answer`, it answers a
/// request for one at once with the rate it has, at the frequency asked for; else it leaves the request unanswered.
#[allow(clippy::too_many_arguments)]
fn auto_run(
    rx: &mut sdrkit_graph::StackGraph,
    rate: f64,
    start: f64,
    seconds: f64,
    wide: f64,
    narrow: Option<f64>,
    answer: bool,
    done: impl Fn(&sdrkit_core::Results, f64) -> bool,
) -> AutoRun {
    use sdrkit_stack_cell::lte::gen::{scene, LteGenerator};
    let piece = (0.02 * rate) as usize;
    let carrier = |n: u32| impaired(LteGenerator::new(rate, scene(n, None), Default::default()), &snr(25.0), 1.0);
    let (a, b) = (carrier(3), carrier(1));
    let mut r = sdrkit_algo_sim::rng::Rng::new(19);
    let (mut out, mut hop, mut step, mut fed, mut at) = (Output::default(), None::<(Vec<f64>, f64, usize)>, 0.0, 0.0, 0usize);
    let mut run = AutoRun { centre: start, statuses: Vec::new(), tunes: 0 };
    rx.configure(&InputInfo::new(rate, start, start)).unwrap();
    let t0 = std::time::Instant::now();
    while fed < seconds && !done(Owner::results(&*rx), run.centre) {
        let c = run.centre;
        let x: Vec<Complex32> = (0..piece)
            .map(|i| {
                let k = (at + i) % a.len();
                let s = if (c - wide).abs() < 1.0 {
                    a[k]
                } else if narrow.is_some_and(|n| (c - n).abs() < 1.0) {
                    b[k]
                } else {
                    Complex32::default()
                };
                s + r.complex_normal(2.0 * 0.01 * 0.01)
            })
            .collect();
        at += piece;
        if let Some(w) = (t0 + std::time::Duration::from_secs_f64(fed)).checked_duration_since(std::time::Instant::now()) {
            std::thread::sleep(w);
        }
        rx.feed(&x, &mut out);
        fed += 0.02;
        step += 0.02;
        let s = &Owner::results(&*rx).status().summary;
        if run.statuses.last() != Some(s) {
            run.statuses.push(s.clone());
        }
        let mut next = None;
        for q in out.requests.drain(..) {
            match q {
                HostRequest::Hop { channels, dwell_s, .. } => {
                    next = Some(channels[0]);
                    hop = Some((channels, dwell_s, 0));
                }
                HostRequest::Retune { tuned_hz, .. } => {
                    hop = None;
                    next = Some(tuned_hz);
                }
                HostRequest::Tune { freq_hz, .. } => {
                    run.tunes += 1;
                    if let Some(f) = freq_hz.filter(|_| answer) {
                        hop = None;
                        run.centre = f;
                        step = 0.0;
                        rx.configure(&InputInfo::new(rate, f, f)).unwrap();
                    }
                }
                _ => {}
            }
        }
        out.clear();
        if next.is_none() {
            if let Some((ch, dwell, i)) = hop.as_mut() {
                if step >= *dwell {
                    *i = (*i + 1) % ch.len();
                    next = Some(ch[*i]);
                }
            }
        }
        if let Some(n) = next {
            // As an engine does, a move to where the radio is reports nothing.
            if n != run.centre {
                rx.retuned(&InputInfo::new(rate, n, n));
            }
            run.centre = n;
            step = 0.0;
        }
    }
    run
}

/// In auto, live at 7.68 Msps, started on a 20 MHz carrier whose operator needs about 18 Msps, with a host that gives no
/// other rate: the stack asks for the rate once, lists the cell, searches on, and stays on the 5 MHz carrier of a later
/// step, whose operator it reads. Without that carrier, the pass ends on the strongest cell found and the status says so.
#[test]
fn auto_searches_on_past_a_cell_it_cannot_read() {
    let rate = 7.68e6;
    let steps = sdrkit_stack_cell::scan::lte_steps(1, rate, &[(2110e6, 2140e6)], &[]);
    assert!(steps.len() >= 3, "{steps:?}");
    let (wide, narrow) = (steps[0], steps[steps.len() - 1]);
    let settings = [("search_bands", J::from("1")), ("search_ranges", J::from("2110-2140"))];
    // A host that answers the request for the rate with the rate it has: the same.
    let mut rx = stack_graph(&CELL, &settings);
    let run = auto_run(&mut rx, rate, wide, 60.0, wide, Some(narrow), true, |r, c| {
        c == narrow && value(r, "identified") == Some(1.0) && r.status().summary.starts_with("Staying on PCI 301")
    });
    assert_eq!((run.tunes, run.centre), (1, narrow), "{:#?}", run.statuses);
    // The search starts on the step the radio is on, which the host does not report as a move.
    let waits = |run: &AutoRun| run.statuses.iter().any(|s| s.contains("waiting for the radio") || s.contains("did not move"));
    assert!(!waits(&run), "{:#?}", run.statuses);
    // A host that does not answer.
    let mut rx = stack_graph(&CELL, &settings);
    let run = auto_run(&mut rx, rate, wide, 60.0, wide, Some(narrow), false, |r, c| {
        c == narrow && value(r, "identified") == Some(1.0) && r.status().summary.starts_with("Staying on PCI 301")
    });
    let r = Owner::results(&rx);
    assert_eq!(run.tunes, 1, "{:#?}", run.statuses);
    assert_eq!(run.centre, narrow, "{:#?}", run.statuses);
    assert_eq!(value(r, "cell_count"), Some(2.0), "{:#?}", run.statuses);
    assert_eq!(value(r, "identified"), Some(1.0), "{:#?}", run.statuses);
    let note = format!(
        "PCI 450 at {:.1} MHz listed; its operator needs about 18 Msps: searching on for a cell that can be read",
        wide / 1e6
    );
    assert!(run.statuses.iter().any(|s| s.starts_with("Searching band 1: ") && s.contains(&note)), "{:#?}", run.statuses);
    assert!(r.status().summary.starts_with("Staying on PCI 301"), "{}", r.status().summary);
    assert_eq!(text(r, "quiet_reason"), None, "{}", r.status().summary);
    // The 20 MHz carrier alone: the pass ends on it.
    let mut rx = stack_graph(&CELL, &settings);
    let run = auto_run(&mut rx, rate, wide, 60.0, wide, None, false, |r, _| {
        r.status().summary.contains("the strongest cell found") && text(r, "quiet_reason").is_some()
    });
    let r = Owner::results(&rx);
    assert_eq!(run.centre, wide, "{:#?}", run.statuses);
    assert_eq!(run.tunes, 1, "{:#?}", run.statuses);
    let s = &r.status().summary;
    assert!(s.starts_with("Staying on PCI 450") && s.contains("the strongest cell found: no cell of the pass can be read at 7.68 Msps"), "{s}");
    let why = text(r, "quiet_reason").unwrap_or_default();
    assert!(why.starts_with("raise the rate to 20 Msps to read this cell's operator"), "{why}");
}

/// The recording `<name>.sigmf-meta` in the folder that the variable `SDRKIT_LIVE_RECORDINGS` names, or None. The
/// recordings of real signals are kept outside the repository; without the variable the test is skipped.
pub(crate) fn live_recording(name: &str) -> Option<sdrkit_dev_test::sigmf::Recording> {
    let Ok(folder) = std::env::var("SDRKIT_LIVE_RECORDINGS") else {
        eprintln!("skipped: SDRKIT_LIVE_RECORDINGS is not set");
        return None;
    };
    let p = std::path::PathBuf::from(folder).join(format!("{name}.sigmf-meta"));
    if !p.exists() {
        eprintln!("skipped: {} is not here", p.display());
        return None;
    }
    let data = p.with_file_name(format!("{name}.sigmf-data"));
    Some(sdrkit_dev_test::sigmf::read_pair(&p, &data).expect("the recording"))
}

/// A radio 15 ppm off at 3.54 GHz (53 kHz) and 12 Msps: the first capture decodes the n78 cell with the wide offset search;
/// the next ones search 2 ppm about the clock error learned from it, and still decode the cell at the same offset.
#[test]
fn nr_learns_the_clock_error_of_the_radio() {
    use sdrkit_stack_cell::decoder::Capture;
    let rate = 12e6;
    let (centre, cells) = scene(2);
    let mut keys = snr(10.0);
    keys.push(("gen_clock_ppm", J::from(15.0)));
    keys.push(("gen_offset_hz", J::from(15e-6 * centre)));
    let x = impaired(NrGenerator::new(rate, cells, NrGenOptions::default()), &keys, 0.24);
    let n = (0.08 * rate) as usize;
    let mut s = sdrkit_stack_cell::nr::search::NrSearcher::default();
    for i in 0..3 {
        let cap = Capture::new(x[i * n..(i + 1) * n].to_vec(), rate, centre, 0.08 * (i + 1) as f64);
        let out = s.search(&cap);
        let f = out.cells.iter().find(|f| f.pci == 17 && f.mib.is_some()).unwrap_or_else(|| panic!("capture {i}"));
        let off = f.cand.offset_hz + f.cfo_hz;
        assert!((off - 15e-6 * centre).abs() < 2e3, "capture {i}: {off}");
    }
}

/// Weak cells that the stack lists from a longer signal show their sync signal in every first look of a band search step:
/// an LTE cell at 0 dB SNR in looks of 50 ms, an NR cell at 0 dB SNR in looks of 80 ms, at five places of the signal.
#[test]
fn weak_cells_show_in_every_first_look() {
    use sdrkit_stack_cell::decoder::Capture;
    let rate = 7.68e6;
    let lte = sdrkit_stack_cell::lte::gen::scene(1, None);
    let lte_hz = sdrkit_stack_cell::bands::earfcn_to_mhz(lte.earfcn) * 1e6;
    let lte_x = impaired(sdrkit_stack_cell::lte::gen::LteGenerator::new(rate, lte, Default::default()), &snr(0.0), 0.45);
    let (nr_hz, nr_cells) = scene(1);
    let nr_x = impaired(NrGenerator::new(rate, nr_cells, NrGenOptions::default()), &snr(0.0), 0.45);
    for (desc, hz, x, look_s) in [(&CELL, lte_hz, &lte_x, 0.05), (&NR, nr_hz, &nr_x, 0.08)] {
        let mut rx = stack_graph(desc, &[("band_search", J::from("off"))]);
        run_graph(&mut rx, InputInfo::new(rate, hz, hz).recording(), x, &[]);
        assert_eq!(cells_of(&rx).len(), 1, "{}: the weak cell is listed", desc.id);
        let n = (look_s * rate) as usize;
        for k in 0..5 {
            let at = k * (x.len() - n) / 5;
            let cap = Capture::new(x[at..at + n].to_vec(), rate, hz, (at + n) as f64 / rate);
            let seen = if desc.id == "nr" {
                let mut s = sdrkit_stack_cell::nr::search::NrSearcher::default();
                let o = s.search(&cap);
                !o.cells.is_empty() || o.best_pss_db >= s.min_pss_db
            } else {
                let mut s = sdrkit_stack_cell::lte::search::LteSearcher::default();
                let o = s.search(&cap);
                !o.cells.is_empty() || o.best_pss_db >= s.min_pss_db
            };
            assert!(seen, "{}: no sync in the look at {at}", desc.id);
        }
    }
}

/// A step whose search ends after the hop has moved on: the host serves the stack's request to search the step longer one
/// step late. The radio goes back, the cell there is read, and the hop goes on from the step after it; the pass searches
/// every step once.
#[test]
fn a_late_result_takes_the_radio_back_to_its_step() {
    let rate = 12e6;
    let (cell_hz, _) = scene(1);
    let steps = sdrkit_stack_cell::scan::nr_steps(28, rate, &[], &[]);
    let mut rx = stack_graph(
        &NR,
        &[("band_search", J::from("survey")), ("search_bands", J::from("28")), ("search_look_s", J::from(0.025))],
    );
    let mut centre = steps[0];
    rx.configure(&InputInfo::new(rate, centre, centre).recording()).unwrap();
    command(&mut rx, "search");
    let piece_s = 0.005;
    let (mut out, mut step, mut fed) = (Output::default(), 0.0, 0.0);
    let mut hop: Option<(Vec<f64>, f64, usize)> = None;
    let mut late: Vec<f64> = Vec::new();
    let mut back = 0;
    let mut visits: Vec<f64> = Vec::new();
    let mut gen_at = centre;
    let mut x = n28_from(centre, 3.0);
    let mut at = 0;
    while value(Owner::results(&rx), "search_passes").unwrap_or(0.0) < 1.0 {
        assert!(fed < 30.0, "no pass after {fed} s");
        if gen_at != centre {
            (gen_at, x, at) = (centre, n28_from(centre, 3.0), 0);
        }
        let n = (piece_s * rate) as usize;
        rx.feed(&x[at..at + n], &mut out);
        at = (at + n) % (x.len() - n);
        fed += piece_s;
        step += piece_s;
        let mut next = None;
        for q in out.requests.drain(..) {
            match q {
                // A newer request takes the place of one that waits.
                HostRequest::Hop { channels, dwell_s, .. } => {
                    late.clear();
                    next = Some(channels[0]);
                    hop = Some((channels, dwell_s, 0));
                }
                // Served one step late.
                HostRequest::Retune { tuned_hz, .. } => late.push(tuned_hz),
                _ => {}
            }
        }
        out.clear();
        if next.is_none() {
            if let Some((ch, dwell, i)) = hop.as_mut() {
                if step >= *dwell {
                    *i = (*i + 1) % ch.len();
                    next = Some(ch[*i]);
                }
            }
        }
        if let Some(n) = next {
            if let Some(b) = late.pop().filter(|b| *b != n) {
                // The hop moved on before the request came: the radio goes there, then back.
                rx.retuned(&InputInfo::new(rate, n, n).recording());
                hop = None;
                back += 1;
                centre = b;
            } else {
                centre = n;
            }
            late.clear();
            step = 0.0;
            visits.push(centre);
            rx.retuned(&InputInfo::new(rate, centre, centre).recording());
        }
    }
    assert!(back >= 1, "no late result: {visits:?}");
    rx.finish(&mut Output::default());
    let cs = cells(&state(&rx));
    let pcis: Vec<i64> = cs.iter().filter_map(|c| c["pci"].as_i64()).collect();
    assert_eq!(pcis, [222], "{cs:#?}");
    assert!(cs[0]["sib1"].is_object(), "{}", cs[0]);
    assert!((cs[0]["frequency_hz"].as_f64().unwrap() - cell_hz).abs() < 10e3);
    for s in &steps {
        assert!(visits.iter().any(|v| v == s), "{s} not visited: {visits:?}");
    }
}
