//! The keyboard MFSK stacks: MFSK with its pictures, and the recordings.

use crate::common::*;
use sdrkit_core::verify::CaseSetting;
use sdrkit_stack_hfkbd as hf;

#[test]
fn mfsk_variants_and_impairments() {
    for (v, off, ppm, snr) in [
        ("MFSK16", 8.0, 200.0, 5.0),
        ("MFSK8", 0.0, 0.0, 20.0),
        ("MFSK31", 0.0, 0.0, 20.0),
        ("MFSK64", 0.0, -100.0, 15.0),
        ("MFSK22", 0.0, 0.0, 20.0),
    ] {
        let h = snr_2500(harness(&hf::MFSK), snr)
            .with_settings(&[pick(&hf::mfsk::VARIANT_SETTING, v)])
            .set(&GEN_OFFSET_HZ, off)
            .set(&GEN_CLOCK_PPM, ppm)
            .seconds(40.0);
        let r = run(&h);
        let t = text(&r);
        assert!(t.contains("K0TST TEST MESSAGE 0123456789"), "{v}: {t:?}");
        assert_eq!(text_field(&r, "callsign"), "K0TST");
    }
}

/// A 40 x 30 picture at 15 dB comes back with a mean error under 25 levels.
#[test]
fn mfsk_picture_round_trip() {
    for seed in [2, 3, 4] {
        picture_round_trip(seed);
    }
}

fn picture_round_trip(seed: u64) {
    let h = snr_2500(harness(&hf::MFSK), 15.0)
        .with_settings(&[pick(&hf::mfsk::VARIANT_SETTING, "MFSK32")])
        .set(&hf::mfsk::GEN_PICTURE_WIDTH, 40)
        .set(&hf::mfsk::GEN_PICTURE_HEIGHT, 30)
        .seed(seed)
        .seconds(40.0);
    let x = h.generate().unwrap();
    let (mut rx, mut tap) = receiver_with_pictures(&h, RATE);
    feed_rx(rx.as_mut(), &x, 800);
    let t = field_text(rx.results(), "text");
    assert!(t.contains("Pic:40x30;"), "{t:?}");
    let pics = pictures(&mut tap);
    let p = pics.last().expect("a picture");
    let sent = hf::picture::test_grey(40, 30);
    let mut err = 0.0;
    for y in 0..30 {
        for x in 0..40 {
            err += (grey(p, x, y) as f64 - sent[(y * 40 + x) as usize] as f64).abs();
        }
    }
    assert!(err / 1200.0 < 25.0, "{}", err / 1200.0);
    // The picture item is a PNG the shared encoder writes and reads back the same.
    let png = sdrkit_codec_image::png::encode(p).unwrap();
    assert_eq!(
        sdrkit_codec_image::png::decode(&png).unwrap().data(),
        p.data()
    );
}

#[test]
fn mfsk16_recording() {
    keyboard_check(
        &hf::MFSK,
        "hfkbd_mfsk16_12k",
        &[
            pick(&hf::mfsk::VARIANT_SETTING, "MFSK16"),
            CaseSetting::new(&hf::mfsk::CENTRE, 1500.0),
        ],
    );
}

#[test]
fn mfsk32_picture_recording() {
    let name = "hfkbd_mfsk32_picture_12k";
    let (w, h, sent) = read_png("mfsk32_picture_sent.png");
    for piece in [800, 4093] {
        let hh = harness(&hf::MFSK)
            .with_settings(&[pick(&hf::mfsk::VARIANT_SETTING, "MFSK32")])
            .set(&hf::mfsk::CENTRE, 1500.0);
        let rec = sdrkit_dev_test::read_recording(&format!("hfkbd/{name}")).expect("the recording");
        let (mut rx, mut tap) = receiver_with_pictures(&hh, rec.rate_hz);
        feed_rx(rx.as_mut(), &rec.samples, piece);
        let res = rx.results();
        let t = field_text(res, "text");
        assert!(t.contains("Pic:40x30;"));
        assert!(t.contains("PICTURE TEST K CQ CQ DE K0TST"), "{t:?}");
        assert_eq!(Some(field_num(res, "pictures")), Some(1.0));
        let pics = pictures(&mut tap);
        let p = pics.last().expect("a finished picture");
        assert_eq!((p.width(), p.height()), (w, h));
        let mut err = 0.0;
        for y in 0..h {
            for x in 0..w {
                let s = sent[((y * w + x) * 3) as usize] as f64;
                err += (grey(p, x, y) as f64 - s).abs();
            }
        }
        assert!(err / (w * h) as f64 <= 25.0);
    }
}

#[test]
fn dominoex_and_thor() {
    for (desc, v, snr) in [
        (&hf::DOMINOEX, "DominoEX 11", 0.0),
        (&hf::DOMINOEX, "DominoEX 22", 10.0),
        (&hf::THOR, "THOR 16", 0.0),
        (&hf::THOR, "THOR 100", 10.0),
        (&hf::THOR, "THOR 25x4", 10.0),
    ] {
        let variant = if desc.id == "thor" {
            &hf::domino::THOR_VARIANT
        } else {
            &hf::domino::DOMINO_VARIANT
        };
        let h = snr_2500(harness(desc), snr)
            .with_settings(&[pick(variant, v)])
            .set(&GEN_OFFSET_HZ, 37.0)
            .set(&GEN_CLOCK_PPM, 200.0)
            .seconds(45.0);
        let r = run(&h);
        let t = text(&r);
        assert!(t.contains("K0TST TEST MESSAGE 0123456789"), "{v}: {t:?}");
    }
}

#[test]
fn thor16_recording() {
    keyboard_check(
        &hf::THOR,
        "hfkbd_thor16_12k",
        &[
            pick(&hf::domino::THOR_VARIANT, "THOR 16"),
            CaseSetting::new(&hf::mfsk::CENTRE, 1500.0),
        ],
    );
}

#[test]
fn dominoex11_recording() {
    keyboard_check(
        &hf::DOMINOEX,
        "hfkbd_dominoex11_12k",
        &[
            pick(&hf::domino::DOMINO_VARIANT, "DominoEX 11"),
            CaseSetting::new(&hf::mfsk::CENTRE, 1500.0),
        ],
    );
}

#[test]
fn olivia_and_contestia() {
    for (desc, t, b, snr) in [
        (&hf::OLIVIA, "32", "1000", -6.0),
        (&hf::CONTESTIA, "16", "500", 0.0),
        (&hf::OLIVIA, "8", "250", 10.0),
    ] {
        let h = snr_2500(harness(desc), snr)
            .set(&hf::olivia::TONES, hf::olivia::Tones::from_key(t).unwrap())
            .set(
                &hf::olivia::BANDWIDTH,
                hf::olivia::Bandwidth::from_key(b).unwrap(),
            )
            .set(&GEN_OFFSET_HZ, 20.0)
            .set(&GEN_CLOCK_PPM, 100.0)
            .seconds(45.0);
        let r = run(&h);
        let tx = text(&r);
        assert!(tx.contains("DE K0TST K0TST TEST"), "{t}/{b}: {tx:?}");
        assert_eq!(
            r.results.status().lock_state,
            sdrkit_core::LockState::Locked
        );
    }
}

#[test]
fn olivia_noise_never_locks() {
    let r = feed(&harness(&hf::OLIVIA), &noise_only(40.0, 8));
    assert_eq!(text(&r), "");
}

#[test]
fn olivia_recording() {
    let name = "hfkbd_olivia_32_1000_12k";
    let r = keyboard_check(
        &hf::OLIVIA,
        name,
        &[
            CaseSetting::new(&hf::olivia::TONES, hf::olivia::Tones::T32),
            CaseSetting::new(&hf::olivia::BANDWIDTH, hf::olivia::Bandwidth::B1000),
            CaseSetting::new(&hf::mfsk::CENTRE, 1500.0),
        ],
    );
    let e = expected(name);
    assert!(in_range(num(&r, "offset_hz"), &e["expect"]["offset_hz"]));
    assert!(
        in_range(num(&r, "snr_db"), &e["expect"]["snr_db"]),
        "{}",
        num(&r, "snr_db")
    );
}

#[test]
fn contestia_recording() {
    keyboard_check(
        &hf::CONTESTIA,
        "hfkbd_contestia_16_500_12k",
        &[
            CaseSetting::new(&hf::olivia::TONES, hf::olivia::Tones::T16),
            CaseSetting::new(&hf::olivia::BANDWIDTH, hf::olivia::Bandwidth::B500),
            CaseSetting::new(&hf::mfsk::CENTRE, 1200.0),
        ],
    );
}
