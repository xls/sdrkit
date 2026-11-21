//! The DTMB frame decoder through the whole receiver: every profile and both carrier forms, echoes, offsets, noise thresholds,
//! gaps and erasure.

use super::super::common::{self, chain, impair, Run};
use sdrkit_core::Complex32;
use sdrkit_proto_mpegts::source::{Numbered, NumberedCheck};
use sdrkit_stack_tvterr::dtmb::gen::{DtmbGenParams, DtmbGenerator};
use sdrkit_stack_tvterr::dtmb::pn::Header;
use sdrkit_stack_tvterr::dtmb::profile::{CodeRate, Interleaver, Mapping};
use sdrkit_stack_tvterr::dtmb::{BODY, DESCRIPTOR};

/// A test signal: paths `(gain, delay in samples)` added to the direct path, then offset, clock error and C/N.
fn signal(
    p: &DtmbGenParams,
    seconds: f64,
    cfo: f64,
    ppm: f64,
    cn: Option<f64>,
    echoes: &[(f32, usize)],
) -> Vec<Complex32> {
    let clean = common::generate(DtmbGenerator::new(p.clone()).expect("a generator"), seconds);
    let mut x = clean.clone();
    for &(g, d) in echoes {
        for i in d..x.len() {
            x[i] += clean[i - d] * g;
        }
    }
    impair(&x, p.rate_hz, &chain(cfo, ppm, cn, p.symbol_rate()))
}

/// The run of the receiver over `x` and the check of its numbered packets.
fn decode(p: &DtmbGenParams, x: &[Complex32]) -> (Run, NumberedCheck) {
    let width = if p.width6 { "6" } else { "8" };
    let mut rx = common::receiver(&DESCRIPTOR, &[("width", width)]);
    let r = common::run(rx.as_mut(), p.rate_hz, x, 40_000, "fec.ts");
    let c = r.check(&Numbered::TERRESTRIAL);
    in_order(&r);
    (r, c)
}

/// The numbers of the good packets strictly increase.
fn in_order(r: &Run) {
    let pattern = Numbered::TERRESTRIAL;
    let nums: Vec<u32> = r
        .packets
        .iter()
        .filter_map(|(p, _)| pattern.number_of(p))
        .collect();
    assert!(nums.windows(2).all(|w| w[1] > w[0]), "packets out of order");
}

fn params(header: Header, mapping: Mapping, rate: CodeRate, il: Interleaver) -> DtmbGenParams {
    DtmbGenParams {
        header,
        mapping,
        code_rate: rate,
        interleaver: il,
        ..Default::default()
    }
}

/// Packets per second of a test signal.
fn packet_rate(p: &DtmbGenParams) -> f64 {
    p.profile().net_rate(p.header_sent(), p.symbol_rate()) / 1504.0
}

/// Packets lost between the first and the last good packet.
fn missing(c: &NumberedCheck) -> u64 {
    match (c.first, c.last) {
        (Some(a), Some(b)) => (b - a + 1) as u64 - c.good,
        _ => u64::MAX,
    }
}

/// The time of the last good packet.
fn last_time(r: &Run) -> f64 {
    let pattern = Numbered::TERRESTRIAL;
    r.packets
        .iter()
        .rev()
        .find(|(p, _)| pattern.number_of(p).is_some())
        .map_or(0.0, |(_, t)| *t)
}

/// No wrong, flagged or misordered packet.
fn clean_run(c: &NumberedCheck, what: &str) {
    assert_eq!((c.wrong, c.flagged), (0, 0), "{what}: {c:?}");
}

/// What the receiver reports matches the signal.
fn reports(r: &Run, p: &DtmbGenParams, what: &str) {
    let pr = p.profile();
    use sdrkit_core::Choice;
    assert_eq!(r.t("header"), p.header_sent().key(), "{what}");
    assert_eq!(r.t("mapping"), pr.mapping.key(), "{what}");
    assert_eq!(r.t("code_rate"), pr.rate.key(), "{what}");
    assert_eq!(r.t("interleaver"), pr.interleaver.key(), "{what}");
    assert_eq!(r.t("carriers"), p.carriers().key(), "{what}");
}

#[test]
fn recording_decodes_to_the_transport_stream() {
    let exp = sdrkit_dev_test::sigmf::read_expected("tvterr").expect("expected.json");
    let e = &exp["files"]["dtmb-pn420-16qam.sigmf-meta"]["expect"];
    let rec = sdrkit_dev_test::read_recording("tvterr/dtmb-pn420-16qam")
        .unwrap_or_else(|| panic!("the recording tvterr/dtmb-pn420-16qam (set SDRKIT_RECORDINGS)"));
    let mut rx = common::receiver(&DESCRIPTOR, &[("width", "8")]);
    let r = common::run(rx.as_mut(), rec.rate_hz, &rec.samples, 65536, "fec.ts");
    let c = r.check(&Numbered::TERRESTRIAL);
    in_order(&r);
    let what = format!("{c:?} {:?}", r.results.log_lines());
    let num = |k: &str| e[k].as_f64().unwrap_or(f64::NAN);
    assert_eq!(r.f("si_index"), num("si_index"), "{what}");
    assert_eq!(r.t("carriers"), e["carriers"].to_string(), "{what}");
    assert_eq!(
        r.t("mapping"),
        e["mapping"].as_str().unwrap_or("").to_lowercase(),
        "{what}"
    );
    assert_eq!(
        r.t("code_rate"),
        e["code_rate"].as_str().unwrap_or(""),
        "{what}"
    );
    assert!(
        c.wrong as f64 <= num("wrong_packets_max") && c.flagged == 0,
        "{what}"
    );
    assert!(
        r.f("ldpc_bad") <= num("ldpc_bad_max"),
        "{what}: bad {}",
        r.f("ldpc_bad")
    );
    assert!(c.good as f64 >= num("good_packets_min"), "{what}");
    assert!(
        c.first_time.unwrap_or(9.0) <= num("first_packet_within_s"),
        "{what}"
    );
    assert_eq!(c.gaps, 0, "{what}");
    assert_eq!(
        r.b("ts_lock"),
        e["ts_lock"].as_bool().unwrap_or(false),
        "{what}"
    );
    let rate = r.f("payload_bps") / 1e6;
    let want = &e["net_bitrate_mbps"];
    assert!(
        (rate - num_of(want, "value")).abs() <= num_of(want, "tolerance"),
        "{what}: {rate} Mbit/s"
    );
}

fn num_of(v: &serde_json::Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or(f64::NAN)
}

#[test]
fn single_carrier_profiles_decode() {
    // (mapping, rate, interleaver, C/N, offset, echo 6 dB down at +40 samples)
    let cases = [
        (
            Mapping::Qam4,
            CodeRate::R04,
            Interleaver::Mode1,
            12.0,
            0.0,
            false,
        ),
        (
            Mapping::Qam4,
            CodeRate::R08,
            Interleaver::Mode2,
            14.0,
            7000.0,
            false,
        ),
        (
            Mapping::Qam4Nr,
            CodeRate::R08,
            Interleaver::Mode1,
            10.0,
            0.0,
            false,
        ),
        (
            Mapping::Qam16,
            CodeRate::R06,
            Interleaver::Mode1,
            28.0,
            -2500.0,
            true,
        ),
        (
            Mapping::Qam32,
            CodeRate::R08,
            Interleaver::Mode1,
            30.0,
            0.0,
            false,
        ),
        (
            Mapping::Qam64,
            CodeRate::R04,
            Interleaver::Mode1,
            30.0,
            0.0,
            false,
        ),
    ];
    for (m, rate, il, cn, cfo, echo) in cases {
        let p = DtmbGenParams {
            single_carrier: true,
            ..params(Header::Pn595, m, rate, il)
        };
        let mode2 = il == Interleaver::Mode2;
        let dur = if mode2 { 2.0 } else { 1.5 };
        let echoes: &[(f32, usize)] = if echo { &[(0.5, 40)] } else { &[] };
        let x = signal(&p, dur, cfo, 0.0, Some(cn), echoes);
        let (r, c) = decode(&p, &x);
        let what = format!("C = 1 {m:?} {rate:?} {il:?}: {c:?} {}", r.t("summary"));
        clean_run(&c, &what);
        assert_eq!(c.gaps, missing(&c).min(c.gaps), "{what}");
        reports(&r, &p, &what);
        let per = p.profile().packets_per_frame() as u64;
        assert!(
            c.first_time.unwrap_or(9.0) < if mode2 { 0.7 } else { 0.45 },
            "{what}"
        );
        assert!(
            missing(&c) <= 3 * per + 8,
            "{what}: missing {}",
            missing(&c)
        );
        assert!(
            c.good as f64 > 0.95 * packet_rate(&p) * (dur - 0.6),
            "{what}"
        );
        assert!(
            (r.f("offset_hz") - cfo).abs() < 40.0,
            "{what}: offset {}",
            r.f("offset_hz")
        );
    }
}

#[test]
fn the_carrier_form_is_decided() {
    for (header, single) in [
        (Header::Pn595, false),
        (Header::Pn595, true),
        (Header::Pn945, false),
    ] {
        let p = DtmbGenParams {
            single_carrier: single,
            ..params(header, Mapping::Qam16, CodeRate::R06, Interleaver::Mode1)
        };
        let x = signal(&p, 1.0, 0.0, 0.0, Some(25.0), &[]);
        let (r, c) = decode(&p, &x);
        let what = format!("{header:?} C = 1 {single}: {c:?} {}", r.t("summary"));
        clean_run(&c, &what);
        assert!(c.good > 8000, "{what}");
        assert_eq!(r.t("carriers"), if single { "1" } else { "3780" }, "{what}");
    }
}

#[test]
fn every_header_and_mapping_decodes_on_air_form() {
    // Each header with a rotating phase (PN595 fixed), every mapping, 6 MHz once.
    for (header, m, rate, width6) in [
        (Header::Pn420, Mapping::Qam16, CodeRate::R08, false),
        (Header::Pn945, Mapping::Qam64, CodeRate::R08, false),
        (Header::Pn595, Mapping::Qam32, CodeRate::R08, false),
        (Header::Pn420, Mapping::Qam4Nr, CodeRate::R08, false),
        (Header::Pn945, Mapping::Qam16, CodeRate::R06, true),
    ] {
        let p = DtmbGenParams {
            width6,
            rate_hz: if width6 { 8e6 } else { 10e6 },
            ..params(header, m, rate, Interleaver::Mode1)
        };
        let x = signal(&p, 1.2, 1500.0, 10.0, Some(30.0), &[]);
        let (r, c) = decode(&p, &x);
        let what = format!("{header:?} {m:?} 6 MHz {width6}: {c:?} {}", r.t("summary"));
        clean_run(&c, &what);
        reports(&r, &p, &what);
        assert!(
            c.good as f64 > 0.95 * packet_rate(&p) * (1.2 - 0.6),
            "{what}"
        );
        assert!(
            missing(&c) <= 3 * p.profile().packets_per_frame() as u64 + 8,
            "{what}"
        );
    }
}

#[test]
fn noise_thresholds() {
    // (header, mapping, rate, interleaver, clean at, losses at)
    let cases = [
        (
            Header::Pn945,
            Mapping::Qam4,
            CodeRate::R04,
            Interleaver::Mode1,
            2.0,
            1.0,
        ),
        (
            Header::Pn420,
            Mapping::Qam4Nr,
            CodeRate::R08,
            Interleaver::Mode1,
            2.0,
            1.0,
        ),
        (
            Header::Pn420,
            Mapping::Qam16,
            CodeRate::R06,
            Interleaver::Mode2,
            10.0,
            8.5,
        ),
        (
            Header::Pn945,
            Mapping::Qam32,
            CodeRate::R08,
            Interleaver::Mode1,
            16.0,
            14.0,
        ),
        (
            Header::Pn595,
            Mapping::Qam64,
            CodeRate::R04,
            Interleaver::Mode1,
            12.0,
            10.0,
        ),
        (
            Header::Pn945,
            Mapping::Qam64,
            CodeRate::R08,
            Interleaver::Mode1,
            18.0,
            17.0,
        ),
    ];
    for (header, m, rate, il, ok_db, loss_db) in cases {
        let p = params(header, m, rate, il);
        let dur = if il == Interleaver::Mode2 { 2.0 } else { 1.5 };
        let x = signal(&p, dur, 0.0, 0.0, Some(ok_db + 1.0), &[]);
        let (r, c) = decode(&p, &x);
        let what = format!(
            "{header:?} {m:?} {rate:?} at {} dB: {c:?} {}",
            ok_db + 1.0,
            r.t("summary")
        );
        clean_run(&c, &what);
        let per = p.profile().packets_per_frame() as u64;
        assert!(c.first_time.unwrap_or(9.0) < 1.0, "{what}");
        assert!(
            missing(&c) <= 2 * per + 4,
            "{what}: missing {}",
            missing(&c)
        );
        assert!(
            c.good as f64 > 0.97 * packet_rate(&p) * (dur - 0.6),
            "{what}"
        );
        assert!(
            (r.f("cn_db") - (ok_db + 1.0)).abs() < 1.0,
            "{what}: C/N {}",
            r.f("cn_db")
        );
        let x = signal(&p, dur, 0.0, 0.0, Some(loss_db - 1.0), &[]);
        let (r, c) = decode(&p, &x);
        let what = format!(
            "{header:?} {m:?} {rate:?} at {} dB: {c:?} {}",
            loss_db - 1.0,
            r.t("summary")
        );
        clean_run(&c, &what);
        let (good, bad) = (r.f("ldpc_ok"), r.f("ldpc_bad"));
        // Losses: more bad words than a quarter of the good ones, no word at all (no alignment), or no decoding state.
        assert!(
            r.t("mapping").is_empty() || good.is_nan() || good == 0.0 || bad > good / 4.0,
            "{what}"
        );
    }
}

/// Zeroes (or removes) the stretches `gaps` (start sample, length) of `x`.
fn with_gaps(x: &[Complex32], gaps: &[(usize, usize)], remove: bool) -> Vec<Complex32> {
    let mut y = x.to_vec();
    for &(a, n) in gaps.iter().rev() {
        let b = (a + n).min(y.len());
        if remove {
            y.drain(a..b);
        } else {
            y[a..b].fill(Complex32::new(0.0, 0.0));
        }
    }
    y
}

/// The log lines of a run that contain `s`.
fn log_count(r: &Run, s: &str) -> usize {
    r.results
        .log_lines()
        .iter()
        .filter(|l| l.contains(s))
        .count()
}

#[test]
fn random_zero_filled_gaps_are_coasted() {
    // Row 21b: PN945 fixed, 4QAM 0.4, mode 2, 25 dB, -110 Hz, +8 ppm, 4 s; gaps start with probability 5 per second, 2 to 10 ms.
    let p = DtmbGenParams {
        fixed_phase: true,
        ..params(
            Header::Pn945,
            Mapping::Qam4,
            CodeRate::R04,
            Interleaver::Mode2,
        )
    };
    let x = signal(&p, 4.0, -110.0, 8.0, Some(25.0), &[]);
    let mut rng = sdrkit_algo_sim::rng::Rng::new(77);
    let mut gaps = Vec::new();
    let mut i = 0usize;
    while i < x.len() {
        if rng.uniform() < 5.0 / p.rate_hz {
            let n = 20_000 + rng.below(80_001) as usize;
            gaps.push((i, n));
            i += n;
        }
        i += 1;
    }
    let (r, c) = decode(&p, &with_gaps(&x, &gaps, false));
    let what = format!(
        "{} gaps zero-filled: {c:?} {:?}",
        gaps.len(),
        r.results.log_lines()
    );
    clean_run(&c, &what);
    assert!(last_time(&r) > 3.5, "{what}");
    assert_eq!(log_count(&r, "headers found"), 1, "{what}");
    let frac = c.good as f64 / (c.good + missing(&c)) as f64;
    eprintln!("gaps filled with zeros: good fraction {frac:.3}");
    assert!(frac >= 0.85, "{what}: fraction {frac}");
    let (_, c) = decode(&p, &with_gaps(&x, &gaps, true));
    clean_run(&c, "gaps removed");
    eprintln!(
        "gaps removed: good fraction {:.3}",
        c.good as f64 / (c.good + missing(&c)) as f64
    );
}

#[test]
fn single_gaps_and_dropouts() {
    let p = params(
        Header::Pn945,
        Mapping::Qam16,
        CodeRate::R06,
        Interleaver::Mode1,
    );
    let rate = p.rate_hz;
    let at = |s: f64| (s * rate) as usize;
    let x = signal(&p, 2.4, 0.0, 0.0, Some(30.0), &[]);
    // 4 ms of zeros at 0.8 s.
    let (r, c) = decode(&p, &with_gaps(&x, &[(at(0.8), at(0.004))], false));
    clean_run(&c, "4 ms zeros");
    assert!(
        last_time(&r) > 2.3 && missing(&c) < 1440,
        "4 ms zeros: {c:?}"
    );
    // 3.3 ms removed.
    let (r, c) = decode(&p, &with_gaps(&x, &[(at(0.8), at(0.0033))], true));
    clean_run(&c, "3.3 ms removed");
    assert!(
        last_time(&r) > 2.3,
        "3.3 ms removed: {c:?} {:?}",
        r.results.log_lines()
    );
    // 100 ms of zeros.
    let (r, c) = decode(&p, &with_gaps(&x, &[(at(0.8), at(0.1))], false));
    clean_run(&c, "100 ms zeros");
    assert!(
        last_time(&r) > 2.3 && missing(&c) < 8000,
        "100 ms zeros: {c:?}"
    );
    // 4 ms of zeros at 1.0 s in 1.8 s; 3.3 ms removed at 1.0 s in 2.2 s.
    let (r, c) = decode(
        &p,
        &with_gaps(&x[..at(1.8)], &[(at(1.0), at(0.004))], false),
    );
    clean_run(&c, "4 ms zeros at 1 s");
    assert!(last_time(&r) > 1.6, "4 ms zeros at 1 s: {c:?}");
    let (r, c) = decode(
        &p,
        &with_gaps(&x[..at(2.2)], &[(at(1.0), at(0.0033))], true),
    );
    clean_run(&c, "3.3 ms removed at 1 s");
    assert!(last_time(&r) > 1.9, "3.3 ms removed at 1 s: {c:?}");
}

#[test]
fn packet_timing_follows_the_signal() {
    for (header, m, rate, il, width6) in [
        (
            Header::Pn420,
            Mapping::Qam64,
            CodeRate::R08,
            Interleaver::Mode1,
            false,
        ),
        (
            Header::Pn595,
            Mapping::Qam4,
            CodeRate::R04,
            Interleaver::Mode1,
            false,
        ),
        (
            Header::Pn945,
            Mapping::Qam32,
            CodeRate::R08,
            Interleaver::Mode1,
            true,
        ),
    ] {
        let p = DtmbGenParams {
            width6,
            rate_hz: if width6 { 8e6 } else { 10e6 },
            ..params(header, m, rate, il)
        };
        let x = signal(&p, 1.2, 0.0, 0.0, None, &[]);
        let (r, c) = decode(&p, &x);
        let what = format!("{header:?} {m:?}: {c:?}");
        assert!(c.good > 1000, "{what}");
        let times: Vec<f64> = r.packets.iter().map(|(_, t)| *t).collect();
        let span = times[times.len() - 1] - times[0];
        let implied = (times.len() - 1) as f64 * 1504.0 / span;
        let want = p.profile().net_rate(p.header_sent(), p.symbol_rate());
        assert!(
            (implied / want - 1.0).abs() < 0.01,
            "{what}: {implied} against {want}"
        );
        let frame = (p.header_sent().len() + BODY) as f64 / p.symbol_rate();
        assert!(
            times
                .windows(2)
                .all(|w| w[1] - w[0] <= 0.010 && w[1] >= w[0] - frame),
            "{what}"
        );
    }
}

#[test]
fn on_air_single_carrier_form_rebuilt() {
    // PN595, C = 1, 4QAM 0.8 mode 2 at 10 Msps: +230 Hz, -0.4 ppm, an echo of 0.25 at 0.7 rad two symbols late, 18 dB, 8-bit
    // samples, 0.8 s.
    let p = DtmbGenParams {
        single_carrier: true,
        ..params(
            Header::Pn595,
            Mapping::Qam4,
            CodeRate::R08,
            Interleaver::Mode2,
        )
    };
    let clean = common::generate(DtmbGenerator::new(p.clone()).expect("a generator"), 0.8);
    let d = (2.0 * p.rate_hz / p.symbol_rate()).round() as usize;
    let g = Complex32::from_polar(0.25, 0.7);
    let mut x = clean.clone();
    for i in d..x.len() {
        x[i] += clean[i - d] * g;
    }
    let mut x = impair(
        &x,
        p.rate_hz,
        &chain(230.0, -0.4, Some(18.0), p.symbol_rate()),
    );
    let peak = x
        .iter()
        .map(|v| v.re.abs().max(v.im.abs()))
        .fold(0f32, f32::max);
    for v in x.iter_mut() {
        *v = Complex32::new((v.re / peak * 127.0).round(), (v.im / peak * 127.0).round()) / 127.0;
    }
    let (r, c) = decode(&p, &x);
    let what = format!("{c:?} {:?}", r.results.log_lines());
    clean_run(&c, &what);
    reports(&r, &p, &what);
    assert_eq!(r.f("si_index"), 10.0, "{what}");
    assert!(c.good >= 2000, "{what}");
}

/// A test signal whose second path has a real gain of `echo_db` dB over the first and comes `delay` samples later; the C/N
/// counts the power of both paths.
fn signal_two_paths(
    p: &DtmbGenParams,
    seconds: f64,
    cfo: f64,
    ppm: f64,
    cn: f64,
    echo: Option<(f64, usize)>,
) -> Vec<Complex32> {
    let clean = common::generate(DtmbGenerator::new(p.clone()).expect("a generator"), seconds);
    let mut x = clean.clone();
    if let Some((db, d)) = echo {
        let g = 10f64.powf(db / 20.0) as f32;
        for i in d..x.len() {
            x[i] += clean[i - d] * g;
        }
    }
    let power = x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
    let cfg = chain(cfo, ppm, Some(cn), p.symbol_rate());
    let info = sdrkit_algo_sim::chain::SignalInfo::new(p.rate_hz).with_power(power);
    let mut c = sdrkit_algo_sim::chain::Chain::new(&cfg, &info).expect("a valid chain");
    let mut y = Vec::with_capacity(x.len() + 1024);
    c.process(&x, &mut y);
    y
}

#[test]
fn rates_at_the_channel_width_decode() {
    // The lowest rate of each width, 1 dB above the clean point of the profile: 8 MHz at 8 Msps, 6 MHz at 6 Msps.
    for (width6, rate, header, m, r, cn) in [
        (
            false,
            8e6,
            Header::Pn945,
            Mapping::Qam64,
            CodeRate::R08,
            19.0,
        ),
        (
            true,
            6e6,
            Header::Pn420,
            Mapping::Qam16,
            CodeRate::R06,
            11.0,
        ),
    ] {
        let p = DtmbGenParams {
            width6,
            rate_hz: rate,
            ..params(header, m, r, Interleaver::Mode1)
        };
        let dur = 1.5;
        let x = signal(&p, dur, 0.0, 0.0, Some(cn), &[]);
        let (r, c) = decode(&p, &x);
        let what = format!(
            "{header:?} {m:?} at {} Msps: {c:?} {}",
            rate / 1e6,
            r.t("summary")
        );
        clean_run(&c, &what);
        reports(&r, &p, &what);
        let per = p.profile().packets_per_frame() as u64;
        assert!(c.first_time.unwrap_or(9.0) < 1.0, "{what}");
        assert!(
            missing(&c) <= 2 * per + 4,
            "{what}: missing {}",
            missing(&c)
        );
        assert!(
            c.good as f64 > 0.97 * packet_rate(&p) * (dur - 0.6),
            "{what}"
        );
        assert!(
            (r.f("cn_db") - cn).abs() < 1.0,
            "{what}: C/N {}",
            r.f("cn_db")
        );
    }
}

#[test]
fn a_strong_echo_does_not_hide_the_carrier_offset() {
    // A second path near the strength of the first bends the occupied band, and the band's centre reads up to 90 kHz off
    // (46 kHz for 3 dB down at 100 samples, 78 kHz for an equal path at 20 samples, 90 kHz for 0.5 dB down at 7 samples). The
    // carrier offsets reach the limit of the test signal.
    for (header, single, cfo, echo) in [
        (Header::Pn945, false, 40_000.0, (-3.0, 100)),
        (Header::Pn945, false, -36_500.0, (0.0, 20)),
        (Header::Pn420, false, 40_000.0, (-0.5, 7)),
        (Header::Pn420, false, -3_200.0, (-3.0, 100)),
        (Header::Pn595, false, -40_000.0, (0.0, 20)),
        (Header::Pn595, true, 31_000.0, (-3.0, 100)),
        (Header::Pn595, true, 310.0, (-0.5, 7)),
    ] {
        let p = DtmbGenParams {
            single_carrier: single,
            ..params(header, Mapping::Qam16, CodeRate::R06, Interleaver::Mode1)
        };
        let dur = 1.0;
        let x = signal_two_paths(&p, dur, cfo, 0.0, 28.0, Some(echo));
        let (r, c) = decode(&p, &x);
        let what = format!(
            "{header:?} C = 1 {single} at {cfo} Hz, echo {echo:?}: {c:?} {} {:?}",
            r.t("summary"),
            r.results.log_lines()
        );
        clean_run(&c, &what);
        reports(&r, &p, &what);
        assert!(
            (r.f("offset_hz") - cfo).abs() < 40.0,
            "{what}: offset {}",
            r.f("offset_hz")
        );
        assert!(c.first_time.unwrap_or(9.0) < 0.45, "{what}");
        assert!(
            c.good as f64 > 0.95 * packet_rate(&p) * (dur - 0.45),
            "{what}"
        );
    }
}

/// `x` without runs of samples: each sample starts a run with probability `per_s` per second, and a run is `min` to `max`
/// samples long.
fn with_random_drops(
    x: &[Complex32],
    rate: f64,
    per_s: f64,
    min: usize,
    max: usize,
    seed: u64,
) -> (Vec<Complex32>, usize) {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
    let mut gaps = Vec::new();
    let mut i = 0usize;
    while i < x.len() {
        if rng.uniform() < per_s / rate {
            let n = min + rng.below((max - min + 1) as u64) as usize;
            gaps.push((i, n));
            i += n;
        }
        i += 1;
    }
    (with_gaps(x, &gaps, true), gaps.len())
}

#[test]
fn random_input_drops_keep_the_lock() {
    // 28 dB, 3 s at 10 Msps, samples lost at random. "Echo" is a second path 4 dB stronger than the first, 37 samples later.
    // (header, fixed phase, 6 MHz, mapping, rate, interleaver, drops per second, shortest, longest, offset, clock, echo, the
    // least share of good packets)
    #[allow(clippy::type_complexity)]
    let cases: [(
        Header,
        bool,
        bool,
        Mapping,
        CodeRate,
        Interleaver,
        f64,
        usize,
        usize,
        f64,
        f64,
        bool,
        f64,
    ); 7] = [
        (
            Header::Pn945,
            false,
            false,
            Mapping::Qam16,
            CodeRate::R06,
            Interleaver::Mode1,
            20.0,
            100,
            3000,
            0.0,
            0.0,
            false,
            0.80,
        ),
        (
            Header::Pn945,
            false,
            false,
            Mapping::Qam16,
            CodeRate::R08,
            Interleaver::Mode2,
            20.0,
            100,
            3000,
            3200.0,
            20.0,
            true,
            0.45,
        ),
        (
            Header::Pn945,
            false,
            false,
            Mapping::Qam64,
            CodeRate::R06,
            Interleaver::Mode1,
            8.0,
            7000,
            55_000,
            -800.0,
            -5.0,
            false,
            0.55,
        ),
        (
            Header::Pn420,
            false,
            false,
            Mapping::Qam4,
            CodeRate::R04,
            Interleaver::Mode1,
            40.0,
            100,
            5000,
            1500.0,
            10.0,
            true,
            0.75,
        ),
        (
            Header::Pn595,
            false,
            false,
            Mapping::Qam32,
            CodeRate::R08,
            Interleaver::Mode1,
            20.0,
            100,
            3000,
            0.0,
            0.0,
            false,
            0.70,
        ),
        (
            Header::Pn595,
            false,
            false,
            Mapping::Qam4Nr,
            CodeRate::R08,
            Interleaver::Mode1,
            30.0,
            300,
            1200,
            -2500.0,
            0.0,
            false,
            0.70,
        ),
        (
            Header::Pn945,
            true,
            true,
            Mapping::Qam16,
            CodeRate::R06,
            Interleaver::Mode1,
            20.0,
            100,
            3000,
            900.0,
            -15.0,
            true,
            0.70,
        ),
    ];
    for (k, (header, fixed, width6, m, rate, il, per_s, min, max, cfo, ppm, echo, least)) in
        cases.into_iter().enumerate()
    {
        let p = DtmbGenParams {
            fixed_phase: fixed,
            width6,
            ..params(header, m, rate, il)
        };
        let x = signal_two_paths(&p, 3.0, cfo, ppm, 28.0, echo.then_some((4.0, 37)));
        let (y, drops) = with_random_drops(&x, p.rate_hz, per_s, min, max, 100 + k as u64);
        let (r, c) = decode(&p, &y);
        let frac = c.good as f64 / (c.good + missing(&c)).max(1) as f64;
        let what = format!(
            "{header:?} {m:?} {rate:?} {il:?} fixed {fixed} 6 MHz {width6}, {drops} drops: share {frac:.3} {c:?} {}",
            r.t("summary")
        );
        eprintln!("random drops: {what}");
        clean_run(&c, &what);
        assert_eq!(
            log_count(&r, "headers found"),
            1,
            "{what}: {:?}",
            r.results.log_lines()
        );
        assert!(last_time(&r) > 2.6, "{what}");
        assert!(frac >= least, "{what}");
    }
}

#[test]
fn a_gap_too_long_to_count_starts_the_decoder_again() {
    // PN420 rotating, 120 ms cut out (216 frames, more than a rotating header shows): the header is found again at once, its
    // place in the super-frame is not, so the lock goes and a new one decodes.
    let p = params(
        Header::Pn420,
        Mapping::Qam16,
        CodeRate::R06,
        Interleaver::Mode1,
    );
    let rate = p.rate_hz;
    let x = signal(&p, 2.0, 0.0, 0.0, Some(28.0), &[]);
    let y = with_gaps(&x, &[((0.8 * rate) as usize, (0.12 * rate) as usize)], true);
    let (r, c) = decode(&p, &y);
    let what = format!("{c:?} {:?}", r.results.log_lines());
    clean_run(&c, &what);
    assert_eq!(log_count(&r, "frame lock lost"), 1, "{what}");
    assert_eq!(log_count(&r, "headers found"), 2, "{what}");
    assert!(last_time(&r) > 1.8, "{what}");
    assert!(
        c.good as f64 > 0.95 * packet_rate(&p) * (1.88 - 0.45 - 0.25),
        "{what}"
    );
}

#[test]
fn a_frame_lost_under_a_fixed_header_moves_the_group_start() {
    // PN595 cannot show a lost frame. 32QAM and 4QAM-NR hold a whole number of code words in two frames, so a cut of one and
    // a half frames leaves the code words half a group off; the decoder finds the group start again.
    for m in [Mapping::Qam32, Mapping::Qam4Nr] {
        let p = params(Header::Pn595, m, CodeRate::R08, Interleaver::Mode1);
        let rate = p.rate_hz;
        let x = signal(&p, 1.6, 0.0, 0.0, Some(28.0), &[]);
        let frame = (595 + BODY) as f64 / p.symbol_rate();
        let y = with_gaps(
            &x,
            &[((0.6 * rate) as usize, (1.5 * frame * rate) as usize)],
            true,
        );
        let (r, c) = decode(&p, &y);
        let what = format!("{m:?}: {c:?} {:?}", r.results.log_lines());
        clean_run(&c, &what);
        assert_eq!(log_count(&r, "headers found"), 1, "{what}");
        assert!(last_time(&r) > 1.5, "{what}");
        assert!(
            c.good as f64 > 0.95 * packet_rate(&p) * (1.6 - 0.45 - 0.3),
            "{what}"
        );
    }
}

#[test]
fn a_mirrored_spectrum_is_turned_after_a_clean_header_without_system_information() {
    // I and Q in the wrong order: the headers are found and fit cleanly, the body gives no system information. After 0.4 s
    // the tracker swaps I and Q and searches again.
    let p = params(
        Header::Pn945,
        Mapping::Qam16,
        CodeRate::R06,
        Interleaver::Mode1,
    );
    let dur = 1.6;
    let x: Vec<Complex32> = signal(&p, dur, 1200.0, 0.0, Some(25.0), &[])
        .iter()
        .map(|v| Complex32::new(v.im, v.re))
        .collect();
    let (r, c) = decode(&p, &x);
    let what = format!("{c:?} {:?}", r.results.log_lines());
    clean_run(&c, &what);
    reports(&r, &p, &what);
    assert_eq!(log_count(&r, "I and Q swapped"), 1, "{what}");
    assert_eq!(log_count(&r, "headers found"), 2, "{what}");
    let first = c.first_time.unwrap_or(9.0);
    assert!((0.4..0.9).contains(&first), "{what}");
    assert!(
        c.good as f64 > 0.95 * packet_rate(&p) * (dur - 0.9),
        "{what}"
    );
}

#[test]
fn a_weak_header_without_system_information_loses_the_lock() {
    // The same wrong order at a header C/N of 3 dB: no swap; after 1.5 s without system information the lock goes and the
    // search starts again.
    let p = params(
        Header::Pn945,
        Mapping::Qam4,
        CodeRate::R04,
        Interleaver::Mode1,
    );
    let x: Vec<Complex32> = signal(&p, 2.0, 0.0, 0.0, Some(3.0), &[])
        .iter()
        .map(|v| Complex32::new(v.im, v.re))
        .collect();
    let (r, c) = decode(&p, &x);
    let what = format!("{c:?} {:?}", r.results.log_lines());
    assert_eq!(c.good + c.wrong, 0, "{what}");
    assert_eq!(log_count(&r, "I and Q swapped"), 0, "{what}");
    assert_eq!(
        log_count(&r, "frame lock lost: no system information"),
        1,
        "{what}"
    );
    assert_eq!(log_count(&r, "headers found"), 2, "{what}");
}

/// The transport packets with their times of `x` through the DTMB receiver, live or as a recording, and the checks of the
/// numbered packets.
fn packets_of(x: &[Complex32], rate: f64, live: bool) -> (Vec<(f64, Vec<u8>)>, NumberedCheck) {
    use sdrkit_core::{InputInfo, Output};
    use sdrkit_graph::{Buffer, Data, Graph, Timed};
    use sdrkit_proto_mpegts::stream::{self, TsPacket};
    let mut rx = common::receiver(&DESCRIPTOR, &[]);
    let input = InputInfo::new(rate, 539e6, 539e6);
    rx.configure(&if live { input } else { input.recording() })
        .expect("rate accepted");
    let mut tap = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("a graph")
        .tap("fec.ts", 1 << 16)
        .expect("port");
    let mut out = Output::default();
    let mut buf = Buffer::new(&stream::kind());
    let pattern = Numbered::TERRESTRIAL;
    let mut c = NumberedCheck::default();
    let mut all = Vec::new();
    for piece in x.chunks(40_000) {
        rx.feed(piece, &mut out);
        out.clear();
        while tap.pop(&mut buf) {
            if let Data::Items(Some(v)) = &buf.data {
                if let Some(items) = v.as_any().downcast_ref::<Vec<Timed<TsPacket>>>() {
                    for pk in items {
                        c.push(&pattern, pk.value.bytes(), pk.time);
                        all.push((pk.time, pk.value.bytes().to_vec()));
                    }
                }
            }
        }
    }
    (all, c)
}

#[test]
fn a_live_signal_gives_the_packets_of_a_recording() {
    // The code words decode in line on a live signal too, so live input loses no code word and gives the same packets
    // at the same times.
    let p = params(
        Header::Pn945,
        Mapping::Qam16,
        CodeRate::R06,
        Interleaver::Mode1,
    );
    let x = signal(&p, 1.0, 0.0, 0.0, Some(30.0), &[]);
    let (live, c) = packets_of(&x, p.rate_hz, true);
    let (recorded, _) = packets_of(&x, p.rate_hz, false);
    assert_eq!((c.wrong, c.flagged), (0, 0), "{c:?}");
    assert!(c.good >= 1000, "{c:?}");
    assert_eq!(live.len(), recorded.len());
    assert!(live == recorded, "the packets differ");
}

/// The crate's DTMB benchmark entries (the test signal at 10 and 8 Msps, noise alone) run and print their processor shares.
/// The target is 0.8 of one core on each in an optimised build on a machine at rest:
/// `cargo test --release ... processor_share -- --nocapture`.
#[test]
fn processor_share() {
    let entries: Vec<_> = sdrkit_stack_tvterr::benchmarks()
        .into_iter()
        .filter(|b| b.name.starts_with("tvterr.dtmb_"))
        .collect();
    assert_eq!(entries.len(), 3);
    for b in entries {
        let share = (b.run)(0.5).cpu_fraction_of_realtime;
        eprintln!("{}: processor share {share:.3} of one core", b.name);
        assert!(share > 0.0 && share.is_finite(), "{}", b.name);
    }
}
