//! `zwave`: the frames of the test signal and of a recording, the channel counts, the declared results and the framework.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_CLOCK_PPM, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::publish::TableDef;
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, Results, StackConfig, Value};
use sdrkit_dev_test::sigmf::read_expected;
use sdrkit_dev_test::Harness;
use sdrkit_stack_ism::family::{from_hex, hex};
use sdrkit_stack_ism::typed::{
    ZWAVE_CHANNELS as CHANNELS, ZWAVE_FRAME, ZWAVE_FRAMES as FRAMES, ZWAVE_NETWORKS,
    ZWAVE_NODES as NODES,
};
use sdrkit_stack_ism::zwave::{self, frame, ZwBurst, ZwOutcome, GEN_CENTRE_HZ, ZWAVE};
use sdrkit_util_bits::crc::catalog::CRC16_SPI_FUJITSU;

fn rate_of(s: &str) -> u8 {
    s[1..].parse().unwrap()
}

/// The cells of column `key` of a declared table as text for people, newest row first.
fn column(r: &Results, def: &TableDef, key: &str) -> Vec<String> {
    crate::util::table(r, def.key)
        .rows_newest_first()
        .map(|(_, row)| {
            let id = row
                .decl()
                .id(key)
                .unwrap_or_else(|| panic!("no column {key}"));
            row.text(id).map(|t| t.to_string()).unwrap_or_default()
        })
        .collect()
}

fn field(r: &Results, key: &str) -> i64 {
    crate::util::number(r, key) as i64
}

#[test]
fn byte_vectors_parse_describe_and_rebuild() {
    let exp = read_expected("ism").unwrap();
    assert_eq!(CRC16_SPI_FUJITSU.checksum(b"123456789"), 0xE5CC);
    let vecs = exp["byte_vectors"]["zwave_mode_schedule_eu"]
        .as_array()
        .unwrap();
    for v in vecs {
        let m = from_hex(v["mpdu_hex"].as_str().unwrap()).unwrap();
        let r = rate_of(v["rate"].as_str().unwrap());
        assert!(frame::check(&m, r), "{}", v["describe"]);
        let p = frame::parse(&m, r).unwrap();
        let want = v["describe"].as_str().unwrap();
        let want = want.split(" (multicast").next().unwrap();
        assert_eq!(frame::describe(&p), want);
        let dst: Vec<u8> = if p.header_type() == 2 {
            m[8..9 + (m[8] & 0x1F) as usize].to_vec()
        } else {
            vec![p.dst]
        };
        assert_eq!(
            frame::build(p.home, p.src, p.fc1, p.fc2, &dst, &p.payload, r),
            m
        );
        if p.header_type() == 2 {
            assert_eq!(p.members, vec![2, 3, 5]);
        }
    }
    // The schedule matches the vectors except the random bytes of the S0 and S2 frames.
    for (i, (r, m)) in zwave::schedule().iter().enumerate() {
        let want = vecs[i]["mpdu_hex"].as_str().unwrap();
        if i == 12 || i == 13 {
            assert_eq!(hex(&m[..10]), want[..20]);
        } else {
            assert_eq!(hex(m), want, "frame {}", i + 1);
        }
        assert_eq!(*r, rate_of(vecs[i]["rate"].as_str().unwrap()));
    }
}

/// The `channels` value of the European region of the data folder.
fn eu() -> Value {
    Value::Text(crate::util::zwave_region("EU").channels_text())
}

/// The European centre of the data folder.
fn eu_centre() -> f64 {
    crate::util::zwave_region("EU").centre_hz
}

/// A harness on the channels and the centre of region `id` of the data folder.
fn harness_in(id: &str) -> Harness {
    let r = crate::util::zwave_region(id);
    Harness::new(&ZWAVE)
        .freq(r.centre_hz)
        .set(&zwave::CHANNELS, &r.channels_text())
        .set(&GEN_CENTRE_HZ, r.centre_hz)
}

fn rx(rate: f64, centre: f64, settings: &[(&str, Value)]) -> Box<dyn Receiver> {
    let cfg = crate::util::stack_with(&ZWAVE, settings);
    let mut r: Box<dyn Receiver> = Box::new(zwave::receiver(&cfg).unwrap());
    r.configure(&InputInfo::new(rate, centre, centre).recording())
        .unwrap();
    r
}

fn feed(r: &mut dyn Receiver, x: &[Complex32]) {
    let mut out = Output::default();
    for c in x.chunks(65536) {
        r.feed(c, &mut out);
        out.clear();
    }
    r.finish(&mut out);
}

#[test]
fn conformance_vector() {
    let rec = sdrkit_dev_test::read_recording("ism/zwave_868_4_1msps")
        .expect("the recording ism/zwave_868_4_1msps");
    // The region of the data folder with the most channels in the recording.
    let region = sdrkit_presets::ism::zwave_region_at(
        &crate::util::zwave_regions(),
        rec.centre_hz,
        rec.rate_hz,
    )
    .cloned()
    .unwrap();
    let mut r = rx(
        rec.rate_hz,
        rec.centre_hz,
        &[
            ("sof_f0_r3", Value::Bool(true)),
            ("channels", Value::Text(region.channels_text())),
        ],
    );
    feed(r.as_mut(), &rec.samples);
    let res = r.results();
    let homes = column(res, &FRAMES, "home_id");
    let routes = column(res, &FRAMES, "route");
    let commands = column(res, &FRAMES, "command");
    assert_eq!(homes.len(), 3, "{homes:?}");
    for (h, src) in [
        ("C0FFEE01", "2 > 1"),
        ("DEADBEEF", "7 > 1"),
        ("12345678", "9 > 1"),
    ] {
        assert!(
            homes.iter().zip(&routes).any(|(x, y)| x == h && y == src),
            "{h} missing: {homes:?}"
        );
    }
    let at = |h: &str| homes.iter().position(|x| x == h).unwrap();
    assert_eq!(
        commands[at("DEADBEEF")],
        "SENSOR_MULTILEVEL Report temperature 21.5 C"
    );
    assert_eq!(commands[at("C0FFEE01")], "SWITCH_BINARY Report off");
}

/// How many frames of the schedule fit in `seconds` (the first at 50 ms, 40 ms between frames), with their acks, S0 and S2.
fn sent_in(seconds: f64, rate: f64) -> (usize, i64, i64, i64) {
    let sched = zwave::schedule();
    let (mut t, mut sent, mut acks, mut s0, mut s2) = (0.05f64, 0usize, 0, 0, 0);
    for k in 0.. {
        let (r, m) = &sched[k % sched.len()];
        let len = zwave::frame_samples(*r, m, rate).len() as f64 / rate;
        if t + len > seconds {
            break;
        }
        sent += 1;
        let p = frame::parse(m, *r).unwrap();
        acks += (p.header_type() == 3) as i64;
        s0 += (p.encryption() == "S0") as i64;
        s2 += (p.encryption() == "S2") as i64;
        t += len + 0.04;
    }
    (sent, acks, s0, s2)
}

#[test]
fn generator_round_trip_eu() {
    let run = harness_in("EU")
        .rate(2e6)
        .seconds(3.0)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 20.0)
        .set(&GEN_OFFSET_HZ, 10e3)
        .set(&GEN_CLOCK_PPM, 100.0)
        .run()
        .unwrap();
    // The harness stops at 3 s, so count what ended by then.
    let (sent, acks, s0, s2) = sent_in(3.0, 2e6);
    let r = &run.results;
    assert!(sent >= 3 * 17);
    assert_eq!(field(r, "frames_ok") as usize, sent, "every frame decoded");
    assert_eq!(
        field(r, "frames_r1") + field(r, "frames_r2") + field(r, "frames_r3"),
        sent as i64
    );
    assert_eq!(field(r, "networks"), 1);
    assert_eq!(column(r, &ZWAVE_NETWORKS, "home_id"), vec!["C0FFEE01"]);
    let nodes = column(r, &NODES, "node");
    for n in 1..=6 {
        assert!(nodes.iter().any(|x| *x == n.to_string()), "node {n}");
    }
    let i = nodes.iter().position(|x| x == "1").unwrap();
    assert_eq!(column(r, &NODES, "role")[i], "controller");
    assert_eq!(
        (
            field(r, "s0_frames"),
            field(r, "s2_frames"),
            field(r, "acks")
        ),
        (s0, s2, acks)
    );
    assert_eq!(field(r, "frames_bad"), 0);
}

/// The frames of each rate (R1, R2, R3) that the schedule sends in `seconds` at `rate`.
fn sent_by_rate(seconds: f64, rate: f64) -> [usize; 3] {
    let sched = zwave::schedule();
    let (mut t, mut sent) = (0.05f64, [0usize; 3]);
    for k in 0.. {
        let (r, m) = &sched[k % sched.len()];
        let len = zwave::frame_samples(*r, m, rate).len() as f64 / rate;
        if t + len > seconds {
            break;
        }
        sent[*r as usize - 1] += 1;
        t += len + 0.04;
    }
    sent
}

/// Chips from tone energies and burst gates at 3 and 1.8 times the floor: at 6 dB in 100 kHz at least 90 % of the R1 frames of
/// the schedule (at least 30) decode with white noise alone, with a 17 kHz carrier error (20 ppm), with a 50 ppm clock and 20 kHz off
/// centre. (R2 and R3 are counted, not bounded: at 6 dB in 100 kHz an R2 chip has Es/N0 = 10 dB and an R3 chip 6 dB, where
/// non-coherent detection of 2-FSK loses 0.34 % and 6.8 % of the bits.)
#[test]
fn tone_chips_near_the_threshold() {
    let secs = 6.5;
    let sent = sent_by_rate(secs, 2e6);
    assert!(sent[0] >= 30, "{sent:?}");
    let mut short = Vec::new();
    for (name, off, ppm) in [
        ("AWGN", 0.0, 0.0),
        ("17 kHz carrier error", 17e3, 0.0),
        ("50 ppm clock", 0.0, 50.0),
        ("20 kHz off centre", -20e3, 0.0),
    ] {
        let run = harness_in("EU")
            .rate(2e6)
            .seconds(secs)
            .seed(7)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, 6.0)
            .set(&GEN_OFFSET_HZ, off)
            .set(&GEN_CLOCK_PPM, ppm)
            .run()
            .unwrap();
        let ok = ["frames_r1", "frames_r2", "frames_r3"].map(|k| field(&run.results, k) as usize);
        println!("{name}: R1, R2, R3 {ok:?} of {sent:?}");
        if ok[0] * 10 < sent[0] * 9 {
            short.push(format!("{name}: R1 {} of {}", ok[0], sent[0]));
        }
    }
    assert!(short.is_empty(), "{short:#?}");
}

/// The lower burst gates open some bursts in noise; the checksum drops them: no frame in 300 trials of 4 s.
#[test]
fn noise_trials_return_no_frames() {
    let (mut frames, mut refused) = (0, 0);
    for seed in 0..300 {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(1000 + seed);
        let x: Vec<Complex32> = (0..2_000_000 * 4)
            .map(|_| rng.complex_normal(2e-4))
            .collect();
        let mut r = rx(2e6, eu_centre(), &[("channels", eu())]);
        feed(r.as_mut(), &x);
        frames += field(r.results(), "frames_ok");
        refused += field(r.results(), "frames_bad");
    }
    println!("noise: {frames} frames, {refused} refused by their checks");
    assert_eq!(frames, 0);
}

#[test]
fn noise_returns_no_frames() {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(1);
    let x: Vec<Complex32> = (0..2_000_000 * 20)
        .map(|_| rng.complex_normal(2e-4))
        .collect();
    let mut r = rx(2e6, eu_centre(), &[("channels", eu())]);
    feed(r.as_mut(), &x);
    assert_eq!(field(r.results(), "frames_ok"), 0);
    assert_eq!(field(r.results(), "nodes"), 0);
}

/// The burst of one frame at `r` on `chan`, as the receiver sees it at 2 Msps around the European centre.
fn burst(r: u8, mpdu: &[u8], chan: f64) -> Vec<Complex32> {
    let rate = 2e6;
    let s = zwave::frame_samples(r, mpdu, rate);
    let mut x = vec![Complex32::default(); 20_000];
    let w = std::f64::consts::TAU * (chan - eu_centre()) / rate;
    x.extend(s.iter().enumerate().map(|(k, v)| {
        let (sin, cos) = (w * k as f64).sin_cos();
        v * Complex32::new(0.1 * cos as f32, 0.1 * sin as f32)
    }));
    x.extend(vec![Complex32::default(); 20_000]);
    x
}

/// Acceptance test 6, second part: a burst whose check fails counts as a failed check and adds no node.
#[test]
fn corrupted_check_counts_and_adds_no_node() {
    for (r, chan) in [(2u8, 868.40e6), (3, 869.85e6)] {
        let (_, good) = zwave::schedule()
            .into_iter()
            .find(|(x, _)| *x == r)
            .unwrap();
        let mut bad = good.clone();
        bad[10] ^= 0x04;
        assert!(frame::check(&good, r) && !frame::check(&bad, r));
        let mut rx1 = rx(2e6, eu_centre(), &[("channels", eu())]);
        feed(rx1.as_mut(), &burst(r, &bad, chan));
        let res = rx1.results();
        assert_eq!(field(res, "frames_ok"), 0, "R{r}");
        assert_eq!(field(res, "nodes") + field(res, "networks"), 0, "R{r}");
        let rates = column(res, &CHANNELS, "rate");
        let failed = column(res, &CHANNELS, "failed_checks");
        let n: i64 = rates
            .iter()
            .zip(&failed)
            .filter(|(x, _)| **x == format!("R{r}"))
            .map(|(_, f)| f.parse::<i64>().unwrap())
            .sum();
        assert_eq!(n, 1, "R{r}");
        // The same frame with its check intact is decoded.
        let mut rx2 = rx(2e6, eu_centre(), &[("channels", eu())]);
        feed(rx2.as_mut(), &burst(r, &good, chan));
        assert_eq!(field(rx2.results(), "frames_ok"), 1, "R{r}");
        assert!(field(rx2.results(), "nodes") > 0);
    }
}

/// G.9959 R1 chips are always 10 or 01, so a pair without a change is a chip error. A 0 bit of the MPDU (chips 01) sent as 00
/// refuses the frame.
#[test]
fn r1_refuses_a_manchester_pair_without_a_change() {
    let (_, mpdu) = zwave::schedule()
        .into_iter()
        .find(|(r, _)| *r == 1)
        .unwrap();
    let mut chips = zwave::frame_chips(1, &mpdu);
    // The chips of the MPDU start after 11 octets of preamble and start of frame, two chips a bit.
    let first = 2 * 8 * 11 + 2 * 8 * 4;
    let k = (first..chips.len() - 2)
        .step_by(2)
        .find(|&k| chips[k] == 0 && chips[k + 1] == 1)
        .unwrap();
    chips[k + 1] = 0;
    let rate = 400e3;
    let samples = zwave::chips_samples(1, &chips, rate);
    let mut s = vec![Complex32::new(1e-3, 0.0); 400];
    s.extend(samples.iter().map(|v| v * 0.1));
    s.extend(vec![Complex32::new(1e-3, 0.0); 400]);
    let b = ZwBurst {
        samples: s,
        rate,
        group_offset_hz: 0.0,
        group_hz: 868.42e6,
        rates: vec![1],
        start: 394,
        noise: 1e-6,
        time: 0.0,
    };
    assert!(!matches!(
        zwave::decode_burst(&b, false),
        ZwOutcome::Frame(_)
    ));
    // The streams split at the refused pair.
    let streams = zwave::r1_streams(&chips);
    assert!(streams.iter().all(|s| s.len() < chips.len() / 2));
    // Without the bad pair the frame decodes.
    let samples = zwave::frame_samples(1, &mpdu, rate);
    let mut s = vec![Complex32::new(1e-3, 0.0); 400];
    s.extend(samples.iter().map(|v| v * 0.1));
    s.extend(vec![Complex32::new(1e-3, 0.0); 400]);
    let good = ZwBurst { samples: s, ..b };
    assert!(matches!(
        zwave::decode_burst(&good, false),
        ZwOutcome::Frame(_)
    ));
}

#[test]
fn every_event_is_declared_and_has_its_frequency() {
    let decl = sdrkit_stack_ism::typed::shape("zwave").results_decl();
    let run = harness_in("EU").seconds(2.0).run().unwrap();
    assert!(run.events.iter().any(|e| e.is(&ZWAVE_FRAME)));
    assert!(crate::util::declared(&decl, &run.events));
    for e in &run.events {
        assert!(e.freq_hz > 800e6, "{} sent at {} Hz", e.kind(), e.freq_hz);
    }
}

/// The channels and the start-of-frame switch refuse a value of the wrong kind or a malformed list; the stack has no
/// region setting.
#[test]
fn a_bad_setting_value_is_refused() {
    let cfg = StackConfig::new(sdrkit_core::Values::defaults(&(ZWAVE.settings)()));
    let mut r = zwave::receiver(&cfg).unwrap();
    assert!(crate::util::set(&mut r, "region", Value::Choice(1)).is_err());
    assert!(crate::util::set(&mut r, "channels", Value::Text("XX".into())).is_err());
    assert!(crate::util::set(&mut r, "channels", Value::Text("868400000/4".into())).is_err());
    assert!(crate::util::set(&mut r, "channels", Value::Choice(1)).is_err());
    assert!(crate::util::set(&mut r, "sof_f0_r3", Value::Text("perhaps".into())).is_err());
    let us = crate::util::zwave_region("US").channels_text();
    assert!(crate::util::set(&mut r, "channels", Value::Text(us)).is_ok());
    assert!(crate::util::set(&mut r, "channels", Value::Text("919800000/123".into())).is_ok());
}

/// The stack runs on the channels of two regions of the data folder: every frame of the test signal decodes, and the
/// channel table holds the channels of the region.
#[test]
fn the_channels_of_two_regions_of_the_data_folder() {
    let rate = 4e6;
    let (sent, _, _, _) = sent_in(2.0, rate);
    for id in ["EU", "ANZ"] {
        let region = crate::util::zwave_region(id);
        let run = harness_in(id).rate(rate).seconds(2.0).run().unwrap();
        let r = &run.results;
        assert_eq!(field(r, "frames_ok") as usize, sent, "{id}");
        assert_eq!(field(r, "frames_bad"), 0, "{id}");
        let freqs = column(r, &CHANNELS, "frequency");
        assert_eq!(freqs.len(), region.channels().len(), "{id}: {freqs:?}");
        assert!(
            column(r, &CHANNELS, "covered").iter().all(|c| c == "yes"),
            "{id}: {:?}",
            column(r, &CHANNELS, "covered")
        );
    }
}

/// With no channel set, every rate is received at the tuned frequency, wherever it is.
#[test]
fn without_channels_the_test_signal_decodes_at_any_frequency() {
    let (sent, _, _, _) = sent_in(2.0, 2e6);
    for hz in [0.0, 433.92e6] {
        let run = Harness::new(&ZWAVE)
            .rate(2e6)
            .seconds(2.0)
            .freq(hz)
            .set(&GEN_CENTRE_HZ, hz)
            .run()
            .unwrap();
        assert_eq!(field(&run.results, "frames_ok") as usize, sent, "{hz}");
    }
}

/// A change of frequency keeps the nodes, the networks and the counts, and drops the counts of the channels of the
/// present band.
#[test]
fn a_change_of_frequency_keeps_the_nodes_and_drops_the_channel_counts() {
    let h = Harness::new(&ZWAVE).rate(2e6).seconds(2.0);
    let mut r = rx(2e6, ZWAVE.tuning.default_hz, &[]);
    feed(&mut *r, &h.generate().unwrap());
    let channel_frames = |r: &dyn Receiver| -> i64 {
        column(r.results(), &CHANNELS, "frames")
            .iter()
            .map(|v| v.parse::<i64>().unwrap_or(0))
            .sum()
    };
    let nodes = column(r.results(), &NODES, "node").len();
    let frames = field(r.results(), "frames_ok");
    assert!(nodes >= 1 && frames > 0);
    assert!(channel_frames(&*r) > 0);
    r.forget();
    assert_eq!(column(r.results(), &NODES, "node").len(), nodes);
    assert_eq!(field(r.results(), "frames_ok"), frames);
    assert_eq!(channel_frames(&*r), 0);
    assert_eq!(r.results().status().snr_db, None);
}
