//! Railway CIR: the CRC, the worked frames, the KLW actions in three layouts, mutations, the word lists with both
//! polarities, random bits, audio in pieces with repeats and clock errors, IQ at two rates with five impairments, the saved
//! state, and random audio.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{InputInfo, Output, Receiver, Results, SettingSet, StackConfig, Values};
use sdrkit_dev_test::Harness;
use sdrkit_stack_cell::railway_cir::air::{encode, Found, Framer};
use sdrkit_stack_cell::lbj::message::Locomotives;
use sdrkit_stack_cell::railway_cir::frame::{action, build, parse, Payload, Protocol, ACTIONS};
use sdrkit_stack_cell::railway_cir::gen::{burst_bits, ffsk_audio, worked_frame, KLW_PAYLOAD, TDCS_PAYLOAD};
use sdrkit_stack_cell::railway_cir::modem::AudioDecoder;
use sdrkit_stack_cell::railway_cir::{RailwayCirSetting, DESCRIPTOR};
use sdrkit_stack_cell::typed::{RailwayCirFields, RailwayCirMessageRow, RAILWAY_CIR_MESSAGES};
use sdrkit_util_bits::crc::catalog::CRC16_XMODEM;

fn klw(p: &[u8]) -> sdrkit_stack_cell::railway_cir::frame::Klw {
    match parse(&build(Protocol::Klw, p), &Locomotives::parse(&crate::common::locomotives_text())) {
        Some(Payload::Klw(k)) => k,
        other => panic!("{other:?}"),
    }
}

fn tdcs(p: &[u8]) -> Option<sdrkit_stack_cell::railway_cir::frame::Tdcs> {
    match parse(&build(Protocol::Tdcs, p), &Locomotives::parse(&crate::common::locomotives_text())) {
        Some(Payload::Tdcs(t)) => Some(t),
        _ => None,
    }
}

#[test]
fn the_crc_check_value_and_the_worked_frames() {
    assert_eq!(CRC16_XMODEM.checksum(b"123456789"), 0x31C3);
    let f = worked_frame(Protocol::Tdcs);
    assert_eq!(f.len(), 45);
    assert_eq!(f[..11], [0x0C, 0x2B, 0x00, 0x3F, 0x1F, 0x00, 0x00, 0x1F, 0x8C, 0x30, 0x20]);
    let t = tdcs(&TDCS_PAYLOAD).unwrap();
    assert_eq!((t.signal_type, t.semaphore_type, t.role), (3, 3, 1));
    assert_eq!(t.train, "T112");
    assert_eq!((t.loco_code, t.loco_serial, t.locomotive.as_str()), (242, 521, "HXD3D-0521"));
    assert_eq!(t.km_post, Some(1161.011));
    assert!(!t.increasing && !t.test_frame);
    assert_eq!(t.speed_kmh, 124);
    assert_eq!((t.weight_t, t.length, t.cars), (800, 41.0, 18));
    assert_eq!((t.station, t.track, t.actual_track, t.driver), (130, 4, 5, 123456));
    let km = |b: [u8; 3]| {
        let mut p = TDCS_PAYLOAD;
        p[14..17].copy_from_slice(&b);
        tdcs(&p).unwrap()
    };
    let k = km([0x7B, 0x00, 0xC0]);
    assert_eq!(k.km_post, Some(-0.123));
    assert!(k.increasing);
    for v in [9_999_888u32, 8_888_888] {
        let b = v.to_le_bytes();
        let k = km([b[0], b[1], b[2]]);
        assert!(k.test_frame && k.km_post.is_none(), "{v}");
    }
    for v in [9_999_999u32, 0xFF_FFFF] {
        let b = v.to_le_bytes();
        let k = km([b[0], b[1], b[2]]);
        assert!(!k.test_frame && k.km_post.is_none(), "{v}");
    }
    assert_eq!(worked_frame(Protocol::Klw).len(), 24);
    let k = klw(&KLW_PAYLOAD);
    assert_eq!(k.locomotive, "HXD3C-0375");
    assert_eq!((k.reserved, k.action, k.action_name.as_str()), (0x00, 0x92, "Pressure response"));
    assert_eq!(k.tail_id.as_deref(), Some("123456"));
    assert_eq!((k.parameter.as_deref(), k.pressure_kpa), (Some("0486"), Some(486)));
}

#[test]
fn the_further_klw_payloads() {
    let k = klw(&[0x12, 0x34, 0x00, 0x24, 0xFF, 0xA5]);
    assert_eq!((k.locomotive.as_str(), k.action_name.as_str(), k.parameter), ("1234", "Vent", None));
    let k = klw(&[0x12, 0x34, 0x00, 0x92, 0x05, 0x00, 0x12, 0x23, 0x90, 0x34, 0x56, 0x00, 0x01, 0x02, 0x03]);
    assert_eq!(k.locomotive, "HXD3C-1234");
    assert_eq!((k.tail_id.as_deref(), k.pressure_kpa, k.extra.as_deref()), (Some("123456"), Some(500), Some("00010203")));
    let k = klw(&[0x23, 0x90, 0x03, 0x75, 0x00, 0xA1, 0x12, 0x34, 0x56, 0xFF, 0xFF, 0x26, 0x10, 0x20, 0x12, 0x34, 0x56]);
    assert_eq!((k.action_name.as_str(), k.clock.as_deref(), k.parameter), ("Clock calibration", Some("2026-10-20T12:34:56"), None));
    let k = klw(&[0x23, 0x90, 0x03, 0x75, 0x00, 0xA2, 0x12, 0x34, 0x56, 0xFF, 0xFF, 0x26, 0x02, 0x30, 0x25, 0x61, 0x56]);
    assert_eq!(k.clock, None);
    assert_eq!(k.action_name, "Clock calibration response");
}

#[test]
fn every_action_in_three_layouts_the_unknown_action_and_zero_pressure() {
    for &cc in &ACTIONS {
        let (name, pressure) = action(cc);
        let layouts: [Vec<u8>; 3] = [
            vec![0x12, 0x34, 0xAB, cc, 0x04, 0x86],
            vec![0x23, 0x90, 0x03, 0x75, 0xAB, cc, 0x12, 0x34, 0x56, 0x04, 0x86],
            vec![0x12, 0x34, 0xAB, cc, 0x04, 0x86, 0x12, 0x23, 0x90, 0x34, 0x56, 0x00, 0x01, 0x02, 0x03],
        ];
        for p in layouts {
            let k = klw(&p);
            assert_eq!((k.action, k.action_name.as_str(), k.reserved), (cc, name.as_str(), 0xAB), "{cc:02x}");
            assert_eq!(k.parameter.as_deref(), Some("0486"));
            assert_eq!(k.pressure_kpa, pressure.then_some(486), "{cc:02x}");
        }
    }
    let k = klw(&[0x12, 0x34, 0xAB, 0x7E, 0x04, 0x86]);
    assert_eq!((k.action_name.as_str(), k.parameter.as_deref(), k.pressure_kpa), ("Unknown 0x7e", Some("0486"), None));
    assert_eq!(klw(&[0x12, 0x34, 0x00, 0x92, 0x00, 0x00]).pressure_kpa, Some(0));
}

#[test]
fn every_changed_bit_of_the_tdcs_frame_and_a_foreign_address_fail() {
    let f = worked_frame(Protocol::Tdcs);
    for i in 0..f.len() * 8 {
        let mut g = f.clone();
        g[i / 8] ^= 0x80 >> (i % 8);
        assert!(parse(&g, &Locomotives::default()).is_none(), "bit {i}");
    }
    let mut g = f[..f.len() - 2].to_vec();
    g[2] = 0x42;
    let crc = CRC16_XMODEM.checksum(&g);
    g.extend_from_slice(&crc.to_be_bytes());
    assert!(parse(&g, &Locomotives::default()).is_none());
}

fn frames_of(bits: &[u8]) -> Vec<(Vec<u8>, u32)> {
    let mut f = Framer::default();
    bits.iter()
        .filter_map(|&b| match f.push(b) {
            Some(Found::Frame(b, n, _)) => Some((b, n)),
            _ => None,
        })
        .collect()
}

/// The bit `k` of the eighth word of a burst of `frame` (bits counted from the first sent).
fn eighth(k: usize) -> usize {
    32 + 7 * 26 + k
}

#[test]
fn words_with_up_to_two_errors_in_both_polarities() {
    let f = worked_frame(Protocol::Klw);
    for invert in [false, true] {
        for (n, errs) in [(0u32, &[][..]), (1, &[2][..]), (2, &[2, 23][..])] {
            let mut bits = encode(&f);
            for &k in errs {
                bits[eighth(k)] ^= 1;
            }
            if invert {
                bits.iter_mut().for_each(|b| *b ^= 1);
            }
            assert_eq!(frames_of(&bits), [(f.clone(), n)], "inverted {invert}, {n} errors");
        }
    }
    let t = worked_frame(Protocol::Tdcs);
    for i in 0..26 {
        let mut bits = encode(&t);
        bits[eighth(i)] ^= 1;
        assert_eq!(frames_of(&bits), [(t.clone(), 1)], "error at {i}");
        for j in i + 1..26 {
            let mut bits = encode(&t);
            bits[eighth(i)] ^= 1;
            bits[eighth(j)] ^= 1;
            assert_eq!(frames_of(&bits), [(t.clone(), 2)], "errors at {i} and {j}");
        }
    }
}

#[test]
fn random_bits_give_nothing_and_a_frame_after_them_and_after_a_bad_one_is_found() {
    let mut rng = Rng::new(5);
    let mut bits: Vec<u8> = (0..200_000).map(|_| u8::from(rng.uniform() < 0.5)).collect();
    assert!(frames_of(&bits).is_empty());
    let f = worked_frame(Protocol::Tdcs);
    bits.extend(encode(&f));
    assert_eq!(frames_of(&bits), [(f.clone(), 0)]);
    let mut bad = f.clone();
    let n = bad.len();
    bad[n - 1] ^= 0x55;
    let mut two = encode(&bad);
    two.extend(encode(&f));
    assert_eq!(frames_of(&two), [(f.clone(), 0)]);
    let raw: Vec<u8> = f.iter().flat_map(|&b| (0..8).rev().map(move |i| b >> i & 1)).collect();
    assert!(frames_of(&raw).is_empty());
}

/// The audio of one burst with a lead and a tail of silence, a DC offset of 0.8 and the bit clock off by `clock`.
fn audio(p: Protocol, invert: bool, clock: f64) -> Vec<f32> {
    let mut a = vec![0.0f32; 2400];
    a.extend(ffsk_audio(&burst_bits(&worked_frame(p), invert), 48e3, clock));
    a.extend(vec![0.0f32; 14_400]);
    a.iter_mut().for_each(|v| *v += 0.8);
    a
}

fn decode_audio(a: &[f32], piece: usize) -> Vec<Vec<u8>> {
    let mut d = AudioDecoder::default();
    let mut out = Vec::new();
    for c in a.chunks(piece) {
        d.push(c, |h| out.push(h.bytes));
    }
    out
}

#[test]
fn audio_in_pieces_repeats_and_clock_errors() {
    for p in [Protocol::Klw, Protocol::Tdcs] {
        for invert in [false, true] {
            let a = audio(p, invert, 0.0);
            for piece in [1, 37, 4096, a.len()] {
                assert_eq!(decode_audio(&a, piece), [worked_frame(p)], "{p:?} inverted {invert} piece {piece}");
            }
        }
        let mut twice = audio(p, false, 0.0);
        twice.extend(audio(p, false, 0.0));
        assert_eq!(decode_audio(&twice, 4096).len(), 2, "{p:?}");
        for clock in [-0.003, 0.003] {
            assert_eq!(decode_audio(&audio(p, false, clock), 4096), [worked_frame(p)], "{p:?} clock {clock}");
        }
    }
    let mut rng = Rng::new(9);
    let noise: Vec<f32> = (0..96_000).map(|_| rng.normal_f32()).collect();
    assert!(decode_audio(&noise, 4096).is_empty());
}

/// The receiver for `submode` with the test signal mixed 50 kHz below the centre.
fn iq_run(submode: &str, rate: f64, offset_hz: f64, invert: bool, cnr: Option<f64>, piece: usize) -> Box<dyn Receiver> {
    let mut h = Harness::new(&DESCRIPTOR).submode(submode).rate(rate).seconds(2.2).seed(3);
    if invert {
        h = h.set(&sdrkit_stack_cell::railway_cir::gen::GEN_INVERT_BITS, true);
    }
    if let Some(c) = cnr {
        h = h.set(&GEN_NOISE, NoiseKind::Snr).set(&GEN_SNR_DB, c);
    }
    let h = h.set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, offset_hz - 50e3);
    let x = h.generate().unwrap();
    let mut rx = h.receiver().unwrap();
    let centre = 866e6;
    rx.configure(&InputInfo::new(rate, centre, centre - 50e3).recording()).unwrap();
    let mut out = Output::default();
    for c in x.chunks(piece) {
        out.clear();
        rx.feed(c, &mut out);
    }
    rx
}

fn newest_raw(r: &Results) -> Option<String> {
    RAILWAY_CIR_MESSAGES.rows::<RailwayCirMessageRow>(r).next().map(|(_, row)| row.raw.to_string())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

#[test]
fn iq_at_two_rates_with_five_impairments_and_the_saved_state() {
    for rate in [240e3, 960e3] {
        for (submode, p) in [("klw", Protocol::Klw), ("tdcs", Protocol::Tdcs)] {
            for (offset, invert, cnr, piece) in [
                (0.0, false, 25.0, 4096),
                (1800.0, false, 25.0, 37),
                (-2200.0, true, 25.0, 4096),
                (8000.0, false, 15.0, 4096),
                (-8000.0, false, 15.0, 4096),
            ] {
                let rx = iq_run(submode, rate, offset, invert, Some(cnr), piece);
                let r = rx.results();
                let f = RailwayCirFields::read(r.fields()).unwrap();
                let at = format!("{rate} {submode} {offset} Hz {cnr} dB");
                assert!(f.messages >= 2, "{at}: {} messages", f.messages);
                assert_eq!(newest_raw(r), Some(hex(&worked_frame(p))), "{at}");
            }
        }
    }
    let rx = iq_run("klw", 240e3, 0.0, false, None, 4096);
    let n = RailwayCirFields::read(rx.results().fields()).unwrap().messages;
    let blob = rx.save_state().unwrap();
    let mut back = (DESCRIPTOR.make_receiver)(
        &StackConfig::new(Values::defaults(&RailwayCirSetting::schema())).with_submode("klw"),
    )
    .unwrap();
    back.configure(&InputInfo::new(240e3, 866e6, 866e6 - 50e3).recording()).unwrap();
    back.restore_state(&blob).unwrap();
    let m = RailwayCirFields::read(back.results().fields()).unwrap().messages;
    assert_eq!(m, n);
    let mut other = (DESCRIPTOR.make_receiver)(
        &StackConfig::new(Values::defaults(&RailwayCirSetting::schema())).with_submode("tdcs"),
    )
    .unwrap();
    other.configure(&InputInfo::new(240e3, 457e6, 457e6 - 50e3).recording()).unwrap();
    assert!(other.restore_state(&blob).is_err());
}

#[test]
fn the_benchmark_runs_faster_than_real_time() {
    for b in sdrkit_stack_cell::railway_cir_benchmarks() {
        let r = (b.run)(0.5);
        println!("{}: {:.2} % of one core in real time", b.name, 100.0 * r.cpu_fraction_of_realtime);
        assert!(r.cpu_fraction_of_realtime > 0.0 && r.cpu_fraction_of_realtime < 1.0, "{}", b.name);
    }
}

#[test]
fn the_data_file_has_both_dials() {
    let file = crate::common::plan_file("railway_cir.json");
    assert_eq!(file.groups.len(), 1);
    let band = &file.groups[0].bands[0];
    let dials: Vec<(&str, f64)> = band.channels.iter().map(|c| (c.name.as_str(), c.centre_hz)).collect();
    assert_eq!(dials, vec![("KLW", 866.2375e6), ("TDCS", 457.55e6)]);
    assert!(band.start_hz <= DESCRIPTOR.tuning.min_hz && DESCRIPTOR.tuning.max_hz <= band.end_hz);
}
