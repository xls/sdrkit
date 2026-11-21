//! MT63: every width and interleave, a wrong interleave, flipped soft values, and the recording.

use crate::common::*;
use sdrkit_core::Choice;
use sdrkit_dev_test::Run;
use sdrkit_stack_hfkbd as hf;

/// The interleave in use, as its key ("long").
fn interleave(r: &Run) -> &'static str {
    let i = r
        .results
        .fields()
        .at(&hf::typed::Mt63Fields::INTERLEAVE)
        .expect("an interleave");
    hf::mt63::InterleaveChoice::CHOICES[usize::from(i.index())]
        .key
        .as_ref()
}

#[test]
fn mt63_widths_and_interleaves() {
    for (w, il, off) in [
        ("1000", "long", 80.0),
        ("500", "short", -50.0),
        ("2000", "long", 0.0),
    ] {
        let h = snr_2500(harness(&hf::MT63), 10.0)
            .set(&hf::decoder::GEN_MESSAGE, hf::mt63::TEST_TEXT)
            .set(
                &hf::mt63::BANDWIDTH,
                hf::mt63::WidthChoice::from_key(w).unwrap(),
            )
            .set(
                &hf::mt63::INTERLEAVE_SETTING,
                hf::mt63::InterleaveChoice::from_key(il).unwrap(),
            )
            .set(&GEN_OFFSET_HZ, off)
            .set(&GEN_CLOCK_PPM, 150.0)
            .seconds(30.0);
        // The receiver finds the width and the interleave by itself.
        let x = h.generate().unwrap();
        let r = feed(&harness(&hf::MT63).seconds(30.0), &x);
        let t = text(&r);
        assert!(t.contains("DE K0TST K0TST MT63 TEST"), "{w} {il}: {t:?}");
        assert_eq!(interleave(&r), il);
        assert_eq!(num(&r, "bandwidth_hz").to_string(), w);
    }
}

#[test]
fn mt63_forced_wrong_interleave_decodes_nothing() {
    let x = snr_2500(harness(&hf::MT63), 20.0)
        .set(
            &hf::mt63::INTERLEAVE_SETTING,
            hf::mt63::InterleaveChoice::Long,
        )
        .seconds(20.0)
        .generate()
        .unwrap();
    let r = feed(
        &harness(&hf::MT63).set(
            &hf::mt63::INTERLEAVE_SETTING,
            hf::mt63::InterleaveChoice::Short,
        ),
        &x,
    );
    assert!(!text(&r).contains("K0TST"), "{:?}", text(&r));
}

#[test]
fn mt63_survives_flipped_soft_values() {
    // With 15 % of the soft values flipped no character of 400 is lost.
    let mut work = [0.0; hf::mt63::CARRIERS];
    let mut rng = sdrkit_algo_sim::rng::Rng::new(17);
    for k in 0..400u32 {
        let code = (k * 37 % 128) as u8;
        let mut c = hf::mt63::chips(code);
        for v in c.iter_mut() {
            if rng.below(100) < 15 {
                *v = -*v;
            }
        }
        assert_eq!(hf::mt63::decode(&c, &mut work).0, code);
    }
}

#[test]
fn mt63_recording() {
    let name = "hfkbd_mt63_1000_12k";
    let e = expected(name);
    for piece in [800, 4093] {
        let r = recording(&harness(&hf::MT63).piece(piece), name);
        let t = text(&r);
        assert!(t.contains(e["expect"]["contains"].as_str().unwrap()), "{t}");
        assert_eq!(num(&r, "bandwidth_hz"), 1000.0);
        assert_eq!(interleave(&r), "short");
        assert!(
            in_range(num(&r, "centre_hz"), &e["expect"]["centre_hz"]),
            "{}",
            num(&r, "centre_hz")
        );
    }
}
