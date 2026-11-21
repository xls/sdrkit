//! 802.11ah S1G numerology (IEEE Std 802.11-2020 clause 23).

use sdrkit_proto_wlan_phy::s1g;

#[test]
fn rates_of_the_mcs_tables() {
    // Tables 23-41 (1 MHz) and 23-44 (2 MHz), one spatial stream, normal guard interval, kbit/s.
    let one = [300, 600, 900, 1200, 1800, 2400, 2700, 3000, 3600, 4000];
    for (m, &want) in one.iter().enumerate() {
        assert_eq!(
            s1g::rate_bps(1, m as u8, 1, false),
            Some(want as f64 * 1e3),
            "MCS {m}"
        );
    }
    assert_eq!(s1g::rate_bps(1, 10, 1, false), Some(150e3));
    let two = [650, 1300, 1950, 2600, 3900, 5200, 5850, 6500, 7800];
    for (m, &want) in two.iter().enumerate() {
        assert_eq!(
            s1g::rate_bps(2, m as u8, 1, false),
            Some(want as f64 * 1e3),
            "MCS {m}"
        );
    }
    // 2 MHz MCS 9 with one stream is not sent (as VHT 20 MHz MCS 9).
    assert_eq!(s1g::rate_bps(2, 9, 1, false), None);
    assert_eq!(s1g::rate_bps(2, 10, 1, false), None);
    // Short guard interval: 4 MHz MCS 0 is 1.35 Mbit/s, 1.5 Mbit/s with the short one.
    assert_eq!(s1g::rate_bps(4, 0, 1, false), Some(1.35e6));
    assert!((s1g::rate_bps(4, 0, 1, true).unwrap() - 1.5e6).abs() < 1e-6);
    // 16 MHz, 4 streams, MCS 9: one tenth of VHT 160 MHz (3120 Mbit/s).
    assert_eq!(s1g::rate_bps(16, 9, 4, false), Some(312e6));
}

#[test]
fn tone_plans() {
    for (w, sd, pilots, fft) in [
        (1, 24, 2, 32),
        (2, 52, 4, 64),
        (4, 108, 6, 128),
        (8, 234, 8, 256),
        (16, 468, 16, 512),
    ] {
        let p = s1g::plan(w).unwrap();
        assert_eq!((p.data.len(), p.pilots.len()), (sd, pilots), "{w} MHz");
        assert_eq!(s1g::n_sd(w), Some(sd));
        assert_eq!(s1g::fft_size(w), Some(fft));
        assert!(!p.used().contains(&0));
        assert!(p.used().iter().all(|k| k.unsigned_abs() < fft as u32 / 2));
    }
    assert!(s1g::plan(3).is_err());
}
