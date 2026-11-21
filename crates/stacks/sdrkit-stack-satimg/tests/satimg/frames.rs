//! The frame-table stacks: the header rule, the frame component, GOES CDA, GOES MDL and GK-2A CDAS on their test signals.

use super::common::{assert_declared, impaired, run, table_rows, Ran};
use sdrkit_core::{Complex32, LockState};
use sdrkit_dev_test::Harness;
use sdrkit_graph::Component;
use sdrkit_proto_ccsds::multisync::QuadSync;
use sdrkit_proto_ccsds::wordsync::WordSyncConfig;
use sdrkit_stack_satimg::components::table::{header_ids, Check, FrameBytes, FrameTableComponent};
use sdrkit_stack_satimg::modes::goes_cda::{self, test_n_frame, test_r_frame, CdaLink};
use sdrkit_stack_satimg::typed::shape;
use sdrkit_util_bits::xorshift::Xorshift64;

#[test]
fn the_header_rule_reads_aos_and_tm_ids() {
    let a = header_ids(0x44, 0x85).unwrap();
    assert_eq!((a.aos, a.spacecraft, a.vcid), (true, 18, 5));
    let t = header_ids(0x14, 0x3A).unwrap();
    assert_eq!((t.aos, t.spacecraft, t.vcid), (false, 323, 5));
    assert!(header_ids(0x80, 0x00).is_none());
}

fn table(link: CdaLink) -> FrameTableComponent {
    let mut c = FrameTableComponent::new(link.table());
    c.configure(&[]).expect("the component");
    c
}

#[test]
fn a_goes_r_frame_takes_16_byte_errors() {
    assert_eq!(220.0 / 256.0, 0.859_375);
    assert_eq!(4000.0 * 220.0 / 256.0, 3437.5);
    assert_eq!(40_000.0 * 220.0 / 256.0, 34_375.0);
    let mut rng = Xorshift64::new(1);
    let f = test_r_frame(0x0102, &mut rng);
    assert_eq!(f.len(), 256);
    assert_eq!(f[..4], [0x1A, 0xCF, 0xFC, 0x1D]);
    // The randomiser starts FF 48 0E C0 9A on byte 4: the header 44 85 01 02 ... becomes BB CD 0F C2 ...
    assert_eq!(f[4..8], [0x44 ^ 0xFF, 0x85 ^ 0x48, 0x0E, 0x01 ^ 0xC0]);
    let mut c = table(CdaLink::R40);
    let (row, good) = c.push_frame(&f, 0.0);
    assert_eq!((row.check, row.bytes), (Check::Good, 256));
    assert!(
        row.hex.starts_with("1A CF FC 1D 44 85 00 01 02"),
        "{}",
        row.hex
    );
    let ids = row.ids.unwrap();
    assert_eq!((ids.aos, ids.spacecraft, ids.vcid), (true, 18, 5));
    assert_eq!(good.unwrap().len(), 224);
    let spoil = |n: usize| {
        let mut g = f.clone();
        for i in 0..n {
            g[4 + 15 * i] ^= 0xA5;
        }
        g
    };
    let (row, good) = c.push_frame(&spoil(16), 0.0);
    assert_eq!(row.check, Check::Good);
    assert_eq!(good.unwrap()[4..], test_frame_data(0x0102));
    let (row, good) = c.push_frame(&spoil(17), 0.0);
    assert_eq!((row.check.text(), good), ("check failed", None));
}

/// The data bytes of GOES-R test frame `counter` of seed 1.
fn test_frame_data(counter: u32) -> Vec<u8> {
    sdrkit_stack_satimg::modes::tables::test_data(counter, 220, &mut Xorshift64::new(1))
}

#[test]
fn a_goes_n_frame_has_no_check() {
    let f = test_n_frame(3, &mut Xorshift64::new(1));
    assert_eq!(f.len(), 512);
    assert_eq!(f[..6], [0xFA, 0xF3, 0x20, 0x1C, 0x44, 0x85]);
    let (row, good) = table(CdaLink::N).push_frame(&f, 0.0);
    assert_eq!((row.check.text(), row.bytes), ("none", 512));
    assert_eq!(good.unwrap(), f);
}

/// A change of the QPSK constellation, on (I, Q) soft values.
type Turn = fn((i8, i8)) -> (i8, i8);

#[test]
fn the_right_qpsk_stream_is_chosen_after_three_sync_words() {
    let cfg = WordSyncConfig {
        word: 0x0BEB_CC80,
        word_bits: 32,
        frame_bits: 3712,
        acquire_errors: 3,
        track_errors: 9,
        complement: true,
        max_misses: 3,
    };
    let mut rng = Xorshift64::new(5);
    let mut bits = Vec::new();
    for _ in 0..12 {
        bits.extend((0..32).rev().map(|i| ((0x0BEB_CC80u32 >> i) & 1) as u8));
        bits.extend((0..3712 - 32).map(|_| (rng.step() >> 40) as u8 & 1));
    }
    let symbols: Vec<(i8, i8)> = bits
        .chunks(2)
        .map(|p| {
            (
                if p[0] == 1 { -60 } else { 60 },
                if p[1] == 1 { -60 } else { 60 },
            )
        })
        .collect();
    // Turned by 0, 90, 180 and 270 degrees, and mirrored.
    let turns: [Turn; 8] = [
        |(i, q)| (i, q),
        |(i, q)| (-q, i),
        |(i, q)| (-i, -q),
        |(i, q)| (q, -i),
        |(i, q)| (q, i),
        |(i, q)| (-i, q),
        |(i, q)| (-q, -i),
        |(i, q)| (i, -q),
    ];
    for t in turns {
        let llrs: Vec<i8> = symbols
            .iter()
            .flat_map(|&s| {
                let (i, q) = t(s);
                [i, q]
            })
            .collect();
        let mut s = QuadSync::new(cfg).unwrap();
        let mut frames = Vec::new();
        s.push_llrs(&llrs, |w, f| frames.push((w, f.bytes[4..8].to_vec())));
        let chosen = s.chosen().expect("a stream");
        // Every frame of the chosen stream comes once, upright.
        let mine: Vec<&Vec<u8>> = frames
            .iter()
            .filter(|(w, _)| *w == chosen)
            .map(|(_, b)| b)
            .collect();
        assert_eq!(mine.len(), 12);
        assert!(frames.iter().all(|(w, _)| *w == chosen));
    }
}

fn go(h: &Harness, x: &[Complex32], rate: f64, centre: f64) -> Ran {
    run::<FrameBytes>(h, x, rate, centre, centre, 65_536, Some("frames.frames"))
}

fn cda(sub: &str, rate: f64, secs: f64, esn0: f64, offset: f64) -> Ran {
    let h = impaired(
        Harness::new(&goes_cda::DESCRIPTOR)
            .submode(sub)
            .rate(rate)
            .seconds(secs),
        Some(esn0),
        offset,
        0.0,
    );
    let x = h.generate().expect("a signal");
    // The test signal lies 100 kHz below the centre of the samples, the radio tuned above it.
    run::<FrameBytes>(
        &h,
        &x,
        rate,
        1693.1e6,
        1693e6,
        65_536,
        Some("frames.frames"),
    )
}

#[test]
fn goes_r_at_40_ksps_lists_frames() {
    let r = cda("r40", 1e6, 3.0, 12.0, 2e3);
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(r.num("frames_ok") >= 30.0, "{}", r.num("frames_ok"));
    assert_eq!((r.num("spacecraft"), r.num("vcid")), (18.0, 5.0));
    let frames = r.of::<FrameBytes>();
    assert!(frames
        .iter()
        .all(|f| f.bytes.len() == 224 && f.bytes[4..6] == [0x44, 0x85]));
    assert_declared(&r, &shape(goes_cda::ID).results_decl());
}

#[test]
fn goes_r_at_4_ksps_lists_frames() {
    let r = cda("r4", 1e6, 12.0, 12.0, 200.0);
    assert!(r.num("frames_ok") >= 4.0, "{}", r.num("frames_ok"));
}

#[test]
fn goes_n_lists_frames() {
    let r = cda("n", 250e3, 12.0, 15.0, 200.0);
    assert!(r.num("frames_ok") >= 4.0, "{}", r.num("frames_ok"));
    assert!(r
        .of::<FrameBytes>()
        .iter()
        .all(|f| f.bytes[..4] == [0xFA, 0xF3, 0x20, 0x1C]));
}

#[test]
fn the_65th_frame_removes_the_oldest_row() {
    let r = cda("r40", 1e6, 4.5, 20.0, 0.0);
    assert!(r.num("frames_ok") >= 66.0, "{}", r.num("frames_ok"));
    let rows: Vec<String> = table_rows(&r.results)
        .into_iter()
        .filter(|(t, _, _)| t == "frames")
        .map(|(_, k, _)| k)
        .collect();
    assert_eq!(rows.len(), 64);
    assert!(!rows.contains(&"0".to_string()));
}

#[test]
fn noise_gives_no_table_row() {
    for (sub, rate) in [("r40", 1e6), ("n", 250e3)] {
        let r = cda(sub, rate, 12.0, -30.0, 0.0);
        assert_eq!(r.num("frames_ok"), 0.0, "{sub}");
        assert!(
            table_rows(&r.results).iter().all(|(t, _, _)| t != "frames"),
            "{sub}"
        );
    }
}

#[test]
fn the_mdl_table_shows_the_frame_inverted() {
    use sdrkit_stack_satimg::modes::goes_mdl::{table, test_frame};
    let f = test_frame(0x0102, &mut Xorshift64::new(1));
    assert_eq!(f.len(), 464);
    assert_eq!(f[..6], [0x0B, 0xEB, 0xCC, 0x80, 0xFD, 0xFE]);
    assert!((200_000.0 * 2.0 / (8.0 * 464.0) - 107.8f64).abs() < 0.05);
    let mut c = FrameTableComponent::new(table());
    c.configure(&[]).unwrap();
    // The synchroniser gives the frame upright from its sync word on: the bytes after it as received.
    let (row, good) = c.push_frame(&f, 0.0);
    assert!(row.hex.starts_with("F4 14 33 7F 02 01"), "{}", row.hex);
    assert_eq!((row.check.text(), row.bytes), ("none", 464));
    assert!(good.is_some());
}

#[test]
fn a_cdas_frame_takes_16_byte_errors_in_each_code_word() {
    use sdrkit_stack_satimg::modes::gk2a_cdas::{table, test_frame};
    let f = test_frame(9, &mut Xorshift64::new(1));
    assert_eq!(f.len(), 1264);
    assert_eq!(4 + 5 * 252, 1264);
    let mut c = FrameTableComponent::new(table());
    c.configure(&[]).unwrap();
    let mut g = f.clone();
    // Byte 4 + i belongs to code word i mod 5: 16 errors in each.
    for w in 0..5 {
        for j in 0..16 {
            g[4 + w + 5 * (3 + 15 * j)] ^= 0xA5;
        }
    }
    let (row, good) = c.push_frame(&g, 0.0);
    assert_eq!(row.check, Check::Good);
    assert_eq!(good.unwrap()[..], f_data(&f)[..]);
    // One more in code word 2.
    g[4 + 2 + 5 * 250] ^= 0x3C;
    let (row, good) = c.push_frame(&g, 0.0);
    assert_eq!((row.check, good), (Check::Failed, None));
}

/// The sync marker and the 1100 data bytes of CDAS test frame 9 of seed 1.
fn f_data(f: &[u8]) -> Vec<u8> {
    let mut v = f[..4].to_vec();
    v.extend(sdrkit_stack_satimg::modes::tables::test_data(
        9,
        1100,
        &mut Xorshift64::new(1),
    ));
    v
}

#[test]
fn goes_mdl_lists_frames() {
    use sdrkit_stack_satimg::modes::goes_mdl::DESCRIPTOR;
    let h = impaired(
        Harness::new(&DESCRIPTOR).rate(3e6).seconds(2.0),
        Some(18.0),
        -4e3,
        0.0,
    );
    let x = h.generate().expect("a signal");
    let r = go(&h, &x, 3e6, 1681.478e6);
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(r.num("frames_ok") >= 40.0, "{}", r.num("frames_ok"));
    let frames = r.of::<FrameBytes>();
    assert!(frames
        .iter()
        .all(|f| f.bytes[..4] == [0xF4, 0x14, 0x33, 0x7F]));
    // The counter runs on, low byte first, once for each frame.
    let counts: Vec<u16> = frames
        .iter()
        .map(|f| u16::from_le_bytes([f.bytes[4], f.bytes[5]]))
        .collect();
    assert!(
        counts.windows(2).all(|w| w[1] == w[0].wrapping_add(1)),
        "{counts:?}"
    );
}

#[test]
fn gk2a_cdas_lists_frames() {
    use sdrkit_stack_satimg::modes::gk2a_cdas::DESCRIPTOR;
    let h = impaired(
        Harness::new(&DESCRIPTOR).rate(8e6).seconds(0.5),
        Some(18.0),
        8e3,
        0.0,
    );
    let x = h.generate().expect("a signal");
    let r = go(&h, &x, 8e6, 1690e6);
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(r.num("frames_ok") >= 60.0, "{}", r.num("frames_ok"));
    assert_eq!((r.num("spacecraft"), r.num("vcid")), (18.0, 5.0));
}

#[test]
fn qpsk_noise_gives_no_table_row() {
    use sdrkit_stack_satimg::modes::{gk2a_cdas, goes_mdl};
    for (d, rate) in [(&goes_mdl::DESCRIPTOR, 3e6), (&gk2a_cdas::DESCRIPTOR, 8e6)] {
        let h = impaired(
            Harness::new(d).rate(rate).seconds(1.5),
            Some(-30.0),
            0.0,
            0.0,
        );
        let x = h.generate().expect("a signal");
        let r = go(&h, &x, rate, 1690e6);
        assert_eq!(r.num("frames_ok"), 0.0, "{}", d.id);
        assert!(
            table_rows(&r.results).iter().all(|(t, _, _)| t != "frames"),
            "{}",
            d.id
        );
    }
}
