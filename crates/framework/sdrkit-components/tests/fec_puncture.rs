//! `fec.puncture` and `fec.depuncture`: the DVB rate 3/4 pattern as EN 300 744 table 2 prints it, a round trip through both,
//! frame starts restarting the pattern, any cut, custom patterns and strict settings.

mod fec_support;
use fec_support::*;

use sdrkit_algo_fec::conv::{codes, depuncture_frame, puncture, Encoder};
use sdrkit_components::fec::{Depuncture, PunctureComponent};
use sdrkit_components::testing::Source;
use sdrkit_core::Value;
use sdrkit_graph::stream::Data;
use sdrkit_graph::Format;

#[test]
fn dvb_3_4_sends_x1_y1_y2_x3() {
    // EN 300 744 table 2, rate 3/4: X 101, Y 110, sent X1 Y1 Y2 X3 of X1 Y1 X2 Y2 X3 Y3.
    let coded: Vec<u8> = vec![1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 1];
    let mut g = Tb::new()
        .add(
            "src",
            Source::new(Format::hard_bits(1200.0), Data::HardBits(coded.clone()), 5),
        )
        .add("p", PunctureComponent::<u8>::default())
        .connect("src", "p")
        .set("p.pattern", txt("dvb_3_4"))
        .build()
        .unwrap();
    let got = run(&mut g, 4, &["p.out"]);
    let sent = hard_bits(&got[0].data);
    assert_eq!(
        sent,
        [coded[0], coded[1], coded[3], coded[4], coded[6], coded[7], coded[9], coded[10]]
    );
    assert_eq!(g.format("p.out").unwrap().rate, 800.0);
}

#[test]
fn puncture_then_depuncture_puts_erasures_in_the_removed_places_for_any_cut() {
    let mut rng = Rng(3);
    let bits = rng.bits(300);
    let mut coded = Vec::new();
    Encoder::new(codes::DVB_K7).encode(&bits, &mut coded);
    let llr = llr_of(&coded, 50);
    for (pattern, p) in [
        ("dvb_7_8", codes::DVB_7_8),
        ("dab_pi_9", codes::DAB_PI[8]),
        ("m17_p1", codes::M17_P1),
    ] {
        let mut want_sent = Vec::new();
        puncture(&p, 2, &llr, &mut want_sent);
        let mut want = vec![0i8; llr.len()];
        // Whole periods only, so the frame form and the stream agree on the length.
        let whole = llr.len() / p.period() * p.period();
        want.truncate(whole);
        let sent_whole = p.punctured_len(2, whole);
        depuncture_frame(&p, 2, &want_sent[..sent_whole], &mut want).unwrap();
        for per_pass in [1usize, 13, 600] {
            let mut g = Tb::new()
                .add(
                    "src",
                    Source::new(
                        Format::llr(2000.0, Some(5.0)),
                        Data::Llr(llr.clone()),
                        per_pass,
                    ),
                )
                .add("p", PunctureComponent::<i8>::default())
                .add("d", Depuncture::default())
                .connect("src", "p")
                .connect("p", "d")
                .set("p.pattern", txt(pattern))
                .set("p.phase", Value::F64(2.0))
                .set("d.pattern", txt(pattern))
                .set("d.phase", Value::F64(2.0))
                .build()
                .unwrap();
            let got = run(&mut g, llr.len() / per_pass + 2, &["p.out", "d.out"]);
            assert_eq!(
                llrs(&got[0].data),
                want_sent,
                "{pattern}, pieces of {per_pass}"
            );
            let back = llrs(&got[1].data);
            assert_eq!(&back[..whole], &want[..], "{pattern}, pieces of {per_pass}");
            assert_eq!(g.format("d.out").unwrap().llr_scale, Some(5.0));
        }
    }
}

#[test]
fn a_frame_start_restarts_the_pattern() {
    // Rate 2/3 (keep 1101): a frame of 4 sent values covers 6 places of two periods ... the second frame starts at value 5.
    let sent: Vec<i8> = (1..=10).collect();
    let mut g = Tb::new()
        .add(
            "src",
            TaggedSource::new(Format::llr(10.0, None), Data::Llr(sent), &[5], 3),
        )
        .add("d", Depuncture::default())
        .connect("src", "d")
        .set("d.pattern", txt("dvb_2_3"))
        .build()
        .unwrap();
    let got = run(&mut g, 5, &["d.out"]);
    // 1 2 _ 3 | 4 5 _ (cut by the frame start) | 6 7 _ 8 | 9 10 _
    assert_eq!(
        llrs(&got[0].data),
        [1, 2, 0, 3, 4, 5, 0, 6, 7, 0, 8, 9, 10, 0]
    );
}

#[test]
fn custom_patterns_and_strict_settings() {
    let mut g = Tb::new()
        .add(
            "src",
            Source::new(
                Format::hard_bits(10.0),
                Data::HardBits(vec![1, 2, 3, 4, 5, 6]),
                6,
            ),
        )
        .add("p", PunctureComponent::<u8>::default())
        .connect("src", "p")
        .set("p.pattern", txt("custom"))
        .set("p.keep", txt("1, 0, 1"))
        .build()
        .unwrap();
    let got = run(&mut g, 2, &["p.out"]);
    assert_eq!(hard_bits(&got[0].data), [1, 3, 4, 6]);

    let mut d = Depuncture::default();
    assert!(set(&mut d, "pattern", &Value::Text("dvb_9_10".into())).is_err());
    assert!(set(&mut d, "keep", &Value::Text("1102".into())).is_err());
    assert!(set(&mut d, "keep", &Value::Text("000".into())).is_err());
    assert!(set(&mut d, "phase", &Value::F64(-1.0)).is_err());
    assert!(set(&mut d, "phase", &Value::F64(1.5)).is_err());
    assert!(set(&mut d, "rate", &Value::F64(1.0)).is_err());
    // A phase beyond the pattern, or custom without a pattern, is refused when the graph is configured.
    for (k, v) in [
        ("d.phase", Value::F64(4.0)),
        ("d.pattern", Value::Text("custom".into())),
    ] {
        let mut g = Tb::new()
            .add(
                "src",
                Source::new(Format::llr(10.0, None), Data::Llr(vec![1]), 1),
            )
            .add("d", Depuncture::default())
            .connect("src", "d")
            .set(k, v)
            .build()
            .unwrap();
        assert!(g.configure_input(Format::iq(1.0, 0.0, 0.0)).is_err(), "{k}");
    }
}
