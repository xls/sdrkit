//! The end-to-end acceptance cases (R, C, W, T). Each runs the generator (counted test pattern on PID
//! 0x0100, 0x0200 for the LP stream) through impairments from `sdrkit-dev-test` into the receiver, fed in blocks of 16384 samples (4096
//! for lock time), after 40 000 samples of noise. Cases of 40 frames or more are marked `ignore` (run them with `--ignored`, best in
//! a release build).

use crate::common::{choice, run, summary, value, Case, Outcome};
use sdrkit_core::Value;
use sdrkit_stack_dvbt::inner::ViterbiChoice;
use sdrkit_stack_dvbt::params::*;
use sdrkit_stack_dvbt::settings::{BandwidthChoice, StreamChoice};
use sdrkit_stack_dvbt::typed::DvbtFields as F;

fn t(mode: Mode, guard: Guard, c: Constellation, r: CodeRate) -> TpsParams {
    TpsParams {
        mode,
        guard,
        constellation: c,
        rate_hp: r,
        rate_lp: r,
        ..Default::default()
    }
}

fn hier(
    mode: Mode,
    guard: Guard,
    c: Constellation,
    a: Hierarchy,
    hp: CodeRate,
    lp: CodeRate,
) -> TpsParams {
    TpsParams {
        mode,
        guard,
        constellation: c,
        hierarchy: a,
        rate_hp: hp,
        rate_lp: lp,
        ..Default::default()
    }
}

fn go(name: &str, c: Case, one_in: usize) -> Outcome {
    let o = run(&c);
    eprintln!("{}", summary(name, &o));
    assert!(o.good(one_in), "{}", summary(name, &o));
    o
}

fn base(snr: f64, bw: Bandwidth, tp: TpsParams) -> Case {
    let mut c = Case::new(bw, tp);
    c.snr_db = Some(snr);
    c
}

const B8: Bandwidth = Bandwidth::B8;
use CodeRate::*;
use Constellation::*;
use Guard::*;
use Mode::*;

fn spacing(n: usize, bw: Bandwidth) -> f64 {
    bw.natural_rate() / n as f64
}

// End-to-end (R).

#[test]
fn r2() {
    go("R2", base(35.0, B8, t(K2, G32, Qam64, R34)), 20);
}

#[test]
fn r5() {
    let mut c = base(30.0, B8, t(K8, G16, Qam64, R34));
    c.cfo_hz = 1800.0;
    go("R5", c, 20);
}

fn r9() -> Case {
    let mut c = base(30.0, B8, t(K8, G16, Qam64, R34));
    c.cfo_hz = 800.0;
    c
}

#[test]
fn r9_base() {
    go("R9", r9(), 50);
}

#[test]
#[ignore]
fn r8_plus_and_minus_20ppm() {
    for ppm in [20.0, -20.0] {
        let mut c = base(32.0, B8, t(K8, G32, Qam64, R23));
        c.clock_ppm = ppm;
        c.frames = 40;
        go(&format!("R8 {ppm} ppm"), c, 20);
    }
}

#[test]
#[ignore]
fn r10_r11_noise_bursts() {
    for (dur, frames) in [(0.2, 50), (0.6, 70)] {
        let mut c = r9();
        c.frames = frames;
        c.burst = Some((1.2, dur, -5.0));
        let o = go(&format!("R10/R11 {dur} s"), c.clone(), 50);
        let lead = c.lead as f64 / B8.natural_rate();
        let worst = o
            .offsets
            .iter()
            .filter(|x| x.0 > lead + 0.3)
            .map(|x| (x.1 - 800.0).abs())
            .fold(0.0, f64::max);
        assert!(worst < 60.0, "offset error {worst} Hz");
        let end = lead + 1.2 + dur + 0.1;
        assert!(
            o.bad_times.iter().all(|&t| t <= end + 0.05),
            "bad packets after the burst: {:?}",
            o.bad_times
                .iter()
                .filter(|&&t| t > end)
                .take(5)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
#[ignore]
fn r12_to_r17_lost_samples() {
    let mut total = 0u64;
    for n in [40usize, 700, 3000, 5000, 9400, 20_000] {
        let mut c = r9();
        c.frames = 50;
        c.drop = Some((1.2, n));
        let o = go(&format!("R12-R17 {n} samples"), c, 20);
        total += o.lost + o.bad_after_first as u64;
    }
    assert!(total <= 1500 * 6, "{total} packets lost or damaged");
}

#[test]
fn r14_700_samples_lost() {
    let mut c = r9();
    c.frames = 25;
    c.drop = Some((0.4, 700));
    let o = go("R14 (shortened)", c, 20);
    assert!(o.lost + (o.bad_after_first as u64) <= 1500);
}

#[test]
fn r18() {
    let mut c = base(30.0, B8, t(K8, G16, Qam64, R34));
    c.cfo_hz = -155.0;
    c.dc = Some((-23.0, 0.7));
    go("R18", c, 20);
}

#[test]
#[ignore]
fn r19_undecodable_start() {
    let mut c = r9();
    c.frames = 45;
    c.bad_start = Some((2.0, 8.0));
    let o = go("R19", c.clone(), 20);
    let decodable = c.lead as f64 / B8.natural_rate() + 2.0;
    let first = o.first_good_s.unwrap();
    assert!(
        first - decodable < 0.8,
        "first good packet {:.3} s after the signal became decodable",
        first - decodable
    );
}

#[test]
#[ignore]
fn r20_120000_samples_lost() {
    let mut c = r9();
    c.frames = 80;
    c.drop = Some((1.2, 120_000));
    let o = go("R20", c, 20);
    assert!(
        o.lost + o.bad_after_first as u64 <= 3500,
        "{} lost or damaged",
        o.lost + o.bad_after_first as u64
    );
}

fn digest(o: &Outcome) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for p in &o.packets {
        for &b in p.iter() {
            h = (h ^ b as u64).wrapping_mul(0x100000001b3);
        }
    }
    h
}

#[test]
fn r21_every_placement_of_cuts_gives_the_packets_of_one_thread() {
    let cases = vec![
        {
            let mut c = base(30.0, B8, t(K8, G8, Qam64, R23));
            c.frames = 10;
            c
        },
        {
            let mut c = base(30.0, B8, t(K2, G4, Qam16, R34));
            c.frames = 30;
            c
        },
        {
            let mut c = r9();
            c.frames = 20;
            c.drop = Some((0.4, 700));
            c
        },
    ];
    for (i, c) in cases.into_iter().enumerate() {
        let a = run(&c);
        eprintln!(
            "R21 case {i}: {} packets, digest {:016x}",
            a.packets.len(),
            digest(&a)
        );
        assert!(a.good > 50, "case {i}: {} good packets", a.good);
        for cuts in 1..8 {
            let mut c2 = c.clone();
            c2.cuts = cuts;
            let b = run(&c2);
            assert_eq!(
                a.packets.len(),
                b.packets.len(),
                "case {i} cuts {cuts:#05b}"
            );
            assert_eq!(digest(&a), digest(&b), "case {i} cuts {cuts:#05b}");
            for k in ["rs_blocks", "rs_clean", "rs_corrected", "rs_failed"] {
                assert_eq!(
                    value(&a.results, k),
                    value(&b.results, k),
                    "case {i} cuts {cuts:#05b} {k}"
                );
            }
        }
    }
}

// Carrier offsets in whole carriers (C).

#[test]
fn c1_7_carriers_at_10msps() {
    let mut c = base(30.0, B8, t(K8, G4, Qam64, R34));
    c.cfo_hz = -7916.0;
    c.input_rate = Some(10e6);
    let o = go("C1", c, 20);
    assert_eq!(o.results.fields().at(&F::CONSTELLATION), Some(Qam64));
    assert_eq!(o.results.fields().at(&F::CODE_RATE_HP), Some(R34));
}

#[test]
fn c3_c4_c6() {
    let mut c = base(30.0, B8, t(K2, G4, Qam16, R23));
    c.cfo_hz = 1.1 * spacing(2048, B8);
    go("C3", c, 20);
    let mut c = base(30.0, B8, t(K8, G8, Qam64, R23));
    c.cfo_hz = -1.9 * spacing(8192, B8);
    go("C4", c, 20);
    let mut c = base(30.0, B8, t(K8, G32, Qam64, R34));
    c.cfo_hz = 8.1 * spacing(8192, B8);
    go("C6", c, 20);
}

// Real-world faults (W), noise 32 dB, at most 1 bad in 50.

fn w(tp: TpsParams) -> Case {
    base(32.0, B8, tp)
}

#[test]
fn w1_plus_43khz() {
    let mut c = w(t(K8, G4, Qam64, R34));
    c.cfo_hz = 43e3;
    go("W1", c, 50);
}

#[test]
fn w5_7mhz_8k() {
    let mut c = w(t(K8, G16, Qam64, R23));
    c.bw = Bandwidth::B7;
    c.cfo_hz = 31e3;
    go("W5", c, 50);
}

// A 7 MHz channel whose transmitter is 125 kHz or 166.67 kHz from the channel centre, at the sample rate that a
// radio session gets (64/7 Msps) and at the natural rate of the channel width (8 Msps).
fn offset_case(mode: Mode, cfo_hz: f64, rate: Option<f64>) {
    let mut c = w(t(mode, G16, Qam64, R23));
    c.bw = Bandwidth::B7;
    c.cfo_hz = cfo_hz;
    c.input_rate = rate;
    let o = go(&format!("W26 {mode:?} {cfo_hz} Hz at {rate:?}"), c, 50);
    assert!(
        (o.offset_hz - cfo_hz).abs() < 500.0,
        "offset {} Hz",
        o.offset_hz
    );
}

#[test]
fn w26_transmitter_offsets_of_a_7mhz_channel_8k() {
    for cfo in [-166.67e3, -125e3, 125e3, 166.67e3] {
        offset_case(K8, cfo, Some(64e6 / 7.0));
    }
    offset_case(K8, 125e3, None);
    offset_case(K8, -166.67e3, None);
}

#[test]
fn w26_transmitter_offsets_of_a_7mhz_channel_2k() {
    for cfo in [-166.67e3, -125e3, 125e3, 166.67e3] {
        offset_case(K2, cfo, Some(64e6 / 7.0));
    }
    offset_case(K2, 125e3, None);
    offset_case(K2, -166.67e3, None);
}

#[test]
fn w6_w7_80ppm() {
    let mut c = w(t(K8, G4, Qam64, R34));
    c.clock_ppm = 80.0;
    c.frames = 10;
    go("W6", c, 50);
    let mut c = w(t(K8, G8, Qam64, R56));
    c.clock_ppm = -80.0;
    c.frames = 10;
    go("W7", c, 50);
}

#[test]
fn w10_w12_w13_hierarchical() {
    let c = w(hier(K8, G32, Qam64, Hierarchy::A4, R34, R78));
    let o = go("W10", c, 50);
    assert_eq!(o.results.fields().at(&F::HIERARCHY), Some(Hierarchy::A4));
    let mut c = w(hier(K2, G8, Qam64, Hierarchy::A1, R12, R23));
    c.lp = true;
    go("W12", c, 50);
    let mut c = w(hier(K8, G32, Qam64, Hierarchy::A4, R23, R12));
    c.lp = true;
    go("W13", c, 50);
}

#[test]
fn w14_reserved_lp_rate() {
    let mut c = w(t(K2, G4, Qam16, R23));
    c.lp_field = Some(7);
    let o = go("W14", c, 50);
    assert_eq!(o.results.fields().at(&F::CODE_RATE_LP), Some(R23));
}

#[test]
fn w15_w16_off_centre() {
    let mut c = w(t(K8, G4, Qam64, R34));
    c.input_rate = Some(10e6);
    c.cfo_hz = 280e3;
    go("W15", c, 50);
    let mut c = w(t(K8, G8, Qam16, R23));
    c.input_rate = Some(10e6);
    c.cfo_hz = -950e3;
    go("W16", c, 50);
}

#[test]
fn w18_w19_iq_imbalance() {
    let mut c = w(t(K8, G4, Qam64, R23));
    c.iq_imbalance = Some((1.0, 5.0));
    go("W18", c, 50);
    let mut c = w(t(K2, G8, Qam16, R34));
    c.iq_imbalance = Some((1.0, 5.0));
    c.input_rate = Some(10e6);
    c.cfo_hz = 200e3;
    go("W19", c, 50);
}

#[test]
fn w20_echo_beyond_the_guard_does_not_loop() {
    let mut c = w(t(K8, G32, Qpsk, R12));
    c.frames = 10;
    c.echo = Some((8192 / 32 * 3 / 2, -12.0, 0.0));
    let o = run(&c);
    eprintln!("{}", summary("W20", &o));
    // Degrades but stays locked: TPS right and packets flow.
    assert!(o.tps_ok);
    assert!(o.good > 50, "{}", summary("W20", &o));
    assert!(o.results.fields().at(&F::RESYNCS).unwrap() < 5);
}

#[test]
fn w21_swapped_8k() {
    let mut c = w(t(K8, G8, Qam16, R23));
    c.swap_iq = true;
    c.cfo_hz = 12e3;
    let o = go("W21", c, 50);
    assert_eq!(o.results.fields().at(&F::SPECTRUM_INVERTED), Some(true));
}

#[test]
fn w24_everything_at_once() {
    let mut c = w(t(K8, G8, Qam16, R23));
    c.frames = 10;
    c.cfo_hz = 43e3;
    c.clock_ppm = 80.0;
    c.echo = Some((8192 / 8 / 3, -6.0, 2.0));
    c.clip8 = Some(0.6);
    c.input_rate = Some(10e6);
    go("W24", c, 50);
}

// Lock time (T1): natural rate, the signal starts part-way into a symbol, blocks of 4096.

#[test]
fn t1_lock_time() {
    let cases = [
        (t(K8, G16, Qam64, R34), 0.0, 35.0, 0.0),
        (t(K8, G16, Qam64, R34), 0.0, 35.0, 0.6),
        (t(K8, G16, Qam64, R34), 30e3, 30.0, 0.3),
        (t(K8, G16, Qam64, R34), -8.3e3, 30.0, 0.7),
        (t(K8, G32, Qam16, R23), 2e3, 20.0, 0.5),
        (t(K8, G4, Qpsk, R12), 1.5e3, 8.0, 0.2),
        (t(K2, G8, Qam16, R23), 0.0, 35.0, 0.4),
        (t(K2, G4, Qpsk, R12), 5e3, 15.0, 0.8),
    ];
    let mut firsts = Vec::new();
    for (i, (tp, cfo, snr, start)) in cases.into_iter().enumerate() {
        let mut c = base(snr, B8, tp);
        c.cfo_hz = cfo;
        c.start_frac = start;
        c.block = 4096;
        c.frames = if tp.mode == K8 { 8 } else { 20 };
        let o = run(&c);
        let first = o.first_good_s.map(|x| x - o.signal_start_s);
        // 20 good packets in a row.
        let mut run20 = None;
        let mut streak = 0;
        let mut last = None;
        for (k, p) in o.packets.iter().enumerate() {
            let ok = crate::common::check_test_packet(p, 0x0100).is_some();
            streak = if ok { streak + 1 } else { 0 };
            if streak == 20 && run20.is_none() {
                run20 = Some(k);
            }
            last = Some(k);
        }
        let _ = last;
        let t20 = run20.map(|k| {
            o.good_s
                .get(k.min(o.good_s.len() - 1))
                .copied()
                .unwrap_or(0.0)
                - o.signal_start_s
        });
        eprintln!(
            "T1 case {i}: first good packet {:?} s, 20 in a row {:?} s",
            first, t20
        );
        let f = first.expect("a good packet");
        assert!(f <= 1.5, "case {i}: {f} s");
        firsts.push(f);
    }
    eprintln!(
        "T1 mean first good packet: {:.3} s",
        firsts.iter().sum::<f64>() / firsts.len() as f64
    );
}

// Settings take effect.

#[test]
fn settings_take_effect() {
    // The LP stream chosen by the setting.
    let mut c = w(hier(K2, G8, Qam16, Hierarchy::A2, R12, R23));
    c.settings.push(("stream", choice(StreamChoice::Lp)));
    c.lp = true;
    go("stream=lp", c, 50);
    // A fixed wrong bandwidth does not decode. Auto finds the right one.
    let mut c = w(t(K2, G8, Qam16, R23));
    c.bw = Bandwidth::B6;
    c.input_rate = Some(8e6);
    c.rx_bandwidth = Some(BandwidthChoice::Auto);
    c.frames = 60;
    let o = run(&c);
    eprintln!("{}", summary("bandwidth auto (6 MHz at 8 Msps)", &o));
    assert_eq!(o.results.fields().at(&F::CHANNEL_WIDTH), Some(6e6));
    assert!(o.good(20));
    // The exact Viterbi decoder, time interpolation off, no interferer weighting, flat CSI and the full window still decode a clean
    // signal.
    for (k, v) in [
        ("viterbi", choice(ViterbiChoice::Exact)),
        ("time_interp", Value::Bool(false)),
        ("interferer_erase", Value::Bool(false)),
        ("test_flat_csi", Value::Bool(true)),
        ("test_full_window", Value::Bool(true)),
        ("test_peak_only", Value::Bool(true)),
    ] {
        let mut c = w(t(K2, G8, Qam16, R23));
        c.frames = 10;
        c.settings.push((k, v.clone()));
        let o = go(&format!("{k}={v:?}"), c, 20);
        assert!(o.good > 300);
    }
}
