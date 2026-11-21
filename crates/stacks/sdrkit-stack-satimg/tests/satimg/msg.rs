//! MSG raw data: the frame and packet layers, the SEVIRI time and line rules, and the receiver on its test signal.

use super::common::{assert_declared, impaired, run, Ran};
use sdrkit_codec_image::strip::Strip;
use sdrkit_core::{Complex32, LockState};
use sdrkit_dev_test::Harness;
use sdrkit_proto_ccsds::packet::{PrimaryHeader, SeqFlags};
use sdrkit_proto_ccsds::tm::FirstHeader;
use sdrkit_stack_satimg::gen::{space_packet, test_value};
use sdrkit_stack_satimg::instruments::seviri::{
    clock, cycle_pos, first_line, hrv_row, row, test_user, SeviriDecoder, HRV, HRV_LINE, LINE,
    SEVIRI, USER_BYTES, WIDTH,
};
use sdrkit_stack_satimg::modes::msg::{
    test_frame, CADU_BYTES, CODED_BYTES, DESCRIPTOR, FRAME, GEN_START,
};

#[test]
fn the_frame_has_a_packet_zone_of_1107_bytes() {
    assert_eq!(
        (CADU_BYTES, CODED_BYTES, FRAME.frame_len),
        (1279, 1275, 1115)
    );
    assert_eq!(FRAME.data_zone_len(), 1107);
    let f = test_frame(323, 7, FirstHeader::At(0), &[0; 1107]);
    assert_eq!(f[..2], [0x50, 0xC0]);
    let p = FRAME.parse(&f).unwrap();
    assert_eq!(
        u16::from(p.header.version) << 8 | u16::from(p.header.spacecraft),
        323
    );
    assert_eq!(p.header.vcid, 0);
}

#[test]
fn the_packet_header_holds_the_part_in_its_count() {
    let p = space_packet(2046, 3, 0x0125, &[0; USER_BYTES], false);
    assert_eq!(p[..6], [0x07, 0xFE, 0xC1, 0x25, 0x38, 0x31]);
    let h = PrimaryHeader::parse(&p).unwrap();
    assert_eq!(
        (h.apid, h.flags, h.count & 15),
        (2046, SeqFlags::Unsegmented, 5)
    );
}

#[test]
fn the_time_places_the_lines_in_the_cycle() {
    let pos = cycle_pos(323, 0x0000_01A6_4A8A).unwrap();
    assert!((pos - 60.291).abs() < 0.001, "{pos}");
    assert_eq!(first_line(0, pos), 300);
    for p in 11..16u8 {
        assert_eq!(first_line(p, pos), 900 + 2 * u32::from(p - 11));
    }
    for (sat, want) in [(322, 142), (323, 538), (324, 807)] {
        let (day, extra) = clock(sat).unwrap();
        assert_eq!((u64::from(day) * 86_400 + u64::from(extra)) % 900, want);
    }
    assert!(clock(321).is_none() && clock(400).is_none());
}

#[test]
fn the_rows_run_from_the_bottom_and_the_columns_are_mirrored() {
    assert_eq!(
        (row(300), row(301), row(302)),
        (Some(2090), None, Some(2089))
    );
    assert_eq!(hrv_row(900), Some(4199));
    assert_eq!(hrv_row(903), Some(4198));
    assert_eq!((hrv_row(904), hrv_row(905)), (None, None));
    assert_eq!(hrv_row(906), Some(4197));
    assert_eq!(hrv_row(908), None);
    // Part 0 and part 3 of line 300, and the five HRV parts.
    let mut d = SeviriDecoder::default();
    for part in [0u8, 3, 11, 12, 13, 14, 15] {
        let user = test_user(323, part, 300, 4000, test_value);
        let mut strips = Vec::new();
        d.push(323, u16::from(part), &user, &mut strips);
        let rows: Vec<u32> = strips.iter().map(|s| s.first_row).collect();
        match part {
            0 | 3 => assert_eq!(rows, [2090, 2089]),
            11 => assert_eq!(rows, [4199]),
            12 => assert_eq!(rows, [4198]),
            14 => assert_eq!(rows, [4197]),
            _ => assert!(rows.is_empty()),
        }
        for s in &strips {
            assert_eq!(s.width as usize, WIDTH);
            let (c, y, step, inst) = if part < 11 {
                (
                    u16::from(part),
                    if s.first_row == 2090 { 300 } else { 302 },
                    2,
                    SEVIRI,
                )
            } else {
                (11, 3 * (4499 - s.first_row), 3, HRV)
            };
            assert_eq!(s.instrument, inst);
            for col in [0usize, 1, 958, 1916] {
                let x = (step * (WIDTH - 1 - col)) as u32;
                assert_eq!(
                    u16::from(s.samples[col]),
                    test_value(c, x, y) >> 2,
                    "part {part} col {col}"
                );
            }
        }
    }
    const { assert!(2 * (WIDTH - 1) < LINE - 1 && 3 * (WIDTH - 1) < HRV_LINE - 1) };
}

#[test]
fn a_new_disc_starts_when_the_line_falls_by_more_than_1000() {
    let mut d = SeviriDecoder::default();
    let mut strips = Vec::new();
    d.push(
        323,
        0,
        &test_user(323, 0, 4470, 400, test_value),
        &mut strips,
    );
    d.push(
        323,
        0,
        &test_user(323, 0, 3500, 400, test_value),
        &mut strips,
    );
    assert_eq!(d.stats.discs, 0);
    d.push(323, 0, &test_user(323, 0, 10, 400, test_value), &mut strips);
    assert_eq!(d.stats.discs, 1);
    assert!(d.disc_done);
    // The packet that started the new disc is stored in it.
    assert_eq!(d.stats.lines, 2);
    assert_eq!(strips.last().unwrap().picture, 1);
    // A satellite without clock numbers gives no row.
    let mut d = SeviriDecoder::default();
    d.push(321, 0, &test_user(323, 0, 10, 400, test_value), &mut strips);
    d.push(400, 0, &test_user(323, 0, 10, 400, test_value), &mut strips);
    assert_eq!((d.stats.rows, d.stats.refused), (0, 2));
}

fn go(h: &Harness, x: &[Complex32]) -> Ran {
    run::<Strip>(h, x, 6e6, 1686.83e6, 1686.83e6, 65_536, Some("frames.out"))
}

fn check(r: &Ran) {
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert_eq!(r.num("satellite_number"), 323.0);
    assert!(r.num("packets") >= 32.0, "{}", r.num("packets"));
    assert!(r.num("lines") >= 2.0, "{}", r.num("lines"));
    assert!(r.num("frames_bad") * 50.0 <= r.num("frames_ok"));
    assert!(r.num("channel_rows") >= 22.0, "{}", r.num("channel_rows"));
    assert!(r.num("hrv_rows") >= 1.0);
    assert!(r.num("rows") > 0.0);
    assert_eq!(r.num("bad_strips"), 0.0);
    for s in r.of::<Strip>() {
        let (c, step, y) = if s.instrument == SEVIRI {
            (s.channel - 1, 2, 2 * (2240 - s.first_row))
        } else {
            (11, 3, 3 * (4499 - s.first_row))
        };
        for col in 0..WIDTH {
            let x = (step * (WIDTH - 1 - col)) as u32;
            assert_eq!(
                u16::from(s.samples[col]),
                test_value(c, x, y % 400) >> 2,
                "{} channel {} row {}",
                s.instrument,
                s.channel,
                s.first_row
            );
        }
    }
}

#[test]
fn the_receiver_draws_exact_rows_on_a_clean_signal() {
    let h = impaired(
        Harness::new(&DESCRIPTOR).rate(6e6).seconds(2.5),
        Some(30.0),
        0.0,
        0.0,
    );
    let x = h.generate().expect("a signal");
    let r = go(&h, &x);
    check(&r);
    assert_declared(&r, &sdrkit_stack_satimg::typed::shape("msg_raw").results_decl());
}

#[test]
fn the_receiver_draws_exact_rows_at_6_db() {
    let h = impaired(
        Harness::new(&DESCRIPTOR).rate(6e6).seconds(2.5),
        Some(6.0),
        40e3,
        20.0,
    );
    let x = h.generate().expect("a signal");
    check(&go(&h, &x));
}

#[test]
fn a_signal_across_the_end_of_a_disc_delivers_it() {
    let h = impaired(
        Harness::new(&DESCRIPTOR).rate(6e6).seconds(2.5),
        Some(30.0),
        0.0,
        0.0,
    )
    .set(&GEN_START, 4470);
    let x = h.generate().expect("a signal");
    let r = go(&h, &x);
    assert_eq!(r.num("discs"), 1.0);
    assert!(r.num("rows") > 0.0);
    assert_eq!(r.num("bad_strips"), 0.0);
}

#[test]
fn noise_gives_no_frame() {
    let h = impaired(
        Harness::new(&DESCRIPTOR).rate(6e6).seconds(3.0),
        Some(-30.0),
        0.0,
        0.0,
    );
    let x = h.generate().expect("a signal");
    let r = go(&h, &x);
    assert_ne!(r.results.status().lock_state, LockState::Locked);
    assert_eq!(r.num("frames_ok"), 0.0);
    assert!(r.of::<Strip>().is_empty());
}
