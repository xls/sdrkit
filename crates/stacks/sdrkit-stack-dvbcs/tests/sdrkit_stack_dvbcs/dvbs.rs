//! DVB-S and DVB-S2: both recordings, every DVB-S rate, DVB-S2 frames of every modulation decoded with the LDPC and BCH codes of
//! the standard, GSE and IP, dummy frames.

use crate::common::*;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_ESN0_DB, GEN_INVERTED, GEN_NOISE, GEN_OFFSET_HZ};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::Choice;
use sdrkit_proto_dvbs2::modcod::Modulation;
use sdrkit_proto_dvbs2::settings::{
    GenRollOff, GenStandardChoice, GEN_DUMMY_EVERY, GEN_GSE, GEN_MODULATION, GEN_PILOTS, GEN_RATE,
    GEN_ROLLOFF, GEN_SHORT, GEN_STANDARD, GEN_SYMBOL_RATE,
};
use sdrkit_proto_dvbs2::sync::Received;
use sdrkit_proto_dvbs2::typed::Dvbs2Fields as D;
use sdrkit_proto_dvbs2::{CodeRate, FrameSize, RollOffSource};
use sdrkit_proto_mpegts::monitor;
use sdrkit_stack_dvbcs::dvbs::DVBS;

#[test]
fn recorded_dvbs_vector_decodes_as_expected() {
    let rec = sdrkit_dev_test::read_recording("dvbcs/dvbs_qpsk34_1msym_2msps")
        .expect("the DVB-S recording");
    let mut rx = Rx::new(&DVBS, &[], &recording(rec.rate_hz, rec.centre_hz));
    rx.feed(&rec.samples, 50000);
    rx.dump();
    let c = rx.check();
    println!("{c:?}");
    assert!(c.good >= 100, "{}", c.good);
    assert_eq!((c.wrong, c.flagged, c.gaps), (0, 0, 0));
    assert!(c.first.unwrap() <= 620);
    assert_eq!(rx.at(&D::STANDARD), Some(Received::Dvbs));
    assert_eq!(rx.at(&D::MODULATION), Some(Modulation::Qpsk));
    assert_eq!(rx.at(&D::CODE_RATE), Some(CodeRate::R34));
    assert_eq!(rx.at(&D::INVERTED), Some(false));
    assert_eq!(rx.at(&D::ROLL_OFF_SOURCE), Some(RollOffSource::Assumed));
    let rs = rx.num("symbol_rate_hz");
    assert!((rs - 1e6).abs() < 200.0, "{rs}");
    let cfo = rx.num("offset_hz");
    assert!((17000.0..=23000.0).contains(&cfo), "{cfo}");
    let es = rx.num("esn0_db");
    assert!((9.0..=11.0).contains(&es), "{es}");
}

/// The DVB-S2 recording decodes: the LDPC and BCH codes of EN 302 307-1 deliver its packets.
#[test]
fn recorded_dvbs2_vector_delivers_its_packets() {
    let rec = sdrkit_dev_test::read_recording("dvbcs/dvbs2_qpsk35_1msym_2msps")
        .expect("the DVB-S2 recording");
    let mut rx = Rx::new(&DVBS, &[], &recording(rec.rate_hz, rec.centre_hz));
    rx.feed(&rec.samples, 50000);
    rx.dump();
    let c = rx.check();
    println!("{c:?}");
    assert_eq!(rx.at(&D::STANDARD), Some(Received::Dvbs2));
    assert_eq!(rx.at(&D::MODCOD), Some(5));
    assert_eq!(rx.at(&D::MODULATION), Some(Modulation::Qpsk));
    assert_eq!(rx.at(&D::CODE_RATE), Some(CodeRate::R35));
    assert_eq!(rx.at(&D::FRAME), Some(FrameSize::Normal));
    assert_eq!(rx.at(&D::PILOTS), Some(false));
    assert_eq!(rx.at(&D::INVERTED), Some(false));
    assert_eq!(rx.at(&D::ROLL_OFF_SOURCE), Some(RollOffSource::Signalled));
    assert_eq!(rx.at(&D::SINGLE_STREAM), Some(true));
    let rs = rx.num("symbol_rate_hz");
    assert!((rs - 1e6).abs() < 200.0, "{rs}");
    let cfo = rx.num("offset_hz");
    assert!((-33000.0..=-27000.0).contains(&cfo), "{cfo}");
    let es = rx.num("esn0_db");
    assert!((7.2..=8.8).contains(&es), "{es}");
    assert!(c.good >= 200, "{c:?}");
    assert_eq!((c.wrong, c.flagged, c.gaps), (0, 0, 0));
    assert!(c.first.unwrap() <= 260);
    assert_eq!(rx.at(&D::BCH_BAD), Some(0));
    assert_eq!(rx.at(&D::TS_LOCK), Some(true));
}

fn s2(
    m: Modulation,
    rate: CodeRate,
    short: bool,
    pilots: bool,
    es: f64,
    extra: &[CaseSetting],
) -> Rx {
    let mut kv = vec![
        CaseSetting::new(&GEN_MODULATION, m),
        CaseSetting::new(&GEN_RATE, rate),
        CaseSetting::new(&GEN_SHORT, short),
        CaseSetting::new(&GEN_PILOTS, pilots),
        CaseSetting::new(&GEN_SYMBOL_RATE, 1e6),
        CaseSetting::new(&GEN_NOISE, NoiseKind::EsN0),
        CaseSetting::new(&GEN_ESN0_DB, es),
    ];
    kv.extend(extra.iter().cloned());
    let x = generate(&DVBS, 4e6, 0.8, &kv);
    let mut rx = Rx::new(&DVBS, &[], &recording(4e6, 1.5e9));
    rx.feed_mixed(&x);
    rx.dump();
    rx
}

#[test]
fn dvbs2_frames_of_every_modulation_round_trip() {
    for (m, rate, short, pilots, es, cfo) in [
        (Modulation::Qpsk, CodeRate::R12, true, false, 8.0, 30e3),
        (Modulation::Psk8, CodeRate::R35, true, true, 11.0, -40e3),
        (Modulation::Apsk16, CodeRate::R34, true, true, 15.0, 10e3),
        (Modulation::Apsk32, CodeRate::R45, true, true, 18.0, 0.0),
        (Modulation::Qpsk, CodeRate::R910, false, false, 12.0, 5e3),
    ] {
        let rx = s2(
            m,
            rate,
            short,
            pilots,
            es,
            &[CaseSetting::new(&GEN_OFFSET_HZ, cfo)],
        );
        let c = rx.check();
        assert!(
            c.good > 150 && c.wrong == 0 && c.gaps == 0,
            "{m:?} {rate:?}: {c:?}"
        );
        assert_eq!(rx.at(&D::STANDARD), Some(Received::Dvbs2));
        assert_eq!(rx.at(&D::MODULATION), Some(m));
        assert_eq!(rx.at(&D::CODE_RATE), Some(rate));
        let frame = if short {
            FrameSize::Short
        } else {
            FrameSize::Normal
        };
        assert_eq!(rx.at(&D::FRAME), Some(frame));
        assert_eq!(rx.at(&D::PILOTS), Some(pilots));
        assert_eq!(rx.at(&D::ROLL_OFF_SOURCE), Some(RollOffSource::Signalled));
        assert_eq!(rx.at(&D::BCH_BAD), Some(0));
        assert_eq!(rx.at(&D::TS_LOCK), Some(true));
        assert_eq!(
            rx.get(&monitor::SERVICES),
            Some("sdrkit test"),
            "{m:?} {rate:?}"
        );
    }
}

/// A signal with no noise at all is found and decoded, at every roll-off: its spectrum has no noise floor, and the skirts
/// of the carrier fall far under the floor that the carrier survey assumes.
#[test]
fn noise_free_signal_locks() {
    for (roll_off, rate, symbol_rate) in [
        (GenRollOff::R35, 4e6, 1e6),
        (GenRollOff::R25, 4e6, 1e6),
        (GenRollOff::R20, 4e6, 1e6),
        (GenRollOff::R25, 6e6, 2_586_148.0),
    ] {
        let kv = [
            CaseSetting::new(&GEN_RATE, CodeRate::R35),
            CaseSetting::new(&GEN_SYMBOL_RATE, symbol_rate),
            CaseSetting::new(&GEN_ROLLOFF, roll_off),
        ];
        let x = generate(&DVBS, rate, 0.6, &kv);
        let mut rx = Rx::new(&DVBS, &[], &recording(rate, 1.5e9));
        rx.feed(&x, 16384);
        rx.finish();
        let c = rx.check();
        let what = format!("roll-off {} at {symbol_rate} sym/s", roll_off.value());
        assert!(c.good > 100 && c.wrong == 0 && c.gaps == 0, "{what}: {c:?}");
        assert_eq!(rx.at(&D::STANDARD), Some(Received::Dvbs2), "{what}");
        assert_eq!(rx.at(&D::BCH_BAD), Some(0), "{what}");
        let misfit = rx.num("misfit");
        assert!(misfit < 1.0, "{what}: misfit {misfit}");
        let mer = rx.num("mer_db");
        assert!(mer > 24.0, "{what}: MER {mer}");
    }
}

/// The DVB-S hunt locks at its first try wherever its window starts in the puncturing period. A window that starts with a
/// Y bit of the mother code needs an erased X before it; without it only one start in two (rate 3/4) to one in four (rate
/// 7/8) could be decoded.
#[test]
fn dvbs_hunt_takes_every_symbol_alignment() {
    use sdrkit_proto_dvbs2::front::{Front, FrontConfig};
    use sdrkit_proto_dvbs2::inner::PunctureRate;
    use sdrkit_proto_dvbs2::sync::{Standard, Sync};
    for rate in [PunctureRate::R23, PunctureRate::R34, PunctureRate::R78] {
        let kv = [
            CaseSetting::new(&GEN_STANDARD, GenStandardChoice::Dvbs),
            CaseSetting::new(&GEN_RATE, CodeRate::from_key(rate.name()).unwrap()),
            CaseSetting::new(&GEN_SYMBOL_RATE, 2e6),
            CaseSetting::new(&GEN_NOISE, NoiseKind::EsN0),
            CaseSetting::new(&GEN_ESN0_DB, 12.0),
        ];
        let x = generate(&DVBS, 4e6, 0.6, &kv);
        let mut front = Front::new(4e6, FrontConfig::default()).unwrap();
        let mut symbols = Vec::new();
        for c in x.chunks(8192) {
            front.process(c, &mut symbols);
        }
        // Past the symbols that the timing loop needed to settle.
        let settled = &symbols[200_000..];
        for start in 0..8 {
            let mut sync = Sync::new(Standard::S, 0);
            sync.push(&settled[start..start + 70_000]);
            assert_eq!(
                (sync.report.standard, sync.report.rate),
                (Some(Received::Dvbs), Some(rate)),
                "rate {} from symbol {start}",
                rate.name()
            );
        }
    }
}

/// A symbol rate above half the sample rate is found: its cyclic line lies above the band of the input, and the survey
/// looks for it in samples interpolated by 2.
#[test]
fn symbol_rate_above_half_the_sample_rate_is_found() {
    let kv = [
        CaseSetting::new(&GEN_RATE, CodeRate::R35),
        CaseSetting::new(&GEN_SYMBOL_RATE, 3.3e6),
        CaseSetting::new(&GEN_NOISE, NoiseKind::EsN0),
        CaseSetting::new(&GEN_ESN0_DB, 12.0),
    ];
    let x = generate(&DVBS, 6e6, 0.5, &kv);
    let mut rx = Rx::new(&DVBS, &[], &recording(6e6, 1.5e9));
    rx.feed(&x, 16384);
    rx.finish();
    rx.dump();
    let rs = rx.num("symbol_rate_hz");
    assert!((rs - 3.3e6).abs() < 1000.0, "{rs}");
    let c = rx.check();
    assert!(c.good > 100 && c.wrong == 0 && c.gaps == 0, "{c:?}");
    assert_eq!(rx.at(&D::BCH_BAD), Some(0));
}

/// The packets delivered are the same in every run and for every size of the pieces fed.
#[test]
fn the_size_of_the_pieces_does_not_change_the_output() {
    let kv = [
        CaseSetting::new(&GEN_MODULATION, Modulation::Psk8),
        CaseSetting::new(&GEN_RATE, CodeRate::R35),
        CaseSetting::new(&GEN_SHORT, true),
        CaseSetting::new(&GEN_SYMBOL_RATE, 1e6),
        CaseSetting::new(&GEN_NOISE, NoiseKind::EsN0),
        CaseSetting::new(&GEN_ESN0_DB, 9.0),
    ];
    let x = generate(&DVBS, 4e6, 0.8, &kv);
    let run = |piece: Option<usize>, finish: bool| {
        let mut rx = Rx::new(&DVBS, &[], &recording(4e6, 1.5e9));
        match piece {
            Some(n) => rx.feed(&x, n),
            None => rx.feed_mixed(&x),
        }
        if finish {
            rx.finish();
        }
        rx.packets
    };
    let whole = run(Some(50_000), true);
    assert!(whole.len() > 500, "{}", whole.len());
    assert_eq!(run(None, true), whole);
    assert_eq!(run(Some(x.len()), true), whole);
    // Without the end of the stream only the frames of the last pass are held back.
    let open = run(Some(50_000), false);
    assert_eq!(run(Some(50_000), false), open);
    assert!(open.len() + 200 > whole.len() && whole.starts_with(&open));
}

#[test]
fn gse_ip_and_dummy_frames() {
    let rx = s2(
        Modulation::Qpsk,
        CodeRate::R23,
        true,
        false,
        9.0,
        &[
            CaseSetting::new(&GEN_GSE, true),
            CaseSetting::new(&GEN_DUMMY_EVERY, 3),
        ],
    );
    let ip = rx.num("ip.ip_packets");
    assert!(ip > 50.0, "{ip}");
    assert_eq!(rx.num("ip.encrypted_flows"), 0.0);
    assert!(rx.num("dummy_frames") > 5.0);
    assert_eq!(rows(rx.results(), "streams"), 1);
}

#[test]
fn dvbs_round_trip_every_rate_with_offsets() {
    use sdrkit_proto_dvbs2::inner::PunctureRate;
    for (i, rate) in PunctureRate::ALL.into_iter().enumerate() {
        let kv = vec![
            CaseSetting::new(&GEN_STANDARD, GenStandardChoice::Dvbs),
            CaseSetting::new(&GEN_RATE, CodeRate::from_key(rate.name()).unwrap()),
            CaseSetting::new(&GEN_SYMBOL_RATE, 2e6),
            CaseSetting::new(&GEN_NOISE, NoiseKind::EsN0),
            CaseSetting::new(&GEN_ESN0_DB, 12.0),
            CaseSetting::new(&GEN_OFFSET_HZ, [-150e3, 40e3, 0.0, 500e3, -20e3][i]),
            CaseSetting::new(&GEN_INVERTED, i == 2),
        ];
        let x = generate(&DVBS, 4e6, 1.6, &kv);
        let mut rx = Rx::new(&DVBS, &[], &recording(4e6, 1.5e9));
        rx.feed(&x, 65536);
        rx.dump();
        let c = rx.check();
        assert!(
            c.good > 500 && c.wrong == 0 && c.flagged == 0 && c.gaps == 0,
            "{rate:?}: {c:?}"
        );
        assert_eq!(rx.at(&D::CODE_RATE), Some(CodeRate::of_punctured(rate)));
        assert_eq!(rx.at(&D::INVERTED), Some(i == 2));
    }
}

/// DVB-S at rate 3/4 and 2 samples per symbol locks at Es/N0 6 dB in its first survey: the symbol rate comes from the
/// cyclic line, which at this ratio is at half the sample rate.
#[test]
fn dvbs_three_quarters_at_2_samples_per_symbol_locks_at_6_db() {
    let kv = [
        CaseSetting::new(&GEN_STANDARD, GenStandardChoice::Dvbs),
        CaseSetting::new(&GEN_RATE, CodeRate::R34),
        CaseSetting::new(&GEN_SYMBOL_RATE, 1e6),
        CaseSetting::new(&GEN_NOISE, NoiseKind::EsN0),
        CaseSetting::new(&GEN_ESN0_DB, 6.0),
    ];
    let x = generate(&DVBS, 2e6, 1.5, &kv);
    let mut rx = Rx::new(&DVBS, &[], &recording(2e6, 1.5e9));
    rx.feed(&x, 16384);
    rx.finish();
    let rs = rx.num("symbol_rate_hz");
    assert!((rs - 1e6).abs() < 200.0, "{rs}");
    let c = rx.check();
    assert!(
        c.good > 1000 && c.wrong == 0 && c.flagged == 0 && c.gaps == 0,
        "{c:?}"
    );
}
