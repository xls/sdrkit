//! T2-Lite and MISO: signals of both profiles in SISO and MISO, with the two MISO groups through paths of other gains and
//! delays and with noise, decode without errors; T2-Lite frames in the FEF parts of a T2-base signal and the other way
//! round; FEF parts longer than 22 bits of T2-Lite; and the combinations that annex I refuses.

use super::common::*;
use sdrkit_core::Complex32;
use sdrkit_stack_dvbt2::error::{Rule, T2Error};
use sdrkit_stack_dvbt2::params::*;
use sdrkit_stack_dvbt2::rx::{RxSettings, RxStatus, Stage};
use sdrkit_stack_dvbt2::tx::{MisoOut, Path, Transmitter, TxConfig};

/// Group 1 with a gain of 0.75 and 2 samples of delay, group 2 with 0.62 at another phase and 9 samples: group 2 arrives
/// 7 samples at 1/T after group 1, and the power gains add to 0.95.
const TWO_PATHS: MisoOut = MisoOut::Sum([
    Path {
        gain: Complex32::new(0.7, -0.25),
        delay: 2,
    },
    Path {
        gain: Complex32::new(-0.3, 0.55),
        delay: 9,
    },
]);

/// White noise at `snr_db` below the unit power of the signal.
fn noisy(x: &mut [Complex32], snr_db: f64, seed: u64) {
    sdrkit_algo_sim::noise::Awgn::with_power(10f64.powf(-snr_db / 10.0), seed)
        .unwrap()
        .process(x);
}

/// A single-PLP signal of `profile`, SISO or MISO through [`TWO_PATHS`].
fn config(profile: Profile, miso: bool, shape: (Fft, Guard, PilotPattern), c: (CodeRate, Modulation), blocks: u16) -> TxConfig {
    let frame = if profile == Profile::Lite { FecFrame::Short } else { FecFrame::Normal };
    let rotated = !(profile == Profile::Lite && c.1 == Modulation::Qam256);
    let mut cfg = single(shape.0, shape.1, shape.2, coding(frame, c.0, c.1, rotated), blocks);
    cfg.profile = profile;
    cfg.shape.miso = miso;
    cfg.miso = TWO_PATHS;
    cfg
}

/// Asserts that the receiver locked on `profile` and MISO as `miso` and returned every packet of at least `frames`
/// frames without an error.
fn decoded(st: &RxStatus, check: &sdrkit_proto_mpegts::source::NumberedCheck, profile: Profile, miso: bool, good: u64) {
    assert_eq!(st.stage, Stage::Locked, "{:?}", st.log);
    assert_eq!(st.profile, Some(profile));
    assert_eq!(st.p1.map(|p| p.miso()), Some(miso));
    assert!(
        check.good >= good && check.wrong == 0 && check.flagged == 0 && check.gaps == 0,
        "{profile:?} MISO {miso}: {check:?}"
    );
    assert_eq!(st.blocks_bad, 0, "{profile:?} MISO {miso}");
}

/// Asserts that the receiver found every P1 where the L1 signalling put it: no L1 failure and no new search.
fn followed(st: &RxStatus) {
    assert_eq!(st.l1_errors, 0, "{:?}", st.log);
    assert!(
        !st.log.iter().any(|s| s.contains("P1 lost") || s.contains("searching again")),
        "{:?}",
        st.log
    );
}

#[test]
fn miso_through_two_paths_of_other_gains_and_delays_with_noise() {
    use CodeRate::*;
    use Modulation::*;
    let cases = [
        (Profile::Base, (Fft::K8, Guard::G1_8, PilotPattern::Pp1), (R2_3, Qam16), 2, 20.0, false),
        (Profile::Base, (Fft::K16, Guard::G1_16, PilotPattern::Pp3), (R3_5, Qam64), 2, 20.0, true),
        (Profile::Base, (Fft::K2, Guard::G1_16, PilotPattern::Pp3), (R1_2, Qpsk), 2, 12.0, false),
        (Profile::Lite, (Fft::K16, Guard::G1_16, PilotPattern::Pp3), (R2_5, Qam64), 4, 18.0, false),
        (Profile::Lite, (Fft::K4, Guard::G1_32, PilotPattern::Pp4), (R1_3, Qpsk), 4, 8.0, false),
    ];
    for (i, (profile, shape, c, blocks, snr, extended)) in cases.into_iter().enumerate() {
        let mut cfg = config(profile, true, shape, c, blocks);
        cfg.shape.extended = extended;
        let mut tx = Transmitter::new(cfg).unwrap();
        let mut x = signal(&mut tx, 6, 3000);
        noisy(&mut x, snr, 40 + i as u64);
        let (st, check, _) = receive(&x, RxSettings::default(), 20_000, 0);
        decoded(&st, &check, profile, true, 20);
    }
}

#[test]
fn t2_lite_siso_with_noise() {
    use CodeRate::*;
    use Modulation::*;
    let cases = [
        ((Fft::K16, Guard::G19_256, PilotPattern::Pp2), (R1_3, Qam256), 2, 18.0),
        ((Fft::K8, Guard::G1_32, PilotPattern::Pp7), (R3_5, Qam64), 4, 18.0),
        ((Fft::K2, Guard::G1_4, PilotPattern::Pp1), (R2_5, Qam16), 4, 12.0),
    ];
    for (i, (shape, c, blocks, snr)) in cases.into_iter().enumerate() {
        let mut cfg = config(Profile::Lite, false, shape, c, blocks);
        cfg.post_scrambled = i == 0;
        let mut tx = Transmitter::new(cfg).unwrap();
        assert_eq!(tx.l1_pre().s1, 0b011);
        assert_eq!(tx.l1_pre().post_scrambled, i == 0);
        let mut x = signal(&mut tx, 6, 3000);
        noisy(&mut x, snr, 60 + i as u64);
        let (st, check, _) = receive(&x, RxSettings::default(), 20_000, 0);
        decoded(&st, &check, Profile::Lite, false, 10);
    }
}

/// The samples of a T2-frame of `tx` without its FEF part.
fn frame_only(tx: &mut Transmitter) -> Vec<Complex32> {
    let n = tx.frame_len();
    let mut f = tx.frame().expect("frame");
    f.truncate(n);
    f
}

#[test]
fn t2_lite_frames_in_the_fef_parts_of_t2_base_and_the_other_way_round() {
    use CodeRate::*;
    use Modulation::*;
    // Each profile signals the frames of the other as its FEF parts (annex I.9): one T2-base frame, then one T2-Lite frame.
    let base = |fef: u32| {
        let mut c = config(Profile::Base, false, (Fft::K8, Guard::G1_8, PilotPattern::Pp3), (R1_2, Qam16), 2);
        c.fef = Some((fef, 1));
        c
    };
    let lite = |fef: u32| {
        let mut c = config(Profile::Lite, false, (Fft::K16, Guard::G1_16, PilotPattern::Pp4), (R2_5, Qpsk), 3);
        c.fef = Some((fef, 1));
        c
    };
    let base_len = Transmitter::new(base(10_000)).unwrap().frame_len();
    let lite_len = Transmitter::new(lite(10_000)).unwrap().frame_len();
    for lite_first in [true, false] {
        let mut b = Transmitter::new(base(lite_len as u32)).unwrap();
        let mut l = Transmitter::new(lite(base_len as u32)).unwrap();
        assert_eq!((b.frame_len(), l.frame_len()), (base_len, lite_len));
        let mut x = vec![Complex32::default(); 2000];
        for _ in 0..6 {
            if lite_first {
                x.extend(frame_only(&mut l));
                x.extend(frame_only(&mut b));
            } else {
                x.extend(frame_only(&mut b));
                x.extend(frame_only(&mut l));
            }
        }
        let (st, check, _) = receive(&x, RxSettings::default(), 20_000, 0);
        let profile = if lite_first { Profile::Lite } else { Profile::Base };
        decoded(&st, &check, profile, false, 20);
        followed(&st);
        assert!(st.frames >= 5, "{} {:?}", st.frames, st.log);
    }
}

#[test]
fn a_t2_lite_fef_part_longer_than_22_bits_states_its_high_bits() {
    let mut c = config(
        Profile::Lite,
        false,
        (Fft::K4, Guard::G1_8, PilotPattern::Pp2),
        (CodeRate::R1_2, Modulation::Qpsk),
        2,
    );
    c.bandwidth = Bandwidth::B5;
    let len = (1u32 << 22) + 300_000;
    c.fef = Some((len, 2));
    let mut tx = Transmitter::new(c).unwrap();
    let x = signal(&mut tx, 4, 2000);
    let (st, check, _) = receive(&x, RxSettings::default(), 50_000, 0);
    decoded(&st, &check, Profile::Lite, false, 10);
    followed(&st);
    assert!(st.frames >= 3, "{} {:?}", st.frames, st.log);
}

#[test]
fn annex_i_refuses_what_t2_lite_does_not_use() {
    use CodeRate::*;
    use Modulation::*;
    let refused = |c: TxConfig| Transmitter::new(c).err();
    let rule = |r: Rule| Some(T2Error::Rule(r));
    let shape = (Fft::K8, Guard::G1_8, PilotPattern::Pp3);
    let lite = |c: (CodeRate, Modulation)| config(Profile::Lite, false, shape, c, 2);
    let mut normal = lite((R1_2, Qpsk));
    normal.plps[0].coding.frame = FecFrame::Normal;
    assert_eq!(refused(normal), rule(Rule::LiteFecFrame));
    assert_eq!(refused(lite((R2_3, Qam256))), rule(Rule::LiteConstellation));
    assert_eq!(refused(lite((R3_4, Qam256))), rule(Rule::LiteConstellation));
    let mut turned = lite((R1_2, Qam256));
    turned.plps[0].coding.rotated = true;
    assert_eq!(refused(turned), rule(Rule::LiteConstellation));
    assert_eq!(refused(lite((R4_5, Qam16))), rule(Rule::RateForProfile));
    let mut base = config(Profile::Base, false, shape, (R1_2, Qpsk), 2);
    base.plps[0].coding = coding(FecFrame::Short, R1_3, Qpsk, false);
    assert_eq!(refused(base), rule(Rule::RateForProfile));
    for (fft, guard, pp) in [
        (Fft::K32, Guard::G1_8, PilotPattern::Pp2),
        (Fft::K1, Guard::G1_8, PilotPattern::Pp2),
        (Fft::K8, Guard::G1_8, PilotPattern::Pp8),
        (Fft::K16, Guard::G1_32, PilotPattern::Pp4),
    ] {
        let c = config(Profile::Lite, false, (fft, guard, pp), (R1_2, Qpsk), 2);
        assert_eq!(refused(c), rule(Rule::PilotsForGuard), "{fft:?} {guard:?} {pp:?}");
    }
    let mut long = lite((R1_2, Qpsk));
    long.fef = Some((1 << 24, 1));
    assert_eq!(refused(long), rule(Rule::FefLength));
    let mut long = config(Profile::Base, false, shape, (R1_2, Qpsk), 2);
    long.fef = Some((1 << 22, 1));
    assert_eq!(refused(long), rule(Rule::FefLength));
    // 1/2 QPSK at 10 MHz in 2K: 40 FEC blocks in 230 data symbols carry more than 4 Mbit/s. Without a stated L_data the
    // transmitter takes more symbols.
    let fast = |l_data: usize| {
        let mut c = config(Profile::Lite, false, (Fft::K2, Guard::G1_32, PilotPattern::Pp7), (R1_2, Qpsk), 40);
        c.bandwidth = Bandwidth::B10;
        c.shape.l_data = l_data;
        c
    };
    assert_eq!(refused(fast(230)), rule(Rule::LiteBitRate));
    let tx = Transmitter::new(fast(0)).unwrap();
    let frame_s = tx.frame_len() as f64 / tx.rate();
    assert!(tx.plp_bits_per_frame() as f64 / frame_s <= 4.0e6);
}
