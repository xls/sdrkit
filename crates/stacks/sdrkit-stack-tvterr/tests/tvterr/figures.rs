//! The key figures of each stack on fixed cases, printed for a comparison of two builds. Run with
//! `cargo test -p sdrkit-stack-tvterr --test tvterr figures -- --ignored --nocapture`.

use super::common::{self, chain, impair};
use sdrkit_proto_mpegts::source::Numbered;
use sdrkit_stack_tvterr::auto::detect::detect;
use sdrkit_stack_tvterr::isdbt::gen::{
    config, layer, layer_pattern, IsdbtGenParams, IsdbtGenerator,
};
use sdrkit_stack_tvterr::isdbt::tmcc::LayerParams;

#[test]
#[ignore]
fn key_figures() {
    // ISDB-T: mode 1 (13 segments 64QAM 3/4, 10 Msps, 7.3 kHz, 28 dB) and +80 ppm.
    let u = LayerParams::UNUSED;
    for (what, ppm, cfo) in [("mode 1", 0.0, 7_300.0), ("+80 ppm", 80.0, 0.0)] {
        let p = IsdbtGenParams {
            mode: 1,
            config: config(false, [layer(3, 2, 0, 13), u, u]),
            rate_hz: 10e6,
            ..Default::default()
        };
        let x = common::generate(IsdbtGenerator::new(p).unwrap(), 1.5);
        let x = impair(&x, 10e6, &chain(cfo, ppm, Some(28.0), 5.57e6));
        let mut rx = common::receiver(&sdrkit_stack_tvterr::isdbt::DESCRIPTOR, &[]);
        let r = common::run(rx.as_mut(), 10e6, &x, 65536, "merge.ts");
        let c = r.check(&layer_pattern(0));
        println!(
            "FIG isdbt {what}: first packet {:.3} s, detection {}, SNR {:.1} dB, offset {:.0} Hz, clock {:.1} ppm, good {} bad {}",
            c.first_time.unwrap_or(f64::NAN),
            r.f("detection"),
            r.f("snr_db"),
            r.f("offset_hz"),
            r.f("clock_ppm"),
            c.good,
            c.flagged + c.wrong
        );
    }
    // ATSC: the two recordings.
    for name in ["atsc-8vsb-clean", "atsc-8vsb-impaired"] {
        let rec = sdrkit_dev_test::read_recording(&format!("tvterr/{name}"))
            .unwrap_or_else(|| panic!("the recording tvterr/{name} (set SDRKIT_RECORDINGS)"));
        let mut rx = common::receiver(&sdrkit_stack_tvterr::atsc::DESCRIPTOR, &[]);
        let r = common::run(rx.as_mut(), rec.rate_hz, &rec.samples, 65536, "fec.ts");
        let c = r.check(&Numbered::TERRESTRIAL);
        println!(
            "FIG atsc {name}: first packet {:.3} s, detection {}, SNR {:.1} dB, offset {:.0} Hz, clock {:.1} ppm, good {} flagged {}",
            c.first_time.unwrap_or(f64::NAN),
            r.f("detection"),
            r.f("snr_db"),
            r.f("offset_hz"),
            r.f("clock_ppm"),
            c.good,
            c.flagged
        );
    }
    // DTMB: the recording.
    let rec = sdrkit_dev_test::read_recording("tvterr/dtmb-pn420-16qam")
        .unwrap_or_else(|| panic!("the DTMB recording"));
    let mut rx = common::receiver(&sdrkit_stack_tvterr::dtmb::DESCRIPTOR, &[]);
    let r = common::run(rx.as_mut(), rec.rate_hz, &rec.samples, 65536, "");
    println!(
        "FIG dtmb recording: C/N {:.1} dB, offset {:.0} Hz, clock {:.1} ppm, frames {}",
        r.f("cn_db"),
        r.f("offset_hz"),
        r.f("clock_ppm"),
        r.f("frames")
    );
    // TV (auto): the ISDB-T, ATSC and DTMB test signals at 12 dB.
    let x = common::generate(
        IsdbtGenerator::new(IsdbtGenParams::default()).unwrap(),
        0.35,
    );
    let x = impair(&x, 8e6, &chain(300e3, 0.0, Some(12.0), 5.57e6));
    let d = detect(&x, 8e6, 473e6, false);
    println!("FIG auto isdbt: {} ({:.2})", d.headline, d.confidence);
    let x = common::generate(
        sdrkit_stack_tvterr::atsc::gen::AtscGenerator::new(Default::default()).unwrap(),
        0.35,
    );
    let x = impair(&x, 8e6, &chain(0.0, 0.0, Some(12.0), 5.38e6));
    let d = detect(&x, 8e6, 539e6, false);
    println!("FIG auto atsc: {} ({:.2})", d.headline, d.confidence);
    let g = sdrkit_stack_tvterr::dtmb::gen::DtmbGenerator::new(Default::default()).unwrap();
    let x = impair(
        &common::generate(g, 0.35),
        10e6,
        &chain(0.0, 0.0, Some(12.0), 7.56e6),
    );
    let d = detect(&x, 10e6, 530e6, false);
    println!("FIG auto dtmb: {} ({:.2})", d.headline, d.confidence);
}
