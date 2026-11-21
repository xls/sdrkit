//! `rc_links`: the packet layouts against their worked values, the DSM codes, and the receiver on its test signal.

use super::{count, rows};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::keys;
use sdrkit_core::Complex32;
use sdrkit_dev_test::Harness;
use sdrkit_stack_drones::typed;
use sdrkit_stack_drones::mavlink_sik::generator::gfsk;
use sdrkit_stack_drones::rc_links::generator::{self, fsk_packet, Tx, TEST_CENTRE_HZ};
use sdrkit_stack_drones::rc_links::sfhss::{self, Sfhss, SfhssCrc};
use sdrkit_stack_drones::rc_links::{dsm, frsky, Submode, RC_LINKS, SUBMODE};

#[test]
fn the_frsky_d8_packet_and_the_d16_checksum_have_their_worked_values() {
    let p = frsky::d8_channels(0xABCD, 3, &[1500, 1600, 1700, 1800, 2000, 2200, 2400, 2600]);
    assert_eq!(
        p,
        [
            0x11, 0xAB, 0xCD, 0x03, 0x00, 0x01, 0xDC, 0x40, 0xA4, 0x08, 0x65, 0x76, 0xD0, 0x98,
            0x60, 0x28, 0x87, 0xA9, 0xBC, 0xA8
        ]
    );
    assert_eq!(
        frsky::parse_d8(&p).map(|f| f.detail()),
        Some("CH1-8: 1500 1600 1700 1800 2000 2200 2400 2600, packet 3".to_string())
    );
    assert_eq!(frsky::d16_crc(0, b"123456789"), 0x604A);
    let d16 = frsky::d16(0x5A3C, 17, 25, false, 29, 0x55);
    let f = frsky::parse_d16(&d16).unwrap();
    assert_eq!(f.detail(), "hop index 17, channel skip 25");
}

#[test]
fn the_sfhss_packet_and_hops_have_their_worked_values() {
    let p = Sfhss {
        head: 0x81,
        id: 0xBC11,
        hop: 7,
        code: 11,
        command: 0x02,
        channels: [1520, 1020, 2020, 1777],
        crc: SfhssCrc::Ccitt,
    };
    let b = sfhss::build(&p);
    assert_eq!(
        b,
        [0x81, 0xBC, 0x11, 0x00, 0x00, 0x3A, 0xF8, 0x1F, 0xE3, 0xF2, 0x37, 0x8A, 0xC2, 0xEE, 0x17]
    );
    let q = sfhss::parse(&b, SfhssCrc::Either).unwrap();
    assert_eq!(
        q.detail(),
        "CH1-4: 1520 1020 2020 1777, hop channel 7, hop code 11, packet 1"
    );
    assert_eq!(q.crc, SfhssCrc::Ccitt);
    assert!(sfhss::parse(&b, SfhssCrc::Cc2500).is_none());
    let mut hops = Vec::new();
    let mut h = 0;
    for _ in 0..10 {
        h = sfhss::next_hop(h, 11);
        hops.push(h);
    }
    assert_eq!(hops, [13, 26, 8, 21, 3, 16, 29, 11, 24, 6]);
}

#[test]
fn every_dsm_byte_comes_back_from_every_code_set_also_with_chip_errors() {
    let mut rng = Rng::new(4);
    for set in dsm::CodeSet::all() {
        for v in 0..=255u8 {
            let mut s = set.symbol(v);
            assert_eq!(set.despread(s), (v, 0));
            for _ in 0..8 {
                s ^= 1 << rng.below(64);
            }
            assert_eq!(set.despread(s).0, v, "row {} col {} byte {v}", set.row, set.col);
        }
    }
}

#[test]
fn the_dsmx_hop_list_follows_its_rules() {
    let id = [0x12, 0x34, 0x56, 0x79];
    let ch = dsm::dsmx_channels(id);
    assert_eq!(ch.len(), 23);
    let mut sorted = ch.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 23);
    assert!(ch.iter().all(|&c| (3..=75).contains(&c) && (c ^ id[3]) & 1 == 0));
    let groups = [
        ch.iter().filter(|&&c| c < 28).count(),
        ch.iter().filter(|&&c| (28..52).contains(&c)).count(),
        ch.iter().filter(|&&c| c >= 52).count(),
    ];
    assert!(groups[0] <= 8 && groups[1] <= 7 && groups[2] <= 8, "{groups:?}");
    assert_eq!(dsm::sop_column(id), ((0x12 + 0x34 + 0x56 + 2) % 8) as u8);
}

#[test]
fn few_random_packets_pass_the_dsm_format_check() {
    let mut rng = Rng::new(11);
    let mut pass = 0;
    for _ in 0..2000 {
        let mut p = [0u8; 16];
        rng.fill_bytes(&mut p);
        pass += usize::from(dsm::parse_data(&p).is_some());
    }
    assert!(pass <= 20, "{pass}");
}

/// `seconds` of noise of `sigma` at `fs` with the bursts of `tx` (cycle `n`) at `place` (fraction of `fs`) and `at` seconds,
/// amplitude 0.4.
fn fsk_signal(b: &[(Tx, f64, f64)], fs: f64, sigma: f32, seed: u64, seconds: f64) -> Vec<Complex32> {
    let n = (fs * seconds) as usize;
    let mut rng = Rng::new(seed);
    let mut x: Vec<Complex32> = (0..n)
        .map(|_| Complex32::new(sigma * rng.normal_f32(), sigma * rng.normal_f32()))
        .collect();
    for (k, &(tx, place, at)) in b.iter().enumerate() {
        let p = fsk_packet(tx, k as u32).unwrap();
        let s = gfsk(&p.air_bits(), p.rate, p.deviation_hz, fs).unwrap();
        add(&mut x, &s, fs, place * fs, at);
    }
    x
}

fn add(x: &mut [Complex32], s: &[Complex32], fs: f64, f: f64, at: f64) {
    let start = (at * fs) as usize;
    for (i, z) in s.iter().enumerate() {
        let Some(v) = x.get_mut(start + i) else {
            break;
        };
        let p = std::f64::consts::TAU * f * (start + i) as f64 / fs;
        *v += z * 0.4 * Complex32::new(p.cos() as f32, p.sin() as f32);
    }
}

fn frames(fs: f64, x: &[Complex32]) -> (i64, Vec<String>) {
    let run = Harness::new(&RC_LINKS).rate(fs).feed(x).unwrap();
    (
        count(&run.results, &keys::FRAMES_OK),
        rows(&run.results, &typed::LINKS),
    )
}

#[test]
fn frsky_d8_at_2_4_msps_decodes_in_noise_and_never_gives_a_false_frame() {
    let b: Vec<_> = (0..4).map(|k| (Tx::D8, -0.2 + 0.1 * k as f64, 0.003 + 0.01 * k as f64)).collect();
    for sigma in [0.04, 0.2] {
        let (n, links) = frames(2.4e6, &fsk_signal(&b, 2.4e6, sigma, 3, 0.06));
        assert!(n >= if sigma < 0.1 { 4 } else { 1 }, "sigma {sigma}: {n}");
        assert!(links.iter().all(|l| l == "FrSky/ABCD"), "{links:?}");
    }
    let (n, links) = frames(2.4e6, &fsk_signal(&b, 2.4e6, 0.3, 3, 0.06));
    assert_eq!(n, 0, "sigma 0.3: {links:?}");
}

#[test]
fn d16_and_both_flysky_layouts_overlapping_in_time_all_decode() {
    let b = [
        (Tx::D16, -0.3, 0.004),
        (Tx::Afhds, 0.05, 0.005),
        (Tx::Afhds2a, 0.3, 0.0052),
    ];
    let (n, mut links) = frames(8e6, &fsk_signal(&b, 8e6, 0.04, 5, 0.03));
    links.sort();
    assert_eq!(n, 3, "{links:?}");
    assert_eq!(links, ["FlySky/12345678", "FlySky/87654321", "FrSky/5A3C"]);
}

#[test]
fn sfhss_decodes_at_both_offsets() {
    let b = [(Tx::Sfhss, -300e3 / 8e6, 0.004), (Tx::Sfhss, 300e3 / 8e6, 0.012)];
    let (n, links) = frames(8e6, &fsk_signal(&b, 8e6, 0.04, 7, 0.03));
    assert_eq!((n, links.as_slice()), (2, &["Futaba S-FHSS/BC11".to_string()][..]));
}

/// A DSM packet of `tx` (cycle `n`) at `offset` Hz from the test centre with a clock error of `ppm`, amplitude 0.4.
fn dsm_signal(tx: Tx, n: u32, shift: f64, ppm: f64, sigma: f32, seed: u64) -> (Vec<Complex32>, u8) {
    let fs = 8e6;
    let (rf, chips) = generator::dsm_packet(tx, n, fs);
    let s = gfsk(&chips, dsm::CHIP_RATE * (1.0 + ppm * 1e-6), 250e3, fs).unwrap();
    let mut rng = Rng::new(seed);
    let mut x: Vec<Complex32> = (0..(fs * 0.006) as usize)
        .map(|_| Complex32::new(sigma * rng.normal_f32(), sigma * rng.normal_f32()))
        .collect();
    let f = 2400e6 + f64::from(rf) * 1e6 - TEST_CENTRE_HZ + shift;
    add(&mut x, &s, fs, f, 0.002);
    (x, rf)
}

#[test]
fn dsm_packets_decode_at_offsets_and_clock_errors_and_dsmx_and_dsm2_are_told_apart() {
    for (k, shift) in [-150e3, -40e3, 0.0, 60e3, 140e3].into_iter().enumerate() {
        for ppm in [-100.0, 100.0] {
            for tx in [Tx::Dsmx, Tx::Dsm2] {
                let (x, _) = dsm_signal(tx, k as u32, shift, ppm, 0.03, 13 + k as u64);
                let run = Harness::new(&RC_LINKS).rate(8e6).feed(&x).unwrap();
                let r = &run.results;
                assert_eq!(count(r, &keys::FRAMES_OK), 1, "{tx:?} {shift} {ppm}");
                let want = match tx {
                    Tx::Dsmx => "Spektrum DSM/5679",
                    _ => "Spektrum DSM/2C3D",
                };
                assert_eq!(rows(r, &typed::LINKS), [want], "{tx:?} {shift} {ppm}");
                let log = rows(r, &typed::FRAMES);
                assert_eq!(log.len(), 1);
            }
        }
    }
}

#[test]
fn the_test_signal_gives_every_transmitter_and_a_submode_only_its_links() {
    let run = super::run(Harness::new(&RC_LINKS).seconds(0.5));
    let mut links = rows(&run.results, &typed::LINKS);
    links.sort();
    assert_eq!(
        links,
        [
            "FlySky/12345678",
            "FlySky/87654321",
            "FrSky/5A3C",
            "FrSky/ABCD",
            "Futaba S-FHSS/BC11",
            "Spektrum DSM/2C3D",
            "Spektrum DSM/5679"
        ]
    );
    assert!(count(&run.results, &keys::FRAMES_OK) >= 60);
    let run = super::run(Harness::new(&RC_LINKS).seconds(0.3).set(&SUBMODE, Submode::Dsm));
    let links = rows(&run.results, &typed::LINKS);
    assert!(!links.is_empty() && links.iter().all(|l| l.starts_with("Spektrum")), "{links:?}");
}

#[test]
fn noise_alone_gives_nothing_and_flysky_and_dsm_are_dropped_at_low_rates() {
    let mut rng = Rng::new(21);
    let x: Vec<Complex32> = (0..4_000_000)
        .map(|_| Complex32::new(0.05 * rng.normal_f32(), 0.05 * rng.normal_f32()))
        .collect();
    let (n, _) = frames(8e6, &x);
    assert_eq!(n, 0);
    let run = Harness::new(&RC_LINKS).rate(2e6).seconds(0.2).run().unwrap();
    let covered = run.results.fields().at(&typed::LinkFields::COVERED).unwrap_or_default();
    assert!(covered.contains("not at this sample rate: FlySky, Spektrum DSM"), "{covered}");
    assert!(count(&run.results, &typed::LinkFields::BURSTS) > 0);
}
