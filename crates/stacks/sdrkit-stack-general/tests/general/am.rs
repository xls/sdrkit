//! AM detectors, the blanker and Auto quality: 250 ksps, the station at 0 Hz, no tune offset.

use crate::common::*;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

const RATE: f64 = 250e3;
const A: f64 = 0.2;

struct Station {
    n: u64,
    offset: f64,
    depth: f64,
    programme: f64,
    cn_db: f64,
    rng: sdrkit_algo_sim::rng::Rng,
    /// A second path: delay (s), amplitude, drift (Hz).
    fade: Option<(f64, f64, f64)>,
    /// An interferer: offset (Hz), relative level (dB), its tone (Hz), keyed at (Hz) with depth.
    interferer: Option<(f64, f64, f64, f64, f64)>,
    /// Impulses: width (s), period (s), level over the carrier (dB).
    impulses: Option<(f64, f64, f64)>,
    impulse_phase: f64,
}

impl Station {
    fn new(cn_db: f64) -> Self {
        Station {
            n: 0,
            offset: 0.0,
            depth: 0.5,
            programme: 0.3,
            cn_db,
            rng: sdrkit_algo_sim::rng::Rng::new(0x1234_5678_9abc_def1),
            fade: None,
            interferer: None,
            impulses: None,
            impulse_phase: 0.0,
        }
    }
    fn u(&mut self) -> f64 {
        self.rng.uniform_open()
    }
    fn g(&mut self) -> f64 {
        self.rng.normal()
    }
    fn env(&self, t: f64) -> f64 {
        A * (1.0 + self.depth * (TAU * 1000.0 * t).sin() + self.programme * (TAU * 400.0 * t).sin())
    }
    fn fill(&mut self, out: &mut [Complex32]) {
        let sigma = (A * A * 10f64.powf(-self.cn_db / 10.0) * RATE / 9000.0 / 2.0).sqrt();
        for v in out.iter_mut() {
            let t = self.n as f64 / RATE;
            self.n += 1;
            let ph = TAU * self.offset * t;
            let mut z = num_complex::Complex64::from_polar(self.env(t), ph);
            if let Some((d, a, drift)) = self.fade {
                let td = t - d;
                z += num_complex::Complex64::from_polar(
                    a * self.env(td),
                    TAU * self.offset * td + TAU * drift * t,
                );
            }
            if let Some((off, lvl, f, key, kd)) = self.interferer {
                let keyed = 1.0 - kd * (0.5 + 0.5 * (TAU * key * t).sin());
                let e = A * 10f64.powf(lvl / 20.0) * (1.0 + 0.5 * keyed * (TAU * f * t).sin());
                z += num_complex::Complex64::from_polar(e, TAU * off * t);
            }
            if let Some((w, p, lvl)) = self.impulses {
                let k = (t / p).floor();
                if t - k * p < w {
                    if (t - k * p) * RATE < 1.0 {
                        self.impulse_phase = TAU * self.u();
                    }
                    z += num_complex::Complex64::from_polar(
                        A * 10f64.powf(lvl / 20.0),
                        self.impulse_phase,
                    );
                }
            }
            z += num_complex::Complex64::new(sigma * self.g(), sigma * self.g());
            *v = Complex32::new(z.re as f32, z.im as f32);
        }
    }
}

fn audio(st: &mut Station, seconds: f64, settings: &[(&str, &str)]) -> Vec<f32> {
    let mut s = vec![("demod", "am"), ("width", "9000"), ("squelch", "off")];
    s.extend_from_slice(settings);
    let mut rx = receiver(RATE, 0.0, &s);
    let mut r = Run::default();
    run(&mut rx, seconds, RATE, 25_000, |b| st.fill(b), &mut r);
    r.audio[(2.0 * 48_000.0) as usize..].to_vec()
}

/// SINAD with both the 1 kHz and the 400 Hz tones counted as signal.
fn sinad(x: &[f32]) -> f64 {
    let ms = x.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / x.len() as f64;
    let mean = x.iter().map(|&v| v as f64).sum::<f64>() / x.len() as f64;
    let a1 = tone(x, 1000.0);
    let a4 = tone(x, 400.0);
    let resid = (ms - mean * mean - a1 * a1 / 2.0 - a4 * a4 / 2.0).max(1e-12);
    10.0 * ((ms - mean * mean) / resid).log10()
}

fn detector(d: &str) -> Vec<(&'static str, &'static str)> {
    match d {
        "envelope" => vec![("am_auto", "0"), ("am_detector", "envelope")],
        "synchronous" => vec![("am_auto", "0"), ("am_detector", "synchronous")],
        "upper" => vec![("am_auto", "0"), ("am_detector", "upper")],
        "lower" => vec![("am_auto", "0"), ("am_detector", "lower")],
        _ => vec![("am_auto", "1"), ("am_detector", "auto")],
    }
}

#[test]
fn detectors_against_noise() {
    for cn in [40.0, 20.0, 8.0] {
        let env = sinad(&audio(&mut Station::new(cn), 6.0, &detector("envelope")));
        for d in ["synchronous", "upper", "lower", "envelope"] {
            let y = audio(&mut Station::new(cn), 6.0, &detector(d));
            let s = sinad(&y);
            if d == "synchronous" {
                assert!(
                    s >= env - 1.0,
                    "C/N {cn}: synchronous {s:.1} against envelope {env:.1}"
                );
            }
            if cn == 40.0 {
                let a = tone(&y, 1000.0);
                assert!((a - 0.25).abs() <= 0.03, "{d}: tone {a}");
            }
        }
    }
}

#[test]
fn the_loop_pulls_in_150_hz_off() {
    let mut st = Station::new(30.0);
    st.offset = 150.0;
    let sync = sinad(&audio(&mut st, 6.0, &detector("synchronous")));
    let mut st = Station::new(30.0);
    st.offset = 150.0;
    let env = sinad(&audio(&mut st, 6.0, &detector("envelope")));
    assert!(sync >= env - 1.0, "{sync:.1} against {env:.1}");
}

fn harmonics_db(x: &[f32]) -> f64 {
    let a1 = tone(x, 1000.0);
    let a2 = tone(x, 2000.0);
    let a3 = tone(x, 3000.0);
    20.0 * (a1 / (a2 * a2 + a3 * a3).sqrt().max(1e-9)).log10()
}

#[test]
fn selective_fading() {
    for delay in [0.1e-3, 0.2e-3, 0.3e-3] {
        let mk = || {
            let mut st = Station::new(35.0);
            st.depth = 0.8;
            st.programme = 0.0;
            st.fade = Some((delay, 0.95, 0.3));
            st
        };
        let env = harmonics_db(&audio(&mut mk(), 12.0, &detector("envelope")));
        for d in ["synchronous", "upper", "lower"] {
            let s = harmonics_db(&audio(&mut mk(), 12.0, &detector(d)));
            assert!(
                s > env + 15.0,
                "delay {delay}: {d} {s:.1} against envelope {env:.1}"
            );
        }
    }
}

#[test]
fn an_interferer_above() {
    let mk = || {
        let mut st = Station::new(35.0);
        st.interferer = Some((5000.0, 10.0, 2300.0, 3.0, 0.7));
        st
    };
    let env = sinad(&audio(&mut mk(), 6.0, &detector("envelope")));
    let lower = sinad(&audio(&mut mk(), 6.0, &detector("lower")));
    assert!(
        lower > env + 15.0,
        "lower {lower:.1} against envelope {env:.1}"
    );
}

#[test]
fn the_blanker() {
    let mk = |cn: f64, imp: bool| {
        let mut st = Station::new(cn);
        if imp {
            st.impulses = Some((60e-6, 7e-3, 25.0));
        }
        st
    };
    let off = sinad(&audio(&mut mk(35.0, true), 6.0, &detector("envelope")));
    let mut s = detector("envelope");
    s.push(("nb", "1"));
    s.push(("nb_threshold", "12"));
    let on = sinad(&audio(&mut mk(35.0, true), 6.0, &s));
    assert!(on > off + 10.0, "on {on:.1} against off {off:.1}");
    let clean_off = sinad(&audio(&mut mk(20.0, false), 6.0, &detector("envelope")));
    let clean_on = sinad(&audio(&mut mk(20.0, false), 6.0, &s));
    assert!(
        clean_on > clean_off - 0.5,
        "{clean_on:.2} against {clean_off:.2}"
    );
}

#[test]
fn auto_quality() {
    let env = sinad(&audio(&mut Station::new(10.0), 10.0, &detector("envelope")));
    let auto = sinad(&audio(&mut Station::new(10.0), 10.0, &detector("auto")));
    assert!(
        auto > env + 2.0,
        "C/N 10: auto {auto:.1} against envelope {env:.1}"
    );
    let mk = || {
        let mut st = Station::new(30.0);
        st.impulses = Some((60e-6, 7e-3, 25.0));
        st
    };
    let env = sinad(&audio(&mut mk(), 10.0, &detector("envelope")));
    let auto = sinad(&audio(&mut mk(), 10.0, &detector("auto")));
    assert!(
        auto > env + 10.0,
        "impulses: auto {auto:.1} against envelope {env:.1}"
    );
    let env = sinad(&audio(&mut Station::new(40.0), 8.0, &detector("envelope")));
    let auto = sinad(&audio(&mut Station::new(40.0), 8.0, &detector("auto")));
    assert!(
        auto >= env - 1.5,
        "clean: auto {auto:.1} against envelope {env:.1}"
    );
}
