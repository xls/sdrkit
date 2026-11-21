//! Inmarsat-C: the recorded vector, the generator through the standard impairments, cut independence, settings and the known
//! answers of the frame layer.

use crate::common::{cell, command, field_text, number, set, values};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, Event, InputInfo, Output, Receiver, StackConfig, TableDef};
use sdrkit_dev_test::{sigmf, Harness};
use sdrkit_stack_satcom::modes::inmarsatc::{self, coding, egc, gen};
use serde_json::{json, Value as Json};

const CENTRE: f64 = 1537.15e6;
const DIAL: f64 = 1537.10e6;

fn rx_for(rate: f64, centre: f64, dial: f64, settings: &[CaseSetting]) -> Box<dyn Receiver> {
    let values = values(&inmarsatc::DESCRIPTOR, settings);
    let mut rx = (inmarsatc::DESCRIPTOR.make_receiver)(&StackConfig::new(values)).unwrap();
    rx.configure(&InputInfo::new(rate, centre, dial).recording())
        .unwrap();
    rx
}

fn feed(rx: &mut dyn Receiver, x: &[Complex32], piece: usize) -> Vec<Event> {
    let mut out = Output::default();
    let mut ev = Vec::new();
    for c in x.chunks(piece.max(1)) {
        out.clear();
        rx.feed(c, &mut out);
        ev.extend(
            out.events
                .drain(..)
                .filter(|e| e.kind().starts_with("inmarsatc.")),
        );
    }
    ev
}

fn num(rx: &dyn Receiver, key: &str) -> f64 {
    let r = rx.results();
    r.decl()
        .field_id(key)
        .and_then(|id| r.fields().value(id))
        .and_then(number)
        .unwrap_or_else(|| panic!("no number {key}"))
}

fn text(rx: &dyn Receiver, key: &str) -> String {
    field_text(rx.results(), key)
}

/// The table declaration of `table`.
fn def(table: &str) -> &'static TableDef {
    match table {
        "messages" => sdrkit_stack_satcom::typed::INMARSATC_MESSAGES.def(),
        "channels" => sdrkit_stack_satcom::typed::INMARSATC_CHANNELS.def(),
        _ => sdrkit_stack_satcom::typed::INMARSATC_PACKET_COUNTS.def(),
    }
}

/// The cells of a table as text, newest row first, in the order of the declared columns.
fn rows(rx: &dyn Receiver, table: &str) -> Vec<Vec<String>> {
    crate::common::rows(rx.results(), table)
        .into_iter()
        .map(|(_, r)| {
            def(table)
                .columns
                .iter()
                .map(|c| cell(r, c.key()))
                .collect()
        })
        .collect()
}

/// The index of a column of a declared table.
fn col(table: &str, key: &str) -> usize {
    def(table)
        .columns
        .iter()
        .position(|c| c.key() == key)
        .unwrap_or_else(|| panic!("no column {key} in {table}"))
}

/// The test signal at `rate` through the standard impairments.
fn signal(rate: f64, seconds: f64, settings: &[CaseSetting]) -> Vec<Complex32> {
    let mut h = Harness::new(&inmarsatc::DESCRIPTOR)
        .rate(rate)
        .seconds(seconds);
    h = h.with_settings(settings);
    h.generate().unwrap()
}

fn txt(s: &str) -> Json {
    json!(s)
}

#[test]
fn vector_ncs_250k() {
    let Some(rec) = sdrkit_dev_test::read_recording("satcom/inmarsatc_ncs_250k") else {
        return;
    };
    let exp = sigmf::read_expected("satcom").unwrap();
    let e = &exp["recordings"]["inmarsatc_ncs_250k"];
    for piece in [25_000usize, 4_097] {
        let mut rx = rx_for(rec.rate_hz, rec.centre_hz, DIAL, &[]);
        let ev = feed(rx.as_mut(), &rec.samples, piece);
        let r = rx.as_ref();
        let s = &e["summary"];
        assert_eq!(
            r.results().status().lock_state,
            sdrkit_core::LockState::Locked
        );
        assert_eq!(num(r, "frames_ok"), s["frames_ok"].as_f64().unwrap());
        assert_eq!(num(r, "frames_bad"), 0.0);
        assert_eq!(num(r, "packets_ok"), s["packets_ok"].as_f64().unwrap());
        assert_eq!(num(r, "packets_bad"), 0.0);
        assert_eq!(
            num(r, "message_count"),
            s["message_count"].as_f64().unwrap()
        );
        assert_eq!(num(r, "polarity_slips"), 0.0);
        assert!(
            num(r, "offset_hz").abs() < 3.0,
            "cfo {}",
            num(r, "offset_hz")
        );
        assert!((num(r, "ebn0_db") - s["ebn0_db"].as_f64().unwrap()).abs() < 1.5);
        assert!((num(r, "esn0_db") - s["esn0_db"].as_f64().unwrap()).abs() < 1.5);
        assert!(num(r, "uw_errors_avg") <= 3.0);
        let n = &e["ncs"];
        for (key, want) in [
            ("ocean_region_code", "ocean_region_code"),
            ("station_id", "station_id"),
            ("channel_type_code", "channel_type"),
            ("network_version", "network_version"),
            ("signalling_channel", "signalling_channel"),
            ("random_interval", "random_interval"),
            ("frame_number", "frame_number_last"),
        ] {
            assert_eq!(num(r, key), n[want].as_f64().unwrap(), "{key}");
        }
        for (key, want) in [
            ("region", "region"),
            ("station_name", "station_name"),
            ("channel_type", "channel_type_name"),
            ("status_text", "status_text"),
            ("services_text", "services_text"),
        ] {
            assert_eq!(text(r, key), n[want].as_str().unwrap(), "{key}");
        }
        for (key, want) in [
            ("status_byte", "status_byte"),
            ("services_mask", "services_mask"),
        ] {
            let hex = n[want].as_str().unwrap();
            assert_eq!(
                num(r, key) as i64,
                i64::from_str_radix(hex, 16).unwrap(),
                "{key}"
            );
        }
        let frame = (num(r, "frame_time") / 8.64).round() as u32;
        assert_eq!(
            egc::frame_time(frame),
            n["frame_time_last"].as_str().unwrap()
        );
        assert!(
            (num(r, "signalling_uplink_hz") - n["signalling_uplink_mhz"].as_f64().unwrap() * 1e6)
                .abs()
                < 1.0
        );
        let m = &rows(r, "messages")[0];
        let w = &e["messages"][0];
        let cell = |k: &str| m[col("messages", k)].clone();
        assert_eq!(cell("message_number"), w["message_number"].to_string());
        let code = i64::from_str_radix(w["service_code"].as_str().unwrap(), 16).unwrap();
        assert_eq!(cell("service_code"), code.to_string());
        assert_eq!(cell("service"), w["service_text"].as_str().unwrap());
        assert_eq!(cell("kind"), w["kind"].as_str().unwrap());
        assert_eq!(cell("priority"), w["priority_name"].as_str().unwrap());
        assert_eq!(cell("presentation"), w["presentation"].as_str().unwrap());
        assert_eq!(cell("packets"), w["packets"].to_string());
        assert_eq!(cell("complete") == "true", w["complete"].as_bool().unwrap());
        assert_eq!(cell("times_heard"), w["times_heard_complete"].to_string());
        assert_eq!(cell("channel"), w["channel"].to_string());
        assert_eq!(cell("area"), w["area"].as_str().unwrap());
        assert_eq!(cell("subject"), w["subject"].as_str().unwrap());
        assert_eq!(
            cell("warning_number"),
            w["warning_number"].as_str().unwrap()
        );
        assert_eq!(cell("text"), w["text"].as_str().unwrap());
        assert_eq!(
            ev.iter()
                .filter(|e| e.is(&sdrkit_stack_satcom::typed::INMARSATC_EGC))
                .count(),
            1
        );
        for e in &ev {
            assert!(e.is(&sdrkit_stack_satcom::typed::INMARSATC_EGC), "{e:?}");
            assert!((e.freq_hz - DIAL).abs() < 100.0, "{}", e.freq_hz);
        }
        assert_eq!(
            r.results()
                .status()
                .summary
                .split(", Eb/N0")
                .next()
                .unwrap(),
            "Inmarsat-C: frame 1001"
        );
    }
}

#[test]
fn round_trip_with_offsets_and_impairments() {
    let rate = 250e3;
    for (off, ppm, drift) in [
        (0.0, 0.0, 0.0),
        (15_400.0, 0.0, 0.0),
        (-19_000.0, 50.0, 0.0),
        (6_000.0, 0.0, 5.0),
    ] {
        let x = signal(
            rate,
            30.0,
            &[
                CaseSetting::new(
                    &sdrkit_algo_sim::chain::GEN_NOISE,
                    sdrkit_algo_sim::chain::NoiseKind::EbN0,
                ),
                CaseSetting::new(&sdrkit_algo_sim::chain::GEN_EBN0_DB, 9.0),
                CaseSetting::new(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, off),
                CaseSetting::new(&sdrkit_algo_sim::chain::GEN_CLOCK_PPM, ppm),
                CaseSetting::new(&sdrkit_algo_sim::chain::GEN_DRIFT_HZ_S, drift),
                CaseSetting::new(&sdrkit_algo_sim::chain::GEN_ECHO_DELAY_S, 2e-5),
                CaseSetting::new(&sdrkit_algo_sim::chain::GEN_ECHO_DB, -15.0),
                CaseSetting::new(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -14.0),
                CaseSetting::new(&sdrkit_algo_sim::chain::GEN_ADC_BITS, 8),
            ],
        );
        let mut rx = rx_for(rate, CENTRE, DIAL, &[]);
        let ev = feed(rx.as_mut(), &x, 7_777);
        let r = rx.as_ref();
        assert_eq!(
            r.results().status().lock_state,
            sdrkit_core::LockState::Locked,
            "offset {off}"
        );
        assert!(num(r, "frames_ok") >= 2.0, "offset {off}");
        // A frame cut by the clock error's slip can fail its check.
        assert!(num(r, "frames_bad") <= 1.0, "offset {off}");
        let want = off + drift * 30.0;
        assert!(
            (num(r, "offset_hz") - want).abs() < 20.0,
            "offset {off}: read {}",
            num(r, "offset_hz")
        );
        let complete = col("messages", "complete");
        assert!(rows(r, "messages").iter().any(|m| m[complete] == "true"));
        for e in &ev {
            assert!(e.is(&sdrkit_stack_satcom::typed::INMARSATC_EGC), "{e:?}");
            assert!((e.freq_hz - (DIAL + want)).abs() < 200.0, "{}", e.freq_hz);
        }
    }
}

#[test]
fn inverted_polarity_and_cut_independence() {
    let rate = 250e3;
    let x = signal(
        rate,
        26.0,
        &[
            CaseSetting::new(
                &sdrkit_stack_satcom::modes::inmarsatc::gen::GEN_INVERT,
                true,
            ),
            CaseSetting::new(
                &sdrkit_stack_satcom::modes::inmarsatc::gen::GEN_START_SYMBOLS,
                4000,
            ),
        ],
    );
    let mut a = rx_for(rate, CENTRE, DIAL, &[]);
    let mut b = rx_for(rate, CENTRE, DIAL, &[]);
    feed(a.as_mut(), &x, 1);
    feed(b.as_mut(), &x, 65_536);
    assert!(num(a.as_ref(), "frames_ok") >= 2.0);
    assert_eq!(num(a.as_ref(), "frames_ok"), num(b.as_ref(), "frames_ok"));
    let t = col("messages", "text");
    let texts = |r: &dyn Receiver| {
        rows(r, "messages")
            .iter()
            .map(|m| m[t].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(texts(a.as_ref()), texts(b.as_ref()));
}

#[test]
fn settings_filters_and_retune_keep_messages() {
    let rate = 250e3;
    let x = signal(rate, 40.0, &[]);
    let mut rx = rx_for(rate, CENTRE, DIAL, &[]);
    feed(rx.as_mut(), &x, 50_000);
    let all = rows(rx.as_ref(), "messages").len();
    assert!(all >= 2, "{all}");
    set(rx.as_mut(), "min_priority", txt("2")).unwrap();
    // A report period of samples, so the results are published again.
    feed(rx.as_mut(), &x[..62_500], 62_500);
    let shown = rows(rx.as_ref(), "messages");
    assert!(shown.len() < all);
    assert!(shown
        .iter()
        .all(|m| m[col("messages", "priority")] == "Urgency"
            || m[col("messages", "priority")] == "Distress"));
    set(rx.as_mut(), "min_priority", txt("0")).unwrap();
    rx.retuned(&InputInfo::new(rate, 1537.25e6, 1537.20e6).recording());
    feed(rx.as_mut(), &x[..62_500], 62_500);
    assert_eq!(rows(rx.as_ref(), "messages").len(), all);
    assert_eq!(num(rx.as_ref(), "frames_ok"), 0.0);
    // Clear empties the messages.
    command(rx.as_mut(), "clear").unwrap();
    feed(rx.as_mut(), &x[..62_500], 62_500);
    assert_eq!(rows(rx.as_ref(), "messages").len(), 0);
}

#[test]
fn bad_setting_values_are_refused() {
    let mut rx = rx_for(250e3, CENTRE, DIAL, &[]);
    assert!(set(rx.as_mut(), "min_priority", txt("7")).is_err());
    assert!(set(rx.as_mut(), "service_filter", txt("weather")).is_err());
    assert!(set(rx.as_mut(), "channel_search", txt("maybe")).is_err());
    assert!(set(rx.as_mut(), "search_span_hz", json!(-5.0)).is_err());
    assert!(set(rx.as_mut(), "satellite", txt("nowhere")).is_err());
}

#[test]
fn a_satellite_choice_asks_the_host_to_retune() {
    let mut rx = rx_for(250e3, CENTRE, DIAL, &[]);
    set(rx.as_mut(), "satellite", txt("i4f3")).unwrap();
    let mut out = Output::default();
    rx.feed(&vec![Complex32::default(); 62_500], &mut out);
    assert_eq!(
        out.requests,
        vec![sdrkit_core::HostRequest::retune(
            1537.70e6,
            "Inmarsat-C NCS common channel"
        )]
    );
}

#[test]
fn saved_state_restores_the_messages() {
    let rate = 250e3;
    let x = signal(rate, 30.0, &[]);
    let mut a = rx_for(rate, CENTRE, DIAL, &[]);
    feed(a.as_mut(), &x, 50_000);
    let n = rows(a.as_ref(), "messages").len();
    assert!(n >= 1);
    let state = a.save_state().unwrap();
    let mut b = rx_for(rate, CENTRE, DIAL, &[]);
    b.restore_state(&state).unwrap();
    feed(b.as_mut(), &x[..62_500], 62_500);
    assert_eq!(rows(b.as_ref(), "messages").len(), n);
}

#[test]
fn extra_channels_in_a_wide_capture() {
    let rate = 2e6;
    let x = signal(
        rate,
        45.0,
        &[
            CaseSetting::new(&sdrkit_stack_satcom::modes::inmarsatc::gen::GEN_EXTRA, 3),
            CaseSetting::new(
                &sdrkit_algo_sim::chain::GEN_NOISE,
                sdrkit_algo_sim::chain::NoiseKind::EbN0,
            ),
            CaseSetting::new(&sdrkit_algo_sim::chain::GEN_EBN0_DB, 14.0),
        ],
    );
    let mut rx = rx_for(rate, CENTRE, DIAL, &[]);
    feed(rx.as_mut(), &x, 100_000);
    let ch = rows(rx.as_ref(), "channels");
    assert_eq!(ch.len(), 4, "{ch:?}");
    let (ok, bad, ty) = (
        col("channels", "frames_ok"),
        col("channels", "frames_bad"),
        col("channels", "channel_type"),
    );
    for c in ch.iter().filter(|c| c[0] != "0") {
        assert!(c[ok].parse::<u64>().unwrap() >= 2, "{c:?}");
        assert!(c[bad].parse::<u64>().unwrap() <= 1);
        assert_eq!(c[ty], "LES TDM");
    }
    // Search off produces only the main channel.
    let mut rx = rx_for(
        rate,
        CENTRE,
        DIAL,
        &[CaseSetting::new(
            &sdrkit_stack_satcom::modes::inmarsatc::CHANNEL_SEARCH,
            false,
        )],
    );
    feed(rx.as_mut(), &x[..(20.0 * rate) as usize], 100_000);
    assert_eq!(rows(rx.as_ref(), "channels").len(), 1);
}

#[test]
fn inmarsatc_scrambler_is_the_8_stage_register() {
    // The Inmarsat-C SDM scrambler: a(n+8) = a(n) + a(n+2) + a(n+3) + a(n+4), 8 stages, so its period is 255 (a 7-stage
    // x^7 + x^5 + x^4 + x^3 register repeats within 127).
    assert_eq!(sdrkit_util_bits::seq::INMARSAT_C.period(1000), Some(255));
    // Known answer: the group mask from the register started at 0x80 (the first 32 groups).
    let g = coding::scramble_groups();
    let first: String = g[..32].iter().map(|b| char::from(b'0' + b)).collect();
    assert_eq!(first, "00000001000111000100101110000001");
}

#[test]
fn ita2_text_uses_the_itu_figures_case() {
    // ITU-R M.625 Table 1: in figures case V is "=" (US teleprinters print ";").
    assert_eq!(egc::ita2(&egc::ita2_encode("A=B")), "A=B");
    assert_eq!(egc::ita2(&[0x1B, 30]), "=");
}

#[test]
fn every_test_signal_event_is_declared() {
    let rate = 250e3;
    let x = signal(rate, 40.0, &[]);
    let mut rx = rx_for(rate, CENTRE, DIAL, &[]);
    let ev = feed(rx.as_mut(), &x, 50_000);
    assert!(!ev.is_empty());
    for e in &ev {
        assert!(e.is(&sdrkit_stack_satcom::typed::INMARSATC_EGC), "{e:?}");
        assert!(e.freq_hz > 1.5e9, "{}", e.freq_hz);
    }

    let _ = gen::GenParams::new(rate);
}
