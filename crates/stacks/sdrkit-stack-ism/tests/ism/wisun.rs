//! `wisun`: the generator round trip, the bit-error correction, mode switches, noise, settings, events and saved state.

use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_CLOCK_PPM, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::owner::Owner;
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, Record, Results, StackConfig, Value};
use sdrkit_dev_test::Harness;
use sdrkit_stack_ism::typed::{WisunFrameRow, WisunLinkRow, WisunNetworkRow, WisunNodeRow};
use sdrkit_stack_ism::wisun::{self, frame, FrameOptions, CLASSES, GEN_CENTRE_HZ, RANGES, WISUN};

fn field(r: &Results, k: &str) -> f64 {
    crate::util::number(r, k)
}

fn rows<'a>(r: &'a Results, t: &str) -> Vec<(&'a str, &'a Record)> {
    crate::util::table(r, t).rows_newest_first().collect()
}

fn defaults() -> StackConfig {
    StackConfig::new(sdrkit_core::Values::defaults(&(WISUN.settings)()))
}

/// The settings of the plan `id` of the data folder.
fn plan_settings(id: &str) -> Vec<(String, Value)> {
    crate::util::wisun_plan(id).settings()
}

/// A harness on the plan `id` of the data folder, tuned to and with the test signal around `centre_hz`.
fn harness_in(id: &str, centre_hz: f64) -> Harness {
    let p = crate::util::wisun_plan(id);
    Harness::new(&WISUN)
        .freq(centre_hz)
        .set(&RANGES, &p.ranges_text())
        .set(&CLASSES, &p.classes_text())
        .set(&GEN_CENTRE_HZ, centre_hz)
}

/// Feeds `x` at `rate` centred on 921.5 MHz to a receiver on the ANZ plan of the data folder, and ends the stream.
fn feed(rate: f64, x: &[Complex32]) -> Results {
    let anz = plan_settings("ANZ");
    let set: Vec<(&str, Value)> = anz.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let mut r = wisun::receiver(&crate::util::stack_with(&WISUN, &set)).unwrap();
    r.configure(&InputInfo::new(rate, 921.5e6, 921.5e6).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(65536) {
        r.feed(c, &mut out);
        out.clear();
    }
    r.finish(&mut out);
    r.results().clone()
}

/// The frames the schedule sends in `seconds` at 5 Msps, with their gaps.
fn sent_in(seconds: f64) -> usize {
    let rate = 5e6;
    let (mut t, mut n) = (0.002f64, 0);
    for (psdu, slow, _, _) in wisun::schedule().iter().cycle() {
        let kb = if *slow { 50.0 } else { 100.0 };
        let h = if *slow { 1.0 } else { 0.5 };
        let len = wisun::frame_samples(psdu, kb, h, rate, FrameOptions::default())
            .unwrap()
            .len() as f64
            / rate;
        if t + len > seconds {
            break;
        }
        n += 1;
        t += len + 0.003;
    }
    n
}

/// Generates 0.2 s at 5 Msps with `h` and runs the receiver on the ANZ plan at 921.5 MHz.
fn run(h: Harness) -> sdrkit_dev_test::Run {
    let x = h.rate(5e6).seconds(0.2).generate().unwrap();
    harness_in("ANZ", 921.5e6).rate(5e6).feed(&x).unwrap()
}

#[test]
fn generator_round_trip() {
    let run = run(harness_in("ANZ", 921.5e6)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 15.0)
        .set(&GEN_OFFSET_HZ, 20e3)
        .set(&GEN_CLOCK_PPM, 40.0));
    let r = &run.results;
    let sent = sent_in(0.2) as f64;
    let frames = rows(r, "frames");
    // The meter is 8 dB down, 7 dB over the noise in 400 kHz: its bursts are at the detector's open level (5 times the floor),
    // so one of them can stay undetected.
    assert!(
        field(r, "frames_ok") >= sent - 1.0,
        "{} of {sent}: bad {} {:?}",
        field(r, "frames_ok"),
        field(r, "frames_bad"),
        frames
            .iter()
            .map(|f| f.1.at(&WisunFrameRow::TYPE).unwrap_or(""))
            .collect::<Vec<_>>()
    );
    let nets = rows(r, "networks");
    assert_eq!(nets.len(), 1, "{nets:?}");
    assert_eq!(nets[0].0, "0x7A31");
    assert_eq!(nets[0].1.at(&WisunNetworkRow::NAME), Some("Test-FAN"));
    assert_eq!(
        nets[0].1.at(&WisunNetworkRow::BORDER_ROUTER),
        Some("00:12:4B:FF:FE:1A:2B:3C")
    );
    let nodes = rows(r, "nodes");
    let meter = &nodes
        .iter()
        .find(|n| n.0 == "02:A1:B2:FF:FE:C3:D4:E5")
        .expect("the meter")
        .1;
    assert_eq!(meter.at(&WisunNodeRow::ROLE), Some("joining"));
    assert_eq!(
        meter.at(&WisunNodeRow::AUTHENTICATOR),
        Some("00:12:4B:FF:FE:1A:2B:3C")
    );
    assert!(meter.at(&WisunNodeRow::EAP).is_some_and(|e| !e.is_empty()));
    let links = rows(r, "links");
    let l = &links
        .iter()
        .find(|l| l.0 == "02:A1:B2:FF:FE:C3:D4:E5 > 00:0D:6F:FF:FE:44:55:66")
        .expect("the link meter > router")
        .1;
    // the EAPOL frame and three Data frames of each loop, all unicast meter > router, each Data frame acked
    let frames_sent = l.at(&WisunLinkRow::FRAMES).unwrap();
    let acks = l.at(&WisunLinkRow::ACKS).unwrap();
    assert!(frames_sent >= 4 && acks >= 3, "{l:?}");
    assert_eq!(l.at(&WisunLinkRow::RSL_DBM), Some(-86.0));
    assert!(frames
        .iter()
        .any(|f| f.1.at(&WisunFrameRow::KBPS) == Some(50)));
    let d = sdrkit_stack_ism::typed::shape("wisun").results_decl();
    assert!(!run.events.is_empty());
    assert!(crate::util::declared(&d, &run.events));
    for e in &run.events {
        assert!(e.freq_hz > 9e8, "{} at {} Hz", e.kind(), e.freq_hz);
    }
}

#[test]
fn bit_errors_corrected_and_never_wrong() {
    let mut p: Vec<u8> = (0..60u8).collect();
    frame::add_fcs(&mut p, false);
    let good = p.clone();
    let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
    let mut wrong = 0;
    for _ in 0..2000 {
        let mut q = good.clone();
        let mut rel: Vec<f32> = (0..q.len() * 8)
            .map(|_| 0.5 + rng.uniform() as f32)
            .collect();
        for _ in 0..20 {
            let b = rng.below((q.len() * 8) as u64) as usize;
            q[b / 8] ^= 1 << (b % 8);
            rel[b] = rng.uniform() as f32;
        }
        if frame::lrb_correct(&mut q, &rel).is_some() && q != good {
            wrong += 1;
        }
    }
    assert_eq!(wrong, 0);
}

#[test]
fn a_frame_with_the_mode_switch_bit_still_decodes() {
    let rate = 2e6;
    let psdu = wisun::schedule()[0].0.clone();
    let o = FrameOptions {
        mode_switch: true,
        ..Default::default()
    };
    let mut f = wisun::frame_samples(&psdu, 100.0, 0.5, rate, o).unwrap();
    for s in &mut f {
        *s *= 0.1;
    }
    Nco::new(rate, 915.4e6 + 6.0 * 400e3 - 921.5e6).mix_in_place(&mut f);
    let mut x = vec![Complex32::default(); 100_000];
    x.extend(f);
    x.extend(vec![Complex32::default(); 100_000]);
    let r = feed(rate, &x);
    assert_eq!(field(&r, "frames_ok"), 1.0, "{:?}", r.fields());
}

#[test]
fn noise_returns_nothing() {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
    let x: Vec<Complex32> = (0..2_000_000 * 2)
        .map(|_| rng.complex_normal(2e-4))
        .collect();
    let r = feed(2e6, &x);
    assert_eq!(field(&r, "frames_ok"), 0.0);
}

/// A malformed plan is refused, and the stack has no region setting.
#[test]
fn a_bad_plan_is_refused() {
    let mut g = wisun::receiver(&defaults()).unwrap();
    assert!(crate::util::set(&mut g, "region", Value::Choice(1)).is_err());
    assert!(crate::util::set(&mut g, "ranges", Value::Text("Atlantis".into())).is_err());
    assert!(crate::util::set(&mut g, "ranges", Value::Text("928000000-915000000".into())).is_err());
    assert!(crate::util::set(&mut g, "classes", Value::Text("915200000/200000".into())).is_err());
    assert!(crate::util::set(
        &mut g,
        "classes",
        Value::Text("915200000/200000/64/50".into())
    )
    .is_err());
    assert!(crate::util::set(&mut g, "classes", Value::Choice(1)).is_err());
    for (k, v) in plan_settings("NA") {
        assert!(crate::util::set(&mut g, &k, v).is_ok(), "{k}");
    }
}

/// The stack runs on the plans of two regions of the data folder: the frames of the test signal decode on channels that
/// the plan numbers.
#[test]
fn the_plans_of_two_regions_of_the_data_folder() {
    let sent = sent_in(0.2) as f64;
    for (id, centre) in [("ANZ", 921.5e6), ("NA", 915e6)] {
        let h = harness_in(id, centre).rate(5e6).seconds(0.2);
        let x = h.generate().unwrap();
        let run = h.feed(&x).unwrap();
        let r = &run.results;
        assert!(
            field(r, "frames_ok") >= sent - 1.0,
            "{id}: {} of {sent}",
            field(r, "frames_ok")
        );
        assert_eq!(rows(r, "networks").len(), 1, "{id}");
        assert!(
            rows(r, "frames")
                .iter()
                .all(|f| f.1.at(&WisunFrameRow::CH).is_some_and(|c| !c.is_empty())),
            "{id}"
        );
    }
}

/// With no plan set, the test signal decodes on the grids through the tuned frequency, and no channel is numbered.
#[test]
fn without_a_plan_the_test_signal_decodes_at_any_frequency() {
    let sent = sent_in(0.2) as f64;
    for hz in [0.0, 433.92e6] {
        let h = Harness::new(&WISUN)
            .rate(5e6)
            .seconds(0.2)
            .freq(hz)
            .set(&GEN_CENTRE_HZ, hz);
        let run = h.feed(&h.generate().unwrap()).unwrap();
        let r = &run.results;
        assert!(
            field(r, "frames_ok") >= sent - 1.0,
            "{hz}: {} of {sent}",
            field(r, "frames_ok")
        );
        assert!(rows(r, "frames")
            .iter()
            .all(|f| f.1.at(&WisunFrameRow::CH).is_none()));
    }
}

#[test]
fn below_the_minimum_rate_the_status_says_why() {
    let mut g = wisun::receiver(&defaults()).unwrap();
    assert!(g.configure(&InputInfo::new(1e6, 921.5e6, 921.5e6)).is_err());
    assert_eq!(
        g.results().status().summary,
        "Wi-SUN FAN needs a sample rate of at least 2 Msps"
    );
}

#[test]
fn saved_state_comes_back() {
    let rate = 5e6;
    let h = Harness::new(&WISUN).rate(rate).seconds(0.2);
    let mut x = h.generate().unwrap();
    x.extend(vec![Complex32::default(); (rate * 0.3) as usize]);
    let mut rx: Box<dyn Receiver> = h.receiver().unwrap();
    rx.configure(&InputInfo::new(rate, 921.5e6, 921.5e6).recording())
        .unwrap();
    let mut out = Output::default();
    rx.feed(&x, &mut out);
    let nodes = crate::util::table(rx.results(), "nodes").len();
    assert!(nodes >= 3);
    let state = rx.save_state().unwrap();
    let mut back = h.receiver().unwrap();
    back.configure(&InputInfo::new(rate, 921.5e6, 921.5e6).recording())
        .unwrap();
    back.restore_state(&state).unwrap();
    back.feed(&vec![Complex32::default(); 500_000], &mut out);
    assert_eq!(crate::util::table(back.results(), "nodes").len(), nodes);
}

/// A change of frequency keeps the networks, the nodes, the links and the counts, and drops the hop chart of the present
/// band.
#[test]
fn a_change_of_frequency_keeps_the_networks_and_drops_the_hops() {
    let rate = 5e6;
    let h = Harness::new(&WISUN).rate(rate).seconds(0.2);
    let mut rx = wisun::receiver(&defaults()).unwrap();
    rx.configure(&InputInfo::new(rate, 921.5e6, 921.5e6).recording())
        .unwrap();
    crate::util::feed_all(&mut rx, &h.generate().unwrap());
    let networks = rows(rx.results(), "networks").len();
    let frames = field(rx.results(), "frames_ok");
    assert!(networks >= 1 && frames > 0.0);
    assert!(crate::util::points(rx.results(), "hops") > 0);
    rx.forget();
    assert_eq!(rows(rx.results(), "networks").len(), networks);
    assert_eq!(field(rx.results(), "frames_ok"), frames);
    assert_eq!(crate::util::points(rx.results(), "hops"), 0);
    assert_eq!(rx.results().status().snr_db, None);
}

/// The burst detectors measure their floor on the first samples. When these are weaker than the rest (the samples of the
/// former rate after a change of rate, for example), a detector sees a burst that does not end; it measures again at the
/// end of the longest burst and finds the frames after it.
#[test]
fn a_floor_measured_on_weaker_samples_recovers() {
    let rate = 20e6;
    let x = Harness::new(&WISUN)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 20.0)
        .rate(rate)
        .seconds(1.0)
        .generate()
        .unwrap();
    // 1 ms of the noise before the first burst, 10 dB down.
    let lead = (rate * 1e-3) as usize;
    let mut y: Vec<Complex32> = x[..lead].iter().map(|z| z * 0.3).collect();
    y.extend_from_slice(&x[..x.len() - lead]);
    let run = Harness::new(&WISUN).rate(rate).feed(&y).unwrap();
    let ok = field(&run.results, "frames_ok");
    // The bursts of the first 0.35 s can be lost while the floor is wrong.
    let after = (sent_in(1.0) - sent_in(0.4)) as f64;
    assert!(ok >= after, "{ok} frames, {after} sent after 0.4 s");
}

/// The frames of the test signal of `plan` around `centre_hz` in `seconds` at `rate`: (start s, end s).
fn sent_frames(plan: &frame::Plan, centre_hz: f64, rate: f64, seconds: f64) -> Vec<(f64, f64)> {
    let half = wisun::gen_half_span(&sdrkit_core::Values::defaults(&(WISUN.settings)()));
    let lim = half.min(0.45 * rate - 200e3);
    let (fast, slow) = wisun::test_modes(&frame::channels(plan, centre_hz, lim)).unwrap();
    let (mut t, mut out) = (0.002f64, Vec::new());
    for (psdu, is_slow, _, _) in wisun::schedule().iter().cycle() {
        let (kb, h) = if *is_slow { slow } else { fast };
        let o = FrameOptions::default();
        let n = wisun::frame_samples(psdu, f64::from(kb), f64::from(h), rate, o).unwrap();
        let len = n.len() as f64 / rate;
        if t + len > seconds {
            break;
        }
        out.push((t, t + len));
        t += len + 0.003;
    }
    out
}

/// Every frame of the test signal decodes, on every plan of the data folder (the 50 kbit/s index-0.5 classes of Europe and
/// India and the single class of China 470 among them), at the least rate and at the stack's rate, at the SNR of `verify`.
#[test]
fn every_frame_decodes_on_every_plan() {
    let secs = 0.3;
    let plans = sdrkit_presets::ism::read_wisun_bands(
        &crate::util::data_folder().join(sdrkit_presets::ism::WISUN_BANDS_FILE),
    )
    .unwrap();
    for p in &plans {
        let (lo, hi) = p.ranges_hz[0];
        let centre = ((lo + hi) / 2.0 / 1e5).round() * 1e5;
        let plan = frame::Plan::parse(&p.ranges_text(), &p.classes_text()).unwrap();
        for rate in [wisun::MIN_RATE_HZ, WISUN.tuning.rate_hz] {
            let h = harness_in(&p.id, centre)
                .rate(rate)
                .seconds(secs)
                .set(&GEN_NOISE, NoiseKind::Snr)
                .set(&GEN_SNR_DB, 20.0);
            let run = h.feed(&h.generate().unwrap()).unwrap();
            let r = &run.results;
            let sent = sent_frames(&plan, centre, rate, secs);
            let times: Vec<f64> = rows(r, "frames")
                .iter()
                .filter_map(|f| f.1.at(&WisunFrameRow::TIME_S))
                .collect();
            let lost: Vec<f64> = sent
                .iter()
                .filter(|s| !times.iter().any(|&t| t >= s.0 - 2e-3 && t <= s.1))
                .map(|s| s.0)
                .collect();
            assert!(
                lost.is_empty() && field(r, "frames_ok") == sent.len() as f64,
                "{} at {rate}: {} of {}, lost at {lost:?}",
                p.id,
                field(r, "frames_ok"),
                sent.len()
            );
        }
    }
}
