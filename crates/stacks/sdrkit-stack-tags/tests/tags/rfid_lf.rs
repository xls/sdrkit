//! The EM4100 reader: the frame of the data sheet layout, the tags at both rates, codes and load polarities, a tag clock
//! 4 % off, carrier offsets, a noise sweep, two tags in turn and a long carrier without a tag.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_DRIFT_HZ_S, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_tags::rfid_lf::frame::{decode, encode, hex, Refused};
use sdrkit_stack_tags::rfid_lf::gen::{
    GEN_BIT_CLOCK_PPM, GEN_CODING, GEN_ID, GEN_ID_S, GEN_INVERT, GEN_RF_DIV, GEN_START_S,
};
use sdrkit_stack_tags::rfid_lf::DESCRIPTOR;
use sdrkit_stack_tags::typed::{
    RfidBitRate, RfidCoding, RfidLfFields, RfidTagEvent, TagsState, TagsStatusFields, RFID_TAG,
};

const CARRIER: f64 = 125e3;

fn h() -> Harness {
    Harness::new(&DESCRIPTOR)
}

fn noisy(cnr: f64) -> Harness {
    h().set(&GEN_NOISE, NoiseKind::Snr).set(&GEN_SNR_DB, cnr)
}

fn tags(r: &Streamed) -> Vec<RfidTagEvent<'_>> {
    r.events
        .iter()
        .filter_map(|e| RFID_TAG.read::<RfidTagEvent>(e))
        .collect()
}

fn fields(r: &Streamed) -> RfidLfFields<'_> {
    RfidLfFields::read(r.results.fields()).expect("the fields")
}

#[test]
fn the_frame_of_01_23456789_follows_the_data_sheet_layout() {
    let f = encode(0x01_2345_6789);
    assert_eq!(hex(&f), "FF80653254C7C642");
    let rows: Vec<u8> = (0..10).map(|r| f[9 + 5 * r + 4]).collect();
    assert_eq!(rows, [0, 1, 1, 0, 1, 0, 0, 1, 1, 0]);
    assert_eq!(&f[59..63], &[0, 0, 0, 1]);
    assert_eq!(decode(&f), Ok(0x01_2345_6789));
    for d in [0xFF_FFFF_FFFFu64, 0x00_0000_0001, 0xA5_5A5A_A5C3] {
        assert_eq!(decode(&encode(d)), Ok(d));
    }
}

#[test]
fn a_flipped_data_bit_fails_its_row_parity_and_its_column_parity() {
    let f = encode(0x01_2345_6789);
    for row in 0..10 {
        for col in 0..4 {
            let mut g = f;
            g[9 + 5 * row + col] ^= 1;
            assert_eq!(decode(&g), Err(Refused::Parity));
            let row_odd = g[9 + 5 * row..9 + 5 * row + 5]
                .iter()
                .fold(0, |a, &b| a ^ b)
                == 1;
            let col_odd = (0..10).fold(g[59 + col], |a, r| a ^ g[9 + 5 * r + col]) == 1;
            assert!(row_odd && col_odd, "row {row} column {col}");
        }
    }
    let mut g = f;
    g[63] = 1;
    assert_eq!(decode(&g), Err(Refused::NoFrame));
}

fn read_one(h: Harness, id: &str, rf: RfidBitRate, coding: RfidCoding, invert: bool) {
    let h = h
        .set(&GEN_ID, id)
        .set(&GEN_RF_DIV, rf)
        .set(&GEN_CODING, coding)
        .set(&GEN_INVERT, invert);
    let r = stream(&DESCRIPTOR, &h, 250e3, CARRIER, CARRIER, 1.0, 1);
    let got = tags(&r);
    let f = fields(&r);
    let at = format!("{id} {rf:?} {coding:?} inverted {invert}");
    assert_eq!(got.len(), 1, "{at}: {got:?}");
    assert_eq!(got[0].tag, id, "{at}");
    assert_eq!((got[0].bit_rate, got[0].coding), (rf, coding), "{at}");
    assert_eq!(f.last_tag, Some(id), "{at}");
    assert!(f.reads >= 10, "{at}: {} reads", f.reads);
    assert_eq!(f.refused, 0, "{at}");
}

#[test]
fn each_tag_reads_at_both_rates_in_both_codes_and_both_polarities() {
    for id in ["0123456789", "FFFFFFFFFF", "0000000001", "A55A5AA5C3"] {
        for rf in [RfidBitRate::Rf64, RfidBitRate::Rf32] {
            for coding in [RfidCoding::Manchester, RfidCoding::Biphase] {
                for invert in [false, true] {
                    read_one(h(), id, rf, coding, invert);
                }
            }
        }
    }
}

#[test]
fn a_tag_clock_4_percent_fast_or_slow_reads() {
    for ppm in [40_000.0, -40_000.0] {
        for rf in [RfidBitRate::Rf64, RfidBitRate::Rf32] {
            read_one(
                h().set(&GEN_BIT_CLOCK_PPM, ppm),
                "0123456789",
                rf,
                RfidCoding::Manchester,
                false,
            );
        }
    }
}

#[test]
fn a_carrier_offset_and_a_drift_read() {
    read_one(
        h().set(&GEN_DRIFT_HZ_S, 800.0),
        "0123456789",
        RfidBitRate::Rf64,
        RfidCoding::Manchester,
        false,
    );
    for off in [2500.0, -2500.0] {
        read_one(
            h().set(&GEN_OFFSET_HZ, off),
            "0123456789",
            RfidBitRate::Rf64,
            RfidCoding::Manchester,
            false,
        );
    }
    // A dial 20 kHz above the centre that the receiver is told.
    let hh = h().set(&GEN_OFFSET_HZ, 20e3);
    let r = stream(&DESCRIPTOR, &hh, 250e3, CARRIER - 20e3, CARRIER, 1.0, 1);
    assert_eq!(fields(&r).last_tag, Some("0123456789"));
}

#[test]
fn reads_hold_down_to_45_db_and_noise_never_gives_a_second_tag() {
    let mut line = String::new();
    for cnr in [60.0, 50.0, 45.0, 40.0, 35.0] {
        let mut good = 0;
        for seed in 1..=3u64 {
            let hh = noisy(cnr).seed(seed);
            let r = stream(&DESCRIPTOR, &hh, 250e3, CARRIER, CARRIER, 2.0, seed);
            let f = fields(&r);
            assert!(f.tags <= 1, "{cnr} dB seed {seed}: {} tags", f.tags);
            for t in tags(&r) {
                assert_eq!(t.tag, "0123456789", "{cnr} dB seed {seed}");
            }
            if f.last_tag == Some("0123456789") {
                good += 1;
            }
        }
        line.push_str(&format!("{cnr} dB {good}/3; "));
        if cnr >= 45.0 {
            assert_eq!(good, 3, "{line}");
        }
    }
    println!("rfid_lf, 8 % depth, CNR in 10 kHz: {line}");
}

#[test]
fn two_tags_in_turn_make_two_rows() {
    let hh = h()
        .set(&GEN_ID, "0123456789, A55A5AA5C3")
        .set(&GEN_ID_S, 1.0);
    let r = stream(&DESCRIPTOR, &hh, 250e3, CARRIER, CARRIER, 2.0, 1);
    let got: Vec<&str> = tags(&r).iter().map(|t| t.tag).collect();
    assert_eq!(got, ["0123456789", "A55A5AA5C3"]);
    assert_eq!(fields(&r).tags, 2);
}

#[test]
fn fifteen_minutes_of_carrier_at_40_db_give_no_tag() {
    // The tag never starts: an unmodulated carrier and noise, at 100 ksps (the front end decimates by 3 to 33.3 ksps).
    let hh = noisy(40.0).set(&GEN_START_S, 3600.0);
    let r = stream(&DESCRIPTOR, &hh, 100e3, CARRIER, CARRIER, 900.0, 7);
    let f = fields(&r);
    assert_eq!((f.tags, f.reads), (0, 0));
    let s = TagsStatusFields::read(r.results.fields()).expect("the status");
    println!(
        "rfid_lf, 900 s of carrier at 40 dB: {} refused, state {:?}",
        f.refused, s.state
    );
    assert_ne!(s.state, TagsState::Decoding);
}

#[test]
fn noise_alone_shows_nothing() {
    for seed in 1..=4u64 {
        let hh = h()
            .set(&GEN_START_S, 3600.0)
            .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -150.0)
            .set(&GEN_NOISE, NoiseKind::Floor)
            .seed(seed);
        let r = stream(&DESCRIPTOR, &hh, 250e3, CARRIER, CARRIER, 10.0, seed);
        let f = fields(&r);
        let s = TagsStatusFields::read(r.results.fields()).expect("the status");
        assert_eq!(
            (f.reads, f.refused, f.depth_pct),
            (0, 0, None),
            "seed {seed}"
        );
        assert_eq!(s.state, TagsState::NoSignal, "seed {seed}");
    }
}

#[test]
fn a_setting_changed_during_a_read_keeps_the_clock_and_the_row() {
    // The gate changes at 1 s, which builds the slicer again; the tag of the same row is read on.
    let r = crate::stream::stream_mid(
        &DESCRIPTOR,
        &h(),
        250e3,
        CARRIER,
        2.0,
        1,
        1.0,
        "bit_rates",
        sdrkit_core::Value::Choice(1),
    );
    assert_eq!(tags(&r).len(), 1);
    let s = TagsStatusFields::read(r.results.fields()).expect("the status");
    assert_eq!(s.state, TagsState::Decoding);
    assert!(fields(&r).reads >= 20, "{}", fields(&r).reads);
}
