//! The SNR result against the noise that was added, the BER after Viterbi with blocks that failed, the sample clock
//! estimate in 2K, and the lock after the sample rate changes and goes back.

use crate::common::{choice, config, drain_packets, signal, value, Case};
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_core::owner::Owner;
use sdrkit_core::{Complex32, Generator, InputInfo, LockState, Output, Receiver, Results, Value};
use sdrkit_stack_dvbt::generator::{DvbtGenerator, GenParams};
use sdrkit_stack_dvbt::items::{Link, Telemetry};
use sdrkit_stack_dvbt::params::*;
use sdrkit_stack_dvbt::settings::BandwidthChoice;

fn tps(mode: Mode, guard: Guard, c: Constellation, r: CodeRate) -> TpsParams {
    TpsParams {
        mode,
        guard,
        constellation: c,
        rate_hp: r,
        rate_lp: r,
        ..Default::default()
    }
}

/// The number in field `k` of `r`.
fn num(r: &Results, k: &str) -> f64 {
    match value(r, k) {
        Some(Value::F64(x) | Value::Hz(x)) => *x,
        Some(Value::I64(x)) => *x as f64,
        v => panic!("field {k} = {v:?} is not a number"),
    }
}

/// What a case leaves at its end.
struct End {
    results: Results,
    telemetry: Telemetry,
    /// The largest drift that the timing loop held after the first second, ppm.
    drift_max: f64,
    /// Packets with the transport error flag clear and set.
    packets: (usize, usize),
}

/// Runs a case with the receiver at the width of the signal.
fn end(c: &Case) -> End {
    let (sig, rate, _) = signal(c);
    let link = Link::new();
    let cfg = config(&[("bandwidth", choice(BandwidthChoice::of(c.bw)))]);
    let shape = sdrkit_stack_dvbt::typed::shape(sdrkit_stack_dvbt::receiver::ID);
    let mut rx = sdrkit_stack_dvbt::receiver::receiver_on(link.clone(), shape, &cfg).unwrap();
    rx.configure(&InputInfo::new(rate, 600e6, 600e6).recording())
        .unwrap();
    let mut tap = rx.graph_mut().tap("fec.out", 1 << 14).unwrap();
    let mut buf = sdrkit_graph::Buffer::new(&sdrkit_proto_mpegts::stream::kind());
    let mut packets = Vec::new();
    let mut out = Output::default();
    let mut drift_max = 0f64;
    for (i, chunk) in sig.chunks(c.block).enumerate() {
        rx.feed(chunk, &mut out);
        out.clear();
        drain_packets(&mut tap, &mut buf, &mut packets);
        if (i * c.block) as f64 > rate {
            let d = link.telemetry.lock().unwrap().drift_ppm;
            drift_max = drift_max.max(d.abs());
        }
    }
    rx.finish(&mut out);
    drain_packets(&mut tap, &mut buf, &mut packets);
    let bad = packets.iter().filter(|p| p[1] & 0x80 != 0).count();
    let telemetry = link.telemetry.lock().unwrap().clone();
    End {
        results: Owner::results(&rx).clone(),
        telemetry,
        drift_max,
        packets: (packets.len() - bad, bad),
    }
}

/// The mean power of a carrier over that of a data cell, dB: the pilots are sent 16/9 as strong.
fn pilot_boost_db(mode: Mode) -> f64 {
    let k = mode.carriers() as f64;
    let tps = if mode == Mode::K8 { 68.0 } else { 17.0 };
    let pilots = k - mode.data_carriers() as f64 - tps;
    10.0 * ((k + pilots * (16.0 / 9.0 - 1.0)) / k).log10()
}

/// `snr_db` is the power of a data cell over the noise of a carrier, estimated from the continual pilots. The test
/// noise is set against the mean power of the carriers, which the boosted pilots lift 0.3 dB above the data cells.
#[test]
fn the_snr_is_that_of_the_added_noise() {
    for (mode, frames) in [(Mode::K2, 40), (Mode::K8, 10)] {
        for added in [5.0, 10.0, 20.0, 30.0] {
            let mut c = Case::new(
                Bandwidth::B8,
                tps(mode, Guard::G8, Constellation::Qpsk, CodeRate::R12),
            );
            c.frames = frames;
            c.snr_db = Some(added);
            let e = end(&c);
            let want = added - pilot_boost_db(mode);
            let got = num(&e.results, "snr_db");
            eprintln!("{mode:?} {added} dB added: data cells at {want:.2} dB, snr_db {got:.2}");
            assert!((got - want).abs() < 1.0, "{mode:?} {added}: {got}");
        }
    }
}

/// A signal 60 dB under full scale reads the same SNR as at full scale: the noise has no floor in absolute units.
#[test]
fn the_snr_does_not_depend_on_the_level() {
    let mut c = Case::new(Bandwidth::B8, TpsParams::default());
    c.snr_db = None;
    let full = num(&end(&c).results, "snr_db");
    let (mut sig, rate, _) = signal(&c);
    sig.iter_mut().for_each(|s| *s *= 1e-3);
    let mut rx = crate::common::stack(&[("bandwidth", choice(BandwidthChoice::B8))]);
    rx.configure(&InputInfo::new(rate, 600e6, 600e6).recording())
        .unwrap();
    let mut out = Output::default();
    for chunk in sig.chunks(c.block) {
        rx.feed(chunk, &mut out);
        out.clear();
    }
    let low = num(Owner::results(&rx), "snr_db");
    eprintln!("no noise: snr_db {full:.1} at full scale, {low:.1} at -60 dB");
    assert!(full > 45.0 && (low - full).abs() < 1.0, "{full} {low}");
}

/// A burst of noise makes Reed-Solomon blocks fail. They are counted in `rs_failed` and stay out of the BER after
/// Viterbi, which is then at most what Reed-Solomon can correct: 8 bytes of 204.
#[test]
fn the_ber_after_viterbi_leaves_out_the_blocks_that_failed() {
    let mut c = Case::new(Bandwidth::B8, TpsParams::default());
    c.frames = 100;
    let seconds = c.frames as f64 * 68.0 * 2304.0 / Bandwidth::B8.natural_rate();
    c.burst = Some((seconds - 0.6, 0.3, 4.0));
    let e = end(&c);
    let (failed, blocks) = (num(&e.results, "rs_failed"), num(&e.results, "rs_blocks"));
    let post = num(&e.results, "ber_post_viterbi");
    eprintln!(
        "burst: {failed} of {blocks} blocks failed, ber after Viterbi {post:.2e}, packets {:?}",
        e.packets
    );
    assert!(failed > 100.0, "{failed} blocks failed");
    assert!(post <= 64.0 / 1632.0, "{post}");
    assert!(post < 5e-3, "{post}");
    // Steady noise near the limit: the ratio is the corrected bits over the bits of the decoded blocks. Each corrected
    // byte has 1 to 8 wrong bits, and the last second is like the whole run.
    let mut c = Case::new(Bandwidth::B8, TpsParams::default());
    c.frames = 100;
    c.snr_db = Some(12.0);
    let e = end(&c);
    let r = &e.results;
    let decoded = num(r, "rs_blocks") - num(r, "rs_failed");
    let bytes = num(r, "rs_bytes_corrected");
    let post = num(r, "ber_post_viterbi");
    eprintln!(
        "12 dB: {bytes} bytes corrected in {decoded} blocks, {} failed, ber after Viterbi {post:.2e}",
        num(r, "rs_failed")
    );
    assert!(num(r, "rs_corrected") > 10.0);
    let per_bit = bytes / (decoded * 1632.0);
    assert!(
        post > 0.5 * per_bit && post < 16.0 * per_bit,
        "{post} {per_bit}"
    );
}

/// A 2K signal whose sample clock has no error: the timing loop holds a drift within 0.2 ppm after the first second
/// and hands no correction to the resampler, at each guard interval, without noise and at 30 and 15 dB.
#[test]
fn a_2k_signal_without_clock_error_gets_no_clock_correction() {
    for guard in [Guard::G32, Guard::G8, Guard::G4] {
        for snr in [None, Some(30.0), Some(15.0)] {
            let mut c = Case::new(
                Bandwidth::B8,
                tps(Mode::K2, guard, Constellation::Qam16, CodeRate::R23),
            );
            c.frames = 150;
            c.snr_db = snr;
            let e = end(&c);
            let applied = num(&e.results, "clock_ppm");
            eprintln!(
                "2K {guard:?} snr {snr:?}: corrected {applied:.3} ppm, drift held at most {:.3} ppm, {:?} packets",
                e.drift_max, e.packets
            );
            assert_eq!(applied, 0.0, "{guard:?} {snr:?}");
            assert_eq!(value(&e.results, "resampling"), Some(&Value::Bool(false)));
            assert!(e.drift_max < 0.2, "{guard:?} {snr:?}: {}", e.drift_max);
            assert_eq!(e.packets.1, 0, "{guard:?} {snr:?}");
        }
    }
}

/// A 2K signal with a clock error of 20 ppm is followed: the correction and the drift that the loop still holds are
/// 20 ppm together, and no packet is lost after the first.
#[test]
fn a_2k_signal_with_20_ppm_of_clock_error_is_followed() {
    for (guard, ppm) in [(Guard::G32, 20.0), (Guard::G8, 20.0), (Guard::G4, -20.0)] {
        let mut c = Case::new(
            Bandwidth::B8,
            tps(Mode::K2, guard, Constellation::Qam16, CodeRate::R23),
        );
        c.frames = 150;
        c.clock_ppm = ppm;
        let e = end(&c);
        let applied = num(&e.results, "clock_ppm");
        let held = e.telemetry.drift_ppm;
        eprintln!(
            "2K {guard:?} {ppm} ppm: corrected {applied:.2} ppm, {held:.2} ppm held, {:?} packets",
            e.packets
        );
        assert!(
            (applied + held - ppm).abs() < 1.0,
            "{guard:?}: {applied} {held}"
        );
        assert!((applied - ppm).abs() < 2.5, "{guard:?}: {applied}");
        assert!(e.packets.0 > 3000 && e.packets.1 == 0, "{:?}", e.packets);
    }
}

/// The test multiplex with its service at `rate`, `seconds` long.
fn multiplex(g: &mut DvbtGenerator, rate: f64, seconds: f64) -> Vec<Complex32> {
    let nat = g.rate_hz();
    let mut x = vec![Complex32::default(); (seconds * nat) as usize];
    g.generate(&mut x);
    if (rate - nat).abs() < 1.0 {
        return x;
    }
    let mut y = Vec::new();
    FractionalResampler::<Complex32>::with_rates(nat, rate)
        .unwrap()
        .process(&x, &mut y);
    y
}

/// The sample rate changes and goes back, twice, with the width set and found automatically: the stack locks again
/// each time, decodes without a failed block, and its summary names the service as before the change.
#[test]
fn the_stack_locks_again_after_the_rate_changes_and_goes_back() {
    let nat = Bandwidth::B8.natural_rate();
    for width in [BandwidthChoice::B8, BandwidthChoice::Auto] {
        let mut g = DvbtGenerator::new(GenParams::new(Bandwidth::B8, TpsParams::default()));
        let mut rx = crate::common::stack(&[("bandwidth", choice(width))]);
        let mut out = Output::default();
        for (step, rate) in [nat, 10e6, nat, 10e6, nat].into_iter().enumerate() {
            let sig = multiplex(&mut g, rate, 2.5);
            rx.configure(&InputInfo::new(rate, 600e6, 600e6)).unwrap();
            let mut locked_at = None;
            for (i, chunk) in sig.chunks(16384).enumerate() {
                rx.feed(chunk, &mut out);
                out.clear();
                let locked = Owner::results(&rx).status().lock_state == LockState::Locked;
                if locked && locked_at.is_none() {
                    locked_at = Some((i * 16384) as f64 / rate);
                }
            }
            let r = Owner::results(&rx);
            eprintln!(
                "{width:?} step {step} at {rate}: locked at {locked_at:?} s, {} blocks failed, {}",
                num(r, "rs_failed"),
                r.status().summary
            );
            assert_eq!(r.status().lock_state, LockState::Locked, "{width:?} {step}");
            assert!(locked_at.is_some_and(|t| t < 1.5), "{width:?} {step}");
            assert!(
                r.status().summary.ends_with("1 services"),
                "{width:?} {step}: {}",
                r.status().summary
            );
            assert_eq!(value(r, "fec_sync"), Some(&Value::Bool(true)));
        }
    }
}
