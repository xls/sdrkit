//! Tests of the line-code slicers with frames laid out as the device documentation describes them.

use crate::common::{bits_of, train};
use sdrkit_proto_ookdev::{encode, BitRows, Coding, Error, Modulation, Slicer, Timing, Train};

fn slice_all(t: &Timing, tr: &Train) -> Vec<BitRows> {
    let mut s = Slicer::new();
    let mut v = Vec::new();
    s.slice(t, tr, |r| v.push(r.clone())).unwrap();
    v
}

/// A pulse-position weather sensor (Nexus layout: 500 µs marks, a 1000 µs space is 0, 2000 µs is 1, 4000 µs between the copies
/// of the 36-bit frame). Timing: PPM 1000 / 2000, reset 5000, gap 3000.
#[test]
fn ppm_rows_per_copy() {
    let frame = &bits_of(&[0x91, 0x80, 0xD5, 0x0F, 0x30])[..36];
    let mut pairs = Vec::new();
    for _ in 0..3 {
        for &b in frame {
            pairs.push((500.0, if b == 1 { 2000.0 } else { 1000.0 }));
        }
        pairs.push((500.0, 4000.0));
    }
    let t = Timing::new(Coding::Ppm, 1000.0, 2000.0, 5000.0).with_gap(3000.0);
    let got = slice_all(&t, &train(Modulation::Ook, 250e3, &pairs));
    assert_eq!(got.len(), 1);
    let rows = &got[0];
    assert_eq!(rows.num_rows(), 3);
    for r in 0..3 {
        assert_eq!(rows.len(r), 36);
        assert_eq!(rows.bits_u64(r, 0, 36), 0x9180D50F3);
    }
}

/// EV1527 encoder datasheet frame: with the oscillator period T, a sync is 1 T high and 31 T low, a 0 is 1 T high and 3 T low,
/// a 1 is 3 T high and 1 T low. The PWM slicer reads a short mark as 1 (464 / 1404, reset 1800,
/// tolerance 200), so each buffer holds the complement of the 24 code bits and then the next frame's sync mark.
#[test]
fn pwm_ev1527_frames() {
    let t_us = 464.0f32;
    let code = 0x5A3C96u32;
    let mut pairs = Vec::new();
    for _ in 0..3 {
        pairs.push((t_us, 31.0 * t_us));
        for k in 0..24 {
            pairs.push(if (code >> (23 - k)) & 1 == 1 {
                (3.0 * t_us, t_us)
            } else {
                (t_us, 3.0 * t_us)
            });
        }
    }
    let t = Timing::new(Coding::Pwm, 464.0, 1404.0, 1800.0).with_tolerance(200.0);
    let got = slice_all(&t, &train(Modulation::Ook, 1e6, &pairs));
    // The first sync alone, two frames with the following sync, and the last frame.
    let lens: Vec<usize> = got.iter().map(|r| r.len(0)).collect();
    assert_eq!(lens, vec![1, 25, 25, 24]);
    for r in &got[1..] {
        assert_eq!(r.bits_u64(0, 0, 24) as u32, !code & 0xFF_FFFF);
    }
    assert_eq!(got[1].bit(0, 24), 1);
}

/// A PWM sensor with a sync mark (LaCrosse TX141 layout: 208 / 417, reset 1700, gap 625, sync 833):
/// windows split at the midpoints between the sorted widths. A sync mark starts a new row.
#[test]
fn pwm_sync_marks_split_rows() {
    let mut pairs = Vec::new();
    for _ in 0..2 {
        pairs.push((833.0, 208.0));
        for k in 0..8 {
            pairs.push(if k % 2 == 0 {
                (208.0, 417.0)
            } else {
                (417.0, 208.0)
            });
        }
    }
    let t = Timing::new(Coding::Pwm, 208.0, 417.0, 1700.0)
        .with_gap(625.0)
        .with_sync(833.0);
    let got = slice_all(&t, &train(Modulation::Ook, 1e6, &pairs));
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].num_rows(), 2);
    assert_eq!(got[0].row(0).bytes, &[0xAA]);
    assert_eq!(got[0].row(1).bytes, &[0xAA]);
}

/// NRZ at 52 µs per bit (the VDO TPMS rate) from a transmitter 2 % slow: the bit width comes from the
/// 0xAA preamble, so every bit of the frame is read, and the zeros at the end of an FSK carrier are data.
#[test]
fn nrz_measures_the_bit_width() {
    let bytes = [0xAA, 0xAA, 0xAA, 0xAA, 0x2D, 0xD4, 0x12, 0x34, 0x56, 0x80];
    let mut tr = Train::new(Modulation::Fsk, 500e3);
    encode::nrz(&bits_of(&bytes), 52.0 * 1.02, &mut tr);
    let t = Timing::new(Coding::Nrz, 52.0, 52.0, 1000.0);
    let got = slice_all(&t, &tr);
    assert_eq!(got.len(), 1);
    let rows = &got[0];
    assert!((rows.bit_us - 53.04).abs() < 0.3, "{}", rows.bit_us);
    assert_eq!(rows.len(0), 80);
    let mut out = [0u8; 10];
    rows.extract(0, 0, 80, &mut out).unwrap();
    assert_eq!(out, bytes);
    // The width test refuses a protocol at twice the bit width.
    let other = Timing::new(Coding::Nrz, 104.0, 104.0, 2000.0).with_width_check();
    assert!(slice_all(&other, &tr).is_empty());
    assert_eq!(
        slice_all(&t.with_width_check(), &tr)[0].bits_u64(0, 48, 16),
        0x1234
    );
}

/// Without a preamble, NRZ measures the width over the single and double runs when there are more than 20.
#[test]
fn nrz_width_from_runs() {
    // Runs of one and two bits, no run of 12 single bits.
    let bits: Vec<u8> = [1u8, 0, 0, 1, 1, 0]
        .iter()
        .copied()
        .cycle()
        .take(120)
        .collect();
    let mut tr = Train::new(Modulation::Fsk, 1e6);
    encode::nrz(&bits, 101.0, &mut tr);
    let got = slice_all(&Timing::new(Coding::Nrz, 100.0, 100.0, 1000.0), &tr);
    assert!((got[0].bit_us - 101.0).abs() < 0.5, "{}", got[0].bit_us);
}

/// Manchester with a zero first bit, half-bit 100 µs: 0 1 1 0 is low-high, high-low, high-low, low-high. The leading low half
/// is idle, so the train starts with the high half of the first 0.
#[test]
fn manchester_zero_bit() {
    let t = Timing::new(Coding::ManchesterZero, 100.0, 0.0, 1000.0);
    let tr = train(
        Modulation::Ook,
        1e6,
        &[(200.0, 100.0), (100.0, 200.0), (100.0, 5000.0)],
    );
    let got = slice_all(&t, &tr);
    assert_eq!(got[0].bits_u64(0, 0, 4), 0b0110);
}

/// The last space of an OOK train is the end of the transmission: it never adds a bit, whatever its length.
#[test]
fn last_ook_space_is_not_data() {
    let t = Timing::new(Coding::Ppm, 1000.0, 2000.0, 5000.0);
    let tr = train(
        Modulation::Ook,
        1e6,
        &[(500.0, 1000.0), (500.0, 2000.0), (500.0, 2000.0)],
    );
    let got = slice_all(&t, &tr);
    assert_eq!(got[0].len(0), 2);
    // The same train as FSK reads the last space as a bit.
    let mut f = tr.clone();
    f.modulation = Modulation::Fsk;
    assert_eq!(slice_all(&t, &f)[0].len(0), 3);
}

#[test]
fn bad_input_is_refused() {
    let mut s = Slicer::new();
    let t = Timing::new(Coding::Pwm, 100.0, 200.0, 1000.0);
    let mut bad = train(Modulation::Ook, 1e6, &[(100.0, 100.0)]);
    bad.pulses[0].mark = f32::NAN;
    assert_eq!(s.slice(&t, &bad, |_| {}), Err(Error::BadTrain));
    bad.pulses[0].mark = -1.0;
    assert_eq!(s.slice(&t, &bad, |_| {}), Err(Error::BadTrain));
    let ok = train(Modulation::Ook, 1e6, &[(100.0, 100.0)]);
    let no_long = Timing::new(Coding::Pwm, 100.0, 0.0, 1000.0);
    assert_eq!(s.slice(&no_long, &ok, |_| {}), Err(Error::BadTiming));
    let mut zero_rate = ok.clone();
    zero_rate.rate_hz = 0.0;
    assert_eq!(s.slice(&t, &zero_rate, |_| {}), Err(Error::BadTrain));
}

/// Every coding: bits through `encode` and back through the slicer.
#[test]
fn encode_then_slice() {
    let bits = bits_of(&[0xC5, 0x3A]);
    let mut tr = Train::new(Modulation::Ook, 1e6);
    encode::pwm(&bits, (300.0, 900.0), (900.0, 300.0), &mut tr);
    let got = slice_all(&Timing::new(Coding::Pwm, 300.0, 900.0, 5000.0), &tr);
    assert_eq!(got[0].bits_u64(0, 0, 16), 0xC53A);

    let mut tr = Train::new(Modulation::Ook, 1e6);
    encode::ppm(&bits, 400.0, 800.0, 1600.0, &mut tr);
    tr.pulses.push(sdrkit_proto_ookdev::Pulse::new(400.0, 0.0));
    let got = slice_all(&Timing::new(Coding::Ppm, 800.0, 1600.0, 5000.0), &tr);
    assert_eq!(got[0].bits_u64(0, 0, 16), 0xC53A);

    let mut tr = Train::new(Modulation::Ook, 1e6);
    let mut chips = Vec::new();
    // A leading 0 bit: its low half is idle and is skipped.
    let mut b0 = vec![0u8];
    b0.extend_from_slice(&bits);
    let skipped = encode::manchester(
        &b0,
        sdrkit_util_bits::line::ManchesterConv::Thomas,
        250.0,
        &mut chips,
        &mut tr,
    );
    assert_eq!(skipped, 1);
    let got = slice_all(
        &Timing::new(Coding::ManchesterZero, 250.0, 0.0, 5000.0),
        &tr,
    );
    assert_eq!(got[0].bits_u64(0, 1, 16), 0xC53A);
}
