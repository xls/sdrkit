use sdrkit_algo_dsp::Frame;
use sdrkit_proto_bcdata::playout::*;

const R: usize = 48_000;

fn tone(n: usize, start: usize, hz: f64) -> Vec<Frame<2>> {
    (start..start + n)
        .map(|i| {
            let v = 0.5 * (std::f64::consts::TAU * hz * i as f64 / R as f64).sin() as f32;
            Frame([v, v])
        })
        .collect()
}

fn max_step(x: &[Frame<2>]) -> f32 {
    x.windows(2)
        .map(|w| (w[1].0[0] - w[0].0[0]).abs())
        .fold(0.0, f32::max)
}

fn run(c: Concealment, lost: &[usize], bursts: usize, burst: usize) -> (Vec<Frame<2>>, Stats) {
    let mut p = Playout::new(Config::new(R as u32, c));
    let mut out = Vec::new();
    for b in 0..bursts {
        if lost.contains(&b) {
            p.push_lost(burst);
        } else {
            p.push_sound(&tone(burst, b * burst, 440.0));
        }
        p.pull(burst, &mut out);
    }
    (out, p.stats())
}

#[test]
fn steady_bursts_play_without_concealment_or_clicks() {
    for c in [Concealment::Repeat, Concealment::Pitch, Concealment::Mute] {
        let (out, s) = run(c, &[], 20, 19_200);
        assert_eq!(s.concealed_s, 0.0, "{c:?}");
        assert_eq!(s.underruns, 0, "{c:?}");
        let started = out.iter().position(|f| f.0[0] != 0.0).unwrap();
        assert!(
            max_step(&out[started + 1..]) <= max_step(&tone(1000, 0, 440.0)) * 1.01,
            "{c:?}"
        );
        assert!(s.queue_s > 0.2 && s.queue_s < 0.7, "{s:?}");
    }
}

#[test]
fn short_losses_are_bridged_smoothly() {
    // One lost DAB+ unit (40 ms) in steady 40 ms units, for each concealment.
    for c in [Concealment::Repeat, Concealment::Pitch] {
        let (out, s) = run(c, &[30], 60, 1920);
        assert_eq!(s.concealments, 1, "{c:?}");
        let started = out.iter().position(|f| f.0[0] != 0.0).unwrap();
        let step = max_step(&out[started + 1..]) / max_step(&tone(1000, 0, 440.0));
        assert!(step < 1.5, "{c:?}: step {step}");
        // The level of the bridged gap stays up: no 10 ms stretch is near silence.
        let quiet = out[started..]
            .chunks(480)
            .filter(|w| w.iter().all(|f| f.0[0].abs() < 0.05))
            .count();
        assert_eq!(quiet, 0, "{c:?}");
    }
}

#[test]
fn concealment_holds_then_fades() {
    let (_, s) = run(Concealment::Pitch, &[8, 9], 20, 19_200);
    // 20 ms hold plus a 200 ms fade, then silence.
    assert!((s.concealed_s - 0.22).abs() < 0.01, "{s:?}");
    let (_, s) = run(Concealment::Repeat, &[8, 9], 20, 19_200);
    assert!((s.concealed_s - 0.16).abs() < 0.01, "{s:?}");
    let (_, s) = run(Concealment::Mute, &[8, 9], 20, 19_200);
    assert!(s.concealed_s < 0.011, "{s:?}");
    let (_, s) = run(Concealment::Pitch, &(5..18).collect::<Vec<_>>(), 30, 19_200);
    assert!(s.muted_s > 4.0 && s.concealed_s < 0.4, "{s:?}");
}

#[test]
fn pitch_concealment_continues_the_period() {
    // A 400 Hz tone (120 samples per period): the concealment repeats whole periods, so it stays close to the tone.
    let mut p = Playout::new(Config::new(R as u32, Concealment::Pitch));
    let mut out = Vec::new();
    p.push_sound(&tone(19_200, 0, 400.0));
    p.push_lost(480);
    p.push_sound(&tone(19_200, 19_200 + 480, 400.0));
    p.pull(19_200 + 480, &mut out);
    let expect = tone(480, 19_200, 400.0);
    let err = out[19_200..19_200 + 480]
        .iter()
        .zip(&expect)
        .map(|(a, b)| (a.0[0] - b.0[0]).abs())
        .fold(0.0, f32::max);
    assert!(err < 1e-3, "{err}");
}

#[test]
fn underrun_then_late_loss_report_is_not_played_twice() {
    let mut p = Playout::new(Config::new(R as u32, Concealment::Repeat));
    let mut out = Vec::new();
    for k in 0..3 {
        p.push_sound(&tone(19_200, k * 19_200, 440.0));
    }
    p.pull(3 * 19_200 + 4800, &mut out);
    let s = p.stats();
    assert_eq!(s.underruns, 1);
    // The decoder reports the 100 ms it lost: already filled.
    p.push_lost(4800);
    assert_eq!(p.queued(), 0);
    p.push_sound(&tone(19_200, 62_400, 440.0));
    p.push_lost(960);
    assert_eq!(p.queued(), 19_200 + 960);
}

#[test]
fn rate_control_keeps_the_level() {
    for ppm in [-1000.0, -100.0, 0.0, 100.0, 1000.0f64] {
        let mut p = Playout::new(Config::new(R as u32, Concealment::Repeat));
        let mut produced = 0usize;
        let mut consumed = 0f64;
        let mut out = vec![Frame::default(); 480];
        let (mut lo, mut hi) = (usize::MAX, 0usize);
        for step in 0..12_000 {
            while (produced as f64) < (step as f64 + 1.0) * 480.0 + 19_200.0 {
                p.push_sound(&tone(960, produced, 440.0));
                produced += 960;
            }
            let n = (480.0 * (1.0 + ppm * 1e-6) + consumed.fract()) as usize;
            consumed += 480.0 * (1.0 + ppm * 1e-6);
            out.resize(n, Frame::default());
            p.pull_resampled(&mut out);
            if step > 1000 {
                lo = lo.min(p.queued());
                hi = hi.max(p.queued());
            }
        }
        assert_eq!(p.stats().underruns, 0, "{ppm}");
        assert!(lo >= 4800 && hi <= 48_000, "{ppm}: {lo} to {hi}");
    }
}

#[test]
fn the_queue_is_bounded() {
    let mut p = Playout::new(Config::new(R as u32, Concealment::Pitch));
    for _ in 0..50 {
        p.push_sound(&tone(19_200, 0, 440.0));
    }
    assert!(p.queued() <= 4 * R);
    assert!(p.stats().dropped > 0);
}
