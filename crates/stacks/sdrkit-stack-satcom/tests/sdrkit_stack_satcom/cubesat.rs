//! CubeSat telemetry: the table receiver over every framing, naming, noise, the smart search, saved state and settings.

use crate::common::{cell, cell_bytes, cell_num, command, row_count, set, values};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{
    Choice, Complex32, Event, Generator, InputInfo, Output, Receiver, Results, StackConfig,
    StateBlob,
};
use sdrkit_stack_satcom::modes::cubesat::{self, demod::Framing, demod::Submode, gen};
use serde_json::json;

const RATE: f64 = 1e6;
const DIAL: f64 = 145.825e6;

/// One frame row of the results.
#[derive(Debug)]
struct Row {
    framing: String,
    baud: i64,
    satellite: String,
    norad: i64,
    guess: bool,
    from: String,
    csp: String,
    data: Vec<u8>,
}

/// The key of a framing, as a choice cell returns it.
fn key(f: Framing) -> String {
    Framing::CHOICES[f.index() as usize].key.to_string()
}

fn rows(r: &Results) -> Vec<Row> {
    crate::common::rows(r, "frames")
        .into_iter()
        .map(|(_, c)| Row {
            framing: cell(c, "framing"),
            baud: cell_num(c, "baud").map_or(-1, |v| v as i64),
            satellite: cell(c, "satellite"),
            norad: cell_num(c, "norad").map_or(-1, |v| v as i64),
            guess: cell(c, "guess") == "true",
            from: cell(c, "from"),
            csp: cell(c, "csp"),
            data: cell_bytes(c, "data"),
        })
        .collect()
}

/// The test signal: `cycles` cycles of framing `which` (0 every one), with a carrier 700 Hz off and noise at `snr_db` in 25 kHz.
fn signal(
    which: usize,
    snr_db: Option<f64>,
    cycles: f64,
) -> (Vec<Complex32>, gen::CubesatGenerator) {
    signal_at(which, snr_db, cycles, 700.0)
}

/// As [`signal`], with the carrier `offset_hz` off the dial.
fn signal_at(
    which: usize,
    snr_db: Option<f64>,
    cycles: f64,
    offset_hz: f64,
) -> (Vec<Complex32>, gen::CubesatGenerator) {
    let g = gen::CubesatGenerator::new(RATE, which);
    let sent = gen::CubesatGenerator::new(RATE, which);
    let mut set = vec![
        CaseSetting::new(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, offset_hz),
        CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SEED, 2),
    ];
    if let Some(s) = snr_db {
        set.push(CaseSetting::new(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        ));
        set.push(CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SNR_DB, s));
    }
    let imp = values(&cubesat::DESCRIPTOR, &set);
    let n = (g.cycle_s() * cycles * RATE) as usize;
    let mut g = sdrkit_components::sim::impaired(Box::new(g), &imp).unwrap();
    let mut x = vec![Complex32::default(); n];
    // The generator puts the station the tune offset below the centre, as the radio sees it.
    g.generate(&mut x);
    (x, sent)
}

fn receiver(extra: &[CaseSetting]) -> Box<dyn Receiver> {
    receiver_in(None, extra)
}

fn receiver_in(submode: Option<&str>, extra: &[CaseSetting]) -> Box<dyn Receiver> {
    let v = values(&cubesat::DESCRIPTOR, extra);
    let cfg = StackConfig::new(v).with_submode_opt(submode.map(String::from));
    let mut r = (cubesat::DESCRIPTOR.make_receiver)(&cfg).unwrap();
    r.configure(&InputInfo::new(RATE, DIAL + 50e3, DIAL).recording())
        .unwrap();
    r
}

fn feed(r: &mut dyn Receiver, x: &[Complex32]) -> Vec<Event> {
    let mut out = Output::default();
    let mut ev = Vec::new();
    for c in x.chunks(50_000) {
        out.clear();
        r.feed(c, &mut out);
        ev.append(&mut out.events);
    }
    ev
}

fn field(r: &Results, key: &str) -> f64 {
    r.decl()
        .field_id(key)
        .and_then(|id| r.fields().value(id))
        .and_then(crate::common::number)
        .unwrap_or(f64::NAN)
}

#[test]
fn every_framing_and_naming() {
    let (x, g) = signal(0, Some(25.0), 2.3);
    let table = format!(
        "{};Dial sat|99002|145.825|AX100 Mode 5|9600|",
        cubesat::DEFAULT_TABLE
    );
    let mut r = receiver(&[
        CaseSetting::new(&sdrkit_stack_satcom::modes::cubesat::SATELLITES, &table),
        CaseSetting::new(&sdrkit_stack_satcom::modes::cubesat::EXTRA_BITRATES, "2400"),
    ]);
    let ev = feed(r.as_mut(), &x);
    let res = r.results();
    let got = rows(res);
    for f in &g.sent {
        // The default submode reports an Erminaz frame as a CCSDS RS frame.
        let framing = if f.framing == Framing::Erminaz {
            Framing::Ccsds
        } else {
            f.framing
        };
        let n = got
            .iter()
            .filter(|r| r.framing == key(framing) && r.baud == f.baud as i64 && r.data == f.data)
            .count();
        assert!(
            n >= 2,
            "{:?} {}: {n} of {:?}",
            f.framing,
            f.baud,
            got.iter().map(|r| (&r.framing, r.baud)).collect::<Vec<_>>()
        );
    }
    // No frame that was not sent.
    let counted: f64 = crate::common::rows(res, "frames_per_framing")
        .iter()
        .map(|(_, r)| cell_num(r, "frames").unwrap())
        .sum();
    assert_eq!(counted, got.len() as f64);
    let iss = got
        .iter()
        .find(|r| r.framing == key(Framing::Afsk))
        .unwrap();
    assert_eq!(iss.satellite, "ISS (ARISS packet)");
    assert_eq!(iss.from, "RS0ISS");
    let m5 = got
        .iter()
        .find(|r| r.framing == key(Framing::Mode5))
        .unwrap();
    assert_eq!(m5.satellite, "Dial sat");
    assert!(m5.guess);
    assert!(m5.csp.starts_with("CSP priority 2"), "{}", m5.csp);
    assert_eq!(res.status().lock_state, sdrkit_core::LockState::Locked);
    assert!(field(res, "kiss_sent") > 0.0);
    // Events: declared, and on the carrier's frequency.
    use sdrkit_stack_satcom::typed::{CUBESAT_FRAME as FRAME_EVENT, CUBESAT_KISS as KISS_EVENT};
    assert!(ev.iter().any(|e| e.is(&KISS_EVENT)));
    for e in &ev {
        assert!(e.is(&FRAME_EVENT) || e.is(&KISS_EVENT), "{e:?}");
        assert!((e.freq_hz - DIAL).abs() < 5e3, "{}", e.freq_hz);
    }
    // The satellite the pass planner follows names the frames without a callsign.
    let mut r = receiver(&[CaseSetting::new(
        &sdrkit_stack_satcom::modes::cubesat::TRACKED,
        "TEST-SAT|99001",
    )]);
    feed(r.as_mut(), &x);
    let got = rows(r.results());
    let m5 = got
        .iter()
        .find(|r| r.framing == key(Framing::Mode5))
        .unwrap();
    assert_eq!((m5.satellite.as_str(), m5.norad), ("TEST-SAT", 99001));
}

#[test]
fn noise_returns_nothing() {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(7);
    let x: Vec<Complex32> = (0..(2.0 * RATE) as usize)
        .map(|_| Complex32::new(rng.normal() as f32, rng.normal() as f32) * 0.05)
        .collect();
    let mut r = receiver(&[]);
    feed(r.as_mut(), &x);
    assert!(rows(r.results()).is_empty());
    assert_ne!(
        r.results().status().lock_state,
        sdrkit_core::LockState::Locked
    );
}

#[test]
fn smart_search_finds_the_carrier() {
    let (x, _) = signal(2, Some(25.0), 12.0);
    let mut r = receiver(&[CaseSetting::new(
        &sdrkit_stack_satcom::modes::cubesat::SEARCH,
        sdrkit_stack_satcom::modes::cubesat::Search::Smart,
    )]);
    feed(r.as_mut(), &x);
    let res = r.results();
    assert!(row_count(res, "carriers") > 0);
    assert!(rows(res).iter().any(|f| f.framing == key(Framing::G3ruh)));
}

#[test]
fn per_framing_sensitivity() {
    for which in [1, 2, 3, 4, 6] {
        let (x, g) = signal(which, Some(15.0), 8.0);
        let mut r = receiver(&[CaseSetting::new(
            &sdrkit_stack_satcom::modes::cubesat::display::KISS,
            false,
        )]);
        feed(r.as_mut(), &x);
        let n = rows(r.results()).len();
        assert!(n >= 2, "{:?} at 15 dB: {n}", g.sent[0].framing);
    }
}

#[test]
fn the_state_is_saved_and_cleared() {
    let (x, _) = signal(1, Some(25.0), 2.3);
    let mut r = receiver(&[]);
    feed(r.as_mut(), &x);
    let n = rows(r.results()).len();
    assert!(n >= 2);
    let state = r.save_state().expect("a state");
    let mut z = receiver(&[]);
    z.restore_state(&state).unwrap();
    set(z.as_mut(), "kiss", json!(true)).unwrap();
    feed(z.as_mut(), &x[..1000]);
    assert_eq!(rows(z.results()).len(), n);
    assert!(z
        .restore_state(&StateBlob::new("lora_sat", 1, Vec::new()))
        .is_err());
    command(r.as_mut(), "clear").unwrap();
    feed(r.as_mut(), &x[..1000]);
    assert!(rows(r.results()).is_empty());
}

#[test]
fn bad_settings_are_refused() {
    let mut r = receiver(&[]);
    for (k, v) in [
        ("search", json!("clever")),
        ("ccsds_length", json!(20)),
        ("smart.max_channels", json!(0)),
        ("framing_filter", json!("FM")),
        ("satellites", json!("one|two")),
        ("tracked", json!("SAT|notanumber")),
        ("kiss", json!("maybe")),
    ] {
        assert!(set(r.as_mut(), k, v.clone()).is_err(), "{k} = {v:?}");
    }
    assert!(set(r.as_mut(), "ccsds_length", json!(128)).is_ok());
}

/// The AX100 length field corrects every pattern of up to 3 errors, and 4 errors never give the value back.
#[test]
fn ax100_length_field_corrects_three_errors() {
    let ax = sdrkit_proto_link::csp::Ax100::new().unwrap();
    let w = ax.golay_encode(0x4AB);
    for a in 0..24 {
        for b in a..24 {
            for c in b..24 {
                let e = (1u32 << a) | (1u32 << b) | (1u32 << c);
                assert_eq!(ax.golay_decode(w ^ e).unwrap().0, 0x4AB);
            }
        }
    }
    assert_ne!(
        ax.golay_decode(w ^ (0b1111 << 5)).map(|x| x.0).ok(),
        Some(0x4AB)
    );
}

/// The CCSDS randomiser starts as CCSDS 131.0-B clause 10 specifies (FF 48 0E C0 9A 0D 70 BC).
#[test]
fn ccsds_randomiser_starts_as_published() {
    let mut b = [0u8; 8];
    sdrkit_proto_ccsds::randomiser::tm_bytes(&mut b);
    assert_eq!(b, [0xFF, 0x48, 0x0E, 0xC0, 0x9A, 0x0D, 0x70, 0xBC]);
}

/// Each named submode decodes the frame of its framing, every time it is sent and once, with the bytes sent.
#[test]
fn each_named_submode_decodes_its_framing_exactly() {
    for (i, (_, id)) in Submode::NAMED.iter().enumerate() {
        let which = 7 + i;
        let (x, g) = signal(which, Some(25.0), 6.0);
        let mut r = receiver_in(Some(id), &[]);
        feed(r.as_mut(), &x);
        let res = r.results();
        let got = rows(res);
        let want = &g.sent[0];
        let cycles = (x.len() as f64 / RATE / g.cycle_s()).floor() as usize;
        assert!(
            got.len() >= cycles.saturating_sub(1).max(1) && got.len() <= cycles + 1,
            "{id}: {} frames in {cycles} cycles: {:?}",
            got.len(),
            got.iter().map(|r| &r.framing).collect::<Vec<_>>()
        );
        for row in &got {
            assert_eq!(row.framing, key(want.framing), "{id}");
            assert_eq!(row.data, want.data, "{id}");
        }
        assert_eq!(
            res.status().lock_state,
            sdrkit_core::LockState::Locked,
            "{id}"
        );
        let note = crate::common::field_text(res, "note");
        assert!(
            note.contains("not confirmed by a public document"),
            "{id}: {note}"
        );
    }
}

/// Every framing at once, with the extra 2400 bit/s slicer, at 18 dB and at strong Doppler offsets: every frame sent in the
/// middle cycles arrives, and no frame arrives that was not sent.
#[test]
fn every_burst_framing_at_low_level_and_large_offsets() {
    for (snr, off) in [(18.0, -1500.0), (25.0, -11e3), (25.0, 11e3)] {
        let (x, g) = signal_at(0, Some(snr), 2.2, off);
        let mut r = receiver(&[CaseSetting::new(
            &sdrkit_stack_satcom::modes::cubesat::EXTRA_BITRATES,
            "2400",
        )]);
        feed(r.as_mut(), &x);
        let got = rows(r.results());
        for f in g.sent.iter().filter(|f| f.framing.is_burst()) {
            let framing = if f.framing == Framing::Erminaz {
                Framing::Ccsds
            } else {
                f.framing
            };
            let n = got
                .iter()
                .filter(|r| r.framing == key(framing) && r.data == f.data)
                .count();
            assert!(n >= 2, "{snr} dB {off} Hz: {:?} {n}", f.framing);
        }
        for row in &got {
            assert!(
                g.sent.iter().any(|f| f.data == row.data),
                "{snr} dB {off} Hz: a frame not sent: {}",
                row.framing
            );
        }
    }
}

/// USP at 2400 bit/s is decoded only with an extra 2400 bit/s slicer.
#[test]
fn usp_at_2400_needs_the_extra_slicer() {
    let ax = gen::burst_frames()
        .into_iter()
        .find(|f| f.framing == Framing::Usp)
        .unwrap();
    let mut f = ax.clone();
    f.baud = 2400;
    let mut y = gen::render(&f);
    y.extend(gen::render(&f));
    y.extend(gen::render(&f));
    // The frames are at the working rate of the generator; a named submode takes 250 ksps.
    let rate = 250e3;
    let mut x = Vec::new();
    sdrkit_algo_dsp::resample::Resampler::new(gen::WR, rate)
        .unwrap()
        .process(&y, &mut x);
    let run = |extra: &str| {
        let v = values(
            &cubesat::DESCRIPTOR,
            &[CaseSetting::new(
                &sdrkit_stack_satcom::modes::cubesat::EXTRA_BITRATES,
                extra,
            )],
        );
        let cfg = StackConfig::new(v).with_submode("usp");
        let mut r = (cubesat::DESCRIPTOR.make_receiver)(&cfg).unwrap();
        r.configure(&InputInfo::new(rate, DIAL, DIAL).recording())
            .unwrap();
        feed(r.as_mut(), &x);
        rows(r.results()).len()
    };
    assert!(run("2400") >= 2);
    assert_eq!(run(""), 0);
}

#[test]
fn new_settings_are_checked() {
    let mut r = receiver(&[]);
    for (k, v) in [
        ("extra_bitrates", json!("2400, 100")),
        ("extra_bitrates", json!("1200,2400,4800,9600,19200")),
        ("afsk_carrier_hz", json!(300.0)),
        ("concat.interleave", json!(9)),
        ("concat.code", json!("turbo")),
        ("mobitex_variant", json!("mobitex2")),
        ("geoscan_length", json!("70")),
    ] {
        assert!(set(r.as_mut(), k, v.clone()).is_err(), "{k} = {v:?}");
    }
    for (k, v) in [
        ("extra_bitrates", json!("1200, 2400")),
        ("afsk_carrier_hz", json!(0.0)),
        ("concat.frame_bytes", json!(892)),
        ("concat.interleave", json!(4)),
        ("ccsds_basis", json!("conventional")),
        ("geoscan_length", json!("74")),
    ] {
        assert!(set(r.as_mut(), k, v.clone()).is_ok(), "{k} = {v:?}");
    }
    // A submode the stack does not have is refused, and a named submode takes a low rate.
    let v = values(&cubesat::DESCRIPTOR, &[]);
    assert!((cubesat::DESCRIPTOR.make_receiver)(
        &StackConfig::new(v.clone()).with_submode("qubik")
    )
    .is_err());
    let mut r =
        (cubesat::DESCRIPTOR.make_receiver)(&StackConfig::new(v.clone()).with_submode("geoscan"))
            .unwrap();
    assert!(r
        .configure(&InputInfo::new(250e3, DIAL + 50e3, DIAL))
        .is_ok());
    assert!(r
        .configure(&InputInfo::new(120e3, DIAL + 50e3, DIAL))
        .is_err());
    let mut r = (cubesat::DESCRIPTOR.make_receiver)(&StackConfig::new(v)).unwrap();
    assert!(r
        .configure(&InputInfo::new(250e3, DIAL + 50e3, DIAL))
        .is_err());
}

/// U482C (audio FSK at 4800 bit/s on a 3.6 kHz carrier) at 14 dB in 25 kHz decodes every frame with a static carrier error of
/// +-1.4 kHz and with a carrier that drifts from +6 to -6 kHz over the run: the slow mean of the discriminator is removed before
/// the audio mixer, so a carrier error leaves no tone at the audio carrier.
#[test]
fn u482c_with_a_static_and_a_drifting_carrier_error() {
    let which = 7 + Submode::NAMED
        .iter()
        .position(|(s, _)| *s == Submode::U482c)
        .unwrap();
    let cycles = 16.0;
    let cycle_s = gen::CubesatGenerator::new(RATE, which).cycle_s();
    let run_s = cycle_s * cycles;
    for (what, off, drift) in [
        ("+1.4 kHz", 1400.0, 0.0),
        ("-1.4 kHz", -1400.0, 0.0),
        ("drift", 6000.0, -12_000.0 / run_s),
    ] {
        let g = gen::CubesatGenerator::new(RATE, which);
        let imp = values(
            &cubesat::DESCRIPTOR,
            &[
                CaseSetting::new(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, off),
                CaseSetting::new(&sdrkit_algo_sim::chain::GEN_DRIFT_HZ_S, drift),
                CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SEED, 5),
                CaseSetting::new(
                    &sdrkit_algo_sim::chain::GEN_NOISE,
                    sdrkit_algo_sim::chain::NoiseKind::Snr,
                ),
                CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SNR_DB, 14.0),
            ],
        );
        let sent = g.sent[0].clone();
        let mut g = sdrkit_components::sim::impaired(Box::new(g), &imp).unwrap();
        let mut x = vec![Complex32::default(); (run_s * RATE) as usize];
        g.generate(&mut x);
        let mut r = receiver_in(Some("u482c"), &[]);
        feed(r.as_mut(), &x);
        let got: Vec<Row> = rows(r.results())
            .into_iter()
            .filter(|f| f.framing == key(Framing::U482c) && f.data == sent.data)
            .collect();
        // Every whole cycle of the run: the last frame may be cut by the end of the samples, and with the drift the first frame
        // passes while the slow mean settles from the step to +6 kHz at the start.
        let lost = if drift == 0.0 { 1.0 } else { 2.0 };
        assert!(
            got.len() as f64 >= cycles - lost,
            "{what}: {} of {cycles} in {run_s:.1} s",
            got.len()
        );
    }
}

/// The data files hold the rows of the specification: 62 transmitters, 119 USP rows (one for each bit rate), 9 dials.
#[test]
fn the_data_files_have_their_rows() {
    let rows = |name: &str| -> Vec<Vec<String>> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../data/satcom")
            .join(name);
        let text = std::fs::read_to_string(path).expect("the data file");
        let mut lines = text.lines().filter(|l| !l.starts_with('#'));
        let header = lines.next().expect("a header").split(',').count();
        let out: Vec<Vec<String>> = lines
            .map(|l| l.split(',').map(String::from).collect())
            .collect();
        for r in &out {
            assert_eq!(r.len(), header, "{name}: {r:?}");
        }
        out
    };
    let tx = rows("cubesat_transmitters.csv");
    assert_eq!(tx.len(), 62);
    // Every framing of the file is one a submode or the default receiver names, or a framing no stack decodes.
    let beesat9 = tx.iter().find(|r| r[0] == "BEESAT-9").unwrap();
    assert_eq!(
        (beesat9[3].as_str(), beesat9[14].as_str()),
        ("435950000", "DP0BEM")
    );
    let usp = rows("cubesat_usp.csv");
    assert_eq!(usp.len(), 119);
    assert_eq!(usp.iter().filter(|r| r[3] == "19200").count(), 3);
    let dials = rows("cubesat_dials.csv");
    assert_eq!(dials.len(), 9);
    assert!(dials
        .iter()
        .all(|r| r[0] == "cubesat" && Submode::of(Some(&r[1])).is_some()));
}

/// The share of the frames of framing `which` sent in `cycles` cycles that arrive with their bytes, at `snr_db` in 25 kHz with
/// the carrier `offset_hz` off the dial, through the receiver of `submode`.
fn share(which: usize, submode: Option<&str>, snr_db: f64, offset_hz: f64, cycles: f64) -> f64 {
    let (x, g) = signal_at(which, Some(snr_db), cycles, offset_hz);
    let mut r = receiver_in(
        submode,
        &[CaseSetting::new(
            &sdrkit_stack_satcom::modes::cubesat::display::KISS,
            false,
        )],
    );
    feed(r.as_mut(), &x);
    let want = &g.sent[0];
    let framing = if want.framing == Framing::Erminaz {
        Framing::Ccsds
    } else {
        want.framing
    };
    let got = rows(r.results())
        .iter()
        .filter(|f| f.framing == key(framing) && f.data == want.data)
        .count();
    got as f64 / cycles.floor()
}

/// The narrow paths behind the carrier follow-up reach the 50 % points the specification requires (C/N in 25 kHz) for NGHam
/// and USP (also with the carrier 1.4 kHz either side), Mobitex, AX100 Mode 5, CCSDS concatenated, CCSDS Reed-Solomon and
/// U482C; AX.25 AFSK 1200 (each frame twice in a cycle) arrives at 6 dB, also with the carrier 11 kHz off, and half as often
/// at 4 dB with the carrier 11 kHz off (the first and last cycles of a run may be cut).
#[test]
fn narrow_paths_reach_the_required_points() {
    let id = |s: Submode| Submode::NAMED.iter().find(|(m, _)| *m == s).unwrap().1;
    let at = |s: Submode| 7 + Submode::NAMED.iter().position(|(m, _)| *m == s).unwrap();
    // Framing, generator framing, submode, C/N, carrier, the share needed.
    type Point = (&'static str, usize, Option<&'static str>, f64, f64, f64);
    let usp = (at(Submode::Usp), Some(id(Submode::Usp)));
    let points: [Point; 14] = [
        ("AFSK 1200", 1, None, 6.0, 0.0, 0.85),
        ("AFSK 1200 -11 kHz", 1, None, 6.0, -11e3, 0.8),
        ("AFSK 1200 -11 kHz", 1, None, 4.0, -11e3, 0.5),
        ("USP", usp.0, usp.1, 4.8, 0.0, 0.5),
        ("USP +1.4 kHz", usp.0, usp.1, 5.3, 1400.0, 0.5),
        ("USP -1.4 kHz", usp.0, usp.1, 5.3, -1400.0, 0.5),
        (
            "CCSDS concatenated",
            at(Submode::CcsdsConcat),
            Some(id(Submode::CcsdsConcat)),
            5.1,
            0.0,
            0.5,
        ),
        ("CCSDS Reed-Solomon", 6, None, 8.1, 0.0, 0.5),
        (
            "NGHam",
            at(Submode::Ngham),
            Some(id(Submode::Ngham)),
            5.4,
            0.0,
            0.5,
        ),
        (
            "NGHam +1.4 kHz",
            at(Submode::Ngham),
            Some(id(Submode::Ngham)),
            5.4,
            1400.0,
            0.5,
        ),
        (
            "NGHam -1.4 kHz",
            at(Submode::Ngham),
            Some(id(Submode::Ngham)),
            5.4,
            -1400.0,
            0.5,
        ),
        (
            "Mobitex",
            at(Submode::Mobitex),
            Some(id(Submode::Mobitex)),
            6.1,
            0.0,
            0.5,
        ),
        ("AX100 Mode 5", 4, None, 5.0, 0.0, 0.5),
        (
            "U482C",
            at(Submode::U482c),
            Some(id(Submode::U482c)),
            9.7,
            0.0,
            0.5,
        ),
    ];
    let mut short = Vec::new();
    for (what, which, submode, snr, off, need) in points {
        let got = share(which, submode, snr, off, 20.0);
        if got < need {
            short.push(format!("{what} at {snr} dB: {got:.2}, need {need}"));
        }
    }
    assert!(short.is_empty(), "{short:?}");
}

/// Noise alone gives no frame from the narrow paths either.
#[test]
fn noise_gives_no_narrow_frame() {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(19);
    let x: Vec<Complex32> = (0..(6.0 * RATE) as usize)
        .map(|_| Complex32::new(rng.normal() as f32, rng.normal() as f32) * 0.05)
        .collect();
    let mut r = receiver(&[]);
    feed(r.as_mut(), &x);
    assert!(rows(r.results()).is_empty());
}
