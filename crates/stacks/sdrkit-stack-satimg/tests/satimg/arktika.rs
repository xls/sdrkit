//! Arktika-M GGAK, the submode `arktika_ggak` of `elektro_l`: the test frame, and the receiver on its test signal.

use super::common::{assert_declared, impaired, run, set, Ran};
use sdrkit_core::{LockState, ResultsDecl};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satimg::instruments::ggak::parse;
use sdrkit_stack_satimg::typed::{GgakFrameEvent, GGAK_EVENT};
use sdrkit_stack_satimg::modes::xrit::{elektro_declaration, ARKTIKA_SUBMODE, ELEKTRO};
use sdrkit_stack_satimg::modes::xrit_gen::{arktika_frame, ARKTIKA_FIELD};

#[test]
fn the_test_frame_counts_its_marker_and_carries_the_field() {
    for k in 0..16u64 {
        let f = arktika_frame(k);
        assert_eq!(f.len(), 224);
        let p = parse(&f, false, 0.0).expect("a frame");
        assert!(p.crc_ok);
        assert_eq!(p.marker, (k % 8) as u8);
        assert_eq!(p.subframe, if k % 8 == 7 { 7 } else { 0 });
        assert_eq!(p.timestamp, k);
        assert!(p.field.iter().all(|t| *t == ARKTIKA_FIELD));
        assert!(f[91..222].iter().all(|&b| b == 0xAA));
    }
}

fn go(rate: f64, secs: f64, esn0: f64, offset: f64, ppm: f64) -> Ran {
    let h = impaired(
        Harness::new(&ELEKTRO)
            .submode(ARKTIKA_SUBMODE)
            .rate(rate)
            .seconds(secs),
        Some(esn0),
        offset,
        ppm,
    );
    let x = h.generate().expect("a signal");
    run::<sdrkit_stack_satimg::items::GgakFrame>(&h, &x, rate, 1703e6, 1703e6, 24_000, None)
}

#[test]
fn frames_at_240_ksps_3_khz_off() {
    let r = go(240e3, 10.0, 20.0, 3000.0, 0.0);
    assert!(r.num("ggak_good") >= 8.0, "{}", r.num("ggak_good"));
    assert_eq!(r.num("ggak_bad"), 0.0);
    let off = r.results.status().offset_hz.expect("an offset");
    assert!((off - 3000.0).abs() < 50.0, "{off}");
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    let s = &r.results.status().summary;
    assert!(s.starts_with("Arktika-M GGAK: "), "{s}");
    assert!(s.contains(" frames (0 bad), marker "), "{s}");
    let e = r
        .events
        .iter()
        .find(|e| e.is(&GGAK_EVENT))
        .expect("a GGAK frame");
    assert_eq!(
        e.at(&GgakFrameEvent::BY),
        Some(i64::from(ARKTIKA_FIELD[1]))
    );
    assert!(r
        .text("note")
        .to_lowercase()
        .contains("not confirmed by a public document"));
    let decl: ResultsDecl = elektro_declaration();
    assert_declared(&r, &decl);
}

#[test]
fn frames_at_1024_ksps_at_10_db_with_a_clock_error() {
    let r = go(1.024e6, 10.0, 10.0, -2000.0, -50.0);
    assert!(r.num("ggak_good") >= 8.0, "{}", r.num("ggak_good"));
}

#[test]
fn the_link_setting_does_not_leave_ggak() {
    let h = Harness::new(&ELEKTRO).submode(ARKTIKA_SUBMODE).set(
        &sdrkit_stack_satimg::modes::xrit::LINK_SETTING,
        sdrkit_stack_satimg::modes::xrit::ElektroLink::Hrit665,
    );
    let mut rx = h.receiver().expect("the receiver");
    set(&mut *rx, "link", "lrit64").expect("the setting is taken");
    set(&mut *rx, "craft", "n4").expect("the setting is taken");
    let h = impaired(
        Harness::new(&ELEKTRO)
            .submode(ARKTIKA_SUBMODE)
            .rate(240e3)
            .seconds(4.0),
        Some(20.0),
        0.0,
        0.0,
    );
    let x = h.generate().expect("a signal");
    let r = super::common::feed::<sdrkit_stack_satimg::items::GgakFrame>(
        {
            rx.configure(&sdrkit_core::InputInfo::new(240e3, 1703e6, 1703e6).recording())
                .expect("the rate");
            rx
        },
        &x,
        24_000,
        None,
    );
    assert_eq!(r.text("link"), "GGAK 5 ksym/s");
    assert!(r.num("ggak_good") >= 2.0, "{}", r.num("ggak_good"));
}

#[test]
fn noise_gives_no_frame() {
    let r = go(240e3, 6.0, -30.0, 0.0, 0.0);
    assert_eq!(r.num("ggak_good"), 0.0);
    assert_ne!(r.results.status().lock_state, LockState::Locked);
}
