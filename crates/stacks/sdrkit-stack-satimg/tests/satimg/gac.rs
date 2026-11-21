//! NOAA GAC: the sequence, the frame layout and the receiver on its test signal.

use super::common::{assert_declared, impaired, run, Ran};
use sdrkit_algo_sim::chain::GEN_DRIFT_HZ_S;
use sdrkit_codec_image::strip::Strip;
use sdrkit_core::{Complex32, LockState};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satimg::gen::test_value;
use sdrkit_stack_satimg::instruments::gac::{
    frame_code, pack_words, scramble_mask, sequence, spacecraft, test_words, tip_frames, word,
    BIT_RATE, FRAME_BITS, FRAME_BYTES, SYNC_WORDS, WIDTH, WORDS,
};
use sdrkit_stack_satimg::modes::gac::DESCRIPTOR;

#[test]
fn the_sequence_is_the_inverted_generator_output() {
    let s = sequence();
    assert_eq!(s.len(), 1023);
    assert_eq!(s[..11], [0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 0]);
    let bytes: Vec<u8> = s[..64]
        .chunks(8)
        .map(|c| c.iter().fold(0u8, |v, &b| (v << 1) | b))
        .collect();
    assert_eq!(bytes, [0x07, 0x40, 0xC9, 0x29, 0x42, 0x5E, 0xEE, 0xA8]);
    assert_eq!(s.iter().filter(|&&b| b == 1).count(), 511);
    assert_eq!(s[1022], 0);
}

#[test]
fn the_frame_starts_with_the_sync_and_identification_words() {
    assert_eq!((WORDS, FRAME_BITS, FRAME_BYTES), (3327, 33_270, 4159));
    assert!((BIT_RATE / FRAME_BITS as f64 - 80.0).abs() < 1e-9);
    let mut w = vec![0u16; WORDS];
    w[..6].copy_from_slice(&SYNC_WORDS);
    w[6] = 0x078;
    let f = pack_words(&w);
    assert_eq!(
        f[..12],
        [0xA1, 0x16, 0xFD, 0x71, 0x9D, 0x83, 0xC9, 0x51, 0xE0, 0x00, 0x00, 0x00]
    );
    let m = scramble_mask();
    let s: Vec<u8> = f.iter().zip(&m).map(|(a, b)| a ^ b).collect();
    assert_eq!(
        s[..12],
        [0xA1, 0x16, 0xFD, 0x71, 0x9D, 0x83, 0xC9, 0x51, 0x94, 0x0C, 0x92, 0x94]
    );
    assert_eq!((frame_code(0x078), spacecraft(0x078)), (0, 15));
    assert_eq!(word(&f, 6), 0x078);
}

#[test]
fn the_avhrr_words_follow_the_tip_frames() {
    let tip = [[0u8; 104]; 5];
    let w = test_words(15, 7, &tip, test_value);
    let f = pack_words(&w);
    // Word 1182 starts at bit 4 of byte 1477.
    assert_eq!((10 * 1182) / 8, 1477);
    assert_eq!(f[1477..1484], [0x06, 0xAE, 0x0C, 0x9B, 0x6C, 0xEC, 0xBD]);
    let m = scramble_mask();
    let s: Vec<u8> = (1477..1484).map(|i| f[i] ^ m[i]).collect();
    assert_eq!(s, [0x26, 0x2D, 0x09, 0x0E, 0x02, 0xE1, 0xA2]);
    let px0: Vec<u16> = (0..5).map(|c| w[1182 + c]).collect();
    assert_eq!(px0, [427, 524, 621, 718, 815]);
    assert_eq!(
        px0.iter().map(|v| v >> 2).collect::<Vec<_>>(),
        [106, 131, 155, 179, 203]
    );
    let last: Vec<u16> = (0..5).map(|c| w[1182 + 5 * (WIDTH - 1) + c] >> 2).collect();
    assert_eq!(last, [104, 129, 153, 177, 201]);
    // The five TIP frames: frame 0 byte 0 is word 103, frame 4 byte 103 is word 622.
    let words: Vec<u16> = (0..WORDS as u32).map(|i| ((i * 7) % 1024) as u16).collect();
    let t = tip_frames(&words);
    assert_eq!(t[0][0], (words[103] >> 2) as u8);
    assert_eq!(t[4][103], (words[622] >> 2) as u8);
}

fn go(h: &Harness, x: &[Complex32]) -> Ran {
    run::<Strip>(h, x, 8e6, 1702.5e6, 1702.5e6, 65_536, Some("gac.out"))
}

#[test]
fn the_receiver_draws_lines_through_offset_drift_and_clock_error() {
    let h = impaired(
        Harness::new(&DESCRIPTOR).rate(8e6).seconds(1.5),
        Some(18.0),
        -30e3,
        30.0,
    )
    .set(&GEN_DRIFT_HZ_S, 500.0);
    let x = h.generate().expect("a signal");
    let r = go(&h, &x);
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(r.num("frames_ok") >= 80.0, "{}", r.num("frames_ok"));
    assert!(r.num("lines") >= 80.0, "{}", r.num("lines"));
    assert_eq!(r.num("spacecraft"), 15.0);
    assert_eq!(r.num("id_code"), 0.0);
    assert!(r.num("tip_frames") >= 400.0);
    let strips = r.of::<Strip>();
    let ch2: Vec<&Strip> = strips
        .iter()
        .filter(|s| s.instrument == "AVHRR GAC" && s.channel == 2)
        .collect();
    let first = ch2[0];
    let near = |s: &Strip, y: u32| {
        (0..WIDTH)
            .filter(|&x| {
                (i32::from(s.samples[x]) - i32::from(test_value(1, x as u32, y) >> 2)).abs() <= 3
            })
            .count()
    };
    let offset = (0..400u32)
        .find(|&y| near(first, y) > WIDTH * 9 / 10)
        .expect("the first row is a line of the test picture");
    let (mut good, mut all) = (0, 0);
    for s in &ch2 {
        good += near(s, (s.first_row - first.first_row + offset) % 400);
        all += WIDTH;
    }
    assert!(good * 100 >= all * 95, "{good} of {all}");
    assert!(r.num("rows") > 0.0);
    assert_eq!(r.num("bad_strips"), 0.0);
    assert_declared(&r, &sdrkit_stack_satimg::typed::shape("noaa_gac").results_decl());
}

#[test]
fn noise_gives_no_frame() {
    let h = impaired(
        Harness::new(&DESCRIPTOR).rate(8e6).seconds(2.0),
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
