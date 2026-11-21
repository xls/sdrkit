//! ISDB-S: three streams in three modulations each decoded alone, with noise, a carrier offset and a clock error; rate refusal.

use crate::common::*;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_CLOCK_PPM, GEN_ESN0_DB, GEN_NOISE, GEN_OFFSET_HZ};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::Receiver;
use sdrkit_proto_mpegts::source::{Fill, Numbered, NumberedCheck};
use sdrkit_stack_dvbcs::isdbs::{
    layout, GenLayout, IsdbsGenParams, IsdbsGenerator, ISDBS, STREAM_SETTING,
};

#[test]
fn three_streams_each_decoded_with_noise_and_offset() {
    let fs = 57.72e6;
    let x = generate(
        &ISDBS,
        fs,
        9.0 * 8.0 * 39936.0 * 2.0 / fs,
        &[
            CaseSetting::new(&GEN_NOISE, NoiseKind::EsN0),
            CaseSetting::new(&GEN_ESN0_DB, 16.0),
            CaseSetting::new(&GEN_OFFSET_HZ, 120e3),
            CaseSetting::new(&GEN_CLOCK_PPM, 15.0),
        ],
    );
    for (stream, min) in [(0u16, 400), (1, 200), (2, 30)] {
        let mut rx = Rx::new(
            &ISDBS,
            &[CaseSetting::new(&STREAM_SETTING, i64::from(stream))],
            &recording(fs, 1049.48e6),
        );
        rx.feed(&x, 100_000);
        rx.dump();
        let mut c = NumberedCheck::default();
        let pattern = Numbered::new(0x20 + stream, Fill::N7I13);
        for p in &rx.packets {
            c.push(&pattern, p, 0.0);
        }
        println!("stream {stream}: {c:?}");
        assert!(
            c.good >= min && c.wrong == 0 && c.flagged == 0 && c.gaps == 0,
            "stream {stream}"
        );
        // Only the selected stream comes out: no numbered packet of another stream.
        for other in (0..3u16).filter(|&o| o != stream) {
            let mut o = NumberedCheck::default();
            for p in &rx.packets {
                o.push(&Numbered::new(0x20 + other, Fill::N7I13), p, 0.0);
            }
            assert_eq!(o.good, 0, "stream {other} leaked into {stream}");
        }
        let cfo = rx.num("offset_hz");
        assert!((cfo - 120e3).abs() < 3e3, "{cfo}");
        let es = rx.num("esn0_db");
        assert!((es - 16.0).abs() < 3.0, "{es}");
        assert_eq!(rows(rx.results(), "streams"), 3);
    }
}

#[test]
fn rates_below_39_msps_and_bad_layouts_are_refused() {
    let mut rx = sdrkit_stack_dvbcs::isdbs::receiver(&config(&ISDBS, &[])).unwrap();
    assert!(rx.configure(&recording(20e6, 1049.48e6)).is_err());
    assert!(rx.configure(&recording(57.72e6, 1049.48e6)).is_ok());
    assert!(rx.configure(&recording(40e6, 1049.48e6)).is_ok());
    assert!(IsdbsGenerator::new(IsdbsGenParams {
        fs: 57.72e6,
        groups: vec![],
    })
    .is_err());
    assert!(IsdbsGenerator::new(IsdbsGenParams {
        fs: 57.72e6,
        groups: layout(GenLayout::Mixed),
    })
    .is_ok());
}
