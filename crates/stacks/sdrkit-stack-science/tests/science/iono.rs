//! The `iono` tool: the hop geometry, the scaling of synthetic ionograms, the matching of known sounders and the receiver on the
//! test signal.

use sci::iono::scale::{
    self, distance_km, group_path_km, scale, scale_vertical, traces, BandState, Geometry,
    KnownSounder, Scaling, TraceOptions,
};
use sci::iono::synth::{ionogram, Ionosphere, Scene};
use sci::iono::Sounders;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, StackConfig};
use sdrkit_stack_science as sci;
use sdrkit_util_geo::Position;

const MHZ: f64 = 1e6;

#[test]
fn iono_geometry_round_trips() {
    for hops in 1..=3 {
        for (d, h) in [(500.0, 110.0), (1500.0, 300.0), (3000.0, 250.0)] {
            let p = group_path_km(hops, d, h);
            let back = distance_km(hops, p, h).unwrap();
            assert!((back - d).abs() < 0.01, "{hops} {d} {h}: {back}");
            let hh = scale::mirror_height_km(hops, p, d).unwrap();
            assert!((hh - h).abs() < 0.01, "{hh}");
        }
    }
    // A path shorter than twice the height has no distance.
    assert_eq!(distance_km(1, 150.0, 100.0), None);
}

fn truth() -> (Ionosphere, f64, f64, f64) {
    let io = Ionosphere::standard(false);
    let hp_e = (20..58)
        .filter_map(|c| io.virtual_height_km(scale::col_hz(c)))
        .fold(f64::INFINITY, f64::min);
    let hp_f = (62..140)
        .filter_map(|c| io.virtual_height_km(scale::col_hz(c)))
        .fold(f64::INFINITY, f64::min);
    let hm = io.virtual_height_km(0.834 * 7.0 * MHZ).unwrap();
    (io, hp_e, hp_f, hm)
}

#[test]
fn iono_scales_a_vertical_ionogram() {
    let (_, hp_e, hp_f, hm) = truth();
    let ion = ionogram(&Scene::vertical(false));
    let Some(Scaling::Vertical(v)) = scale(&ion, &TraceOptions::default(), Geometry::Auto, None)
    else {
        panic!("not vertical");
    };
    let fo_e = v.fo_e.unwrap();
    let fo_f2 = v.fo_f2.unwrap();
    let fx = (1.3 + (1.3f64 * 1.3 + 4.0 * 49.0).sqrt()) / 2.0;
    assert!((fo_e.hz / MHZ - 3.0).abs() < 0.2, "{v:?}");
    assert!((fo_f2.hz / MHZ - 7.0).abs() < 0.2, "{v:?}");
    assert!((v.fx_f2.unwrap().hz / MHZ - fx).abs() < 0.25, "{v:?}");
    assert!((v.hp_e_km.unwrap() - hp_e).abs() < 15.0, "{v:?} {hp_e}");
    assert!((v.hp_f_km.unwrap() - hp_f).abs() < 20.0, "{v:?} {hp_f}");
    let m = 1490.0 / (hm + 176.0);
    assert!((v.m3000.unwrap() - m).abs() < 0.15, "{v:?} {m}");
    assert!((v.muf3000_hz.unwrap() / MHZ - 7.0 * m).abs() < 0.8, "{v:?}");
    assert_eq!(v.fo_f1, None);
    assert!((v.fmin_hz.unwrap() / MHZ - 1.0).abs() < 0.3, "{v:?}");
    assert!(v.multiple);
    let bands = scale::bands_open(v.muf3000_hz, v.fmin_hz);
    let state = |n: &str| bands.iter().find(|b| b.0 == n).unwrap().1;
    assert_eq!(state("20 m"), BandState::Open);
    assert_eq!(state("10 m"), BandState::Closed);
}

#[test]
fn iono_finds_the_f1_cusp() {
    let ion = ionogram(&Scene::vertical(true));
    let v = scale_vertical(&ion, &TraceOptions::default());
    let f1 = v.fo_f1.expect("foF1");
    assert!((f1.hz / MHZ - 4.5).abs() < 0.3, "{v:?}");
}

#[test]
fn iono_a_cut_trace_returns_a_lower_bound() {
    let mut s = Scene::vertical(false);
    s.swept_hz = (1e6, 6.5e6);
    let v = scale_vertical(&ionogram(&s), &TraceOptions::default());
    assert!(v.fo_f2.unwrap().lower_bound, "{v:?}");
    assert_eq!(v.muf3000_hz, None);
}

#[test]
fn iono_noise_has_no_trace_and_carriers_do_not_move_the_scaling() {
    let mut s = Scene::vertical(false);
    s.noise_only = true;
    let ion = ionogram(&s);
    assert!(traces(&ion, &TraceOptions::default()).is_empty());
    assert_eq!(
        scale(&ion, &TraceOptions::default(), Geometry::Auto, None),
        None
    );
    let clean = scale_vertical(&ionogram(&Scene::vertical(false)), &TraceOptions::default());
    let mut s = Scene::vertical(false);
    s.carriers_hz = vec![2.0e6, 5.0e6, 9.0e6];
    let v = scale_vertical(&ionogram(&s), &TraceOptions::default());
    for (a, b) in [(clean.fo_e, v.fo_e), (clean.fo_f2, v.fo_f2)] {
        assert!(
            (a.unwrap().hz - b.unwrap().hz).abs() < 0.2e6,
            "{clean:?} {v:?}"
        );
    }
}

#[test]
fn iono_scales_oblique_paths() {
    for d in [1000.0, 2000.0] {
        let ion = ionogram(&Scene::oblique(d));
        let Some(Scaling::Oblique(o)) = scale(&ion, &TraceOptions::default(), Geometry::Auto, None)
        else {
            panic!("not oblique at {d} km");
        };
        let tol = if d < 1500.0 { 0.08 } else { 0.10 };
        assert!((o.distance_km - d).abs() < tol * d, "{d}: {o:?}");
        let names: Vec<&str> = o.modes.iter().map(|m| m.name.as_str()).collect();
        assert!(
            names.contains(&"1F") && names.contains(&"2F"),
            "{d}: {names:?}"
        );
        // The path MUF is the nose of the 1F trace: the highest f / cos φ over the F layer.
        let io = Ionosphere::standard(false);
        let mut muf = 0.0f64;
        let mut fv = 3.05e6;
        while fv < 7.0e6 {
            if let Some(h) = io.virtual_height_km(fv) {
                muf = muf.max(fv / scale::incidence_rad(1, d, h).cos());
            }
            fv += 10e3;
        }
        let got = o.path_muf.unwrap().hz;
        assert!(
            (got - muf).abs() < 0.4e6,
            "{d}: {} vs {}",
            got / MHZ,
            muf / MHZ
        );
    }
}

fn site(lat: f64, lon: f64, period: f64, offset: f64) -> KnownSounder {
    KnownSounder {
        name: format!("{lat} {lon}"),
        rate_hz_s: 100e3,
        period_s: Some(period),
        offset_s: Some(offset),
        site: Position::new(lat, lon),
        verified: false,
        note: String::new(),
    }
}

#[test]
fn iono_matches_known_sounders() {
    let list = vec![site(52.0, 0.0, 30.0, 2.0), site(45.0, 10.0, 30.0, 12.0)];
    let rx = Position::new(48.0, 2.0);
    let m = scale::match_sounder(&list, 100e3, Some(30.0), Some(2.0), Some(rx), None);
    assert_eq!((m.level, m.index, m.candidates.len()), (2, Some(0), 2));
    let truth = list[0].site.distance_m(&rx) / 1e3;
    assert!((m.distance_km.unwrap() - truth).abs() < 1.0);
    let m = scale::match_sounder(&list, 100e3, None, None, None, None);
    assert_eq!((m.level, m.candidates.len()), (0, 2));
    // A sounder near the receiver: match 1, and the vertical geometry.
    let near = vec![site(48.1, 2.0, 60.0, 0.0)];
    let m = scale::match_sounder(&near, 100e3, None, None, Some(rx), None);
    assert_eq!(m.level, 1);
    let ion = ionogram(&Scene::vertical(false));
    let s = scale(
        &ion,
        &TraceOptions::default(),
        Geometry::Auto,
        m.distance_km,
    );
    assert!(matches!(s, Some(Scaling::Vertical(_))));
    assert_eq!(scale::known_sounders().len(), 6);
}

/// Runs the receiver on the generator for a radio at 0.5 MHz with 1 Msps.
fn run(seconds: f64, settings: &[CaseSetting]) -> Box<sdrkit_graph::StackGraph> {
    let rate = 1e6;
    let centre = 0.5e6;
    let h = sdrkit_dev_test::Harness::new(&sci::IONO)
        .set(&sdrkit_stack_science::iono::GEN_CENTRE, centre)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, -10.0)
        .set(&sdrkit_algo_sim::chain::GEN_SEED, 3)
        .with_settings(settings);
    let v = h.values().unwrap();
    let gen = sci::iono::make_generator(&sdrkit_core::GenConfig::new(rate, v.clone())).unwrap();
    let mut g = sdrkit_components::sim::impaired(gen, &v).unwrap();
    let mut rx = Box::new(sci::iono::receiver(&StackConfig::new(v)).unwrap());
    rx.configure(&InputInfo::new(rate, centre, centre).recording())
        .unwrap();
    let mut x = vec![Complex32::default(); 10_000];
    let mut out = Output::default();
    for _ in 0..(seconds * rate / 1e4) as usize {
        g.generate(&mut x);
        out.clear();
        rx.feed(&x, &mut out);
    }
    rx
}

fn sounders(rx: &sdrkit_graph::StackGraph) -> &Sounders {
    rx.graph().component_as::<Sounders>("sounders").unwrap()
}

/// The two strongest delay rows of a sounder's ionogram.
fn strongest_rows(s: &sci::iono::Sounder) -> Vec<usize> {
    let db = s.ionogram.db();
    let mut rows: Vec<(f32, usize)> = (0..scale::ROWS)
        .map(|r| {
            let v: f32 = (0..scale::COLS)
                .filter(|&c| db.swept[c])
                .map(|c| db.at(r, c))
                .sum();
            (v, r)
        })
        .collect();
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut out = vec![rows[0].1];
    out.extend(rows.iter().map(|r| r.1).find(|&r| r.abs_diff(out[0]) > 2));
    out.sort();
    out
}

#[test]
fn iono_finds_the_two_test_sounders() {
    // A radio at 0.5 MHz with 1 Msps, for 50 s.
    let rx = run(50.0, &[]);
    let s = sounders(&rx);
    let st = s.store();
    assert_eq!(
        st.sounders.len(),
        2,
        "{:?}",
        st.sounders
            .iter()
            .map(|s| (s.rate_hz_s, s.last_start_s))
            .collect::<Vec<_>>()
    );
    let a = st.sounders.iter().find(|s| s.rate_hz_s == 100e3).unwrap();
    let b = st.sounders.iter().find(|s| s.rate_hz_s == 500e3).unwrap();
    assert!((a.period_s.unwrap() - 30.0).abs() < 1e-6 && (a.offset_s.unwrap() - 2.0).abs() < 1e-6);
    assert!((b.period_s.unwrap() - 20.0).abs() < 1e-6 && (b.offset_s.unwrap() - 7.0).abs() < 1e-6);
    assert_eq!((a.sweeps, a.missed), (2, 0));
    assert_eq!((b.sweeps, b.missed), (3, 0));
    for (s, rows) in [(a, [32usize, 48]), (b, [36, 58])] {
        let got = strongest_rows(s);
        assert_eq!(got.len(), 2);
        for (g, t) in got.iter().zip(rows) {
            assert!(g.abs_diff(t) <= 1, "{}: {got:?} {rows:?}", s.rate_hz_s);
        }
        let swept = s.ionogram.swept();
        assert!((15..=19).contains(&swept), "{swept}");
        assert_eq!(s.ionogram.csv().lines().count(), 1 + swept * scale::ROWS);
    }
    let cov: Vec<usize> = (0..scale::COLS).filter(|&c| s.coverage()[c]).collect();
    assert_eq!(cov, (1..=18).collect::<Vec<_>>());
    // The scaling of the generator's flat traces: a history, one CSV line per point.
    assert!(!a.history.is_empty() && a.scaling.is_some());
    assert_eq!(
        sci::iono::history_csv(&a.history).lines().count(),
        1 + a.history.len()
    );
    assert_eq!(st.timeline.len(), 5);
}

#[test]
fn iono_unlocked_keeps_the_delay_difference() {
    let rx = run(
        36.0,
        &[CaseSetting::new(&sdrkit_stack_science::iono::LOCKED, false)],
    );
    let st = sounders(&rx).store();
    assert_eq!(st.sounders.len(), 2);
    let a = st.sounders.iter().find(|s| s.rate_hz_s == 100e3).unwrap();
    let r = strongest_rows(a);
    assert!((r[1] - r[0]).abs_diff(16) <= 1, "{r:?}");
}

#[test]
fn iono_noise_finds_nothing() {
    let rx = run(
        12.0,
        &[CaseSetting::new(
            &sdrkit_stack_science::iono::GEN_NOISE_ONLY,
            true,
        )],
    );
    let st = sounders(&rx).store();
    assert!(st.sounders.is_empty() && st.timeline.is_empty());
}
