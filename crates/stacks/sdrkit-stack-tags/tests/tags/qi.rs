//! The Qi reader: the byte and packet formats of section 5.2.2, the named packets, the list of nine packets at 250 ksps with
//! a bit clock 4 % off, other depths, offsets and wander, refused packets, a change after the last bit, a noise sweep and long
//! runs of power signal and of noise alone.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_DRIFT_HZ_S, GEN_LEVEL_DBFS, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ,
    GEN_SNR_DB,
};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::Values;
use sdrkit_dev_test::Harness;
use sdrkit_stack_tags::qi::gen::{
    list_s, GEN_BIT_CLOCK_PPM, GEN_DEPTH, GEN_END_EDGE_BITS, GEN_PACKETS, GEN_WANDER_DEPTH,
    GEN_WANDER_HZ,
};
use sdrkit_stack_tags::qi::packet::{
    byte_bits, message_len, packet_bits, Packet, PacketReader, Step,
};
use sdrkit_stack_tags::qi::DESCRIPTOR;
use sdrkit_stack_tags::typed::{QiFields, QiMessage, QiPacketEvent, QI_PACKET};

const DIAL: f64 = 140e3;

fn h() -> Harness {
    Harness::new(&DESCRIPTOR)
}

fn packets(r: &Streamed) -> Vec<QiPacketEvent<'_>> {
    r.events
        .iter()
        .filter_map(|e| QI_PACKET.read::<QiPacketEvent>(e))
        .collect()
}

fn fields(r: &Streamed) -> QiFields {
    QiFields::read(r.results.fields()).expect("the fields")
}

fn seconds(h: &Harness) -> f64 {
    let v: Values = h.values().expect("the settings");
    list_s(&v)
}

fn bits(s: &str) -> Vec<u8> {
    s.bytes()
        .filter(|c| !c.is_ascii_whitespace())
        .map(|c| c - b'0')
        .collect()
}

#[test]
fn the_worked_packet_01_a0_has_the_bits_of_the_byte_format() {
    let want = bits("111111111111 0 10000000 0 1 0 00000101 1 1 0 10000101 0 1");
    assert_eq!(packet_bits(12, &[0x01, 0xA0, 0xA1]), want);
    // Fig. 37 of the standard: 0x35 holds four ones, so its parity bit is one.
    assert_eq!(byte_bits(0x35), [0, 1, 0, 1, 0, 1, 1, 0, 0, 1, 1]);
}

#[test]
fn the_message_length_follows_the_header() {
    let got: Vec<usize> = [0x1F, 0x20, 0x7F, 0x80, 0xDF, 0xE0, 0xFF]
        .iter()
        .map(|&h| message_len(h))
        .collect();
    assert_eq!(got, [1, 2, 7, 8, 19, 20, 27]);
}

fn read_bits(b: &[u8]) -> Vec<Step> {
    let mut r = PacketReader::default();
    b.iter()
        .map(|&x| r.bit(x))
        .filter(|s| *s != Step::Nothing)
        .collect()
}

#[test]
fn four_preamble_bits_and_a_start_bit_begin_a_packet_and_three_do_not() {
    let p = Packet {
        header: 0x01,
        message: vec![0xA0],
    };
    assert_eq!(
        read_bits(&packet_bits(4, &[0x01, 0xA0, 0xA1])),
        [Step::Packet(p)]
    );
    assert_eq!(read_bits(&packet_bits(3, &[0x01, 0xA0, 0xA1])), []);
    // A wrong checksum and a wrong parity bit fail.
    assert_eq!(
        read_bits(&packet_bits(11, &[0x01, 0xA0, 0xA0])),
        [Step::Failed]
    );
    let mut b = packet_bits(11, &[0x01, 0xA0, 0xA1]);
    b[11 + 9] ^= 1;
    assert_eq!(read_bits(&b), [Step::Failed]);
}

#[test]
fn each_named_packet_gives_its_fields() {
    let p = |h: u8, m: &[u8]| Packet {
        header: h,
        message: m.to_vec(),
    };
    assert_eq!(p(0x01, &[0xA0]).parse().value, Some(160));
    let e = p(0x02, &[0x01]).parse();
    assert_eq!((e.value, e.end_reason), (Some(1), Some("charge complete")));
    assert_eq!(
        p(0x02, &[0x0B]).parse().end_reason,
        Some("restart power transfer")
    );
    assert_eq!(
        p(0x02, &[0x07]).parse().end_reason,
        Some("reserved (reconfigure, no longer used)")
    );
    assert_eq!(p(0x03, &[0xF6]).parse().value, Some(-10));
    assert_eq!(p(0x04, &[0x50]).parse().value, Some(80));
    assert_eq!(p(0x05, &[0x64]).parse().charge.as_deref(), Some("100 %"));
    assert_eq!(
        p(0x05, &[0xFF]).parse().charge.as_deref(),
        Some("no rechargeable store or no information")
    );
    assert_eq!(p(0x06, &[0x0A]).parse().value, Some(10));
    let c = p(0x51, &[0x0C, 0, 0, 0, 0]).parse();
    assert_eq!((c.power_class, c.max_power), (Some(0), Some(12)));
    let i = p(0x71, &[0x11, 0x00, 0x1D, 0x12, 0x34, 0x56, 0x78]).parse();
    assert_eq!(i.version.as_deref(), Some("1.1"));
    assert_eq!(i.manufacturer.as_deref(), Some("001D"));
    assert_eq!(i.device_id.as_deref(), Some("12345678"));
    assert_eq!(i.extended, Some(false));
    assert_eq!(p(0x84, &[0; 8]).kind(), QiMessage::Proprietary);
}

/// Checks the nine rows of the default list.
fn nine(r: &Streamed, at: &str) {
    let got = packets(r);
    let f = fields(r);
    let summary: Vec<(String, Option<i64>)> = got
        .iter()
        .map(|p| (p.header.to_string(), p.value))
        .collect();
    assert_eq!(
        got.len(),
        9,
        "{at}: {summary:?}, {} failed",
        f.packets_failed
    );
    let want = [
        ("01", QiMessage::SignalStrength, Some(160)),
        ("02", QiMessage::EndPowerTransfer, Some(1)),
        ("03", QiMessage::ControlError, Some(-10)),
        ("04", QiMessage::ReceivedPower, Some(80)),
        ("05", QiMessage::ChargeStatus, Some(100)),
        ("06", QiMessage::HoldOff, Some(10)),
        ("51", QiMessage::Configuration, None),
        ("71", QiMessage::Identification, None),
        ("04", QiMessage::ReceivedPower, Some(18)),
    ];
    for (p, (hd, m, v)) in got.iter().zip(want) {
        assert_eq!((p.header, p.message, p.value), (hd, m, v), "{at}");
    }
    assert_eq!(got[1].end_reason, Some("charge complete"), "{at}");
    assert_eq!(
        (got[6].power_class, got[6].max_power),
        (Some(0), Some(12)),
        "{at}"
    );
    assert_eq!(
        (got[7].version, got[7].manufacturer, got[7].device_id),
        (Some("1.1"), Some("001D"), Some("12345678")),
        "{at}"
    );
    assert_eq!(f.packets_failed, 0, "{at}");
}

fn run(h: Harness, at: &str) {
    let s = seconds(&h);
    let r = stream(&DESCRIPTOR, &h, 250e3, DIAL, DIAL, s, 1);
    nine(&r, at);
}

#[test]
fn the_nine_packets_read_with_their_fields() {
    run(h(), "defaults");
}

#[test]
fn a_bit_clock_4_percent_off_and_other_depths_read() {
    for ppm in [40_000.0, -40_000.0] {
        run(
            h().set(&GEN_BIT_CLOCK_PPM, ppm),
            &format!("clock {ppm} ppm"),
        );
    }
    for d in [0.10, 0.03] {
        run(h().set(&GEN_DEPTH, d), &format!("depth {d}"));
    }
}

#[test]
fn offsets_drift_and_wander_read() {
    run(h().set(&GEN_DRIFT_HZ_S, 600.0), "drift 600 Hz/s");
    for off in [3000.0, -3000.0] {
        run(h().set(&GEN_OFFSET_HZ, off), &format!("offset {off} Hz"));
    }
    run(
        h().set(&GEN_WANDER_HZ, 0.5).set(&GEN_WANDER_DEPTH, 0.04),
        "wander 0.5 Hz, 4 %",
    );
    // A dial 30 kHz above the centre that the receiver is told.
    let hh = h().set(&GEN_OFFSET_HZ, 30e3);
    let r = stream(&DESCRIPTOR, &hh, 250e3, DIAL - 30e3, DIAL, seconds(&hh), 1);
    nine(&r, "told 30 kHz");
}

#[test]
fn a_change_after_the_last_stop_bit_does_not_fail_the_packet() {
    for e in [0.5, 0.77, 1.2, 3.0] {
        run(
            h().set(&GEN_END_EDGE_BITS, e),
            &format!("a change {e} bit after the last middle"),
        );
    }
}

#[test]
fn a_wrong_checksum_and_a_wrong_parity_bit_give_no_row() {
    // 01 A0 with the checksum A0; 04 50 with its first parity bit (preamble 12, start 1, data 8) inverted.
    let hh = h().set(&GEN_PACKETS, "x 01 A0 A0; f21 04 50");
    let r = stream(&DESCRIPTOR, &hh, 250e3, DIAL, DIAL, seconds(&hh), 1);
    assert!(packets(&r).is_empty());
    assert_eq!(fields(&r).packets_failed, 2);
}

#[test]
fn packets_hold_down_to_55_db_without_an_extra_row() {
    let mut line = String::new();
    for cnr in [70.0, 60.0, 55.0, 50.0, 45.0, 40.0] {
        let mut good = 0;
        for seed in 1..=3u64 {
            let hh = h()
                .set(&GEN_NOISE, NoiseKind::Snr)
                .set(&GEN_SNR_DB, cnr)
                .seed(seed);
            let r = stream(&DESCRIPTOR, &hh, 250e3, DIAL, DIAL, seconds(&hh), seed);
            let got = packets(&r);
            assert!(got.len() <= 9, "{cnr} dB seed {seed}: {} rows", got.len());
            good += got.len();
        }
        line.push_str(&format!("{cnr} dB {good}/27; "));
        if cnr >= 55.0 {
            assert_eq!(good, 27, "{line}");
        }
    }
    println!("qi, 5 % depth, CNR in 10 kHz: {line}");
}

#[test]
fn fifteen_minutes_of_power_signal_and_of_noise_alone_give_no_packet() {
    // 15 runs of 60 s at 50 ksps, a power signal without packets 45 dB over the noise in 10 kHz and noise alone in turn.
    let mut failed = 0;
    for k in 0..15u64 {
        let hh = if k % 2 == 0 {
            h().set(&GEN_PACKETS, "")
                .set(&GEN_NOISE, NoiseKind::Snr)
                .set(&GEN_SNR_DB, 45.0)
        } else {
            h().set(&GEN_PACKETS, "")
                .set(&GEN_LEVEL_DBFS, -150.0)
                .set(&GEN_NOISE, NoiseKind::Floor)
                .set(&GEN_NOISE_FLOOR_DBFS, -30.0)
        };
        let r = stream(&DESCRIPTOR, &hh.seed(k + 1), 50e3, DIAL, DIAL, 60.0, k + 1);
        assert!(packets(&r).is_empty(), "run {k}");
        assert_eq!(fields(&r).packets_ok, 0, "run {k}");
        failed += fields(&r).packets_failed;
    }
    println!("qi, 900 s of power signal and noise: 0 packets, {failed} failed");
}
