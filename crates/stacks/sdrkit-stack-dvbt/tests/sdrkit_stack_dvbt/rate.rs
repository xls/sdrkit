//! A 7 MHz channel at its own rate of 8 Msps and at the rate of an 8 MHz channel: the input stage resamples for the
//! rate only at the second one, and the transport packets are the same.

use crate::common::{choice, config, drain_packets, run, run_signal, signal, value, Case};
use sdrkit_core::{InputInfo, Output, Owner, Receiver, Value};
use sdrkit_stack_dvbt::input::BYPASS_PPM;
use sdrkit_stack_dvbt::items::Link;
use sdrkit_stack_dvbt::params::{Bandwidth, TpsParams};
use sdrkit_stack_dvbt::settings::BandwidthChoice;

/// The packets of `a` and `b` from the first one that both have, and how many those are.
fn common<'a>(a: &'a [[u8; 188]], b: &'a [[u8; 188]]) -> (&'a [[u8; 188]], &'a [[u8; 188]]) {
    let (skip_a, skip_b) = match a.iter().position(|p| *p == b[0]) {
        Some(i) => (i, 0),
        None => (
            0,
            b.iter().position(|p| *p == a[0]).expect("a common packet"),
        ),
    };
    let n = (a.len() - skip_a).min(b.len() - skip_b);
    (&a[skip_a..skip_a + n], &b[skip_b..skip_b + n])
}

/// A 7 MHz channel of `frames` frames at 8 Msps.
fn at_8_msps(frames: usize) -> Case {
    let mut c = Case::new(Bandwidth::B7, TpsParams::default());
    c.frames = frames;
    c
}

#[test]
fn a_small_clock_correction_at_8_msps_takes_the_resampler_out_with_the_same_packets() {
    let c = at_8_msps(200);
    let (sig, rate, start) = signal(&c);
    assert_eq!(rate, 8e6);
    assert!(sig.len() as f64 / rate > 3.3, "{} samples", sig.len());
    let plain = run_signal(&c, &sig, rate, start);
    assert_eq!(
        value(&plain.results, "resampling"),
        Some(&Value::Bool(false))
    );
    assert_eq!(plain.bad_after_first, 0);
    // The timing loop corrects the clock by `first` ppm while the stack is locked, then takes nearly all of it back:
    // 0.05 ppm stay. The resampler goes out where its output falls on an input sample, at most one sample of drift
    // later (2.5 s at 0.05 ppm).
    for (first, back) in [(3.0, -3.05), (3.0, -2.95), (-3.0, 2.95)] {
        let link = Link::new();
        let cfg = config(&[("bandwidth", choice(BandwidthChoice::B7))]);
        let shape = sdrkit_stack_dvbt::typed::shape(sdrkit_stack_dvbt::receiver::ID);
        let mut rx = sdrkit_stack_dvbt::receiver::receiver_on(link.clone(), shape, &cfg).unwrap();
        rx.configure(&InputInfo::new(rate, 600e6, 600e6).recording())
            .unwrap();
        let mut tap = rx.graph_mut().tap("fec.out", 1 << 14).unwrap();
        let mut buf = sdrkit_graph::Buffer::new(&sdrkit_proto_mpegts::stream::kind());
        let (mut packets, mut out) = (Vec::new(), Output::default());
        let (mut corrected, mut resampled, mut left_at) = (0, false, None);
        for (i, chunk) in sig.chunks(c.block).enumerate() {
            let at_s = (i * c.block) as f64 / rate;
            if at_s > 0.4 && corrected < 2 {
                link.add_clock([first, back][corrected]);
                corrected += 1;
            }
            rx.feed(chunk, &mut out);
            out.clear();
            drain_packets(&mut tap, &mut buf, &mut packets);
            let on = link.clock_ppm() != 0.0;
            if resampled && !on && left_at.is_none() {
                left_at = Some(at_s);
            }
            resampled |= on;
        }
        rx.finish(&mut out);
        drain_packets(&mut tap, &mut buf, &mut packets);
        let left_at = left_at.expect("the resampler goes out");
        eprintln!("{first} then {back} ppm: resampler in at 0.4 s, out at {left_at:.2} s");
        assert!(resampled && left_at < 0.4 + 2.7, "{left_at}");
        let r = Owner::results(&rx);
        assert_eq!(value(r, "resampling"), Some(&Value::Bool(false)));
        assert_eq!(value(r, "clock_ppm"), Some(&Value::F64(0.0)));
        let (a, b) = common(&plain.packets, &packets);
        assert!(
            a.len() + 20 > plain.packets.len(),
            "{} of {}",
            a.len(),
            plain.packets.len()
        );
        assert!(a == b, "{first} then {back} ppm: the packets differ");
    }
}

#[test]
fn a_clock_error_below_the_bypass_limit_gives_the_same_packets() {
    let c = at_8_msps(110);
    let plain = run(&c);
    // 2 s: the clock drifts 8 samples against the signal at 0.5 ppm. The timing loop sends no correction for an error
    // under 2 ppm and the resampler stays out; the stages after it work with that drift.
    for (ppm, corrects) in [(0.5_f64, false), (-0.5, false), (0.9, false), (-0.9, false)] {
        assert!(ppm.abs() < BYPASS_PPM);
        let mut off = c.clone();
        off.clock_ppm = ppm;
        let o = run(&off);
        let Some(&Value::F64(applied)) = value(&o.results, "clock_ppm") else {
            panic!("no clock correction in the results");
        };
        eprintln!(
            "{ppm} ppm: corrected by {applied:.2} ppm, {:.2} ppm stay",
            ppm - applied
        );
        assert_eq!(applied != 0.0, corrects, "{ppm}");
        assert_eq!(
            value(&o.results, "resampling"),
            Some(&Value::Bool(corrects)),
            "{ppm}"
        );
        assert_eq!(o.bad_after_first, 0, "{ppm}");
        let (a, b) = common(&plain.packets, &o.packets);
        assert!(
            a.len() + 20 > plain.packets.len(),
            "{ppm}: {} of {}",
            a.len(),
            plain.packets.len()
        );
        assert!(a == b, "{ppm} ppm: the packets differ");
    }
}

#[test]
fn a_7_mhz_channel_at_8_msps_needs_no_resampling_for_the_rate_and_gives_the_same_packets() {
    let own = Case::new(Bandwidth::B7, TpsParams::default());
    assert_eq!(Bandwidth::B7.natural_rate(), 8e6);
    let mut wide = own.clone();
    wide.input_rate = Some(Bandwidth::B8.natural_rate());
    let (a, b) = (run(&own), run(&wide));
    assert_eq!(value(&a.results, "input_rate"), Some(&Value::Hz(8e6)));
    assert_eq!(value(&a.results, "natural_rate"), Some(&Value::Hz(8e6)));
    // At its own rate the input stage resamples only while it corrects the sample clock.
    let corrects = value(&a.results, "clock_ppm") != Some(&Value::F64(0.0));
    assert_eq!(
        value(&a.results, "resampling"),
        Some(&Value::Bool(corrects))
    );
    assert_eq!(value(&b.results, "resampling"), Some(&Value::Bool(true)));
    assert!(a.good > 50, "{} good packets", a.good);
    assert_eq!(a.bad_after_first, 0);
    assert_eq!(b.bad_after_first, 0);
    // The two runs may start and end a packet apart: the packets that both have are the same.
    let (x, y) = common(&a.packets, &b.packets);
    assert!(
        x.len() + 20 > a.packets.len(),
        "{} common of {}",
        x.len(),
        a.packets.len()
    );
    assert!(x == y);
}
