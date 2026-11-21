//! The one test program of sdrkit-proto-tones: known answers from the standards, the receiver limits, cuts, components and the
//! allocation count of every streaming path.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_proto_tones::gen::ToneSequence;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// An allocator that counts the allocations of the current thread (tests run on several threads).
struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// The allocations `f` makes on this thread.
fn allocations(f: impl FnOnce()) -> usize {
    let before = ALLOCS.with(Cell::get);
    f();
    ALLOCS.with(Cell::get) - before
}

const RATE: f64 = 8000.0;

/// The whole sequence, then `pad_s` of silence.
fn render(mut s: ToneSequence, pad_s: f64) -> Vec<f32> {
    let mut x = vec![0f32; s.len() as usize + (pad_s * s.rate()) as usize];
    s.fill(&mut x);
    x
}

/// Adds white noise of standard deviation `sd`.
fn noisy(mut x: Vec<f32>, sd: f32, seed: u64) -> Vec<f32> {
    let mut r = Rng::new(seed);
    x.iter_mut().for_each(|v| *v += sd * r.normal_f32());
    x
}

/// Concatenates pieces of audio.
fn join(parts: &[Vec<f32>]) -> Vec<f32> {
    parts.concat()
}

mod gen {
    use super::*;
    use sdrkit_algo_dsp::goertzel::goertzel;
    use sdrkit_proto_tones::gen::Step;

    #[test]
    fn any_cut_returns_the_same_samples() {
        let steps = [
            Step::one(1000.0, 0.5, 0.05),
            Step::two([697.0, 1209.0], [0.3, 0.2], 0.04),
            Step::silence(0.01),
            Step {
                phase_step: [0.5, 0.0],
                ..Step::one(1000.0, 0.5, 0.03)
            },
        ];
        let whole = render(ToneSequence::new(RATE, &steps, 0.002).unwrap(), 0.0);
        for cut in [1, 7, 333] {
            let mut s = ToneSequence::new(RATE, &steps, 0.002).unwrap();
            let mut got = vec![0f32; whole.len()];
            for c in got.chunks_mut(cut) {
                s.fill(c);
            }
            assert_eq!(got, whole, "{cut}");
        }
    }

    #[test]
    fn tones_amplitudes_and_a_phase_reversal() {
        let x = render(
            ToneSequence::new(
                RATE,
                &[
                    Step::one(500.0, 0.4, 0.1),
                    Step {
                        phase_step: [0.5, 0.0],
                        ..Step::one(500.0, 0.4, 0.1)
                    },
                ],
                0.0,
            )
            .unwrap(),
            0.0,
        );
        let a = goertzel(&x[0..400], 500.0 / RATE);
        let b = goertzel(&x[800..1200], 500.0 / RATE);
        // Amplitude: N A / 2.
        assert!((a.norm() - 80.0).abs() < 0.5, "{}", a.norm());
        // The second window starts 800 samples later: 50 whole cycles, so only the reversal changes the phase.
        let d = (b * a.conj()).arg().abs();
        assert!((d - std::f64::consts::PI).abs() < 0.01, "{d}");
    }

    #[test]
    fn bad_steps_are_refused() {
        assert!(ToneSequence::new(RATE, &[Step::one(4000.0, 0.1, 0.1)], 0.0).is_err());
        assert!(ToneSequence::new(RATE, &[Step::one(100.0, 0.1, -1.0)], 0.0).is_err());
        assert!(ToneSequence::new(RATE, &[Step::one(100.0, f32::NAN, 1.0)], 0.0).is_err());
        assert!(ToneSequence::new(0.0, &[], 0.0).is_err());
        assert!(ToneSequence::new(RATE, &[], 0.0).unwrap().is_empty());
    }
}

mod ctcss {
    use super::*;
    use sdrkit_proto_tones::ctcss::{
        ctcss_sequence, CtcssConfig, CtcssDecoder, CtcssEvent, CtcssEventKind,
    };
    use sdrkit_util_ids::tones::ctcss::TONES;

    fn run(x: &[f32], cfg: CtcssConfig) -> Vec<CtcssEvent> {
        let mut d = CtcssDecoder::new(RATE, cfg).unwrap();
        let mut v = Vec::new();
        d.process(x, |e| v.push(e));
        v
    }

    #[test]
    fn every_tone_of_the_table_locks_and_is_measured() {
        // Each of the 50 tones (the 38 of EIA RS-220-A and the 12 others), 0.15 amplitude, a little noise.
        for (i, t) in TONES.iter().enumerate() {
            let x = noisy(
                render(
                    ctcss_sequence(RATE, t.hz as f64, 0.15, 2.0, None).unwrap(),
                    0.0,
                ),
                0.01,
                i as u64,
            );
            let e = run(&x, CtcssConfig::default());
            assert!(!e.is_empty(), "{}", t.hz);
            assert_eq!((e[0].kind, e[0].tone.hz), (CtcssEventKind::Locked, t.hz));
            assert!(
                (e[0].measured_hz - t.hz).abs() < 0.3,
                "{} {}",
                t.hz,
                e[0].measured_hz
            );
            assert!((e[0].level_db - (-16.48)).abs() < 0.5, "{}", e[0].level_db);
            // About 1.3 s to lock.
            assert!((1.0..1.6).contains(&(e[0].at as f64 / RATE)), "{}", e[0].at);
        }
    }

    #[test]
    fn the_closest_tones_and_an_offset_tone() {
        // 159.8 and 162.2 Hz are 2.4 Hz apart.
        for hz in [159.8, 162.2] {
            let e = run(
                &render(ctcss_sequence(RATE, hz, 0.15, 2.0, None).unwrap(), 0.0),
                CtcssConfig::default(),
            );
            assert_eq!(e[0].tone.hz, hz as f32);
        }
        // A tone 0.8 Hz high still reads as its tone, and the measure shows the offset.
        let e = run(
            &render(ctcss_sequence(RATE, 100.8, 0.15, 2.0, None).unwrap(), 0.0),
            CtcssConfig::default(),
        );
        assert_eq!(e[0].tone.hz, 100.0);
        assert!(
            (e[0].measured_hz - 100.8).abs() < 0.2,
            "{}",
            e[0].measured_hz
        );
        // The 12 further tones are not read with eia_only.
        let cfg = CtcssConfig {
            eia_only: true,
            ..CtcssConfig::default()
        };
        let e = run(
            &render(ctcss_sequence(RATE, 165.5, 0.15, 2.0, None).unwrap(), 0.0),
            cfg,
        );
        assert!(e.iter().all(|e| e.tone.hz != 165.5));
    }

    #[test]
    fn reverse_burst_ends_at_once_and_a_plain_end_releases_later() {
        for cycles in [0.5, 1.0 / 3.0, 2.0 / 3.0] {
            let x = render(
                ctcss_sequence(RATE, 131.8, 0.15, 3.0, Some((cycles, 0.18))).unwrap(),
                2.0,
            );
            let e = run(&x, CtcssConfig::default());
            assert_eq!(e.len(), 2, "{cycles}");
            assert_eq!(e[1].kind, CtcssEventKind::TurnOff, "{cycles}");
            // Within 0.3 s of the burst's start (3.0 s).
            assert!(
                (3.0..3.3).contains(&(e[1].at as f64 / RATE)),
                "{}",
                e[1].at as f64 / RATE
            );
        }
        let x = render(ctcss_sequence(RATE, 131.8, 0.15, 3.0, None).unwrap(), 2.0);
        let e = run(&x, CtcssConfig::default());
        assert_eq!(
            e.iter().map(|e| e.kind).collect::<Vec<_>>(),
            [CtcssEventKind::Locked, CtcssEventKind::Released]
        );
        // Without the turn-off search, a burst is only a release.
        let x = render(
            ctcss_sequence(RATE, 131.8, 0.15, 3.0, Some((0.5, 0.18))).unwrap(),
            2.0,
        );
        let cfg = CtcssConfig {
            turn_off: false,
            ..CtcssConfig::default()
        };
        assert_eq!(run(&x, cfg)[1].kind, CtcssEventKind::Released);
    }

    #[test]
    fn voice_above_300_hz_does_not_hide_the_tone_and_noise_alone_returns_nothing() {
        let tone = render(ctcss_sequence(RATE, 88.5, 0.1, 3.0, None).unwrap(), 0.0);
        let voice = render(
            sdrkit_proto_tones::gen::ToneSequence::new(
                RATE,
                &[
                    sdrkit_proto_tones::gen::Step::two([450.0, 1100.0], [0.4, 0.3], 1.5),
                    sdrkit_proto_tones::gen::Step::two([700.0, 2300.0], [0.4, 0.3], 1.5),
                ],
                0.01,
            )
            .unwrap(),
            0.0,
        );
        let x: Vec<f32> = tone.iter().zip(&voice).map(|(a, b)| a + b).collect();
        let e = run(&noisy(x, 0.05, 3), CtcssConfig::default());
        assert_eq!((e[0].kind, e[0].tone.hz), (CtcssEventKind::Locked, 88.5));
        let e = run(&noisy(vec![0.0; 40000], 0.2, 4), CtcssConfig::default());
        assert!(e.is_empty());
    }

    #[test]
    fn any_cut_returns_the_same_events() {
        let x = render(
            ctcss_sequence(RATE, 107.2, 0.15, 2.5, Some((0.5, 0.2))).unwrap(),
            1.0,
        );
        let whole = run(&x, CtcssConfig::default());
        for cut in [1, 100, 4097] {
            let mut d = CtcssDecoder::new(RATE, CtcssConfig::default()).unwrap();
            let mut v = Vec::new();
            for c in x.chunks(cut) {
                d.process(c, |e| v.push(e));
            }
            assert_eq!(v, whole, "{cut}");
        }
    }

    #[test]
    fn bad_settings_are_refused() {
        let ok = CtcssConfig::default();
        assert!(CtcssDecoder::new(999.0, ok).is_err());
        assert!(CtcssDecoder::new(f64::NAN, ok).is_err());
        for c in [
            CtcssConfig {
                window_s: 0.1,
                ..ok
            },
            CtcssConfig { hop_s: 2.0, ..ok },
            CtcssConfig {
                min_share: 1.5,
                ..ok
            },
            CtcssConfig {
                dominance: 0.5,
                ..ok
            },
            CtcssConfig { lock_hops: 0, ..ok },
            CtcssConfig {
                min_level_db: f32::NAN,
                ..ok
            },
        ] {
            assert!(CtcssDecoder::new(RATE, c).is_err(), "{c:?}");
        }
        // Samples that are not numbers count as 0.
        let mut d = CtcssDecoder::new(RATE, ok).unwrap();
        d.process(&[f32::NAN, f32::INFINITY, 0.0], |_| panic!());
    }
}

mod dcs {
    use super::*;
    use sdrkit_proto_tones::dcs::{DcsConfig, DcsDecoder, DcsEncoder, DcsEvent, DcsEventKind};
    use sdrkit_util_ids::tones::dcs::{Dcs, STANDARD};

    fn audio(code: &str, inverted: bool, seconds: f64, turn_off: bool) -> Vec<f32> {
        let mut e = DcsEncoder::new(
            RATE,
            Dcs::from_octal(code).unwrap(),
            inverted,
            0.2,
            seconds,
            turn_off,
        )
        .unwrap();
        let mut x = vec![0f32; e.len() as usize + 16000];
        e.fill(&mut x);
        x
    }

    fn run(x: &[f32]) -> Vec<DcsEvent> {
        let mut d = DcsDecoder::new(RATE, DcsConfig::default()).unwrap();
        let mut v = Vec::new();
        d.process(x, |e| v.push(e));
        v
    }

    /// A DCS word as a polynomial of sdrkit-algo-fec (bit i is the coefficient of x^i). The 12 data bits are the high terms and the 11
    /// check bits the low terms. On the air each part goes lowest term first.
    fn as_polynomial(w: u32) -> u128 {
        (((w & 0xFFF) << 11) | (w >> 12)) as u128
    }

    #[test]
    fn the_word_of_023_and_the_golay_code_of_sdrkit_fec() {
        // D023: the published word 0x763813 (bit i = i-th bit sent).
        let d023 = Dcs::from_octal("023").unwrap();
        assert_eq!(d023.word(), 0x76_3813);
        // Every DCS word is a codeword of the (23,12) Golay code of sdrkit-algo-fec (generator 0xC75, TIA-603).
        let g = sdrkit_algo_fec::small::SmallCode::golay_23_12();
        for v in 0..=0o777u16 {
            let w = as_polynomial(Dcs::new(v).unwrap().word());
            assert_eq!(g.syndrome(w), 0, "{v:o}");
            assert_eq!(g.encode(g.info(w)), w);
        }
    }

    #[test]
    fn every_standard_code_is_read_at_normal_polarity() {
        for (i, &c) in STANDARD.iter().enumerate() {
            let code = Dcs::new(c).unwrap();
            let mut e = DcsEncoder::new(RATE, code, false, 0.2, 1.2, false).unwrap();
            let mut x = vec![0f32; e.len() as usize];
            e.fill(&mut x);
            let ev = run(&noisy(x, 0.01, i as u64));
            assert!(!ev.is_empty(), "{code}");
            assert_eq!(ev[0].kind, DcsEventKind::Locked);
            assert_eq!(
                (ev[0].reading.code, ev[0].reading.inverted),
                (code, false),
                "{code}"
            );
        }
    }

    #[test]
    fn octal_codes_are_octal() {
        // The general crate read "023" as decimal 23 (= 0o27). The shared decoder takes the octal code type.
        let d023 = Dcs::from_octal("023").unwrap();
        assert_eq!(d023.value(), 19);
        assert_ne!(d023, Dcs::new(23).unwrap());
        let ev = run(&audio("023", false, 1.5, false));
        assert_eq!(ev[0].reading.name(), "D023N");
        assert_eq!(
            ev[0].reading.alias.map(|a| (a.0.to_string(), a.1)),
            Some(("D047".to_string(), true))
        );
    }

    #[test]
    fn inverted_codes_read_as_their_normal_twin() {
        // D023 sent inverted is the same signal as D047 normal (published pairs).
        let ev = run(&audio("023", true, 1.5, false));
        assert_eq!(ev[0].reading.name(), "D047N");
        assert_eq!(
            ev[0].reading.alias.map(|a| (a.0.to_string(), a.1)),
            Some(("D023".to_string(), true))
        );
        // D172 inverted reads only as codes outside the 83 at normal polarity. The alias then names 172 inverted.
        let ev = run(&audio("172", true, 1.5, false));
        assert!(
            !ev[0].reading.inverted && !ev[0].reading.code.is_standard(),
            "{:?}",
            ev[0].reading
        );
        assert_eq!(
            ev[0].reading.alias.map(|a| (a.0.to_string(), a.1)),
            Some(("D172".to_string(), true))
        );
    }

    #[test]
    fn turn_off_and_release() {
        let ev = run(&audio("754", false, 2.0, true));
        assert_eq!(
            ev.iter().map(|e| e.kind).collect::<Vec<_>>(),
            [DcsEventKind::Locked, DcsEventKind::TurnOff]
        );
        assert!(
            (2.0..2.3).contains(&(ev[1].at as f64 / RATE)),
            "{}",
            ev[1].at as f64 / RATE
        );
        let ev = run(&audio("754", false, 2.0, false));
        assert_eq!(
            ev.iter().map(|e| e.kind).collect::<Vec<_>>(),
            [DcsEventKind::Locked, DcsEventKind::Released]
        );
        assert!(ev[1].at as f64 / RATE > 3.4);
    }

    #[test]
    fn noise_and_a_ctcss_tone_are_not_codes() {
        assert!(run(&noisy(vec![0.0; 40000], 0.1, 9)).is_empty());
        let tone = render(
            sdrkit_proto_tones::ctcss::ctcss_sequence(RATE, 134.4, 0.2, 4.0, None).unwrap(),
            0.0,
        );
        assert!(run(&tone).is_empty());
    }

    #[test]
    fn any_cut_returns_the_same_events() {
        let x = audio("516", false, 2.0, true);
        let whole = run(&x);
        for cut in [1, 64, 5000] {
            let mut d = DcsDecoder::new(RATE, DcsConfig::default()).unwrap();
            let mut v = Vec::new();
            for c in x.chunks(cut) {
                d.process(c, |e| v.push(e));
            }
            assert_eq!(v, whole, "{cut}");
        }
        assert!(DcsDecoder::new(500.0, DcsConfig::default()).is_err());
        assert!(DcsDecoder::new(
            RATE,
            DcsConfig {
                min_swing: 0.0,
                ..DcsConfig::default()
            }
        )
        .is_err());
        assert!(DcsEncoder::new(RATE, Dcs::new(0o23).unwrap(), false, 0.1, -1.0, false).is_err());
    }
}

mod dtmf {
    use super::*;
    use sdrkit_proto_tones::dtmf::{dtmf_sequence, DtmfConfig, DtmfDecoder, DtmfEvent};
    use sdrkit_proto_tones::gen::Step;
    use sdrkit_util_ids::tones::dtmf::{tones, KEYS};

    fn run(x: &[f32]) -> Vec<DtmfEvent> {
        let mut d = DtmfDecoder::new(RATE, DtmfConfig::default()).unwrap();
        let mut v = Vec::new();
        d.process(x, |e| v.push(e));
        v
    }

    fn keys(x: &[f32]) -> String {
        run(x).iter().filter(|e| e.down).map(|e| e.key).collect()
    }

    /// One key with its tones moved by the fractions `dr` and `dc`, at amplitudes `ar` and `ac`, for `s` seconds.
    fn key(k: char, dr: f64, dc: f64, ar: f32, ac: f32, s: f64) -> Vec<f32> {
        let (r, c) = tones(k).unwrap();
        render(
            ToneSequence::new(
                RATE,
                &[
                    Step::silence(0.05),
                    Step::two([r as f64 * (1.0 + dr), c as f64 * (1.0 + dc)], [ar, ac], s),
                ],
                0.002,
            )
            .unwrap(),
            0.1,
        )
    }

    #[test]
    fn the_16_keys_of_q23() {
        let all: String = KEYS.iter().flatten().collect();
        let x = noisy(
            render(
                dtmf_sequence(RATE, &all, 0.2, 0.2, 0.05, 0.05).unwrap(),
                0.1,
            ),
            0.02,
            1,
        );
        assert_eq!(keys(&x), all);
        let ev = run(&x);
        assert_eq!(ev.len(), 32, "every key goes down and up");
        assert!(ev
            .iter()
            .filter(|e| e.down)
            .all(|e| (e.row_db + 14.0).abs() < 1.0 && (e.column_db + 14.0).abs() < 1.0));
        // Lower case A to D.
        assert_eq!(
            keys(&render(
                dtmf_sequence(RATE, "abcd", 0.2, 0.2, 0.05, 0.05).unwrap(),
                0.1
            )),
            "ABCD"
        );
    }

    #[test]
    fn q24_frequency_limits() {
        // Q.24: a deviation of 1.5 % must operate the receiver, 3.5 % must not.
        for k in ['1', '5', '9', 'D', '*', '0'] {
            for d in [-0.015, 0.015] {
                assert_eq!(
                    keys(&key(k, d, 0.0, 0.2, 0.2, 0.06)),
                    k.to_string(),
                    "{k} row {d}"
                );
                assert_eq!(
                    keys(&key(k, 0.0, d, 0.2, 0.2, 0.06)),
                    k.to_string(),
                    "{k} column {d}"
                );
            }
            for d in [-0.035, 0.035] {
                assert_eq!(keys(&key(k, d, 0.0, 0.2, 0.2, 0.06)), "", "{k} row {d}");
                assert_eq!(keys(&key(k, 0.0, d, 0.2, 0.2, 0.06)), "", "{k} column {d}");
            }
        }
    }

    #[test]
    fn q24_timing_limits() {
        // 40 ms operates, 20 ms does not.
        for k in ['2', 'C', '#'] {
            assert_eq!(keys(&key(k, 0.0, 0.0, 0.2, 0.2, 0.040)), k.to_string());
            assert_eq!(keys(&key(k, 0.0, 0.0, 0.2, 0.2, 0.020)), "");
        }
        // A pause of 40 ms separates two equal keys.
        assert_eq!(
            keys(&render(
                dtmf_sequence(RATE, "77", 0.2, 0.2, 0.05, 0.04).unwrap(),
                0.1
            )),
            "77"
        );
        // An interruption of 10 ms is not a pause.
        let (r, c) = tones('7').unwrap();
        let x = render(
            ToneSequence::new(
                RATE,
                &[
                    Step::two([r as f64, c as f64], [0.2, 0.2], 0.06),
                    Step::silence(0.010),
                    Step::two([r as f64, c as f64], [0.2, 0.2], 0.06),
                ],
                0.0,
            )
            .unwrap(),
            0.1,
        );
        assert_eq!(keys(&x), "7");
    }

    #[test]
    fn twist_and_levels() {
        let db = |d: f32| 0.2 * 10f32.powf(d / 20.0);
        // Column 4 dB above the row: accepted. 6 dB: refused.
        assert_eq!(keys(&key('8', 0.0, 0.0, 0.2, db(3.9), 0.06)), "8");
        assert_eq!(keys(&key('8', 0.0, 0.0, 0.2, db(6.0), 0.06)), "");
        // Column 8 dB below: accepted. 10 dB: refused.
        assert_eq!(keys(&key('8', 0.0, 0.0, 0.2, db(-7.9), 0.06)), "8");
        assert_eq!(keys(&key('8', 0.0, 0.0, 0.2, db(-10.0), 0.06)), "");
        // Too weak for the floor of -45 dB (amplitude 0.004 is -48 dB).
        assert_eq!(keys(&key('8', 0.0, 0.0, 0.004, 0.004, 0.06)), "");
        // A weak key well above the noise is read.
        assert_eq!(
            keys(&noisy(key('8', 0.0, 0.0, 0.02, 0.02, 0.06), 0.002, 5)),
            "8"
        );
    }

    #[test]
    fn harmonics_and_a_third_tone_are_refused() {
        let (r, c) = tones('3').unwrap();
        let (r, c) = (r as f64, c as f64);
        // A strong second harmonic of the row tone (as in speech).
        let harm = render(
            ToneSequence::new(
                RATE,
                &[Step::two([r, c], [0.2, 0.2], 0.08), Step::silence(0.1)],
                0.002,
            )
            .unwrap(),
            0.0,
        );
        let h2 = render(
            ToneSequence::new(
                RATE,
                &[Step::one(2.0 * r, 0.12, 0.08), Step::silence(0.1)],
                0.002,
            )
            .unwrap(),
            0.0,
        );
        let x: Vec<f32> = harm.iter().zip(&h2).map(|(a, b)| a + b).collect();
        assert_eq!(keys(&x), "");
        let mut d = DtmfDecoder::new(
            RATE,
            DtmfConfig {
                harmonics: false,
                ..DtmfConfig::default()
            },
        )
        .unwrap();
        let mut got = String::new();
        d.process(&x, |e| {
            if e.down {
                got.push(e.key)
            }
        });
        assert_eq!(got, "3");
        // Two row tones at once.
        let two_rows = render(
            ToneSequence::new(RATE, &[Step::two([697.0, 770.0], [0.2, 0.2], 0.08)], 0.002).unwrap(),
            0.0,
        );
        assert_eq!(keys(&two_rows), "");
        let _ = c;
    }

    #[test]
    fn other_rates_and_cuts() {
        for rate in [11025.0, 16000.0, 48000.0] {
            let mut s = dtmf_sequence(rate, "0123", 0.2, 0.2, 0.05, 0.05).unwrap();
            let mut x = vec![0f32; s.len() as usize];
            s.fill(&mut x);
            let mut d = DtmfDecoder::new(rate, DtmfConfig::default()).unwrap();
            let mut got = String::new();
            d.process(&x, |e| {
                if e.down {
                    got.push(e.key)
                }
            });
            assert_eq!(got, "0123", "{rate}");
        }
        let x = render(
            dtmf_sequence(RATE, "5#9", 0.2, 0.2, 0.05, 0.05).unwrap(),
            0.1,
        );
        let whole = run(&x);
        for cut in [1, 51, 1000] {
            let mut d = DtmfDecoder::new(RATE, DtmfConfig::default()).unwrap();
            let mut v = Vec::new();
            for c in x.chunks(cut) {
                d.process(c, |e| v.push(e));
            }
            assert_eq!(v, whole);
        }
        assert!(DtmfDecoder::new(3000.0, DtmfConfig::default()).is_err());
        assert!(dtmf_sequence(RATE, "12E", 0.1, 0.1, 0.05, 0.05).is_err());
    }
}

mod selcall {
    use super::*;
    use sdrkit_proto_tones::selcall::{
        selcall_sequence, SelcallCall, SelcallConfig, SelcallDecoder,
    };
    use sdrkit_util_ids::tones::selcall::{by_name, TONE_SETS};

    fn run(x: &[f32], cfg: SelcallConfig) -> Vec<SelcallCall> {
        let mut d = SelcallDecoder::new(RATE, cfg).unwrap();
        let mut v = Vec::new();
        d.process(x, |c| v.push(c));
        v
    }

    #[test]
    fn the_tones_of_a_call() {
        // ZVEI1 "12345": 1060, 1160, 1270, 1400, 1530 Hz, 70 ms each. "22" is sent as 2 E (2600 Hz).
        let z1 = by_name("ZVEI1").unwrap();
        for (digits, want) in [
            ("12345", [1060.0, 1160.0, 1270.0, 1400.0, 1530.0]),
            ("22345", [1160.0, 2600.0, 1270.0, 1400.0, 1530.0]),
        ] {
            let x = render(selcall_sequence(RATE, z1, digits, 0.3, None).unwrap(), 0.0);
            assert_eq!(x.len(), 2800);
            for (k, &f) in want.iter().enumerate() {
                let seg = &x[k * 560 + 40..(k + 1) * 560 - 40];
                let p = sdrkit_algo_dsp::goertzel::goertzel(seg, f / RATE).norm()
                    / (seg.len() as f64 / 2.0);
                assert!((p - 0.3).abs() < 0.02, "{digits} {k} {p}");
            }
        }
    }

    #[test]
    fn every_set_reads_its_calls() {
        for (i, set) in TONE_SETS.iter().enumerate() {
            for digits in ["12345", "55123", "90817"] {
                let x = noisy(
                    render(selcall_sequence(RATE, set, digits, 0.3, None).unwrap(), 0.5),
                    0.01,
                    i as u64,
                );
                // With only its own set.
                let calls = run(&x, SelcallConfig::only(set.name).unwrap());
                assert_eq!(calls.len(), 1, "{} {digits}", set.name);
                assert_eq!(calls[0].digits(), digits);
                assert_eq!(calls[0].set.name, set.name);
                // With every set: one call. The tones of some calls fit several sets (ZVEI3 "55123" is also ZVEI2 "40012"), so
                // either the call is read in this set, or this set is among the others the tones fit.
                let calls = run(&x, SelcallConfig::default());
                assert_eq!(calls.len(), 1, "{} {digits}", set.name);
                if calls[0].set.name == set.name {
                    assert_eq!(calls[0].digits(), digits);
                } else {
                    assert!(
                        calls[0].also_fits & (1 << i) != 0,
                        "{} read as {}",
                        set.name,
                        calls[0].set.name
                    );
                }
            }
        }
    }

    #[test]
    fn zvei2_and_zvei3_are_two_sets() {
        // The trunk crate merged ZVEI2 and ZVEI3. ZVEI3 has 2200 Hz for 0 and a 2400 Hz repeat tone. ZVEI2 has the ZVEI1 digits
        // and a 970 Hz repeat tone.
        let z3 = by_name("ZVEI3").unwrap();
        assert_eq!((z3.tone('0'), z3.repeat_hz), (Some(2200.0), 2400.0));
        let x = render(selcall_sequence(RATE, z3, "10022", 0.3, None).unwrap(), 0.5);
        let calls = run(&x, SelcallConfig::only("ZVEI3").unwrap());
        assert_eq!((calls[0].digits(), calls[0].sent()), ("10022", "10E2E"));
        assert!(run(&x, SelcallConfig::only("ZVEI2").unwrap()).is_empty());
        let z2 = by_name("ZVEI2").unwrap();
        let x = render(selcall_sequence(RATE, z2, "10022", 0.3, None).unwrap(), 0.5);
        let calls = run(&x, SelcallConfig::only("ZVEI2").unwrap());
        assert_eq!(calls[0].digits(), "10022");
        // The same tones are another call in ZVEI3 (970 Hz is its digit 1).
        assert_eq!(
            run(&x, SelcallConfig::only("ZVEI3").unwrap())[0].digits(),
            "22131"
        );
    }

    #[test]
    fn group_tone_long_tones_and_short_groups() {
        let z1 = by_name("ZVEI1").unwrap();
        let x = render(selcall_sequence(RATE, z1, "123A5", 0.3, None).unwrap(), 0.5);
        assert_eq!(
            run(&x, SelcallConfig::only("ZVEI1").unwrap())[0].digits(),
            "123A5"
        );
        // A tone of 1 s is no selcall tone.
        let x = render(
            selcall_sequence(RATE, z1, "123", 0.3, Some(1.0)).unwrap(),
            0.5,
        );
        assert!(run(&x, SelcallConfig::default()).is_empty());
        // Two tones are too few.
        let x = render(selcall_sequence(RATE, z1, "12", 0.3, None).unwrap(), 0.5);
        assert!(run(&x, SelcallConfig::default()).is_empty());
        assert!(selcall_sequence(RATE, by_name("ZVEI2").unwrap(), "1A", 0.3, None).is_err());
        assert!(selcall_sequence(RATE, z1, "12x", 0.3, None).is_err());
        assert!(SelcallConfig::only("nope").is_none());
        assert!(SelcallDecoder::new(4000.0, SelcallConfig::default()).is_err());
    }

    #[test]
    fn any_cut_returns_the_same_calls() {
        let ccir = by_name("CCIR1").unwrap();
        let x = join(&[
            render(
                selcall_sequence(RATE, ccir, "13579", 0.3, None).unwrap(),
                0.5,
            ),
            render(
                selcall_sequence(RATE, ccir, "24680", 0.3, None).unwrap(),
                0.5,
            ),
        ]);
        let whole = run(&x, SelcallConfig::default());
        assert_eq!(whole.len(), 2);
        for cut in [1, 64, 3000] {
            let mut d = SelcallDecoder::new(RATE, SelcallConfig::default()).unwrap();
            let mut v = Vec::new();
            for c in x.chunks(cut) {
                d.process(c, |e| v.push(e));
            }
            assert_eq!(v, whole);
        }
    }
}

mod selcal {
    use super::*;
    use sdrkit_proto_tones::selcal::{
        selcal_sequence, SelcalCall, SelcalConfig, SelcalDecoder, STACK_ID,
    };
    use sdrkit_util_ids::tones::selcal::Code;

    fn run(x: &[f32], cfg: SelcalConfig) -> Vec<SelcalCall> {
        let mut d = SelcalDecoder::new(RATE, cfg).unwrap();
        let mut v = Vec::new();
        d.process(x, |c| v.push(c));
        v
    }

    fn call(code: &str, pulse: f64, gap: f64, shift: f64) -> Vec<f32> {
        render(
            selcal_sequence(RATE, Code::parse(code).unwrap(), 0.25, pulse, gap, shift).unwrap(),
            1.0,
        )
    }

    #[test]
    fn asri_tones_of_a_code() {
        // AM-CP: A 312.6, M 977.2, C 384.6, P 1083.9 Hz (ASRI Table 2-1), 1 s pulses 0.2 s apart (ARINC 714).
        let x = call("AM-CP", 1.0, 0.2, 0.0);
        let amp = |a: usize, f: f64| {
            let seg = &x[a..a + 4000];
            sdrkit_algo_dsp::goertzel::goertzel(seg, f / RATE).norm() / 2000.0
        };
        for f in [312.6, 977.2] {
            assert!((amp(2000, f) - 0.25).abs() < 0.01);
        }
        for f in [384.6, 1083.9] {
            assert!((amp(11600, f) - 0.25).abs() < 0.01);
            assert!(amp(2000, f) < 0.01);
        }
        assert_eq!(STACK_ID, "selcal");
        assert_eq!(sdrkit_proto_tones::SELCAL_STACK_ID, "selcal");
    }

    #[test]
    fn classic_and_new_codes() {
        for code in ["AM-CP", "DH-KS", "BE-FQ", "AS-JR", "GL-HP"] {
            let c = run(
                &noisy(call(code, 1.0, 0.2, 0.0), 0.02, 1),
                SelcalConfig::default(),
            );
            assert_eq!(c.len(), 1, "{code}");
            assert_eq!(c[0].code.to_string(), code);
            assert!((c[0].level_db - (-12.04)).abs() < 0.5, "{}", c[0].level_db);
        }
        // Tones of the 2022 set: T, U, 9 and 1.
        let c = run(&call("AT-U9", 1.0, 0.2, 0.0), SelcalConfig::default());
        assert_eq!(c[0].code.to_string(), "AT-U9");
        let c = run(&call("1B-CW", 1.0, 0.2, 0.0), SelcalConfig::default());
        assert_eq!(
            c[0].code.to_string(),
            "B1-CW",
            "each pair lower frequency first"
        );
        // Not with the 16 classic tones only.
        let cfg = SelcalConfig {
            classic_only: true,
            window_s: 0.1,
            ..SelcalConfig::default()
        };
        assert!(run(&call("AT-U9", 1.0, 0.2, 0.0), cfg).is_empty());
        assert_eq!(
            run(&call("AM-CP", 1.0, 0.2, 0.0), cfg)[0].code.to_string(),
            "AM-CP"
        );
    }

    #[test]
    fn arinc_714_timing() {
        // Pulses of 0.75 to 1.25 s and gaps of 0.1 to 0.3 s are calls.
        for (p, g) in [(0.75, 0.1), (1.25, 0.3), (1.0, 0.2)] {
            assert_eq!(
                run(&call("EK-LQ", p, g, 0.0), SelcalConfig::default()).len(),
                1,
                "{p} {g}"
            );
        }
        // Pulses of 0.3 s or a gap of 1.5 s are not.
        assert!(run(&call("EK-LQ", 0.3, 0.2, 0.0), SelcalConfig::default()).is_empty());
        assert!(run(&call("EK-LQ", 1.0, 1.5, 0.0), SelcalConfig::default()).is_empty());
        // The same pair twice is not a call.
        assert!(run(&call("EK-EK", 1.0, 0.2, 0.0), SelcalConfig::default()).is_empty());
    }

    #[test]
    fn a_sideband_receiver_off_tune() {
        let x = call("FG-HJ", 1.0, 0.2, 20.0);
        assert!(run(&x, SelcalConfig::default())
            .iter()
            .all(|c| c.code.to_string() != "FG-HJ"));
        let cfg = SelcalConfig {
            ssb_search: true,
            ..SelcalConfig::default()
        };
        let c = run(&x, cfg);
        assert_eq!(c.len(), 1);
        assert_eq!(
            (c[0].code.to_string(), c[0].shift_hz),
            ("FG-HJ".to_string(), 20.0)
        );
    }

    #[test]
    fn any_cut_returns_the_same_calls() {
        let x = join(&[call("AM-CP", 1.0, 0.2, 0.0), call("DH-KS", 1.0, 0.2, 0.0)]);
        let whole = run(&x, SelcalConfig::default());
        assert_eq!(whole.len(), 2);
        for cut in [1, 400, 9999] {
            let mut d = SelcalDecoder::new(RATE, SelcalConfig::default()).unwrap();
            let mut v = Vec::new();
            for c in x.chunks(cut) {
                d.process(c, |e| v.push(e));
            }
            assert_eq!(v, whole);
        }
        assert!(SelcalDecoder::new(2000.0, SelcalConfig::default()).is_err());
        assert!(run(&noisy(vec![0.0; 40000], 0.2, 2), SelcalConfig::default()).is_empty());
    }
}

mod twotone {
    use super::*;
    use sdrkit_proto_tones::twotone::{
        two_tone_sequence, TwoToneConfig, TwoToneDecoder, TwoTonePage,
    };

    fn run(x: &[f32]) -> Vec<TwoTonePage> {
        let mut d = TwoToneDecoder::new(RATE, TwoToneConfig::default()).unwrap();
        let mut v = Vec::new();
        d.process(x, |p| v.push(p));
        v
    }

    #[test]
    fn pages_and_not_pages() {
        // Quick Call II style: A 1 s, B 3 s.
        for (a, b) in [(349.0, 1153.4), (1251.4, 600.9), (2468.2, 288.5)] {
            let x = noisy(
                render(two_tone_sequence(RATE, a, b, 1.0, 3.0, 0.3).unwrap(), 0.5),
                0.01,
                3,
            );
            let p = run(&x);
            assert_eq!(p.len(), 2, "{a} {b}");
            assert!(
                (p[0].a_hz - a).abs() < 0.01 * a && (p[0].b_hz - b).abs() < 0.01 * b,
                "{p:?}"
            );
            assert!(
                (p[1].a_s - 1.0).abs() < 0.1 && (p[1].b_s - 3.0).abs() < 0.1,
                "{p:?}"
            );
        }
        // One tone alone, a short A, two tones too close.
        assert!(run(&render(
            two_tone_sequence(RATE, 800.0, 800.0, 1.0, 3.0, 0.3).unwrap(),
            0.5
        ))
        .is_empty());
        assert!(run(&render(
            two_tone_sequence(RATE, 800.0, 1200.0, 0.1, 3.0, 0.3).unwrap(),
            0.5
        ))
        .is_empty());
        assert!(run(&render(
            two_tone_sequence(RATE, 800.0, 820.0, 1.0, 3.0, 0.3).unwrap(),
            0.5
        ))
        .is_empty());
        assert!(run(&noisy(vec![0.0; 40000], 0.2, 1)).is_empty());
        assert!(TwoToneDecoder::new(
            RATE,
            TwoToneConfig {
                a_max_s: 0.1,
                ..TwoToneConfig::default()
            }
        )
        .is_err());
    }
}

mod components {
    use super::*;
    use sdrkit_components::testing::Source;
    use sdrkit_core::Choice;
    use sdrkit_core::Output;
    use sdrkit_graph::stream::Data;
    use sdrkit_graph::{Format, GraphBuilder, Item, Kind, Registry, Timed};
    use sdrkit_proto_tones::components::{
        register, DtmfComponent, DtmfFields, SelcalComponent, SubAudioChange, SubAudioComponent,
        SubAudioEvent, SubAudioFields, SubAudioKind, CHANGE, DTMF_EVENT, SUBAUDIO_EVENT,
    };
    use sdrkit_proto_tones::dtmf::{dtmf_sequence, DtmfEvent};

    /// Runs `comp` over `x` in pieces of 1000 and returns its items.
    fn items<T: Item, C: sdrkit_graph::Component + 'static>(comp: C, x: Vec<f32>) -> Vec<Timed<T>> {
        let n = x.len();
        let mut g = GraphBuilder::new()
            .add(
                "src",
                Source::new(Format::new(Kind::Real, RATE), Data::Real(x), 1000),
            )
            .add("dec", comp)
            .connect("src", "dec")
            .output("dec")
            .build()
            .unwrap();
        g.configure_input(Format::iq(1.0, 0.0, 0.0)).unwrap();
        let mut out = Output::default();
        let mut all = Vec::new();
        for _ in 0..n / 1000 + 2 {
            out.clear();
            g.process(&[], &mut out);
            if let Some(b) = g.buffer("dec.out") {
                if let Data::Items(Some(v)) = &b.data {
                    all.extend(
                        v.as_any()
                            .downcast_ref::<Vec<Timed<T>>>()
                            .unwrap()
                            .iter()
                            .cloned(),
                    );
                }
            }
        }
        all
    }

    #[test]
    fn registered_names() {
        let mut r = Registry::new();
        register(&mut r);
        let names: Vec<&str> = r.names().collect();
        for n in [
            "tones.subaudio",
            "tones.dtmf",
            "tones.selcall",
            "tones.selcal",
            "tones.two_tone",
        ] {
            assert!(names.contains(&n), "{n}");
            assert_eq!(r.make(n).unwrap().info().name, n);
        }
    }

    #[test]
    fn dtmf_items_are_timed() {
        let x = render(dtmf_sequence(RATE, "*7", 0.2, 0.2, 0.1, 0.1).unwrap(), 0.1);
        let v: Vec<Timed<DtmfEvent>> = items(DtmfComponent::default(), x);
        let downs: Vec<(char, f64)> = v
            .iter()
            .filter(|t| t.value.down)
            .map(|t| (t.value.key, t.time))
            .collect();
        assert_eq!(downs.len(), 2);
        assert_eq!((downs[0].0, downs[1].0), ('*', '7'));
        assert!((downs[1].1 - downs[0].1 - 0.2).abs() < 0.01, "{downs:?}");
        let mut e = DTMF_EVENT.event(v[0].time, 0.0);
        v[0].value.write_fields(&mut e.fields);
        assert_eq!(e.at(&DtmfFields::KEY), Some("*"));
    }

    #[test]
    fn selcal_settings_are_strict() {
        use sdrkit_core::{Choice, SettingSet, Value};
        use sdrkit_graph::Component;
        use sdrkit_proto_tones::components::{SelcalSetting, SelcalTones};
        let mut c = SelcalComponent::default();
        let schema = SelcalSetting::schema();
        let tones = SelcalSetting::Tones.id();
        let classic = Value::Choice(SelcalTones::Classic.index());
        assert!(schema.check(tones, &classic).is_ok());
        assert!(c.apply(tones, &classic).is_ok());
        assert!(schema.check(tones, &Value::Choice(7)).is_err());
        assert!(schema
            .check(SelcalSetting::WindowS.id(), &Value::Duration(5.0))
            .is_err());
        assert!(schema.id("nope").is_err());
        assert_eq!(c.values().value(tones), Some(&classic));
    }

    #[test]
    fn subaudio_reads_ctcss_then_dcs() {
        use sdrkit_util_ids::tones::dcs::Dcs;
        let tone = render(
            sdrkit_proto_tones::ctcss::ctcss_sequence(RATE, 146.2, 0.15, 3.0, Some((0.5, 0.2)))
                .unwrap(),
            1.0,
        );
        let mut e = sdrkit_proto_tones::dcs::DcsEncoder::new(
            RATE,
            Dcs::from_octal("245").unwrap(),
            false,
            0.2,
            2.0,
            true,
        )
        .unwrap();
        let mut d = vec![0f32; e.len() as usize + 8000];
        e.fill(&mut d);
        let v: Vec<Timed<SubAudioEvent>> = items(SubAudioComponent::default(), join(&[tone, d]));
        let names: Vec<String> = v
            .iter()
            .map(|t| {
                let mut e = SUBAUDIO_EVENT.event(t.time, 0.0);
                t.value.write_fields(&mut e.fields);
                let kind = e.at(&SubAudioFields::KIND).unwrap();
                let change = e.get(&CHANGE).unwrap();
                format!(
                    "{} {}",
                    SubAudioKind::CHOICES[usize::from(kind.index())].key,
                    SubAudioChange::CHOICES[usize::from(change.index())].key
                )
            })
            .collect();
        assert_eq!(
            names,
            [
                "ctcss locked",
                "ctcss turn_off",
                "dcs locked",
                "dcs turn_off"
            ]
        );
        assert!(v.windows(2).all(|w| w[0].time <= w[1].time));
        use sdrkit_proto_tones::components::TONE_HZ;
        let mut e = SUBAUDIO_EVENT.event(v[0].time, 0.0);
        v[0].value.write_fields(&mut e.fields);
        assert_eq!(e.at(&SubAudioFields::CODE), None);
        assert_eq!(e.get(&TONE_HZ), Some(146.2_f32 as f64));
    }
}

mod allocation {
    use super::*;
    use sdrkit_proto_tones::{ctcss, dcs, dtmf, selcal, selcall, twotone};
    use sdrkit_util_ids::tones::dcs::Dcs;
    use sdrkit_util_ids::tones::selcal::Code;
    use sdrkit_util_ids::tones::selcall::by_name;

    /// Feeds `x` 1,000 times in pieces and expects no allocation (the decoders were made before).
    fn none(mut feed: impl FnMut(&[f32]), x: &[f32]) {
        let piece = x.len() / 1000;
        let n = allocations(|| {
            for c in x.chunks(piece.max(1)) {
                feed(c);
            }
        });
        assert_eq!(n, 0);
    }

    #[test]
    fn steady_state_decoding_allocates_nothing() {
        let x = join(&[
            render(
                ctcss::ctcss_sequence(RATE, 100.0, 0.15, 2.0, Some((0.5, 0.2))).unwrap(),
                0.5,
            ),
            render(
                dtmf::dtmf_sequence(RATE, "123", 0.2, 0.2, 0.05, 0.05).unwrap(),
                0.2,
            ),
            render(
                selcall::selcall_sequence(RATE, by_name("ZVEI1").unwrap(), "12345", 0.3, None)
                    .unwrap(),
                0.3,
            ),
            render(
                selcal::selcal_sequence(RATE, Code::parse("AM-CP").unwrap(), 0.25, 1.0, 0.2, 0.0)
                    .unwrap(),
                0.3,
            ),
            render(
                twotone::two_tone_sequence(RATE, 500.0, 900.0, 1.0, 1.5, 0.3).unwrap(),
                0.3,
            ),
        ]);
        let mut dc = vec![0f32; 24000];
        dcs::DcsEncoder::new(RATE, Dcs::from_octal("023").unwrap(), false, 0.2, 2.5, true)
            .unwrap()
            .fill(&mut dc);
        let x = join(&[x, dc]);
        let mut events = 0usize;
        let mut c = ctcss::CtcssDecoder::new(RATE, ctcss::CtcssConfig::default()).unwrap();
        none(|p| c.process(p, |_| events += 1), &x);
        let mut d = dcs::DcsDecoder::new(RATE, dcs::DcsConfig::default()).unwrap();
        none(|p| d.process(p, |_| events += 1), &x);
        let mut t = dtmf::DtmfDecoder::new(RATE, dtmf::DtmfConfig::default()).unwrap();
        none(|p| t.process(p, |_| events += 1), &x);
        let mut s = selcall::SelcallDecoder::new(RATE, selcall::SelcallConfig::default()).unwrap();
        none(|p| s.process(p, |_| events += 1), &x);
        let mut a = selcal::SelcalDecoder::new(
            RATE,
            selcal::SelcalConfig {
                ssb_search: true,
                ..selcal::SelcalConfig::default()
            },
        )
        .unwrap();
        none(|p| a.process(p, |_| events += 1), &x);
        let mut w = twotone::TwoToneDecoder::new(RATE, twotone::TwoToneConfig::default()).unwrap();
        none(|p| w.process(p, |_| events += 1), &x);
        // Each decoder found its signal: CTCSS 2, DCS 2, DTMF 6, selcall 1, SELCAL 1, two-tone 2.
        assert!(events >= 14, "{events}");
        // The generators too.
        let mut g = dtmf::dtmf_sequence(RATE, "0123456789", 0.2, 0.2, 0.05, 0.05).unwrap();
        let mut e =
            dcs::DcsEncoder::new(RATE, Dcs::from_octal("754").unwrap(), true, 0.2, 2.0, true)
                .unwrap();
        let mut buf = vec![0f32; 100];
        assert_eq!(
            allocations(|| {
                for _ in 0..1000 {
                    g.fill(&mut buf);
                    e.fill(&mut buf);
                }
            }),
            0
        );
    }
}

/// The shared interface of the crate lists its five event types, each with the fields of its declaration.
#[test]
fn the_shared_interface_lists_the_event_types() {
    let shared = sdrkit_proto_tones::components::interface();
    assert!(shared.shared);
    let kinds: Vec<&str> = shared.stacks[0].events.iter().map(|e| e.def.kind).collect();
    assert_eq!(
        kinds,
        ["tones.subaudio", "tones.dtmf", "tones.selcall", "tones.selcal", "tones.two_tone"]
    );
    for e in &shared.stacks[0].events {
        assert_eq!(e.def.fields.len(), e.fields.fields.len());
    }
}
