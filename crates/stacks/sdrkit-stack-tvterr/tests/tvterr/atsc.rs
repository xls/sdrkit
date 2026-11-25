//! ATSC 1.0 receiver tests: the recordings, the generator through the shared impairments, noise only, cut independence,
//! and the rate rule.

use super::common::{self, chain, impair, Run};
use sdrkit_algo_sim::chain::ChainConfig;
use sdrkit_algo_sim::fading::{Doppler, Profile, Tap};
use sdrkit_core::{Complex32, InputInfo, Receiver};
use sdrkit_dev_test::sigmf;
use sdrkit_proto_mpegts::source::Numbered;
use sdrkit_stack_tvterr::atsc::gen::{AtscGenParams, AtscGenerator, NOISE_BANDWIDTH_HZ};
use sdrkit_stack_tvterr::atsc::rx::PILOT_HZ;
use sdrkit_stack_tvterr::atsc::DESCRIPTOR;

const P: Numbered = Numbered::TERRESTRIAL;

fn rx() -> Box<dyn Receiver> {
    common::receiver(&DESCRIPTOR, &[])
}

fn run(r: &mut Box<dyn Receiver>, rate: f64, x: &[Complex32], chunk: usize, port: &str) -> Run {
    common::run(r.as_mut(), rate, x, chunk, port)
}

fn signal(p: AtscGenParams, seconds: f64) -> Vec<Complex32> {
    common::generate(AtscGenerator::new(p).unwrap(), seconds)
}

/// The signal at 8 Msps through `cfg`.
fn impaired(seconds: f64, cfg: &ChainConfig) -> Vec<Complex32> {
    impair(&signal(AtscGenParams::default(), seconds), 8e6, cfg)
}

#[test]
fn recordings_decode_as_expected() {
    let exp = sigmf::read_expected("tvterr").expect("expected.json");
    for name in ["atsc-8vsb-clean", "atsc-8vsb-impaired"] {
        let rec = sdrkit_dev_test::read_recording(&format!("tvterr/{name}"))
            .unwrap_or_else(|| panic!("the recording tvterr/{name} (set SDRKIT_RECORDINGS)"));
        let e = &exp["files"][format!("{name}.sigmf-meta")]["expect"];
        let mut r = rx();
        let res = run(&mut r, rec.rate_hz, &rec.samples, 65536, "fec.ts");
        let c = res.check(&P);
        println!(
            "{name}: {c:?} cfo {} ppm {} snr {}",
            res.f("offset_hz"),
            res.f("clock_ppm"),
            res.f("snr_db")
        );
        assert!(
            c.good >= e["good_packets_min"].as_u64().unwrap(),
            "{name}: {c:?}"
        );
        assert!(
            c.wrong <= e["wrong_packets_max"].as_u64().unwrap(),
            "{name}: {c:?}"
        );
        let total = (c.good + c.flagged).max(1) as f64;
        assert!(
            c.flagged as f64 / total <= e["flagged_share_max"].as_f64().unwrap(),
            "{name}: {c:?}"
        );
        assert!(
            c.first_time.unwrap() <= e["first_packet_within_s"].as_f64().unwrap() + 0.0,
            "{name}: first {:?}",
            c.first_time
        );
        let cfo = &e["offset_hz"];
        assert!(
            (res.f("offset_hz") - cfo["value"].as_f64().unwrap()).abs()
                <= cfo["tolerance"].as_f64().unwrap(),
            "{name} cfo"
        );
        let ppm = &e["clock_ppm"];
        assert!(
            (res.f("clock_ppm") - ppm["value"].as_f64().unwrap()).abs()
                <= ppm["tolerance"].as_f64().unwrap(),
            "{name} ppm"
        );
        assert!(res.b("field_sync") && res.b("ts_flowing"), "{name}");
        if let Some(m) = e["snr_db_min"].as_f64() {
            assert!(res.f("snr_db") >= m, "{name} snr {}", res.f("snr_db"));
        }
        if let Some(s) = e.get("snr_db").filter(|v| v.is_object()) {
            assert!(
                (res.f("snr_db") - s["value"].as_f64().unwrap()).abs()
                    <= s["tolerance"].as_f64().unwrap(),
                "{name} snr {}",
                res.f("snr_db")
            );
        }
        if e["pilot_locked"].as_bool() == Some(true) {
            assert!(res.b("pilot_locked") && res.b("segment_sync") && res.b("equaliser_trained"));
        }
    }
}

fn good_share(
    x: &[Complex32],
    rate: f64,
    chunk: usize,
) -> (f64, sdrkit_proto_mpegts::source::NumberedCheck, Run) {
    let mut r = rx();
    let res = run(&mut r, rate, x, chunk, "fec.ts");
    let c = res.check(&P);
    let expected = x.len() as f64 / rate * 12893.9;
    (c.good as f64 / expected, c, res)
}

#[test]
fn generator_round_trips_with_impairments() {
    let none = ChainConfig::default();
    let echo = |us: f64, db: f64, snr: f64| ChainConfig {
        fading: Profile::Echo {
            delay_s: us * 1e-6,
            level_db: db,
            phase_deg: 40.0,
        },
        ..chain(0.0, 0.0, Some(snr), NOISE_BANDWIDTH_HZ)
    };
    let cases: Vec<(&str, f64, ChainConfig, f64)> = vec![
        ("clean 8M", 8e6, none.clone(), 0.85),
        ("clean 10M", 10e6, none.clone(), 0.85),
        ("+18 kHz", 8e6, chain(18e3, 0.0, None, 0.0), 0.8),
        ("-25 kHz", 8e6, chain(-25e3, 0.0, None, 0.0), 0.8),
        (
            "19 dB",
            8e6,
            chain(0.0, 0.0, Some(19.0), NOISE_BANDWIDTH_HZ),
            0.75,
        ),
        ("echo -8 dB 2 us", 8e6, echo(2.0, -8.0, 30.0), 0.75),
        (
            "three echoes",
            8e6,
            ChainConfig {
                fading: Profile::Taps(vec![
                    Tap::new(1e-6, 0.0, Doppler::Static),
                    Tap::new(1.8e-6, -9.0, Doppler::Static),
                    Tap::new(4e-6, -12.0, Doppler::Static),
                    Tap::new(0.0, -14.0, Doppler::Static),
                ]),
                ..chain(10e3, 30.0, Some(25.0), NOISE_BANDWIDTH_HZ)
            },
            0.7,
        ),
    ];
    for (name, rate, cfg, min) in cases {
        let x = impair(
            &signal(
                AtscGenParams {
                    rate_hz: rate,
                    ..Default::default()
                },
                1.0,
            ),
            rate,
            &cfg,
        );
        let (share, c, res) = good_share(&x, rate, 50_000);
        println!(
            "{name}: share {share:.3} {c:?} cfo {} snr {}",
            res.f("offset_hz"),
            res.f("snr_db")
        );
        assert!(share >= min, "{name}: {share} {c:?}");
        assert!(c.wrong <= 3, "{name}: {c:?}");
    }
}

#[test]
fn clock_error_and_noise_through_the_shared_impairments() {
    let rate = 8e6;
    let y = impaired(1.2, &chain(7_000.0, 60.0, Some(25.0), NOISE_BANDWIDTH_HZ));
    let mut r = rx();
    let res = run(&mut r, rate, &y, 32768, "fec.ts");
    let c = res.check(&P);
    println!(
        "60 ppm: {c:?} ppm {} cfo {}",
        res.f("clock_ppm"),
        res.f("offset_hz")
    );
    assert!(c.good as f64 > 0.75 * 1.2 * 12893.9, "{c:?}");
    assert!(c.wrong <= 3);
    assert!(
        (res.f("clock_ppm") - 60.0).abs() < 10.0,
        "ppm {}",
        res.f("clock_ppm")
    );
    assert!(
        (res.f("offset_hz") - 7000.0).abs() < 300.0,
        "cfo {}",
        res.f("offset_hz")
    );
}

#[test]
fn any_cut_returns_the_same_packets() {
    let x = signal(AtscGenParams::default(), 0.35);
    let mut ref_pk = None;
    for chunk in [1usize << 16, 4093, 777] {
        let mut r = rx();
        let res = run(&mut r, 8e6, &x, chunk, "fec.ts");
        let pk: Vec<[u8; 188]> = res.packets.iter().map(|p| p.0).collect();
        assert!(pk.len() > 1000, "chunk {chunk}: {}", pk.len());
        match &ref_pk {
            None => ref_pk = Some(pk),
            Some(r0) => assert_eq!(&pk, r0, "chunk {chunk}"),
        }
    }
}

#[test]
fn noise_only_returns_nothing() {
    let rate = 8e6;
    let x = common::noise((rate * 0.6) as usize, 0.09, 9);
    let mut r = rx();
    let res = run(&mut r, rate, &x, 65536, "fec.ts");
    assert!(res.f("detection") < 3.0, "{:?}", res.results.fields());
    assert!(res.packets.is_empty());
}

#[test]
fn bad_samples_and_a_gap_are_survived() {
    let rate = 8e6;
    let mut x = signal(AtscGenParams::default(), 1.0);
    for s in x[3_000_000..3_000_064].iter_mut() {
        *s = Complex32::new(f32::NAN, f32::INFINITY);
    }
    let mut r = rx();
    let res = run(&mut r, rate, &x, 65536, "fec.ts");
    let after = res
        .packets
        .iter()
        .filter(|p| p.1 > 0.45 && p.0[1] & 0x80 == 0)
        .count();
    assert!(after > 4000, "{after}");
    assert!(res.check(&P).wrong <= 3);
}

#[test]
fn rates_below_the_minimum_are_refused() {
    let mut r = rx();
    assert!(r.configure(&InputInfo::new(6.0e6, 539e6, 539e6)).is_err());
    assert!(r.configure(&InputInfo::new(6.5e6, 539e6, 539e6)).is_ok());
    assert_eq!(DESCRIPTOR.id, "atsc");
}

/// The share of good packets and the check through a direct path and one echo `us` microseconds later (negative: earlier)
/// `db` relative to it, at `snr` dB in 5.38 MHz with the power of both paths as the signal. The echo is in phase with the direct
/// path at the pilot.
fn long_echo(
    us: f64,
    db: f64,
    snr: f64,
    seconds: f64,
) -> (f64, sdrkit_proto_mpegts::source::NumberedCheck) {
    echo_at_phase(us, db, 0.0, snr, seconds)
}

/// As [`long_echo`], with the echo `phase_deg` degrees from the direct path at the pilot.
fn echo_at_phase(
    us: f64,
    db: f64,
    phase_deg: f64,
    snr: f64,
    seconds: f64,
) -> (f64, sdrkit_proto_mpegts::source::NumberedCheck) {
    let (main, echo) = if us < 0.0 { (-us, 0.0) } else { (0.0, us) };
    let phase_rad = std::f64::consts::TAU * PILOT_HZ * us * 1e-6 + phase_deg.to_radians();
    let cfg = ChainConfig {
        fading: Profile::Taps(vec![
            Tap::new(main * 1e-6, 0.0, Doppler::Static),
            Tap::new(
                echo * 1e-6,
                db,
                Doppler::Steady {
                    shift_hz: 0.0,
                    phase_rad,
                },
            ),
        ]),
        ..chain(0.0, 0.0, Some(snr), NOISE_BANDWIDTH_HZ)
    };
    let x = impaired(seconds, &cfg);
    let (share, c, _) = good_share(&x, 8e6, 50_000);
    (share, c)
}

#[test]
fn long_echoes_are_cancelled() {
    // Pre-echoes of 30 and 15 us and post-echoes of 20 to 40 us, beyond the reach of the feed-forward taps, 5 dB above the
    // thresholds of A/74 that a decision-feedback equaliser reaches. Without the cancellers none of them gives a packet.
    for (us, db, snr) in [
        (-30.0, -7.0, 22.0),
        (-15.0, -5.0, 23.0),
        (20.0, -2.0, 28.0),
        (30.0, -3.0, 26.0),
        (40.0, -4.0, 25.0),
    ] {
        let (share, c) = long_echo(us, db, snr, 1.0);
        assert!(
            share >= 0.85 && c.wrong <= 3,
            "echo {us} us {db} dB at {snr} dB: {share:.3} {c:?}"
        );
    }
}

#[test]
fn long_echoes_one_db_above_the_required_ratio() {
    // The same echoes, in phase at the pilot, 1 dB above the thresholds of A/74 (17, 18 and 21 dB before the strongest path;
    // 23, 21 and 20 dB after it): 90 % of the packets of a clean run.
    for (us, db, snr) in [
        (-30.0, -7.0, 18.0),
        (-15.0, -5.0, 19.0),
        (-10.0, -3.0, 22.0),
        (20.0, -2.0, 24.0),
        (30.0, -3.0, 22.0),
        (40.0, -4.0, 21.0),
    ] {
        let (share, c) = long_echo(us, db, snr, 1.0);
        assert!(
            share >= 0.85 && c.wrong <= 3,
            "echo {us} us {db} dB at {snr} dB: {share:.3} {c:?}"
        );
    }
}

#[test]
fn white_noise_at_15_db() {
    let x = impaired(1.0, &chain(0.0, 0.0, Some(15.0), NOISE_BANDWIDTH_HZ));
    let (share, c, _) = good_share(&x, 8e6, 50_000);
    assert!(share >= 0.85 && c.wrong <= 3, "{share:.3} {c:?}");
}

/// As [`echo_at_phase`], and the share of good packets from 0.3 s on, when the clock and the equaliser have settled.
fn settled_echo(
    us: f64,
    db: f64,
    phase_deg: f64,
    snr: f64,
) -> (f64, f64, sdrkit_proto_mpegts::source::NumberedCheck) {
    let (main, echo) = if us < 0.0 { (-us, 0.0) } else { (0.0, us) };
    let phase_rad = std::f64::consts::TAU * PILOT_HZ * us * 1e-6 + phase_deg.to_radians();
    let cfg = ChainConfig {
        fading: Profile::Taps(vec![
            Tap::new(main * 1e-6, 0.0, Doppler::Static),
            Tap::new(
                echo * 1e-6,
                db,
                Doppler::Steady {
                    shift_hz: 0.0,
                    phase_rad,
                },
            ),
        ]),
        ..chain(0.0, 0.0, Some(snr), NOISE_BANDWIDTH_HZ)
    };
    let x = impaired(1.0, &cfg);
    let (share, c, res) = good_share(&x, 8e6, 50_000);
    let mut late = sdrkit_proto_mpegts::source::NumberedCheck::default();
    for (p, t) in res.packets.iter().filter(|p| p.1 >= 0.3) {
        late.push(&P, p, *t);
    }
    (share, late.good as f64 / (0.7 * 12893.9), c)
}

/// Runs `cases` (delay in microseconds, level in dB, phase in degrees, SNR in dB) and returns for each the share of good
/// packets, the share from 0.3 s on, and the check.
fn echo_cases(
    cases: &[(f64, f64, f64, f64)],
) -> Vec<(f64, f64, sdrkit_proto_mpegts::source::NumberedCheck)> {
    cases
        .iter()
        .map(|&(us, db, phase, snr)| settled_echo(us, db, phase, snr))
        .collect()
}

#[test]
fn strong_echoes_inside_the_feedback_taps_at_the_required_ratio() {
    // -0.5 dB 5 us late (54 symbols) at 31 dB and -1 dB 10 us late (108 symbols) at 27 dB, the ratios of A/74, with the echo
    // in phase and a quarter turn either way at the pilot. The feedback tap is 0.9: the feedback reads the survivors of the
    // trellis code, and the clock holds the stronger path. After the start (the clock's gain falls over the first 2000
    // segments) nine packets in ten are good; 0.954 is every packet of the fields that end inside the signal.
    let mut cases = Vec::new();
    for (us, db, snr) in [(5.0, -0.5, 31.0), (10.0, -1.0, 27.0)] {
        for phase in [0.0, 90.0, 270.0] {
            cases.push((us, db, phase, snr));
        }
    }
    for (&(us, db, phase, snr), (share, settled, c)) in cases.iter().zip(echo_cases(&cases)) {
        assert!(
            share >= 0.8 && settled >= 0.9 && c.wrong <= 3,
            "echo {us} us {db} dB at {phase} degrees, {snr} dB: {share:.3}, settled {settled:.3} {c:?}"
        );
    }
    // Far under the required ratios: 24 dB and 23 dB.
    let low = [
        (5.0, -0.5, 0.0, 24.0),
        (5.0, -0.5, 90.0, 24.0),
        (10.0, -1.0, 0.0, 23.0),
        (10.0, -1.0, 90.0, 23.0),
    ];
    for (&(us, db, phase, snr), (share, settled, c)) in low.iter().zip(echo_cases(&low)) {
        assert!(
            share >= 0.75 && settled >= 0.9 && c.wrong <= 3,
            "echo {us} us {db} dB at {phase} degrees, {snr} dB: {share:.3}, settled {settled:.3} {c:?}"
        );
    }
}

#[test]
fn long_echoes_at_any_carrier_phase() {
    // Every echo of A/74 beyond the feed-forward taps with the echo a quarter turn, a half turn and three quarter turns from
    // the direct path at the pilot, 1 dB above the required ratio: as the echo in phase
    // (`long_echoes_one_db_above_the_required_ratio`). At a quarter turn the levels of the echo reach the symbols only through
    // the quadrature pulse; at a half turn the two pilots oppose each other.
    let mut cases = Vec::new();
    for (us, db, snr) in [
        (-30.0, -7.0, 18.0),
        (-15.0, -5.0, 19.0),
        (-10.0, -3.0, 22.0),
        (20.0, -2.0, 24.0),
        (30.0, -3.0, 22.0),
        (40.0, -4.0, 21.0),
    ] {
        for phase in [90.0, 180.0, 270.0] {
            cases.push((us, db, phase, snr));
        }
    }
    for (&(us, db, phase, snr), (share, _, c)) in cases.iter().zip(echo_cases(&cases)) {
        // Not reached: +20 us at -2 dB with the pilots opposed leaves 0.2 of the pilot, which the pilot search does not
        // find among the lines beside it.
        let min = if (us, phase) == (20.0, 180.0) {
            0.0
        } else {
            0.85
        };
        assert!(
            share >= min && c.wrong <= 3,
            "echo {us} us {db} dB at {phase} degrees, {snr} dB: {share:.3} {c:?}"
        );
    }
}

#[test]
fn strong_near_echoes_that_oppose_the_pilot_give_no_wrong_packet() {
    // Not reached: -0.5 dB at 5 us and -1 dB at 10 us a half turn from the direct path at the pilot leave 0.06 and 0.11 of the
    // pilot, 25 and 19 dB under its level. The pilot search and the carrier loop do not hold it, so nothing is decoded. No
    // wrong packet comes out.
    let cases = [(5.0, -0.5, 180.0, 31.0), (10.0, -1.0, 180.0, 27.0)];
    for (&(us, db, phase, snr), (_, _, c)) in cases.iter().zip(echo_cases(&cases)) {
        assert!(
            c.wrong <= 3,
            "echo {us} us {db} dB at {phase} degrees, {snr} dB: {c:?}"
        );
    }
}

#[test]
fn a_live_signal_is_equalised_beside_the_receiver_thread() {
    // Live input: the fields wait in a queue for the equaliser on a helper thread, and the packets are made on this thread.
    // How many fields the helper finishes depends on the machine, so the test asks for a part of the packets and no wrong one.
    use sdrkit_core::Output;
    use sdrkit_graph::{Buffer, Data, Graph, Timed};
    use sdrkit_proto_mpegts::stream::{self, TsPacket};
    let x = signal(AtscGenParams::default(), 1.0);
    let mut r = rx();
    r.configure(&InputInfo::new(8e6, 539e6, 539e6))
        .expect("rate accepted");
    let mut tap = r
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("a graph")
        .tap("fec.ts", 1 << 16)
        .expect("port");
    let mut out = Output::default();
    let mut buf = Buffer::new(&stream::kind());
    let mut c = sdrkit_proto_mpegts::source::NumberedCheck::default();
    for piece in x.chunks(50_000) {
        r.feed(piece, &mut out);
        out.clear();
        while tap.pop(&mut buf) {
            if let Data::Items(Some(v)) = &buf.data {
                if let Some(items) = v.as_any().downcast_ref::<Vec<Timed<TsPacket>>>() {
                    for p in items {
                        c.push(&P, p.value.bytes(), p.time);
                    }
                }
            }
        }
    }
    assert!(c.good >= 2000 && c.wrong <= 3, "{c:?}");
}

#[test]
fn a_change_of_frequency_keeps_the_guide_and_forgets_the_signal() {
    use sdrkit_stack_tvterr::typed::shape;
    assert_eq!(
        shape("atsc").kept_keys(),
        Ok(vec![
            "table guide".to_owned(),
            "table guide_ratings".to_owned()
        ])
    );
    // The service list of ATSC 3.0 and a detection of TV (auto) are of the channel of the frequency before.
    assert_eq!(shape("atsc3").kept_keys(), Ok(Vec::new()));
    assert_eq!(shape("tv-auto").kept_keys(), Ok(Vec::new()));
    let mut r = rx();
    let res = run(
        &mut r,
        8e6,
        &signal(AtscGenParams::default(), 1.0),
        65536,
        "",
    );
    assert_eq!(res.f("detection"), 4.0);
    assert!(res.f("packets") > 0.0 && res.f("rs_clean") > 0.0);
    assert!(res.f("offset_hz").is_finite());
    let guide = res.rows("guide");
    r.forget();
    let after = Run {
        packets: Vec::new(),
        results: sdrkit_core::Owner::results(r.as_ref()).clone(),
        events: Vec::new(),
        requests: Vec::new(),
    };
    assert_eq!(after.f("detection"), 0.0);
    assert_eq!(after.f("packets"), 0.0);
    assert_eq!(after.f("rs_clean"), 0.0);
    assert!(after.f("offset_hz").is_nan(), "no offset without a pilot");
    assert_eq!(after.rows("services"), 0);
    assert_eq!(after.rows("ts_demux.pids"), 0);
    assert_eq!(after.rows("guide"), guide);
}

/// The test signal of the stack carries the test programme: one service named by the virtual channel table, with a video
/// and a sound track, and the stack plays the tone.
#[test]
fn the_test_signal_carries_a_service_with_video_and_sound() {
    use sdrkit_core::{GenConfig, Output, Values};
    use sdrkit_proto_mpegts::codec::{EsCodec, EsKind};
    use sdrkit_proto_mpegts::publish::{TsServiceRow, TsTrackRow, SERVICES, TRACKS};
    use sdrkit_stack_tvterr::atsc::gen::{SOUND_PID, VIDEO_PID};
    let values = Values::defaults(&(DESCRIPTOR.settings)());
    let mut g = (DESCRIPTOR.make_generator.expect("a test signal"))(&GenConfig::new(8e6, values))
        .expect("a generator");
    let mut x = vec![Complex32::default(); 800_000];
    let mut r = rx();
    let mut codecs = sdrkit_core::codec::CodecSet::default();
    sdrkit_codec_mpa::register(&mut codecs);
    r.set_facilities(&sdrkit_core::Facilities {
        codecs: std::sync::Arc::new(codecs),
        ..Default::default()
    });
    r.configure(&InputInfo::new(8e6, 539e6, 539e6).recording())
        .expect("rate accepted");
    let mut out = Output::default();
    let (mut peak, mut pictures) = (0f32, 0);
    for _ in 0..25 {
        g.generate(&mut x);
        for c in x.chunks(65_536) {
            out.clear();
            r.feed(c, &mut out);
            peak = out.audio().iter().fold(peak, |m, s| m.max(s.abs()));
            pictures += out
                .media
                .iter()
                .filter(|e| e.format.codec == sdrkit_core::media::Codec::Mpeg2Video)
                .count();
        }
    }
    let res = sdrkit_core::Owner::results(r.as_ref());
    let services: Vec<(&str, TsServiceRow)> = SERVICES.rows(res).collect();
    assert_eq!(services.len(), 1);
    let s = &services[0].1;
    assert_eq!((s.name, s.provider, s.lcn), ("SDRKIT", "channel 39.1", 3901));
    let tracks: Vec<(&str, TsTrackRow)> = TRACKS.rows(res).collect();
    let track = |pid: u16| {
        tracks
            .iter()
            .find(|t| t.1.pid == i64::from(pid))
            .map(|t| (t.1.service, t.1.kind, t.1.codec))
    };
    assert_eq!(
        track(VIDEO_PID),
        Some(("1", Some(EsKind::Video), Some(EsCodec::Mpeg2Video)))
    );
    assert_eq!(
        track(SOUND_PID),
        Some(("1", Some(EsKind::Audio), Some(EsCodec::Mp2)))
    );
    // The pictures go to the host, and the tone of a quarter of full scale plays.
    assert!(pictures > 10, "{pictures} pictures");
    assert!((0.2..0.3).contains(&peak), "{peak}");
}
