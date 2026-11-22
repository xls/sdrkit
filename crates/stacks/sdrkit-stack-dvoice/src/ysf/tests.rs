//! The tables, worked examples and self-checks of the System Fusion voice specification, and the frame layouts.

use super::*;
use sdrkit_codec_mbe::{codeword_channel_bits, codewords, Coder, Decoder, FRAME};

/// Bits of a hexadecimal string, first bit most significant.
fn hex_bits(s: &str, n: usize) -> Vec<u8> {
    s.chars()
        .flat_map(|c| {
            let v = c.to_digit(16).unwrap() as u8;
            (0..4).rev().map(move |k| (v >> k) & 1)
        })
        .take(n)
        .collect()
}

/// Bits of a string of 0 and 1.
fn bin(s: &str) -> Vec<u8> {
    s.bytes().map(|c| c - b'0').collect()
}

/// Soft values of hard bits, 1 positive.
fn soft(b: &[u8]) -> Vec<f32> {
    b.iter().map(|&b| if b != 0 { 1.0 } else { -1.0 }).collect()
}

/// A simple bit sequence for round trips.
fn pattern(n: usize, seed: usize) -> Vec<u8> {
    (0..n).map(|i| (((i + seed) * 2_654_435_761) >> 7 & 1) as u8).collect()
}

/// Table 4.2: the code word bits (word, bit) of the 72 channel bits of a V/D mode 1 VCH, in the order sent.
const VD1_TABLE: &str = "0.23 0.5 1.10 2.3 0.22 0.4 1.9 2.2 0.21 0.3 1.8 2.1 0.20 0.2 1.7 2.0 0.19 0.1 1.6 3.13 \
    0.18 0.0 1.5 3.12 0.17 1.22 1.4 3.11 0.16 1.21 1.3 3.10 0.15 1.20 1.2 3.9 0.14 1.19 1.1 3.8 0.13 1.18 1.0 3.7 \
    0.12 1.17 2.10 3.6 0.11 1.16 2.9 3.5 0.10 1.15 2.8 3.4 0.9 1.14 2.7 3.3 0.8 1.13 2.6 3.2 0.7 1.12 2.5 3.1 \
    0.6 1.11 2.4 3.0";

/// Table 6.2: the code word bits (word, bit) of the 144 channel bits of a voice full-rate VCH, in the order sent.
const FR_TABLE: &str = "0.22 1.21 2.20 3.19 4.10 5.1 1.20 0.21 3.18 2.19 5.0 4.9 0.20 1.19 2.18 3.17 4.8 6.14 \
    1.18 0.19 3.16 2.17 6.13 4.7 0.18 1.17 2.16 3.15 4.6 6.12 1.16 0.17 3.14 2.15 6.11 4.5 0.16 1.15 2.14 3.13 4.4 6.10 \
    1.14 0.15 3.12 2.13 6.9 4.3 0.14 1.13 2.12 3.11 4.2 6.8 1.12 0.13 3.10 2.11 6.7 4.1 0.12 1.11 2.10 3.9 4.0 6.6 \
    1.10 0.11 3.8 2.9 6.5 5.14 0.10 1.9 2.8 3.7 5.13 6.4 1.8 0.9 3.6 2.7 6.3 5.12 0.8 1.7 2.6 3.5 5.11 6.2 \
    1.6 0.7 3.4 2.5 6.1 5.10 0.6 1.5 2.4 3.3 5.9 6.0 1.4 0.5 3.2 2.3 7.6 5.8 0.4 1.3 2.2 3.1 5.7 7.5 \
    1.2 0.3 3.0 2.1 7.4 5.6 0.2 1.1 2.0 4.14 5.5 7.3 1.0 0.1 4.13 3.22 7.2 5.4 0.0 2.22 3.21 4.12 5.3 7.1 \
    2.21 1.22 4.11 3.20 7.0 5.2";

/// The (word, bit) entries of a table.
fn entries(t: &str) -> Vec<(usize, usize)> {
    t.split_whitespace()
        .map(|e| {
            let (w, b) = e.split_once('.').unwrap();
            (w.parse().unwrap(), b.parse().unwrap())
        })
        .collect()
}

/// The code words of `ch` by table `t` with word lengths `lens`, each as a string of bits, its top bit first.
fn split(t: &str, lens: &[usize], ch: &[u8]) -> Vec<String> {
    let mut w: Vec<Vec<u8>> = lens.iter().map(|&n| vec![0; n]).collect();
    for (&(k, b), &x) in entries(t).iter().zip(ch) {
        w[k][lens[k] - 1 - b] = x;
    }
    w.iter()
        .map(|v| v.iter().map(|&b| (b'0' + b) as char).collect())
        .collect()
}

const VD1_LENS: [usize; 4] = [24, 23, 11, 14];
const FR_LENS: [usize; 8] = [23, 23, 23, 23, 15, 15, 15, 7];

#[test]
fn fich_fields_sit_at_the_bits_of_the_standard() {
    let f = Fich {
        dt: 2,
        sc: 127,
        ..Default::default()
    };
    let b = f.to_bits();
    assert_eq!(b.len(), 32);
    // DT is bits 9, 8: positions 22 and 23 of the 32.
    assert_eq!(&b[22..24], &[1, 0]);
    assert!(b[25..32].iter().all(|&x| x == 1), "{b:?}");
    assert!(b[..22].iter().all(|&x| x == 0));
    let one = |p: usize| {
        let mut r = vec![0u8; 32];
        r[p] = 1;
        Fich::from_bits(&r)
    };
    assert_eq!(one(22).dt, 2);
    assert_eq!(one(23).dt, 1);
    assert!(one(17).dev);
    assert_eq!(one(18).mr, 4);
    assert!(one(21).voip);
    assert!(one(24).sql);
    assert_eq!(one(25).sc, 64);
    assert_eq!(one(16), Fich::default());
}

#[test]
fn fich_interleave_moves_dibits_by_5_by_20() {
    let c: Vec<u8> = (0..200).map(|i| i as u8).collect();
    let sent = interleave(&c);
    for d in 0..100 {
        for b in 0..2 {
            assert_eq!(sent[2 * (20 * (d % 5) + d / 5) + b] as usize, 2 * d + b);
        }
    }
    // Coded bit 1 is sent as bit 1, coded dibit 1 as dibit 20.
    assert_eq!(sent[1], 1);
    assert_eq!(sent[40], 2);
}

#[test]
fn the_long_dch_unit_moves_dibits_by_9_by_20() {
    let c: Vec<u16> = (0..360).collect();
    let mut sent = vec![0u16; 360];
    LONG.apply(&c, &mut sent).unwrap();
    for j in 0..180 {
        let d = 9 * (j % 20) + j / 20;
        assert_eq!(sent[2 * j] as usize, 2 * d);
        assert_eq!(sent[2 * j + 1] as usize, 2 * d + 1);
    }
}

#[test]
fn the_whitening_sequence_has_its_first_bits_and_period_511() {
    let mut w = [0u8; 160];
    whitening().apply_bits(&mut w);
    assert_eq!(
        &w[..104],
        &bin(concat!(
            "10010011110101110101000100100001100111000010111101101100",
            "110100001110111100001111111110000011110111110001"
        ))[..]
    );
    assert_eq!(&w[..], &hex_bits("93D751219C2F6CD0EF0FF83DF1732094ED1E7CD8", 160)[..]);
    let r = Lfsr::recurrence(9, 0x11, 0x1C9);
    assert_eq!(r.period(1000), Some(511));
}

#[test]
fn dch_data_is_whitened_before_the_crc() {
    for n in [10, 20] {
        let s = soft(&encode_dch(&vec![0; n]));
        let c = deinterleave(&s).unwrap();
        let mut conv = channel();
        let (b, _) = conv.decode(&c, s.len() / 2, true).unwrap();
        let w = hex_bits("93D751219C2F6CD0EF0FF83DF1732094ED1E7CD8", 8 * n);
        assert_eq!(&b[..8 * n], &w[..], "{n} bytes");
        assert!(CRC16_GSM.verify_bits(&b[..8 * n + 16]));
    }
}

#[test]
fn fich_and_dch_round_trip() {
    let f = Fich {
        fi: 1,
        cs: 2,
        cm: 3,
        bn: 0,
        bt: 1,
        fn_: 5,
        ft: 6,
        dev: true,
        mr: 3,
        voip: true,
        dt: 2,
        sql: true,
        sc: 99,
    };
    let mut s = soft(&encode_fich(&f));
    s[11] = -s[11];
    assert_eq!(decode_fich(&mut channel(), &s).unwrap().0, f);
    for d in [b"HELLO 1234".to_vec(), b"TWENTY BYTES OF TEXT".to_vec()] {
        let mut s = soft(&encode_dch(&d));
        s[7] = -s[7];
        let (got, fixed) = decode_dch(&mut channel(), &s).unwrap();
        assert_eq!((got, fixed), (d, 1));
    }
}

#[test]
fn the_voice_interleave_tables_are_permutations() {
    for (t, lens) in [(VD1_TABLE, &VD1_LENS[..]), (FR_TABLE, &FR_LENS[..])] {
        let e = entries(t);
        assert_eq!(e.len(), lens.iter().sum::<usize>());
        let mut seen = std::collections::HashSet::new();
        for &(w, b) in &e {
            assert!(b < lens[w] && seen.insert((w, b)), "({w}, {b})");
        }
    }
    let mut seen = [false; 104];
    for i in 0..104 {
        let t = VD2_INTERLEAVE.dest(i).unwrap();
        assert_eq!(t, 4 * (i % 26) + i / 26);
        seen[t] = true;
    }
    assert!(seen.iter().all(|&s| s));
    // Entries of the full table of 5.3: channel position t carries S.
    for (t, s) in [(1, 26), (27, 84), (53, 39), (77, 45), (99, 102), (103, 103)] {
        assert_eq!(VD2_INTERLEAVE.src(t), Some(s));
    }
}

#[test]
fn the_v_d_mode_1_table_is_the_half_rate_code_word_layout() {
    // Each channel bit lands in the `codewords` payload at the place of its code word bit.
    for (p, &(w, b)) in entries(VD1_TABLE).iter().enumerate() {
        let mut ch = [0u8; 72];
        ch[p] = 1;
        let cw = codewords(&ch);
        let at = 24 * w + (VD1_LENS[w] - 1 - b);
        assert_eq!(cw[at / 8], 0x80 >> (at % 8), "channel bit {p}");
        assert_eq!(cw.iter().map(|x| x.count_ones()).sum::<u32>(), 1);
    }
}

/// Example 8.1: the 72 channel bits of a V/D mode 1 VCH.
fn vd1_example() -> Vec<u8> {
    hex_bits("6DA1536FA2121A9FDC", 72)
}

/// Example 8.2: the 104 channel bits of a V/D mode 2 VCH and VeCH.
fn vd2_example() -> [u8; 104] {
    hex_bits("80238F523E396BA968583AB5EF", 104).try_into().unwrap()
}

/// The 49 speech bits of examples 8.1 and 8.2.
const SPEECH_49: &str = "0110000110001001011010011010100010111010010101110";

#[test]
fn v_d_mode_1_example_splits_into_its_code_words_and_decodes_cleanly() {
    let ch = vd1_example();
    let w = split(VD1_TABLE, &VD1_LENS, &ch);
    assert_eq!(
        w,
        [
            "011000011000011111110010",
            "11000000011110100111110",
            "10101000101",
            "11010010101110"
        ]
    );
    // The `codewords` payload carries the same words, each from the top of its 24-bit row.
    let cw = codewords(&ch);
    let row = |r: usize, n: usize| -> String {
        (0..n)
            .map(|j| {
                let b = 24 * r + j;
                (b'0' + ((cw[b / 8] >> (7 - b % 8)) & 1)) as char
            })
            .collect()
    };
    for (r, &n) in VD1_LENS.iter().enumerate() {
        assert_eq!(row(r, n), w[r]);
    }
    let mut out = [0i16; FRAME];
    let st = Decoder::new(Coder::HalfRate).decode(&codeword_channel_bits(&cw), &mut out);
    assert_eq!(st.errors, 0);
}

#[test]
fn v_d_mode_2_example_gives_its_49_speech_bits() {
    let (info, errors) = vd2_speech(&vd2_example());
    let mut want = bin(SPEECH_49);
    want.resize(56, 0);
    assert_eq!(&info[..], &bits_to_bytes(&want)[..]);
    assert_eq!(info, [0x61, 0x89, 0x69, 0xA8, 0xBA, 0x57, 0x00]);
    assert_eq!(errors, 0);
    assert!(!vd2_lost(errors));
    // One wrong channel bit: the first copy of speech bit 0. The vote keeps the bits; one triple differs.
    let mut ch = vd2_example();
    ch[0] ^= 1;
    assert_eq!(vd2_speech(&ch), (info, 1));
}

#[test]
fn the_v_d_mode_examples_give_the_same_speech_through_the_decoder() {
    let mut a = [0i16; FRAME];
    let mut b = [0i16; FRAME];
    let sa = Decoder::new(Coder::HalfRate).decode(&vd1_example(), &mut a);
    let speech = bin(SPEECH_49);
    let sb = Decoder::new(Coder::HalfRate).decode_speech(&speech, &mut b);
    assert_eq!((sa.errors, sa.kind), (0, sb.kind));
    assert_eq!(a, b);
    // The V/D mode 2 payload through the plug-in gives the same samples.
    let (info, _) = vd2_speech(&vd2_example());
    let mut d = crate::mbe::SpeechDecoder::new(Coder::HalfRate);
    let mut pcm = sdrkit_core::codec::Pcm::default();
    sdrkit_core::codec::AudioCodec::decode(&mut d, &info, false, &mut pcm).unwrap();
    assert_eq!(&pcm.samples[..], &b[..]);
}

#[test]
fn voice_full_rate_example_splits_into_its_code_words_and_decodes_cleanly() {
    let ch = hex_bits("6B709E86BE87205FF90916F42F708EB926A3", 144);
    let w = split(FR_TABLE, &FR_LENS, &ch);
    assert_eq!(
        w,
        [
            "01011010001101010100110",
            "01100011000110011010000",
            "11110100111110001111011",
            "01000010100001100000001",
            "100011100111011",
            "111101110101101",
            "011101001000101",
            "1010101"
        ]
    );
    // The payload is the channel bits as sent; the full-rate decoder finds no errors and b0 = 90.
    let cb = crate::voice::channel_bits::<18>(&ch);
    assert_eq!(bytes_to_bits(&cb), ch);
    let mut dec = Decoder::new(Coder::FullRate);
    let mut out = [0i16; FRAME];
    assert_eq!(dec.decode(&ch, &mut out).errors, 0);
    let f0 = dec.params().f0_hz();
    assert!((f0 - 16_000.0 / (90.0 + 39.5)).abs() < 0.1, "{f0} Hz");
}

#[test]
fn voice_full_rate_example_gives_its_88_speech_bits_through_the_decoder() {
    let ch = hex_bits("6B709E86BE87205FF90916F42F708EB926A3", 144);
    let speech = bin(concat!(
        "010110100011",
        "000011110000",
        "110000111100",
        "100110011001",
        "01010101010",
        "10101010101",
        "00100100011",
        "1010101"
    ));
    assert_eq!(speech.len(), 88);
    let (mut a, mut b) = ([0i16; FRAME], [0i16; FRAME]);
    let sa = Decoder::new(Coder::FullRate).decode(&ch, &mut a);
    let sb = Decoder::new(Coder::FullRate).decode_speech(&speech, &mut b);
    assert_eq!((sa.errors, sa.kind), (0, sb.kind));
    assert_eq!(a, b);
}

#[test]
fn the_full_rate_encoder_frames_have_a_clean_first_word_by_table_6_2() {
    let g = SmallCode::golay_23_12();
    for f in crate::voice::test_speech_full().iter().take(10) {
        let w = split(FR_TABLE, &FR_LENS, f);
        let c0 = u128::from_str_radix(&w[0], 2).unwrap();
        assert_eq!(g.decode(c0), Some((c0, 0)));
    }
}

#[test]
fn speech_bits_round_trip_through_each_voice_type() {
    for seed in 0..20 {
        let s: [u8; 49] = pattern(49, seed).try_into().unwrap();
        let (info, errors) = vd2_speech(&vd2_channel(&s));
        let mut want = s.to_vec();
        want.resize(56, 0);
        assert_eq!((&info[..], errors), (&bits_to_bytes(&want)[..], 0));
        let ch = pattern(72, seed);
        assert_eq!(codeword_channel_bits(&codewords(&ch)).to_vec(), ch);
        let ch = pattern(144, seed);
        assert_eq!(bytes_to_bits(&crate::voice::channel_bits::<18>(&ch)), ch);
    }
}

#[test]
fn a_clean_v_d_mode_2_unit_is_unanimous_with_a_zero_last_bit() {
    for s in crate::voice::test_speech_half_bits() {
        let ch = vd2_channel(s);
        let mut x = [0u8; 104];
        VD2_INTERLEAVE.invert(&ch, &mut x).unwrap();
        whitening().apply_bits(&mut x);
        assert_eq!((x[103], vd2_errors(&x)), (0, 0));
    }
    // Random channel bits are lost.
    let ch: [u8; 104] = pattern(104, 3).try_into().unwrap();
    assert!(vd2_lost(vd2_speech(&ch).1));
}

#[test]
fn voice_and_data_sit_at_their_frame_bits() {
    let f = Fich {
        fi: 1,
        ..Default::default()
    };
    let mark = |n: usize| vec![1u8; n];
    // (layout, first frame bit of voice frame k, its length, the voice frames)
    for (kind, at, len, count) in [
        (Kind::Vd1, 312, 72, 5),
        (Kind::Vd2, 280, 104, 5),
        (Kind::VoiceFr, 240, 144, 5),
        (Kind::VoiceFrSub, 672, 144, 2),
    ] {
        let v: Vec<Vec<u8>> = (0..count).map(|_| mark(len)).collect();
        let b = frame_bits(&f, kind, &[], &v);
        assert_eq!(b.len(), 960);
        let ones: Vec<usize> = (240..960).filter(|&i| b[i] == 1).collect();
        let want: Vec<usize> = (0..count).flat_map(|k| at + 144 * k..at + 144 * k + len).collect();
        assert_eq!(ones, want, "{kind:?}");
    }
    // The data pieces.
    for (kind, unit, at, len) in [
        (Kind::Vd1, 0, 240, 72),
        (Kind::Vd2, 0, 240, 40),
        (Kind::HeaderTerminator, 0, 240, 72),
        (Kind::HeaderTerminator, 1, 312, 72),
        (Kind::DataFr, 1, 312, 72),
    ] {
        let part = kind.dch()[unit];
        let mut units = vec![vec![0u8; 5 * len]; 2];
        units[unit] = mark(5 * len);
        let b = frame_bits(&f, kind, &units, &[]);
        let ones: Vec<usize> = (240..960).filter(|&i| b[i] == 1).collect();
        let want: Vec<usize> = (0..5).flat_map(|k| at + 144 * k..at + 144 * k + len).collect();
        assert_eq!(ones, want, "{kind:?} unit {unit} {part:?}");
    }
    let b = frame_bits(&f, Kind::VoiceFrSub, &[mark(360)], &[]);
    assert!((240..600).all(|i| b[i] == 1) && (600..960).all(|i| b[i] == 0));
}

#[test]
fn the_voice_full_rate_sub_header_frame_follows_the_header_or_has_ft_1_fn_0() {
    let f = |fn_, ft| Fich {
        fi: 1,
        dt: 3,
        fn_,
        ft,
        ..Default::default()
    };
    assert_eq!(Kind::of(&f(0, 0), true), Some(Kind::VoiceFrSub));
    assert_eq!(Kind::of(&f(0, 1), false), Some(Kind::VoiceFrSub));
    assert_eq!(Kind::of(&f(0, 0), false), Some(Kind::VoiceFr));
    assert_eq!(Kind::of(&f(1, 1), false), Some(Kind::VoiceFr));
    let mut v = f(0, 0);
    v.dt = 2;
    assert_eq!(Kind::of(&v, true), Some(Kind::Vd2));
}

#[test]
fn call_signs_round_trip_through_the_header_units() {
    let c = Call {
        dest: "ALL".into(),
        src: "N0CALL".into(),
        down: "DOWNLINK".into(),
        up: "UPLINK".into(),
        ..Default::default()
    };
    let u = call_sign_units(&c);
    let mut got = Call::default();
    read_call_signs(&[Some(u[0].to_vec()), Some(u[1].to_vec())], &mut got);
    assert_eq!(
        (got.dest, got.src, got.down, got.up),
        (c.dest, c.src, c.down, c.up)
    );
}
