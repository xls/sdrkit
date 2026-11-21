//! VDL Mode 3: the Golay and Reed-Solomon codes, the training sequences, the bursts both ways, and the receiver on its test
//! signal with noise, offsets, drift, a clock error, an adjacent station and noise alone.

use crate::common::{self, clock, field, int, noise, offset, rows, text};
use sdrkit_algo_fec::rs::{codes::VDL3_72_62, Rs};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, Generator, LockState};
use sdrkit_stack_avdata::d8psk::{correlate, turns};
use sdrkit_stack_avdata::typed::{Vdl3BurstRow, Vdl3Fields, VDL3_BURSTS};
use sdrkit_stack_avdata::vdl3::gen::{GenParams, Sent, Vdl3Generator};
use sdrkit_stack_avdata::vdl3::phy::{
    payload_bits, BurstKind, Content, Golay, OctetOrder, Payload, PayloadDecoder, Training,
};
use sdrkit_stack_avdata::vdl3::{self, printable, NOTE, TUNE_OFFSET_HZ};

const F: f64 = vdl3::TEST_SIGNAL_HZ;

#[test]
fn golay_words_and_weights() {
    let g = Golay::default();
    for (d, w) in [
        (0x001, 0xAE3001),
        (0x800, 0xD71800),
        (0xA5C, 0xEDFA5C),
        (0x123, 0x885123),
    ] {
        assert_eq!(g.encode(d), w, "{d:03X}");
    }
    let mut weights = [0u32; 25];
    for d in 0..4096u16 {
        weights[g.encode(d).count_ones() as usize] += 1;
    }
    assert_eq!(
        (
            weights[0],
            weights[8],
            weights[12],
            weights[16],
            weights[24]
        ),
        (1, 759, 2576, 759, 1)
    );
    let mut rng = Rng::new(3);
    for _ in 0..2000 {
        let d = (rng.next_u64() & 0xFFF) as u16;
        let w = g.encode(d);
        let mut e = 0u32;
        let n = (rng.next_u64() % 5) as u32;
        while e.count_ones() < n {
            e |= 1 << (rng.next_u64() % 24);
        }
        match n {
            0..=3 => assert_eq!(g.decode(w ^ e), Some((d, n as u8))),
            _ => assert_eq!(g.decode(w ^ e), None, "4 errors are detected"),
        }
    }
}

#[test]
fn reed_solomon_72_62() {
    let mut rs = Rs::new(VDL3_72_62).unwrap();
    let mut rng = Rng::new(7);
    for errors in 0..=6 {
        let mut word = [0u8; 72];
        for b in &mut word[..62] {
            *b = rng.next_u64() as u8;
        }
        let (d, p) = word.split_at_mut(62);
        rs.encode(d, p).unwrap();
        let good = word;
        let mut at = Vec::new();
        while at.len() < errors {
            let i = (rng.next_u64() % 72) as usize;
            if !at.contains(&i) {
                at.push(i);
                word[i] ^= 1 + (rng.next_u64() % 255) as u8;
            }
        }
        let r = rs.decode(&mut word, &[]);
        if errors <= 5 {
            assert_eq!(r.unwrap().corrected as usize, errors);
            assert_eq!(word, good);
        } else {
            assert!(
                r.is_err() || word != good,
                "6 errors are refused or not taken for the sent word"
            );
        }
    }
}

#[test]
fn training_sequences() {
    // S1 and S2 in phase steps (units of pi/4) by the Gray map, the first from the ramp.
    assert_eq!(
        Training::S1.steps(),
        [0, 5, 1, 1, 3, 4, 0, 2, 7, 4, 2, 5, 3, 6, 7, 6]
    );
    assert_eq!(
        Training::S2.steps(),
        [0, 5, 2, 3, 0, 7, 1, 3, 7, 6, 2, 4, 1, 4, 6, 5]
    );
    // S1* from its bits: every change turned by 180 degrees from S1.
    assert_eq!(
        Training::S1Star.steps(),
        [0, 1, 5, 5, 7, 0, 4, 6, 3, 0, 6, 1, 7, 2, 3, 2]
    );
    assert_eq!(
        Training::S2Star.steps(),
        [0, 1, 6, 7, 4, 3, 5, 7, 3, 2, 6, 0, 5, 0, 2, 1]
    );
    // The symbols of each sequence correlate fully with it, S1 and S2 little with each other, the starred ones negatively.
    let sym = |t: Training| -> [Complex32; 16] {
        let s = t.steps();
        let mut ph = 0;
        std::array::from_fn(|k| {
            ph += s[k];
            Complex32::from_polar(1.0, std::f32::consts::FRAC_PI_4 * ph as f32)
        })
    };
    let t1 = turns(|k| Training::S1.steps()[k]);
    let t2 = turns(|k| Training::S2.steps()[k]);
    assert!((correlate(&sym(Training::S1), &t1).re - 15.0).abs() < 1e-3);
    assert!((correlate(&sym(Training::S1Star), &t1).re + 15.0).abs() < 1e-3);
    assert!((correlate(&sym(Training::S2Star), &t2).re + 15.0).abs() < 1e-3);
    assert!(correlate(&sym(Training::S2), &t1).norm() < 6.0);
    assert!(correlate(&sym(Training::S1), &t2).norm() < 6.0);
}

#[test]
fn every_kind_round_trips_with_bit_errors() {
    let mut dec = PayloadDecoder::default();
    let mut rng = Rng::new(11);
    for kind in [
        BurstKind::MUplink,
        BurstKind::MUplink3T,
        BurstKind::Handoff,
        BurstKind::PollResponse,
        BurstKind::MDownlink,
        BurstKind::NetEntry,
        BurstKind::VdData,
        BurstKind::VdVoice,
    ] {
        for order in [OctetOrder::LsbFirst, OctetOrder::MsbFirst] {
            let content = match kind {
                BurstKind::VdData => Content::Data(0x123, b"VDL3 TEST 7".to_vec()),
                BurstKind::VdVoice => {
                    Content::Voice(0x456, (0..576).map(|i| (i % 3 == 0) as u8).collect())
                }
                k => Content::Words(
                    (0..k.words())
                        .map(|i| (0x100 * i as u16 + 0x5A) & 0xFFF)
                        .collect(),
                ),
            };
            let mut bits = Vec::new();
            payload_bits(kind, &content, order, &mut bits);
            assert_eq!(bits.len(), 3 * kind.symbols(), "{kind:?}");
            // One wrong bit in each Golay word, three octets wrong in the data word.
            for w in 0..kind.words() {
                bits[24 * w + (rng.next_u64() % 24) as usize] ^= 1;
            }
            if kind == BurstKind::VdData {
                for o in [3usize, 30, 70] {
                    bits[24 + 8 * o + 2] ^= 1;
                }
            }
            let mut p = Payload::default();
            dec.decode(
                &mut bits,
                kind.words(),
                kind.training() == Training::S2,
                order,
                &mut p,
            );
            assert!(p.golay_ok, "{kind:?}");
            assert_eq!(p.golay_corrected as usize, kind.words());
            match &content {
                Content::Words(w) => assert_eq!(&p.words, w),
                Content::Data(h, d) => {
                    assert_eq!((p.words[0], p.rs_ok, p.rs_corrected), (*h, true, 3));
                    assert_eq!(&p.octets[..d.len()], d.as_slice());
                    assert_eq!(printable(&p.octets), "VDL3 TEST 7");
                }
                Content::Voice(h, _) => assert_eq!((p.words[0], p.rs_ok), (*h, false)),
            }
        }
    }
}

/// The receiver on `seconds` of the test signal at `rate` through `impair`; returns the rows and the bursts sent that end
/// inside the signal.
fn run(
    rate: f64,
    seconds: f64,
    adjacent: bool,
    impair: impl FnOnce(Vec<Complex32>) -> Vec<Complex32>,
) -> (Vec<common::Row>, Vec<Sent>, sdrkit_core::Results) {
    let mut g = Vdl3Generator::new(GenParams {
        rate_hz: rate,
        channel_hz: -TUNE_OFFSET_HZ,
        seed: 5,
        adjacent,
    });
    let mut x = vec![Complex32::default(); (rate * seconds) as usize];
    g.generate(&mut x);
    let x = impair(x);
    let mut rx = common::receiver(&vdl3::DESCRIPTOR, rate, F + TUNE_OFFSET_HZ, F, &[]);
    common::run(rx.as_mut(), &x, 50_000);
    common::run(
        rx.as_mut(),
        &vec![Complex32::default(); (0.3 * rate) as usize],
        50_000,
    );
    let sent = g.sent().to_vec();
    let r = sdrkit_core::owner::Owner::results(rx.as_ref()).clone();
    (rows(&r, &VDL3_BURSTS), sent, r)
}

/// The bursts sent that end inside the first `seconds`, how many of them were received with their kind and content, and the
/// rows that match no burst sent.
fn score(rows: &[common::Row], sent: &[Sent], seconds: f64) -> (usize, usize, Vec<String>) {
    let kind_of = |r: &common::Row| r.at(&Vdl3BurstRow::KIND);
    let mut found = 0;
    let inside: Vec<&Sent> = sent.iter().filter(|s| s.time_s + 0.025 < seconds).collect();
    for s in &inside {
        let hit = rows.iter().any(|r| {
            (common::num(r, "time_s") - s.time_s).abs() < 2e-3
                && kind_of(r) == Some(s.kind)
                && match &s.content {
                    Content::Words(w) => {
                        text(r, "info")
                            == w.iter()
                                .map(|v| format!("{v:03X}"))
                                .collect::<Vec<_>>()
                                .join(" ")
                    }
                    Content::Data(_, d) => text(r, "data_text") == String::from_utf8_lossy(d),
                    Content::Voice(..) => true,
                }
        });
        found += usize::from(hit);
    }
    let wrong = rows
        .iter()
        .filter(|r| {
            common::flag(r, "golay_ok")
                && common::num(r, "evm_pct") < 25.0
                && common::num(r, "time_s") + 0.025 < seconds
        })
        .filter(|r| {
            !sent.iter().any(|s| {
                (common::num(r, "time_s") - s.time_s).abs() < 2e-3 && kind_of(r) == Some(s.kind)
            })
        })
        .map(|r| text(r, "summary").to_string())
        .collect();
    (inside.len(), found, wrong)
}

fn es_n0(x: &mut [Complex32], db: f64, rate: f64) {
    noise(x, 1.0, db, 10_500.0, rate, 9);
}

#[test]
fn every_burst_at_30_db_and_two_rates() {
    for rate in [2.4e6, 250e3] {
        let (rows, sent, r) = run(rate, 1.1, false, |mut x| {
            offset(&mut x, rate, -1500.0);
            es_n0(&mut x, 30.0, rate);
            x
        });
        let (n, found, wrong) = score(&rows, &sent, 1.1);
        assert!(n >= 60, "{n}");
        assert_eq!(found, n, "at {rate}");
        assert!(wrong.is_empty(), "{wrong:?}");
        // Every kind is in the test signal and was received.
        for (k, c) in [
            Vdl3Fields::M_UPLINK,
            Vdl3Fields::M_UPLINK_3T,
            Vdl3Fields::HANDOFF,
            Vdl3Fields::POLL_RESPONSE,
            Vdl3Fields::M_DOWNLINK,
            Vdl3Fields::NET_ENTRY,
            Vdl3Fields::VD_DATA,
            Vdl3Fields::VD_VOICE,
        ]
        .iter()
        .enumerate()
        {
            assert!(field(&r, c.key) >= 1.0, "kind {k}");
        }
        let cfo = rows
            .iter()
            .filter(|r| r.at(&Vdl3BurstRow::KIND) == Some(BurstKind::MUplink))
            .map(|r| common::num(r, "cfo_hz"))
            .next()
            .unwrap();
        assert!((cfo + 1500.0).abs() < 100.0, "{cfo}");
        assert_eq!(r.status().lock_state, LockState::Locked);
        assert_eq!(field(&r, "rx_state"), 2.0);
        assert_eq!(r.fields().at(&Vdl3Fields::NOTE), Some(NOTE));
        let data = rows
            .iter()
            .find(|r| r.at(&Vdl3BurstRow::KIND) == Some(BurstKind::VdData))
            .unwrap();
        assert!(text(data, "summary").starts_with("header 123, data \"VDL3 TEST "));
        assert_eq!(text(data, "data_hex").len(), 124);
        let voice = rows
            .iter()
            .find(|r| r.at(&Vdl3BurstRow::KIND) == Some(BurstKind::VdVoice))
            .unwrap();
        assert_eq!(
            text(voice, "summary"),
            "header 123, vocoder frames (not decoded)"
        );
        assert_eq!(text(voice, "data_hex").len(), 144);
    }
}

#[test]
fn every_burst_at_21_db() {
    let rate = 250e3;
    let (rows, sent, _) = run(rate, 1.1, false, |mut x| {
        es_n0(&mut x, 21.0, rate);
        x
    });
    let (n, found, wrong) = score(&rows, &sent, 1.1);
    assert_eq!(found, n);
    assert!(wrong.is_empty(), "{wrong:?}");
}

/// At 18 dB at least 40 of 44 bursts are read and at 15 dB at least 10 of 44, with no wrong burst.
#[test]
fn every_burst_at_18_and_15_db() {
    let rate = 250e3;
    for (db, need) in [(18.0, 40.0 / 44.0), (15.0, 10.0 / 44.0)] {
        let (rows, sent, _) = run(rate, 1.1, false, |mut x| {
            es_n0(&mut x, db, rate);
            x
        });
        let (n, found, strict) = score(&rows, &sent, 1.1);
        assert!(found as f64 >= need * n as f64, "{db} dB: {found} of {n}");
        if db >= 18.0 {
            assert!(strict.is_empty(), "{db} dB: {strict:?}");
        }
        // A V/D burst whose Reed-Solomon word fails is shown as voice: at 15 dB a data burst may come back as voice, never as
        // anything else and never as data with other text.
        let wrong: Vec<&str> = rows
            .iter()
            .filter(|r| common::flag(r, "golay_ok") && common::num(r, "evm_pct") < 25.0)
            .filter(|r| {
                !sent.iter().any(|s| {
                    let k = r.at(&Vdl3BurstRow::KIND);
                    (common::num(r, "time_s") - s.time_s).abs() < 2e-3
                        && (k == Some(s.kind)
                            || (k == Some(BurstKind::VdVoice) && s.kind == BurstKind::VdData))
                })
            })
            .map(|r| text(r, "summary"))
            .collect();
        assert!(wrong.is_empty(), "{db} dB: {wrong:?}");
    }
}

#[test]
fn drift_clock_error_and_an_adjacent_station() {
    let rate = 1e6;
    let (rows, sent, _) = run(rate, 1.1, false, |mut x| {
        sdrkit_algo_sim::offset::FrequencyOffset::new(rate, 200.0, 150.0)
            .unwrap()
            .process(&mut x);
        let mut y = clock(&x, 60.0);
        es_n0(&mut y, 30.0, rate);
        y
    });
    let (n, found, wrong) = score(&rows, &sent, 1.1);
    assert_eq!(found, n, "drift and clock");
    assert!(wrong.is_empty(), "{wrong:?}");

    let (rows, sent, _) = run(rate, 1.1, true, |mut x| {
        es_n0(&mut x, 30.0, rate);
        x
    });
    let (n, found, wrong) = score(&rows, &sent, 1.1);
    assert_eq!(found, n, "adjacent station");
    assert!(wrong.is_empty(), "{wrong:?}");
}

#[test]
fn noise_alone_finds_no_training() {
    let rate = 250e3;
    let mut x = vec![Complex32::default(); (2.0 * rate) as usize];
    noise(&mut x, 1.0, 0.0, rate, rate, 4);
    let mut rx = common::receiver(&vdl3::DESCRIPTOR, rate, F + TUNE_OFFSET_HZ, F, &[]);
    common::run(rx.as_mut(), &x, 50_000);
    let r = sdrkit_core::owner::Owner::results(rx.as_ref());
    assert_eq!(field(r, "trainings"), 0.0);
    assert!(rows(r, &VDL3_BURSTS).is_empty());
    assert_eq!(int_state(r), 0);
    // A rate below 250 ksps is refused with a status text.
    let mut rx = (vdl3::DESCRIPTOR.make_receiver)(&sdrkit_core::StackConfig::new(common::values(
        &vdl3::DESCRIPTOR,
        &[],
    )))
    .unwrap();
    assert!(rx
        .configure(&sdrkit_core::InputInfo::new(200e3, F, F).recording())
        .is_err());
    assert!(sdrkit_core::owner::Owner::results(rx.as_ref())
        .status()
        .summary
        .contains("needs a sample rate of at least"));
}

/// Digital silence gives the shared smallest floor (-120 dB of full scale), not zero; the SNR of a burst over it stays
/// defined.
#[test]
fn silence_gives_the_smallest_floor() {
    use sdrkit_algo_dsp::floor::DEFAULT_MIN_FLOOR;
    use sdrkit_stack_avdata::vdl3::rx::{Counts, Vdl3Rx};
    let mut rx = Vdl3Rx::new(250e3, 0.0).unwrap();
    let mut counts = Counts::default();
    rx.process(&vec![Complex32::default(); 50_000], &mut counts, |_| {});
    assert_eq!(rx.floor(), Some(f64::from(DEFAULT_MIN_FLOOR)));
}

/// The test signal with no added noise as 8-bit samples (steps of 1/128 of full scale): the noise between bursts rounds to
/// zero, and every one of the 75 bursts is read.
#[test]
fn every_burst_with_8_bit_samples() {
    let rate = 250e3;
    let (rows, sent, _) = run(rate, 1.19, false, |x| {
        x.iter()
            .map(|v| {
                let q = |a: f32| ((0.5 * a * 128.0).round().clamp(-128.0, 127.0)) / 128.0;
                Complex32::new(q(v.re), q(v.im))
            })
            .collect()
    });
    let (n, found, wrong) = score(&rows, &sent, 1.19);
    assert_eq!((found, n), (75, 75));
    assert!(wrong.is_empty(), "{wrong:?}");
    for r in &rows {
        let snr = common::num(r, "snr_db");
        // Over the floor of silence (-120 dB) the SNR is high but measured.
        assert!(snr.is_finite() && snr > 30.0, "{snr}");
    }
}

fn int_state(r: &sdrkit_core::Results) -> i64 {
    field(r, "rx_state") as i64
}

#[test]
fn the_data_file_has_the_vhf_channels() {
    let file = crate::plan_file("vdl3.json");
    assert_eq!(file.groups.len(), 1);
    let b = &file.groups[0].bands[0];
    assert_eq!((b.start_hz, b.end_hz), (117.9875e6, 136.9875e6));
    let n = b.numbering.as_ref().unwrap();
    assert_eq!(n.last - n.first + 1, 760);
    let _ = int;
}
