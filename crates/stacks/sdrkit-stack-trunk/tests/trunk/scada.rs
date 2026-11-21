//! SCADA: every link and framing through the air, impairments, noise at the stated C/N0, noise and an idle carrier, a burst
//! that is not recognised, damaged frames, the worked frames on Bell 202, the export and the table commands.

use crate::common::*;
use num_complex::Complex32;
use sdrkit_core::{Generator, InputInfo, LockState};
use sdrkit_dev_test::Awgn;
use sdrkit_stack_trunk::scada::frame::{dnp3_frame, rtu_frame};
use sdrkit_stack_trunk::scada::gen::{
    dnp3_traffic, modbus_ascii_traffic, modbus_traffic, ScadaGenerator, ScadaOptions, TxBurst,
    TxFraming, TxLink,
};
use sdrkit_stack_trunk::scada::modbus::hex;
use sdrkit_stack_trunk::typed::{SCADA_DEVICES, SCADA_MESSAGES};
use sdrkit_stack_trunk::SCADA;
use std::collections::BTreeMap;

const DIAL: f64 = 952.0125e6;

#[test]
fn the_data_file_has_the_bands() {
    let file = crate::ermes::plan_file("scada.json");
    let band = &file.groups[0].bands[0];
    assert_eq!((band.start_hz, band.end_hz), (952e6, 953e6));
    assert_eq!(file.groups.len(), 2);
}

/// One pass of the signal of `o` at `rate`, the channel at the centre, then 0.6 s without a carrier.
fn signal(rate: f64, o: &ScadaOptions) -> Vec<Complex32> {
    let mut g = ScadaGenerator::new(rate, 0.0, o.clone());
    let mut x = vec![Complex32::new(0.0, 0.0); (g.length_s() * rate) as usize];
    g.generate(&mut x);
    x.extend(std::iter::repeat_n(
        Complex32::new(0.0, 0.0),
        (0.6 * rate) as usize,
    ));
    x
}

fn receive(x: &[Complex32], rate: f64) -> (Box<dyn sdrkit_core::Receiver>, Fed) {
    let mut rx = make(&SCADA, &[]);
    configure(rx.as_mut(), rate, DIAL, 0.0);
    let fed = feed(rx.as_mut(), x, 8192);
    let mut out = sdrkit_core::Output::default();
    rx.finish(&mut out);
    (rx, fed)
}

fn messages(rx: &dyn sdrkit_core::Receiver) -> Vec<BTreeMap<String, String>> {
    rows(results(rx), &SCADA_MESSAGES)
}

/// The raw hex of each burst as a message shows it (ASCII frames as their binary bytes).
fn sent_hex(o: &ScadaOptions) -> Vec<String> {
    o.bursts
        .iter()
        .map(|b| {
            if b.bytes.first() == Some(&b':') {
                let body = &b.bytes[1..b.bytes.len() - 2];
                let v: Vec<u8> = body
                    .chunks(2)
                    .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
                    .collect();
                hex(&v)
            } else {
                hex(&b.bytes)
            }
        })
        .collect()
}

struct Case {
    link: TxLink,
    framing: TxFraming,
    traffic: Vec<TxBurst>,
    name: &'static str,
    inverted: bool,
}

fn cases() -> Vec<Case> {
    use TxFraming::*;
    use TxLink::*;
    let c = |link, framing, traffic, name, inverted| Case {
        link,
        framing,
        traffic,
        name,
        inverted,
    };
    vec![
        c(
            Bell202,
            A8N1,
            dnp3_traffic(),
            "Bell 202 AFSK 1200 Bd, 8N1",
            false,
        ),
        c(
            Bell202,
            A8E1,
            dnp3_traffic(),
            "Bell 202 AFSK 1200 Bd, 8E1",
            false,
        ),
        c(
            Bell202,
            A8O1,
            dnp3_traffic(),
            "Bell 202 AFSK 1200 Bd, 8O1",
            false,
        ),
        c(
            Bell202,
            A8N2,
            modbus_traffic(),
            "Bell 202 AFSK 1200 Bd, 8N2",
            false,
        ),
        c(
            Bell202,
            A7E1,
            modbus_ascii_traffic(),
            "Bell 202 AFSK 1200 Bd, 7E1",
            false,
        ),
        c(V23, A8E1, modbus_traffic(), "V.23 AFSK 1200 Bd, 8E1", false),
        c(
            Fsk2(4800.0),
            A8O1,
            modbus_traffic(),
            "2-FSK 4800 bit/s, 8O1",
            false,
        ),
        c(
            Fsk2(9600.0),
            A8N1,
            dnp3_traffic(),
            "2-FSK 9600 bit/s, 8N1",
            false,
        ),
        c(
            Fsk2(19_200.0),
            A8E1,
            modbus_traffic(),
            "2-FSK 19200 bit/s, 8E1",
            false,
        ),
        c(
            Fsk4(4800.0),
            A8N2,
            modbus_traffic(),
            "4-FSK 4800 bit/s (2400 symbols/s), 8N2",
            false,
        ),
        c(
            Fsk4(9600.0),
            A8E1,
            dnp3_traffic(),
            "4-FSK 9600 bit/s (4800 symbols/s), 8E1",
            false,
        ),
        c(
            Fsk4(19_200.0),
            A8N1,
            dnp3_traffic(),
            "4-FSK 19200 bit/s (9600 symbols/s), 8N1",
            false,
        ),
        c(
            Fsk2(9600.0),
            HdlcNrzi,
            dnp3_traffic(),
            "2-FSK 9600 bit/s, HDLC NRZI",
            false,
        ),
        c(
            Fsk4(9600.0),
            HdlcNrz,
            modbus_traffic(),
            "4-FSK 9600 bit/s (4800 symbols/s), HDLC NRZ",
            false,
        ),
        c(
            Fsk2(9600.0),
            A8N1,
            dnp3_traffic(),
            "2-FSK 9600 bit/s, 8N1, inverted",
            true,
        ),
    ]
}

/// The frames of `o` received at `rate` from `x`: how many of those sent, how many with a good check that were not sent,
/// the CRC failures and the bursts not recognised.
fn tally(rx: &dyn sdrkit_core::Receiver, o: &ScadaOptions) -> (usize, usize, usize, usize) {
    let sent = sent_hex(o);
    let m = messages(rx);
    let got = sent
        .iter()
        .filter(|h| m.iter().any(|r| r["raw_hex"] == **h && r["crc"] == "good"))
        .count();
    let wrong = m
        .iter()
        .filter(|r| r["crc"] == "good" && !sent.contains(&r["raw_hex"]))
        .count();
    let failed = m.iter().filter(|r| r["crc"] == "failed").count();
    let unknown = m
        .iter()
        .filter(|r| r["function"] == "not recognised")
        .count();
    (got, wrong, failed, unknown)
}

#[test]
fn every_link_and_framing_through_the_air() {
    let rate = 192e3;
    for c in cases() {
        let mut o = ScadaOptions::new(c.link, c.framing, c.traffic);
        o.inverted = c.inverted;
        let (rx, fed) = receive(&signal(rate, &o), rate);
        let (got, wrong, failed, unknown) = tally(rx.as_ref(), &o);
        let m = messages(rx.as_ref());
        assert_eq!(
            (got, wrong, failed, unknown),
            (o.bursts.len(), 0, 0, 0),
            "{}: {m:?}",
            c.name
        );
        for r in &m {
            assert_eq!(r["link"], c.name, "{r:?}");
        }
        let r = results(rx.as_ref());
        let link = text(r, "link");
        assert!(link.starts_with(c.name), "{link}");
        let dev = num(r, "deviation_hz");
        let want = c.link.default_deviation();
        match c.link {
            TxLink::Bell202 | TxLink::V23 => {
                assert!((dev - want).abs() < 600.0, "{}: {link}", c.name);
                let tones = if c.link == TxLink::Bell202 {
                    (1200.0, 2200.0)
                } else {
                    (1300.0, 2100.0)
                };
                let t = link.split("tones ").nth(1).unwrap();
                let mut it = t.split([' ', '/']).filter(|s| !s.is_empty());
                let mark: f64 = it.next().unwrap().parse().unwrap();
                let space: f64 = it.next().unwrap().parse().unwrap();
                assert!(
                    (mark - tones.0).abs() < 60.0 && (space - tones.1).abs() < 60.0,
                    "{link}"
                );
            }
            _ => assert!((dev - want).abs() < 0.2 * want, "{}: {link}", c.name),
        }
        check_events(
            &kinds(&sdrkit_stack_trunk::scada::declaration()),
            &fed.events,
            true,
        );
    }
}

/// A representative link of each chain with its framing and traffic.
fn chain_cases() -> Vec<(TxLink, TxFraming, Vec<TxBurst>)> {
    vec![
        (TxLink::Bell202, TxFraming::A8E1, modbus_traffic()),
        (TxLink::Fsk2(4800.0), TxFraming::A8O1, modbus_traffic()),
        (TxLink::Fsk2(9600.0), TxFraming::A8N1, dnp3_traffic()),
        (TxLink::Fsk2(19_200.0), TxFraming::A8E1, modbus_traffic()),
        (TxLink::Fsk4(4800.0), TxFraming::A8N2, modbus_traffic()),
        (TxLink::Fsk4(9600.0), TxFraming::A8E1, dnp3_traffic()),
        (TxLink::Fsk4(19_200.0), TxFraming::A8N1, dnp3_traffic()),
    ]
}

#[test]
fn every_chain_over_impairments() {
    for (link, framing, traffic) in chain_cases() {
        let audio = matches!(link, TxLink::Bell202 | TxLink::V23);
        let dev = link.default_deviation();
        let base = ScadaOptions::new(link, framing, traffic);
        let mut cases: Vec<(String, f64, ScadaOptions)> = vec![
            (
                "+1.5 kHz".into(),
                192e3,
                ScadaOptions {
                    offset_hz: 1500.0,
                    ..base.clone()
                },
            ),
            (
                "-1.5 kHz".into(),
                192e3,
                ScadaOptions {
                    offset_hz: -1500.0,
                    ..base.clone()
                },
            ),
            (
                "drift 400 Hz/s".into(),
                192e3,
                ScadaOptions {
                    drift_hz_s: 400.0,
                    ..base.clone()
                },
            ),
            (
                "level x 0.001".into(),
                192e3,
                ScadaOptions {
                    level: 0.001,
                    ..base.clone()
                },
            ),
            (
                "level x 30".into(),
                192e3,
                ScadaOptions {
                    level: 30.0,
                    ..base.clone()
                },
            ),
            (
                "clock +300 ppm".into(),
                192e3,
                ScadaOptions {
                    clock_ppm: 300.0,
                    ..base.clone()
                },
            ),
            (
                "clock -300 ppm".into(),
                192e3,
                ScadaOptions {
                    clock_ppm: -300.0,
                    ..base.clone()
                },
            ),
            ("1 Msps".into(), 1e6, base.clone()),
            (
                "carrier between bursts".into(),
                192e3,
                ScadaOptions {
                    carrier_between: true,
                    ..base.clone()
                },
            ),
        ];
        let (low, high) = if audio { (0.6, 1.5) } else { (0.7, 1.3) };
        for f in [low, high] {
            cases.push((
                format!("deviation x {f}"),
                192e3,
                ScadaOptions {
                    deviation_hz: Some(dev * f),
                    ..base.clone()
                },
            ));
        }
        if audio {
            for tilt in [-6.0, 6.0] {
                cases.push((
                    format!("tilt {tilt} dB"),
                    192e3,
                    ScadaOptions {
                        tilt_db: tilt,
                        ..base.clone()
                    },
                ));
            }
        }
        for (what, rate, o) in cases {
            let (rx, _) = receive(&signal(rate, &o), rate);
            let (got, wrong, failed, _) = tally(rx.as_ref(), &o);
            assert_eq!(
                (got, wrong, failed),
                (o.bursts.len(), 0, 0),
                "{link:?} {what}: {:?}",
                messages(rx.as_ref())
                    .iter()
                    .map(|r| (r["link"].clone(), r["raw_hex"].clone(), r["crc"].clone()))
                    .collect::<Vec<_>>()
            );
        }
    }
}

/// Ten Modbus requests and replies (five pairs) with noise at `cn0` dB-Hz: the frames received with a good CRC, and those
/// with a good CRC that were not sent.
fn noisy(link: TxLink, framing: TxFraming, cn0: f64, seed: u64) -> (usize, usize) {
    let rate = 192e3;
    let mut bursts = Vec::new();
    for k in 0..5u8 {
        let a = 0x11 + k;
        bursts.push(TxBurst {
            bytes: rtu_frame(&[a, 3, 0, 0x6B, 0, 3]),
            gap_s: 0.05,
        });
        bursts.push(TxBurst {
            bytes: rtu_frame(&[a, 3, 6, 2, 0x2B, 0, k, 0, 0x64]),
            gap_s: 0.1,
        });
    }
    let o = ScadaOptions::new(link, framing, bursts);
    let mut x = signal(rate, &o);
    Awgn::new(1.0, cn0, 1.0, rate, seed).process(&mut x);
    let (rx, _) = receive(&x, rate);
    let (got, wrong, _, _) = tally(rx.as_ref(), &o);
    (got, wrong)
}

#[test]
fn frames_at_the_stated_noise() {
    let cases = [
        (TxLink::Bell202, TxFraming::A8E1, 56.0),
        (TxLink::Fsk2(4800.0), TxFraming::A8E1, 56.0),
        (TxLink::Fsk2(9600.0), TxFraming::A8E1, 59.0),
        (TxLink::Fsk2(19_200.0), TxFraming::A8E1, 62.0),
        (TxLink::Fsk4(4800.0), TxFraming::A8E1, 62.0),
        (TxLink::Fsk4(9600.0), TxFraming::A8E1, 67.0),
        (TxLink::Fsk4(19_200.0), TxFraming::A8E1, 70.0),
    ];
    let mut lines = Vec::new();
    let mut bad = Vec::new();
    for (link, framing, cn0) in cases {
        let (got, wrong) = noisy(link, framing, cn0, 5);
        lines.push(format!("{link:?} at {cn0} dB-Hz: {got} of 10"));
        if got < 9 || wrong > 0 {
            bad.push(format!(
                "{link:?} at {cn0} dB-Hz: {got} of 10, {wrong} wrong"
            ));
        }
    }
    println!("{}", lines.join("\n"));
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn a_noise_sweep_gives_no_frame_that_was_not_sent() {
    for link in [TxLink::Bell202, TxLink::Fsk2(9600.0), TxLink::Fsk4(9600.0)] {
        for cn0 in [72.0, 64.0, 58.0, 52.0, 48.0, 44.0] {
            let (_, wrong) = noisy(link, TxFraming::A8N2, cn0, 11);
            assert_eq!(wrong, 0, "{link:?} at {cn0} dB-Hz");
        }
    }
}

#[test]
fn noise_and_an_idle_carrier_give_no_message() {
    let rate = 192e3;
    let noise = Awgn::with_sigma(0.1, 4).noise((rate * 5.0) as usize);
    let (rx, fed) = receive(&noise, rate);
    assert!(messages(rx.as_ref()).is_empty());
    assert!(fed.events.is_empty());
    assert!(results(rx.as_ref()).status().lock_state != LockState::Locked);
    for link in [TxLink::Bell202, TxLink::Fsk2(9600.0), TxLink::Fsk4(9600.0)] {
        let o = ScadaOptions {
            carrier_between: true,
            ..ScadaOptions::new(
                link,
                TxFraming::A8N1,
                vec![TxBurst {
                    bytes: Vec::new(),
                    gap_s: 3.0,
                }],
            )
        };
        let mut x = signal(rate, &o);
        Awgn::new(1.0, 60.0, 1.0, rate, 3).process(&mut x);
        let (rx, _) = receive(&x, rate);
        assert!(
            messages(rx.as_ref()).is_empty(),
            "{link:?}: {:?}",
            messages(rx.as_ref())
        );
    }
}

#[test]
fn an_unknown_burst_is_not_recognised_once() {
    let rate = 192e3;
    let mut x = 0x9E37_79B9u32;
    let bytes: Vec<u8> = (0..60)
        .map(|k| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            if k == 0 {
                0xFF
            } else {
                x as u8
            }
        })
        .collect();
    let o = ScadaOptions::new(
        TxLink::Fsk2(9600.0),
        TxFraming::A8N1,
        vec![TxBurst { bytes, gap_s: 0.1 }],
    );
    let (rx, _) = receive(&signal(rate, &o), rate);
    let m = messages(rx.as_ref());
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!(m[0]["function"], "not recognised");
    assert_eq!(m[0]["link"], "2-FSK 9600 bit/s");
    assert_eq!(num(results(rx.as_ref()), "not_recognised"), 1.0);
}

#[test]
fn damaged_frames_count_as_crc_failures() {
    let rate = 192e3;
    let mut modbus = rtu_frame(&[0x11, 3, 0, 0x6B, 0, 3]);
    modbus[3] ^= 0x40;
    let mut dnp = dnp3_frame(0xC4, 1, 3, &[0xC0, 0xC1, 1, 0x3C, 2, 6, 0x3C, 3, 6]);
    dnp[12] ^= 0x01;
    let o = ScadaOptions::new(
        TxLink::Bell202,
        TxFraming::A8E1,
        vec![
            TxBurst {
                bytes: modbus,
                gap_s: 0.2,
            },
            TxBurst {
                bytes: dnp,
                gap_s: 0.2,
            },
        ],
    );
    let (rx, _) = receive(&signal(rate, &o), rate);
    let m = messages(rx.as_ref());
    let failed: Vec<&str> = m
        .iter()
        .filter(|r| r["crc"] == "failed")
        .map(|r| r["protocol"].as_str())
        .collect();
    assert_eq!(failed, ["Modbus RTU", "DNP3"], "{m:?}");
    assert_eq!(num(results(rx.as_ref()), "crc_failures"), 2.0);
    assert!(
        m[1]["summary"].contains("CRC failed in data block 1"),
        "{m:?}"
    );
}

#[test]
fn the_worked_frames_on_bell_202_pair_with_a_response_time() {
    let rate = 192e3;
    let o = ScadaOptions::new(TxLink::Bell202, TxFraming::A8E1, modbus_traffic());
    let (rx, fed) = receive(&signal(rate, &o), rate);
    let m = messages(rx.as_ref());
    assert_eq!(m.len(), 4, "{m:?}");
    assert_eq!(m[0]["link"], "Bell 202 AFSK 1200 Bd, 8E1");
    assert_eq!(m[0]["summary"], "start 107 (register 40108), quantity 3");
    assert_eq!(
        m[1]["summary"],
        "6 bytes: 40108 = 555, 40109 = 0, 40110 = 100"
    );
    assert_eq!(m[1]["pair_id"], m[0]["id"]);
    let ms: f64 = m[1]["response_ms"].parse().unwrap();
    // The reply follows 0.05 s after the request and lasts 0.16 s; the time runs from the end of one to the end of the other.
    assert!(ms > 150.0 && ms < 400.0, "{ms}");
    assert_eq!(m[3]["summary"], "exception 2: illegal data address");
    assert_eq!(m[3]["pair_id"], m[2]["id"]);
    let d = rows(results(rx.as_ref()), &SCADA_DEVICES);
    assert_eq!(d.len(), 2, "{d:?}");
    assert!(d
        .iter()
        .any(|r| r["address"] == "17" && r["points"].contains("40108 = 555")));
    assert!(d
        .iter()
        .any(|r| r["address"] == "1" && r["exceptions"] == "1"));
    assert_eq!(
        fed.events
            .iter()
            .filter(|e| e.kind() == "scada.device")
            .count(),
        2
    );
}

#[test]
fn the_test_signal_locks_with_the_default_settings_and_exports() {
    let rate = SCADA.tuning.rate_hz;
    let v = sdrkit_dev_test::Harness::new(&SCADA).values().unwrap();
    let mut g = (SCADA.make_generator.unwrap())(&sdrkit_core::GenConfig::new(rate, v)).unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * 2.0) as usize];
    g.generate(&mut x);
    let mut rx = make(&SCADA, &[]);
    configure(rx.as_mut(), rate, DIAL + 50e3, -50e3);
    let fed = feed(rx.as_mut(), &x, 8192);
    let r = results(rx.as_ref());
    assert!(num(r, "messages") >= 4.0, "{}", num(r, "messages"));
    assert_eq!(r.status().lock_state, LockState::Locked);
    check_events(
        &kinds(&sdrkit_stack_trunk::scada::declaration()),
        &fed.events,
        true,
    );
    assert!(text(r, "note").contains("Not confirmed by a public document"));
    let id = rx.commands().id("export_csv").unwrap();
    let args = rx.commands().defs()[id.index()].new_args();
    let reply = sdrkit_core::owner::invoke(rx.as_mut(), id, &args)
        .unwrap()
        .unwrap();
    let csv = reply
        .iter()
        .find(|(d, _)| d.key == "csv")
        .map(|(_, v)| value_text(v, &[]))
        .unwrap();
    let mut lines = csv.lines();
    assert_eq!(lines.next(), Some(sdrkit_stack_trunk::scada::CSV_HEADER));
    let first = lines.next().unwrap();
    assert!(first.starts_with("1,"), "{first}");
    assert!(first.contains("Bell 202 AFSK 1200 Bd, 8E1"), "{first}");
}

#[test]
fn a_rate_below_the_least_is_refused() {
    let mut rx = make(&SCADA, &[]);
    assert!(rx.configure(&InputInfo::new(48e3, DIAL, DIAL)).is_err());
}

#[test]
fn clear_empties_and_the_state_round_trips() {
    let rate = 192e3;
    let o = ScadaOptions::new(TxLink::Fsk2(9600.0), TxFraming::A8E1, modbus_traffic());
    let (mut rx, _) = receive(&signal(rate, &o), rate);
    assert_eq!(messages(rx.as_ref()).len(), 4);
    let saved = rx.save_state().expect("a saved state");
    invoke(rx.as_mut(), "clear");
    feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 25_000], 8192);
    assert!(messages(rx.as_ref()).is_empty());
    assert!(rows(results(rx.as_ref()), &SCADA_DEVICES).is_empty());
    let mut back = make(&SCADA, &[]);
    configure(back.as_mut(), rate, DIAL, 0.0);
    back.restore_state(&saved).expect("the state is taken");
    feed(back.as_mut(), &vec![Complex32::new(0.0, 0.0); 25_000], 8192);
    assert_eq!(messages(back.as_ref()).len(), 4);
    assert_eq!(rows(results(back.as_ref()), &SCADA_DEVICES).len(), 2);
}
