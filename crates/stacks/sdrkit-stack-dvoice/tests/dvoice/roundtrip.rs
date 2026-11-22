//! Generator, impairments and receiver, for each stack.
use crate::common::*;
use sdrkit_core::Complex32;

fn decode(id: &str, rate: f64, x: &[Complex32]) -> Vec<serde_json::Value> {
    let mut rx = receiver(id, &[]);
    let (r, _) = run(rx.as_mut(), rate, x, 4096);
    calls(&r)
}

fn has(
    calls: &[serde_json::Value],
    kind: &str,
    src: serde_json::Value,
    dst: serde_json::Value,
) -> bool {
    calls
        .iter()
        .any(|c| c["kind"] == kind && c["src"] == src && c["dst"] == dst)
}

#[test]
fn dmr_round_trip_with_offset_clock_error_and_noise() {
    let x = impaired("dmr", 96_000.0, 1_200.0, 40.0, 20.0);
    let c = decode("dmr", 96_000.0, &x);
    let s = show(&c);
    assert!(has(&c, "group_voice", 3101234.into(), 91.into()), "{s}");
    assert!(
        has(&c, "private_voice", 2623266.into(), 250997.into()),
        "{s}"
    );
    assert!(c.iter().any(|c| c["alias"] == "Ops Desk"), "{s}");
    assert!(
        c.iter()
            .any(|c| c["kind"] == "data" && c["text"] == "Battery low on unit 7 #1"),
        "{s}"
    );
    assert!(
        c.iter()
            .any(|c| c["encryption"]["state"] == "encrypted" && c["dst"] == 300),
        "{s}"
    );
    assert!(
        c.iter().any(|c| c["emergency"] == true && c["dst"] == 9),
        "{s}"
    );
}

#[test]
fn p25_round_trip_with_offset_clock_error_and_noise() {
    let x = impaired("p25", 250_000.0, -900.0, -30.0, 22.0);
    let c = decode("p25", 250_000.0, &x);
    let s = show(&c);
    assert!(has(&c, "voice", 1234567.into(), 1201.into()), "{s}");
    assert!(
        c.iter().any(|c| c["kind"] == "voice"
            && c["dst"] == 1300
            && c["encryption"]["state"] == "encrypted"),
        "{s}"
    );
    assert!(
        c.iter()
            .any(|c| c["kind"] == "grant" && c["details"]["channel"] == "1-0010"),
        "{s}"
    );
    assert!(
        c.iter().any(|c| c["kind"] == "data" && c["dst"] == 2002),
        "{s}"
    );
    assert!(
        c.iter()
            .any(|c| c["kind"] == "voice" && c["dst"] == 2003 && c["emergency"] == true),
        "{s}"
    );
}

#[test]
fn dpmr_round_trip_with_offset_clock_error_and_noise() {
    let x = impaired("dpmr", 96_000.0, 700.0, 30.0, 20.0);
    let c = decode("dpmr", 96_000.0, &x);
    let s = show(&c);
    assert!(has(&c, "voice", 1001.into(), 200.into()), "{s}");
    assert!(
        c.iter()
            .any(|c| c["kind"] == "status" && c["text"] == "status 17"),
        "{s}"
    );
    assert!(
        c.iter()
            .any(|c| c["kind"] == "short_message" && c["text"] == "TEST MESSAGE ON CHANNEL ONE"),
        "{s}"
    );
    assert!(
        c.iter()
            .any(|c| c["kind"] == "packet" && c["dst"] == "ALL (FFFFFF)"),
        "{s}"
    );
    assert!(
        c.iter().any(|c| c["kind"] == "voice"
            && c["emergency"] == true
            && c["access_kind"] == "colour_code"
            && c["access_code"] == 12),
        "{s}"
    );
}

#[test]
fn nxdn_round_trip_with_offset_clock_error_and_noise() {
    let x = impaired("nxdn", 96_000.0, -1_500.0, 20.0, 22.0);
    let c = decode("nxdn", 96_000.0, &x);
    let s = show(&c);
    assert!(has(&c, "voice", 1001.into(), 200.into()), "{s}");
    assert!(
        c.iter()
            .any(|c| c["kind"] == "status" && c["text"] == "status 17"),
        "{s}"
    );
    assert!(
        c.iter()
            .any(|c| c["kind"] == "short_data" && c["text"] == "TEST MESSAGE 96"),
        "{s}"
    );
    assert!(has(&c, "voice", 2001.into(), 2002.into()), "{s}");
}

#[test]
fn m17_round_trip_with_offset_clock_error_and_noise() {
    let x = impaired("m17", 500_000.0, 2_000.0, -40.0, 22.0);
    let mut rx = receiver("m17", &[]);
    let (r, _) = run(rx.as_mut(), 500_000.0, &x, 4096);
    let c = calls(&r);
    let s = show(&c);
    assert!(
        c.iter()
            .any(|c| c["src"] == "N0CALL" && c["text"] == "Test signal one, 73"),
        "{s}"
    );
    assert!(
        c.iter()
            .any(|c| c["kind"] == "packet" && c["text"] == "Hello from the test signal"),
        "{s}"
    );
    assert!(
        c.iter()
            .any(|c| c["dst"] == "ECHO" && c["details"]["position"].is_array()),
        "{s}"
    );
    assert!(
        r["detail"]["positions"]
            .as_array()
            .is_some_and(|p| p.iter().any(|p| p["station_type"] == "mobile")),
        "{}",
        r["detail"]["positions"]
    );
    assert!(
        c.iter()
            .any(|c| c["encryption"]["state"] == "encrypted" && c["media_frames"] == 0),
        "{s}"
    );
}

#[test]
fn dstar_round_trip_with_offset_clock_error_and_noise() {
    let x = impaired("dstar", 96_000.0, 600.0, 25.0, 20.0);
    let c = decode("dstar", 96_000.0, &x);
    let s = show(&c);
    assert!(
        c.iter().any(|c| c["src"] == "N0CALL"
            && c["text"] == "TEST SIGNAL ONE 73"
            && c["details"]["rpt1"] == "N0RPT  B"),
        "{s}"
    );
    assert!(
        c.iter()
            .any(|c| c["src"] == "N1TEST" && c["details"]["squelch"] == 12),
        "{s}"
    );
}

#[test]
fn ysf_round_trip_with_offset_clock_error_and_noise() {
    let x = impaired("ysf", 96_000.0, -800.0, 30.0, 20.0);
    let c = decode("ysf", 96_000.0, &x);
    let s = show(&c);
    assert!(
        c.iter().any(|c| c["src"] == "N0CALL"
            && c["text"] == "Fusion test one"
            && c["voice_frames"].as_u64().unwrap() >= 90),
        "{s}"
    );
    assert!(c.iter().any(|c| c["src"] == "N1TEST"), "{s}");
}

#[test]
fn ysf_call_signs_and_text_decode_in_every_voice_type() {
    for (voice, text) in [("vd1", true), ("vd2", true), ("voice_fr", false)] {
        let x = generate_all(generator("ysf", 96_000.0, 1, &[("gen_voice", voice)]).as_mut());
        let c = decode("ysf", 96_000.0, &x);
        let s = show(&c);
        assert!(
            c.iter().any(|c| c["src"] == "N0CALL"
                && c["dst"] == "ALL"
                && (!text || c["text"] == "Fusion test one")
                && c["voice_frames"].as_u64().unwrap() >= 90),
            "{voice}: {s}"
        );
        assert!(c.iter().any(|c| c["src"] == "N1TEST"), "{voice}: {s}");
    }
}

#[test]
fn p25p2_round_trip_with_offset_and_noise() {
    let x = impaired("p25p2", 96_000.0, 300.0, 0.0, 22.0);
    let c = decode("p25p2", 96_000.0, &x);
    let s = show(&c);
    assert!(
        c.iter().any(|c| c["slot"] == 1
            && c["src"] == 1234567
            && c["dst"] == 1201
            && c["encryption"]["state"] == "clear"
            && c["media_frames"].as_u64().unwrap() > 50),
        "{s}"
    );
    assert!(
        c.iter().any(|c| c["slot"] == 2
            && c["dst"] == 1300
            && c["encryption"]["state"] == "encrypted"
            && c["media_frames"] == 0),
        "{s}"
    );
}

#[test]
fn tetrapol_round_trip_keeps_no_identities_of_encrypted_calls() {
    let x = impaired("tetrapol", 250_000.0, 400.0, 20.0, 20.0);
    let c = decode("tetrapol", 250_000.0, &x);
    let s = show(&c);
    assert!(
        c.iter().any(|c| c["src"] == 1201
            && c["dst"] == 3001
            && c["media_frames"].as_u64().unwrap() >= 55),
        "{s}"
    );
    assert!(
        c.iter().any(|c| c["encryption"]["state"] == "encrypted"
            && c["src"].is_null()
            && c["dst"].is_null()
            && c["media_frames"] == 0),
        "{s}"
    );
}

#[test]
fn tetra_round_trip_with_offset_and_noise() {
    let x = impaired("tetra", 250_000.0, 500.0, 0.0, 22.0);
    let c = decode("tetra", 250_000.0, &x);
    let s = show(&c);
    assert!(
        c.iter().any(|c| c["src"] == 2001234
            && c["dst"] == 9001
            && c["details"]["mcc"] == 234
            && c["details"]["mnc"] == 10
            && c["media_frames"].as_u64().unwrap() > 80),
        "{s}"
    );
    assert!(
        c.iter().any(|c| c["encryption"]["state"] == "encrypted"
            && c["src"].is_null()
            && c["media_frames"] == 0),
        "{s}"
    );
}

#[test]
fn freedv_round_trip_with_noise() {
    let x = impaired("freedv", 96_000.0, 8.0, 0.0, 25.0);
    let c = decode("freedv", 96_000.0, &x);
    let s = show(&c);
    assert!(
        c.iter()
            .any(|c| c["src"] == "N0CALL" && c["voice_frames"].as_u64().unwrap() > 80),
        "{s}"
    );
}

#[test]
fn the_automatic_detector_finds_dmr_p25_and_dpmr() {
    let x = impaired("dvoice-auto", 96_000.0, 400.0, 10.0, 22.0);
    let mut rx = receiver("dvoice-auto", &[]);
    let (r, _) = run(rx.as_mut(), 96_000.0, &x, 4096);
    let c = calls(&r);
    let s = show(&c);
    assert!(has(&c, "group_voice", 3101234.into(), 91.into()), "{s}");
    assert!(has(&c, "voice", 1234567.into(), 1201.into()), "{s}");
    assert!(
        c.iter()
            .any(|c| c["kind"] == "short_message" && c["text"] == "TEST MESSAGE ON CHANNEL ONE"),
        "{s}"
    );
    let rows = table_rows(&r["tables"], "modes_seen");
    let seen: Vec<&str> = rows.iter().filter_map(|m| m["stack"].as_str()).collect();
    assert!(
        seen.contains(&"dmr") && seen.contains(&"p25") && seen.contains(&"dpmr"),
        "{seen:?}"
    );
}

#[test]
fn the_freedv_verification_case_decodes_at_its_offset() {
    let hooks = sdrkit_stack_dvoice::FREEDV
        .verify
        .expect("the FreeDV hooks");
    let o = sdrkit_core::verify::VerifyOptions {
        seconds: 3.0,
        clean: false,
        snr_db: 30.0,
    };
    let case = &(hooks.cases)(&o)[0];
    let x = impaired("freedv", 96_000.0, case.offset_hz, 0.0, 30.0);
    let c = decode("freedv", 96_000.0, &x);
    assert!(
        c.iter().any(|c| c["src"] == "N0CALL"),
        "offset {} Hz: {}",
        case.offset_hz,
        show(&c)
    );
}

#[test]
fn the_freedv_pilot_loop_pulls_in_from_8_hz_either_side() {
    for off in [-8.0, 8.0] {
        let x = impaired("freedv", 96_000.0, off, 0.0, 30.0);
        let c = decode("freedv", 96_000.0, &x);
        assert!(
            c.iter().any(|c| c["src"] == "N0CALL"),
            "offset {off} Hz: {}",
            show(&c)
        );
    }
}

/// The verdict after each quarter second of the impaired test signal.
fn verdicts(id: &str, offset: f64, snr_db: f64) -> Vec<(f64, String)> {
    let rate = 96_000.0;
    let x = impaired(id, rate, offset, 0.0, snr_db);
    let mut rx = receiver(id, &[]);
    rx.configure(&sdrkit_core::InputInfo::new(rate, 0.0, 0.0))
        .expect("configure");
    let mut out = sdrkit_core::Output::default();
    let mut v = Vec::new();
    for (i, c) in x.chunks((rate * 0.25) as usize).enumerate() {
        rx.feed(c, &mut out);
        out.clear();
        let r = view(rx.results());
        v.push((
            (i + 1) as f64 * 0.25,
            r["fields"]["verdict"].as_str().unwrap_or("").to_string(),
        ));
    }
    v
}

#[test]
fn the_dstar_verdict_stays_decoding_through_the_voice_frames() {
    let v = verdicts("dstar", 400.0, 30.0);
    let bad: Vec<_> = v
        .iter()
        .filter(|(_, k)| k == "fec" || k == "identity")
        .collect();
    assert!(bad.is_empty(), "{v:?}");
}

#[test]
fn the_p25p2_verdict_stays_decoding_through_the_voice_slots() {
    for off in [400.0, -400.0] {
        let v = verdicts("p25p2", off, 30.0);
        let bad: Vec<_> = v
            .iter()
            .filter(|(_, k)| k == "fec" || k == "identity")
            .collect();
        assert!(bad.is_empty(), "offset {off}: {v:?}");
    }
}

#[test]
fn the_dqpsk_carrier_offset_is_measured_in_hertz() {
    for off in [400.0, -400.0] {
        // The first half second holds the network and call headers, which decode and measure the carrier.
        let x = impaired("p25p2", 96_000.0, off, 0.0, 30.0);
        let mut rx = receiver("p25p2", &[]);
        rx.configure(&sdrkit_core::InputInfo::new(96_000.0, 0.0, 0.0))
            .expect("configure");
        let mut out = sdrkit_core::Output::default();
        rx.feed(&x[..48_000], &mut out);
        let r = view(rx.results());
        let shown = r["status"]["offset_hz"].as_f64().expect("an offset");
        assert!(
            (shown - off).abs() < 40.0,
            "sent {off} Hz, shown {shown} Hz"
        );
    }
}
