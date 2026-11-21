//! `fec.viterbi`: the Galileo I/NAV example of the OS SIS ICD (annex D.2) decoded as a frame, a long stream with errors decoded
//! exactly for any cut, the error count by re-encoding, frame starts, and strict settings.

mod fec_support;
use fec_support::*;

use sdrkit_algo_fec::conv::{codes, puncture, Encoder};
use sdrkit_components::fec::{Depuncture, ViterbiDecode};
use sdrkit_components::testing::Source;
use sdrkit_core::Value;
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Component, Format, TagKind};

/// Galileo OS SIS ICD issue 2.1 annex D.2: the I/NAV example input (120 bits, the last 6 the tail) and its 240 symbols.
const GAL_INAV_IN: &str =
    "11111111 11110000 11001100 10101010 00000000 00001111 00110011 01010101 \
    11100011 11101100 11011111 10001010 00011100 00010011 01000000";
const GAL_INAV_OUT: &str =
    "10001100 00011010 10101010 01110011 00110001 01011010 01101111 01011001 \
    01111000 10010101 01010101 10001100 11001110 10100101 10010000 10100110 \
    10000100 00000010 00000010 00011011 10011001 11101011 01011100 00011000 \
    10111011 11111101 11111101 11100100 01010011 00100010";

#[test]
fn galileo_inav_example_decodes_as_a_terminated_frame() {
    let bits = bits_of(GAL_INAV_IN);
    let symbols = bits_of(GAL_INAV_OUT);
    assert_eq!((bits.len(), symbols.len()), (120, 240));
    let mut llr = llr_of(&symbols, 30);
    let flips: Vec<usize> = (5..llr.len()).step_by(41).collect();
    for &i in &flips {
        llr[i] = -llr[i];
    }
    // Two frames back to back.
    let mut both = llr.clone();
    both.extend_from_slice(&llr);
    for per_pass in [1usize, 37, 480] {
        let mut g = Tb::new()
            .add(
                "src",
                Source::new(Format::llr(4.0, None), Data::Llr(both.clone()), per_pass),
            )
            .add("v", ViterbiDecode::default())
            .connect("src", "v")
            .set("v.code", txt("galileo_k7"))
            .set("v.ends", txt("zero"))
            .set("v.frame", Value::F64(120.0))
            .build()
            .unwrap();
        let got = run(&mut g, both.len() / per_pass + 2, &["v.out"]);
        let out = hard_bits(&got[0].data);
        let mut want = bits[..114].to_vec();
        want.extend_from_slice(&bits[..114]);
        assert_eq!(out, want, "pieces of {per_pass}");
        // Each frame's first bit is marked.
        let starts: Vec<u64> = got[0]
            .tags
            .iter()
            .filter(|t| matches!(t.kind, TagKind::FrameStart { .. }))
            .map(|t| t.at)
            .collect();
        assert_eq!(starts, [0, 114]);
        assert_eq!(field(&g, "v", "bits_corrected"), 2.0 * flips.len() as f64);
        assert_eq!(g.format("v.out").unwrap().rate, 4.0 / 2.0 * 114.0 / 120.0);
    }
}

#[test]
fn a_punctured_stream_decodes_exactly_for_any_cut() {
    let mut rng = Rng(21);
    let bits = rng.bits(20_000);
    let mut coded = Vec::new();
    Encoder::new(codes::DVB_K7).encode(&bits, &mut coded);
    let mut sent = Vec::new();
    puncture(&codes::DVB_3_4, 0, &coded, &mut sent);
    let mut llr = llr_of(&sent, 40);
    let flips = rng.positions(llr.len(), 60);
    for &i in &flips {
        llr[i] = -llr[i];
    }
    let mut first_cut: Option<Vec<u8>> = None;
    for per_pass in [1usize, 333, 4096] {
        let mut g = Tb::new()
            .add(
                "src",
                Source::new(
                    Format::llr(40.0, Some(8.0)),
                    Data::Llr(llr.clone()),
                    per_pass,
                ),
            )
            .add("d", Depuncture::default())
            .add("v", ViterbiDecode::default())
            .connect("src", "d")
            .connect("d", "v")
            .set("d.pattern", txt("dvb_3_4"))
            .set("v.code", txt("dvb_k7"))
            .set("v.ends", txt("start"))
            .set("v.traceback", Value::F64(160.0))
            .build()
            .unwrap();
        let got = run(&mut g, llr.len() / per_pass + 2, &["v.out"]);
        let out = hard_bits(&got[0].data);
        // Everything but the last window is released, and exact.
        assert!(out.len() > bits.len() - 400, "{} bits", out.len());
        assert_eq!(out, bits[..out.len()], "pieces of {per_pass}");
        match &first_cut {
            None => first_cut = Some(out),
            Some(r) => assert_eq!(&out, r),
        }
        // The re-encoding count (published every quarter second of signal, 10 values here) finds the flips released so far.
        let n = field(&g, "v", "bits_corrected");
        assert!((55.0..=60.0).contains(&n), "{n} corrected");
        assert!(field(&g, "v", "mean_gain") > 50.0);
    }
}

#[test]
fn a_frame_start_after_junk_puts_the_frames_in_place() {
    // Four terminated frames of 64 steps (60 bits and the K = 5 tail), after 10 values of junk.
    let mut rng = Rng(5);
    let bits = rng.bits(240);
    let mut llr = vec![9i8; 10];
    for f in bits.chunks(60) {
        let mut coded = Vec::new();
        let mut e = Encoder::new(codes::NXDN_K5);
        e.encode(f, &mut coded);
        e.terminate(&mut coded);
        llr.extend(llr_of(&coded, 20));
    }
    for per_pass in [7usize, 50, 600] {
        let mut g = Tb::new()
            .add(
                "src",
                TaggedSource::new(
                    Format::llr(100.0, None),
                    Data::Llr(llr.clone()),
                    &[10],
                    per_pass,
                ),
            )
            .add("v", ViterbiDecode::default())
            .connect("src", "v")
            .set("v.code", txt("nxdn_k5"))
            .set("v.ends", txt("zero"))
            .set("v.frame", Value::F64(64.0))
            .build()
            .unwrap();
        let got = run(&mut g, llr.len() / per_pass + 2, &["v.out"]);
        assert_eq!(hard_bits(&got[0].data), bits, "pieces of {per_pass}");
    }
}

#[test]
fn an_end_of_stream_releases_the_rest() {
    let mut rng = Rng(8);
    let bits = rng.bits(500);
    let mut coded = Vec::new();
    let mut e = Encoder::new(codes::GSM_K5);
    e.encode(&bits, &mut coded);
    e.terminate(&mut coded);
    let llr = llr_of(&coded, 30);
    struct Eos(Source, usize);
    impl Component for Eos {
        fn info(&self) -> sdrkit_graph::ComponentInfo {
            self.0.info()
        }
        fn values(&self) -> sdrkit_core::Values {
            self.0.values()
        }
        fn configure(&mut self, f: &[Format]) -> Result<Vec<Format>, sdrkit_core::ConfigError> {
            self.0.configure(f)
        }
        fn process(&mut self, io: &mut sdrkit_graph::Io) {
            self.0.process(io);
            let end = io.output(0).end();
            if end == self.1 as u64 && !io.output(0).is_empty() {
                io.output(0).push_tag(end, TagKind::EndOfStream);
            }
        }
    }
    let n = llr.len();
    let mut g = Tb::new()
        .add(
            "src",
            Eos(
                Source::new(Format::llr(100.0, None), Data::Llr(llr), 128),
                n,
            ),
        )
        .add("v", ViterbiDecode::default())
        .connect("src", "v")
        .set("v.code", txt("gsm_k5"))
        .set("v.ends", txt("zero"))
        .build()
        .unwrap();
    let got = run(&mut g, 12, &["v.out"]);
    assert_eq!(hard_bits(&got[0].data), bits);
}

#[test]
fn settings_are_strict() {
    let mut v = ViterbiDecode::default();
    assert!(set(&mut v, "code", &Value::Text("wspr_k32".into())).is_err());
    assert!(set(&mut v, "code", &Value::Text("DVB_K7".into())).is_err());
    assert!(set(&mut v, "ends", &Value::Text("closed".into())).is_err());
    assert!(set(&mut v, "traceback", &Value::F64(-5.0)).is_err());
    assert!(set(&mut v, "frame", &Value::F64(2.5)).is_err());
    assert!(set(&mut v, "frame", &Value::Text("long".into())).is_err());
    assert!(set(&mut v, "depth", &Value::F64(5.0)).is_err());
    // Tail-biting needs frames; a frame shorter than K is refused.
    for (k, v) in [
        ("v.ends", Value::Text("tail_biting".into())),
        ("v.frame", Value::F64(3.0)),
    ] {
        let mut g = Tb::new()
            .add(
                "src",
                Source::new(Format::llr(10.0, None), Data::Llr(vec![1]), 1),
            )
            .add("v", ViterbiDecode::default())
            .connect("src", "v")
            .set(k, v)
            .build()
            .unwrap();
        assert!(g.configure_input(Format::iq(1.0, 0.0, 0.0)).is_err(), "{k}");
    }
}
