//! Protocol 2 against a fake radio on a fake network.

use crate::fake2::Fake2;
use crate::tests::{all, cells, drain, drv, has, hpsdr_error, kind, on, run, take, tx_values};
use crate::{HpsdrError, HpsdrKindCommand, HPSDR2};
use sdrkit_core::net::NetError;
use sdrkit_core::{Complex32, SettingSet};
use sdrkit_driver_core::{DriverError, TxSetting, TxState};
use std::time::{Duration, Instant};

#[test]
fn discovery_of_a_protocol2_radio() {
    let fake = Fake2::orion_mk2();
    let mut k = kind(drv(HPSDR2, fake.transport()));
    let f = run(
        &mut k,
        HpsdrKindCommand::Discover,
        "",
        Duration::from_millis(200),
    )
    .unwrap();
    assert_eq!(f.len(), 1);
    let (address, _, board, receivers, mac, version, protocol) = cells(&f[0]);
    assert_eq!((board, receivers), (5, Some(4)));
    assert_eq!(
        (mac.as_str(), version.as_str(), protocol),
        ("00:1C:C0:A2:56:78", "21", Some(38))
    );
    assert_eq!(address, fake.addr());
}

#[test]
fn protocol2_open_sends_the_documented_packets() {
    let fake = Fake2::orion_mk2();
    let d = drv(HPSDR2, fake.transport());
    let mut src = d
        .open_source(
            &fake.addr(),
            &d.tune(7_074_000.0, 192_000.0, Some(-10.0), &[]),
        )
        .unwrap();
    let (rate, mut r) = src.start();
    assert_eq!(rate, 192_000.0);
    let l = src.lines();
    assert!(!has(&l, "specified for up to"), "{l:?}");
    // General.
    let g = fake.with(|s| s.general[0].clone());
    let mut want = vec![
        0, 0, 0, 0, 0, 0x04, 0x01, 0x04, 0x02, 0x04, 0x03, 0x04, 0x01, 0x04, 0x04, 0x04, 0x05,
        0x04, 0x0b, 0x04, 0x02, 0x04, 0x03,
    ];
    want.extend([0u8; 14]);
    want.extend([0x08, 0x01]);
    want.extend([0u8; 19]);
    want.extend([0x00, 0x01]);
    assert_eq!(g, want);
    // Receiver-specific: byte 4 is the ADC count (2 on an Orion MkII).
    let rs = fake.with(|s| s.rx_specific[0].clone());
    assert_eq!(rs.len(), 1444);
    assert_eq!(rs[..4], [0, 0, 0, 0]);
    assert_eq!(rs[4], 2);
    assert_eq!(rs[5..8], [0, 0, 0x01]);
    assert!(rs[8..17].iter().all(|&b| b == 0));
    assert_eq!(rs[17..23], [0x00, 0x00, 0xc0, 0x00, 0x00, 0x18]);
    assert!(rs[23..].iter().all(|&b| b == 0));
    // Transmitter-specific.
    let ts = fake.with(|s| s.tx_specific[0].clone());
    let mut want = vec![0u8; 60];
    want[4] = 1;
    want[14..17].copy_from_slice(&[0x00, 0xc0, 0x18]);
    assert_eq!(ts, want);
    // High-priority.
    let hp = fake.with(|s| s.hp[0].1.clone());
    assert_eq!(hp.len(), 1444);
    assert_eq!(hp[..5], [0, 0, 0, 0, 1]);
    assert!(hp[5..9].iter().all(|&b| b == 0));
    for n in 0..80 {
        assert_eq!(
            hp[9 + 4 * n..13 + 4 * n],
            [0x0e, 0xbc, 0xcc, 0xcd],
            "DDC {n}"
        );
    }
    assert_eq!(hp[329..333], [0x0e, 0xbc, 0xcc, 0xcd]);
    assert_eq!(hp[345], 0);
    assert_eq!((hp[1442], hp[1443]), (0x0a, 0x0a));
    assert_eq!(
        u32::from_be_bytes([hp[1432], hp[1433], hp[1434], hp[1435]]),
        crate::plan::alex::HPF_6_5
    );
    // Samples, I first.
    assert!(all(&take(&mut r, 2000), 0.5, -0.25, 1e-6));
    src.retune(&d.tune(7_074_000.0, 192_000.0, Some(-10.0), &[("swap_iq", on())]))
        .unwrap();
    std::thread::sleep(Duration::from_millis(30));
    drain(&mut r);
    assert!(all(&take(&mut r, 1000), -0.25, 0.5, 1e-6));
    // DDC1 is not open: its packets are ignored.
    fake.with(|s| s.stray_ddc1 = true);
    std::thread::sleep(Duration::from_millis(50));
    drain(&mut r);
    assert!(all(&take(&mut r, 2000), -0.25, 0.5, 1e-6));
    // 16 bits per sample: ignored.
    fake.with(|s| s.bits = 16);
    std::thread::sleep(Duration::from_millis(30));
    drain(&mut r);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(r.available(), 0);
    fake.with(|s| s.bits = 24);
    // Keep-alive every 200 ms, with the next sequence number.
    let n0 = fake.with(|s| s.hp.len());
    std::thread::sleep(Duration::from_millis(1050));
    let hps = fake.with(|s| s.hp.clone());
    let k = hps.len() - n0;
    assert!((4..=7).contains(&k), "{k}");
    for w in hps.windows(2) {
        let (a, b) = (
            u32::from_be_bytes(w[0].1[..4].try_into().unwrap()),
            u32::from_be_bytes(w[1].1[..4].try_into().unwrap()),
        );
        assert_eq!(b, a + 1);
    }
    // Overload status.
    fake.with(|s| s.overload = true);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(src.flag(&crate::OVERLOAD), Some(true));
    assert_eq!(src.flag(&crate::PLL), Some(true));
    // Stop: two high-priority packets with byte 4 = 0.
    let n1 = fake.with(|s| s.hp.len());
    let t = Instant::now();
    src.stop();
    assert!(t.elapsed() < Duration::from_millis(250));
    let hps = fake.with(|s| s.hp[n1..].to_vec());
    assert!(hps.len() >= 2);
    let last2 = &hps[hps.len() - 2..];
    assert!(last2.iter().all(|(_, b)| b[4] == 0));
    assert!(last2[1].0 - last2[0].0 >= Duration::from_millis(9));
    assert_eq!(d.live(), 0);
}

#[test]
fn protocol2_receivers_have_their_own_rates() {
    let fake = Fake2::orion_mk2();
    let d = drv(HPSDR2, fake.transport());
    let mut a = d
        .open_source(&fake.addr(), &d.tune(7_074_000.0, 192_000.0, None, &[]))
        .unwrap();
    let (_, mut ra) = a.start();
    let mut b = d
        .open_source(
            &format!("{}/rx2", fake.addr()),
            &d.tune(14_200_000.0, 48_000.0, None, &[]),
        )
        .unwrap();
    let (rate, mut rb) = b.start();
    assert_eq!(rate, 48_000.0);
    let l = b.lines();
    assert!(!has(&l, "share one sample rate"), "{l:?}");
    assert!(fake.wait(Duration::from_secs(1), |s| s
        .rx_specific
        .last()
        .is_some_and(|p| p[7] == 0x03)));
    let rs = fake.with(|s| s.rx_specific.last().unwrap().clone());
    assert_eq!(rs[23..29], [0x00, 0x00, 0x30, 0x00, 0x00, 0x18]);
    assert!(all(&take(&mut ra, 1000), 0.5, -0.25, 1e-6));
    assert!(all(&take(&mut rb, 1000), 0.375, -0.25, 1e-6));
    let hp = fake.with(|s| s.hp.last().unwrap().1.clone());
    assert_eq!(
        hp[13..17],
        crate::plan::phase_word(14_200_000).to_be_bytes()
    );
    assert_eq!(hp[17..21], crate::plan::phase_word(7_074_000).to_be_bytes());
    b.stop();
    assert!(fake.wait(Duration::from_secs(1), |s| s
        .rx_specific
        .last()
        .is_some_and(|p| p[7] == 0x01)));
    assert!(fake.with(|s| s.running));
    a.stop();
    assert!(fake.wait(Duration::from_millis(300), |s| !s.running));
}

#[test]
fn protocol2_transmit() {
    let fake = Fake2::orion_mk2();
    let d = drv(HPSDR2, fake.transport());
    let mut src = d
        .open_source(&fake.addr(), &d.tune(7_074_000.0, 192_000.0, None, &[]))
        .unwrap();
    let (_, _r) = src.start();
    let mut tx = src.transmitter(&tx_values(7.1e6, 6.0)).unwrap();
    assert!(fake.with(|s| s.general.iter().all(|g| g[58] == 0)));
    tx.arm().unwrap();
    assert!(fake.wait(Duration::from_millis(300), |s| s
        .general
        .last()
        .is_some_and(|g| g[58] == 1)));
    assert!(fake.wait(Duration::from_millis(300), |s| s
        .hp
        .last()
        .is_some_and(|h| h.1[4] == 0x03)));
    let hp = fake.with(|s| s.hp.last().unwrap().1.clone());
    assert_eq!(hp[329..333], [0x0e, 0xca, 0xaa, 0xab]);
    assert_eq!(hp[345], 128);
    let chunk = vec![Complex32::new(0.5, -0.25); 4096];
    let d0 = fake.with(|s| s.duc);
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(1) {
        let _ = tx.push(&chunk, Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(2));
    }
    let rate = (fake.with(|s| s.duc) - d0) as f64 / t0.elapsed().as_secs_f64();
    assert!((rate - 800.0).abs() < 800.0 * 0.1, "{rate}");
    let p = fake.with(|s| s.duc_last.clone().unwrap());
    assert_eq!(p.len(), 1444);
    for k in 0..240 {
        assert_eq!(
            p[4 + 6 * k..10 + 6 * k],
            [0x40, 0x00, 0x00, 0xe0, 0x00, 0x00],
            "sample {k}"
        );
    }
    tx.disarm();
    assert!(fake.wait(Duration::from_millis(300), |s| s
        .hp
        .last()
        .is_some_and(|h| h.1[4] == 0x01)));
    assert!(fake.wait(Duration::from_millis(300), |s| s
        .general
        .last()
        .is_some_and(|g| g[58] == 0)));
    assert_eq!(tx.state(), TxState::Disarmed);
    tx.close();
    src.stop();
}

#[test]
fn protocol2_open_failures_and_a_busy_radio() {
    // No DDC packet within 1.5 s, General and High-priority resent every 400 ms.
    let fake = Fake2::orion_mk2();
    fake.with(|s| s.silent = true);
    let d = drv(HPSDR2, fake.transport()).with_timing(crate::Timing { open_attempts: 1 });
    let t = Instant::now();
    let e = d
        .open_source(&fake.addr(), &d.tune(7_074_000.0, 192_000.0, None, &[]))
        .err()
        .unwrap();
    let took = t.elapsed();
    assert_eq!(e, NetError::TimedOut.into());
    assert!(e.retryable());
    assert!(
        took >= Duration::from_millis(1500) && took < Duration::from_millis(2300),
        "{took:?}"
    );
    let (g, hp) = fake.with(|s| (s.general.len(), s.hp.len()));
    assert!((3..=5).contains(&g), "General sent {g} times");
    assert!((3..=5).contains(&hp), "High-priority sent {hp} times");
    assert_eq!(d.live(), 0);

    // A radio busy at discovery is taken over, with a log line. Its readings show it.
    fake.with(|s| {
        s.silent = false;
        s.reply[4] = 0x03;
    });
    let mut src = d
        .open_source(&fake.addr(), &d.tune(7_074_000.0, 192_000.0, None, &[]))
        .unwrap();
    let (_, mut r) = src.start();
    assert!(all(&take(&mut r, 1000), 0.5, -0.25, 1e-6));
    let l = src.lines();
    assert!(
        l.iter()
            .any(|x| x == "HPSDR: the radio is streaming to another program; it is taken over"),
        "{l:?}"
    );
    assert_eq!(src.flag(&crate::BUSY), Some(true));
    assert_eq!(src.int(&crate::BOARD), Some(5));
    assert_eq!(src.text(&crate::PROTOCOL).as_deref(), Some("3.8"));
    assert_eq!(src.int(&crate::RADIO_RECEIVERS), Some(4));
    // RX5 of a radio that said four DDCs.
    let e = d
        .open_source(
            &format!("{}/rx5", fake.addr()),
            &d.tune(7e6, 48_000.0, None, &[]),
        )
        .err()
        .unwrap();
    assert_eq!(e, DriverError::Streams { given: 5, has: 4 });
    assert!(!e.retryable());
    src.stop();
}

#[test]
fn protocol2_sequence_gaps_are_counted_per_ddc() {
    let fake = Fake2::orion_mk2();
    let d = drv(HPSDR2, fake.transport());
    let mut src = d
        .open_source(&fake.addr(), &d.tune(7_074_000.0, 192_000.0, None, &[]))
        .unwrap();
    let (_, mut r) = src.start();
    assert!(all(&take(&mut r, 1000), 0.5, -0.25, 1e-6));
    assert_eq!(src.int(&crate::SEQ_LOST), Some(0));
    fake.with(|s| s.skip_ddc0 = 5);
    assert!(fake.wait(Duration::from_secs(1), |s| s.skip_ddc0 == 0));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(src.int(&crate::SEQ_LOST), Some(5 * 238));
    // A jump of 100 000 or more is a restart of the radio's counter, not a loss.
    fake.with(|s| s.skip_ddc0 = 200_000);
    assert!(fake.wait(Duration::from_secs(1), |s| s.skip_ddc0 == 0));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(src.int(&crate::SEQ_LOST), Some(5 * 238));
    src.stop();
}

#[test]
fn protocol2_lost_stream_is_reopened_without_a_stop() {
    let fake = Fake2::orion_mk2();
    let d = drv(HPSDR2, fake.transport());
    let mut src = d
        .open_source(&fake.addr(), &d.tune(7_074_000.0, 192_000.0, None, &[]))
        .unwrap();
    let (_, mut r) = src.start();
    assert!(all(&take(&mut r, 1000), 0.5, -0.25, 1e-6));
    let mut tx = src.transmitter(&tx_values(7.1e6, 14.0)).unwrap();
    tx.arm().unwrap();
    assert!(fake.wait(Duration::from_millis(300), |s| s
        .hp
        .last()
        .is_some_and(|h| h.1[4] == 0x03)));
    let _ = src.lines();
    fake.with(|s| s.silent = true);
    let t0 = Instant::now();
    let want = format!("HPSDR: no samples from {} for 3 s", fake.addr());
    let mut seen = Vec::new();
    let lost_after = loop {
        seen.extend(src.lines());
        if seen.iter().any(|l| l == &want) {
            break t0.elapsed();
        }
        assert!(t0.elapsed() < Duration::from_secs(5), "{seen:?}");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(
        lost_after >= Duration::from_millis(2700) && lost_after <= Duration::from_millis(3400),
        "{lost_after:?}"
    );
    // The transmitter is closed with the stream.
    assert_eq!(tx.state(), TxState::Closed);
    // A lost stream is not sent a stop (run = 0).
    assert!(fake.with(|s| s.hp.iter().all(|h| h.1[4] & 1 == 1)));
    std::thread::sleep(Duration::from_millis(1500));
    fake.with(|s| s.silent = false);
    let end = Instant::now() + Duration::from_secs(6);
    loop {
        seen.extend(src.lines());
        if seen
            .iter()
            .any(|l| l.contains("HPSDR RX1: back after") && l.contains("samples lost"))
        {
            break;
        }
        assert!(Instant::now() < end, "{seen:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(src.loss().unwrap().link_down > 0);
    drain(&mut r);
    assert!(all(&take(&mut r, 1000), 0.5, -0.25, 1e-6));
    assert!(fake.with(|s| s.hp.iter().all(|h| h.1[4] & 1 == 1)));
    drop(tx);
    src.stop();
    assert!(fake.wait(Duration::from_millis(300), |s| !s.running));
    assert_eq!(d.live(), 0);
}

#[test]
fn protocol2_transmit_sets_the_pa_bit_and_the_alex_filters_only_on_air() {
    let fake = Fake2::orion_mk2();
    let d = drv(HPSDR2, fake.transport());
    let mut src = d
        .open_source(&fake.addr(), &d.tune(14_200_000.0, 192_000.0, None, &[]))
        .unwrap();
    let (_, _r) = src.start();
    assert!(src.capabilities().transmit.is_some());
    let alex = |h: &[u8]| u32::from_be_bytes([h[1432], h[1433], h[1434], h[1435]]);
    let mut tx = src.transmitter(&tx_values(3.6e6, 0.0)).unwrap();
    // An open transmitter that is not armed: no PTT, no drive, no PA.
    std::thread::sleep(Duration::from_millis(250));
    assert!(fake.with(|s| s.general.iter().all(|g| g[58] == 0)));
    assert!(fake.with(|s| s.hp.iter().all(|h| h.1[4] & 2 == 0 && h.1[345] == 0)));
    // Outside the range: refused, final.
    let e = tx.configure(&tx_values(70e6, 0.0)).err().unwrap();
    assert_eq!(
        e,
        DriverError::Refused {
            id: TxSetting::Frequency.id()
        }
    );
    assert!(!e.retryable());
    // A second transmitter of the same radio.
    let e = src.transmitter(&tx_values(3.6e6, 0.0)).err().unwrap();
    assert_eq!(hpsdr_error(&e), Some(HpsdrError::TransmitterTaken));
    // On air: the low-pass filter for the transmit frequency joins the high-pass of DDC0.
    tx.arm().unwrap();
    assert!(fake.wait(Duration::from_millis(300), |s| s
        .hp
        .last()
        .is_some_and(|h| h.1[4] == 0x03)));
    assert!(fake.wait(Duration::from_millis(300), |s| s
        .general
        .last()
        .is_some_and(|g| g[58] == 1)));
    let hp = fake.with(|s| s.hp.last().unwrap().1.clone());
    assert_eq!(
        alex(&hp),
        crate::plan::alex::HPF_13 | crate::plan::alex::LPF_80
    );
    assert_eq!(hp[345], 255);
    assert_eq!(
        hp[329..333],
        crate::plan::phase_word(3_600_000).to_be_bytes()
    );
    tx.close();
    assert!(fake.wait(Duration::from_millis(300), |s| s
        .hp
        .last()
        .is_some_and(|h| h.1[4] == 0x01)));
    assert!(fake.wait(Duration::from_millis(300), |s| s
        .general
        .last()
        .is_some_and(|g| g[58] == 0)));
    let hp = fake.with(|s| s.hp.last().unwrap().1.clone());
    assert_eq!(alex(&hp), crate::plan::alex::HPF_13);
    assert_eq!(hp[345], 0);
    assert_eq!(
        hp[329..333],
        crate::plan::phase_word(14_200_000).to_be_bytes()
    );
    src.stop();
}

/// The General packet carries the amplifier bit, so the stream sends it again every second: a radio that missed the
/// packet that turned the amplifier off receives the next one.
#[test]
fn protocol2_sends_the_general_packet_again_while_it_runs() {
    let fake = Fake2::orion_mk2();
    let d = drv(HPSDR2, fake.transport());
    let mut src = d
        .open_source(&fake.addr(), &d.tune(7_074_000.0, 192_000.0, None, &[]))
        .unwrap();
    let (_, _r) = src.start();
    let at_start = fake.with(|s| s.general.len());
    assert!(fake.wait(Duration::from_millis(2600), |s| s.general.len()
        >= at_start + 2));
    assert!(fake.with(|s| s.general.iter().all(|g| g[58] == 0)));
}
