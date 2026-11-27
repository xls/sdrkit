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
    assert_eq!(hop.len(), sdrkit_stack_cell::scan::nr_steps(28, 12e6).len() + 1);
    // The radio moves there: the cell is found and the search stays on it.
    rx.retuned(&InputInfo::new(12e6, cell_hz, cell_hz).recording());
    out.clear();
    rx.feed(&n28_from(cell_hz, 0.3), &mut out);
    assert_eq!(cells_of(&rx), [222]);
    assert!(out.requests.iter().any(|q| matches!(q, HostRequest::Retune { tuned_hz, .. } if *tuned_hz == cell_hz)), "{:?}", out.requests);
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

/// A network radio still sends samples of the old centre for a short time after it moved. A cell in them is not listed a
/// second time at the new centre, at the old offset from it.
#[test]
fn nr_samples_of_the_old_centre_after_a_move_give_no_second_cell() {
    let (cell_hz, _) = scene(1);
    let mut rx = stack_graph(&NR, &[("band_search", J::from("off"))]);
    // Two neighbouring steps of a band search at 12 Msps: the cell is 1.48 MHz above the first and outside the second.
    let a = cell_hz - 1.48e6;
    let b = a + 7.2e6;
    rx.configure(&InputInfo::new(12e6, a, a).recording()).unwrap();
    rx.feed(&n28_from(a, 0.5), &mut Output::default());
    rx.retuned(&InputInfo::new(12e6, b, b).recording());
    let late = n28_from(a, 0.2);
    rx.feed(&late, &mut Output::default());
    rx.feed(&n28_from(b, 0.4), &mut Output::default());
    rx.finish(&mut Output::default());
    assert_eq!(cells_of(&rx), [222], "{:?}", cell_freqs(&rx));
    let f = cell_freqs(&rx);
    assert!((f[0] - cell_hz).abs() < 10e3, "{f:?} for {cell_hz}");
}

/// The same for LTE: a 1.4 MHz carrier 2 MHz above the old centre, and samples of the old centre after a move of 10 MHz.
#[test]
fn lte_samples_of_the_old_centre_after_a_move_give_no_second_cell() {
    let rate = 12e6;
    let c = sdrkit_stack_cell::lte::gen::scene(2, None);
    let cell_hz = sdrkit_stack_cell::bands::earfcn_to_mhz(c.earfcn) * 1e6;
    let (a, b) = (cell_hz - 2e6, cell_hz + 8e6);
    let from_a = |seconds: f64| {
        let mut keys = snr(20.0);
        keys.push(("gen_offset_hz", J::from(2e6)));
        impaired(sdrkit_stack_cell::lte::gen::LteGenerator::new(rate, c.clone(), Default::default()), &keys, seconds)
    };
    let mut rx = stack_graph(&CELL, &[("band_search", J::from("off"))]);
    rx.configure(&InputInfo::new(rate, a, a).recording()).unwrap();
    rx.feed(&from_a(0.6), &mut Output::default());
    assert_eq!(cells_of(&rx), [7], "the cell at the old centre");
    rx.retuned(&InputInfo::new(rate, b, b).recording());
    rx.feed(&from_a(0.25), &mut Output::default());
    rx.feed(&noise(rate, 0.5, 0.01, 4), &mut Output::default());
    rx.finish(&mut Output::default());
    assert_eq!(cells_of(&rx), [7], "{:?}", cell_freqs(&rx));
}

fn cells_of(rx: &sdrkit_graph::StackGraph) -> Vec<i64> {
    cells(&state(rx)).iter().filter_map(|c| c["pci"].as_i64()).collect()
}

/// On live samples a stack whose tuned frequency shows no cell for a second starts the band search by itself: a hop over the
/// steps of the band of the tuned frequency. A recording never starts one.
#[test]
fn a_quiet_live_frequency_starts_the_search() {
    for live in [true, false] {
        let rate = 2.56e6;
        let mut rx = stack_graph(&CELL, &[]);
        let input = InputInfo::new(rate, 778e6, 778e6);
        rx.configure(&if live { input } else { input.recording() }).unwrap();
        let mut out = Output::default();
        let mut hops = Vec::new();
        for p in noise(rate, 2.5, 0.05, 7).chunks(65536) {
            // Live samples arrive at about their rate; the helper thread searches as they come.
            if live {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            rx.feed(p, &mut out);
            hops.append(&mut out.requests);
            out.clear();
        }
        rx.finish(&mut out);
        hops.append(&mut out.requests);
        let steps = sdrkit_stack_cell::scan::lte_steps(28, rate);
        if live {
            assert!(
                hops.iter().any(|h| matches!(h, HostRequest::Hop { channels, .. } if *channels == steps)),
                "{hops:?}"
            );
        } else {
            assert!(hops.is_empty(), "{hops:?}");
        }
    }
}

/// A start in Australia without a setting of its own: the host sets the country's band order, and the band search of a
/// quiet live frequency steps through every band of that order, in that order.
#[test]
fn a_default_start_in_australia_searches_the_bands_of_the_country() {
    let data = sdrkit_presets::Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap();
    let au = sdrkit_presets::Region::new("AU").unwrap();
    let rate = 12e6;
    for (desc, want, steps) in [
        (&CELL, &[28u16, 5, 26, 8, 3, 1, 7, 40][..], sdrkit_stack_cell::scan::lte_steps as fn(u16, f64) -> Vec<f64>),
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
        let order = &p.settings.iter().find(|(k, _)| k == "band_order").expect("a band order").1;
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
        // Every step of each band of the order is in the hop, which starts on the first band.
        for b in want {
            let missing: Vec<f64> = steps(*b, rate).into_iter().filter(|c| !hop.iter().any(|h| (h - c).abs() < 1.0)).collect();
            assert!(missing.is_empty(), "{}: band {b} misses {missing:?}", desc.id);
        }
        assert_eq!(hop[0], steps(want[0], rate)[0], "{}: the first band first", desc.id);
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
    let steps = sdrkit_stack_cell::scan::lte_steps(20, rate).len();
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

/// The recording `<name>.sigmf-meta` in the folder that the variable `SDRKIT_LIVE_RECORDINGS` names, or None. The
/// recordings of real signals are kept outside the repository; without the variable the test is skipped.
fn live_recording(name: &str) -> Option<sdrkit_dev_test::sigmf::Recording> {
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
