//! Meteor HRPT (`hrpt`, submode `meteor`): the transport and MSU-MR frames, and the receiver on its test signal.

use super::common::{assert_declared, impaired, run, Ran};
use sdrkit_algo_sim::chain::GEN_DRIFT_HZ_S;
use sdrkit_codec_image::strip::Strip;
use sdrkit_core::{Complex32, LockState};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satimg::gen::test_value;
use sdrkit_stack_satimg::instruments::msumr_hrpt::{
    payload, pixel, test_line, transport_frame, MsuHrptDecoder, FRAME_BYTES, PAYLOAD_BYTES,
    TRANSPORT_BYTES, WIDTH,
};
use sdrkit_stack_satimg::modes::hrpt::DESCRIPTOR;
use sdrkit_stack_satimg::modes::hrpt_meteor::{declaration, BIT_RATE};

#[test]
fn the_transport_frame_has_948_payload_bytes() {
    assert_eq!(TRANSPORT_BYTES, 1024);
    let piece: Vec<u8> = (0..PAYLOAD_BYTES).map(|i| i as u8).collect();
    let f = transport_frame(0x01_0203, &piece);
    assert_eq!(f[..7], [0x1A, 0xCF, 0xFC, 0x1D, 1, 2, 3]);
    assert_eq!(
        (f[22], f[259], f[260], f[277], f[278]),
        (0, 237, 0x55, 0x55, 238)
    );
    assert_eq!((f[790], f[1023]), ((714 % 256) as u8, (947 % 256) as u8));
    let mut p = Vec::new();
    payload(&f, &mut p);
    assert_eq!(p, piece);
    assert!((BIT_RATE / 8192.0 - 81.2).abs() < 0.05);
}

#[test]
fn the_msumr_frame_packs_four_pixels_in_five_bytes() {
    let f = test_line(3, test_value);
    assert_eq!(f.len(), FRAME_BYTES);
    assert_eq!(f[50..55], [0x2D, 0xCB, 0xC3, 0x04, 0xC6]);
    assert_eq!(f[55..60], [0x46, 0x11, 0xD4, 0x89, 0x27]);
    assert_eq!([0, 1, 2, 3].map(|x| pixel(&f, 0, x) >> 2), [45, 47, 48, 49]);
    assert_eq!([0, 1, 2, 3].map(|x| pixel(&f, 1, x) >> 2), [70, 71, 72, 73]);
    // Group 392 of channel 6 is the bytes 11 835 to 11 839.
    assert_eq!(pixel(&f, 5, 1571), test_value(5, 1571, 3));
    assert_eq!(50 + 30 * 392 + 5 * 5, 11_835);
    assert!((BIT_RATE / 8.0 / 1024.0 * 948.0 / 11_850.0 - 6.5).abs() < 0.01);
}

#[test]
fn a_line_across_13_transport_frames_decodes_exact() {
    // Five bits of junk put the sync word of the first line at bit 5 of a payload byte.
    let mut bits: Vec<u8> = vec![1, 0, 1, 1, 0];
    for y in 0..3 {
        for b in test_line(y, test_value) {
            bits.extend((0..8).rev().map(|k| (b >> k) & 1));
        }
    }
    let bytes: Vec<u8> = bits
        .chunks(8)
        .map(|c| c.iter().fold(0u8, |v, &b| (v << 1) | b) << (8 - c.len()))
        .collect();
    let mut d = MsuHrptDecoder::default();
    let mut strips = Vec::new();
    let mut frames = 0;
    for piece in bytes.chunks(PAYLOAD_BYTES) {
        let f = transport_frame(frames, piece);
        frames += 1;
        let mut p = Vec::new();
        payload(&f, &mut p);
        d.push(&p, &mut strips);
    }
    // Each line spans 13 transport frames. The first line is held until the second sync word confirms the lock.
    assert!(frames >= 13 * 2);
    assert_eq!(d.stats.rows, 3);
    for s in &strips {
        let y = s.first_row;
        for x in 0..WIDTH {
            let want = test_value(s.channel - 1, x as u32, y) >> 2;
            assert_eq!(u16::from(s.samples[x]), want);
        }
    }
}

fn go(h: &Harness, x: &[Complex32]) -> Ran {
    run::<Strip>(h, x, 4e6, 1700e6, 1700e6, 65_536, Some("msumr.out"))
}

#[test]
fn the_receiver_draws_lines_through_offset_drift_and_clock_error() {
    let h = impaired(
        Harness::new(&DESCRIPTOR)
            .submode("meteor")
            .rate(4e6)
            .seconds(2.5),
        Some(20.0),
        22e3,
        40.0,
    )
    .set(&GEN_DRIFT_HZ_S, -600.0);
    let x = h.generate().expect("a signal");
    let r = go(&h, &x);
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(r.num("frames_ok") >= 150.0, "{}", r.num("frames_ok"));
    assert!(r.num("lines") >= 3.0, "{}", r.num("lines"));
    // Channel 3 within 3 counts in 95 % of the pixels, the rows put at one offset from the test lines.
    let strips: Vec<Strip> = r.of::<Strip>();
    let ch3: Vec<&Strip> = strips.iter().filter(|s| s.channel == 3).collect();
    assert!(ch3.len() >= 3);
    let first = ch3[0];
    let offset = (0..60u32)
        .find(|&y| {
            (0..WIDTH)
                .filter(|&x| {
                    (i32::from(first.samples[x]) - i32::from(test_value(2, x as u32, y) >> 2)).abs()
                        <= 3
                })
                .count()
                > WIDTH * 9 / 10
        })
        .expect("the first row is a line of the test picture");
    let (mut good, mut all) = (0, 0);
    for s in &ch3 {
        let y = (s.first_row - first.first_row + offset) % 60;
        for x in 0..WIDTH {
            all += 1;
            good += usize::from(
                (i32::from(s.samples[x]) - i32::from(test_value(2, x as u32, y) >> 2)).abs() <= 3,
            );
        }
    }
    assert!(good * 100 >= all * 95, "{good} of {all}");
    assert!(r.num("rows") > 0.0);
    assert_eq!(r.num("bad_strips"), 0.0);
    assert_declared(&r, &declaration());
}

#[test]
fn noise_gives_no_frame() {
    let h = impaired(
        Harness::new(&DESCRIPTOR)
            .submode("meteor")
            .rate(4e6)
            .seconds(3.0),
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
