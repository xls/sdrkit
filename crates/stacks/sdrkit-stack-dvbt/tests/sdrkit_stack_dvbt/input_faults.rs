//! Faults of the input that are not the receiver's: a radio whose gain control hunts, and runs of samples that the
//! radio lost. The receiver keeps its measured channel width and names the hunting gain. The two
//! recordings of a cable feed (Australian channels 6 and 11, 64/7 Msps) run when their files are named by
//! `SDRKIT_DVBT_CABLE_A` (219.5 MHz) and `SDRKIT_DVBT_CABLE_B` (177.5 MHz).

use crate::common::{config, receiver, rows, signal, Case};
use sdrkit_core::owner::Owner;
use sdrkit_core::{Complex32, InputInfo, LockState, Output, Receiver};
use sdrkit_stack_dvbt::bandwidth::WidthSource;
use sdrkit_stack_dvbt::params::*;
use sdrkit_stack_dvbt::settings::BandwidthChoice;
use sdrkit_stack_dvbt::typed::DvbtFields as F;

/// The sample rate of the cable recordings, 64/7 MHz.
const RATE: f64 = 9_142_856.0;

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

/// The gain of a radio whose gain control hunts, as recording B shows it: three levels in 3 ms, 1.2 dB and 4.7 dB
/// apart, and the highest with the phase turned by 172 degrees.
fn hunting_gain(x: &mut [Complex32], rate: f64) {
    let third = (1e-3 * rate) as usize;
    let turned = Complex32::from_polar(1.72, 172f32.to_radians());
    for (i, s) in x.iter_mut().enumerate() {
        match (i / third) % 3 {
            0 => {}
            1 => *s *= 1.15,
            _ => *s *= turned,
        }
    }
}

/// A 7 MHz multiplex (8K 1/16) at 64/7 Msps, 30 dB, through a radio whose gain hunts: no symbol is whole, so
/// nothing locks.
/// The width that the spectrum gave stays (the other widths are not tried over it), and the log names the fault.
#[test]
fn a_measured_width_stays_and_the_hunting_gain_is_named() {
    let mut c = Case::new(
        Bandwidth::B7,
        tps(Mode::K8, Guard::G16, Constellation::Qam64, CodeRate::R34),
    );
    c.frames = 48;
    c.input_rate = Some(Bandwidth::B8.natural_rate());
    c.rx_bandwidth = Some(BandwidthChoice::Auto);
    let (mut sig, rate, _) = signal(&c);
    hunting_gain(&mut sig, rate);
    assert!(
        sig.len() as f64 > 3.0 * rate,
        "longer than the time a width has to lock"
    );
    let (mut rx, _tap) = receiver(&c, rate);
    let mut out = Output::default();
    let mut widths = Vec::new();
    for chunk in sig.chunks(65536) {
        rx.feed(chunk, &mut out);
        out.clear();
        if let Some(w) = Owner::results(&rx).fields().at(&F::CHANNEL_WIDTH) {
            if widths.last() != Some(&w) {
                widths.push(w);
            }
        }
    }
    rx.finish(&mut out);
    let r = Owner::results(&rx);
    assert_eq!(widths, [8e6, 7e6], "the guess, then the measured width");
    assert_eq!(
        r.fields().at(&F::BANDWIDTH_FROM),
        Some(WidthSource::Measured)
    );
    assert_ne!(r.status().lock_state, LockState::Locked);
    assert!(
        r.log_lines().iter().any(|l| l.contains("gain control")),
        "{:?}",
        r.log_lines()
    );
}

/// A radio that changed its rate still delivers samples of the former rate for a while. A 7 MHz multiplex sampled at
/// 8 Msps looks 8 MHz wide to a stack that was told 64/7 Msps. The width that those samples gave brings no lock, is
/// measured again on the samples of the stated rate, and the stack locks.
#[test]
fn a_width_measured_on_samples_of_a_former_rate_is_measured_again() {
    let mut former = Case::new(Bandwidth::B7, TpsParams::default());
    former.frames = 20;
    let (stale, stale_rate, _) = signal(&former);
    assert_eq!(stale_rate, 8e6);
    let mut c = Case::new(Bandwidth::B7, TpsParams::default());
    c.frames = 200;
    c.input_rate = Some(Bandwidth::B8.natural_rate());
    c.rx_bandwidth = Some(BandwidthChoice::Auto);
    let (sig, rate, _) = signal(&c);
    let (mut rx, _tap) = receiver(&c, rate);
    let mut out = Output::default();
    let mut widths = Vec::new();
    for chunk in stale.chunks(65536).chain(sig.chunks(65536)) {
        rx.feed(chunk, &mut out);
        out.clear();
        if let Some(w) = Owner::results(&rx).fields().at(&F::CHANNEL_WIDTH) {
            if widths.last() != Some(&w) {
                widths.push(w);
            }
        }
    }
    rx.finish(&mut out);
    let r = Owner::results(&rx);
    assert_eq!(widths, [8e6, 7e6], "{:?}", r.log_lines());
    assert_eq!(
        r.fields().at(&F::BANDWIDTH_FROM),
        Some(WidthSource::Measured)
    );
    assert_eq!(
        r.status().lock_state,
        LockState::Locked,
        "{:?}",
        r.log_lines()
    );
}

/// The samples of the recording that the environment variable `var` names, or `None` with the reason printed.
fn cable(var: &str) -> Option<Vec<Complex32>> {
    let Some(path) = std::env::var_os(var).filter(|p| !p.is_empty()) else {
        eprintln!("skipped: set {var} to the .sigmf-data file of the recording");
        return None;
    };
    let raw = std::fs::read(&path).expect("the recording reads");
    eprintln!("{var}: {} samples", raw.len() / 4);
    Some(
        raw.as_chunks::<4>()
            .0
            .iter()
            .map(|c| {
                let i = i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0;
                let q = i16::from_le_bytes([c[2], c[3]]) as f32 / 32768.0;
                Complex32::new(i, q)
            })
            .collect(),
    )
}

/// Recording A through every placement of cuts: the transport packets are those of one thread.
#[test]
fn cable_recording_a_gives_the_same_packets_for_every_placement_of_cuts() {
    let Some(x) = cable("SDRKIT_DVBT_CABLE_A") else {
        return;
    };
    let mut case = Case::new(Bandwidth::B8, Default::default());
    case.rx_bandwidth = Some(BandwidthChoice::Auto);
    let whole = crate::common::run_signal(&case, &x, RATE, 0).packets;
    assert!(whole.len() > 10_000, "{} packets", whole.len());
    for cuts in 1..8 {
        case.cuts = cuts;
        let cut = crate::common::run_signal(&case, &x, RATE, 0).packets;
        assert!(
            cut == whole,
            "cut at {cuts:#05b}: {} packets, {} on one thread",
            cut.len(),
            whole.len()
        );
    }
}

/// Recording A: 8K 1/16 64-QAM 3/4 at 33 dB with nine services. The file has runs of lost samples from 0.86 s to
/// 0.92 s and from 3.81 s to 4.07 s. Outside them no Reed-Solomon frame fails.
#[test]
fn cable_recording_a_decodes_clean_between_its_lost_samples() {
    let Some(x) = cable("SDRKIT_DVBT_CABLE_A") else {
        return;
    };
    let mut rx = sdrkit_stack_dvbt::receiver::receiver(&config(&[])).unwrap();
    rx.configure(&InputInfo::new(RATE, 219.5e6, 219.5e6).recording())
        .unwrap();
    let mut out = Output::default();
    let mut locked_at = None;
    // The failed frames at the edges of the whole stretches: 1.2 s, 3.7 s, 4.8 s and the end.
    let mut failed = [None; 4];
    let marks = [1.2, 3.7, 4.8, f64::MAX];
    let mut snr = Vec::new();
    for (k, c) in x.chunks(16384).enumerate() {
        rx.feed(c, &mut out);
        out.clear();
        let t = (k + 1) as f64 * 16384.0 / RATE;
        let r = Owner::results(&rx);
        let locked = r.status().lock_state == LockState::Locked;
        if locked && locked_at.is_none() {
            locked_at = Some(t);
        }
        for (f, m) in failed.iter_mut().zip(marks) {
            if t <= m {
                *f = r.fields().at(&F::RS_FAILED);
            }
        }
        if (1.2..3.7).contains(&t) || t > 4.8 {
            assert!(locked, "locked at {t:.2} s");
        }
        if locked {
            snr.extend(r.fields().at(&F::SNR_DB));
        }
    }
    let r = Owner::results(&rx);
    let f = r.fields();
    eprintln!(
        "locked at {locked_at:?} s, failed frames {failed:?}, MER {:?}, BER {:?} / {:?}",
        f.at(&F::MER_DB),
        f.at(&F::BER_PRE_VITERBI),
        f.at(&F::BER_POST_VITERBI)
    );
    assert!(locked_at.is_some_and(|t| t < 0.5), "{locked_at:?}");
    assert_eq!(f.at(&F::CHANNEL_WIDTH), Some(7e6));
    assert_eq!(f.at(&F::BANDWIDTH_FROM), Some(WidthSource::Measured));
    assert_eq!(f.at(&F::CONSTELLATION), Some(Constellation::Qam64));
    assert_eq!(f.at(&F::CODE_RATE_HP), Some(CodeRate::R34));
    assert_eq!(failed[0], failed[1], "no failed frame from 1.2 s to 3.7 s");
    assert_eq!(
        failed[2], failed[3],
        "no failed frame from 4.8 s to the end"
    );
    assert_eq!(rows(r, "services"), 9);
    assert!(f.at(&F::MER_DB).is_some_and(|m| m > 31.5));
    let lo = snr.iter().copied().fold(f64::MAX, f64::min);
    assert!(lo > 31.0, "lowest SNR {lo:.1} dB");
}

/// Recording B: the gain control of the radio hunts (three levels every 3 ms), so few symbols are whole. The receiver
/// keeps the measured 7 MHz and names the fault.
#[test]
fn cable_recording_b_keeps_7_mhz_and_names_the_hunting_gain() {
    let Some(x) = cable("SDRKIT_DVBT_CABLE_B") else {
        return;
    };
    let mut rx = sdrkit_stack_dvbt::receiver::receiver(&config(&[])).unwrap();
    rx.configure(&InputInfo::new(RATE, 177.5e6, 177.5e6).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(16384) {
        rx.feed(c, &mut out);
        out.clear();
    }
    let r = Owner::results(&rx);
    assert_eq!(r.fields().at(&F::CHANNEL_WIDTH), Some(7e6));
    assert_eq!(
        r.fields().at(&F::BANDWIDTH_FROM),
        Some(WidthSource::Measured)
    );
    assert!(
        r.log_lines().iter().any(|l| l.contains("gain control")),
        "{:?}",
        r.log_lines()
    );
}
