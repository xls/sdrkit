//! Sferics through the receiver: placed strokes counted and timed at each rate, the floor, the threshold, flashes, mains
//! trains, impulses too long, noise alone, the test signal, pieces of any size, `clear` and `reset`. Rows longer than a few
//! seconds at 2 Msps or more are fed in pieces made on the fly, so no signal is held whole.

use sci::family::field;
use sci::sferics::count::{Counter, Phase};
use sci::sferics::gen::{add_mains, add_stroke};
use sci::sferics::{
    self, DESCRIPTOR, GEN_SCENE, GEN_STRIKE_RATE, MAINS_FILTER, START_UTC, THRESHOLD,
};
use sci::typed::{SfericsStormEvent, SfericsStrikeRow, SFERICS_STORM};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_core::{Complex32, Event, InputInfo, LockState, Output, Receiver, Results};
use sdrkit_dev_test::Harness;
use sdrkit_stack_science as sci;

/// The stroke times of the 8-stroke storm, s.
const STORM: [f64; 8] = [1.0, 2.3, 3.1, 4.6, 5.3, 6.9, 7.6, 8.8];

/// A signal made piece by piece: strokes, a mains train, a rectangle, a carrier offset and white noise.
#[derive(Clone)]
struct Signal {
    rate: f64,
    seconds: f64,
    noise_rms: f64,
    /// (time s, peak SNR dB).
    strokes: Vec<(f64, f64)>,
    /// (impulses a second, peak SNR dB).
    mains: Option<(f64, f64)>,
    /// (start s, length s, amplitude).
    rectangle: Option<(f64, f64, f64)>,
    offset_hz: f64,
}

impl Signal {
    fn new(rate: f64, seconds: f64, noise_rms: f64) -> Signal {
        Signal {
            rate,
            seconds,
            noise_rms,
            strokes: Vec::new(),
            mains: None,
            rectangle: None,
            offset_hz: 0.0,
        }
    }

    fn strokes(mut self, times: &[f64], snr_db: f64) -> Signal {
        self.strokes.extend(times.iter().map(|&t| (t, snr_db)));
        self
    }

    fn amp(&self, snr_db: f64) -> f64 {
        self.noise_rms * 10f64.powf(snr_db / 20.0)
    }

    /// Fills `x`, whose first sample has the index `first`.
    fn fill(&self, x: &mut [Complex32], first: u64, noise: &mut Awgn) {
        x.fill(Complex32::default());
        for (i, &(t, snr)) in self.strokes.iter().enumerate() {
            let start = (t * self.rate).round() as u64;
            add_stroke(x, first, self.rate, start, self.amp(snr), 0.7 * i as f64);
        }
        if let Some((t, len, a)) = self.rectangle {
            let (s, e) = ((t * self.rate) as u64, ((t + len) * self.rate) as u64);
            for (k, v) in x.iter_mut().enumerate() {
                let n = first + k as u64;
                if (s..e).contains(&n) {
                    *v += Complex32::new(a as f32, 0.0);
                }
            }
        }
        if self.offset_hz != 0.0 {
            let w = self.offset_hz / self.rate;
            for (k, v) in x.iter_mut().enumerate() {
                let ph = std::f64::consts::TAU * (w * (first + k as u64) as f64).fract();
                *v *= Complex32::from_polar(1.0, ph as f32);
            }
        }
        if let Some((per_s, snr)) = self.mains {
            add_mains(x, first, self.rate, per_s, self.amp(snr));
        }
        noise.process(x);
    }

    fn len(&self) -> u64 {
        (self.seconds * self.rate).round() as u64
    }

    /// The whole signal (for short ones).
    fn all(&self) -> Vec<Complex32> {
        let mut x = vec![Complex32::default(); self.len() as usize];
        let mut noise = Awgn::with_power(self.noise_rms * self.noise_rms, 17).unwrap();
        self.fill(&mut x, 0, &mut noise);
        x
    }
}

/// What a run returns.
struct Got {
    results: Results,
    events: Vec<Event>,
}

impl Got {
    fn n(&self, key: &str) -> f64 {
        field(&self.results, key)
    }
    fn lock(&self) -> LockState {
        self.results.status().lock_state
    }
    /// The `time_s` column of the strike table, oldest first.
    fn strike_times(&self) -> Vec<f64> {
        let id = self.results.decl().table_id("strikes").unwrap();
        let mut v: Vec<f64> = self
            .results
            .table(id)
            .unwrap()
            .rows_newest_first()
            .map(|(_, r)| r.at(&SfericsStrikeRow::TIME_S).unwrap())
            .collect();
        v.reverse();
        v
    }
    fn table_rows(&self) -> Vec<String> {
        let id = self.results.decl().table_id("strikes").unwrap();
        self.results
            .table(id)
            .unwrap()
            .rows_newest_first()
            .map(|(k, r)| format!("{k} {r:?}"))
            .collect()
    }
}

/// A configured receiver of `h` at `rate`.
fn receiver(h: &Harness, rate: f64) -> Box<dyn Receiver> {
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(rate, 3e6, 3e6).recording())
        .unwrap();
    rx
}

fn feed(rx: &mut dyn Receiver, x: &[Complex32], events: &mut Vec<Event>) {
    let mut out = Output::default();
    rx.feed(x, &mut out);
    events.append(&mut out.events);
}

fn end(mut rx: Box<dyn Receiver>, mut events: Vec<Event>) -> Got {
    let mut out = Output::default();
    rx.finish(&mut out);
    events.append(&mut out.events);
    Got {
        results: sdrkit_core::Owner::results(rx.as_ref()).clone(),
        events,
    }
}

/// Feeds `s` to the receiver of `h`, made in pieces of `piece` samples.
fn run_with(h: Harness, s: &Signal, piece: usize) -> Got {
    let mut rx = receiver(&h, s.rate);
    let mut noise = Awgn::with_power(s.noise_rms * s.noise_rms, 17).unwrap();
    let mut x = vec![Complex32::default(); piece];
    let mut events = Vec::new();
    let mut first = 0u64;
    while first < s.len() {
        let n = (s.len() - first).min(piece as u64) as usize;
        s.fill(&mut x[..n], first, &mut noise);
        feed(rx.as_mut(), &x[..n], &mut events);
        first += n as u64;
    }
    end(rx, events)
}

fn run(s: &Signal) -> Got {
    run_with(Harness::new(&DESCRIPTOR).rate(s.rate), s, 1 << 16)
}

#[test]
fn eight_strokes_at_every_rate() {
    for rate in [48e3, 250e3, 1e6, 2e6, 2.4e6] {
        let g = run(&Signal::new(rate, 11.0, 0.03).strokes(&STORM, 25.0));
        let at = format!("{rate}: {}", g.results.status().summary);
        assert_eq!(g.n("strikes"), 8.0, "{at}");
        assert_eq!(g.n("flashes"), 8.0, "{at}");
        assert_eq!(g.n("mains"), 0.0, "{at}");
        assert_eq!(g.lock(), LockState::Locked, "{at}");
    }
}

#[test]
fn a_carrier_offset_changes_nothing() {
    for offset in [-300e3, 150e3] {
        let mut s = Signal::new(2e6, 11.0, 0.03).strokes(&STORM, 25.0);
        s.offset_hz = offset;
        assert_eq!(run(&s).n("strikes"), 8.0, "{offset}");
    }
}

#[test]
fn the_floor_follows_the_noise() {
    for rms in [0.002, 0.05] {
        let g = run(&Signal::new(1e6, 11.0, rms).strokes(&STORM, 20.0));
        assert_eq!(g.n("strikes"), 8.0, "{rms}");
        let want = 20.0 * rms.log10();
        assert!(
            (g.n("floor_db") - want).abs() < 1.5,
            "{rms}: {}",
            g.n("floor_db")
        );
    }
}

#[test]
fn strike_times_are_within_a_fifth_of_a_millisecond() {
    let g = run(&Signal::new(2e6, 11.0, 0.03).strokes(&STORM, 25.0));
    let t = g.strike_times();
    assert_eq!(t.len(), 8);
    for (got, want) in t.iter().zip(STORM) {
        assert!((got - want).abs() < 0.2e-3, "{got} {want}");
    }
}

#[test]
fn weak_strokes_against_the_threshold() {
    let s = Signal::new(1e6, 11.0, 0.03).strokes(&STORM, 14.0);
    let low = run_with(
        Harness::new(&DESCRIPTOR).rate(1e6).set(&THRESHOLD, 10.0),
        &s,
        1 << 16,
    );
    let n = low.n("strikes");
    assert!((7.0..=8.0).contains(&n), "{n}");
    let high = run_with(
        Harness::new(&DESCRIPTOR).rate(1e6).set(&THRESHOLD, 20.0),
        &s,
        1 << 16,
    );
    assert_eq!(high.n("strikes"), 0.0);
}

#[test]
fn strokes_close_together_are_one_flash() {
    let g = run(&Signal::new(250e3, 4.0, 0.03).strokes(&[1.0, 1.06, 1.12, 2.5], 25.0));
    assert_eq!((g.n("strikes"), g.n("flashes")), (4.0, 2.0));
}

#[test]
fn a_mains_train_alone_is_left_out() {
    for per_s in [100.0, 120.0] {
        let mut s = Signal::new(1e6, 8.0, 0.03);
        s.mains = Some((per_s, 20.0));
        let on = run(&s);
        assert!(on.n("strikes") <= 2.0, "{per_s}: {}", on.n("strikes"));
        assert!(on.n("mains") > 600.0, "{per_s}: {}", on.n("mains"));
        assert_eq!(on.n("mains_active"), 1.0);
        let off = run_with(
            Harness::new(&DESCRIPTOR)
                .rate(1e6)
                .set(&MAINS_FILTER, false),
            &s,
            1 << 16,
        );
        assert!(off.n("strikes") > 600.0, "{per_s}: {}", off.n("strikes"));
        assert_eq!(off.n("mains"), 0.0);
    }
}

#[test]
fn a_storm_under_a_mains_train() {
    let mut s = Signal::new(1e6, 11.0, 0.03).strokes(&STORM, 30.0);
    s.mains = Some((100.0, 14.0));
    let g = run(&s);
    let n = g.n("strikes");
    assert!((8.0..=11.0).contains(&n), "{n}");
    assert!(g.n("mains") > 600.0, "{}", g.n("mains"));
}

#[test]
fn a_long_rectangle_is_too_long() {
    let mut s = Signal::new(250e3, 4.0, 0.03);
    s.rectangle = Some((2.0, 0.02, 0.5));
    let g = run(&s);
    assert_eq!((g.n("strikes"), g.n("too_long")), (0.0, 1.0));
}

#[test]
fn noise_alone_counts_nothing() {
    for (rate, secs) in [(48e3, 60.0), (2e6, 12.0)] {
        let s = Signal::new(rate, secs, 0.03);
        let g = run_with(
            Harness::new(&DESCRIPTOR).rate(rate).set(&THRESHOLD, 6.0),
            &s,
            1 << 16,
        );
        assert_eq!(g.n("strikes"), 0.0, "{rate}");
        assert_eq!(g.lock(), LockState::Tracking, "{rate}");
    }
}

#[test]
fn the_test_signal_is_counted() {
    let r = Harness::new(&DESCRIPTOR)
        .rate(1e6)
        .seconds(12.0)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 25.0)
        .set(&GEN_STRIKE_RATE, 1.5)
        .run()
        .unwrap();
    let n = field(&r.results, "strikes");
    let f = field(&r.results, "flashes");
    assert!((8.0..=40.0).contains(&n), "{n}");
    assert!(f >= 6.0, "{f}");
}

#[test]
fn the_mains_scene_of_the_test_signal_is_left_out() {
    let r = Harness::new(&DESCRIPTOR)
        .rate(250e3)
        .seconds(6.0)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 25.0)
        .set(&GEN_SCENE, sferics::Scene::Mains)
        .run()
        .unwrap();
    assert!(field(&r.results, "strikes") <= 2.0);
    assert!(field(&r.results, "mains") > 400.0);
}

#[test]
fn pieces_of_any_size_give_the_same_counts() {
    let mut s = Signal::new(250e3, 4.0, 0.03).strokes(&[1.0, 1.06, 2.5, 3.1], 25.0);
    s.mains = Some((100.0, 20.0));
    let x = s.all();
    let h = || {
        Harness::new(&DESCRIPTOR)
            .rate(250e3)
            .set(&START_UTC, 1_700_000_000.0)
    };
    let whole = h().piece(x.len()).feed(&x).unwrap();
    let keys = [
        "strikes",
        "flashes",
        "mains",
        "too_long",
        "floor_db",
        "last_peak_db",
    ];
    let rows = |r: &Results| {
        Got {
            results: r.clone(),
            events: Vec::new(),
        }
        .table_rows()
    };
    // The 4 strokes, and at most 2 impulses of the train before it is recognised.
    let n = field(&whole.results, "strikes");
    assert!((4.0..=6.0).contains(&n), "{n}");
    assert!(field(&whole.results, "mains") > 250.0);
    for piece in [777, 4093, 65_536] {
        let r = h().piece(piece).feed(&x).unwrap();
        for k in keys {
            assert_eq!(
                field(&r.results, k),
                field(&whole.results, k),
                "{piece} {k}"
            );
        }
        assert_eq!(rows(&r.results), rows(&whole.results), "{piece}");
        assert_eq!(r.events.len(), whole.events.len(), "{piece}");
    }
}

/// Runs the stack command `key` of `rx`.
fn command(rx: &mut dyn Receiver, key: &str) {
    let id = rx.commands().id(key).unwrap();
    let args = rx.commands().def(id).unwrap().new_args();
    sdrkit_core::owner::invoke(rx, id, &args).unwrap();
}

#[test]
fn clear_empties_and_reset_learns_again() {
    let s = Signal::new(250e3, 6.0, 0.03).strokes(&[1.0, 2.0, 3.5, 4.5], 25.0);
    let x = s.all();
    let half = (3.0 * 250e3) as usize;
    let h = Harness::new(&DESCRIPTOR).rate(250e3).set(&THRESHOLD, 12.0);
    let mut rx = receiver(&h, 250e3);
    let mut ev = Vec::new();
    feed(rx.as_mut(), &x[..half], &mut ev);
    command(rx.as_mut(), "clear");
    let r = sdrkit_core::Owner::results(rx.as_ref());
    assert_eq!(field(r, "strikes"), 0.0);
    let id = r.decl().table_id("strikes").unwrap();
    assert_eq!(r.table(id).unwrap().len(), 0);
    // The floor is kept: the strokes after the clear count at once.
    feed(rx.as_mut(), &x[half..], &mut ev);
    let g = end(rx, ev);
    assert_eq!(g.n("strikes"), 2.0);
    assert_eq!(g.n("threshold_db"), 12.0);

    let mut rx = receiver(&h, 250e3);
    let mut ev = Vec::new();
    feed(rx.as_mut(), &x[..half], &mut ev);
    command(rx.as_mut(), "reset");
    // 0.3 s after the reset the floor is not valid yet.
    feed(rx.as_mut(), &x[half..half + 75_000], &mut ev);
    assert_eq!(
        sdrkit_core::Owner::results(rx.as_ref()).status().lock_state,
        LockState::Searching
    );
    feed(rx.as_mut(), &x[half + 75_000..], &mut ev);
    let g = end(rx, ev);
    assert_eq!(g.n("strikes"), 2.0);
    assert_eq!(g.n("threshold_db"), 12.0);
}

#[test]
fn a_stronger_storm_is_approaching() {
    // 5 minutes of 3 strikes a minute, then 5 minutes of 6 a minute, 6 dB stronger; 20 ksps through the plain counter.
    let rate = 20e3;
    let mut times = Vec::new();
    for i in 0..15 {
        times.push((10.0 + i as f64 * 20.0, 36.0));
    }
    for i in 0..30 {
        times.push((315.0 + i as f64 * 9.7, 42.0));
    }
    let s = Signal {
        strokes: times,
        ..Signal::new(rate, 612.0, 0.01)
    };
    let mut c = Counter::new(rate, 10.0, 0.5, true).unwrap();
    let mut noise = Awgn::with_power(1e-4, 5).unwrap();
    let mut x = vec![Complex32::default(); 1 << 15];
    let mut first = 0u64;
    let mut changes = Vec::new();
    while first < s.len() {
        let n = (s.len() - first).min(x.len() as u64) as usize;
        s.fill(&mut x[..n], first, &mut noise);
        c.process(&x[..n], first);
        changes.extend(c.take_changes());
        first += n as u64;
    }
    assert_eq!(c.strikes, 45);
    assert!(
        changes
            .iter()
            .any(|ch| ch.trend.phase == Phase::Approaching),
        "{changes:?}"
    );
    // The same through the receiver: the event leaves the graph.
    let g = run_with(Harness::new(&DESCRIPTOR).rate(rate), &s, 1 << 15);
    assert_eq!(g.n("strikes"), 45.0);
    let storms: Vec<_> = g.events.iter().filter(|e| e.is(&SFERICS_STORM)).collect();
    assert!(
        storms
            .iter()
            .any(|e| e.at(&SfericsStormEvent::STORM_PHASE) == Some("approaching")),
        "{storms:?}"
    );
}
