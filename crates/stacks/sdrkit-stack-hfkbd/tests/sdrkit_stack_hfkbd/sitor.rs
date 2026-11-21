//! SITOR-B: round trips at several offsets, rates and piece sizes, the recording, and the mode B phasing on the air.

use crate::common::*;
use sdrkit_core::LockState;
use sdrkit_stack_hfkbd as hf;
use sdrkit_util_text::ccir476::{ALPHA, PHASING_1, PHASING_2};

#[test]
fn sitor_b() {
    for (off, rate, piece) in [
        (150.0, 12000.0, 7),
        (-300.0, 48000.0, 4096),
        (45.0, 24000.0, 4096),
    ] {
        let h = snr_2500(harness(&hf::SITOR), 15.0)
            .set(&GEN_OFFSET_HZ, off)
            .rate(rate)
            .piece(piece)
            .seconds(25.0);
        let r = run(&h);
        let t = text(&r);
        assert!(
            t.contains("TEST MESSAGE FROM K0TST 0123456789"),
            "{off}: {t:?}"
        );
        assert_eq!(r.results.status().lock_state, LockState::Locked);
        assert!(!flag(&r, "inverted"));
    }
}

#[test]
fn sitor_recording() {
    let name = "hfkbd_sitorb_12k";
    let e = expected(name);
    for piece in [800, 4093] {
        let r = recording(&harness(&hf::SITOR).piece(piece), name);
        let t = text(&r);
        assert!(
            t.contains(e["expect"]["contains"].as_str().unwrap()),
            "{t:?}"
        );
        assert_eq!(r.results.status().lock_state, LockState::Locked);
        assert!(in_range(num(&r, "shift_hz"), &e["expect"]["shift_hz"]));
        assert!(in_range(
            num(&r, "symbol_rate_hz"),
            &e["expect"]["symbol_rate_hz"]
        ));
        let inv = e["expect"]["inverted"].as_bool().unwrap();
        assert_eq!(flag(&r, "inverted"), inv);
    }
}

/// M.625-4 4.4.2: while phasing, the DX positions carry phasing signal 2 and the RX positions phasing signal 1. Idle is alpha.
#[test]
fn sitor_phasing_sends_phasing_signal_2_in_dx_positions() {
    let w = hf::sitor::air_words("CQ");
    assert!(w.len() > 40);
    for pair in w[..32].chunks(2) {
        assert_eq!(pair[0], PHASING_2);
        assert_eq!(pair[1], PHASING_1);
    }
    assert_eq!(w[w.len() - 2], ALPHA);
    assert!(!w.contains(&sdrkit_util_text::ccir476::BETA));
}

mod arq {
    use super::*;
    use hf::decoder::{test_dial, Modulator};
    use hf::sitor::arq::{ArqLink, LinkParams};
    use sdrkit_core::{Complex32, InputInfo, Output, Results};

    const T1: &str = "CQ DE TEST VESSEL. POSITION 54 30N 010 12E. ALL WELL 1234567890 /";
    const T2: &str = "QSL TNX FOR TRAFFIC 73";

    struct Case {
        rate: f64,
        piece: usize,
        snr_db: Option<f64>,
        carrier_hz: f64,
        swap: bool,
        link: LinkParams,
        text: &'static str,
    }

    impl Default for Case {
        fn default() -> Self {
            Case {
                rate: 250e3,
                piece: 65_536,
                snr_db: None,
                carrier_hz: 0.0,
                swap: false,
                link: LinkParams::default(),
                text: T1,
            }
        }
    }

    struct Got {
        results: Results,
        events: Vec<String>,
    }

    impl Got {
        fn text(&self) -> String {
            field_text(&self.results, "arq_text")
        }
        fn num(&self, key: &str) -> f64 {
            field_num(&self.results, key)
        }
    }

    /// The link with its FSK centre 1700 Hz above the dial, the dial 20 kHz below the centre of the samples, with noise for an
    /// SNR of station A in 3 kHz, fed in pieces.
    fn run_case(c: Case) -> Got {
        let mut p = c.link;
        p.centre_hz = test_dial(c.rate) + 1700.0 + c.carrier_hz;
        if c.swap {
            p.shift_hz = -p.shift_hz;
        }
        let mut link = ArqLink::new(c.rate, c.text, p);
        let mut noise = c.snr_db.map(|snr| {
            sdrkit_algo_sim::noise::Awgn::with_power(10f64.powf(-snr / 10.0) * c.rate / 3000.0, 7)
                .unwrap()
        });
        let mut rx = harness(&hf::SITOR).receiver().unwrap();
        let dial = 14.07e6;
        rx.configure(&InputInfo::new(c.rate, dial - test_dial(c.rate), dial).recording())
            .unwrap();
        let mut out = Output::default();
        let mut events = Vec::new();
        let mut x = vec![Complex32::default(); c.piece];
        let mut left = link.len() as usize;
        while left > 0 {
            let n = left.min(c.piece);
            link.fill(&mut x[..n]);
            if let Some(a) = noise.as_mut() {
                a.process(&mut x[..n]);
            }
            out.clear();
            rx.feed(&x[..n], &mut out);
            events.extend(
                out.events
                    .iter()
                    .filter(|e| e.kind() == "sitor.arq")
                    .map(|e| e.text("text")),
            );
            left -= n;
        }
        out.clear();
        rx.finish(&mut out);
        Got {
            results: rx.results().clone(),
            events,
        }
    }

    fn assert_full(g: &Got, what: &str) {
        let t = g.text();
        assert_eq!(t.matches(T1).count(), 1, "{what}: {t:?}");
        assert_eq!(field_text(&g.results, "selcall"), "QCXT", "{what}");
        assert!(
            g.events.iter().any(|e| e == "end of communication"),
            "{what}: {:?}",
            g.events
        );
        assert!(g.num("arq_repeats") >= 1.0, "{what}");
        assert!(
            g.results
                .status()
                .summary
                .starts_with("SITOR-A (ARQ) locked"),
            "{what}: {}",
            g.results.status().summary
        );
    }

    #[test]
    fn clean_links_at_two_rates() {
        assert_full(&run_case(Case::default()), "clean");
        assert_full(
            &run_case(Case {
                rate: 2e6,
                ..Case::default()
            }),
            "2 Msps",
        );
    }

    #[test]
    fn offsets_swapped_tones_and_drift() {
        for off in [-60.0, 45.0, 150.0] {
            assert_full(
                &run_case(Case {
                    carrier_hz: off,
                    ..Case::default()
                }),
                &format!("offset {off}"),
            );
        }
        assert_full(
            &run_case(Case {
                swap: true,
                ..Case::default()
            }),
            "swapped",
        );
        let link = LinkParams {
            drift_hz_s: 1.0,
            ..LinkParams::default()
        };
        assert_full(
            &run_case(Case {
                link,
                ..Case::default()
            }),
            "drift",
        );
    }

    #[test]
    fn noise() {
        for snr in [15.0, 8.0, 4.0] {
            assert_full(
                &run_case(Case {
                    snr_db: Some(snr),
                    ..Case::default()
                }),
                &format!("SNR {snr}"),
            );
        }
        let g = run_case(Case {
            snr_db: Some(0.0),
            ..Case::default()
        });
        assert!(g.text().contains("POSITION 54 30N"), "{:?}", g.text());
    }

    #[test]
    fn the_receiving_station() {
        type Change = fn(&mut LinkParams);
        let cases: [(&str, Change, Option<f64>); 5] = [
            ("weak IRS", |l| l.irs_level = 0.1, Some(25.0)),
            ("delay 3.0", |l| l.irs_delay_bits = 3.0, None),
            ("delay 15.6", |l| l.irs_delay_bits = 15.6, None),
            ("every 3rd", |l| l.repeat_every = 3, None),
            ("25 Hz off", |l| l.irs_offset_hz = 25.0, None),
        ];
        for (what, f, snr_db) in cases {
            let mut link = LinkParams::default();
            f(&mut link);
            assert_full(
                &run_case(Case {
                    link,
                    snr_db,
                    ..Case::default()
                }),
                what,
            );
        }
    }

    #[test]
    fn pieces() {
        for piece in [5, 1000] {
            assert_full(
                &run_case(Case {
                    piece,
                    ..Case::default()
                }),
                &format!("pieces of {piece}"),
            );
        }
    }

    #[test]
    fn no_call_and_no_irs() {
        let link = LinkParams {
            call: false,
            ..LinkParams::default()
        };
        let g = run_case(Case {
            link,
            ..Case::default()
        });
        assert_eq!(g.text().matches(T1).count(), 1, "{:?}", g.text());
        assert_eq!(field_text(&g.results, "selcall"), "");
        let link = LinkParams {
            irs_level: 0.0,
            ..LinkParams::default()
        };
        let g = run_case(Case {
            link,
            ..Case::default()
        });
        assert_eq!(g.text().matches(T1).count(), 1, "{:?}", g.text());
    }

    #[test]
    fn change_over() {
        let link = LinkParams {
            change_over: true,
            second_text: T2.into(),
            ..LinkParams::default()
        };
        let g = run_case(Case {
            link,
            ..Case::default()
        });
        let t = g.text();
        assert_eq!(t.matches(&format!("{T1}+?")).count(), 1, "{t:?}");
        assert_eq!(t.matches(T2).count(), 1, "{t:?}");
        assert!(g.events.iter().any(|e| e.contains("CS3")), "{:?}", g.events);
        assert!(
            g.events
                .iter()
                .any(|e| e.contains("other station is sending")),
            "{:?}",
            g.events
        );
    }

    #[test]
    fn noise_only() {
        let link = LinkParams {
            call: false,
            repeat_every: 0,
            ..LinkParams::default()
        };
        let g = run_case(Case {
            link,
            text: "",
            snr_db: Some(-30.0),
            ..Case::default()
        });
        assert!(g.num("arq_new_blocks") <= 2.0);
        assert!(g.num("arq_chars") < 6.0);
    }
}
