//! The measurements that the round trips do not check: the cell id assembled from two frames, the decision-directed MER, the BER
//! before and after the Viterbi decoder, the quality figures, the frame duration and net bit rate, and the automatic bandwidth at
//! every channel width.

use crate::common::{receiver, signal, value, Case};
use sdrkit_core::owner::Owner;
use sdrkit_core::{LockState, Output, Receiver, Results, Value};
use sdrkit_stack_dvbt::bandwidth::{BandwidthDetector, WidthSource};
use sdrkit_stack_dvbt::params::*;
use sdrkit_stack_dvbt::results::Quality;
use sdrkit_stack_dvbt::settings::BandwidthChoice;
use sdrkit_stack_dvbt::typed::DvbtFields as F;

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

/// Runs a case and returns the results at the end, and every cell id reported while locked.
fn run_fields(c: &Case) -> (Results, Vec<i64>) {
    let (sig, rate, _) = signal(c);
    let (mut rx, _tap) = receiver(c, rate);
    let mut out = Output::default();
    let mut cells = Vec::new();
    for chunk in sig.chunks(c.block) {
        rx.feed(chunk, &mut out);
        out.clear();
        let r = Owner::results(&rx);
        if r.status().lock_state == LockState::Locked {
            cells.extend(r.fields().at(&F::CELL_ID));
        }
    }
    rx.finish(&mut out);
    (Owner::results(&rx).clone(), cells)
}

/// The number in field `k` of `r`.
fn num(r: &Results, k: &str) -> f64 {
    match value(r, k) {
        Some(Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x)) => *x,
        Some(Value::I64(x)) => *x as f64,
        v => panic!("field {k} = {v:?} is not a number"),
    }
}

/// The quality figures of `r` for the log.
fn quality(r: &Results) -> (f64, Option<Quality>) {
    (
        r.fields().at(&F::QUALITY_PCT).unwrap_or(0.0),
        r.fields().at(&F::QUALITY_LABEL),
    )
}

#[test]
fn cell_id_mer_ber_and_quality() {
    let mut t = tps(Mode::K2, Guard::G8, Constellation::Qam16, CodeRate::R23);
    t.cell_id = 0x1234;
    t.length = 31;
    let mut c = Case::new(Bandwidth::B8, t);
    c.block = 4096;
    let (f, cells) = run_fields(&c);
    let snr = num(&f, "snr_db");
    let mer = num(&f, "mer_db");
    let pre = num(&f, "ber_pre_viterbi");
    let post = num(&f, "ber_post_viterbi");
    eprintln!(
        "clean: snr {snr:.1} mer {mer:.1} ber pre {pre:.2e} post {post:.2e} quality {:?}",
        quality(&f)
    );
    // The 16-bit cell id is put together from b15..b8 (frames 1 and 3) and b7..b0 (frames 2 and 4). The results are reported
    // at most every 33 ms of wall time, so the moment with only one half known is not always seen. Whatever is seen while locked is
    // 0 (not yet known) or the whole id, never one half.
    assert!(!cells.is_empty());
    assert!(cells.iter().all(|&c| c == 0 || c == 0x1234), "{cells:?}");
    assert_eq!(num(&f, "cell_id") as u32, 0x1234);
    assert_eq!(f.fields().at(&F::TPS_LENGTH), Some(31));
    // MER on the data cells against their decisions agrees with the pilot SNR.
    assert!((25.0..35.0).contains(&snr), "snr {snr}");
    assert!((mer - snr).abs() < 3.0, "mer {mer} against snr {snr}");
    // At 30 dB with 16-QAM nearly every hard decision is right, and nothing reaches Reed-Solomon wrong.
    assert!(pre < 2e-3, "ber before Viterbi {pre}");
    assert!(post < 1e-6, "ber after Viterbi {post}");
    // Quality: required C/N of 16-QAM 2/3 is 11.1 dB (Annex A). The margin is SNR minus it. 30 dB is excellent.
    assert!((num(&f, "required_cn_db") - 11.1).abs() < 1e-3);
    assert!((num(&f, "margin_db") - (snr - 11.1)).abs() < 0.05);
    assert!(num(&f, "quality_pct") >= 85.0);
    assert_eq!(f.fields().at(&F::QUALITY_LABEL), Some(Quality::Excellent));
    // The frame duration and the net bit rate.
    let nat = Bandwidth::B8.natural_rate();
    let frame_ms = 68.0 * (2048.0 + 256.0) / nat * 1e3;
    assert!(
        (num(&f, "frame_duration") * 1e3 - frame_ms).abs() < 1e-3,
        "frame duration"
    );
    let net = 1512.0 * 4.0 * (2.0 / 3.0) * (188.0 / 204.0) / ((2048.0 + 256.0) / nat);
    assert!(
        (num(&f, "net_bitrate_bps") - net).abs() < 1.0,
        "net bit rate"
    );
    assert!(num(&f, "rs_blocks") > 0.0);
    assert_eq!(num(&f, "rs_failed"), 0.0);
    assert_eq!(f.fields().at(&F::FEC_SYNC), Some(true));

    // Near the threshold the hard decisions are often wrong while the Viterbi decoder still cleans the stream. Both MER and SNR
    // follow the noise down.
    let mut noisy = c.clone();
    noisy.snr_db = Some(13.0);
    noisy.block = 16384;
    let (g, _) = run_fields(&noisy);
    let (snr2, mer2) = (num(&g, "snr_db"), num(&g, "mer_db"));
    let (pre2, post2) = (num(&g, "ber_pre_viterbi"), num(&g, "ber_post_viterbi"));
    eprintln!(
        "noisy: snr {snr2:.1} mer {mer2:.1} ber pre {pre2:.2e} post {post2:.2e} quality {:?}",
        quality(&g)
    );
    assert!((10.0..16.0).contains(&snr2), "snr {snr2}");
    assert!((mer2 - snr2).abs() < 3.0, "mer {mer2} against snr {snr2}");
    assert!(pre2 > 1e-3 && pre2 < 0.2, "ber before Viterbi {pre2}");
    assert!(
        pre2 > 10.0 * pre.max(1e-6),
        "ber before Viterbi rises with the noise"
    );
    assert!(post2 < 2e-4, "ber after Viterbi {post2} (quasi error free)");
    assert!(num(&g, "quality_pct") < num(&f, "quality_pct"));
    assert!((num(&g, "margin_db") - (snr2 - 11.1)).abs() < 0.05);
}

#[test]
fn automatic_bandwidth_at_every_width() {
    for (bw, want) in [
        (Bandwidth::B8, 7.61e6),
        (Bandwidth::B7, 6.66e6),
        (Bandwidth::B6, 5.71e6),
        (Bandwidth::B5, 4.76e6),
    ] {
        let mut c = Case::new(
            bw,
            tps(Mode::K2, Guard::G8, Constellation::Qam16, CodeRate::R23),
        );
        c.frames = 8;
        c.input_rate = Some(10e6);
        let (sig, rate, _) = signal(&c);
        let mut d = BandwidthDetector::new(rate);
        let mut decided = None;
        for chunk in sig.chunks(16384) {
            if let Some(b) = d.push(chunk) {
                decided = Some(b);
            }
        }
        let width = d.width_hz.unwrap_or(0.0);
        eprintln!(
            "{} MHz: measured {:.2} MHz, decided {:?}, occupancy {:.1} dB",
            bw.mhz(),
            width / 1e6,
            decided.map(|b| b.mhz()),
            d.occupancy_db.unwrap_or(0.0)
        );
        assert_eq!(decided, Some(bw), "{} MHz channel", bw.mhz());
        assert!((width - want).abs() < 0.35e6, "measured width {width}");
    }
    // Noise alone has no flat top: no decision.
    let rate = Bandwidth::B8.natural_rate();
    let mut noise = vec![sdrkit_core::Complex32::default(); 1_000_000];
    sdrkit_dev_test::Awgn::new(1.0, 0.0, rate, rate, 3).process(&mut noise);
    let mut d = BandwidthDetector::new(rate);
    assert!(noise.chunks(16384).all(|ch| d.push(ch).is_none()));
}

/// Shifts `x` (at `rate`) by `hz` and scales it by `db`.
fn shifted(
    x: &[sdrkit_core::Complex32],
    rate: f64,
    hz: f64,
    db: f64,
) -> Vec<sdrkit_core::Complex32> {
    let g = 10f64.powf(db / 20.0);
    x.iter()
        .enumerate()
        .map(|(n, v)| {
            let a = std::f64::consts::TAU * hz * n as f64 / rate;
            *v * sdrkit_core::Complex32::new((g * a.cos()) as f32, (g * a.sin()) as f32)
        })
        .collect()
}

/// Two 7 MHz multiplexes on the channels either side of the wanted one (Sydney's channels 11 and 12 are both on air): the gaps
/// between them are only 0.34 MHz, and the measured width must stay the one channel's (6.66 MHz, a 7 MHz channel), not run on over
/// the neighbours. The same with one strong neighbour on one side only.
#[test]
fn automatic_bandwidth_does_not_merge_neighbouring_multiplexes() {
    let rate = 30e6;
    let mk = |start_frac: f64| {
        let mut c = Case::new(
            Bandwidth::B7,
            tps(Mode::K2, Guard::G8, Constellation::Qam64, CodeRate::R23),
        );
        c.frames = 6;
        c.input_rate = Some(rate);
        c.start_frac = start_frac;
        signal(&c).0
    };
    let wanted = mk(0.0);
    let (a, b) = (mk(0.3), mk(0.6));
    let n = wanted.len().min(a.len()).min(b.len());
    for (lower_db, upper_db) in [
        (Some(0.0), Some(-3.0)),
        (Some(10.0), None),
        (None, Some(6.0)),
    ] {
        let mut x = wanted[..n].to_vec();
        for (src, side, db) in [(&a, -7e6, lower_db), (&b, 7e6, upper_db)] {
            if let Some(db) = db {
                for (v, w) in x.iter_mut().zip(shifted(&src[..n], rate, side, db)) {
                    *v += w;
                }
            }
        }
        sdrkit_dev_test::Awgn::new(0.001, 0.0, rate, rate, 5).process(&mut x);
        let mut d = BandwidthDetector::new(rate);
        let mut decided = None;
        for chunk in x.chunks(16384) {
            if let Some(b) = d.push(chunk) {
                decided = Some(b);
            }
        }
        let width = d.width_hz.unwrap_or(0.0);
        eprintln!(
            "neighbours {lower_db:?} / {upper_db:?} dB: measured {:.2} MHz, decided {:?}",
            width / 1e6,
            decided.map(|b| b.mhz())
        );
        assert_eq!(
            decided,
            Some(Bandwidth::B7),
            "neighbours {lower_db:?} / {upper_db:?}"
        );
        assert!((width - 6.66e6).abs() < 0.35e6, "measured width {width}");
    }
}

/// Auto with a spectrum too noisy to measure (5 dB in the channel): the first guess from the rate (8 MHz at
/// 64/7 Msps) does not lock, and the receiver tries the other standard widths, nearest first, until the 7 MHz multiplex locks.
#[test]
fn automatic_bandwidth_tries_the_other_widths_when_nothing_locks() {
    let mut c = Case::new(
        Bandwidth::B7,
        tps(Mode::K2, Guard::G8, Constellation::Qpsk, CodeRate::R12),
    );
    c.frames = 200;
    c.snr_db = Some(5.0);
    c.input_rate = Some(Bandwidth::B8.natural_rate());
    c.rx_bandwidth = Some(BandwidthChoice::Auto);
    let (sig, rate, _) = signal(&c);
    let (mut rx, _tap) = receiver(&c, rate);
    let mut out = Output::default();
    let mut locked_at = None;
    for (k, chunk) in sig.chunks(65536).enumerate() {
        rx.feed(chunk, &mut out);
        out.clear();
        if k == 0 {
            rx.refresh_results();
        }
        let r = Owner::results(&rx);
        if k == 0 {
            assert_eq!(
                r.fields().at(&F::CHANNEL_WIDTH),
                Some(8e6),
                "the first guess"
            );
            assert_eq!(r.fields().at(&F::BANDWIDTH_FROM), Some(WidthSource::Guess));
        }
        if locked_at.is_none() && r.status().lock_state == LockState::Locked {
            locked_at = Some(k as f64 * 65536.0 / rate);
        }
    }
    rx.finish(&mut out);
    let f = Owner::results(&rx).fields();
    let (width, from) = (f.at(&F::CHANNEL_WIDTH), f.at(&F::BANDWIDTH_FROM));
    eprintln!("locked after {locked_at:?} s at {width:?} Hz ({from:?})");
    assert_eq!(width, Some(7e6));
    assert_eq!(from, Some(WidthSource::Tried));
    assert!(locked_at.is_some_and(|t| t < 4.0), "{locked_at:?}");
}
