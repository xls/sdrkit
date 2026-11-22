//! The device families sent from their on-air timing: each worked example frame of the timing specification is encoded, its
//! bits as the stack slices them are the example's, and the stack reads the same field values back from the samples at its
//! rate.

use super::send::Station;
use num_complex::Complex32;
use sdrkit_core::Owner;
use sdrkit_proto_ookdev::slicer::Slicer;
use sdrkit_proto_ookdev::{BitRows, Reading, Train};
use sdrkit_stack_ism::ism::encode as enc;
use sdrkit_stack_ism::ism::gen::train_samples;
use sdrkit_stack_ism::ism::protocols::PROTOCOLS;
use sdrkit_stack_ism::ism::{protocol_list, ISM};
use sdrkit_stack_ism::typed::{IsmDeviceRow, ISM_DEVICES};
use sdrkit_util_bits::line::ManchesterConv;

/// The readings of the protocol list for `t`.
fn decode(t: &Train) -> Vec<Reading> {
    let mut out = Vec::new();
    protocol_list().decode(t, &mut out).unwrap();
    out
}

/// The sets of rows that protocol `num` (1 to 46) slices from `t`.
fn sets(num: usize, t: &Train) -> Vec<BitRows> {
    let mut out = Vec::new();
    Slicer::new()
        .slice(&PROTOCOLS[num - 1].timing, t, |r| out.push(r.clone()))
        .unwrap();
    out
}

/// The rows of every set that protocol `num` slices from `t`, as (bits, octets).
fn rows(num: usize, t: &Train) -> Vec<(usize, Vec<u8>)> {
    let mut out = Vec::new();
    for r in sets(num, t) {
        for i in 0..r.num_rows() {
            out.push((r.len(i), r.row(i).bytes.to_vec()));
        }
    }
    out
}

/// How many rows of `n` bits that start with the octets `b` protocol `num` slices from `t`.
fn count(num: usize, t: &Train, n: usize, b: &[u8]) -> usize {
    rows(num, t)
        .iter()
        .filter(|(len, bytes)| *len == n && bytes.starts_with(b))
        .count()
}

/// The octets of `bits` (most significant bit first, zero padded).
fn pack(bits: &[u8]) -> Vec<u8> {
    bits.chunks(8)
        .map(|c| {
            c.iter()
                .enumerate()
                .fold(0u8, |a, (i, &b)| a | b << (7 - i))
        })
        .collect()
}

/// The octets that row 0 of the first set of protocol `num` decodes to after the pattern `pre` of `bits` bits, in
/// Manchester by `conv`.
fn manchester_after(
    num: usize,
    t: &Train,
    pre: &[u8],
    bits: usize,
    conv: ManchesterConv,
) -> Vec<u8> {
    let r = &sets(num, t)[0];
    let p = r.search(0, 0, pre, bits).expect("the preamble");
    let mut out = BitRows::default();
    r.decode_manchester(0, p + bits, 4096, conv, &mut out);
    out.row(0).bytes[..out.len(0) / 8].to_vec()
}

/// Model, id, channel, battery good and readings.
type DeviceLine = (String, String, Option<String>, Option<bool>, String);

/// The devices that the stack reads from `t` keyed at its rate.
fn heard(t: &Train) -> Vec<DeviceLine> {
    let s = Station {
        stack: &ISM,
        set: &[],
        rate: 1e6,
        hz: 433.92e6,
        tune: 0.0,
    };
    let x: Vec<Complex32> = train_samples(t, s.rate, 0.0)
        .iter()
        .map(|z| z * 0.5)
        .collect();
    let (rx, _) = s.receive(&x, 0.05);
    ISM_DEVICES
        .rows::<IsmDeviceRow>(Owner::results(rx.as_ref()))
        .map(|(_, r)| {
            (
                r.model.to_owned(),
                r.id.to_owned(),
                r.channel.map(str::to_owned),
                r.battery_ok,
                r.readings.to_owned(),
            )
        })
        .collect()
}

/// The one device of `model` read from `t`, with its id and its readings containing each of `has`.
fn reads(t: &Train, model: &str, id: &str, has: &[&str]) -> DeviceLine {
    assert!(
        decode(t).iter().any(|r| r.model == model),
        "{model}: {:#?}",
        decode(t)
    );
    let got = heard(t);
    let mine: Vec<&DeviceLine> = got.iter().filter(|d| d.0 == model).collect();
    assert_eq!(mine.len(), 1, "{model}: {got:?}");
    let d = mine[0].clone();
    assert_eq!(d.1, id, "{model}: {got:?}");
    for h in has {
        assert!(d.4.contains(h), "{model}: {h} in {}", d.4);
    }
    d
}

#[test]
fn itron_ert_scm_reads_at_the_published_rate() {
    // Id 12345678, ERT type 7, no tamper, consumption 4660.
    let t = enc::ert_scm(12_345_678, 7, 0, 0, 4660);
    // The stack starts the row with a 0 bit, so octet 0 (F9) does not read back; octets 1 to 11 are the example's.
    let r = rows(19, &t);
    let frame = [
        0x53, 0x00, 0x1C, 0x00, 0x12, 0x34, 0xBC, 0x61, 0x4E, 0xDE, 0x34,
    ];
    assert!(
        r.iter().any(|(n, b)| *n == 96 && b[1..12] == frame),
        "{r:02x?}"
    );
    reads(
        &t,
        "ERT-SCM",
        "12345678",
        &["Consumption 4660", "ERT type 7"],
    );
}

#[test]
fn itron_ert_idm() {
    let mut m = [0u8; 86];
    m[..11].copy_from_slice(&[
        0x1C, 0x5C, 0xC6, 0x04, 0x07, 0x00, 0xAC, 0x17, 0x1D, 0xF6, 0xBC,
    ]);
    m[25..29].copy_from_slice(&[0x00, 0x05, 0x30, 0x04]);
    m[82..86].copy_from_slice(&[0x01, 0xDC, 0xEA, 0xBA]);
    let t = enc::ert_idm(m);
    let r = &sets(20, &t)[0];
    let p = r
        .search(0, 0, &[0x16, 0xA3, 0x1C], 24)
        .expect("the frame sync");
    assert_eq!(r.len(0) - p, 720, "the 90 octets from the frame sync");
    assert_eq!(r.bits_u64(0, p + 704, 16), 0x5165, "the packet CRC");
    reads(
        &t,
        "ERT-IDM",
        "11278109",
        &[
            "ERT type 7",
            "Application version 4",
            "Interval count 246",
            "Last consumption count 339972",
            "Transmit time offset 476",
        ],
    );
}

/// The field values of the worked examples of the 36-bit Rubicson frame: id 0x5A, battery good, channel 2, 21.3 °C.
const RUBICSON: [u8; 5] = [0x5A, 0x90, 0xD5, 0xF0, 0x10];

#[test]
fn rubicson() {
    let t = enc::rubicson(0x5A, 1, true, 213);
    assert_eq!(count(2, &t, 36, &RUBICSON), 12);
    let d = reads(&t, "Rubicson-Temperature", "90", &["Temperature 21.3"]);
    assert_eq!((d.2.as_deref(), d.3), (Some("2"), Some(true)));
}

#[test]
fn solight() {
    let t = enc::solight(0x5A, 1, true, 213);
    assert_eq!(count(21, &t, 37, &RUBICSON), 12);
    let d = reads(&t, "Solight-TE44", "90", &["Temperature 21.30"]);
    assert_eq!((d.2.as_deref(), d.3), (Some("2"), Some(true)));
}

#[test]
fn generic_temperature() {
    let t = enc::generic_temp(0x5A, 2, 213);
    assert_eq!(count(3, &t, 24, &[0x5A, 0x83, 0x54]), 10);
    reads(
        &t,
        "Generic-Temperature",
        "90",
        &["Temperature 21.30", "Battery bits 2"],
    );
}

#[test]
fn lacrosse_tx141_b_bv2_bv3() {
    // The frame 5A 82 C9 00 as the stack slices it: every bit inverted.
    let t = enc::tx141b(32, 12, 0x5A, 0, true, 713);
    assert_eq!(count(4, &t, 32, &[0xA5, 0x7D, 0x36, 0xFF]), 12);
    let d = reads(&t, "LaCrosse-TX141B", "5a", &["Temperature 21.3"]);
    assert_eq!((d.2.as_deref(), d.3), (None, Some(true)));
    for (len, copies, model) in [(37, 12, "LaCrosse-TX141Bv2"), (33, 4, "LaCrosse-TX141Bv3")] {
        let t = enc::tx141b(len, copies, 0x5A, 2, false, 713);
        assert_eq!(
            count(4, &t, len, &[0xA5, 0xDD, 0x36, 0xFF]),
            copies,
            "{model}"
        );
        let d = reads(&t, model, "5a", &["Temperature 21.3"]);
        assert_eq!((d.2.as_deref(), d.3), (Some("2"), Some(false)), "{model}");
    }
}

#[test]
fn acurite_606tx() {
    let t = enc::acurite606(0x5A, 0, true, false, 213);
    assert_eq!(count(9, &t, 33, &[0x5A, 0x80, 0xD5, 0xA5, 0x00]), 8);
    let d = reads(&t, "Acurite-606TX", "90", &["Temperature 21.3"]);
    assert_eq!((d.2.as_deref(), d.3), (Some("1"), Some(true)));
}

#[test]
fn acurite_609txc() {
    let t = enc::acurite609(0x5A, true, 213, 45);
    assert_eq!(count(10, &t, 40, &[0x5A, 0x20, 0xD5, 0x2D, 0x7C]), 3);
    let d = reads(
        &t,
        "Acurite-609TXC",
        "90",
        &["Temperature 21.3", "Humidity 45"],
    );
    assert_eq!(d.3, Some(true));
}

#[test]
fn kedsum() {
    let t = enc::kedsum(0x5A, 0, true, 1600, 45, 0);
    let mut bits = vec![0u8, 0];
    for b in [0x5Au8, 0x80, 0x46, 0xD2, 0x05] {
        bits.extend((0..8).rev().map(|i| (b >> i) & 1));
    }
    assert_eq!(count(12, &t, 42, &pack(&bits)), 6);
    let d = reads(&t, "Kedsum-TH", "90", &["Temperature 21.1", "Humidity 45"]);
    assert_eq!((d.2.as_deref(), d.3), (Some("1"), Some(true)));
}

#[test]
fn globaltronics_gt_wt_02() {
    // The published row 34 00 ED 47 and 01100.
    let t = enc::gtwt02(0x34, 0, true, 237, 35);
    assert_eq!(count(13, &t, 37, &[0x34, 0x00, 0xED, 0x47, 0x60]), 3);
    let d = reads(&t, "GT-WT02", "52", &["Temperature 23.7", "Humidity 35"]);
    assert_eq!((d.2.as_deref(), d.3), (Some("1"), Some(true)));
}

#[test]
fn fine_offset_wh2_family() {
    use enc::Wh2Model as M;
    let t = enc::wh2(M::Wh2, 0x5A, 213, 45);
    assert_eq!(count(15, &t, 48, &[0xFF, 0x45, 0xA0, 0xD5, 0x2D, 0xA0]), 2);
    reads(
        &t,
        "Fineoffset-WH2",
        "90",
        &["Temperature 21.3", "Humidity 45"],
    );
    let t = enc::wh2(M::Telldus, 0x5A, 213, 45);
    reads(
        &t,
        "Fineoffset-TelldusProove",
        "90",
        &["Temperature 21.3", "Humidity 45"],
    );
    let t = enc::wh2(M::Wh2a, 0x5A, 213, 45);
    reads(
        &t,
        "Fineoffset-WH2A",
        "90",
        &["Temperature 21.3", "Humidity 45"],
    );
    let t = enc::wh2(M::Wh5, 0x5A, 613, 45);
    reads(
        &t,
        "Fineoffset-WH5",
        "90",
        &["Temperature 21.3", "Humidity 45"],
    );
    let t = enc::wh2(M::Tfa, 0x5A, 0x800 | 613, 0xFF);
    let d = reads(&t, "TFA-303225", "90", &["Temperature 21.3"]);
    assert_eq!(d.3, Some(false));
}

#[test]
fn thermopro_tp11() {
    // The published row DB 41 57 C2.
    let t = enc::tp11(0xDB4, 0x157);
    assert_eq!(count(16, &t, 32, &[0xDB, 0x41, 0x57, 0xC2]), 4);
    reads(&t, "Thermopro-TP11", "3508", &["Temperature 14.3"]);
}

#[test]
fn thermopro_tp12() {
    // The published row 38 73 21 BB 81 and the trailing 1.
    let t = enc::tp12(0x38, 0x273, 0x1BB);
    assert_eq!(count(17, &t, 41, &[0x38, 0x73, 0x21, 0xBB, 0x81, 0x80]), 16);
    assert_eq!(count(17, &t, 40, &[0x38, 0x73, 0x21, 0xBB, 0x81]), 1);
    reads(
        &t,
        "Thermopro-TP12",
        "56",
        &["Temperature 1 42.7", "Temperature 2 24.3"],
    );
}

#[test]
fn ft_004_b() {
    // The published copy 2F CF 24 78 21 and 110010, three times.
    let t = enc::ft004b(1054);
    let mut copy = Vec::new();
    for b in [0x2Fu8, 0xCF, 0x24, 0x78, 0x21] {
        copy.extend((0..8).rev().map(|i| (b >> i) & 1));
    }
    copy.extend_from_slice(&[1, 1, 0, 0, 1, 0]);
    let row: Vec<u8> = copy.iter().cycle().take(138).copied().collect();
    assert_eq!(count(18, &t, 138, &pack(&row)), 1);
    reads(&t, "FT-004B", "0", &["Temperature 12.7"]);
}

#[test]
fn honeywell() {
    let t = enc::honeywell(8, 0x12345, 0x80);
    // The frame ends with a 0 bit, whose space chip is not in the row: 47 bits of 81 23 45 80 8B 86.
    let b = manchester_after(22, &t, &[0x55, 0x55, 0x56], 24, ManchesterConv::Ieee);
    assert_eq!(b, [0x81, 0x23, 0x45, 0x80, 0x8B], "{b:02x?}");
    let d = reads(&t, "Honeywell-Security", "12345", &["Door open"]);
    assert_eq!((d.2.as_deref(), d.3), (Some("8"), Some(true)));
}

#[test]
fn kerui() {
    let t = enc::kerui(0x12345, 0xA);
    assert_eq!(count(23, &t, 25, &[0xED, 0xCB, 0xA5, 0x80]), 25);
    let d = reads(&t, "Kerui-Security", "12345", &["motion"]);
    assert_eq!(d.3, Some(true));
}

#[test]
fn springfield() {
    let t = enc::springfield(0x5A, 0, true, false, 215, 5);
    assert_eq!(count(24, &t, 37, &[0x5A, 0x00, 0xD7, 0x50, 0x00]), 3);
    assert_eq!(count(24, &t, 36, &[0x5A, 0x00, 0xD7, 0x50, 0x00]), 1);
    let d = reads(
        &t,
        "Springfield-Soil",
        "90",
        &["Temperature 21.5", "Moisture 50"],
    );
    assert_eq!((d.2.as_deref(), d.3), (Some("1"), Some(true)));
}

#[test]
fn pacific_pmv_107j() {
    let t = enc::pmv107j(0xABC_DEF1, 0x08, 129, 65);
    // The preamble chips 1111110, then the first 16 chips of the data.
    let r = &sets(35, &t)[0];
    assert_eq!(
        r.bits_u64(0, 0, 23),
        0b111_1110_1101_0010_1101_0011,
        "{:023b}",
        r.bits_u64(0, 0, 23)
    );
    assert_eq!(r.len(0), 7 + 136);
    let d = reads(
        &t,
        "PMV-107J",
        "0abcdef1",
        &["Pressure 220.7", "Temperature 25"],
    );
    assert_eq!(d.3, Some(true));
}

#[test]
fn hyundai_kia_vdo() {
    let t = enc::hyundai([0x20, 0x12, 0x34, 0x56, 0x78, 0x01, 0xA0, 0x4B, 0x00]);
    // The space chip that starts the preamble is not in the row.
    let b = manchester_after(
        39,
        &t,
        &[0xAA, 0xAA, 0xAA, 0xAC],
        31,
        ManchesterConv::Thomas,
    );
    assert!(
        b.starts_with(&[0x20, 0x12, 0x34, 0x56, 0x78, 0x01, 0xA0, 0x4B, 0x00, 0x1A]),
        "{b:02x?}"
    );
    reads(
        &t,
        "Hyundai-VDO",
        "12345678",
        &["Pressure 220.0", "Temperature 25", "Repeat 1"],
    );
}

#[test]
fn abarth_124_vdo() {
    let t = enc::abarth([0x12, 0x34, 0x56, 0x78, 0x00, 0xA0, 0x4B, 0x00]);
    let b = manchester_after(40, &t, &[0x55, 0x55, 0x56], 24, ManchesterConv::Thomas);
    assert!(
        b.starts_with(&[0x12, 0x34, 0x56, 0x78, 0x00, 0xA0, 0x4B, 0x00, 0xE3]),
        "{b:02x?}"
    );
    assert!((151..=209).contains(&sets(40, &t)[0].len(0)));
    reads(
        &t,
        "Abarth-124Spider",
        "12345678",
        &["Pressure 220.8", "Temperature 25"],
    );
}

/// The number of devices that the stack reads from `t` at 1 Msps, at a peak of 0.5, in white noise of power `sigma2` per
/// sample from `seed`, with 50 ms of noise before and after (no train when `t` is `None`).
fn heard_in_noise(t: Option<&Train>, sigma2: f64, seed: u64, seconds: f64) -> usize {
    let s = Station {
        stack: &ISM,
        set: &[],
        rate: 1e6,
        hz: 433.92e6,
        tune: 0.0,
    };
    let pad = (0.05 * s.rate) as usize;
    let mut x = vec![Complex32::default(); pad];
    if let Some(t) = t {
        x.extend(train_samples(t, s.rate, 0.0).iter().map(|z| z * 0.5));
    }
    x.resize(
        x.len().max((seconds * s.rate) as usize) + pad,
        Complex32::default(),
    );
    let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
    for v in &mut x {
        *v += rng.complex_normal(sigma2);
    }
    let (rx, _) = s.receive(&x, 0.0);
    ISM_DEVICES
        .rows::<IsmDeviceRow>(Owner::results(rx.as_ref()))
        .count()
}

/// The whole-channel retry in noise: the ERT SCM burst at 15 dB SNR (mark power over the noise in 200 kHz) reads in at
/// least 9 of 10 trials, and noise alone at the same level gives no reading in 10 trials of 1 s.
#[test]
fn itron_ert_scm_reads_in_noise_and_noise_reads_as_nothing() {
    let t = enc::ert_scm(12_345_678, 7, 0, 0, 4660);
    let sigma2 = 0.25 / (10f64.powf(1.5) * 0.2);
    let read = (0..10)
        .filter(|&k| heard_in_noise(Some(&t), sigma2, 100 + k, 0.0) == 1)
        .count();
    assert!(read >= 9, "{read} of 10");
    let false_reads: usize = (0..10)
        .map(|k| heard_in_noise(None, sigma2, 200 + k, 1.0))
        .sum();
    assert_eq!(false_reads, 0);
}
