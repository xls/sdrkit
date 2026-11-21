//! `sigfox`: the frame layer, the joint decoder of the repetitions, the conformance vector, the test signal through the standard
//! impairments, and the Sigfox chain inside `ism`.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_DRIFT_HZ_S, GEN_NOISE, GEN_SNR_DB};
use sdrkit_core::owner::Owner;
use sdrkit_core::{Choice, Complex32, Event, InputInfo, Output, Receiver, StackConfig, Value};
use sdrkit_dev_test::sigmf::read_expected;
use sdrkit_dev_test::Harness;
use sdrkit_graph::StackGraph;
use sdrkit_stack_ism::sigfox::{
    self, build_frame, decode_ranks, encode_rank, match_frame_type, parse_frame, Demodulated,
    Joint, Messages, MessagesSink, SoftFrame, PREAMBLE, SIGFOX,
};

const DIAL: f64 = 868.13e6;

fn rx(rate: f64, dial: f64, cfg: &StackConfig) -> StackGraph {
    let mut r = sigfox::receiver(cfg).unwrap();
    r.configure(&InputInfo::new(rate, dial, dial).recording())
        .unwrap();
    r
}

/// Feeds `x` and ends the stream; returns the messages and the events.
fn feed(r: &mut StackGraph, x: &[Complex32]) -> (Messages, Vec<Event>) {
    let mut out = Output::default();
    let mut events = Vec::new();
    for c in x.chunks(32768) {
        r.feed(c, &mut out);
        events.append(&mut out.events);
        out.clear();
    }
    r.finish(&mut out);
    events.append(&mut out.events);
    let m = r
        .graph()
        .component_as::<MessagesSink>("messages")
        .expect("the message list")
        .messages()
        .clone();
    (m, events)
}

/// The test signal of `h` through its impairments, decoded.
fn run(h: &Harness) -> (Messages, Vec<Event>) {
    let x = h.generate().expect("a test signal");
    let mut r = rx(400e3, DIAL, &StackConfig::new(h.values().unwrap()));
    feed(&mut r, &x)
}

fn harness(bit_rate: &str, reps: &str, seconds: f64) -> Harness {
    Harness::new(&SIGFOX)
        .rate(400e3)
        .freq(DIAL)
        .set(
            &sigfox::GEN_BIT_RATE,
            sigfox::GenBitRate::from_key(bit_rate).unwrap(),
        )
        .set(&sigfox::GEN_REPS, sigfox::GenReps::from_key(reps).unwrap())
        .seconds(seconds)
}

/// The public example frame (in `expected.json` as `sigfox_public_example_frame`).
#[test]
fn public_example_frame() {
    let bytes = sdrkit_stack_ism::family::from_hex("aaaaa35f01a0db334d00abad1deadc32758a").unwrap();
    let mut bits = Vec::new();
    sdrkit_util_bits::pack::unpack(&bytes, sdrkit_util_bits::BitOrder::Msb, &mut bits);
    assert_eq!(&bits[..19], &PREAMBLE);
    let (u, _) = parse_frame(&bits[19..]).expect("the example parses");
    assert_eq!(u.frame_type, 0x035F);
    assert_eq!((u.container, u.rank), (12, 1));
    assert_eq!(u.device_id, 0x004D33DB);
    assert_eq!(u.counter, 0x1A0);
    assert_eq!(u.payload, vec![0xAB, 0xAD, 0x1D, 0xEA]);
    assert_eq!(u.auth, vec![0xDC, 0x32]);
    assert_eq!(u.crc, 0x758A);
    assert!(!u.bf);
}

#[test]
fn frame_round_trip_every_size_and_rank() {
    for (pl, ob) in [
        (0usize, None),
        (0, Some(1u8)),
        (1, None),
        (2, None),
        (3, None),
        (4, None),
        (5, None),
        (8, None),
        (9, None),
        (12, None),
    ] {
        for rank in 1..=3 {
            for bf in [false, true] {
                let payload: Vec<u8> = (0..pl as u8).collect();
                let bits = build_frame(0x12345678, 0x456, &payload, ob, bf, rank, 7);
                let (u, _) = parse_frame(&bits[19..]).expect("parses");
                assert_eq!(u.payload, payload);
                assert_eq!(u.one_bit, ob);
                assert_eq!(u.bf, bf);
                assert_eq!(u.rank, rank);
                assert_eq!(u.device_id, 0x12345678);
                // One container bit flipped fails the CRC.
                let mut bad = bits.clone();
                bad[19 + 13 + 20] ^= 1;
                assert!(parse_frame(&bad[19..]).is_none());
                // A frame type with one bit wrong still parses.
                let mut ft1 = bits.clone();
                ft1[19 + 3] ^= 1;
                assert!(parse_frame(&ft1[19..]).is_some());
            }
        }
    }
}

/// A frame type with two bits wrong is more than one bit from its own entry, so it never parses as that entry.
#[test]
fn a_frame_type_with_two_bits_wrong_never_parses_as_its_own_entry() {
    assert!(match_frame_type(0).is_none());
    let mut refused = 0;
    for pl in [0usize, 1, 4, 8, 12] {
        for rank in 1..=3 {
            let payload: Vec<u8> = (0..pl as u8).collect();
            let bits = build_frame(0x12345678, 0x456, &payload, None, false, rank, 7);
            let ft = bits[19..32].iter().fold(0u16, |a, &b| (a << 1) | b as u16);
            for (i, j) in [(3usize, 7usize), (0, 12), (5, 6)] {
                let mut b = bits.clone();
                b[19 + i] ^= 1;
                b[19 + j] ^= 1;
                let wrong = b[19..32].iter().fold(0u16, |a, &x| (a << 1) | x as u16);
                assert_eq!((wrong ^ ft).count_ones(), 2);
                match match_frame_type(wrong) {
                    Some((_, _, _, exact)) => assert_ne!(exact, ft),
                    None => {
                        assert!(parse_frame(&b[19..]).is_none());
                        refused += 1;
                    }
                }
            }
        }
    }
    assert!(refused > 0);
}

/// The container and CRC bits of a 12-byte message.
fn message_bits() -> Vec<u8> {
    let frame = build_frame(
        0x00C0FFEE,
        0x123,
        &[0x1F, 0x3A, 0x00, 0x7C],
        None,
        false,
        1,
        9,
    );
    frame[19 + 13..].to_vec()
}

fn llr_of(bits: &[u8]) -> Vec<i8> {
    bits.iter()
        .map(|&b| if b == 0 { 40 } else { -40 })
        .collect()
}

/// The ranks of one message together form a rate 1/3 code. The Viterbi decoder corrects one wrong channel bit of a frame of rank
/// 2 or 3.
#[test]
fn one_channel_error_stays_local_when_the_ranks_decode_together() {
    let plain = message_bits();
    let r2 = encode_rank(&plain, 2);
    let r3 = encode_rank(&plain, 3);
    let mut bad2 = r2.clone();
    bad2[10] ^= 1;
    // Alone, the rank-2 frame is a rate-1 code: the inverse spreads the error to the end of the frame.
    let alone = decode_ranks(&[(2, &llr_of(&bad2))]).unwrap();
    let wrong = alone.iter().zip(&plain).filter(|(a, b)| a != b).count();
    assert!(wrong > 20, "{wrong} wrong bits");
    // Together with rank 3 the error is corrected.
    let both = decode_ranks(&[(2, &llr_of(&bad2)), (3, &llr_of(&r3))]).unwrap();
    assert_eq!(both, plain);
    // All three ranks, one error in each.
    let mut bad3 = r3.clone();
    bad3[40] ^= 1;
    let mut bad1 = plain.clone();
    bad1[70] ^= 1;
    let all = decode_ranks(&[
        (1, &llr_of(&bad1)),
        (2, &llr_of(&bad2)),
        (3, &llr_of(&bad3)),
    ])
    .unwrap();
    assert_eq!(all, plain);
}

#[test]
fn the_joint_decoder_returns_both_frames_of_a_repaired_message() {
    let plain = message_bits();
    let soft = |rank: u8, flip: usize, time: f64| {
        let mut c = encode_rank(&plain, rank);
        c[flip] ^= 1;
        SoftFrame {
            container: 12,
            rank,
            control: false,
            frame_type: sigfox::frame_type(12, rank, false).unwrap(),
            llr: llr_of(&c),
            bit_rate: 100,
            offset_hz: 1000.0 * rank as f64,
            level_db: -20.0,
            time,
        }
    };
    let mut j = Joint::default();
    // Alone each frame fails its CRC.
    let f2 = soft(2, 30, 1.0);
    let l: Vec<i8> = f2.llr.clone();
    assert!(decode_ranks(&[(2, &l)]).map(|d| d != plain).unwrap_or(true));
    assert!(j.push(f2).is_empty());
    let got = j.push(soft(3, 77, 2.6));
    assert_eq!(got.len(), 2);
    for u in &got {
        assert_eq!(u.device_id, 0x00C0FFEE);
        assert_eq!(u.payload, vec![0x1F, 0x3A, 0x00, 0x7C]);
        assert!(u.joint);
    }
    let mut ranks: Vec<u8> = got.iter().map(|u| u.rank).collect();
    ranks.sort_unstable();
    assert_eq!(ranks, vec![2, 3]);
    // A frame of another message size is not joined.
    let mut other = soft(1, 5, 3.0);
    other.container = 16;
    assert!(j.push(other).is_empty());
}

#[test]
fn conformance_vector() {
    let rec = sdrkit_dev_test::read_recording("ism/sigfox_rc1_400k")
        .expect("the recording ism/sigfox_rc1_400k");
    let exp = read_expected("ism").unwrap();
    let m = &exp["recordings"]["sigfox_rc1_400k"]["expect"]["message"];
    let cfg = StackConfig::new(sdrkit_core::Values::defaults(&(SIGFOX.settings)()));
    let mut r = rx(rec.rate_hz, rec.centre_hz, &cfg);
    let (d, events) = feed(&mut r, &rec.samples);
    assert_eq!(d.list.len(), 1, "{:?}", d.list);
    let g = &d.list[0];
    assert_eq!(g.device_id, m["device_id"].as_str().unwrap());
    assert_eq!(g.counter as u64, m["counter"].as_u64().unwrap());
    assert_eq!(g.payload, m["payload_hex"].as_str().unwrap());
    assert_eq!(g.payload_bytes as u64, m["payload_bytes"].as_u64().unwrap());
    assert_eq!(g.auth, m["auth_hex"].as_str().unwrap());
    assert_eq!(g.crc, m["crc_hex"].as_str().unwrap());
    assert_eq!(g.frame_type, m["frame_type_hex"].as_str().unwrap());
    assert_eq!(g.li as u64, m["li"].as_u64().unwrap());
    assert_eq!(g.container as u64, m["container_bytes"].as_u64().unwrap());
    assert_eq!(g.ranks as u64, m["ranks_heard_mask"].as_u64().unwrap());
    assert_eq!(g.bit_rate as u64, m["bit_rate"].as_u64().unwrap());
    assert_eq!(g.reps as u64, m["repetitions"].as_u64().unwrap());
    assert_eq!(g.downlink, m["downlink_request"].as_bool().unwrap());
    let want = m["offset_hz"].as_f64().unwrap();
    let tol = m["offset_tolerance_hz"].as_f64().unwrap();
    assert!((g.freqs[0] - want).abs() <= tol, "{}", g.freqs[0]);
    // The event is at the uplink's own frequency.
    assert_eq!(events.len(), 1);
    assert!((events[0].freq_hz - (rec.centre_hz + want)).abs() <= tol);
}

#[test]
fn three_repetitions_merge() {
    let (d, _) = run(&harness("100", "3", 6.5).seed(3));
    let m = d
        .list
        .iter()
        .find(|m| m.device_id == "00C0FFEE")
        .expect("message");
    assert_eq!(m.reps, 3, "{m:?}");
    assert_eq!(m.ranks, 7);
    assert_eq!(m.freqs.len(), 3);
}

#[test]
fn every_test_device_with_noise_and_drift() {
    let h = harness("100", "1", 12.0)
        .seed(4)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 15.0)
        .set(&GEN_DRIFT_HZ_S, 30.0);
    let (d, _) = run(&h);
    for id in ["00C0FFEE", "0BADF00D", "00ABCDEF", "01234567"] {
        assert!(
            d.list.iter().any(|m| m.device_id == id),
            "{id} missing: {:?}",
            d.list.iter().map(|m| &m.device_id).collect::<Vec<_>>()
        );
    }
    let get = |id: &str| d.list.iter().find(|m| m.device_id == id).unwrap();
    assert_eq!(get("0BADF00D").payload, "bit 1");
    assert_eq!(get("00ABCDEF").payload, "48454C4C4F20574F524C4421");
    assert!(get("01234567").downlink);
}

#[test]
fn sensitivity_at_12_db_in_100_hz() {
    let h = harness("100", "1", 22.0)
        .seed(7)
        .set(&sigfox::GEN_GAP, 0.2)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 12.0);
    let (d, _) = run(&h);
    let (clean, _) = run(&harness("100", "1", 22.0).seed(7).set(&sigfox::GEN_GAP, 0.2));
    // The test accepts the loss of one of the frames that the clean signal produces.
    assert!(clean.frames >= 10, "{} clean frames", clean.frames);
    assert!(
        d.frames + 1 >= clean.frames,
        "{} of {} frames",
        d.frames,
        clean.frames
    );
}

#[test]
fn rate_600() {
    let h = harness("600", "1", 4.0)
        .seed(5)
        .set(&sigfox::GEN_GAP, 0.5)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 25.0);
    let (d, _) = run(&h);
    assert!(d.list.len() >= 3, "{:?}", d.list);
    assert!(d.list.iter().all(|m| m.bit_rate == 600));
}

#[test]
fn no_false_messages_from_noise_or_a_carrier() {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
    let mut x: Vec<Complex32> = (0..400_000 * 20)
        .map(|_| rng.complex_normal(0.02))
        .collect();
    let mut nco = sdrkit_algo_dsp::nco::Nco::new(400e3, 15e3);
    let mut tone = vec![Complex32::default(); 800_000];
    nco.fill(&mut tone);
    for (s, t) in x[400_000..1_200_000].iter_mut().zip(&tone) {
        *s += t * 0.05;
    }
    let cfg = StackConfig::new(sdrkit_core::Values::defaults(&(SIGFOX.settings)()));
    let mut r = rx(400e3, DIAL, &cfg);
    let (d, _) = feed(&mut r, &x);
    assert!(d.list.is_empty());
}

#[test]
fn every_event_is_declared_and_has_its_frequency() {
    let decl = sdrkit_stack_ism::typed::shape("sigfox").results_decl();
    let (d, events) = run(&harness("100", "1", 8.0).seed(2));
    assert!(!d.list.is_empty());
    assert!(!events.is_empty());
    assert!(crate::util::declared(&decl, &events));
    for e in &events {
        assert!(e.freq_hz > 800e6, "{} at {} Hz", e.kind(), e.freq_hz);
    }
}

/// A setting the stack does not have, and a bit rate that is not one of the choices, are refused.
#[test]
fn a_bad_setting_value_is_refused() {
    let cfg = StackConfig::new(sdrkit_core::Values::defaults(&(SIGFOX.settings)()));
    let mut r = sigfox::receiver(&cfg).unwrap();
    assert!(crate::util::set(&mut r, "no_such_setting", Value::Bool(true)).is_err());
    let schema = (SIGFOX.settings)();
    let mut s = sdrkit_core::Values::defaults(&schema);
    let id = schema.id("gen_bit_rate").unwrap();
    assert!(s.set(id, &Value::Text("700".into())).is_err());
    assert!(s.set(id, &Value::Choice(2)).is_err());
}

#[test]
fn a_dbpsk_segment_with_a_broken_crc_is_kept_for_the_joint_decoder() {
    let bits = build_frame(0x00C0FFEE, 0x123, &[1, 2, 3, 4], None, false, 2, 5);
    let mut x = vec![Complex32::default(); 40_000];
    let mut b = bits.clone();
    b[19 + 13 + 30] ^= 1;
    x.extend(sigfox::modulate(&b, 100.0, 400e3));
    x.extend(vec![Complex32::default(); 40_000]);
    let seg = sigfox::Segment {
        samples: x,
        rate: 400e3,
        freq_hz: 0.0,
        start: 0,
    };
    match sigfox::demod_segment(&seg, 100) {
        Demodulated::Soft(f) => {
            assert_eq!(f.rank, 2);
            assert_eq!(f.container, 12);
        }
        other => panic!("{other:?}"),
    }
    let mut ok = bits.clone();
    ok.truncate(bits.len());
    let seg = sigfox::Segment {
        samples: [
            vec![Complex32::default(); 40_000],
            sigfox::modulate(&ok, 100.0, 400e3),
            vec![Complex32::default(); 40_000],
        ]
        .concat(),
        rate: 400e3,
        freq_hz: 0.0,
        start: 0,
    };
    assert!(matches!(
        sigfox::demod_segment(&seg, 100),
        Demodulated::Uplink(_)
    ));
}

#[test]
fn inside_ism_only_on_a_zone() {
    use sdrkit_stack_ism::ism::{self, devices::Devices, ISM};
    let mut g = sigfox::generator(
        1e6,
        sigfox::GenParams {
            bit_rate: 100,
            reps: 1,
            gap_s: 0.5,
            seed: 6,
        },
    )
    .unwrap();
    let mut y = vec![Complex32::default(); 3_000_000];
    sdrkit_core::Generator::generate(&mut g, &mut y);
    for (dial, want) in [(868.13e6, true), (433.92e6, false)] {
        let settings = sdrkit_core::Values::defaults(&(ISM.settings)());
        let mut r = ism::receiver(&StackConfig::new(settings)).unwrap();
        r.configure(&InputInfo::new(1e6, dial, dial).recording())
            .unwrap();
        let mut out = Output::default();
        for c in y.chunks(65536) {
            r.feed(c, &mut out);
        }
        r.feed(&vec![Complex32::default(); 500_000], &mut out);
        let d = r
            .graph()
            .component_as::<Devices>("devices")
            .unwrap()
            .store();
        assert_eq!(!d.sigfox.is_empty(), want, "{dial}");
    }
}

/// A change of frequency keeps the messages and their counts, and drops the waterfall of the present band.
#[test]
fn a_change_of_frequency_keeps_the_messages_and_drops_the_waterfall() {
    let h = harness("100", "1", 8.0).seed(2);
    let mut r = rx(400e3, DIAL, &StackConfig::new(h.values().unwrap()));
    feed(&mut r, &h.generate().unwrap());
    let messages = crate::util::table(r.results(), "sigfox").len();
    let frames = crate::util::number(r.results(), "frames_ok");
    assert!(messages >= 1 && frames > 0.0);
    assert!(crate::util::points(r.results(), "waterfall") > 0);
    r.forget();
    assert_eq!(crate::util::table(r.results(), "sigfox").len(), messages);
    assert_eq!(crate::util::number(r.results(), "frames_ok"), frames);
    assert_eq!(crate::util::points(r.results(), "waterfall"), 0);
    assert_eq!(r.results().status().offset_hz, None);
}
