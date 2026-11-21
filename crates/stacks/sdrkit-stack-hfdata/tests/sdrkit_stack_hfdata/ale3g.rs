//! 3G-ALE: the test exchange, each burst type through offsets, noise, an echo and the HF fading of ITU-R F.520, the datagrams,
//! noise alone.

use crate::common::*;
use sdrkit_algo_sim::fading::profiles::Watterson;
use sdrkit_algo_sim::fading::Profile;
use sdrkit_core::Complex32;
use sdrkit_stack_hfdata::ale3g::pdu::{self, hdl_packet, ldl_packet, sequence};
use sdrkit_stack_hfdata::ale3g::signal::{self, BW2_TAPS_DRAWN};
use sdrkit_stack_hfdata::ale3g::ALE3G;
use sdrkit_stack_hfdata::serial::gen::SymbolGen;

/// The bursts at 24 kHz, each after 0.2 s of silence, with 0.5 s at the end.
fn bursts(list: &[Vec<Complex32>]) -> Vec<Complex32> {
    let mut sym = Vec::new();
    for b in list {
        sym.extend(vec![Complex32::default(); 480]);
        sym.extend_from_slice(b);
    }
    sym.extend(vec![Complex32::default(); 1200]);
    let n = sym.len() * 10;
    generate(&mut SymbolGen::serial(24e3, sym), n as f64 / 24e3)
}

/// The texts of the PDU events of `x` through `ch`, fed in pieces of `piece`.
fn pdus(x: &[Complex32], ch: &Channel, piece: usize) -> (Vec<String>, Vec<String>) {
    let (_, out) = run_24k(&ALE3G, &through(x, 24e3, ch), piece);
    (
        event_texts(&out, "ale3g.pdu"),
        event_texts(&out, "ale3g.datagram"),
    )
}

#[test]
fn the_test_exchange_is_read() {
    let x = test_signal(&ALE3G, 24e3, &[], 9.0);
    let (p, d) = pdus(&x, &Channel::default(), 4801);
    let kinds: Vec<&str> = p
        .iter()
        .map(|t| t.split(':').next().unwrap_or(""))
        .collect();
    assert_eq!(
        kinds,
        [
            "BW0 LE_Call",
            "BW0 LE_Handshake",
            "BW0 LE_Notification",
            "BW1 TM_REQUEST",
            "BW1 TM_CONFIRM",
            "BW3 LDL_DATA",
            "BW4 LDL_ACK / LDL_EOM",
            "BW1 TM_TERM"
        ],
        "{p:?}"
    );
    assert!(
        p[0].contains("member 17 of group 3 calls member 5: analog voice circuit"),
        "{p:?}"
    );
    assert!(p[1].contains("link 39: voice traffic, channel 12"), "{p:?}");
    assert_eq!(d.len(), 1);
    assert!(
        d[0].starts_with("LDL datagram complete, 1 packet, 64 bytes: \"THE QUICK"),
        "{d:?}"
    );
}

fn bw0_set() -> Vec<Vec<Complex32>> {
    vec![
        signal::bw0(&pdu::le_call(5, 2, 17, 3)),
        signal::bw0(&pdu::le_handshake(0x3D, 1, 42)),
        signal::bw0(&pdu::le_broadcast(2, 0, 77)),
        signal::bw0(&pdu::le_notification(7, 30, 12)),
    ]
}

#[test]
fn bw0_through_its_channels() {
    let x = bursts(&bw0_set());
    let echo = Profile::Echo {
        delay_s: 0.001,
        level_db: -6.0,
        phase_deg: 50.0,
    };
    let cases: Vec<(&str, Channel)> = vec![
        ("clean", Channel::default()),
        (
            "+60 Hz",
            Channel {
                offset_hz: 60.0,
                ..Channel::default()
            },
        ),
        (
            "-75 Hz",
            Channel {
                offset_hz: -75.0,
                ..Channel::default()
            },
        ),
        (
            "3 Hz/s",
            Channel {
                drift_hz_s: 3.0,
                ..Channel::default()
            },
        ),
        (
            "+60 ppm",
            Channel {
                ppm: 60.0,
                ..Channel::default()
            },
        ),
        (
            "-60 ppm",
            Channel {
                ppm: -60.0,
                ..Channel::default()
            },
        ),
        (
            "echo",
            Channel {
                fading: Some(echo),
                snr_db: Some(20.0),
                seed: 2,
                ..Channel::default()
            },
        ),
        (
            "6 dB",
            Channel {
                snr_db: Some(6.0),
                seed: 3,
                ..Channel::default()
            },
        ),
        (
            "0 dB",
            Channel {
                snr_db: Some(0.0),
                seed: 4,
                ..Channel::default()
            },
        ),
        (
            "-4 dB",
            Channel {
                snr_db: Some(-4.0),
                seed: 5,
                ..Channel::default()
            },
        ),
    ];
    for (name, ch) in cases {
        let (p, _) = pdus(&x, &ch, 4801);
        assert_eq!(p.len(), 4, "{name}: {p:?}");
    }
    let mut n = 0;
    for seed in 0..2 {
        let ch = Channel {
            snr_db: Some(-8.0),
            seed: 10 + seed,
            ..Channel::default()
        };
        n += pdus(&x, &ch, 4801).0.len();
    }
    assert!(n >= 5, "-8 dB: {n} of 8");
}

#[test]
fn bw1_through_its_channels() {
    let set = vec![
        signal::bw1(&pdu::tm(2, 163, 547, 0, 7)),
        signal::bw1(&pdu::tm(3, 547, 163, 1, 7)),
        signal::bw1(&pdu::tm(3, 547, 163, 2, 0)),
        signal::bw1(&pdu::hdl_ack(0xF0_000F)),
        signal::bw1(&pdu::hdl_eom()),
    ];
    let x = bursts(&set);
    let cases: Vec<(&str, Channel, usize)> = vec![
        ("clean", Channel::default(), 4801),
        (
            "+60 Hz",
            Channel {
                offset_hz: 60.0,
                ..Channel::default()
            },
            4801,
        ),
        (
            "-75 Hz",
            Channel {
                offset_hz: -75.0,
                ..Channel::default()
            },
            4801,
        ),
        (
            "0 dB",
            Channel {
                snr_db: Some(0.0),
                seed: 6,
                ..Channel::default()
            },
            4801,
        ),
        (
            "-4 dB",
            Channel {
                snr_db: Some(-4.0),
                seed: 7,
                ..Channel::default()
            },
            77,
        ),
    ];
    for (name, ch, piece) in cases {
        let (p, _) = pdus(&x, &ch, piece);
        assert_eq!(p.len(), 5, "{name}: {p:?}");
    }
    // ITU-R Poor at 10 dB.
    let mut n = 0;
    for seed in 0..2 {
        let ch = Channel {
            snr_db: Some(10.0),
            fading: Some(Profile::Watterson(Watterson::Poor)),
            seed: 20 + seed,
            ..Channel::default()
        };
        n += pdus(&x, &ch, 4801).0.len();
    }
    assert!(n >= 7, "ITU-R Poor: {n} of 10");
}

#[test]
fn bw2_datagrams() {
    let data: Vec<u8> = (0..600u32)
        .map(|i| b"0123456789ABCDEF"[(i % 16) as usize])
        .collect();
    let chunks: Vec<&[u8]> = data.chunks(233).collect();
    let packets = vec![
        hdl_packet(chunks[0], sequence(true, false, 2)),
        hdl_packet(chunks[1], sequence(false, false, 1)),
        hdl_packet(chunks[2], sequence(false, true, chunks[2].len() as u32)),
    ];
    for (ft, snr, off) in [
        (0u32, None, 0.0),
        (1, Some(25.0), 0.0),
        (5, Some(18.0), 35.0),
        (7, Some(20.0), -50.0),
    ] {
        let x = bursts(&[signal::bw2(&packets, ft, BW2_TAPS_DRAWN)]);
        let ch = Channel {
            snr_db: snr,
            offset_hz: off,
            seed: 30 + u64::from(ft),
            ..Channel::default()
        };
        let (p, d) = pdus(&x, &ch, 4801);
        assert_eq!(p.len(), 3, "FTcount {ft}: {p:?}");
        assert_eq!(d.len(), 1, "FTcount {ft}");
        assert!(d[0].contains("600 bytes: \"0123456789ABCDEF"), "{d:?}");
    }
}

#[test]
fn bw3_packets_and_both_halves() {
    let text: Vec<u8> = (0..128u32).map(|i| b'A' + (i % 26) as u8).collect();
    let echo = Profile::Echo {
        delay_s: 0.001,
        level_db: -6.0,
        phase_deg: 120.0,
    };
    for n in [64usize, 128] {
        let pkt = ldl_packet(&text[..n], n, sequence(true, true, n as u32));
        for ft in [0u32, 1] {
            let x = bursts(&[signal::bw3(&pkt, n, ft).unwrap()]);
            for (name, ch) in [
                ("clean", Channel::default()),
                (
                    "-40 Hz 3 dB",
                    Channel {
                        offset_hz: -40.0,
                        snr_db: Some(3.0),
                        seed: 40,
                        ..Channel::default()
                    },
                ),
                (
                    "echo",
                    Channel {
                        fading: Some(echo.clone()),
                        snr_db: Some(15.0),
                        seed: 41,
                        ..Channel::default()
                    },
                ),
            ] {
                let (p, d) = pdus(&x, &ch, 4801);
                assert_eq!(p.len(), 1, "{n} bytes, FTcount {ft}, {name}: {p:?}");
                assert_eq!(d.len(), 1, "{n} bytes, FTcount {ft}, {name}");
            }
        }
    }
}

#[test]
fn bw4_words() {
    let x = bursts(&(0..4).map(signal::bw4).collect::<Vec<_>>());
    for (name, ch) in [
        ("clean", Channel::default()),
        (
            "-6 dB +55 Hz",
            Channel {
                snr_db: Some(-6.0),
                offset_hz: 55.0,
                seed: 50,
                ..Channel::default()
            },
        ),
        (
            "ITU-R Poor 3 dB",
            Channel {
                snr_db: Some(3.0),
                fading: Some(Profile::Watterson(Watterson::Poor)),
                seed: 51,
                ..Channel::default()
            },
        ),
    ] {
        let (p, _) = pdus(&x, &ch, 4801);
        let words: Vec<String> = p
            .iter()
            .map(|t| t.split(": ").nth(1).unwrap_or("").to_string())
            .collect();
        assert_eq!(words, ["00", "01", "10", "11"], "{name}: {p:?}");
    }
}

#[test]
fn the_exchange_with_offset_clock_and_noise() {
    let x = test_signal(&ALE3G, 24e3, &[], 9.0);
    let ch = Channel {
        offset_hz: 25.0,
        ppm: 20.0,
        snr_db: Some(15.0),
        seed: 60,
        ..Channel::default()
    };
    let (p, d) = pdus(&x, &ch, 4801);
    assert_eq!(p.len(), 8, "{p:?}");
    assert_eq!(d.len(), 1);
}

#[test]
fn noise_gives_no_good_pdu() {
    let x = vec![Complex32::default(); 24_000 * 8];
    let (p, d) = pdus(
        &x,
        &Channel {
            snr_db: Some(0.0),
            seed: 70,
            ..Channel::default()
        },
        4801,
    );
    assert!(p.is_empty() && d.is_empty(), "{p:?}");
}

#[test]
fn bw3_through_itu_poor() {
    let text: Vec<u8> = (0..64u32).map(|i| b'a' + (i % 26) as u8).collect();
    let pkt = ldl_packet(&text, 64, sequence(true, true, 64));
    let x = bursts(&[signal::bw3(&pkt, 64, 0).unwrap()]);
    let mut n = 0;
    for seed in 0..10 {
        let ch = Channel {
            snr_db: Some(10.0),
            fading: Some(Profile::Watterson(Watterson::Poor)),
            seed: 80 + seed,
            ..Channel::default()
        };
        n += pdus(&x, &ch, 4801).0.len();
    }
    assert!(n >= 3, "ITU-R Poor 10 dB: {n} of 10");
}

#[test]
fn a_bw2_datagram_of_six_packets() {
    let data: Vec<u8> = (0..1200u32).map(|i| b'A' + (i % 26) as u8).collect();
    let chunks: Vec<&[u8]> = data.chunks(233).collect();
    let n = chunks.len();
    let packets: Vec<Vec<u8>> = chunks
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let seq = if i == 0 {
                sequence(true, false, (n - 1) as u32)
            } else if i == n - 1 {
                sequence(false, true, c.len() as u32)
            } else {
                sequence(false, false, (n - 1 - i) as u32)
            };
            hdl_packet(c, seq)
        })
        .collect();
    let x = bursts(&[signal::bw2(&packets, 2, BW2_TAPS_DRAWN)]);
    let ch = Channel {
        snr_db: Some(18.0),
        seed: 90,
        ..Channel::default()
    };
    let (p, d) = pdus(&x, &ch, 4801);
    assert_eq!(p.len(), 6, "{p:?}");
    assert_eq!(d.len(), 1);
    assert!(d[0].contains("1200 bytes"), "{d:?}");
}
