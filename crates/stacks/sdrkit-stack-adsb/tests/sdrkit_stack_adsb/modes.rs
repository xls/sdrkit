//! The other stacks (UAT, Mode A/C, 1030 MHz interrogations), the descriptors, and the feeds out and in over loopback.

use super::common::*;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_algo_sim::offset::FrequencyOffset;
use sdrkit_core::{Complex32, GenConfig, Generator, InputInfo, Output, Receiver, StackConfig};
use sdrkit_stack_adsb::uat::generator::{UatGenerator, UatSchedule, EXAMPLE_PAYLOAD};
use serde_json::{json, Value as J};
use std::io::{Read, Write};

fn cfg(desc: &sdrkit_core::StackDescriptor, extra: &[(&str, J)]) -> StackConfig {
    config(desc, extra)
}

#[test]
fn descriptors_are_complete() {
    let d = sdrkit_stack_adsb::descriptors();
    let ids: Vec<&str> = d.iter().map(|d| d.id).collect();
    assert_eq!(ids, ["adsb", "uat", "modeac", "uplink1030"]);
    for m in d {
        assert!(m
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
        let s = (m.settings)();
        assert!(!s.is_empty());
        let mut rx = (m.make_receiver)(&cfg(m, &[])).unwrap();
        assert!(
            rx.configure(&InputInfo::new(
                m.tuning.min_rate_hz * 0.7,
                m.tuning.default_hz,
                m.tuning.default_hz
            ))
            .is_err(),
            "{}",
            m.id
        );
        assert!(
            rx.configure(&InputInfo::new(
                m.tuning.rate_hz,
                m.tuning.default_hz,
                m.tuning.default_hz
            ))
            .is_ok(),
            "{}",
            m.id
        );
        let mut g = (m.make_generator.unwrap())(&GenConfig::new(m.tuning.rate_hz, cfg(m, &[]).settings).with_seed(1))
        .unwrap();
        let mut x = vec![Complex32::new(0.0, 0.0); 10000];
        assert_eq!(g.generate(&mut x), 10000);
    }
    assert!(!sdrkit_stack_adsb::benchmarks().is_empty());
}

#[test]
fn uat_round_trip_with_impairments() {
    for rate in [2.083334e6, 2e6, 1.9e6, 4e6, 6e6] {
        let mut g = UatGenerator::new(
            rate,
            UatSchedule::Airspace {
                aircraft: 6,
                uplink: true,
            },
            3,
        );
        g.keep_log = true;
        let mut x = vec![Complex32::new(0.0, 0.0); (rate * 3.2) as usize];
        g.generate(&mut x);
        uat_noise(&mut x, rate, 25.0, 3);
        FrequencyOffset::new(rate, 3000.0, 0.0)
            .unwrap()
            .process(&mut x);
        let mut rx = sdrkit_stack_adsb::uat::receiver(&cfg(&sdrkit_stack_adsb::UAT, &[])).unwrap();
        at_station(&mut rx, 47.45, -122.31);
        let out = feed_all(&mut rx, rate, &x, 65536);
        let d = &view(rx.results());
        let sent = g.log.iter().filter(|l| l.0 < 3.1).count() as u64;
        eprintln!("{rate}: {} frames of {sent}", d["frames"]);
        assert!(d["frames"].as_u64().unwrap() + 1 >= sent, "{rate}");
        assert_eq!(d["aircraft"].as_array().unwrap().len(), 6, "{rate}");
        assert!(d["aircraft"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["lat"].is_number() && a["callsign"].is_string()));
        let names: Vec<&str> = d["fisb"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap())
            .collect();
        for n in [
            "METAR",
            "NEXRAD regional",
            "AIRMET (TWGO)",
            "G-AIRMET (TWGO)",
        ] {
            assert!(names.contains(&n), "{rate}: {names:?}");
        }
        assert!(d["fisb"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["text"].as_str().unwrap().starts_with("METAR KSEA")));
        assert!(d["tisb_recent"].as_u64().unwrap() > 0);
        assert_eq!(d["state"], 2);
        assert!(!out.events.is_empty());
    }
}

#[test]
fn uat_example_frame_and_cuts() {
    let rate = 2e6;
    let mut g = UatGenerator::new(rate, UatSchedule::Example { bursts: 4 }, 1);
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * 7.0) as usize];
    g.generate(&mut x);
    uat_noise(&mut x, rate, 35.0, 1);
    let run = |block: usize| {
        let mut rx = sdrkit_stack_adsb::uat::receiver(&cfg(&sdrkit_stack_adsb::UAT, &[])).unwrap();
        feed_all(&mut rx, rate, &x, block);
        view(rx.results())
    };
    let a = run(65536);
    assert_eq!(a["frames"], 4);
    let ac = &a["aircraft"][0];
    assert_eq!(ac["icao"], "7E6987");
    assert!((ac["lat"].as_f64().unwrap() + 31.75252676010132).abs() < 1e-9);
    for b in [1usize, 7, 4096] {
        assert_eq!(run(b)["frames"], a["frames"], "{b}");
    }
    let _ = EXAMPLE_PAYLOAD;
    // Noise alone produces no output; empty input does not crash.
    let mut n = vec![Complex32::new(0.0, 0.0); 4_000_000];
    Awgn::with_power(2.0 * 0.02 * 0.02, 4)
        .unwrap()
        .process(&mut n);
    let mut rx = sdrkit_stack_adsb::uat::receiver(&cfg(&sdrkit_stack_adsb::UAT, &[])).unwrap();
    feed_all(&mut rx, rate, &n, 65536);
    assert_eq!(view(rx.results())["frames"], 0);
    let mut out = Output::default();
    rx.feed(&[], &mut out);
}

#[test]
fn mode_ac_round_trip() {
    use sdrkit_stack_adsb::modeac::{mode_c_code, ModeAcGenerator};
    for rate in [2e6, 4e6, 10e6] {
        let mut g = ModeAcGenerator::new(rate, 6, 2);
        g.keep_log = true;
        let mut x = vec![Complex32::new(0.0, 0.0); (rate * 1.0) as usize];
        g.generate(&mut x);
        add_noise(&mut x, rate, 30.0, 2);
        let mut rx =
            (sdrkit_stack_adsb::MODEAC.make_receiver)(&cfg(&sdrkit_stack_adsb::MODEAC, &[]))
                .unwrap();
        let out = feed_all(rx.as_mut(), rate, &x, 65536);
        let replies = out
            .events
            .iter()
            .filter(|e| e.is(&sdrkit_stack_adsb::typed::MODEAC_REPLY))
            .count();
        let sent = g.log.iter().filter(|l| l.0 < 0.999).count();
        eprintln!("{rate}: {replies} of {sent}");
        assert!(
            replies as f64 >= 0.95 * sent as f64 && replies <= sent,
            "{rate}: {replies} of {sent}"
        );
        let codes = view(rx.results())["codes"].as_array().unwrap().clone();
        for (sq, alt, _, _) in &g.targets {
            let s = sdrkit_stack_adsb::codes::squawk_text(*sq);
            assert!(
                codes.iter().any(|c| c["squawk"] == s.as_str()),
                "{rate}: {s}"
            );
            let c = mode_c_code(*alt).unwrap();
            assert!(
                codes
                    .iter()
                    .any(|x| x["code"] == format!("{c:04X}") && x["alt_ft"] == *alt),
                "{rate}: {alt}"
            );
        }
    }
}

#[test]
fn interrogations_round_trip() {
    use sdrkit_stack_adsb::typed::InterrogationEvent;
    use sdrkit_stack_adsb::uplink::{InterrogationKind, UplinkGenerator};
    for rate in [8e6, 10e6, 20e6] {
        let mut g = UplinkGenerator::new(rate, 5);
        g.keep_log = true;
        let mut x = vec![Complex32::new(0.0, 0.0); (rate * 0.5) as usize];
        g.generate(&mut x);
        add_noise(&mut x, rate, 30.0, 5);
        let mut rx = (sdrkit_stack_adsb::UPLINK1030.make_receiver)(&cfg(
            &sdrkit_stack_adsb::UPLINK1030,
            &[],
        ))
        .unwrap();
        let out = feed_all(rx.as_mut(), rate, &x, 65536);
        let ms: Vec<_> = out
            .events
            .iter()
            .filter(|e| e.at(&InterrogationEvent::KIND) == Some(InterrogationKind::ModeS))
            .collect();
        let sent = g.log.iter().filter(|l| l.0 < 0.499).count();
        let good = ms
            .iter()
            .filter(|e| {
                g.log.iter().any(|(_, m)| {
                    e.at(&InterrogationEvent::HEX)
                        == Some(
                            m.iter()
                                .map(|b| format!("{b:02X}"))
                                .collect::<String>()
                                .as_str(),
                        )
                })
            })
            .count();
        eprintln!(
            "{rate}: {good} of {sent} Mode S, {} events",
            out.events.len()
        );
        if std::env::var("SHOW_MISSED").is_ok() {
            for (t, m) in g.log.iter().take(30) {
                let h: String = m.iter().map(|b| format!("{b:02X}")).collect();
                let got = ms.iter().find(|e| (e.time - t).abs() < 2e-6);
                eprintln!(
                    "  {t:.6} {h} -> {:?}",
                    got.map(|e| e.at(&InterrogationEvent::HEX))
                );
            }
        }
        assert!(
            good as f64 >= 0.95 * sent as f64,
            "{rate}: {good} of {sent}"
        );
        let d = &view(rx.results());
        for t in g.targets() {
            assert!(
                d["addressed"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|a| a["icao"] == format!("{t:06X}")),
                "{rate}: {t:06X}"
            );
        }
        for k in ["mode_a", "mode_c", "all_call_a", "ac_only_all_call"] {
            assert!(d["by_kind"][k].as_u64().unwrap_or(0) > 0, "{rate}: {k}");
        }
    }
}

#[test]
fn beast_and_sbs_servers_and_a_feed_in() {
    use sdrkit_stack_adsb::generator::*;
    // A receiver with its servers on free ports.
    let mut rx = receiver(&[
        ("beast_server", json!(true)),
        ("beast_port", json!(0)),
        ("sbs_server", json!(true)),
        ("sbs_port", json!(0)),
        ("asterix_server", json!(true)),
        ("asterix_port", json!(0)),
    ]);
    rx.configure(&InputInfo::new(4e6, 1090e6, 1090e6).recording())
        .unwrap();
    let mut out = Output::default();
    rx.feed(&vec![Complex32::new(0.0, 0.0); 1_000_000], &mut out);
    let feeds = view(rx.results())["feeds"].clone();
    let port = |k: &str| {
        feeds
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == k)
            .and_then(|f| f["port"].as_u64())
            .unwrap() as u16
    };
    let mut beast = std::net::TcpStream::connect(("127.0.0.1", port("beast"))).unwrap();
    let mut sbs = std::net::TcpStream::connect(("127.0.0.1", port("sbs"))).unwrap();
    let mut ast = std::net::TcpStream::connect(("127.0.0.1", port("asterix"))).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let a = 0x4840D6;
    let frames = [
        es_frame(17, 5, a, me_ident('A', 3, "KLM1023")),
        es_frame(
            17,
            5,
            a,
            me_airborne(
                11,
                0,
                sdrkit_stack_adsb::codes::encode_ac12_25ft(38000).unwrap(),
                false,
                52.2572,
                3.9194,
            ),
        ),
        es_frame(
            17,
            5,
            a,
            me_airborne(
                11,
                0,
                sdrkit_stack_adsb::codes::encode_ac12_25ft(38000).unwrap(),
                true,
                52.2572,
                3.9194,
            ),
        ),
    ];
    let bursts = frames
        .iter()
        .enumerate()
        .map(|(i, f)| {
            (
                Burst::mode_s(0.002 + i as f64 * 0.001, f, 0.1, 0.0, 0.0),
                Some(*f),
            )
        })
        .collect();
    let mut g = AdsbGenerator::script(4e6, 3.5e6, bursts, 1);
    let mut x = vec![Complex32::new(0.0, 0.0); 40000];
    g.generate(&mut x);
    add_noise(&mut x, 4e6, 30.0, 1);
    rx.feed(&x, &mut out);
    rx.feed(&vec![Complex32::new(0.0, 0.0); 1_000_000], &mut out);
    let read = |s: &mut std::net::TcpStream| {
        s.set_read_timeout(Some(std::time::Duration::from_millis(500)))
            .unwrap();
        let mut v = Vec::new();
        let mut b = [0u8; 4096];
        while let Ok(n) = s.read(&mut b) {
            if n == 0 {
                break;
            }
            v.extend_from_slice(&b[..n]);
        }
        v
    };
    let bb = read(&mut beast);
    let mut parsed = Vec::new();
    sdrkit_stack_adsb::feeds::BeastParser::default().feed(&bb, &mut parsed);
    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed[0].frame, frames[0]);
    let text = String::from_utf8(read(&mut sbs)).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    assert!(lines[0].starts_with("MSG,1,1,1,4840D6") && lines[0].contains("KLM1023"));
    let pos = sdrkit_stack_adsb::feeds::sbs_parse(lines[2]).unwrap();
    assert!((pos.pos.unwrap().0 - 52.2572).abs() < 1e-3);
    let a_bytes = read(&mut ast);
    let mut states = Vec::new();
    sdrkit_stack_adsb::asterix::decode_stream(&a_bytes, &mut states);
    assert!(states
        .iter()
        .any(|s| s.key == 0x4840D6 && s.callsign.as_deref() == Some("KLM1023")));

    // A second receiver fed by the first's Beast output, through a local server of our own replaying those bytes.
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let p = l.local_addr().unwrap().port();
    let srv = std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        s.write_all(&bb).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1500));
    });
    let mut rx2 = receiver(&[("feeds_in", json!(format!("127.0.0.1:{p}")))]);
    rx2.configure(&InputInfo::new(4e6, 1090e6, 1090e6).recording())
        .unwrap();
    for _ in 0..20 {
        rx2.feed(&vec![Complex32::new(0.0, 0.0); 200_000], &mut out);
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let d = &view(rx2.results());
    let ac = d["aircraft"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["icao"] == "4840D6")
        .expect("remote aircraft")
        .clone();
    assert_eq!(ac["callsign"], "KLM1023");
    assert_eq!(ac["source"], "1");
    assert_eq!(ac["position_kind"], "global");
    assert_eq!(d["good"], 0);
    assert_eq!(d["sources"][1]["format"], "beast");
    srv.join().unwrap();
    // A host outside the local network is refused.
    let mut rx3 = receiver(&[("feeds_in", json!("8.8.8.8:30005"))]);
    rx3.configure(&InputInfo::new(4e6, 1090e6, 1090e6).recording())
        .unwrap();
    rx3.feed(&vec![Complex32::new(0.0, 0.0); 1_100_000], &mut out);
    assert!(view(rx3.results())["sources"][1]["error"]
        .as_str()
        .unwrap()
        .contains("local network"));
}

/// Adds white noise so that a UAT burst of amplitude 0.08 (the generator's `signal_info`) is `snr_db` above the noise in the
/// demodulator's 2.083 MHz.
fn uat_noise(x: &mut [Complex32], rate: f64, snr_db: f64, seed: u64) {
    let p = 0.08f64.powi(2) / 10f64.powf(snr_db / 10.0) * rate
        / sdrkit_stack_adsb::uat::demod::WORK_RATE;
    Awgn::with_power(p, seed).unwrap().process(x);
}
