//! Times the decoders: `cargo run --release -p sdrkit-proto-tones --example bench`. Each line is the best of several runs. It returns
//! millions of input samples per second, and how many times faster than real time at 8 kHz and at 48 kHz.

use sdrkit_proto_tones::{ctcss, dcs, dtmf, selcal, selcall, twotone};
use sdrkit_util_ids::tones::selcal::Code;
use sdrkit_util_ids::tones::selcall::by_name;
use std::hint::black_box;
use std::time::Instant;

fn measure(name: &str, rate: f64, x: &[f32], mut step: impl FnMut(&[f32])) {
    step(x);
    let start = Instant::now();
    let mut best = f64::INFINITY;
    let mut runs = 0;
    while runs < 3 || start.elapsed().as_secs_f64() < 1.0 {
        let t = Instant::now();
        step(black_box(x));
        best = best.min(t.elapsed().as_secs_f64());
        runs += 1;
    }
    let msps = x.len() as f64 / best / 1e6;
    println!(
        "{name:36} {msps:8.2} M/s {:10.0}x real time",
        msps * 1e6 / rate
    );
}

/// A mix of every signal at `rate`: about 12 s.
fn audio(rate: f64) -> Vec<f32> {
    let mut parts: Vec<Vec<f32>> = Vec::new();
    let mut render = |mut s: sdrkit_proto_tones::gen::ToneSequence| {
        let mut x = vec![0f32; s.len() as usize];
        s.fill(&mut x);
        parts.push(x);
    };
    render(ctcss::ctcss_sequence(rate, 100.0, 0.15, 2.0, Some((0.5, 0.2))).unwrap());
    render(dtmf::dtmf_sequence(rate, "0123456789", 0.2, 0.2, 0.05, 0.05).unwrap());
    render(selcall::selcall_sequence(rate, by_name("ZVEI1").unwrap(), "12345", 0.3, None).unwrap());
    render(
        selcal::selcal_sequence(rate, Code::parse("AM-CP").unwrap(), 0.25, 1.0, 0.2, 0.0).unwrap(),
    );
    render(twotone::two_tone_sequence(rate, 500.0, 900.0, 1.0, 3.0, 0.3).unwrap());
    let mut e = dcs::DcsEncoder::new(
        rate,
        sdrkit_util_ids::tones::dcs::Dcs::from_octal("023").unwrap(),
        false,
        0.2,
        3.0,
        true,
    )
    .unwrap();
    let mut d = vec![0f32; e.len() as usize];
    e.fill(&mut d);
    parts.push(d);
    parts.concat()
}

fn main() {
    for rate in [8000.0, 48000.0] {
        let x = audio(rate);
        println!("at {rate} Hz, {:.1} s of audio", x.len() as f64 / rate);
        let mut n = 0usize;
        let mut c = ctcss::CtcssDecoder::new(rate, ctcss::CtcssConfig::default()).unwrap();
        measure("ctcss (50 tones)", rate, &x, |x| c.process(x, |_| n += 1));
        let mut d = dcs::DcsDecoder::new(rate, dcs::DcsConfig::default()).unwrap();
        measure("dcs", rate, &x, |x| d.process(x, |_| n += 1));
        let mut t = dtmf::DtmfDecoder::new(rate, dtmf::DtmfConfig::default()).unwrap();
        measure("dtmf", rate, &x, |x| t.process(x, |_| n += 1));
        let mut s = selcall::SelcallDecoder::new(rate, selcall::SelcallConfig::default()).unwrap();
        measure("selcall (all 14 sets)", rate, &x, |x| {
            s.process(x, |_| n += 1)
        });
        let mut a = selcal::SelcalDecoder::new(rate, selcal::SelcalConfig::default()).unwrap();
        measure("selcal (32 tones)", rate, &x, |x| a.process(x, |_| n += 1));
        let cfg = selcal::SelcalConfig {
            ssb_search: true,
            ..selcal::SelcalConfig::default()
        };
        let mut a = selcal::SelcalDecoder::new(rate, cfg).unwrap();
        measure("selcal (32 tones, sideband search)", rate, &x, |x| {
            a.process(x, |_| n += 1)
        });
        let mut w = twotone::TwoToneDecoder::new(rate, twotone::TwoToneConfig::default()).unwrap();
        measure("two-tone", rate, &x, |x| w.process(x, |_| n += 1));
        black_box(n);
    }
}
