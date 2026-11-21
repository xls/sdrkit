//! TV (auto): the detector distinguishes the standards on the generators of this crate, noise is never sure, and the receiver
//! requests from the host the receiver it found.

use super::common::{self, chain, impair};
use sdrkit_core::{Complex32, HostRequest, SignalType, Value};
use sdrkit_stack_tvterr::auto::detect::{detect, Standard, Verdict};
use sdrkit_stack_tvterr::isdbt::gen::{IsdbtGenParams, IsdbtGenerator};

#[test]
fn tv_auto_tells_the_standards_apart() {
    // ISDB-T 6 MHz at 8 Msps, 300 kHz off centre.
    let x = common::generate(
        IsdbtGenerator::new(IsdbtGenParams::default()).unwrap(),
        0.35,
    );
    let x = impair(&x, 8e6, &chain(300e3, 0.0, Some(12.0), 5.57e6));
    let d = detect(&x, 8e6, 473e6, false);
    assert_eq!(
        d.standard,
        Standard::IsdbT,
        "{} / {:?}",
        d.headline,
        d.evidence
    );
    assert_eq!((d.isdbt_mode, d.width_mhz), (3, 6.0));
    assert!((d.guard - 0.125).abs() < 1e-9);
    // ISDB-T 8 MHz at 10 Msps.
    let x = common::generate(
        IsdbtGenerator::new(IsdbtGenParams {
            rate_hz: 10e6,
            width_mhz: 8.0,
            ..Default::default()
        })
        .unwrap(),
        0.35,
    );
    let x = impair(&x, 10e6, &chain(0.0, 0.0, Some(12.0), 7.43e6));
    let d = detect(&x, 10e6, 473e6, false);
    assert_eq!(
        (d.standard, d.width_mhz),
        (Standard::IsdbT, 8.0),
        "{}",
        d.headline
    );
    // ATSC 8-VSB at 8 Msps.
    let x = common::generate(
        sdrkit_stack_tvterr::atsc::gen::AtscGenerator::new(Default::default()).unwrap(),
        0.35,
    );
    let x = impair(&x, 8e6, &chain(0.0, 0.0, Some(12.0), 5.38e6));
    let d = detect(&x, 8e6, 539e6, false);
    assert_eq!(d.standard, Standard::Atsc, "{}", d.headline);
    // DTMB PN945 and PN420 at 10 Msps.
    for h in [
        sdrkit_stack_tvterr::dtmb::pn::Header::Pn945,
        sdrkit_stack_tvterr::dtmb::pn::Header::Pn420,
    ] {
        let g = sdrkit_stack_tvterr::dtmb::gen::DtmbGenerator::new(
            sdrkit_stack_tvterr::dtmb::gen::DtmbGenParams {
                header: h,
                ..Default::default()
            },
        )
        .unwrap();
        let x = impair(
            &common::generate(g, 0.35),
            10e6,
            &chain(0.0, 0.0, Some(12.0), 7.56e6),
        );
        let d = detect(&x, 10e6, 530e6, false);
        assert_eq!(
            d.standard,
            Standard::Dtmb,
            "{} {:?}",
            d.headline,
            d.evidence
        );
        assert_eq!(d.dtmb_header, h.name());
    }
    // A DVB-T-like OFDM: 8K, guard 1/4, 8 MHz, at 10 Msps.
    let x = dvbt_like(8192, 0.25, 8.0, 10e6, 3_500_000);
    let x = impair(&x, 10e6, &chain(0.0, 0.0, Some(12.0), 7.6e6));
    let d = detect(&x, 10e6, 600e6, false);
    assert!(
        matches!(d.standard, Standard::DvbT | Standard::DvbT2),
        "{}",
        d.headline
    );
    assert_eq!((d.fft, d.width_mhz), (8192, 8.0));
    assert!((d.guard - 0.25).abs() < 1e-9);
}

#[test]
fn tv_auto_noise_is_never_sure() {
    for seed in 0..3 {
        let x = common::noise(2_800_000, 0.04, seed);
        let d = detect(&x, 8e6, 500e6, false);
        assert!(d.verdict < Verdict::Sure, "{}", d.headline);
    }
    let d = detect(&[Complex32::new(0.0, 0.0); 1000], 8e6, 500e6, false);
    assert_eq!(d.headline, "too few samples");
}

#[test]
fn tv_auto_asks_the_host_for_the_receiver() {
    let rate = 8e6;
    let x = common::generate(IsdbtGenerator::new(IsdbtGenParams::default()).unwrap(), 1.5);
    let x = impair(&x, rate, &chain(0.0, 0.0, Some(20.0), 5.57e6));
    let mut rx = common::receiver(
        &sdrkit_stack_tvterr::auto::DESCRIPTOR,
        &[("decode_video", "true")],
    );
    let r = common::run(rx.as_mut(), rate, &x, 65536, "");
    assert_eq!(r.t("switch_to"), "isdbt", "{:?}", r.results.fields());
    assert!(r
        .events
        .iter()
        .any(|e| e.is(&sdrkit_stack_tvterr::typed::AUTO_FOUND)));
    let follow = r.requests.iter().find_map(|q| match q {
        HostRequest::Follow {
            signal,
            freq_hz,
            settings,
            ..
        } => Some((*signal, *freq_hz, settings.clone())),
        _ => None,
    });
    // The stack switched on decodes video as TV (auto) was told.
    let decode = vec![("decode_video".to_owned(), Value::Bool(true))];
    assert_eq!(follow, Some((Some(SignalType::IsdbT), 539e6, decode)));
    assert!(r.requests.iter().all(|q| q.validate().is_ok()));
}

/// An OFDM signal like DVB-T: `fft` carriers of which 0.832 are used, random QPSK, guard, at the rate of `width_mhz`, unit power.
fn dvbt_like(fft: usize, guard: f64, width_mhz: f64, rate: f64, n: usize) -> Vec<Complex32> {
    let fs = 8e6 * width_mhz / 7.0;
    let used = (fft as f64 * 0.832) as usize;
    let mut ifft = sdrkit_algo_dsp::fft::Fft::inverse(fft);
    let gl = (fft as f64 * guard) as usize;
    let mut rng = sdrkit_algo_sim::rng::Rng::new(7);
    let mut native = Vec::new();
    while (native.len() as f64) < n as f64 * fs / rate + 2.0 * (fft + gl) as f64 {
        let mut f = vec![Complex32::new(0.0, 0.0); fft];
        for k in 0..used {
            let s = rng.next_u64();
            let bin = (k + fft - used / 2) % fft;
            f[bin] = Complex32::new(
                if s & 1 == 0 { 1.0 } else { -1.0 },
                if s & 2 == 0 { 1.0 } else { -1.0 },
            );
        }
        ifft.process(&mut f);
        native.extend_from_slice(&f[fft - gl..]);
        native.extend_from_slice(&f);
    }
    let p: f64 = native.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / native.len() as f64;
    native.iter_mut().for_each(|v| *v /= p.sqrt() as f32);
    let mut rs =
        sdrkit_stack_tvterr::band::resampler(fs, rate, 0.45 * fs.min(rate), 0.5 * fs.min(rate))
            .unwrap();
    let mut out = Vec::new();
    rs.process(&native, &mut out);
    out.truncate(n);
    out
}
