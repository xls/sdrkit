//! The Gen2 reader: the CRCs of Annex F and the bit vectors of Annex A, the command layouts, the list of eight commands at
//! three rates and several timings, a reader clock 1 % off, offsets with drift, refused frames, a noise sweep and long runs of
//! carrier and of noise alone.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_DRIFT_HZ_S, GEN_LEVEL_DBFS, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ,
    GEN_SNR_DB,
};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_tags::rfid_uhf::command::{
    bits_of, decode, ebv, encode, read_ebv, value_of, Refused,
};
use sdrkit_stack_tags::rfid_uhf::gen::{
    parse_commands, pass_s, GEN_COMMANDS, GEN_DATA1_TARI, GEN_DEPTH, GEN_READER_CLOCK_PPM,
    GEN_TARI_US, GEN_TRCAL_RTCAL,
};
use sdrkit_stack_tags::rfid_uhf::{DESCRIPTOR, LIST_UNCHECKED};
use sdrkit_stack_tags::typed::{Gen2Command, RfidCommandEvent, RfidUhfFields, RFID_COMMAND};
use sdrkit_util_bits::crc::catalog::{CRC16_GENIBUS, CRC5_EPC_C1G2};

const DIAL: f64 = 866.3e6;

fn h() -> Harness {
    Harness::new(&DESCRIPTOR)
}

fn rows(r: &Streamed) -> Vec<RfidCommandEvent<'_>> {
    r.events
        .iter()
        .filter_map(|e| RFID_COMMAND.read::<RfidCommandEvent>(e))
        .collect()
}

fn fields(r: &Streamed) -> RfidUhfFields {
    RfidUhfFields::read(r.results.fields()).expect("the fields")
}

fn once(h: &Harness, rate: f64) -> Streamed {
    let s = pass_s(&h.values().expect("the settings"));
    stream(&DESCRIPTOR, h, rate, DIAL, DIAL, s, 1)
}

#[test]
fn the_crcs_follow_annex_f() {
    assert_eq!(CRC5_EPC_C1G2.checksum(b"123456789"), 0x00);
    // Query DR 0, Miller 4, TRext 1, Sel SL, S2, target B, Q 4: CRC-5 01011, and the 22 bits leave the register at 0.
    let q = encode::query(0, 2, 1, 3, 2, 1, 4);
    assert_eq!(&q[17..], &[0, 1, 0, 1, 1]);
    assert!(CRC5_EPC_C1G2.verify_bits(&q));
    // Annex F.3: the StoredCRC of StoredPC and EPC words. With the CRC appended the register ends at 1D0F, whose ones
    // complement the engine returns.
    for (words, crc) in [
        (&[0x0000u16][..], 0xE2F0u16),
        (&[0x0800, 0x1111], 0xCCAE),
        (&[0x1000, 0x1111, 0x2222], 0x968F),
    ] {
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
        assert_eq!(CRC16_GENIBUS.checksum(&bytes), crc);
        let mut bits: Vec<u8> = bytes
            .iter()
            .flat_map(|&b| bits_of(u64::from(b), 8))
            .collect();
        bits.extend(bits_of(u64::from(crc), 16));
        assert_eq!(CRC16_GENIBUS.bits(&bits) ^ 0xFFFF, 0x1D0F);
    }
}

#[test]
fn bit_vectors_follow_annex_a() {
    assert_eq!(ebv(127), bits_of(0x7F, 8));
    assert_eq!(ebv(128), bits_of(0b1000_0001_0000_0000, 16));
    assert_eq!(ebv(16383), bits_of(0b1111_1111_0111_1111, 16));
    assert_eq!(ebv(16384), bits_of(0b1000_0001_1000_0000_0000_0000, 24));
    for v in [0u64, 5, 127, 128, 16383, 16384] {
        let b = ebv(v);
        assert_eq!(read_ebv(&b), Some((v, b.len())));
    }
}

#[test]
fn each_command_of_the_list_decodes_and_a_flipped_bit_does_not() {
    let frames =
        parse_commands(sdrkit_stack_tags::rfid_uhf::gen::DEFAULT_COMMANDS).expect("the list");
    let names: Vec<Gen2Command> = frames
        .iter()
        .map(|f| {
            decode(&f.bits, f.trcal)
                .expect("a command")
                .command
                .expect("its name")
        })
        .collect();
    use Gen2Command::*;
    assert_eq!(
        names,
        [Query, Ack, QueryRep, QueryAdjust, Nak, Select, ReqRn, Read]
    );
    // A flipped bit after the code fails the CRC of every command that has one.
    for f in frames
        .iter()
        .filter(|f| f.trcal || CRC16_GENIBUS.verify_bits(&f.bits))
    {
        let mut g = f.bits.clone();
        let k = g.len() - 20;
        g[k] ^= 1;
        assert!(
            decode(&g, f.trcal).is_err(),
            "code {:b}",
            value_of(&f.bits[..4])
        );
    }
    // The other commands, whose secret parts are never kept.
    let w = decode(&encode::write(3, 2, 0xC0DE, 0x1111), false).expect("Write");
    assert_eq!(
        (w.command, w.bank, w.pointer, w.rn16.as_deref()),
        (Some(Write), Some("User"), Some(2), Some("1111"))
    );
    let k = decode(&encode::kill(0x1234, 0, 0x2222), false).expect("Kill");
    assert_eq!((k.command, k.rn16.as_deref()), (Some(Kill), Some("2222")));
    let l = decode(&encode::lock(0xABCDE, 0x3333), false).expect("Lock");
    assert_eq!(l.lock_payload.as_deref(), Some("ABCDE"));
    let a = decode(&encode::access(0x5555, 0x4444), false).expect("Access");
    assert_eq!((a.command, a.rn16.as_deref()), (Some(Access), Some("4444")));
    assert_eq!(
        decode(&encode::query(0, 0, 0, 0, 0, 0, 0), false),
        Err(Refused::Layout)
    );
}

/// Checks the eight rows of the default list.
fn eight(r: &Streamed, at: &str) {
    let got = rows(r);
    let f = fields(r);
    let names: Vec<Gen2Command> = got.iter().map(|e| e.command).collect();
    use Gen2Command::*;
    assert_eq!(
        names,
        [Query, Ack, QueryRep, QueryAdjust, Nak, Select, ReqRn, Read],
        "{at}: {} failed",
        f.commands_failed
    );
    let q = &got[0];
    assert_eq!(
        (q.q, q.session, q.slots, q.target, q.sel, q.coding, q.pilot, q.dr),
        (
            Some(4),
            Some(2),
            Some(16),
            Some("B"),
            Some("SL"),
            Some("Miller 4"),
            Some(true),
            Some("8")
        ),
        "{at}"
    );
    assert_eq!(got[1].rn16, Some("BEEF"), "{at}");
    assert_eq!(got[2].session, Some(2), "{at}");
    assert_eq!((got[3].session, got[3].updn), (Some(1), Some("+1")), "{at}");
    let s = &got[5];
    assert_eq!(
        (s.target, s.action, s.bank, s.pointer, s.mask, s.truncate),
        (
            Some("SL"),
            Some(0),
            Some("EPC"),
            Some(32),
            Some("1010011100110101"),
            Some(false)
        ),
        "{at}"
    );
    assert_eq!(got[6].rn16, Some("1234"), "{at}");
    let rd = &got[7];
    assert_eq!(
        (rd.bank, rd.pointer, rd.word_count, rd.rn16),
        (Some("User"), Some(5), Some(4), Some("A5A5")),
        "{at}"
    );
    assert_eq!((f.commands_ok, f.commands_failed), (8, 0), "{at}");
}

#[test]
fn the_eight_commands_read_with_their_fields_and_timings() {
    let r = once(&h(), 2e6);
    eight(&r, "defaults");
    let f = fields(&r);
    let (tari, rtcal, trcal) = (f.tari_us.unwrap(), f.rtcal_us.unwrap(), f.trcal_us.unwrap());
    assert!(
        (tari - 12.5).abs() < 0.3 && (rtcal - 34.375).abs() < 0.5,
        "{tari} {rtcal}"
    );
    assert!((trcal - 68.75).abs() < 0.8, "{trcal}");
    let blf = f.blf_dr8_khz.unwrap();
    assert!((blf - 8.0 / 68.75e-6 / 1e3).abs() < 2.0, "{blf}");
}

#[test]
fn three_rates_and_the_timings_of_the_standard_read() {
    for rate in [1e6, 2e6, 2.4e6] {
        for (tari, trcal) in [(12.5, 2.5), (25.0, 2.5), (6.25, 1.5)] {
            if tari == 6.25 && rate < 2.4e6 {
                continue;
            }
            for d1 in [1.5, 2.0] {
                let hh = h()
                    .set(&GEN_TARI_US, tari)
                    .set(&GEN_TRCAL_RTCAL, trcal)
                    .set(&GEN_DATA1_TARI, d1)
                    .set(&GEN_DEPTH, 0.85);
                eight(
                    &once(&hh, rate),
                    &format!("{rate} sps, Tari {tari} us, data-1 {d1}"),
                );
            }
        }
    }
}

#[test]
fn a_reader_clock_1_percent_off_and_offsets_with_drift_read() {
    for ppm in [10_000.0, -10_000.0] {
        eight(
            &once(&h().set(&GEN_READER_CLOCK_PPM, ppm), 2e6),
            &format!("clock {ppm} ppm"),
        );
    }
    for off in [0.0, 80e3, -80e3] {
        let hh = h().set(&GEN_OFFSET_HZ, off).set(&GEN_DRIFT_HZ_S, 20e3);
        eight(&once(&hh, 2e6), &format!("offset {off} Hz, drift 20 kHz/s"));
    }
    let hh = h().set(&GEN_OFFSET_HZ, 300e3);
    let s = pass_s(&hh.values().unwrap());
    eight(
        &stream(&DESCRIPTOR, &hh, 2e6, DIAL - 300e3, DIAL, s, 1),
        "told 300 kHz",
    );
}

#[test]
fn a_query_and_a_read_with_a_wrong_bit_give_no_row() {
    let hh = h().set(
        &GEN_COMMANDS,
        "query dr=0 m=2 trext=1 sel=3 session=2 target=1 q=4 flip=19; read bank=3 pointer=5 count=4 rn=A5A5 flip=30; ack rn=BEEF",
    );
    let r = once(&hh, 2e6);
    let got: Vec<Gen2Command> = rows(&r).iter().map(|e| e.command).collect();
    assert_eq!(got, [Gen2Command::Ack]);
    assert_eq!(fields(&r).commands_failed, 2);
}

#[test]
fn unchecked_commands_count_without_rows_when_asked() {
    let r = once(&h().set(&LIST_UNCHECKED, false), 2e6);
    assert_eq!(rows(&r).len(), 4);
    let f = fields(&r);
    assert_eq!((f.commands_ok, f.count_ack, f.count_nak), (8, 1, 1));
}

#[test]
fn commands_hold_down_to_20_db_without_an_extra_row() {
    let mut line = String::new();
    for cnr in [30.0, 24.0, 20.0, 17.0, 14.0, 11.0] {
        let mut good = 0;
        for seed in 1..=4u64 {
            let hh = h()
                .set(&GEN_NOISE, NoiseKind::Snr)
                .set(&GEN_SNR_DB, cnr)
                .seed(seed);
            let s = pass_s(&hh.values().unwrap());
            let r = stream(&DESCRIPTOR, &hh, 2e6, DIAL, DIAL, s, seed);
            let got = rows(&r);
            assert!(got.len() <= 8, "{cnr} dB seed {seed}: {} rows", got.len());
            good += got.len();
        }
        line.push_str(&format!("{cnr} dB {good}/32; "));
        if cnr >= 20.0 {
            assert_eq!(good, 32, "{line}");
        }
    }
    println!("rfid_uhf, 90 % depth, Tari 12.5 us, CNR in 600 kHz: {line}");
}

#[test]
fn carrier_and_noise_alone_give_no_command() {
    // 15 runs of 15 s at 2 Msps: an unmodulated carrier 20 dB over the noise in 600 kHz and noise alone in turn.
    let mut failed = 0;
    for k in 0..15u64 {
        let hh = if k % 2 == 0 {
            h().set(&GEN_COMMANDS, "")
                .set(&GEN_NOISE, NoiseKind::Snr)
                .set(&GEN_SNR_DB, 20.0)
        } else {
            h().set(&GEN_LEVEL_DBFS, -150.0)
                .set(&GEN_NOISE, NoiseKind::Floor)
                .set(&GEN_NOISE_FLOOR_DBFS, -20.0)
        };
        let r = stream(&DESCRIPTOR, &hh.seed(k + 1), 2e6, DIAL, DIAL, 15.0, k + 1);
        assert!(rows(&r).is_empty(), "run {k}");
        let f = fields(&r);
        assert_eq!(f.commands_ok, 0, "run {k}");
        assert!(!f.reader_carrier, "run {k}");
        failed += f.commands_failed;
    }
    println!("rfid_uhf, 225 s of carrier and of noise alone: 0 commands, {failed} frames failed");
}
