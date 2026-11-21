//! RTTY: round trips at every speed and shift, noise alone, the radio's tuning offset and the recording.

use crate::common::*;
use sdrkit_core::InputInfo;
use sdrkit_stack_hfkbd as hf;

const SENT: &str = "RYRYRY CQ DE K0TST TEST 0123456789 1/2 -.:? THE QUICK BROWN FOX";

#[test]
fn rtty_round_trips() {
    for (baud, shift, off, rev) in [
        ("45.45", "170", 100.0, false),
        ("50", "450", -100.0, false),
        ("45.45", "170", 0.0, true),
        ("75", "850", 0.0, false),
    ] {
        let h = snr_2500(harness(&hf::RTTY), 10.0)
            .set(&hf::rtty::BAUD, hf::rtty::Baud::from_key(baud).unwrap())
            .set(&hf::rtty::SHIFT, hf::rtty::Shift::from_key(shift).unwrap())
            .set(&hf::rtty::REVERSED, rev)
            .set(&GEN_OFFSET_HZ, off)
            .seconds(30.0);
        let r = run(&h);
        let t = text(&r);
        assert!(t.contains(SENT), "{baud}/{shift}: {t:?}");
        let shift_hz: f64 = shift.parse().unwrap();
        let want = 2125.0 + off + if rev { 0.0 } else { shift_hz };
        let mark = num(&r, "mark_hz");
        assert!((mark - want).abs() < 8.0, "{mark} vs {want}");
        assert!(num(&r, "framing_errors") <= 6.0);
        assert!(num(&r, "quality_pct") > 20.0);
    }
}

#[test]
fn rtty_noise_returns_nothing() {
    let r = feed(&harness(&hf::RTTY), &noise_only(30.0, 5));
    assert_eq!(text(&r), "");
    assert_eq!(
        r.results.status().lock_state,
        sdrkit_core::LockState::Searching
    );
}

#[test]
fn rtty_through_the_radio_offset() {
    // The radio is tuned 20 kHz above the dial at 96 kHz: the front end moves the dial to 0 Hz.
    let rate = 96000.0;
    let h = snr_2500(harness(&hf::RTTY), 20.0).rate(rate).seconds(20.0);
    let mut x = h.generate().unwrap();
    sdrkit_algo_sim::offset::FrequencyOffset::new(rate, -20000.0, 0.0)
        .unwrap()
        .process(&mut x);
    let mut rx = h.receiver().unwrap();
    let dial = 14.08e6;
    rx.configure(&InputInfo::new(rate, dial + 20000.0, dial))
        .unwrap();
    let mut out = sdrkit_core::Output::default();
    for c in x.chunks(9600) {
        rx.feed(c, &mut out);
    }
    let res = rx.results();
    let t = field_text(res, "text");
    assert!(t.contains("CQ DE K0TST TEST"), "{t}");
    let f = |k: &str| field_num(res, k);
    // The mark is the upper tone: the higher radio frequency.
    assert!((f("mark_hz") - 2295.0).abs() < 10.0);
    assert!((f("space_hz") - 2125.0).abs() < 10.0);
}

#[test]
fn rtty_recording() {
    let name = "hfkbd_rtty_45_170_12k";
    let e = expected(name);
    for piece in [800, 4093] {
        // The recording has the mark on the lower tone, the reverse of the mark as the higher radio frequency.
        let r = recording(
            &harness(&hf::RTTY)
                .piece(piece)
                .set(&hf::rtty::REVERSED, true),
            name,
        );
        let t = text(&r);
        assert!(
            t.contains(e["expect"]["contains"].as_str().unwrap()),
            "{t:?}"
        );
        assert!(in_range(num(&r, "mark_hz"), &e["expect"]["mark_hz"]));
        assert!(in_range(num(&r, "space_hz"), &e["expect"]["space_hz"]));
        assert!(num(&r, "framing_errors") <= 6.0);
        assert!(num(&r, "quality_pct") >= 20.0);
        assert_eq!(
            r.results.status().lock_state,
            sdrkit_core::LockState::Locked
        );
    }
}
