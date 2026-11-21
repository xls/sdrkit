use sdrkit_core::Choice;
use sdrkit_proto_link::hdlc::{Line, Receiver, Transmitter, AX25_MAX, AX25_MIN};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
use sdrkit_util_bits::seq;

fn frame(n: usize, seed: u8) -> Vec<u8> {
    (0..n)
        .map(|i| (i as u8).wrapping_mul(37).wrapping_add(seed) | if i % 7 == 0 { 0xFF } else { 0 })
        .collect()
}

fn decode(line: Line, bits: &[u8]) -> Vec<Vec<u8>> {
    let mut rx = Receiver::new(line, 1, 400).unwrap();
    let mut got = Vec::new();
    rx.push_bits(bits, |f| got.push(f.to_vec()));
    got
}

#[test]
fn the_fcs_is_crc16_ibm_sdlc_with_check_value_0x906e() {
    // ISO/IEC 13239 clause 4.2.5 FCS: the catalogue check value of "123456789".
    assert_eq!(CRC16_IBM_SDLC.checksum(b"123456789"), 0x906E);
}

#[test]
fn nrzi_line_bits_of_a_flag_and_a_known_frame() {
    // A flag 0x7E is sent 0 1 1 1 1 1 1 0 (least significant bit first). In NRZI from level 0 it is 1 1 1 1 1 1 1 0.
    let mut tx = Transmitter::new(Line::Nrzi);
    let mut bits = Vec::new();
    tx.flags(1, &mut bits);
    assert_eq!(bits, [1, 1, 1, 1, 1, 1, 1, 0]);
    // NRZ: frame 0x01 with FCS. FCS of [0x01] is CRC16_IBM_SDLC, sent low byte first, framed by flags.
    let mut tx = Transmitter::new(Line::Nrz);
    let mut bits = Vec::new();
    tx.encode(&[&[0x01]], 1, &mut bits);
    let fcs = CRC16_IBM_SDLC.checksum(&[0x01]);
    let mut want = vec![0, 1, 1, 1, 1, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0];
    let mut ones = 0;
    for k in 0..16 {
        let b = ((fcs >> k) & 1) as u8;
        want.push(b);
        ones = if b == 1 { ones + 1 } else { 0 };
        if ones == 5 {
            want.push(0);
            ones = 0;
        }
    }
    want.extend_from_slice(&[0, 1, 1, 1, 1, 1, 1, 0]);
    assert_eq!(bits, want);
}

#[test]
fn the_g3ruh_scrambler_has_the_impulse_response_of_x17_x12_1() {
    // 1 / (1 + x^12 + x^17): the output is y[n] = x[n] + y[n - 12] + y[n - 17] (the self-synchronising scrambler of G3RUH).
    let mut s = seq::g3ruh(0);
    let mut b = vec![0u8; 40];
    b[0] = 1;
    s.scramble(&mut b);
    let ones: Vec<usize> = b
        .iter()
        .enumerate()
        .filter(|(_, &v)| v == 1)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(ones, [0, 12, 17, 24, 34, 36]);
}

#[test]
fn every_line_code_round_trips_frames_sharing_flags() {
    let a = frame(40, 1);
    let b = frame(17, 9);
    for line in [Line::Nrz, Line::Nrzi, Line::G3ruh] {
        let mut tx = Transmitter::new(line);
        let mut bits = Vec::new();
        tx.encode(&[&a, &b], 24, &mut bits);
        tx.encode(&[&a], 1, &mut bits);
        assert_eq!(
            decode(line, &bits),
            [a.clone(), b.clone(), a.clone()],
            "{line:?}"
        );
    }
}

#[test]
fn nrzi_and_g3ruh_ignore_the_polarity_of_the_line() {
    let a = frame(30, 3);
    for line in [Line::Nrzi, Line::G3ruh] {
        let mut tx = Transmitter::new(line);
        let mut bits = Vec::new();
        tx.encode(&[&a], 24, &mut bits);
        let inverted: Vec<u8> = bits.iter().map(|b| b ^ 1).collect();
        assert_eq!(
            decode(line, &inverted),
            std::slice::from_ref(&a),
            "{line:?}"
        );
    }
}

#[test]
fn seven_ones_abort_a_frame_and_bad_fcs_and_lengths_are_counted() {
    let a = frame(30, 5);
    let mut tx = Transmitter::new(Line::Nrz);
    let mut bits = Vec::new();
    tx.encode(&[&a], 2, &mut bits);
    // Abort: seven ones in the middle of the frame.
    let mut aborted = bits.clone();
    for b in &mut aborted[40..47] {
        *b = 1;
    }
    let mut rx = Receiver::new(Line::Nrz, 1, 400).unwrap();
    let mut n = 0;
    rx.push_bits(&aborted, |_| n += 1);
    assert_eq!(n, 0);
    assert_eq!(rx.stats().aborted, 1);
    // A flipped bit: a bad FCS.
    let mut bad = bits.clone();
    bad[60] ^= 1;
    let mut rx = Receiver::new(Line::Nrz, 1, 400).unwrap();
    rx.push_bits(&bad, |_| n += 1);
    assert_eq!(n, 0);
    assert_eq!(rx.stats().bad_fcs + rx.stats().aborted, 1);
    // Lengths count without the FCS: a 30-byte frame passes 30..30 and fails 31..400 and 1..29.
    for (min, max, ok) in [(30, 30, true), (31, 400, false), (1, 29, false)] {
        let mut rx = Receiver::new(Line::Nrz, min, max).unwrap();
        let mut got = 0;
        rx.push_bits(&bits, |_| got += 1);
        assert_eq!(got == 1, ok, "{min} {max}");
    }
    assert!(Receiver::new(Line::Nrz, 10, 5).is_err());
    let r = Receiver::ax25(Line::Nrzi);
    assert_eq!(r.line(), Line::Nrzi);
    assert_eq!((AX25_MIN, AX25_MAX), (15, 400));
    assert_eq!(Line::CHOICES[usize::from(Line::G3ruh.index())].key, "g3ruh");
    assert_eq!(Line::from_index(3), None);
}

#[test]
fn the_receiver_does_not_allocate_per_bit() {
    let a = frame(200, 7);
    let mut tx = Transmitter::new(Line::G3ruh);
    let mut bits = Vec::new();
    tx.encode(&[&a, &a, &a], 8, &mut bits);
    let mut rx = Receiver::new(Line::G3ruh, 1, 400).unwrap();
    let mut n = 0;
    let allocs = crate::allocations(|| {
        for _ in 0..4 {
            rx.push_bits(&bits, |_| n += 1);
        }
    });
    assert_eq!(allocs, 0);
    assert_eq!(n, 12);
}

#[test]
fn frames_without_an_fcs_are_bit_stuffed_between_flags() {
    // AX100 Mode 6 sends its Reed-Solomon word in an HDLC frame with no FCS.
    let a = vec![0xFF, 0x7E, 0x00, 0x3F];
    let mut tx = Transmitter::new(Line::Nrz);
    let mut bits = Vec::new();
    tx.encode_without_fcs(&[&a], 1, &mut bits);
    assert_eq!(&bits[..8], &[0, 1, 1, 1, 1, 1, 1, 0]);
    assert_eq!(&bits[bits.len() - 8..], &[0, 1, 1, 1, 1, 1, 1, 0]);
    let mut raw = Vec::new();
    sdrkit_util_bits::hdlc::unstuff_bits(&bits[8..bits.len() - 8], &mut raw).unwrap();
    let mut bytes = Vec::new();
    sdrkit_util_bits::pack::pack(
        &raw,
        sdrkit_util_bits::BitOrder::Lsb,
        sdrkit_util_bits::Pad::Zero,
        &mut bytes,
    );
    assert_eq!(bytes, a);
}

/// A frame sent with the FCS of `good` but the bytes of `sent`, on `line`, as line bits.
fn sent_with_fcs_of(line: Line, sent: &[u8], good: &[u8]) -> Vec<u8> {
    let fcs = CRC16_IBM_SDLC.checksum(good).to_le_bytes();
    let mut body = sent.to_vec();
    body.extend_from_slice(&fcs);
    let mut tx = Transmitter::new(line);
    let mut bits = Vec::new();
    tx.encode_without_fcs(&[&body], 4, &mut bits);
    bits
}

#[test]
fn a_frame_comes_with_the_fcs_it_carried_good_or_bad() {
    use sdrkit_util_bits::hdlc::FrameEnd;
    // ISO/IEC 13239 4.2.5: the FCS of "123456789" is 0x906E, sent low byte first.
    for line in [Line::Nrz, Line::Nrzi, Line::G3ruh] {
        let mut tx = Transmitter::new(line);
        let mut bits = Vec::new();
        tx.encode(&[b"123456789"], 4, &mut bits);
        let mut rx = Receiver::new(line, 1, 400).unwrap().deliver_bad(true);
        let mut ends = Vec::new();
        for &b in &bits {
            if let Some(e) = rx.push(b) {
                ends.push(e);
                assert_eq!(rx.frame_with_fcs(), b"123456789\x6e\x90", "{line:?}");
            }
        }
        assert_eq!(
            ends,
            [FrameEnd {
                len: 9,
                fcs_ok: true
            }]
        );
        // The same bytes with one bit changed: reported bad, with the FCS of the bytes that were meant.
        let mut wrong = *b"123456789";
        wrong[3] ^= 0x10;
        let bits = sent_with_fcs_of(line, &wrong, b"123456789");
        let mut ends = Vec::new();
        for &b in &bits {
            if let Some(e) = rx.push(b) {
                ends.push(e);
                assert_eq!(rx.frame(), wrong);
                assert_eq!(&rx.frame_with_fcs()[..9], wrong);
                assert_eq!(&rx.frame_with_fcs()[9..], [0x6e, 0x90]);
            }
        }
        assert_eq!(
            ends,
            [FrameEnd {
                len: 9,
                fcs_ok: false
            }],
            "{line:?}"
        );
        assert_eq!(rx.stats().bad_fcs, 1);
    }
    // Without the option, a bad frame is not reported and nothing is kept.
    let mut rx = Receiver::new(Line::Nrzi, 1, 400).unwrap();
    let bits = sent_with_fcs_of(Line::Nrzi, b"12345678X", b"123456789");
    assert!(bits.iter().all(|&b| rx.push(b).is_none()));
    assert!(rx.frame_with_fcs().is_empty());
}

#[test]
fn repair_finds_every_single_and_adjacent_pair_error_and_only_that() {
    use sdrkit_proto_link::hdlc::{repair, repair_ax25, Repair};
    // The generator x^16 + x^12 + x^5 + 1 is (x + 1) p(x) with p(x) primitive of period 32767, so within a frame shorter than
    // 32767 bits each error of one bit or two adjacent bits has its own syndrome and the repair returns the frame that was sent.
    let good =
        sdrkit_proto_link::ax25::Frame::from_tnc2("N0CALL-9>APRS,WIDE1-1:!4903.50N/07201.75W-fix")
            .unwrap();
    let mut sent = Vec::new();
    good.encode(&mut sent).unwrap();
    let mut with_fcs = sent.clone();
    with_fcs.extend_from_slice(&CRC16_IBM_SDLC.checksum(&sent).to_le_bytes());
    let bits = 8 * with_fcs.len();
    for width in [1usize, 2] {
        for i in 0..=bits - width {
            let mut b = with_fcs.clone();
            for k in 0..width {
                b[(i + k) / 8] ^= 1 << ((i + k) % 8);
            }
            let r = repair(&mut b, &CRC16_IBM_SDLC, 2, |_| true);
            assert_eq!(
                r,
                Some(Repair {
                    bits: width as u8,
                    at: i
                }),
                "width {width} at {i}"
            );
            assert_eq!(b, with_fcs);
        }
    }
    // With `max_bits` 1 a pair is not tried.
    let mut b = with_fcs.clone();
    b[5] ^= 0b11;
    assert_eq!(repair(&mut b, &CRC16_IBM_SDLC, 1, |_| true), None);
    assert_eq!(repair_ax25(&mut b, 2), Some(Repair { bits: 2, at: 40 }));
    // The AX.25 test: bytes that are not an AX.25 frame are not given back, even when the FCS passes after a flip.
    let mut junk: Vec<u8> = (0..30u8).map(|i| i.wrapping_mul(91)).collect();
    let fcs = CRC16_IBM_SDLC.checksum(&junk).to_le_bytes();
    junk.extend_from_slice(&fcs);
    junk[2] ^= 4;
    let before = junk.clone();
    assert_eq!(repair_ax25(&mut junk, 2), None);
    assert_eq!(junk, before);
    assert!(repair(&mut junk, &CRC16_IBM_SDLC, 2, |_| true).is_some());
    assert_eq!(repair(&mut [1u8, 2], &CRC16_IBM_SDLC, 2, |_| true), None);
}
